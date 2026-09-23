//! Bounded Schema scheduling and separate prospective ownership collections.
use crate::{
    assessment_support::{blocked, path, recover, targets},
    handlers::finding,
    ValidationCollector,
};
use core_types::{CommitValidationViolationKind, HolonError, HolonId};
use holons_core::{
    core_shared_objects::transactions::TransactionContext,
    descriptors::{
        resolve_schema_ownership_with_reader, SchemaOwnershipKind, SchemaOwnershipResolution,
    },
    DescriptorReader, HolonReference, ProspectiveDescriptorReader, ProspectiveIdentity,
    StagedReference,
};
use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
};
use type_names::CoreRelationshipTypeName;

/// The two membership namespaces are intentionally independent.
pub(crate) struct ProspectiveSchema {
    pub schema: HolonReference,
    pub components: Vec<HolonReference>,
    pub rules: Vec<HolonReference>,
}

/// A staged commitment remains in the workset even if ownership cannot be resolved.
pub(crate) struct OwnedCandidate {
    pub subject: HolonReference,
    pub kind: SchemaOwnershipKind,
}

/// Identity-indexed scope built before any commitment is assessed.
pub(crate) struct SchemaWorkset {
    pub schemas: Vec<ProspectiveSchema>,
    pub ownership: HashMap<ProspectiveIdentity, Vec<HolonReference>>,
}

impl SchemaWorkset {
    pub fn prepare(
        context: &Arc<TransactionContext>,
        reader: &ProspectiveDescriptorReader,
        schema_type: &HolonReference,
        directly_staged: &[HolonReference],
        owned: &[OwnedCandidate],
        collector: &mut ValidationCollector,
    ) -> Result<Self, HolonError> {
        let mut schemas = Vec::new();
        let mut scheduled = HashSet::new();
        let mut ownership = HashMap::new();
        let mut staged_members = HashMap::<(ProspectiveIdentity, bool), Vec<HolonReference>>::new();
        let mut changed = HashSet::new();
        for schema in directly_staged {
            schedule(context, schema, &mut scheduled, &mut schemas)?;
        }
        for candidate in owned {
            let identity = ProspectiveIdentity::for_reference(&candidate.subject, context)?;
            // Discover the old owner from saved content, never through replacement selection.
            // Removing membership affects that Schema even if prospective ownership is invalid.
            if let ProspectiveIdentity::Saved(source) = &identity {
                let saved =
                    HolonReference::smart_from_id(context.space_read_handle(), source.clone());
                for owner in targets(&saved, ownership_edge(candidate.kind))? {
                    schedule(context, &owner, &mut scheduled, &mut schemas)?;
                }
            }
            // Retain readable authored owner identities even when selecting one owner's
            // describing content is blocked. Contention must not erase aggregate scheduling.
            if let Some(selected) =
                recover(reader.select(&candidate.subject), &candidate.subject, collector)?
            {
                for owner in targets(&selected, ownership_edge(candidate.kind))? {
                    schedule(context, &owner, &mut scheduled, &mut schemas)?;
                }
            }
            let Some(resolution) = recover(
                resolve_schema_ownership_with_reader(
                    &candidate.subject,
                    candidate.kind,
                    schema_type,
                    reader,
                ),
                &candidate.subject,
                collector,
            )?
            else {
                ownership.insert(identity, Vec::new());
                continue;
            };
            changed.insert(identity.clone());
            let (owners, defect) = match resolution {
                SchemaOwnershipResolution::OwnedBy(owner) => (vec![owner], None),
                SchemaOwnershipResolution::Missing => {
                    (Vec::new(), Some("missing owner".to_string()))
                }
                SchemaOwnershipResolution::Multiple(owners) => {
                    let count = owners.len();
                    (owners, Some(format!("{count} owners")))
                }
                SchemaOwnershipResolution::InvalidOwner { owner, .. } => {
                    (vec![owner], Some("target is not described by a Schema type".into()))
                }
                SchemaOwnershipResolution::MalformedOwnerLineage { owner, defect } => {
                    (vec![owner], Some(format!("owner has malformed describing lineage: {defect}")))
                }
            };
            if let Some(defect) = defect {
                finding(
                    collector,
                    CommitValidationViolationKind::RuleViolation { code: "SchemaOwnership".into() },
                    None,
                    &path(&candidate.subject),
                    None,
                    format!(
                        "{} requires exactly one Schema through {}: {defect}.",
                        candidate.subject.reference_id_string(),
                        ownership_edge(candidate.kind).as_relationship_name()
                    ),
                );
            }
            for owner in &owners {
                let owner_id = ProspectiveIdentity::for_reference(owner, context)?;
                schedule(context, owner, &mut scheduled, &mut schemas)?;
                staged_members
                    .entry((owner_id, is_rule(candidate.kind)))
                    .or_default()
                    .push(candidate.subject.clone());
            }
            ownership.insert(identity, owners);
        }
        for view in &mut schemas {
            let identity = ProspectiveIdentity::for_reference(&view.schema, context)?;
            if let Some(selected) = recover(reader.select(&view.schema), &view.schema, collector)? {
                view.schema = selected;
            } else {
                blocked(
                    collector,
                    &view.schema,
                    "Schema aggregate has no authoritative prospective definition.".into(),
                );
            }
            // Read persisted inverse membership from the saved source. A staged Schema clone
            // intentionally omits materialized inverses and is not a complete membership view.
            let saved = match &identity {
                ProspectiveIdentity::Saved(id) => {
                    Some(HolonReference::smart_from_id(context.space_read_handle(), id.clone()))
                }
                _ => None,
            };
            for (edge, rule, destination) in [
                (CoreRelationshipTypeName::Components, false, &mut view.components),
                (CoreRelationshipTypeName::Rules, true, &mut view.rules),
            ] {
                let mut seen = HashSet::new();
                if let Some(saved) = &saved {
                    for member in targets(saved, edge)? {
                        let member_id = ProspectiveIdentity::for_reference(&member, context)?;
                        if changed.contains(&member_id) || !seen.insert(member_id) {
                            continue;
                        }
                        match reader.select(&member) {
                            Ok(selected) => destination.push(selected),
                            Err(error) => {
                                recover::<HolonReference>(Err(error), &view.schema, collector)?;
                                // Keep the identity, without falling back to its saved contents.
                                destination.push(member);
                            }
                        }
                    }
                }
                for member in staged_members.remove(&(identity.clone(), rule)).unwrap_or_default() {
                    if seen.insert(ProspectiveIdentity::for_reference(&member, context)?) {
                        destination.push(member);
                    }
                }
            }
        }
        Ok(Self { schemas, ownership })
    }
}

fn schedule(
    context: &Arc<TransactionContext>,
    schema: &HolonReference,
    seen: &mut HashSet<ProspectiveIdentity>,
    views: &mut Vec<ProspectiveSchema>,
) -> Result<(), HolonError> {
    if seen.insert(ProspectiveIdentity::for_reference(schema, context)?) {
        views.push(ProspectiveSchema {
            schema: schema.clone(),
            components: Vec::new(),
            rules: Vec::new(),
        });
    }
    Ok(())
}
fn is_rule(kind: SchemaOwnershipKind) -> bool {
    matches!(kind, SchemaOwnershipKind::Rule)
}
pub(crate) fn ownership_edge(kind: SchemaOwnershipKind) -> CoreRelationshipTypeName {
    match kind {
        SchemaOwnershipKind::Component => CoreRelationshipTypeName::ComponentOf,
        SchemaOwnershipKind::Rule => CoreRelationshipTypeName::RuleOf,
    }
}

/// Saved ancestry is explicit; creates sharing a key never acquire a saved baseline.
pub(crate) fn saved_source(
    context: &Arc<TransactionContext>,
    candidate: &StagedReference,
) -> Result<Option<HolonReference>, HolonError> {
    Ok(candidate
        .versioned_source_id()?
        .map(|id| HolonReference::smart_from_id(context.space_read_handle(), HolonId::Local(id))))
}
