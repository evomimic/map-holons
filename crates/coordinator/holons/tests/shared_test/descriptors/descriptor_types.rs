// Shared types for Descriptors
use holons::helpers::define_local_target;
use holons::holon_reference::{HolonReference, LocalHolonReference};
use holons::holon_reference::HolonReference::*;
use holons::holon_types::{Holon};
use holons::relationship::RelationshipTarget;
use holons::relationship::RelationshipTarget::*;
use shared_types_holon::BaseType::*;
use derive_new::*;


use shared_types_holon::holon_node::{BaseValue, BaseType};
// TODO: Is SemanticVersion struct needed, since SemanticVersion is just a Holon?
//#[hdk_entry_helper]
#[derive(new, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct SemanticVersion {
    major: u8,
    minor: u8,
    patch: u8,
}

impl Default for SemanticVersion {
    fn default() -> Self {
        SemanticVersion {
            major: 0,
            minor: 0,
            patch: 1,
        }
    }
}

pub const TYPE_DESCRIPTION_TEMPLATE: "Descriptor for {}";