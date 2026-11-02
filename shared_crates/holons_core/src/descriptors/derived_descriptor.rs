//! Reference-backed, sealed effective products for value and property descriptors.
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
/// - Shared: self-first `DerivedLineage`, inherited `DerivedConstraints` and
///   `DerivedValidationBindings`, collection-aligned `DerivedOrigins`, and
///   `DerivedInstanceTypeKind` (an optional index into that exact lineage).
/// - ValueType: additionally `DerivedOperators` and `DerivedValueKind`; array
///   classification is a projection of the prepared native value kind.
/// - PropertyType: consumes constraints and bindings; the generic carrier's
///   other fields do not define additional PropertyType semantics.
///
/// One resolved Extends lineage supplies all products. Targets are not eagerly
/// materialized, and preparation does not evaluate constraints or validation.
/// Effective Instance TypeKind is represented by its anchor identity, independently
/// of `DerivedValueKind`. Registered results share the space Session retirement
/// token; detached Commit results follow their own transaction lifetime.
///
/// See map-dev-docs `docs/core/core-runtime/descriptors/descriptors-design-spec.md`,
/// "Derived payload inventory by descriptor kind"; the space Session lifecycle
/// belongs to `docs/core/core-runtime/space-session-design-spec.md`.
#[derive(Debug, Clone)]
pub struct DerivedDescriptor {
    holon: HolonReference,
    session: Option<Arc<crate::core_shared_objects::space_session::SpaceSession>>,
}

/// Wrapper-private memoization. Only saved definitions and saved dependencies are retained;
/// mutable inputs are recomputed so edits through another reference cannot leave stale products.
/// Failed acquisition never populates the cell. This runtime state is not serialized.
#[derive(Debug, Clone, Default)]
pub(crate) struct DerivedFields(Arc<Mutex<Option<DerivedDescriptor>>>);

impl DerivedFields {
    pub(crate) fn acquire(&self, target: &HolonReference) -> Result<DerivedDescriptor, HolonError> {
        if matches!(target, HolonReference::Smart(_)) {
            let mut memo = self.0.lock().map_err(lock_error)?;
            if let Some(result) = memo.as_ref() {
                match result.check_active() {
                    Ok(()) => return Ok(result.clone()),
                    Err(HolonError::ObsoleteSession) => *memo = None,
                    Err(error) => return Err(error),
                }
            }
        }
        let result = DerivedDescriptor::acquire(target.clone())?;
        if matches!(target, HolonReference::Smart(_))
            && result.session.is_some()
            && result.has_saved_dependencies()?
        {
            *self.0.lock().map_err(lock_error)? = Some(result.clone());
        }
        Ok(result)
    }

    /// Retry only retirement, including a reset between acquisition and product read.
    pub(crate) fn read<T>(
        &self,
        target: &HolonReference,
        read: impl Fn(&DerivedDescriptor) -> Result<T, HolonError>,
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

impl DerivedDescriptor {
    /// Wraps a complete result returned by the trusted construction service.
    pub fn from_constructed_holon(holon: HolonReference) -> Self {
        Self { holon, session: None }
    }

    pub(crate) fn in_session(
        mut self,
        session: Arc<crate::core_shared_objects::space_session::SpaceSession>,
    ) -> Self {
        self.session = Some(session);
        self
    }

    fn check_active(&self) -> Result<(), HolonError> {
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
        context.get_holon_service().acquire_derived_descriptor_internal(&context, target)
    }

    fn read<T>(&self, read: impl FnOnce() -> Result<T, HolonError>) -> Result<T, HolonError> {
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

    /// Finds a saved-derived product in a transported Commit snapshot without walking Extends.
    pub fn find_in_context(
        context: &Arc<TransactionContext>,
        target: &HolonReference,
    ) -> Result<Option<Self>, HolonError> {
        if !matches!(target, HolonReference::Smart(_)) {
            return Ok(None);
        }
        let Some(root) = context.descriptor_registry_root()? else {
            return Ok(None);
        };
        for member in members(&root.into(), R::PreparedDerivedDescriptors)? {
            let result = Self::from_constructed_holon(member);
            if result.lineage()?.first().is_some_and(|first| first == target) {
                return Ok(Some(result));
            }
        }
        Ok(None)
    }

    /// Returns the source and its ancestors in self-first order, without traversal.
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
    let lineage = ancestors(&target)?;
    let (instance_kind, value_kind) = super::value_kind::prepare_kind_products(&target, &lineage)?;

    let mut products = Vec::new();
    let mut origins = Vec::new();
    for (source, result) in collections() {
        let contributions = relationship_targets_in_lineage(&lineage, source)?;
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
}
