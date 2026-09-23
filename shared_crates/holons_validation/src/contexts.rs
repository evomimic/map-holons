use std::sync::Arc;

use core_types::HolonError;
use holons_core::core_shared_objects::transactions::TransactionContext;
use holons_core::{
    resolve_core_descriptor, HolonReference, ResolvedValueTypeRoots, UniversalDescriptorContract,
};
use type_names::CoreValidationRuleName;

/// Immutable execution dependencies for one holon-validation pass.
///
/// Resolve once after input completion. These are transaction-bound identities,
/// not cached descriptor contents. Discard the context before schema mutation or
/// transaction replacement. Subjects must belong to that same transaction snapshot.
pub struct HolonValidationContext {
    pub(crate) universal: UniversalDescriptorContract,
    pub(crate) values: ValueValidationContext,
}

/// Property-local dependencies and the minimum decision supplied by holon validation.
///
/// The Boolean is the result of `EnforceMinimum(H, M)`, not a parent handle. A
/// standalone caller must compute it from the same completed input snapshot.
pub struct PropertyValidationContext<'a> {
    /// Whether this contract member must enforce its declared minimum.
    pub enforce_minimum: bool,
    /// Immutable lower-level services shared throughout the pass.
    pub values: &'a ValueValidationContext,
}

/// Descriptor services shared by the C1 validators, including standalone value assessment.
///
/// The binding anchors cover the entire C1 cohort so all entry points use the same
/// compatibility rules. They reference schema definitions, never containing subject
/// holons, properties, or Nursery state; they provide no upward subject navigation.
pub struct ValueValidationContext {
    pub(crate) roots: ResolvedValueTypeRoots,
    pub(crate) bindings: BindingRoots,
}

impl HolonValidationContext {
    /// Resolves the Core anchors through ordinary transaction-scoped lookup.
    /// All C1 anchors must already be loaded and completed. Missing or ambiguous
    /// anchors prevent constructing a reliable pass and return an operational error.
    pub fn resolve(context: &Arc<TransactionContext>) -> Result<Self, HolonError> {
        Ok(Self {
            universal: UniversalDescriptorContract::resolve(context)?,
            values: ValueValidationContext::resolve(context)?,
        })
    }

    /// Borrows the value-local services for direct property/value assessment.
    pub fn value_context(&self) -> &ValueValidationContext {
        &self.values
    }
}

impl ValueValidationContext {
    /// Prepares the existing active cohort with prospective reference semantics.
    /// This does not resolve or activate any of the later C2 rule roots.
    pub fn resolve_in_view(
        context: &Arc<TransactionContext>,
        reader: &holons_core::ProspectiveDescriptorReader,
    ) -> Result<Self, holons_core::AssessmentReadError> {
        Ok(Self {
            roots: ResolvedValueTypeRoots::resolve_with_reader(context, reader)?,
            bindings: BindingRoots::resolve_in_view(context, reader)?,
        })
    }

    /// Resolves immutable anchors for a standalone value or property pass.
    pub fn resolve(context: &Arc<TransactionContext>) -> Result<Self, HolonError> {
        Ok(Self {
            roots: ResolvedValueTypeRoots::resolve(context)?,
            bindings: BindingRoots::resolve(context)?,
        })
    }
}

/// Expected invocation boundary, distinct from a schema's authored display labels.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum SubjectLevel {
    Holon,
    Property,
    Value,
}

/// Execution routing within a subject level, independent of binding compatibility.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum BindingDispatchRoute {
    Subject,
    // Reserved until the C2 handlers and canonical bindings activate together.
    #[allow(dead_code)]
    Descriptor,
    #[allow(dead_code)]
    SchemaAggregate,
}

pub(crate) struct BindingRoot {
    pub name: CoreValidationRuleName,
    pub rule: HolonReference,
    pub family: HolonReference,
    pub descriptor_family: HolonReference,
    pub level: SubjectLevel,
    // Read when C2 descriptor and aggregate dispatch activate together.
    #[allow(dead_code)]
    pub route: BindingDispatchRoute,
}

pub(crate) struct BindingRoots {
    pub entries: Vec<BindingRoot>,
}

impl BindingRoots {
    fn resolve(context: &Arc<TransactionContext>) -> Result<Self, HolonError> {
        Self::resolve_using(|key| resolve_core_descriptor(context, key))
    }

    fn resolve_in_view(
        context: &Arc<TransactionContext>,
        reader: &holons_core::ProspectiveDescriptorReader,
    ) -> Result<Self, holons_core::AssessmentReadError> {
        Self::resolve_using(|key| {
            crate::prospective::resolve_validation_anchor_in_view(context, key, reader)
        })
    }

    // Deliberately separate from resolve(): Phase 9 activates this cohort with its TDL.
    pub(crate) fn add_c2_in_view(
        &mut self,
        context: &Arc<TransactionContext>,
        reader: &holons_core::ProspectiveDescriptorReader,
    ) -> Result<(), holons_core::AssessmentReadError> {
        use CoreValidationRuleName::*;
        for (name, descriptor_family, route) in [
            (
                AtMostOneDirectParent,
                "MetaTypeDescriptor.HolonType",
                BindingDispatchRoute::Descriptor,
            ),
            (
                AcyclicExtendsLineage,
                "MetaTypeDescriptor.HolonType",
                BindingDispatchRoute::Descriptor,
            ),
            (
                ExtendsLineageTerminatesAtTypeDescriptor,
                "MetaTypeDescriptor.HolonType",
                BindingDispatchRoute::Descriptor,
            ),
            (
                UniqueTypeDescriptorRoot,
                "MetaTypeDescriptor.HolonType",
                BindingDispatchRoute::Descriptor,
            ),
            (
                LocalInstanceKindAnchorDesignation,
                "MetaTypeDescriptor.HolonType",
                BindingDispatchRoute::Descriptor,
            ),
            (
                InstanceKindAnchorsAreAbstract,
                "MetaTypeDescriptor.HolonType",
                BindingDispatchRoute::Descriptor,
            ),
            (
                TypeDescriptorRootKindException,
                "MetaTypeDescriptor.HolonType",
                BindingDispatchRoute::Descriptor,
            ),
            (
                DescribingCategoryCompatibility,
                "HolonType.TypeDescriptor",
                BindingDispatchRoute::Descriptor,
            ),
            (
                DescriptorMetaTypeCorrespondence,
                "HolonType.TypeDescriptor",
                BindingDispatchRoute::Descriptor,
            ),
            (
                NoInheritedMemberRedeclaration,
                "MetaTypeDescriptor.HolonType",
                BindingDispatchRoute::Descriptor,
            ),
            (
                UniqueSemanticMemberNames,
                "MetaTypeDescriptor.HolonType",
                BindingDispatchRoute::Descriptor,
            ),
            (
                WellFormedEffectiveMemberDefinitions,
                "MetaTypeDescriptor.HolonType",
                BindingDispatchRoute::Descriptor,
            ),
            (
                ContractMemberKindCompatibility,
                "MetaTypeDescriptor.HolonType",
                BindingDispatchRoute::Descriptor,
            ),
            (
                InheritedValueConstraintNonRelaxation,
                "MetaTypeDescriptor.HolonType",
                BindingDispatchRoute::Descriptor,
            ),
            (SchemaDependenciesAcyclic, "Schema.HolonType", BindingDispatchRoute::SchemaAggregate),
            (
                CrossSchemaDependenciesDeclared,
                "Schema.HolonType",
                BindingDispatchRoute::SchemaAggregate,
            ),
        ] {
            self.entries.push(BindingRoot {
                name,
                rule: crate::resolve_validation_anchor_in_view(context, name.as_str(), reader)?,
                family: crate::resolve_validation_anchor_in_view(
                    context,
                    "HolonValidationRule.HolonType",
                    reader,
                )?,
                descriptor_family: crate::resolve_validation_anchor_in_view(
                    context,
                    descriptor_family,
                    reader,
                )?,
                level: SubjectLevel::Holon,
                route,
            });
        }
        Ok(())
    }

    fn resolve_using<E>(
        mut resolve: impl FnMut(&str) -> Result<HolonReference, E>,
    ) -> Result<Self, E> {
        use CoreValidationRuleName::*;
        use SubjectLevel::*;
        // Names are confined to resolution. Placement and rule selection thereafter
        // compare resolved reference identities and use the existing lineage product.
        let entries = [
            (
                RequiredPropertyPresence,
                "PropertyValidationRule.HolonType",
                "PropertyType.TypeDescriptor",
                Property,
            ),
            (
                NoUndescribedProperties,
                "HolonValidationRule.HolonType",
                "HolonType.TypeDescriptor",
                Holon,
            ),
            (
                BaseValueKindMatchesString,
                "StringValidationRule.HolonType",
                "StringValueType.ValueType",
                Value,
            ),
            (
                BaseValueKindMatchesInteger,
                "IntegerValidationRule.HolonType",
                "IntegerValueType.ValueType",
                Value,
            ),
            (
                BaseValueKindMatchesBoolean,
                "BooleanValidationRule.HolonType",
                "BooleanValueType.ValueType",
                Value,
            ),
            (
                BaseValueKindMatchesBytes,
                "BytesValidationRule.HolonType",
                "BytesValueType.ValueType",
                Value,
            ),
            (
                BaseValueKindMatchesEnum,
                "EnumValueValidationRule.HolonType",
                "EnumValueType.ValueType",
                Value,
            ),
        ]
        .into_iter()
        .map(|(name, family, descriptor_family, level)| {
            Ok(BindingRoot {
                name,
                rule: resolve(name.as_str())?,
                family: resolve(family)?,
                descriptor_family: resolve(descriptor_family)?,
                level,
                route: BindingDispatchRoute::Subject,
            })
        })
        .collect::<Result<_, E>>()?;
        Ok(Self { entries })
    }
}
