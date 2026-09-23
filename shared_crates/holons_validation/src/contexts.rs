use std::{collections::HashMap, sync::Arc};

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

/// Descriptor services shared by subject validators, including standalone value assessment.
///
/// The binding anchors cover the complete rule inventory so all entry points use the same
/// compatibility rules. They reference schema definitions, never containing subject
/// holons, properties, or Nursery state; they provide no upward subject navigation.
pub struct ValueValidationContext {
    pub(crate) roots: ResolvedValueTypeRoots,
    pub(crate) bindings: BindingRoots,
}

impl HolonValidationContext {
    /// Resolves the Core anchors through ordinary transaction-scoped lookup.
    /// All binding anchors must already be loaded and completed. Missing or ambiguous
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
    /// Resolves the complete binding inventory with prospective reference selection.
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
    Descriptor,
    SchemaAggregate,
}

pub(crate) struct BindingRoot {
    pub name: CoreValidationRuleName,
    pub rule: HolonReference,
    pub family: HolonReference,
    pub descriptor_family: HolonReference,
    pub level: SubjectLevel,
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

    fn resolve_using<E>(
        mut resolve: impl FnMut(&str) -> Result<HolonReference, E>,
    ) -> Result<Self, E> {
        use BindingDispatchRoute::*;
        use CoreValidationRuleName::*;
        use SubjectLevel::*;
        // The complete table is required for placement compatibility even when a
        // particular assessment pass dispatches only one route.
        let definitions = [
            (
                RequiredPropertyPresence,
                "PropertyValidationRule.HolonType",
                "PropertyType.TypeDescriptor",
                Property,
                Subject,
            ),
            (
                NoUndescribedProperties,
                "HolonValidationRule.HolonType",
                "HolonType.TypeDescriptor",
                Holon,
                Subject,
            ),
            (
                BaseValueKindMatchesString,
                "StringValidationRule.HolonType",
                "StringValueType.ValueType",
                Value,
                Subject,
            ),
            (
                BaseValueKindMatchesInteger,
                "IntegerValidationRule.HolonType",
                "IntegerValueType.ValueType",
                Value,
                Subject,
            ),
            (
                BaseValueKindMatchesBoolean,
                "BooleanValidationRule.HolonType",
                "BooleanValueType.ValueType",
                Value,
                Subject,
            ),
            (
                BaseValueKindMatchesBytes,
                "BytesValidationRule.HolonType",
                "BytesValueType.ValueType",
                Value,
                Subject,
            ),
            (
                BaseValueKindMatchesEnum,
                "EnumValueValidationRule.HolonType",
                "EnumValueType.ValueType",
                Value,
                Subject,
            ),
            (
                AtMostOneDirectParent,
                "HolonValidationRule.HolonType",
                "MetaTypeDescriptor.HolonType",
                Holon,
                Descriptor,
            ),
            (
                AcyclicExtendsLineage,
                "HolonValidationRule.HolonType",
                "MetaTypeDescriptor.HolonType",
                Holon,
                Descriptor,
            ),
            (
                ExtendsLineageTerminatesAtTypeDescriptor,
                "HolonValidationRule.HolonType",
                "MetaTypeDescriptor.HolonType",
                Holon,
                Descriptor,
            ),
            (
                UniqueTypeDescriptorRoot,
                "HolonValidationRule.HolonType",
                "MetaTypeDescriptor.HolonType",
                Holon,
                Descriptor,
            ),
            (
                LocalInstanceKindAnchorDesignation,
                "HolonValidationRule.HolonType",
                "MetaTypeDescriptor.HolonType",
                Holon,
                Descriptor,
            ),
            (
                InstanceKindAnchorsAreAbstract,
                "HolonValidationRule.HolonType",
                "MetaTypeDescriptor.HolonType",
                Holon,
                Descriptor,
            ),
            (
                TypeDescriptorRootKindException,
                "HolonValidationRule.HolonType",
                "MetaTypeDescriptor.HolonType",
                Holon,
                Descriptor,
            ),
            (
                DescribingCategoryCompatibility,
                "HolonValidationRule.HolonType",
                "HolonType.TypeDescriptor",
                Holon,
                Descriptor,
            ),
            (
                DescriptorMetaTypeCorrespondence,
                "HolonValidationRule.HolonType",
                "HolonType.TypeDescriptor",
                Holon,
                Descriptor,
            ),
            (
                NoInheritedMemberRedeclaration,
                "HolonValidationRule.HolonType",
                "MetaTypeDescriptor.HolonType",
                Holon,
                Descriptor,
            ),
            (
                UniqueSemanticMemberNames,
                "HolonValidationRule.HolonType",
                "MetaTypeDescriptor.HolonType",
                Holon,
                Descriptor,
            ),
            (
                WellFormedEffectiveMemberDefinitions,
                "HolonValidationRule.HolonType",
                "MetaTypeDescriptor.HolonType",
                Holon,
                Descriptor,
            ),
            (
                ContractMemberKindCompatibility,
                "HolonValidationRule.HolonType",
                "MetaTypeDescriptor.HolonType",
                Holon,
                Descriptor,
            ),
            (
                InheritedValueConstraintNonRelaxation,
                "HolonValidationRule.HolonType",
                "MetaTypeDescriptor.HolonType",
                Holon,
                Descriptor,
            ),
            (
                SchemaDependenciesAcyclic,
                "HolonValidationRule.HolonType",
                "Schema.HolonType",
                Holon,
                SchemaAggregate,
            ),
            (
                CrossSchemaDependenciesDeclared,
                "HolonValidationRule.HolonType",
                "Schema.HolonType",
                Holon,
                SchemaAggregate,
            ),
        ];
        let mut resolved = HashMap::<&str, HolonReference>::new();
        let mut get = |key: &'static str| -> Result<HolonReference, E> {
            if let Some(reference) = resolved.get(key) {
                return Ok(reference.clone());
            }
            let reference = resolve(key)?;
            resolved.insert(key, reference.clone());
            Ok(reference)
        };
        let entries = definitions
            .into_iter()
            .map(|(name, family, descriptor_family, level, route)| {
                Ok(BindingRoot {
                    name,
                    rule: get(name.as_str())?,
                    family: get(family)?,
                    descriptor_family: get(descriptor_family)?,
                    level,
                    route,
                })
            })
            .collect::<Result<_, E>>()?;
        Ok(Self { entries })
    }
}
