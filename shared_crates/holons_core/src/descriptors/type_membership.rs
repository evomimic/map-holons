//! Inheritance-aware type membership by descriptor identity.
//!
//! A holon has a type when its describing descriptor is that type's canonical
//! descriptor or extends it. Identity decides, not type name: an unrelated
//! descriptor that happens to share a type name is a different type, while an
//! extension type is accepted wherever its supported ancestor is.

use base_types::MapString;
use core_types::HolonError;
use type_names::holon_type_descriptor_key;

use super::{equals_or_extends, resolve_core_descriptor, same_definition, walk_extends_chain};
use super::{Descriptor, TypeHeader};
use crate::reference_layer::{HolonReference, ReadableHolon};

/// Returns true when `holon` is described by `expected` or by a descriptor
/// that extends it.
pub fn has_type(holon: &HolonReference, expected: &HolonReference) -> Result<bool, HolonError> {
    equals_or_extends(holon.holon_descriptor()?.holon(), expected)
}

/// Requires `holon` to have the canonical concrete holon type `type_name`.
///
/// The canonical descriptor is resolved by its key (`<TypeName>.HolonType`) in
/// `holon`'s resolution context. A holon of any other type is reported as
/// `WrongDescriptorKind`, naming its own describing type.
pub fn require_holon_type(holon: &HolonReference, type_name: &MapString) -> Result<(), HolonError> {
    if has_type(holon, &canonical_holon_type(holon, type_name)?)? {
        return Ok(());
    }
    Err(HolonError::WrongDescriptorKind {
        expected: type_name.to_string(),
        found: holon.holon_descriptor()?.header().type_name()?.to_string(),
        descriptor: holon.summarize()?,
    })
}

/// Classifies `holon` by the nearest supported type in its descriptor lineage.
///
/// Walks the describing descriptor's `Extends` chain self-first. An ancestor
/// whose type name names a candidate matches only when it is that candidate's
/// canonical descriptor. Returns `None` when no ancestor is a supported type.
pub fn classify_holon_type<T: Clone>(
    holon: &HolonReference,
    candidates: &[(MapString, T)],
) -> Result<Option<T>, HolonError> {
    let descriptor = holon.holon_descriptor()?;
    for ancestor in walk_extends_chain(descriptor.holon()) {
        let ancestor = ancestor?;
        let type_name = TypeHeader::new(&ancestor).type_name()?;
        let Some((name, candidate)) = candidates.iter().find(|(name, _)| *name == type_name) else {
            continue;
        };
        if same_definition(&ancestor, &canonical_holon_type(holon, name)?) {
            return Ok(Some(candidate.clone()));
        }
    }
    Ok(None)
}

/// Resolves the canonical descriptor of concrete holon type `type_name` in
/// `holon`'s resolution context.
fn canonical_holon_type(
    holon: &HolonReference,
    type_name: &MapString,
) -> Result<HolonReference, HolonError> {
    resolve_core_descriptor(&holon.resolution_context()?, &holon_type_descriptor_key(type_name).0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::descriptors::test_support::{
        build_context, new_holon_type_descriptor, new_test_holon,
    };
    use crate::reference_layer::{TransientReference, WritableHolon};
    use type_names::CoreRelationshipTypeName;

    /// Stages a canonical `<type_name>.HolonType` descriptor, optionally extending `parent`.
    fn stage_type(
        context: &std::sync::Arc<crate::core_shared_objects::transactions::TransactionContext>,
        type_name: &str,
        parent: Option<&HolonReference>,
    ) -> HolonReference {
        let mut descriptor =
            new_holon_type_descriptor(context, &format!("{type_name}.HolonType"), type_name)
                .unwrap();
        if let Some(parent) = parent {
            descriptor
                .add_related_holons(CoreRelationshipTypeName::Extends, vec![parent.clone()])
                .unwrap();
        }
        context.mutation().stage_new_holon(descriptor).unwrap().into()
    }

    fn instance_of(
        context: &std::sync::Arc<crate::core_shared_objects::transactions::TransactionContext>,
        key: &str,
        descriptor: HolonReference,
    ) -> HolonReference {
        let mut holon: TransientReference = new_test_holon(context, key).unwrap();
        holon.with_descriptor(descriptor).unwrap();
        holon.into()
    }

    fn name(value: &str) -> MapString {
        MapString(value.to_string())
    }

    #[test]
    fn a_holon_has_its_own_type_and_every_ancestor_type() {
        let context = build_context();
        let expression = stage_type(&context, "Expression", None);
        let skip = stage_type(&context, "Skip", Some(&expression));
        let holon = instance_of(&context, "skip", skip.clone());

        assert!(has_type(&holon, &skip).unwrap());
        assert!(has_type(&holon, &expression).unwrap());
        require_holon_type(&holon, &name("Expression")).unwrap();
        require_holon_type(&holon, &name("Skip")).unwrap();
    }

    #[test]
    fn a_same_named_unrelated_descriptor_is_a_different_type() {
        let context = build_context();
        stage_type(&context, "Skip", None);
        let impostor = new_holon_type_descriptor(&context, "Impostor.Skip", "Skip").unwrap();
        let holon = instance_of(&context, "impostor", impostor.into());

        let error = require_holon_type(&holon, &name("Skip")).unwrap_err();
        assert!(
            matches!(&error, HolonError::WrongDescriptorKind { expected, found, .. }
                if expected == "Skip" && found == "Skip"),
            "unexpected error: {error:?}"
        );
        assert_eq!(classify_holon_type(&holon, &[(name("Skip"), 1)]).unwrap(), None);
    }

    #[test]
    fn classification_picks_the_nearest_supported_ancestor() {
        let context = build_context();
        let skip = stage_type(&context, "Skip", None);
        let paged = stage_type(&context, "PagedSkip", Some(&skip));
        let holon = instance_of(&context, "paged", paged);
        let candidates = [(name("Limit"), "limit"), (name("Skip"), "skip")];

        assert_eq!(classify_holon_type(&holon, &candidates).unwrap(), Some("skip"));
    }
}
