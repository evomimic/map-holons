//! This file offers the following benefits:
//! - Decouple identity from keys: Using test_labels avoids overloading holon keys as test-only identifiers
//! and allows fixtures to deliberately create duplicate keys when that’s the behavior under test.
//! - Cleaner fixture contracts: A test_label → TestReference map makes the fixture’s contract explicit, self-documenting, and stable across internal changes.
//! - Supports keyless holons naturally: Fixtures can now return references to holons that legitimately have no key at all.
use super::TestReference;
use core_types::RelationshipName;
use holons_prelude::prelude::MapString;
use std::collections::BTreeMap;

/// This is a pure utility type offered by, but NOT used by the harness.
/// It allows fixture libraries (like setup_book_and_authors_fixture) to populate a map of TestLabel-> TestReference
/// and relationships by assigned name.
/// The set of "labels" constitutes the contract between the helper function and its consumers.
/// Labels can be any text and don't necessarily have any relationship to a test Holons Key.
#[derive(Default)]
pub struct FixtureBindings {
    bindings: BTreeMap<MapString, TestReference>, // Label, Token
    relationship_name_map: BTreeMap<MapString, RelationshipName>,
}

impl FixtureBindings {
    pub fn insert_token(&mut self, label: MapString, token: TestReference) {
        self.bindings.insert(label, token);
    }

    pub fn get_token(&self, label: &MapString) -> Option<&TestReference> {
        self.bindings.get(label)
    }

    pub fn relationship_by_name(&self, label: &MapString) -> Option<&RelationshipName> {
        self.relationship_name_map.get(label)
    }

    pub fn set_relationship_name(&mut self, label: MapString, name: RelationshipName) {
        self.relationship_name_map.insert(label, name);
    }
}
