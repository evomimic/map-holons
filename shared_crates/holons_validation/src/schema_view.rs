//! Bounded Schema scheduling and separate staged ownership collections.
use crate::{
    assessment_support::{path, targets},
    handlers::finding,
    ValidationCollector,
};
use core_types::{CommitValidationViolationKind, HolonError};
use holons_core::{
    core_shared_objects::transactions::TransactionContext,
    descriptors::{resolve_schema_ownership, SchemaOwnershipKind, SchemaOwnershipResolution},
    DefinitionIdentity, HolonReference,
};
use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
};
use type_names::CoreRelationshipTypeName;

/// Staged declaration subjects grouped by their exact Schema owner.
/// Saved inverse membership is not a validation-subject discovery path.
pub(crate) struct ProspectiveSchema {
    pub schema: HolonReference,
    pub staged_components: Vec<HolonReference>,
    pub staged_rules: Vec<HolonReference>,
}

/// A staged commitment remains in the workset even if ownership cannot be resolved.
pub(crate) struct OwnedCandidate {
    pub subject: HolonReference,
    pub kind: SchemaOwnershipKind,
}

/// Identity-indexed scope built before any commitment is assessed.
pub(crate) struct SchemaWorkset {
    pub schemas: Vec<ProspectiveSchema>,
}

impl SchemaWorkset {
    pub fn prepare(
        context: &Arc<TransactionContext>,
        directly_staged: &[HolonReference],
        owned: &[OwnedCandidate],
        collector: &mut ValidationCollector,
    ) -> Result<Self, HolonError> {
        let mut schemas = Vec::new();
        let mut scheduled = HashSet::new();
        // Membership grouping is by exact owner definition, not candidate or ancestry.
        let mut staged_members = HashMap::<(DefinitionIdentity, bool), Vec<HolonReference>>::new();
        for schema in directly_staged {
            schedule(context, schema, &mut scheduled, &mut schemas)?;
        }
        for candidate in owned {
            // A successor adds its own membership; it does not remove its predecessor's.
            for owner in targets(&candidate.subject, ownership_edge(candidate.kind))? {
                schedule(context, &owner, &mut scheduled, &mut schemas)?;
            }
            let resolution = resolve_schema_ownership(&candidate.subject, candidate.kind)?;
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
                let owner_id = DefinitionIdentity::for_reference(owner, context)?;
                schedule(context, owner, &mut scheduled, &mut schemas)?;
                staged_members
                    .entry((owner_id, is_rule(candidate.kind)))
                    .or_default()
                    .push(candidate.subject.clone());
            }
        }
        for view in &mut schemas {
            let identity = DefinitionIdentity::for_reference(&view.schema, context)?;
            for (rule, destination) in
                [(false, &mut view.staged_components), (true, &mut view.staged_rules)]
            {
                // Deduplicate exact member definitions within this owner.
                let mut seen = HashSet::new();
                for member in staged_members.remove(&(identity.clone(), rule)).unwrap_or_default() {
                    if seen.insert(DefinitionIdentity::for_reference(&member, context)?) {
                        destination.push(member);
                    }
                }
            }
        }
        Ok(Self { schemas })
    }
}

fn schedule(
    context: &Arc<TransactionContext>,
    schema: &HolonReference,
    seen: &mut HashSet<DefinitionIdentity>,
    views: &mut Vec<ProspectiveSchema>,
) -> Result<(), HolonError> {
    let identity = DefinitionIdentity::for_reference(schema, context)?;
    if seen.insert(identity.clone()) {
        views.push(ProspectiveSchema {
            schema: identity.definition_reference(context, schema),
            staged_components: Vec::new(),
            staged_rules: Vec::new(),
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
