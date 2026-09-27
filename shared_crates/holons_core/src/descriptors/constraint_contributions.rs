use super::{
    definition_identity::definition_identity, effective_relationship_targets_with_reader,
    DescriptorReader, EffectiveRelationshipMember, ValidExtendsLineage,
};
use crate::HolonReference;
use type_names::CoreRelationshipTypeName;

/// Parent and child effective obligations, retaining attachment provenance before normalization.
/// A broader local contribution cannot erase a retained inherited obligation.
#[derive(Clone, Debug)]
pub struct ConstraintContributions {
    pub parent: Option<HolonReference>,
    pub inherited: Vec<EffectiveRelationshipMember>,
    pub effective: Vec<EffectiveRelationshipMember>,
}

impl ConstraintContributions {
    /// Reads both effective sets through the kernel's additive inheritance policy.
    /// Supply a lineage diagnosed using the same reader and unchanged input snapshot.
    pub fn resolve_with_reader<R: DescriptorReader>(
        lineage: ValidExtendsLineage<'_>,
        reader: &R,
    ) -> Result<Self, R::Error> {
        let parent = lineage.members().get(1).cloned();
        let inherited = match &parent {
            Some(parent) => effective_relationship_targets_with_reader(
                parent,
                CoreRelationshipTypeName::Constraints,
                reader,
            )?,
            None => Vec::new(),
        };
        let effective = effective_relationship_targets_with_reader(
            lineage.subject(),
            CoreRelationshipTypeName::Constraints,
            reader,
        )?;
        Ok(Self { parent, inherited, effective })
    }

    /// Finds lost identities or original declaring descriptors in linear time.
    /// All references must already be selected in the same prospective view.
    /// No configuration comparison is involved: retained obligations still apply.
    pub fn missing_inherited(&self) -> Vec<&EffectiveRelationshipMember> {
        let retained: std::collections::HashSet<_> = self
            .effective
            .iter()
            .map(|contribution| {
                (
                    definition_identity(&contribution.member),
                    definition_identity(&contribution.declared_on),
                )
            })
            .collect();
        self.inherited
            .iter()
            .filter(|contribution| {
                !retained.contains(&(
                    definition_identity(&contribution.member),
                    definition_identity(&contribution.declared_on),
                ))
            })
            .collect()
    }
}
