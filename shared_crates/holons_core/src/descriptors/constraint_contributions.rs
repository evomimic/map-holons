use super::{
    inheritance::relationship_targets_in_lineage, EffectiveRelationshipMember, ValidExtendsLineage,
};
use crate::{DefinitionIdentity, HolonReference};
use core_types::HolonError;
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
    /// Supply a lineage diagnosed from the same unchanged input snapshot.
    pub fn resolve(lineage: ValidExtendsLineage<'_>) -> Result<Self, HolonError> {
        let parent = lineage.members().get(1).cloned();
        let inherited = relationship_targets_in_lineage(
            &lineage.members()[1..],
            CoreRelationshipTypeName::Constraints,
        )?;
        let effective = lineage.relationship_targets(CoreRelationshipTypeName::Constraints)?;
        Ok(Self { parent, inherited, effective })
    }

    /// Finds lost definition identities or original declaring descriptors in linear time.
    /// Mutable references must belong to the same transaction.
    /// No configuration comparison is involved: retained obligations still apply.
    pub fn missing_inherited(&self) -> Result<Vec<&EffectiveRelationshipMember>, HolonError> {
        let context = self
            .effective
            .iter()
            .chain(&self.inherited)
            .flat_map(|contribution| [&contribution.member, &contribution.declared_on])
            .find_map(HolonReference::transaction_context);
        let identity = |reference: &HolonReference| {
            DefinitionIdentity::for_optional_context(reference, context.as_ref())
        };
        let identities = |contribution: &EffectiveRelationshipMember| {
            Ok((identity(&contribution.member)?, identity(&contribution.declared_on)?))
        };
        let retained = self
            .effective
            .iter()
            .map(identities)
            .collect::<Result<std::collections::HashSet<_>, HolonError>>()?;
        let mut missing = Vec::new();
        for contribution in &self.inherited {
            if !retained.contains(&identities(contribution)?) {
                missing.push(contribution);
            }
        }
        Ok(missing)
    }
}
