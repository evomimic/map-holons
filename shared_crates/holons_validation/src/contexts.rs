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

pub(crate) struct BindingRoot {
    pub name: CoreValidationRuleName,
    pub rule: HolonReference,
    pub family: HolonReference,
    pub descriptor_family: HolonReference,
    pub level: SubjectLevel,
}

pub(crate) struct BindingRoots {
    pub entries: Vec<BindingRoot>,
}

impl BindingRoots {
    fn resolve(context: &Arc<TransactionContext>) -> Result<Self, HolonError> {
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
                rule: resolve_core_descriptor(context, name.as_str())?,
                family: resolve_core_descriptor(context, family)?,
                descriptor_family: resolve_core_descriptor(context, descriptor_family)?,
                level,
            })
        })
        .collect::<Result<_, HolonError>>()?;
        Ok(Self { entries })
    }
}
