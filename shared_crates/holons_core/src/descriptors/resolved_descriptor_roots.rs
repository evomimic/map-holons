//! Snapshot-scoped resolution of canonical descriptor identities.

use std::sync::Arc;

use base_types::{BaseValueKind, MapString};
use core_types::HolonError;

use crate::core_shared_objects::transactions::TransactionContext;
use crate::descriptors::value_descriptor::ValueDescriptorKind;
use crate::reference_layer::HolonReference;

/// Canonical value-family identities resolved for one transaction snapshot.
///
/// Construct once when preparing a validation pass and share by immutable borrow.
/// This holds bound references, not descriptor contents or another inheritance graph.
/// Discard it after the pass; it must not be reused after schema mutation.
#[derive(Clone, Debug)]
pub struct ResolvedValueTypeRoots {
    pub(super) families: Vec<(HolonReference, ValueDescriptorKind)>,
    pub(super) context: Arc<TransactionContext>,
}

impl ResolvedValueTypeRoots {
    /// Resolves the seven canonical Core family roots through transaction lookup.
    /// Staged definitions take precedence; only absence permits persisted lookup.
    pub fn resolve(context: &Arc<TransactionContext>) -> Result<Self, HolonError> {
        let families = [
            ("StringValueType.ValueType", ValueDescriptorKind::BaseValue(BaseValueKind::String)),
            ("IntegerValueType.ValueType", ValueDescriptorKind::BaseValue(BaseValueKind::Integer)),
            ("BooleanValueType.ValueType", ValueDescriptorKind::BaseValue(BaseValueKind::Boolean)),
            ("BytesValueType.ValueType", ValueDescriptorKind::BaseValue(BaseValueKind::Bytes)),
            ("EnumValueType.ValueType", ValueDescriptorKind::BaseValue(BaseValueKind::Enum)),
            ("BaseValueValueType.ValueType", ValueDescriptorKind::AnyBaseValue),
            ("ValueArrayValueType.ValueType", ValueDescriptorKind::ValueArray),
        ]
        .into_iter()
        .map(|(key, kind)| Ok((resolve_core_descriptor(context, key)?, kind)))
        .collect::<Result<Vec<_>, HolonError>>()?;
        Ok(Self { families, context: Arc::clone(context) })
    }
}

/// Resolves a canonical Core schema key through the transaction lookup façade.
///
/// Staged definitions take precedence; only `HolonNotFound` permits saved lookup.
/// Duplicate-key and operational failures propagate unchanged. This resolves schema
/// identities (including rule holons), without asserting a particular descriptor kind.
/// Resolve at the pass boundary and compare the resulting bound references by identity;
/// do not reuse them across transactions or schema mutations.
pub fn resolve_core_descriptor(
    context: &Arc<TransactionContext>,
    key: &str,
) -> Result<HolonReference, HolonError> {
    let key = MapString(key.to_owned());
    match context.lookup().get_staged_holon_by_base_key(&key) {
        Ok(reference) => Ok(reference.into()),
        Err(HolonError::HolonNotFound(_)) => {
            Ok(context.lookup().get_saved_holon_by_key(&key)?.into())
        }
        Err(error) => Err(error),
    }
}

/// Reject cross-transaction inputs before walking a graph with snapshot-bound anchors.
pub(super) fn assert_same_transaction(
    reference: &HolonReference,
    context: &Arc<TransactionContext>,
) -> Result<(), HolonError> {
    // Numeric transaction IDs are only unique within one space manager.
    if !Arc::ptr_eq(&reference.bound_context(), context) {
        return Err(HolonError::CrossTransactionReference {
            reference_kind: "descriptor".to_owned(),
            reference_id: reference.reference_id_string(),
            reference_tx: reference.tx_id().value(),
            context_tx: context.tx_id().value(),
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core_shared_objects::holon::SavedHolon;
    use crate::descriptors::test_support::{
        build_context, build_context_with_saved_holons, new_test_holon,
    };
    use crate::{ReadableHolon, ValueDescriptor};
    use base_types::{BaseValue, MapInteger};
    use core_types::{HolonId, LocalId, PropertyMap};
    use std::collections::HashMap;
    use type_names::ToPropertyName;

    #[test]
    fn saved_family_roots_resolve_by_key_even_when_type_names_differ() -> Result<(), HolonError> {
        let names = [
            "StringValueType",
            "IntegerValueType",
            "BooleanValueType",
            "BytesValueType",
            "EnumValueType",
            "BaseValueValueType",
            "ValueArrayValueType",
        ];
        let holons = names
            .iter()
            .enumerate()
            .map(|(index, name)| {
                let properties = PropertyMap::from([
                    (
                        "Key".to_property_name(),
                        BaseValue::StringValue(MapString(format!("{name}.ValueType"))),
                    ),
                    (
                        "TypeName".to_property_name(),
                        BaseValue::StringValue(MapString("RenamedLabel".into())),
                    ),
                ]);
                SavedHolon::new(LocalId(vec![index as u8; 39]), properties, None, MapInteger(1))
            })
            .collect();
        let context = build_context_with_saved_holons(holons, HashMap::new());
        let roots = ResolvedValueTypeRoots::resolve(&context)?;
        for (index, (root, expected)) in roots.families.iter().enumerate() {
            assert_eq!(root.holon_id()?, HolonId::Local(LocalId(vec![index as u8; 39])));
            assert_eq!(ValueDescriptor::from_holon(root.clone()).value_kind(&roots)?, *expected);
        }
        // Staged Core definitions are visible during bootstrap before persistence.
        let staged = context
            .mutation()
            .stage_new_holon(new_test_holon(&context, "StringValueType.ValueType")?)?;
        assert_eq!(resolve_core_descriptor(&context, "StringValueType.ValueType")?, staged.into());
        Ok(())
    }

    #[test]
    fn missing_anchor_prevents_resolution() {
        assert!(matches!(
            ResolvedValueTypeRoots::resolve(&build_context()),
            Err(HolonError::HolonNotFound(_))
        ));
    }
}
