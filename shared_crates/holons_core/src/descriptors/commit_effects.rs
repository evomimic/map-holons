//! Conservative invalidation of prepared products from Commit's physical write surface.
use std::collections::HashSet;

use core_types::{LocalId, RelationshipName};

/// Effects accumulated during one persistence attempt, independent of registry contents.
/// Exact node versions keep their property content; mutable relationship collections
/// can change the inputs used by EHD/DD construction. No reference resolution is needed.
#[derive(Debug, Default)]
pub struct DescriptorCommitEffects {
    new_nodes: HashSet<LocalId>,
    reset_required: bool,
}

impl DescriptorCommitEffects {
    /// Records successful root or new-version publication, never a graph-only update.
    pub fn record_new_node(&mut self, id: LocalId) {
        self.new_nodes.insert(id);
    }

    /// Observes an attempted physical forward or inverse relationship write.
    /// Idempotent replay can conservatively invalidate too. Freshness is local to
    /// this attempt; a previously committed retry source cannot claim initialization.
    pub fn record_relationship_write(&mut self, source: &LocalId, name: &RelationshipName) {
        self.reset_required |= match name.0 .0.as_str() {
            "DescribedBy" => !self.new_nodes.contains(source),
            "Extends"
            | "InstanceProperties"
            | "InstanceRelationships"
            | "AffordsDance"
            | "AffordsCommand"
            | "InstanceKeyRule"
            | "Constraints"
            | "ValidationBindings"
            | "AffordsOperator"
            | "SourceOf"
            | "SourceType" => true,
            _ => false,
        };
    }

    /// Whether any observed effect can change a prepared descriptor input.
    pub fn reset_required(&self) -> bool {
        self.reset_required
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use type_names::{CoreRelationshipTypeName as R, ToRelationshipName};

    fn id(value: u8) -> LocalId {
        LocalId(vec![value; 39])
    }

    #[test]
    fn ordinary_instance_versions_and_links_preserve_prepared_products() {
        let mut effects = DescriptorCommitEffects::default();
        effects.record_new_node(id(1));
        for name in [R::DescribedBy, R::OwnedBy, R::Predecessor] {
            effects.record_relationship_write(&id(1), &name.to_relationship_name());
        }
        for name in [R::Instances, R::Owns, R::Successor] {
            effects.record_relationship_write(&id(2), &name.to_relationship_name());
        }
        effects.record_relationship_write(&id(2), &"Authors".to_relationship_name());
        assert!(!effects.reset_required());
    }

    #[test]
    fn every_prepared_input_invalidates_even_on_a_new_source() {
        for name in [
            R::Extends,
            R::InstanceProperties,
            R::InstanceRelationships,
            R::AffordsDance,
            R::AffordsCommand,
            R::InstanceKeyRule,
            R::Constraints,
            R::ValidationBindings,
            R::AffordsOperator,
            R::SourceOf,
            R::SourceType,
        ] {
            let mut effects = DescriptorCommitEffects::default();
            effects.record_new_node(id(1));
            effects.record_relationship_write(&id(1), &name.clone().to_relationship_name());
            assert!(effects.reset_required(), "{name:?}");
        }
    }

    #[test]
    fn existing_or_retry_sources_do_not_claim_described_by_initialization() {
        let mut effects = DescriptorCommitEffects::default();
        effects.record_new_node(id(1));
        effects.record_relationship_write(&id(2), &R::DescribedBy.to_relationship_name());
        effects.record_relationship_write(&id(1), &R::OwnedBy.to_relationship_name());
        assert!(effects.reset_required());
    }

    #[test]
    fn inverse_write_on_an_unstaged_type_invalidates() {
        let mut effects = DescriptorCommitEffects::default();
        effects.record_new_node(id(1));
        effects.record_relationship_write(&id(1), &"Other".to_relationship_name());
        effects.record_relationship_write(&id(2), &R::SourceOf.to_relationship_name());
        assert!(effects.reset_required());
    }
}
