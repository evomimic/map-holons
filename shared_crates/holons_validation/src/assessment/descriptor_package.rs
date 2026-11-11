//! Assessment-local effective contracts. Holon content remains in bound references/caches.
use crate::assessment::schema_view::SchemaWorkset;
use crate::assessment::support::targets;
use core_types::HolonError;
use holons_core::descriptors::{
    constraint_applies_to_lineage,
    value_constraints::{
        classify_constraint_type, interpret_constraint, ClassifiedConstraintType,
        ConfigurationDiagnosis, ConstraintConfiguration, ConstraintInterpretation,
        ConstraintParameters, ConstraintTypeDefect,
    },
};
use holons_core::{
    core_shared_objects::transactions::TransactionContext, ContractContributions,
    DefinitionIdentity, EffectiveRelationshipMember, HolonReference, ReadableHolon,
};
use std::{
    cell::Cell,
    collections::{BTreeMap, HashMap, HashSet, VecDeque},
    sync::Arc,
};
use type_names::CoreRelationshipTypeName as R;

/// Owned interpretation shared by all subjects reaching one exact constraint.
#[derive(Clone, Debug)]
pub(crate) enum PreparedConstraint {
    Defective { defect: ConstraintTypeDefect, describing_type: Option<HolonReference> },
    Interpreted { constraint_type: HolonReference, parameters: PreparedConstraintParameters },
}
impl PreparedConstraint {
    #[cfg(test)]
    pub(crate) fn parameters(&self) -> Option<&PreparedConstraintParameters> {
        match self {
            Self::Interpreted { parameters, .. } => Some(parameters),
            _ => None,
        }
    }
}

#[derive(Clone, Debug)]
pub(crate) enum PreparedConstraintParameters {
    Valid(ConstraintParameters),
    Invalid(Vec<ConfigurationDiagnosis>),
    Uninterpreted,
}

/// Applicability belongs to an attachment, not to the shared configured constraint.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum AttachmentApplicability {
    Applies,
    DoesNotApply,
    Blocked(String),
}

/// The root's effective contract retains kernel contribution ordering and provenance.
pub(crate) struct DescriptorPackage {
    pub root: HolonReference,
    pub contract: Result<Arc<ContractContributions>, HolonError>,
    consumed: Cell<bool>,
}

/// Package roots share exact prepared definitions.
/// Entries are discarded before another assessment or schema mutation.
pub(crate) struct DescriptorPackages {
    context: Arc<TransactionContext>,
    // Immutable products and their consumption metrics belong to exact definitions.
    packages: HashMap<DefinitionIdentity, DescriptorPackage>,
    definitions: HashMap<DefinitionIdentity, PreparedDefinition>,
    _content: Vec<holons_core::core_shared_objects::holon_cache::SavedHolonRetention>,
    preparation: crate::PreparationMetrics,
    properties: HashMap<DefinitionIdentity, PropertyFacts>,
    pub(crate) constraints: HashMap<DefinitionIdentity, PreparedConstraint>,
    pub(crate) attachment_applicability:
        HashMap<(DefinitionIdentity, DefinitionIdentity), AttachmentApplicability>,
    value_kinds: HashMap<DefinitionIdentity, Result<holons_core::ValueDescriptorKind, HolonError>>,
    pub(crate) diagnoses: Arc<crate::assessment::structural_diagnoses::StructuralDiagnoses>,
}

/// Shared kernel products; owned constraint parameters are prepared separately.
struct PreparedDefinition {
    reference: HolonReference,
    // Provenance distinguishes authored structural defects from acquisition/read failures.
    lineage_defect: Option<HolonError>,
    constraints_consumed: Cell<bool>,
    bindings_consumed: Cell<bool>,
    bindings_prepared: Cell<bool>,
    contract: Result<Arc<ContractContributions>, HolonError>,
    schema: Result<Arc<holons_core::descriptors::ComputedEffectiveSchema>, HolonError>,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
enum PreparationRole {
    Governing,
    Defined,
    Declaration,
    Property,
    PropertyMember,
    Binding,
    Value,
    RelationshipMember,
    Constraint,
    ConstraintType,
    Schema,
    Evidence,
    Retained,
    DefaultMeta,
    Obligations,
}
impl PreparationRole {
    fn label(self) -> &'static str {
        match self {
            Self::Governing => "Governing",
            Self::Defined => "Defined",
            Self::Declaration => "Declaration",
            Self::Property => "Property",
            Self::PropertyMember => "PropertyMember",
            Self::Binding => "Binding",
            Self::Value => "Value",
            Self::RelationshipMember => "RelationshipMember",
            Self::Constraint => "Constraint",
            Self::ConstraintType => "ConstraintType",
            Self::Schema => "Schema",
            Self::Evidence => "Evidence",
            Self::Retained => "Retained",
            Self::DefaultMeta => "DefaultMeta",
            Self::Obligations => "Obligations",
        }
    }
}
/// A usable declaration or an authored defect explaining the scoped guard.
pub(crate) type Requiredness = Result<bool, String>;

struct PropertyFacts {
    required: Requiredness,
    value_type: Option<HolonReference>,
}

impl DescriptorPackages {
    #[cfg(test)]
    pub fn construct(
        context: &Arc<TransactionContext>,
        roots: &[HolonReference],
        subjects: &[HolonReference],
        workset: &SchemaWorkset,
    ) -> Result<Self, HolonError> {
        Self::construct_with_observer(
            context,
            roots,
            subjects,
            workset,
            Arc::new(crate::assessment::structural_diagnoses::StructuralDiagnoses::new(context)),
            &mut |_| {},
        )
    }

    pub fn construct_with_observer(
        context: &Arc<TransactionContext>,
        roots: &[HolonReference],
        subjects: &[HolonReference],
        workset: &SchemaWorkset,
        diagnoses: Arc<crate::assessment::structural_diagnoses::StructuralDiagnoses>,
        observer: &mut impl FnMut(crate::AssessmentEvent),
    ) -> Result<Self, HolonError> {
        observer(crate::AssessmentEvent::Phase(crate::AssessmentPhase::PackageDiscovery));
        let mut edge_demand: BTreeMap<(&str, String), (usize, usize)> = BTreeMap::new();
        let mut packages = HashMap::new();
        for root in roots {
            let identity = DefinitionIdentity::for_reference(root, context)?;
            packages.entry(identity.clone()).or_insert_with(|| DescriptorPackage {
                root: identity.definition_reference(context, root),
                contract: Err(HolonError::InvalidState(
                    "Descriptor package is still constructing".into(),
                )
                .into()),
                consumed: Cell::new(false),
            });
        }

        use PreparationRole::*;
        let mut pending: VecDeque<_> = roots.iter().cloned().map(|r| (r, Governing)).collect();
        // Subjects are declaration seeds, not automatically effective-contract roots.
        pending.extend(subjects.iter().cloned().map(|r| (r, Declaration)));
        pending.extend(workset.schemas.iter().map(|s| (s.schema.clone(), Schema)));
        let mut seen = HashSet::new();
        let mut content_seen = HashSet::new();
        let mut prefetched = HashSet::new();
        let mut content = Vec::new();
        let mut definitions = HashMap::new();
        let mut properties = HashMap::new();
        let mut value_kinds = HashMap::new();
        let mut constraint_references = HashMap::new();
        while let Some((reference, role)) = pending.pop_front() {
            let identity = DefinitionIdentity::for_reference(&reference, context)?;
            let reference = identity.definition_reference(context, &reference);
            if !seen.insert((identity.clone(), role)) {
                continue;
            }
            observer(crate::AssessmentEvent::PreparationVisitStarted);
            let distinct_content = content_seen.insert(identity.clone());
            if distinct_content {
                if let HolonReference::Smart(saved) = &reference {
                    content.push(crate::assessment::phases::observe_operation(
                        observer,
                        "ContentRetention",
                        "SavedContent",
                        || saved.retain_content(),
                    )?);
                }
            }
            if role == Constraint {
                constraint_references.insert(identity.clone(), reference.clone());
            }
            let mut edges = Vec::new();
            match role {
                Declaration => {
                    let diagnosis = diagnoses.get(&reference)?;
                    if let Some(lineage) = diagnosis.valid_lineage() {
                        if lineage.contains_role(type_names::CoreDefinitionRole::TypeDescriptor) {
                            pending.push_back((reference.clone(), Defined));
                            if let Some(parent) = lineage.members().get(1) {
                                pending.push_back((parent.clone(), Obligations));
                            }
                        }
                    }
                    // Unused staged constraints still have declaration consumers.
                    if let holons_core::DescribingTypeResolution::Unique(governor) =
                        holons_core::resolve_describing_type(&reference)?
                    {
                        if diagnoses
                            .get(&governor)?
                            .encountered_role(type_names::CoreDefinitionRole::ConstraintType)
                        {
                            pending.push_back((reference.clone(), Constraint));
                        }
                    }
                }
                Schema => edges.extend([R::DependsOn, R::DescribedBy]),
                Constraint | Binding => edges.push(R::DescribedBy),
                PropertyMember => {
                    diagnoses.get(&reference)?;
                    edges.push(R::ValueType);
                }
                ConstraintType => {
                    diagnoses.get(&reference)?;
                    edges.extend([
                        R::Extends,
                        R::ApplicableToDescriptorTypes,
                        R::InstanceProperties,
                    ]);
                }
                RelationshipMember => {
                    diagnoses.get(&reference)?;
                    edges.extend([R::Extends, R::SourceType, R::TargetType]);
                }
                Evidence => {
                    diagnoses.get(&reference)?;
                }
                Property => edges.extend([R::DescribedBy, R::ValueType]),
                _ => {}
            }
            let needs_product =
                matches!(role, Governing | Defined | Property | Value | DefaultMeta | Obligations);
            if needs_product && !definitions.contains_key(&identity) {
                let prepared = crate::assessment::phases::observe_operation(
                    observer,
                    "RegistryLookup",
                    "PreparedDefinition",
                    || {
                        holons_core::descriptors::EffectiveHolonDescriptor::find_in_context(
                            context, &reference,
                        )
                    },
                )?;
                let mut lineage_defect = None;
                let schema = match prepared {
                    Some(descriptor) => crate::assessment::phases::observe_operation(
                        observer,
                        "SavedRestoration",
                        "PreparedDefinition",
                        || {
                            holons_core::descriptors::ComputedEffectiveSchema::from_prepared(
                                &descriptor,
                            )
                        },
                    ),
                    None => {
                        let diagnosis = crate::assessment::phases::observe_operation(
                            observer,
                            "StructuralDiagnosis",
                            "CategoryRouting",
                            || diagnoses.get(&reference),
                        )?;
                        match diagnosis.valid_lineage() {
                            None => {
                                let error = diagnosed_lineage_error(&diagnosis, &reference);
                                lineage_defect = Some(error.clone());
                                Err(error)
                            }
                            Some(lineage) => {
                                use type_names::CoreDefinitionRole as Role;
                                for member in lineage.members() {
                                    let member_id =
                                        DefinitionIdentity::for_reference(member, context)?;
                                    if content_seen.insert(member_id.clone()) {
                                        if let HolonReference::Smart(saved) = member {
                                            let retained =
                                                crate::assessment::phases::observe_operation(
                                                    observer,
                                                    "ContentRetention",
                                                    "SavedContent",
                                                    || saved.retain_content(),
                                                )?;
                                            content.push(retained);
                                        }
                                    }
                                    if let HolonReference::Smart(saved) = member {
                                        let mut collections = holons_core::descriptors::ComputedEffectiveSchema::source_collections();
                                        if lineage.contains_role(Role::PropertyType)
                                            || lineage.contains_role(Role::ValueType)
                                        {
                                            collections.push(R::AffordsOperator);
                                        }
                                        let names: Vec<_> = collections
                                            .iter()
                                            .map(|edge| edge.as_relationship_name())
                                            .filter(|name| {
                                                prefetched.insert((member_id.clone(), name.clone()))
                                            })
                                            .collect();
                                        if !names.is_empty() {
                                            crate::assessment::phases::observe_operation(
                                                observer,
                                                "MembershipPrefetch",
                                                role.label(),
                                                || saved.prepare_relationships(&names),
                                            )?;
                                        }
                                    }
                                }
                                if lineage.contains_role(Role::ValueType) {
                                    context.retain_frozen_descriptor_lineage(lineage)?;
                                    crate::assessment::phases::observe_operation(
                                        observer,
                                        "DerivedAcquisition",
                                        "ValueType",
                                        || {
                                            holons_core::ValueDescriptor::from_holon(
                                                reference.clone(),
                                            )
                                            .effective_schema()
                                        },
                                    )
                                } else if lineage.contains_role(Role::PropertyType) {
                                    context.retain_frozen_descriptor_lineage(lineage)?;
                                    crate::assessment::phases::observe_operation(
                                        observer,
                                        "DerivedAcquisition",
                                        "PropertyType",
                                        || {
                                            holons_core::PropertyDescriptor::from_holon(
                                                reference.clone(),
                                            )
                                            .effective_schema()
                                        },
                                    )
                                } else {
                                    crate::assessment::phases::observe_operation(
                                        observer,
                                        "FreshEffectiveSchema",
                                        "PreparedDefinition",
                                        || {
                                            holons_core::descriptors::ComputedEffectiveSchema::from_valid_lineage(lineage)
                                        },
                                    )
                                }
                            }
                        }
                    }
                };
                let schema = schema.map(Arc::new);
                let contract = crate::assessment::phases::observe_operation(
                    observer,
                    "ContributionProjection",
                    "PreparedDefinition",
                    || {
                        schema
                            .as_ref()
                            .map(|schema| Arc::new(schema.contract_contributions()))
                            .map_err(Clone::clone)
                    },
                );
                definitions.insert(
                    DefinitionIdentity::for_reference(&reference, context)?,
                    PreparedDefinition {
                        reference: reference.clone(),
                        lineage_defect,
                        constraints_consumed: Cell::new(false),
                        bindings_consumed: Cell::new(false),
                        bindings_prepared: Cell::new(false),
                        contract,
                        schema,
                    },
                );
            }
            if needs_product {
                if let Some(Ok(schema)) =
                    definitions.get(&identity).map(|d: &PreparedDefinition| &d.schema)
                {
                    for member in &schema.lineage {
                        if content_seen.insert(DefinitionIdentity::for_reference(member, context)?)
                        {
                            if let HolonReference::Smart(saved) = member {
                                content.push(crate::assessment::phases::observe_operation(
                                    observer,
                                    "ContentRetention",
                                    "SavedContent",
                                    || saved.retain_content(),
                                )?);
                            }
                        }
                    }
                    for edge in
                        holons_core::descriptors::ComputedEffectiveSchema::source_collections()
                    {
                        for entry in schema.contributions(edge) {
                            if content_seen
                                .insert(DefinitionIdentity::for_reference(&entry.member, context)?)
                            {
                                if let HolonReference::Smart(saved) = &entry.member {
                                    content.push(crate::assessment::phases::observe_operation(
                                        observer,
                                        "ContentRetention",
                                        "SavedContent",
                                        || saved.retain_content(),
                                    )?);
                                }
                            }
                        }
                    }
                    if role != DefaultMeta {
                        for entry in schema.contributions(R::Constraints) {
                            pending.push_back((entry.member.clone(), Constraint));
                        }
                    }
                    if matches!(role, Governing | Property | Value) {
                        definitions[&identity].bindings_prepared.set(true);
                        for entry in schema.contributions(R::ValidationBindings) {
                            // Reject unknown keys without acquiring family or declaring-role inputs.
                            if crate::commitments::required_key(&entry.member)
                                .ok()
                                .and_then(|key| type_names::CoreValidationRuleName::from_key(&key))
                                .map(crate::dispatch::registration)
                                .is_some_and(|r| r.implementation.is_some())
                            {
                                pending.push_back((entry.member.clone(), Binding));
                                pending.push_back((entry.declared_on.clone(), Evidence));
                            } else {
                                pending.push_back((entry.member.clone(), Retained));
                            }
                        }
                    }
                    // Universal minimum checks consume the exact governing meta-type's contributions.
                    if role == Governing {
                        for member in &schema.lineage {
                            if matches!(member.key()?, Some(key) if key.to_string() == type_names::CoreDefinitionRole::MetaTypeDescriptor.key())
                            {
                                pending.push_back((member.clone(), DefaultMeta));
                            }
                        }
                    }
                    let derived_category = diagnoses.existing(&reference)?.is_some_and(|d| {
                        d.encountered_role(type_names::CoreDefinitionRole::PropertyType)
                            || d.encountered_role(type_names::CoreDefinitionRole::ValueType)
                    });
                    if matches!(role, Governing | Defined) && !derived_category {
                        let defines_instances = schema.defines_holon_instances()?;
                        let navigation = crate::assessment::phases::observe_operation_scope(
                            observer,
                            "Navigation",
                            role.label(),
                        );
                        if defines_instances {
                            for ancestor in &schema.lineage {
                                if let HolonReference::Smart(saved) = ancestor {
                                    saved.prepare_relationships(&[
                                        R::SourceOf.as_relationship_name()
                                    ])?;
                                }
                                for member in targets(ancestor, R::SourceOf)? {
                                    if let HolonReference::Smart(saved) = &member {
                                        if content_seen.insert(DefinitionIdentity::for_reference(
                                            &member, context,
                                        )?) {
                                            content.push(saved.retain_content()?);
                                        }
                                        saved.prepare_relationships(&[
                                            R::SourceType.as_relationship_name()
                                        ])?;
                                    }
                                    pending.push_back((member, Evidence));
                                }
                            }
                            // Declared navigation candidates also need endpoint applicability.
                            for entry in schema.contributions(R::InstanceRelationships) {
                                if let HolonReference::Smart(saved) = &entry.member {
                                    saved.prepare_relationships(&[
                                        R::SourceType.as_relationship_name()
                                    ])?;
                                }
                            }
                        }
                        drop(navigation);
                        let indexes = crate::assessment::phases::observe_operation_scope(
                            observer,
                            "NameIndexes",
                            role.label(),
                        );
                        for vocabulary in [
                            holons_core::descriptors::EffectiveVocabulary::Properties,
                            holons_core::descriptors::EffectiveVocabulary::Dances,
                            holons_core::descriptors::EffectiveVocabulary::Commands,
                        ] {
                            let _ = schema.name_index(vocabulary);
                        }
                        if defines_instances {
                            let _ = schema.name_index(
                                holons_core::descriptors::EffectiveVocabulary::Relationships,
                            );
                        }
                        drop(indexes);
                        for entry in schema.contributions(R::InstanceProperties) {
                            pending.push_back((
                                entry.member.clone(),
                                // Only governing contracts need property conformance facts.
                                if role == Governing { Property } else { PropertyMember },
                            ));
                        }
                        for entry in schema.contributions(R::InstanceRelationships) {
                            pending.push_back((entry.member.clone(), RelationshipMember));
                        }
                        for edge in [R::AffordsDance, R::AffordsCommand] {
                            for entry in schema.contributions(edge) {
                                pending.push_back((entry.member.clone(), Evidence));
                            }
                        }
                    }
                    if role == Value
                        || diagnoses.existing(&reference)?.is_some_and(|d| {
                            d.encountered_role(type_names::CoreDefinitionRole::ValueType)
                        })
                    {
                        value_kinds.entry(identity.clone()).or_insert_with(|| {
                            holons_core::ValueDescriptor::from_holon(reference.clone()).value_kind()
                        });
                    }
                }
            }
            if let HolonReference::Smart(saved) = &reference {
                if !edges.is_empty() {
                    crate::assessment::phases::observe_operation(
                        observer,
                        "MembershipPrefetch",
                        role.label(),
                        || {
                            saved.prepare_relationships(
                                &edges.iter().map(|e| e.as_relationship_name()).collect::<Vec<_>>(),
                            )
                        },
                    )?;
                }
            }
            for edge in edges {
                let members = targets(&reference, edge.clone())?;
                let demand = edge_demand
                    .entry((role.label(), edge.as_relationship_name().to_string()))
                    .or_default();
                demand.0 += 1;
                demand.1 += members.len();
                // Endpoint/applicability checks consume exact identities, not target content.
                if matches!(edge, R::SourceType | R::TargetType | R::ApplicableToDescriptorTypes) {
                    continue;
                }
                let next = match (role, &edge) {
                    (Property, R::ValueType) => Value,
                    (PropertyMember, R::ValueType) => Evidence,
                    (Binding, R::DescribedBy) => Evidence,
                    (Property, R::DescribedBy) => {
                        if reference
                            .property_value(type_names::CorePropertyTypeName::IsValueRequired)?
                            .is_none()
                        {
                            DefaultMeta
                        } else {
                            Retained
                        }
                    }
                    (Schema, R::DescribedBy) => Governing,
                    (Schema, R::DependsOn) => Schema,
                    (Constraint, R::DescribedBy) => ConstraintType,
                    (ConstraintType, R::Extends) => ConstraintType,
                    (RelationshipMember, R::Extends) => RelationshipMember,
                    _ => Retained,
                };
                pending.extend(members.into_iter().map(|r| (r, next)));
            }
            observer(crate::AssessmentEvent::PreparationVisitFinished {
                type_name: role.label().into(),
                distinct_content,
            });
        }
        observer(crate::AssessmentEvent::Phase(crate::AssessmentPhase::PreparedProducts));
        let mut constraints = HashMap::new();
        let mut requests: Vec<_> = constraint_references.into_iter().collect();
        requests.sort_by_key(|(_, reference)| reference.reference_id_string());
        for (identity, reference) in requests {
            let prepared = match classify_constraint_type(&reference, |reference| {
                diagnoses.get(reference)
            })? {
                ClassifiedConstraintType::Defective { defect, describing_type } => {
                    PreparedConstraint::Defective { defect, describing_type }
                }
                ClassifiedConstraintType::Concrete { reference: constraint_type, diagnosis } => {
                    let lineage = diagnosis.valid_lineage().ok_or_else(|| {
                        HolonError::InvalidState("Concrete classification lacks lineage".into())
                    })?;
                    let parameters = match interpret_constraint(
                        Ok(lineage),
                        &ConstraintConfiguration::acquire(&reference)?,
                    ) {
                        ConstraintInterpretation::Parameters(parameters) => {
                            PreparedConstraintParameters::Valid(parameters)
                        }
                        ConstraintInterpretation::InvalidConfiguration(diagnoses) => {
                            PreparedConstraintParameters::Invalid(diagnoses)
                        }
                        ConstraintInterpretation::UnrecognizedFamily => {
                            PreparedConstraintParameters::Uninterpreted
                        }
                        ConstraintInterpretation::StructurallyInvalid(defect) => {
                            constraints.insert(
                                identity,
                                PreparedConstraint::Defective {
                                    defect,
                                    describing_type: Some(constraint_type),
                                },
                            );
                            continue;
                        }
                    };
                    PreparedConstraint::Interpreted { constraint_type, parameters }
                }
            };
            constraints.insert(identity, prepared);
        }
        let mut attachment_applicability = HashMap::new();
        for (descriptor_id, definition) in &definitions {
            // Default metadata supplies requiredness only; it creates no attachment demand.
            if ![Governing, Defined, Property, Value, Obligations]
                .into_iter()
                .any(|role| seen.contains(&(descriptor_id.clone(), role)))
            {
                continue;
            }
            let Ok(schema) = &definition.schema else { continue };
            let diagnosis = if schema.contributions(R::Constraints).is_empty() {
                None
            } else {
                Some(diagnoses.get(&definition.reference)?)
            };
            for attachment in schema.contributions(R::Constraints) {
                let constraint_id = diagnoses.identity(&attachment.member)?;
                let prepared = constraints.get(&constraint_id).ok_or_else(|| {
                    HolonError::InvalidState("Missing prepared attachment constraint".into())
                })?;
                let applicability = match prepared {
                    PreparedConstraint::Defective { defect, .. } => {
                        AttachmentApplicability::Blocked(defect.to_string())
                    }
                    PreparedConstraint::Interpreted { constraint_type, .. } => {
                        match diagnosis.as_ref().and_then(|diagnosis| diagnosis.valid_lineage()) {
                            None => AttachmentApplicability::Blocked(
                                "Unusable descriptor lineage".into(),
                            ),
                            Some(lineage) => {
                                if constraint_applies_to_lineage(constraint_type, lineage)? {
                                    AttachmentApplicability::Applies
                                } else {
                                    AttachmentApplicability::DoesNotApply
                                }
                            }
                        }
                    }
                };
                attachment_applicability
                    .insert((descriptor_id.clone(), constraint_id), applicability);
            }
        }
        // Requiredness and concrete ValueType selection are independent facts.
        let mut defaults = HashMap::new();
        let mut property_requests: Vec<_> =
            seen.iter().filter(|(_, role)| *role == Property).collect();
        property_requests
            .sort_by_key(|(identity, _)| definitions[identity].reference.reference_id_string());
        for (identity, _) in property_requests {
            let reference = definitions[identity].reference.clone();
            let required = prepare_requiredness(&reference, context, &definitions, &mut defaults)?;
            let targets = targets(&reference, R::ValueType)?;
            properties.insert(
                identity.clone(),
                PropertyFacts {
                    required,
                    value_type: match targets.as_slice() {
                        [target] => Some(target.clone()),
                        _ => None,
                    },
                },
            );
        }
        let mut preparation = crate::PreparationMetrics {
            prepared_constraint_parameters: constraints.len(),
            package_roots: packages.len(),
            subjects: subjects.len(),
            affected_schemas: workset.schemas.len(),
            distinct_content: content_seen.len(),
            requested_schema_products: definitions.len(),
            identity_role_visits: seen.len(),
            ..Default::default()
        };
        preparation.constructed_schema_products =
            definitions.values().filter(|definition| definition.schema.is_ok()).count();
        for root in roots {
            let identity = DefinitionIdentity::for_reference(root, context)?;
            let entry = packages.get_mut(&identity).expect("root registered before discovery");
            entry.contract = definitions
                .get(&identity)
                .ok_or_else(|| HolonError::InvalidState("Missing prepared descriptor".into()))?
                .contract
                .clone();
        }
        for ((role, edge), (requests, members)) in edge_demand {
            observer(crate::AssessmentEvent::PreparationEdge { role, edge, requests, members });
        }
        Ok(Self {
            context: Arc::clone(context),
            packages,
            definitions,
            _content: content,
            preparation,
            diagnoses,
            properties,
            value_kinds,
            constraints,
            attachment_applicability,
        })
    }

    #[cfg(test)]
    pub(crate) fn declarations(&self) -> crate::ConstraintDeclarationAssessment<'_> {
        crate::ConstraintDeclarationAssessment::new(self)
    }

    pub(crate) fn prepared_constraint(
        &self,
        reference: &HolonReference,
    ) -> Result<&PreparedConstraint, HolonError> {
        self.constraints.get(&self.diagnoses.identity(reference)?).ok_or_else(|| {
            HolonError::InvalidState("Constraint interpretation was not prepared".into())
        })
    }

    pub(crate) fn attachment_applicability(
        &self,
        descriptor: &HolonReference,
        constraint: &HolonReference,
    ) -> Result<&AttachmentApplicability, HolonError> {
        self.attachment_applicability
            .get(&(self.diagnoses.identity(descriptor)?, self.diagnoses.identity(constraint)?))
            .ok_or_else(|| {
                HolonError::InvalidState("Attachment applicability was not prepared".into())
            })
    }

    pub fn requiredness(&self, reference: &HolonReference) -> Result<Requiredness, HolonError> {
        Ok(self
            .properties
            .get(&self.diagnoses.identity(reference)?)
            .ok_or_else(|| {
                HolonError::InvalidState(format!(
                    "Missing requiredness for {} (subject conformance)",
                    reference.reference_id_string()
                ))
            })?
            .required
            .clone())
    }
    pub fn value_type(
        &self,
        reference: &HolonReference,
    ) -> Result<Option<&HolonReference>, HolonError> {
        Ok(self
            .properties
            .get(&self.diagnoses.identity(reference)?)
            .ok_or_else(|| {
                HolonError::InvalidState(format!(
                    "Missing ValueType selection for {} (subject conformance)",
                    reference.reference_id_string()
                ))
            })?
            .value_type
            .as_ref())
    }
    pub fn value_kind(
        &self,
        reference: &HolonReference,
    ) -> Result<holons_core::ValueDescriptorKind, HolonError> {
        self.value_kinds
            .get(&self.diagnoses.identity(reference)?)
            .ok_or_else(|| {
                HolonError::InvalidState(format!(
                    "Missing native kind for {} (value assessment)",
                    reference.reference_id_string()
                ))
            })?
            .clone()
    }

    /// Supplies reached binding contributions for construction-time role preparation.
    pub fn binding_contributions(&self) -> Vec<EffectiveRelationshipMember> {
        self.definitions
            .values()
            .filter(|definition| definition.bindings_prepared.get())
            .filter_map(|definition| definition.schema.as_ref().ok())
            .flat_map(|schema| schema.contributions(R::ValidationBindings))
            .map(|entry| EffectiveRelationshipMember {
                member: entry.member.clone(),
                declared_on: entry.declared_on.clone(),
            })
            .collect()
    }

    /// Projects obligations from complete prepared occurrences without another collection walk.
    pub fn constraint_contributions(
        &self,
        reference: &HolonReference,
    ) -> Result<holons_core::descriptors::ConstraintContributions, HolonError> {
        let diagnosis = self.diagnoses.get(reference)?;
        let lineage = diagnosis.valid_lineage().ok_or_else(|| {
            HolonError::InvalidState(format!(
                "Constraint contributions require a valid lineage for {}",
                reference.reference_id_string()
            ))
        })?;
        let parent = lineage.members().get(1).cloned();
        let effective = self.effective_constraints(reference)?;
        let inherited = match &parent {
            Some(parent) => self.effective_constraints(parent)?,
            None => Vec::new(),
        };
        Ok(holons_core::descriptors::ConstraintContributions { parent, inherited, effective })
    }

    pub fn effective_schema(
        &self,
        reference: &HolonReference,
    ) -> Result<&holons_core::descriptors::ComputedEffectiveSchema, HolonError> {
        self.prepared_definition(reference)?.schema.as_ref().map(Arc::as_ref).map_err(Clone::clone)
    }

    pub fn effective_bindings(
        &self,
        reference: &HolonReference,
    ) -> Result<Vec<EffectiveRelationshipMember>, HolonError> {
        let definition = self.prepared_definition(reference)?;
        definition.bindings_consumed.set(true);
        Ok(definition
            .schema
            .as_ref()
            .map_err(Clone::clone)?
            .contributions(R::ValidationBindings)
            .iter()
            .map(|entry| EffectiveRelationshipMember {
                member: entry.member.clone(),
                declared_on: entry.declared_on.clone(),
            })
            .collect())
    }

    pub fn effective_constraints(
        &self,
        reference: &HolonReference,
    ) -> Result<Vec<EffectiveRelationshipMember>, HolonError> {
        let definition = self.prepared_definition(reference)?;
        definition.constraints_consumed.set(true);
        Ok(definition
            .schema
            .as_ref()
            .map_err(Clone::clone)?
            .contributions(R::Constraints)
            .iter()
            .map(|entry| EffectiveRelationshipMember {
                member: entry.member.clone(),
                declared_on: entry.declared_on.clone(),
            })
            .collect())
    }

    fn prepared_definition(
        &self,
        reference: &HolonReference,
    ) -> Result<&PreparedDefinition, HolonError> {
        self.definitions
            .get(&DefinitionIdentity::for_reference(reference, &self.context)?)
            .ok_or_else(|| {
                HolonError::InvalidState(format!(
                    "Definition {} has no prepared schema (contract consumer)",
                    reference.reference_id_string()
                ))
                .into()
            })
    }

    pub fn get(
        &self,
        context: &Arc<TransactionContext>,
        root: &HolonReference,
    ) -> Result<&DescriptorPackage, HolonError> {
        let package =
            self.packages.get(&DefinitionIdentity::for_reference(root, context)?).ok_or_else(
                || HolonError::InvalidState("Descriptor package was not constructed".into()),
            )?;
        package.consumed.set(true);
        Ok(package)
    }

    /// Counts distinct prepared products requested by assessment consumers, including errors.
    pub fn preparation_metrics(&self) -> crate::PreparationMetrics {
        crate::PreparationMetrics {
            contract_consumers: self
                .packages
                .values()
                .filter(|package| package.consumed.get())
                .count(),
            constraint_consumers: self
                .definitions
                .values()
                .filter(|definition| definition.constraints_consumed.get())
                .count(),
            binding_consumers: self
                .definitions
                .values()
                .filter(|definition| definition.bindings_consumed.get())
                .count(),
            ..self.preparation.clone()
        }
    }
}

/// Preserve typed structural defects separately from missing promised preparation.
fn diagnosed_lineage_error(
    diagnosis: &holons_core::ExtendsLineageDiagnosis,
    reference: &HolonReference,
) -> HolonError {
    use holons_core::ExtendsLineageDefect;
    match &diagnosis.defects[0] {
        ExtendsLineageDefect::MultipleParents { subject, count } => {
            HolonError::MultipleExtends { descriptor: subject.reference_id_string(), count: *count }
        }
        ExtendsLineageDefect::Cycle { repeated_descriptor, .. } => {
            HolonError::CyclicExtends { descriptor: repeated_descriptor.clone() }
        }
        ExtendsLineageDefect::WrongTermination { terminal } => HolonError::WrongDescriptorKind {
            expected: "Extends lineage terminating at TypeDescriptor".into(),
            found: terminal.reference_id_string(),
            descriptor: reference.reference_id_string(),
        },
        ExtendsLineageDefect::RootHasParent => HolonError::InvalidParameter(
            "Diagnosed TypeDescriptor root has an Extends parent".into(),
        ),
    }
}

/// Requiredness is a concrete declaration, with an absent-value default on its meta-type.
fn prepare_requiredness(
    reference: &HolonReference,
    context: &Arc<TransactionContext>,
    definitions: &HashMap<DefinitionIdentity, PreparedDefinition>,
    defaults: &mut HashMap<DefinitionIdentity, Result<Requiredness, HolonError>>,
) -> Result<Requiredness, HolonError> {
    use type_names::CorePropertyTypeName as P;
    fn boolean(value: Option<core_types::BaseValue>) -> Result<Requiredness, HolonError> {
        match value {
            Some(core_types::BaseValue::BooleanValue(value)) => Ok(Ok(value.0)),
            Some(other) => Ok(Err(format!("IsValueRequired requires Boolean; found {other:?}"))),
            None => Ok(Err("IsValueRequired has no DefaultValue".into())),
        }
    }
    if let Some(value) = reference.property_value(P::IsValueRequired)? {
        return boolean(Some(value));
    }
    let meta = match holons_core::descriptors::resolve_describing_type(reference)? {
        holons_core::DescribingTypeResolution::Unique(meta) => meta,
        holons_core::DescribingTypeResolution::Missing => {
            return Ok(Err("Requiredness default requires one DescribedBy; found none".into()))
        }
        holons_core::DescribingTypeResolution::Multiple(targets) => {
            return Ok(Err(format!(
                "Requiredness default requires one DescribedBy; found {}",
                targets.len()
            )))
        }
    };
    let identity = DefinitionIdentity::for_reference(&meta, context)?;
    defaults
        .entry(identity.clone())
        .or_insert_with(|| {
            let definition = definitions.get(&identity).ok_or_else(|| {
                HolonError::InvalidState(format!(
                    "Missing requiredness default product for {}",
                    meta.reference_id_string()
                ))
            })?;
            if let Some(defect) = &definition.lineage_defect {
                return Ok(Err(format!("Governing meta-type lineage is unusable: {defect}")));
            }
            let schema = definition.schema.as_ref().map_err(Clone::clone)?;
            let matches = schema
                .contributions(R::InstanceProperties)
                .iter()
                .filter_map(|entry| match entry.member.property_value(P::TypeName) {
                    Ok(Some(core_types::BaseValue::StringValue(name)))
                        if name.0 == P::IsValueRequired.as_property_name().to_string() =>
                    {
                        Some(Ok(&entry.member))
                    }
                    Err(error) => Some(Err(error)),
                    _ => None,
                })
                .collect::<Result<Vec<_>, HolonError>>()?;
            match matches.as_slice() {
                [member] => boolean(member.property_value(P::DefaultValue)?),
                _ => Ok(Err(format!(
                    "Meta-type {} requires one IsValueRequired default member",
                    meta.reference_id_string()
                ))),
            }
        })
        .clone()
}
