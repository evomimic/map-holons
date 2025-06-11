// Shared types for Descriptors
use holons::helpers::define_local_target;
use holons::holon_reference::HolonReference::*;
use holons::holon_reference::{HolonReference, LocalHolonReference};
use holons::holon_types::Holon;
use holons::relationship::HolonCollection;
use holons::relationship::HolonCollection::*;

use derive_new::*;

use integrity_core_types::value_types::BaseValue;
// TODO: Is SemanticVersion struct needed, since SemanticVersion is just a Holon?
#[derive(new, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct SemanticVersion {
    major: u8,
    minor: u8,
    patch: u8,
}

impl Default for SemanticVersion {
    fn default() -> Self {
        SemanticVersion { major: 0, minor: 0, patch: 1 }
    }
}

pub const TYPE_DESCRIPTION_TEMPLATE: &str = "Descriptor for {}";
