//! Space-authoritative dances bind the logical Space, independently of its saved version.
use crate::core_shared_objects::transactions::TransactionContext;
use crate::{HolonReference, LineageIdentity};
use core_types::HolonError;
use std::sync::Arc;

/// Retain the adopted infrastructure root while accepting versions of that same Space.
/// Names and keys never establish authority; saved record lineage does.
pub(super) fn require_transaction_space(
    context: &Arc<TransactionContext>,
    affording: &HolonReference,
    operation: &str,
) -> Result<(), HolonError> {
    let space = context.get_space_holon()?.ok_or_else(|| {
        HolonError::InvalidParameter(format!("{operation} requires a persisted HolonSpace"))
    })?;
    if LineageIdentity::for_reference(affording, context)?
        != LineageIdentity::for_reference(&space, context)?
    {
        return Err(HolonError::InvalidParameter(format!(
            "{operation} must be afforded by this transaction's HolonSpace"
        )));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core_shared_objects::holon::SavedHolon;
    use crate::descriptors::test_support::build_context_with_saved_holons;
    use base_types::{MapInteger, ToBaseValue};
    use core_types::{HolonId, LocalId, PropertyMap};
    use std::collections::HashMap;
    use type_names::{CorePropertyTypeName, ToPropertyName};

    #[test]
    fn versions_share_space_authority_but_names_and_new_holons_do_not() {
        let root = LocalId(vec![1; 39]);
        let version = LocalId(vec![2; 39]);
        let foreign = LocalId(vec![3; 39]);
        let mut properties = PropertyMap::new();
        properties.insert(
            CorePropertyTypeName::Key.to_property_name(),
            "Alex Example's Personal Space".to_base_value(),
        );
        let context = build_context_with_saved_holons(
            vec![
                SavedHolon::new(
                    root.clone(),
                    properties.clone(),
                    Some(root.clone()),
                    MapInteger(1),
                ),
                SavedHolon::new(
                    version.clone(),
                    properties.clone(),
                    Some(root.clone()),
                    MapInteger(2),
                ),
                SavedHolon::new(foreign.clone(), properties, Some(foreign.clone()), MapInteger(1)),
            ],
            HashMap::new(),
        );
        context.set_space_holon_id(HolonId::Local(root.clone())).unwrap();
        for id in [root, version] {
            let reference =
                HolonReference::smart_from_id(context.space_read_handle(), HolonId::Local(id));
            require_transaction_space(&context, &reference, "LoadHolons").unwrap();
        }
        let other =
            HolonReference::smart_from_id(context.space_read_handle(), HolonId::Local(foreign));
        assert!(matches!(
            require_transaction_space(&context, &other, "LoadHolons"),
            Err(HolonError::InvalidParameter(_))
        ));
        let impostor =
            context.mutation().new_holon(Some("Alex Example's Personal Space".into())).unwrap();
        assert!(matches!(
            require_transaction_space(&context, &impostor.into(), "LoadHolons"),
            Err(HolonError::InvalidParameter(_))
        ));
    }
}
