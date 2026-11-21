//! Agent Spaces domain operations through bound references; no execution authority is granted.
use crate::{
    core_shared_objects::transactions::TransactionContext, descriptors::Descriptor, HolonReference,
    ReadableHolon, RelationshipReadHint, SmartReference, StagedReference, WritableHolon,
};
use base_types::{MapString, ToBaseValue};
use core_types::HolonError;
use std::sync::Arc;
use type_names::{
    AgentSpaceHolonTypeName, AgentSpacePropertyTypeName, AgentSpaceRelationshipTypeName,
};

/// Personal naming inputs; a composed key grants no authority.
pub struct PersonalName {
    pub last_name: MapString,
    pub first_name: Option<MapString>,
    pub middle_name: Option<MapString>,
}

impl PersonalName {
    /// The PersonAgentName rule's last/first/middle order, omitting absent optional names.
    pub fn person_key(&self) -> Result<MapString, HolonError> {
        self.require_last_name()?;
        Ok(MapString(
            [Some(&self.last_name), self.first_name.as_ref(), self.middle_name.as_ref()]
                .into_iter()
                .flatten()
                .map(|name| name.0.as_str())
                .collect::<Vec<_>>()
                .join("."),
        ))
    }

    /// The personal Space's actual name and HolonSpaceNameRule key.
    pub fn space_name(&self) -> Result<MapString, HolonError> {
        self.require_last_name()?;
        let full_name =
            [self.first_name.as_ref(), self.middle_name.as_ref(), Some(&self.last_name)]
                .into_iter()
                .flatten()
                .map(|name| name.0.as_str())
                .collect::<Vec<_>>()
                .join(" ");
        Ok(MapString(format!("{full_name}'s Personal Space")))
    }

    fn require_last_name(&self) -> Result<(), HolonError> {
        if self.last_name.0.trim().is_empty()
            || [self.first_name.as_ref(), self.middle_name.as_ref()]
                .into_iter()
                .flatten()
                .any(|name| name.0.trim().is_empty())
        {
            return Err(HolonError::InvalidParameter(
                "Last name is required; absent optional names must be omitted".into(),
            ));
        }
        Ok(())
    }
}

/// Resolve the current saved Space version while retaining its adopted root for infrastructure.
/// Follow persisted Successor references; an old key is historical discovery data,
/// not an alias for a renamed head. Final lookup verifies the head is currently visible.
pub fn current_personal_space(
    context: &Arc<TransactionContext>,
) -> Result<HolonReference, HolonError> {
    let root = context.get_space_holon()?.ok_or_else(|| {
        HolonError::InvalidState("Startup has not adopted a local HolonSpace".into())
    })?;
    let root_reference = SmartReference::new_from_id(context.space_read_handle(), root.holon_id()?);
    let root_id = root_reference.lineage_root_id()?;
    let mut visited = std::collections::HashSet::new();
    let mut active_path = std::collections::HashSet::new();
    let mut heads = Vec::new();
    let mut pending = vec![(root_reference, false)];
    while let Some((current, leaving)) = pending.pop() {
        if leaving {
            active_path.remove(&current.holon_id());
            continue;
        }
        if active_path.contains(&current.holon_id()) || current.lineage_root_id()? != root_id {
            return Err(HolonError::InvalidState("Invalid local Space successor lineage".into()));
        }
        if !visited.insert(current.holon_id()) {
            continue;
        }
        active_path.insert(current.holon_id());
        pending.push((current.clone(), true));
        let successors = current.related_holons_with_hint(
            type_names::CoreRelationshipTypeName::Successor,
            RelationshipReadHint::RequireFresh,
        )?;
        let members = successors
            .read()
            .map_err(|error| HolonError::FailedToAcquireLock(error.to_string()))?
            .get_members()
            .clone();
        if members.is_empty() {
            heads.push(current);
        } else {
            for next in members {
                pending.push((
                    SmartReference::new_from_id(context.space_read_handle(), next.holon_id()?),
                    false,
                ));
            }
        }
    }
    let [current] = heads.as_slice() else {
        return Err(HolonError::InvalidState("Local Space has competing successor heads".into()));
    };
    if visited.len() == 1 {
        return Ok(current.clone().into());
    }
    let key =
        current.key()?.ok_or_else(|| HolonError::InvalidState("Local Space has no key".into()))?;
    if context.lookup().get_saved_holon_by_key(&key)?.holon_id() != current.holon_id() {
        return Err(HolonError::InvalidState("Local Space head is not uniquely visible".into()));
    }
    Ok(current.clone().into())
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

/// Stage the named PersonAgent, a named Space version, and their interior association together.
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
    let space = current_personal_space(context)?;
    if !matches!(sovereign_agent(&space)?, SovereignAgent::Unclaimed) {
        return Err(HolonError::InvalidState(
            "A sovereign association is already recorded; do not create another PersonAgent".into(),
        ));
    }
    let descriptor = context.lookup().get_saved_holon_by_key(
        &AgentSpaceHolonTypeName::PersonAgent
            .descriptor_key(AgentSpaceHolonTypeName::BioSocialAgent),
    )?;
    let person_key = names.person_key()?;
    let space_name = names.space_name()?;
    let mut transient = context.mutation().new_holon(Some(person_key))?;
    transient.with_descriptor(descriptor.into())?;
    // This domain producer supports the configured personal-name contract. Reject
    // other configurations rather than treating every keyed strategy as a name rule.
    let selected_rule = transient.effective_descriptor()?.effective_key_rule()?;
    let name_rule =
        context.lookup().get_saved_holon_by_key(&"PersonAgentName.FormatRule".into())?;
    if selected_rule.holon().holon_id()? != name_rule.holon_id()
        || name_rule.property_value("TemplateString")? != Some("{0}.[1].[2]".to_base_value())
    {
        return Err(HolonError::InvalidState(
            "Personal setup requires the configured PersonAgentName rule".into(),
        ));
    }
    let parameters = name_rule.related_holons("TemplateParameters")?;
    let parameters = parameters
        .read()
        .map_err(|error| HolonError::FailedToAcquireLock(error.to_string()))?
        .get_members()
        .iter()
        .map(|parameter| parameter.holon_id())
        .collect::<Result<Vec<_>, _>>()?;
    let expected = ["LastName.PropertyType", "FirstName.PropertyType", "MiddleName.PropertyType"]
        .into_iter()
        .map(|key| {
            context
                .lookup()
                .get_saved_holon_by_key(&key.into())
                .map(|parameter| parameter.holon_id())
        })
        .collect::<Result<Vec<_>, _>>()?;
    if parameters != expected {
        return Err(HolonError::InvalidState(
            "Personal name rule parameters are incompatible".into(),
        ));
    }
    transient.with_property_value(AgentSpacePropertyTypeName::LastName, names.last_name)?;
    if let Some(value) = names.first_name {
        transient.with_property_value(AgentSpacePropertyTypeName::FirstName, value)?;
    }
    if let Some(value) = names.middle_name {
        transient.with_property_value(AgentSpacePropertyTypeName::MiddleName, value)?;
    }
    let mut named_space = context.mutation().stage_new_version(SmartReference::new_from_id(
        context.space_read_handle(),
        space.holon_id()?,
    ))?;
    named_space.with_property_value(type_names::CorePropertyTypeName::Key, space_name.clone())?;
    named_space.with_property_value(
        type_names::CorePropertyTypeName::HolonSpaceName,
        space_name.clone(),
    )?;
    named_space.with_property_value(type_names::CorePropertyTypeName::DisplayName, space_name)?;
    named_space.with_property_value(
        type_names::CorePropertyTypeName::Description,
        "Your personal Space, including its MAP foundation.",
    )?;
    let mut person = context.mutation().stage_new_holon(transient)?;
    person.add_related_holons(
        AgentSpaceRelationshipTypeName::HasInterior,
        vec![named_space.into()],
    )?;
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
    fn personal_identity_names_use_their_declared_orders_and_omit_absent_names() {
        for (first, middle, person, space) in [
            (None, None, "Melville", "Melville's Personal Space"),
            (Some("Steve"), None, "Melville.Steve", "Steve Melville's Personal Space"),
            (None, Some("James"), "Melville.James", "James Melville's Personal Space"),
            (
                Some("Steve"),
                Some("James"),
                "Melville.Steve.James",
                "Steve James Melville's Personal Space",
            ),
        ] {
            let names = PersonalName {
                last_name: "Melville".into(),
                first_name: first.map(MapString::from),
                middle_name: middle.map(MapString::from),
            };
            assert_eq!(names.person_key().unwrap().0, person);
            assert_eq!(names.space_name().unwrap().0, space);
        }
        let invalid = PersonalName { last_name: " ".into(), first_name: None, middle_name: None };
        assert!(invalid.person_key().is_err());
        assert!(invalid.space_name().is_err());
    }

    #[test]
    fn current_space_follows_renames_and_merges_but_rejects_forks_and_cycles() {
        use base_types::ToBaseValue;
        use type_names::ToPropertyName;
        let ids: Vec<_> = (1..=4).map(|n| LocalId(vec![n; 39])).collect();
        let context = build_context_with_saved_holons(
            ids.iter()
                .enumerate()
                .map(|(index, id)| {
                    let mut properties = PropertyMap::new();
                    properties.insert(
                        type_names::CorePropertyTypeName::Key.to_property_name(),
                        format!("space-{index}").to_base_value(),
                    );
                    SavedHolon::new(
                        id.clone(),
                        properties,
                        (index != 0).then(|| ids[0].clone()),
                        MapInteger(index as i64 + 1),
                    )
                })
                .collect(),
            HashMap::new(),
        );
        let holon_ids: Vec<_> = ids.into_iter().map(HolonId::Local).collect();
        context.set_space_holon_id(holon_ids[0].clone()).unwrap();
        let successor = type_names::CoreRelationshipTypeName::Successor.to_relationship_name();
        let set = |source: usize, targets: &[usize]| {
            replace_saved_relationship(
                &context,
                holon_ids[source].clone(),
                successor.clone(),
                targets.iter().map(|index| holon_ids[*index].clone()).collect(),
            )
        };
        set(0, &[1]);
        assert_eq!(current_personal_space(&context).unwrap().holon_id().unwrap(), holon_ids[1]);
        set(0, &[1, 2]);
        assert!(current_personal_space(&context).is_err());
        set(1, &[3]);
        set(2, &[3]);
        assert_eq!(current_personal_space(&context).unwrap().holon_id().unwrap(), holon_ids[3]);
        set(3, &[0]);
        assert!(current_personal_space(&context).is_err());
    }

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
