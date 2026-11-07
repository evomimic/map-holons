//! Reference-backed, sealed effective products for value, property and relationship descriptors.
use super::{accessor_helpers, OperatorDescriptor, ValueDescriptorKind};
use std::{
    collections::HashSet,
    sync::{Arc, Mutex},
};
use type_names::ToOperatorName;

use base_types::MapString;
use core_types::HolonError;
use type_names::{
    CoreHolonTypeName as H, CorePropertyTypeName as P, CoreRelationshipTypeName as R,
};

use super::{
    ancestors, inheritance::relationship_targets_in_lineage, resolve_core_descriptor,
    same_definition, Descriptor, EffectiveRelationshipMember,
};
use crate::{
    core_shared_objects::transactions::TransactionContext, HolonReference, ReadableHolon,
    WritableHolon,
};

/// A transaction-bound transient result. It contains references, not copied definitions.
/// Collections retain duplicates, ancestor-first ordering, and declaring origins.
///
/// # Prepared payload
///
/// - Shared: self-first `DerivedLineage` and `DerivedInstanceTypeKind` (an
///   optional index into that exact lineage), with private retirement metadata.
/// - HolonType: EffectiveHolonDescriptor specializes this handle with instance
///   properties/relationships, singular/plural navigation, Dances, Commands,
///   constraints, bindings, key rule, prepared policies and declaring origins.
/// - ValueType/PropertyType: inherited `DerivedConstraints` and
///   `DerivedValidationBindings`, with collection-aligned `DerivedOrigins`.
/// - ValueType: additionally `DerivedOperators` and `DerivedValueKind`; array
///   classification is a projection of the prepared native value kind.
/// - PropertyType: consumes constraints and bindings; the generic carrier's
///   other fields do not define additional PropertyType semantics.
///
/// - RelationshipType: direction and final membership cache policy, backed by the
///   same lineage. Declared definitional membership reuses; other directions use
///   inherited maximum age. Unused value/property collections are left empty.
///
/// One resolved Extends lineage supplies all products. Targets are not eagerly
/// materialized, and preparation does not evaluate constraints or validation.
/// Effective Instance TypeKind is represented by its anchor identity, independently
/// of `DerivedValueKind`. Registered saved results share the space Session token.
/// Mutable inputs share completed products only during frozen validation; direct
/// holders become obsolete when their assessment scope ends.
///
/// See map-dev-docs `docs/core/core-runtime/descriptors/descriptors-design-spec.md`,
/// "Derived payload inventory by descriptor kind"; the space Session lifecycle
/// belongs to `docs/core/core-runtime/space-session-design-spec.md`.
#[derive(Debug, Clone)]
pub struct DerivedDescriptor {
    holon: HolonReference,
    session: Option<Arc<crate::core_shared_objects::space_session::SpaceSession>>,
    validation: Option<Arc<crate::core_shared_objects::transactions::FrozenValidationToken>>,
}

/// Wrapper-private memoization. Only saved definitions and saved dependencies are retained;
/// mutable inputs consult their owner's frozen scope or are recomputed outside it.
/// Failed acquisition never populates the cell. This runtime state is not serialized.
#[derive(Debug, Clone)]
pub(crate) struct DerivedFields<D = DerivedDescriptor>(Arc<Mutex<Option<D>>>);

impl<D> Default for DerivedFields<D> {
    fn default() -> Self {
        Self(Arc::new(Mutex::new(None)))
    }
}

/// Shared acquisition and retirement contract for specialized descriptor products.
pub(crate) trait DerivedProduct: Clone {
    fn acquire_product(target: HolonReference) -> Result<Self, HolonError>;
    fn retained_result(&self) -> &DerivedDescriptor;
}

impl DerivedProduct for DerivedDescriptor {
    fn acquire_product(target: HolonReference) -> Result<Self, HolonError> {
        Self::acquire(target)
    }
    fn retained_result(&self) -> &DerivedDescriptor {
        self
    }
}

impl<D: DerivedProduct> DerivedFields<D> {
    pub(crate) fn acquire(&self, target: &HolonReference) -> Result<D, HolonError> {
        if matches!(target, HolonReference::Smart(_)) {
            let mut memo = self.0.lock().map_err(lock_error)?;
            if let Some(result) = memo.as_ref() {
                match result.retained_result().check_active() {
                    Ok(()) => return Ok(result.clone()),
                    Err(HolonError::ObsoleteSession) => *memo = None,
                    Err(error) => return Err(error),
                }
            }
        }
        let result = D::acquire_product(target.clone())?;
        if matches!(target, HolonReference::Smart(_))
            && result.retained_result().session.is_some()
            && result.retained_result().has_saved_dependencies()?
        {
            *self.0.lock().map_err(lock_error)? = Some(result.clone());
        }
        Ok(result)
    }

    /// Retry only retirement, including a reset between acquisition and product read.
    pub(crate) fn read<T>(
        &self,
        target: &HolonReference,
        read: impl Fn(&D) -> Result<T, HolonError>,
    ) -> Result<T, HolonError> {
        loop {
            let result = self.acquire(target).and_then(|derived| read(&derived));
            match result {
                Err(HolonError::ObsoleteSession) => continue,
                other => return other,
            }
        }
    }
}

/// Compact direction-specific policy, transported as part of the sealed derived result.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub(crate) struct DerivedRelationshipPolicy {
    pub direction: super::RelationshipDirection,
    pub policy: crate::RelationshipCachePolicy,
}

impl DerivedDescriptor {
    /// Wraps a complete result returned by the trusted construction service.
    pub fn from_constructed_holon(holon: HolonReference) -> Self {
        Self { holon, session: None, validation: None }
    }

    pub(crate) fn in_frozen_validation(
        mut self,
        token: Arc<crate::core_shared_objects::transactions::FrozenValidationToken>,
    ) -> Self {
        self.validation = Some(token);
        self
    }

    pub(crate) fn in_session(
        mut self,
        session: Arc<crate::core_shared_objects::space_session::SpaceSession>,
    ) -> Self {
        self.session = Some(session);
        self
    }

    pub(super) fn check_active(&self) -> Result<(), HolonError> {
        if let Some(validation) = &self.validation {
            validation.check_active()?;
        }
        if let Some(session) = &self.session {
            session.check_active()?;
        }
        self.holon.key()?;
        Ok(())
    }

    /// Trusted construction/registry/transport access; never an ordinary semantic API.
    pub(crate) fn holon(&self) -> &HolonReference {
        &self.holon
    }

    /// Acquires the products of this exact descriptor, not of its DescribedBy target.
    pub fn acquire(target: HolonReference) -> Result<Self, HolonError> {
        let context = if let Some(context) = target.transaction_context() {
            context
        } else {
            let space = target.space_read_handle();
            if let Some(registry) = space.descriptor_registry() {
                if let Some(result) = registry.get_derived(&target.holon_id()?)? {
                    return Ok(result);
                }
                registry.context()?
            } else {
                space.open_descriptor_construction_context()?
            }
        };
        let acquire = || {
            context
                .get_holon_service()
                .acquire_derived_descriptor_internal(&context, target.clone())
        };
        if matches!(target, HolonReference::Smart(_)) {
            acquire()
        } else {
            context.acquire_frozen_derived(&target, acquire)
        }
    }

    pub(super) fn read<T>(
        &self,
        read: impl FnOnce() -> Result<T, HolonError>,
    ) -> Result<T, HolonError> {
        self.check_active()?;
        let result = read();
        self.check_active()?;
        result
    }

    pub fn lineage(&self) -> Result<Vec<HolonReference>, HolonError> {
        self.read(|| self.lineage_unchecked())
    }
    pub fn effective_constraints(&self) -> Result<Vec<EffectiveRelationshipMember>, HolonError> {
        self.read(|| self.effective_constraints_unchecked())
    }
    pub fn effective_validation_bindings(
        &self,
    ) -> Result<Vec<EffectiveRelationshipMember>, HolonError> {
        self.read(|| self.effective_validation_bindings_unchecked())
    }
    pub fn supported_operators(&self) -> Result<Vec<OperatorDescriptor>, HolonError> {
        self.read(|| self.supported_operators_unchecked())
    }
    /// Returns the exact nearest Instance TypeKind anchor from prepared lineage metadata.
    /// TypeDescriptor itself has no anchor. No Extends edges are followed here.
    pub fn instance_type_kind(&self) -> Result<Option<HolonReference>, HolonError> {
        self.read(|| {
            let encoded = match self.holon.property_value(P::DerivedInstanceTypeKind)? {
                Some(base_types::BaseValue::StringValue(value)) => value.0,
                _ => {
                    return Err(HolonError::InvalidParameter(
                        "Missing derived Instance TypeKind".into(),
                    ))
                }
            };
            let index: Result<Option<usize>, HolonError> = serde_json::from_str(&encoded)
                .map_err(|e| HolonError::InvalidParameter(e.to_string()))?;
            index?
                .map(|index| {
                    self.lineage_unchecked()?.get(index).cloned().ok_or_else(|| {
                        HolonError::InvalidParameter("Derived kind anchor outside lineage".into())
                    })
                })
                .transpose()
        })
    }

    /// Reads only the prepared scalar; unrelated descriptor kinds carry no policy product.
    pub(crate) fn relationship_policy(&self) -> Result<DerivedRelationshipPolicy, HolonError> {
        self.read(|| match self.holon.property_value(P::DerivedRelationshipPolicy)? {
            Some(base_types::BaseValue::StringValue(value)) => {
                let prepared: Option<Result<DerivedRelationshipPolicy, HolonError>> =
                    serde_json::from_str(&value.0)
                        .map_err(|error| HolonError::InvalidParameter(error.to_string()))?;
                prepared.unwrap_or_else(|| {
                    Err(HolonError::WrongDescriptorKind {
                        expected: "DeclaredRelationshipType or InverseRelationshipType".into(),
                        found: "No relationship direction anchor".into(),
                        descriptor: self.holon.full_reference_id_string(),
                    })
                })
            }
            _ => Err(HolonError::InvalidParameter("Missing derived relationship policy".into())),
        })
    }

    pub fn value_kind(&self) -> Result<ValueDescriptorKind, HolonError> {
        self.read(|| self.value_kind_unchecked())
    }
    pub fn supports_operator(&self, op: &OperatorDescriptor) -> Result<bool, HolonError> {
        self.read(|| self.supports_operator_unchecked(op))
    }
    pub fn get_operator_by_name<N: ToOperatorName>(
        &self,
        name: N,
    ) -> Result<OperatorDescriptor, HolonError> {
        self.read(|| self.get_operator_by_name_unchecked(name))
    }
    pub fn supports_operator_by_name<N: ToOperatorName>(
        &self,
        name: N,
    ) -> Result<bool, HolonError> {
        self.read(|| self.supports_operator_by_name_unchecked(name))
    }

    /// Finds a saved-derived product in the installed space registry without walking Extends.
    pub fn find_in_context(
        context: &Arc<TransactionContext>,
        target: &HolonReference,
    ) -> Result<Option<Self>, HolonError> {
        if !matches!(target, HolonReference::Smart(_)) {
            return Ok(None);
        }
        match context.descriptor_registry() {
            Some(registry) => registry.get_derived(&target.holon_id()?),
            None => Ok(None),
        }
    }

    fn lineage_unchecked(&self) -> Result<Vec<HolonReference>, HolonError> {
        members(&self.holon, R::DerivedLineage)
    }

    pub(crate) fn has_saved_dependencies(&self) -> Result<bool, HolonError> {
        for (_, collection) in self.holon.all_related_holons()?.iter() {
            if collection
                .read()
                .map_err(lock_error)?
                .get_members()
                .iter()
                .any(|r| !matches!(r, HolonReference::Smart(_)))
            {
                return Ok(false);
            }
        }
        Ok(true)
    }

    /// Returns ancestor-first constraint occurrences with declaring origins.
    fn effective_constraints_unchecked(
        &self,
    ) -> Result<Vec<EffectiveRelationshipMember>, HolonError> {
        self.contributions(0)
    }
    /// Returns ancestor-first validation commitments without evaluating them.
    fn effective_validation_bindings_unchecked(
        &self,
    ) -> Result<Vec<EffectiveRelationshipMember>, HolonError> {
        self.contributions(1)
    }
    /// Returns operator declarations without loading their authoritative targets.
    fn supported_operators_unchecked(&self) -> Result<Vec<super::OperatorDescriptor>, HolonError> {
        Ok(members(&self.holon, R::DerivedOperators)?
            .into_iter()
            .map(super::OperatorDescriptor::from_holon)
            .collect())
    }

    /// Reads the classification computed with the original lineage, including semantic errors.
    /// Operational preparation failures never become published products.
    fn value_kind_unchecked(&self) -> Result<ValueDescriptorKind, HolonError> {
        match self.holon.property_value(P::DerivedValueKind)? {
            Some(base_types::BaseValue::StringValue(value)) => {
                let result: Result<ValueDescriptorKind, HolonError> =
                    serde_json::from_str(&value.0).map_err(|e| {
                        HolonError::InvalidParameter(format!("Invalid derived value kind: {e}"))
                    })?;
                result
            }
            _ => Err(HolonError::InvalidParameter("Missing derived value kind".into())),
        }
    }

    /// Tests exact afforded identity; specializing an operator does not afford it implicitly.
    fn supports_operator_unchecked(
        &self,
        operator: &OperatorDescriptor,
    ) -> Result<bool, HolonError> {
        for member in members(&self.holon, R::DerivedOperators)? {
            if same_definition(&member, operator.holon())? {
                return Ok(true);
            }
        }
        Ok(false)
    }

    /// Resolves an operator by semantic name, retaining duplicate-name diagnostics.
    fn get_operator_by_name_unchecked<N: ToOperatorName>(
        &self,
        operator_name: N,
    ) -> Result<OperatorDescriptor, HolonError> {
        let requested_name = operator_name.to_operator_name();
        let requested = requested_name.to_string();
        let mut seen = HashSet::new();
        let mut found = None;

        for operator_descriptor in self.supported_operators()? {
            let declaration_name = operator_descriptor.operator_name()?;
            let declaration_label = declaration_name.to_string();
            if !seen.insert(declaration_label.clone()) {
                return Err(HolonError::DuplicateInheritedDeclaration {
                    kind: "operator".to_string(),
                    name: declaration_label,
                    descriptor: accessor_helpers::descriptor_label(&self.source()?),
                });
            }
            if declaration_name == requested_name {
                found = Some(operator_descriptor);
            }
        }

        match found {
            Some(operator) => Ok(operator),
            None => Err(HolonError::DescriptorDeclarationNotFound {
                kind: "operator".into(),
                name: requested,
                descriptor: accessor_helpers::descriptor_label(&self.source()?),
            }),
        }
    }

    /// Checks the prepared affordances without another inheritance computation.
    fn supports_operator_by_name_unchecked<N: ToOperatorName>(
        &self,
        operator_name: N,
    ) -> Result<bool, HolonError> {
        let requested_name = operator_name.to_operator_name();
        for operator_descriptor in self.supported_operators()? {
            if operator_descriptor.operator_name()? == requested_name {
                return Ok(true);
            }
        }
        Ok(false)
    }

    fn source(&self) -> Result<HolonReference, HolonError> {
        self.lineage()?
            .into_iter()
            .next()
            .ok_or_else(|| HolonError::InvalidParameter("Missing derived source".into()))
    }

    fn contributions(&self, index: usize) -> Result<Vec<EffectiveRelationshipMember>, HolonError> {
        let origins: Vec<Vec<usize>> = match self.holon.property_value(P::DerivedOrigins)? {
            Some(base_types::BaseValue::StringValue(value)) => serde_json::from_str(&value.0)
                .map_err(|e| {
                    HolonError::InvalidParameter(format!("Invalid derived origins: {e}"))
                })?,
            _ => return Err(HolonError::InvalidParameter("Missing derived origins".into())),
        };
        let lineage = self.lineage()?;
        let targets = members(&self.holon, collections()[index].1.clone())?;
        let positions = origins.get(index).ok_or_else(|| {
            HolonError::InvalidParameter("Missing derived collection origins".into())
        })?;
        if targets.len() != positions.len() {
            return Err(HolonError::InvalidParameter(
                "Derived collection/origin length mismatch".into(),
            ));
        }
        targets
            .into_iter()
            .zip(positions)
            .map(|(member, position)| {
                let declared_on = lineage
                    .get(*position)
                    .ok_or_else(|| {
                        HolonError::InvalidParameter("Derived origin outside lineage".into())
                    })?
                    .clone();
                Ok(EffectiveRelationshipMember { member, declared_on })
            })
            .collect()
    }
}

/// One lineage computation supplies all products. No source validation, target hydration,
/// contribution holons, or duplicate normalization is performed during preparation.
pub fn construct_derived_descriptor(
    context: &Arc<TransactionContext>,
    target: HolonReference,
) -> Result<DerivedDescriptor, HolonError> {
    crate::core_shared_objects::read_observations::observe_attempt(
        "derived_builder_attempts",
        "derived_builder_successes",
        || build_derived_descriptor(context, target),
    )
}

fn build_derived_descriptor(
    context: &Arc<TransactionContext>,
    target: HolonReference,
) -> Result<DerivedDescriptor, HolonError> {
    let lineage = match context.frozen_descriptor_lineage(&target)? {
        Some(lineage) => lineage,
        None => ancestors(&target)?,
    };
    let (instance_kind, value_kind) = super::value_kind::prepare_kind_products(&target, &lineage)?;

    let relationship_policy = prepare_relationship_policy(&target, &lineage)?;
    let mut products = Vec::new();
    let mut origins = Vec::new();
    for (source, result) in collections() {
        // RelationshipType's supported product is policy. The common carrier's
        // value/property collections are unused, so do not read them speculatively.
        let contributions = if relationship_policy.is_some() {
            Vec::new()
        } else {
            relationship_targets_in_lineage(&lineage, source)?
        };
        let mut positions = Vec::new();
        let mut targets = Vec::new();
        for contribution in contributions {
            let mut origin = None;
            for (index, ancestor) in lineage.iter().enumerate() {
                if same_definition(ancestor, &contribution.declared_on)? {
                    origin = Some(index);
                    break;
                }
            }
            positions.push(origin.ok_or_else(|| {
                HolonError::InvalidState("Derived origin outside lineage".into())
            })?);
            targets.push(contribution.member);
        }
        products.push((result, targets));
        origins.push(positions);
    }
    let result_type =
        resolve_core_descriptor(context, &H::DerivedDescriptor.descriptor_key(&H::HolonType).0)?;
    let mut result = context.mutation().new_holon(None)?;
    result.add_related_references(R::DescribedBy, vec![result_type])?;
    if let Some(space) = context.get_space_holon()? {
        result.add_related_references(R::OwnedBy, vec![space])?;
    }
    result.with_property_value(
        P::DerivedInstanceTypeKind,
        MapString(
            serde_json::to_string(&instance_kind)
                .map_err(|e| HolonError::InvalidParameter(e.to_string()))?,
        ),
    )?;
    result.with_property_value(
        P::DerivedRelationshipPolicy,
        MapString(
            serde_json::to_string(&relationship_policy)
                .map_err(|error| HolonError::InvalidParameter(error.to_string()))?,
        ),
    )?;
    result.add_related_references(R::DerivedLineage, lineage)?;
    result.with_property_value(
        P::DerivedValueKind,
        MapString(
            serde_json::to_string(&value_kind)
                .map_err(|e| HolonError::InvalidParameter(e.to_string()))?,
        ),
    )?;

    for (relationship, targets) in products {
        result.add_related_references(relationship, targets)?;
    }
    result.with_property_value(
        P::DerivedOrigins,
        MapString(
            serde_json::to_string(&origins)
                .map_err(|e| HolonError::InvalidParameter(e.to_string()))?,
        ),
    )?;
    result.mark_as_immutable()?;
    Ok(DerivedDescriptor::from_constructed_holon(result.into()))
}

/// Uses the constructor's single lineage. Field read failures prevent publication;
/// readable semantic errors are retained so unrelated descriptor products remain usable.
fn prepare_relationship_policy(
    target: &HolonReference,
    lineage: &[HolonReference],
) -> Result<Option<Result<DerivedRelationshipPolicy, HolonError>>, HolonError> {
    use super::RelationshipDirection as Direction;
    use crate::RelationshipCachePolicy as Policy;
    let mut direction = None;
    for ancestor in lineage {
        match ancestor.property_value(P::TypeName)? {
            Some(base_types::BaseValue::StringValue(name))
                if name == H::DeclaredRelationshipType.as_holon_name() =>
            {
                direction = Some(Direction::Declared);
                break;
            }
            Some(base_types::BaseValue::StringValue(name))
                if name == H::InverseRelationshipType.as_holon_name() =>
            {
                direction = Some(Direction::Inverse);
                break;
            }
            _ => {}
        }
    }
    let Some(direction) = direction else {
        return Ok(None);
    };
    if direction == Direction::Declared {
        match target.property_value(P::IsDefinitional)? {
            Some(base_types::BaseValue::BooleanValue(value)) if value.0 => {
                return Ok(Some(Ok(DerivedRelationshipPolicy {
                    direction,
                    policy: Policy::Reuse,
                })));
            }
            Some(base_types::BaseValue::BooleanValue(_)) => {}
            Some(other) => {
                return Ok(Some(Err(HolonError::UnexpectedValueType(
                    format!("{other:?}"),
                    "Boolean".into(),
                ))))
            }
            None => return Ok(Some(Err(HolonError::EmptyField("IsDefinitional".into())))),
        }
    }
    Ok(Some(relationship_max_age(lineage)?.map(|age| DerivedRelationshipPolicy {
        direction,
        policy: if age == 0 { Policy::Fresh } else { Policy::MaxAgeMillis(age) },
    })))
}

/// Inherited optional age: explicit zero overrides an ancestor; omission resolves Fresh.
/// The outer result distinguishes failed reads from readable invalid schema values.
pub(crate) fn relationship_max_age(
    lineage: &[HolonReference],
) -> Result<Result<u64, HolonError>, HolonError> {
    for ancestor in lineage {
        match ancestor.property_value("MembershipCacheMaxAgeMillis")? {
            None => continue,
            Some(base_types::BaseValue::IntegerValue(value)) if value.0 >= 0 => {
                return Ok(Ok(value.0 as u64))
            }
            _ => {
                return Ok(Err(HolonError::InvalidParameter(
                    "MembershipCacheMaxAgeMillis must be a nonnegative integer".into(),
                )))
            }
        }
    }
    Ok(Ok(0))
}

fn collections() -> [(R, R); 3] {
    [
        (R::Constraints, R::DerivedConstraints),
        (R::ValidationBindings, R::DerivedValidationBindings),
        (R::AffordsOperator, R::DerivedOperators),
    ]
}
fn members(reference: &HolonReference, relationship: R) -> Result<Vec<HolonReference>, HolonError> {
    Ok(reference.related_holons(relationship)?.read().map_err(lock_error)?.get_members().clone())
}
fn lock_error(error: impl std::fmt::Display) -> HolonError {
    HolonError::FailedToAcquireLock(error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::descriptors::{
        test_support::{build_context, new_test_holon},
        PropertyDescriptor, ValueDescriptor,
    };

    #[test]
    fn prepared_collection_inventory_requires_commit_reset() {
        use type_names::ToRelationshipName;
        let source_id = core_types::LocalId(vec![91; 39]);
        for (source, _) in collections() {
            let name = source.to_relationship_name();
            let mut effects = crate::descriptors::DescriptorCommitEffects::default();
            effects.record_relationship_write(&source_id, &name);
            assert!(effects.reset_required(), "Untracked prepared input: {name}");
        }
    }

    #[test]
    fn relationship_preparation_does_not_read_unused_value_property_collections(
    ) -> Result<(), HolonError> {
        use crate::core_shared_objects::{holon::SavedHolon, read_observations::CacheReadProbe};
        use crate::descriptors::test_support::build_context_with_saved_holons;
        use type_names::ToPropertyName;
        let id = core_types::LocalId(vec![81; 39]);
        let context = build_context_with_saved_holons(
            vec![SavedHolon::new(
                id.clone(),
                core_types::PropertyMap::from([
                    (
                        "Key".to_property_name(),
                        base_types::BaseValue::StringValue(MapString("inverse".into())),
                    ),
                    (
                        "TypeName".to_property_name(),
                        base_types::BaseValue::StringValue(MapString(
                            "InverseRelationshipType".into(),
                        )),
                    ),
                ]),
                None,
                base_types::MapInteger(1),
            )],
            Default::default(),
        );
        let source = HolonReference::smart_from_id(
            context.space_read_handle(),
            core_types::HolonId::Local(id),
        );
        let probe = CacheReadProbe::start(true);
        let result = construct_derived_descriptor(&context, source)?;
        assert_eq!(result.relationship_policy()?.policy, crate::RelationshipCachePolicy::Fresh);
        for entry in probe.finish() {
            assert!(![R::Constraints, R::ValidationBindings, R::AffordsOperator].iter().any(
                |name| {
                    use type_names::ToRelationshipName;
                    entry.relationship == Some(name.to_relationship_name())
                }
            ));
        }
        Ok(())
    }

    #[test]
    fn prepared_kind_reads_do_not_resolve_the_lineage_targets() -> Result<(), HolonError> {
        let context = build_context();
        let mut result = context.mutation().new_holon(None)?;
        let unavailable = HolonReference::smart_from_id(
            context.space_read_handle(),
            core_types::HolonId::Local(core_types::LocalId(vec![99; 39])),
        );
        result.add_related_references(R::DerivedLineage, vec![unavailable])?;
        let expected: Result<ValueDescriptorKind, HolonError> =
            Ok(ValueDescriptorKind::BaseValue(base_types::BaseValueKind::String));
        result.with_property_value(
            P::DerivedValueKind,
            MapString(serde_json::to_string(&expected).unwrap()),
        )?;
        let descriptor = DerivedDescriptor::from_constructed_holon(result.into());
        assert_eq!(descriptor.value_kind()?, expected.unwrap());
        Ok(())
    }

    #[test]
    fn failed_acquisition_retries_and_mutable_edits_never_reuse_stale_products(
    ) -> Result<(), HolonError> {
        let context = build_context();
        let mut parent = new_test_holon(&context, "abstract-property")?;
        let mut child = new_test_holon(&context, "concrete-property")?;
        let first_constraint: HolonReference = new_test_holon(&context, "first-constraint")?.into();
        parent.add_related_holons(R::Constraints, vec![first_constraint.clone()])?;
        child.add_related_holons(R::Extends, vec![parent.clone().into()])?;
        parent.add_related_holons(R::Extends, vec![child.clone().into()])?;
        let property = PropertyDescriptor::from_holon(child.clone().into());
        assert!(matches!(property.effective_constraints(), Err(HolonError::CyclicExtends { .. })));
        parent.remove_related_holons(R::Extends, vec![child.clone().into()])?;
        assert_eq!(property.effective_constraints()?.len(), 1);
        let second_constraint: HolonReference =
            new_test_holon(&context, "second-constraint")?.into();
        child.add_related_holons(R::Constraints, vec![second_constraint])?;
        let changed = property.effective_constraints()?;
        assert_eq!(changed.len(), 2);
        assert_eq!(changed[0].member, first_constraint);
        let value = ValueDescriptor::from_holon(child.into());
        assert_eq!(value.effective_constraints()?.len(), 2);
        assert!(value.effective_validation_bindings()?.is_empty());
        Ok(())
    }

    #[test]
    fn frozen_validation_shares_mutable_products_and_retires_them() -> Result<(), HolonError> {
        let context = build_context();
        let transient = new_test_holon(&context, "transient-property")?;
        let staged =
            context.mutation().stage_new_holon(new_test_holon(&context, "staged-property")?)?;
        for mut target in [HolonReference::from(transient), HolonReference::from(staged)] {
            let first_constraint: HolonReference =
                context.mutation().stage_new_holon(new_test_holon(&context, "first")?)?.into();
            target.add_related_holons(R::Constraints, vec![first_constraint])?;
            let property = PropertyDescriptor::from_holon(target.clone());
            let retained = {
                let _scope = context.begin_frozen_validation()?;
                assert_eq!(property.effective_constraints()?.len(), 1);
                let first = DerivedDescriptor::acquire(target.clone())?;
                let allocated = context.lookup().transient_count()?;
                // Independent wrappers and direct acquisition select the same product.
                let independent = PropertyDescriptor::from_holon(target.clone());
                assert_eq!(independent.effective_constraints()?.len(), 1);
                assert_eq!(first.holon(), DerivedDescriptor::acquire(target.clone())?.holon());
                assert_eq!(context.lookup().transient_count()?, allocated);
                // Derived outputs and other transient output allocation remain permitted.
                new_test_holon(&context, "validation-output")?;
                assert!(context.begin_frozen_validation().is_err());
                first
            };
            assert!(matches!(retained.lineage(), Err(HolonError::ObsoleteSession)));
            let extra: HolonReference =
                context.mutation().stage_new_holon(new_test_holon(&context, "extra")?)?.into();
            target.add_related_holons(R::Constraints, vec![extra])?;
            assert_eq!(property.effective_constraints()?.len(), 2);
            {
                let _retry = context.begin_frozen_validation()?;
                let next = DerivedDescriptor::acquire(target.clone())?;
                assert_ne!(next.holon(), retained.holon());
                assert_eq!(next.effective_constraints()?.len(), 2);
            }
            // Outside validation, even unchanged inputs are acquired fresh.
            let first = DerivedDescriptor::acquire(target.clone())?;
            let second = DerivedDescriptor::acquire(target)?;
            assert_ne!(first.holon(), second.holon());
        }
        Ok(())
    }

    #[test]
    fn holon_type_products_share_the_frozen_scope_and_retry_after_edits() -> Result<(), HolonError>
    {
        let context = build_context();
        let transient = new_test_holon(&context, "frozen-holon-type")?;
        let staged =
            context.mutation().stage_new_holon(new_test_holon(&context, "staged-holon-type")?)?;
        for mut target in [HolonReference::from(transient), HolonReference::from(staged)] {
            let wrapper = super::super::HolonDescriptor::from_holon(target.clone());
            let selected = {
                let _scope = context.begin_frozen_validation()?;
                let first = wrapper.effective_descriptor()?;
                let allocated = context.lookup().transient_count()?;
                let independent = super::super::HolonDescriptor::from_holon(target.clone());
                assert_eq!(first.holon(), independent.effective_descriptor()?.holon());
                assert_eq!(context.lookup().transient_count()?, allocated);
                first
            };
            assert!(matches!(selected.instance_properties(), Err(HolonError::ObsoleteSession)));
            let constraint: HolonReference = context
                .mutation()
                .stage_new_holon(new_test_holon(&context, "extra-holon-constraint")?)?
                .into();
            target.add_related_holons(R::Constraints, vec![constraint])?;
            {
                let _retry = context.begin_frozen_validation()?;
                let next = wrapper.effective_descriptor()?;
                assert_ne!(next.holon(), selected.holon());
                assert_eq!(next.effective_constraints()?.len(), 1);
            }
            let fresh = wrapper.effective_descriptor()?;
            assert_ne!(fresh.holon(), wrapper.effective_descriptor()?.holon());
        }
        Ok(())
    }

    #[test]
    fn frozen_validation_error_exit_retires_results_and_allows_corrected_retry(
    ) -> Result<(), HolonError> {
        let context = build_context();
        let mut target: HolonReference = new_test_holon(&context, "property")?.into();
        let mut retained = None;
        let outcome = (|| -> Result<(), HolonError> {
            let _scope = context.begin_frozen_validation()?;
            retained = Some(DerivedDescriptor::acquire(target.clone())?);
            Err(HolonError::InvalidState("assessment failed".into()))
        })();
        assert!(outcome.is_err());
        assert!(matches!(retained.unwrap().lineage(), Err(HolonError::ObsoleteSession)));
        // A failed construction is not retained, and correction is observed next assessment.
        target.add_related_holons(R::Extends, vec![target.clone()])?;
        {
            let _scope = context.begin_frozen_validation()?;
            assert!(matches!(
                DerivedDescriptor::acquire(target.clone()),
                Err(HolonError::CyclicExtends { .. })
            ));
        }
        target.remove_related_holons(R::Extends, vec![target.clone()])?;
        let _retry = context.begin_frozen_validation()?;
        assert_eq!(DerivedDescriptor::acquire(target)?.lineage()?.len(), 1);
        Ok(())
    }

    #[test]
    fn diagnosed_frozen_lineage_is_not_walked_again_by_acquisition() -> Result<(), HolonError> {
        use crate::core_shared_objects::{holon::SavedHolon, read_observations::CacheReadProbe};
        use base_types::{BaseValue, MapInteger};
        use core_types::{HolonId, LocalId, PropertyMap, PropertyName};
        let id = HolonId::Local(LocalId(vec![42; 39]));
        let parent = SavedHolon::new(
            id.local_id().clone(),
            PropertyMap::from([
                (PropertyName("Key".into()), BaseValue::StringValue("TypeDescriptor".into())),
                (
                    PropertyName("DefinesInstanceTypeKind".into()),
                    BaseValue::BooleanValue(false.into()),
                ),
            ]),
            None,
            MapInteger(1),
        );
        let context = super::super::test_support::build_context_with_saved_holons(
            vec![parent],
            Default::default(),
        );
        let parent = HolonReference::smart_from_id(context.space_read_handle(), id);
        let mut child: HolonReference = new_test_holon(&context, "child")?.into();
        child.add_related_holons(R::Extends, vec![parent])?;
        let diagnosis = super::super::ExtendsLineageDiagnosis::assess(&child)?;
        let lineage = diagnosis.valid_lineage().expect("valid diagnosed lineage");
        let _scope = context.begin_frozen_validation()?;
        context.retain_frozen_descriptor_lineage(lineage)?;
        let probe = CacheReadProbe::start(true);
        let property = PropertyDescriptor::from_holon(child.clone());
        property.effective_constraints()?;
        ValueDescriptor::from_holon(child).effective_validation_bindings()?;
        let reads = probe.finish();
        assert!(
            !reads
                .iter()
                .any(|read| read.relationship.as_ref() == Some(&R::Extends.as_relationship_name())),
            "acquisition must reuse diagnosed lineage rather than reading Extends again"
        );
        Ok(())
    }
}
