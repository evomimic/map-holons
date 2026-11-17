//! Agent Spaces domain operations through bound references; no execution authority is granted.
use crate::{
    core_shared_objects::transactions::TransactionContext, HolonReference, ReadableHolon,
    RelationshipReadHint, StagedReference, WritableHolon,
};
use base_types::MapString;
use core_types::HolonError;
use std::sync::Arc;
use type_names::{
    AgentSpaceHolonTypeName, AgentSpacePropertyTypeName, AgentSpaceRelationshipTypeName,
};

/// Personal naming data, independent of keys, principal identifiers and authority.
pub struct PersonalName {
    pub last_name: MapString,
    pub first_name: Option<MapString>,
    pub middle_name: Option<MapString>,
}

/// Current saved sovereign membership, not proof of historical eligibility or authority.
#[derive(Debug)]
pub enum SovereignAgent {
    Unclaimed,
    Agent(HolonReference),
    Ambiguous,
}

/// Resolve fresh sovereign membership through the Space's bound Reference Layer.
/// Snapshot members before resolving any target to avoid holding a collection lock.
pub fn sovereign_agent(space: &HolonReference) -> Result<SovereignAgent, HolonError> {
    let related = space.related_holons_with_hint(
        AgentSpaceRelationshipTypeName::IsInteriorOf,
        RelationshipReadHint::RequireFresh,
    )?;
    let members = related
        .read()
        .map_err(|e| HolonError::FailedToAcquireLock(e.to_string()))?
        .get_members()
        .clone();
    Ok(match members.as_slice() {
        [] => SovereignAgent::Unclaimed,
        [agent] => SovereignAgent::Agent(agent.clone()),
        _ => SovereignAgent::Ambiguous,
    })
}

/// Stage a keyless PersonAgent and declared interior association in an ordinary empty transaction.
/// The caller owns authorization, Commit and retention for remediation/resubmission.
pub fn stage_personal_agent(
    context: &Arc<TransactionContext>,
    names: PersonalName,
) -> Result<StagedReference, HolonError> {
    if context.is_bootstrap_provisioning() || !context.staged_references()?.is_empty() {
        return Err(HolonError::InvalidState(
            "Initial PersonAgent creation requires an ordinary empty transaction; retained work must be remediated and resubmitted".into(),
        ));
    }
    let space = context.get_space_holon()?.ok_or_else(|| {
        HolonError::InvalidState(
            "PersonAgent creation requires an existing local HolonSpace".into(),
        )
    })?;
    if !matches!(sovereign_agent(&space)?, SovereignAgent::Unclaimed) {
        return Err(HolonError::InvalidState(
            "A sovereign association is already recorded; do not create another PersonAgent".into(),
        ));
    }
    let descriptor = context.lookup().get_saved_holon_by_key(
        &AgentSpaceHolonTypeName::PersonAgent
            .descriptor_key(AgentSpaceHolonTypeName::BioSocialAgent),
    )?;
    let mut transient = context.mutation().new_holon(None)?;
    transient.with_descriptor(descriptor.into())?;
    if !transient.effective_descriptor()?.effective_key_rule()?.is_keyless()? {
        return Err(HolonError::InvalidState(
            "This installation's PersonAgent descriptor is not keyless; use compatible test state or explicitly update its schema".into(),
        ));
    }
    transient.with_property_value(AgentSpacePropertyTypeName::LastName, names.last_name)?;
    if let Some(value) = names.first_name {
        transient.with_property_value(AgentSpacePropertyTypeName::FirstName, value)?;
    }
    if let Some(value) = names.middle_name {
        transient.with_property_value(AgentSpacePropertyTypeName::MiddleName, value)?;
    }
    let mut person = context.mutation().stage_new_holon(transient)?;
    person.add_related_holons(AgentSpaceRelationshipTypeName::HasInterior, vec![space])?;
    Ok(person)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        core_shared_objects::holon::SavedHolon,
        descriptors::test_support::{build_context_with_saved_holons, replace_saved_relationship},
    };
    use base_types::MapInteger;
    use core_types::{HolonId, LocalId, PropertyMap};
    use std::collections::HashMap;
    use type_names::ToRelationshipName;

    #[test]
    fn sovereign_observation_refreshes_cached_absence_and_preserves_ambiguity(
    ) -> Result<(), HolonError> {
        let ids: Vec<_> = (1..=3).map(|n| LocalId(vec![n; 39])).collect();
        let context = build_context_with_saved_holons(
            ids.iter()
                .map(|id| SavedHolon::new(id.clone(), PropertyMap::new(), None, MapInteger(1)))
                .collect(),
            HashMap::new(),
        );
        let space_id = HolonId::Local(ids[0].clone());
        let agent_id = HolonId::Local(ids[1].clone());
        let other_id = HolonId::Local(ids[2].clone());
        let space = HolonReference::smart_from_id(context.space_read_handle(), space_id.clone());
        assert!(matches!(sovereign_agent(&space)?, SovereignAgent::Unclaimed));
        let name = AgentSpaceRelationshipTypeName::IsInteriorOf.to_relationship_name();
        replace_saved_relationship(
            &context,
            space_id.clone(),
            name.clone(),
            vec![agent_id.clone()],
        );
        let SovereignAgent::Agent(agent) = sovereign_agent(&space)? else {
            panic!("fresh association")
        };
        assert_eq!(agent.holon_id()?, agent_id);
        replace_saved_relationship(&context, space_id, name, vec![agent_id, other_id]);
        assert!(matches!(sovereign_agent(&space)?, SovereignAgent::Ambiguous));
        Ok(())
    }

    #[test]
    fn unavailable_space_is_not_reported_as_unclaimed() {
        let context = build_context_with_saved_holons(Vec::new(), HashMap::new());
        let missing = HolonReference::smart_from_id(
            context.space_read_handle(),
            HolonId::Local(LocalId(vec![42; 39])),
        );
        assert!(sovereign_agent(&missing).is_err());
    }
}
