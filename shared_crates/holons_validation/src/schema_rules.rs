//! Schema rule products over the bounded prospective workset.
use crate::{
    assessment_support::{blocked, recover, targets},
    handlers::rule_violation,
    schema_view::{ProspectiveSchema, SchemaWorkset},
    RuleOutcome, ValidationCollector, ValidationInvocation,
};
use core_types::{HolonError, RelationshipName};
use holons_core::{
    core_shared_objects::transactions::TransactionContext,
    descriptors::{resolve_describing_type_with_reader, schema_dependencies_with_reader},
    AssessmentReadError, ContractContributions, DescribingTypeResolution, DescriptorReader,
    HolonReference, ProspectiveDescriptorReader, ProspectiveIdentity, ReadableHolon,
};
use std::{
    collections::{BTreeMap, HashMap, HashSet},
    sync::Arc,
};
use type_names::{CorePropertyTypeName, CoreRelationshipTypeName, CoreValidationRuleName};

/// Prepared once per affected Schema; bound handlers never repeat graph traversal.
#[derive(Default)]
pub struct SchemaRuleProducts {
    diagnostics: BTreeMap<CoreValidationRuleName, Vec<String>>,
}

/// One diagnostic per affected identity, with a bounded example of the authored edges.
struct ReferenceGroup {
    definition: HolonReference,
    owner_count: usize,
    count: usize,
    example: (String, String, String),
}

impl ReferenceGroup {
    fn add(&mut self, example: (String, String, String)) {
        self.count += 1;
        if example < self.example {
            self.example = example;
        }
    }
}

fn grouped_references(groups: HashMap<ProspectiveIdentity, ReferenceGroup>) -> Vec<ReferenceGroup> {
    let mut groups: Vec<_> = groups.into_iter().collect();
    // Diagnostic reference strings are truncated; order by complete identity bytes.
    groups.sort_by_cached_key(|(identity, _)| match identity {
        ProspectiveIdentity::Saved(id) => (0, id.to_canonical_bytes()),
        ProspectiveIdentity::Staged(id) => (1, id.0.as_bytes().to_vec()),
        ProspectiveIdentity::Transient(id) => (2, id.0.as_bytes().to_vec()),
    });
    groups.into_iter().map(|(_, group)| group).collect()
}
impl SchemaRuleProducts {
    pub(crate) fn record(&mut self, rule: CoreValidationRuleName, message: String) {
        self.diagnostics.entry(rule).or_default().push(message);
    }
    pub(crate) fn rules_with_findings(&self) -> impl Iterator<Item = CoreValidationRuleName> + '_ {
        self.diagnostics.keys().copied()
    }

    pub(crate) fn has_findings(&self) -> bool {
        !self.diagnostics.is_empty()
    }
}

/// Uses an explicit DFS stack so a long versioned dependency chain cannot exhaust guest WASM.
/// Memoization spans the entire aggregate scope; shared dependency suffixes are read once.
pub(crate) fn dependency_cycles(
    context: &Arc<TransactionContext>,
    reader: &ProspectiveDescriptorReader,
    schemas: &[ProspectiveSchema],
    collector: &mut ValidationCollector,
) -> Result<HashMap<ProspectiveIdentity, Option<HolonReference>>, HolonError> {
    let mut results = HashMap::new();
    let mut active = HashSet::new();
    for view in schemas {
        let mut stack = vec![(view.schema.clone(), false, Vec::<HolonReference>::new())];
        while let Some((schema, leaving, dependencies)) = stack.pop() {
            let id = ProspectiveIdentity::for_reference(&schema, context)?;
            if leaving {
                active.remove(&id);
                let mut witness = results.get(&id).cloned().flatten();
                for dependency in dependencies {
                    let dependency_id = ProspectiveIdentity::for_reference(&dependency, context)?;
                    witness = witness.or_else(|| results.get(&dependency_id).cloned().flatten());
                }
                results.insert(id, witness);
            } else if active.contains(&id) {
                results.insert(id, Some(schema));
            } else if !results.contains_key(&id) {
                let Some(dependencies) = recover(
                    schema_dependencies_with_reader(&schema, reader),
                    &view.schema,
                    collector,
                )?
                else {
                    continue;
                };
                active.insert(id);
                stack.push((schema, true, dependencies.clone()));
                for dependency in dependencies.into_iter().rev() {
                    stack.push((dependency, false, Vec::new()));
                }
            }
        }
    }
    Ok(results)
}

/// Cross-schema declarations are local edges. Transitive lookup never licenses an edge.
pub(crate) fn cross_schema_references(
    context: &Arc<TransactionContext>,
    reader: &ProspectiveDescriptorReader,
    view: &ProspectiveSchema,
    workset: &SchemaWorkset,
    products: &mut SchemaRuleProducts,
    collector: &mut ValidationCollector,
) -> Result<(), HolonError> {
    let Some(dependencies) =
        recover(schema_dependencies_with_reader(&view.schema, reader), &view.schema, collector)?
    else {
        return Ok(());
    };
    let local = ProspectiveIdentity::for_reference(&view.schema, context)?;
    let direct = dependencies
        .iter()
        .map(|target| ProspectiveIdentity::for_reference(target, context))
        .collect::<Result<HashSet<_>, _>>()?;
    let mut owners = HashMap::new();
    let mut ambiguous = HashMap::<ProspectiveIdentity, ReferenceGroup>::new();
    let mut missing_dependencies = HashMap::<ProspectiveIdentity, ReferenceGroup>::new();
    for source in view.components.iter().chain(&view.rules) {
        let Some(edges) = recover(authored_targets(source, reader), &view.schema, collector)?
        else {
            continue;
        };
        for (name, target) in edges {
            let target_id = ProspectiveIdentity::for_reference(&target, context)?;
            if !owners.contains_key(&target_id) && !workset.ownership.contains_key(&target_id) {
                let Some(selected) = recover(reader.select(&target), &view.schema, collector)?
                else {
                    continue;
                };
                // Schema-valued references (notably ComponentOf/RuleOf) name their owner directly.
                let targets = if name
                    == CoreRelationshipTypeName::ComponentOf.as_relationship_name()
                    || name == CoreRelationshipTypeName::RuleOf.as_relationship_name()
                {
                    vec![selected]
                } else {
                    let mut result = targets(&selected, CoreRelationshipTypeName::ComponentOf)?;
                    result.extend(targets(&selected, CoreRelationshipTypeName::RuleOf)?);
                    result
                };
                owners.insert(target_id.clone(), targets);
            }
            let target_owners =
                workset.ownership.get(&target_id).or_else(|| owners.get(&target_id)).ok_or_else(
                    || HolonError::CommitFailure("Missing assessed ownership".into()),
                )?;
            // Spaces, Schema holons, and other unowned instances are outside this rule.
            // A staged definition missing required ownership is diagnosed by the workset.
            if target_owners.is_empty() {
                continue;
            }
            let example =
                (source.reference_id_string(), name.to_string(), target.reference_id_string());
            if target_owners.len() > 1 {
                ambiguous
                    .entry(target_id)
                    .or_insert_with(|| ReferenceGroup {
                        definition: target.clone(),
                        owner_count: target_owners.len(),
                        count: 0,
                        example: example.clone(),
                    })
                    .add(example);
                continue;
            }
            let owner = &target_owners[0];
            let owner_id = ProspectiveIdentity::for_reference(owner, context)?;
            if owner_id != local && !direct.contains(&owner_id) {
                missing_dependencies
                    .entry(owner_id)
                    .or_insert_with(|| ReferenceGroup {
                        definition: owner.clone(),
                        owner_count: 1,
                        count: 0,
                        example: example.clone(),
                    })
                    .add(example);
            }
        }
    }
    for group in grouped_references(ambiguous) {
        let (source, name, target) = group.example;
        blocked(collector, &view.schema, format!(
            "Cannot check {} authored reference(s) to {}: target has {} prospective owners; for example {source} -[{name}]-> {target}.",
            group.count, group.definition.reference_id_string(), group.owner_count
        ));
    }
    for group in grouped_references(missing_dependencies) {
        let (source, name, target) = group.example;
        products.record(CoreValidationRuleName::CrossSchemaDependenciesDeclared, format!(
            "Schema {} must directly DependsOn {} for {} authored reference(s); for example {source} -[{name}]-> {target}. Transitive reachability is insufficient.",
            view.schema.reference_id_string(), group.definition.reference_id_string(), group.count
        ));
    }
    Ok(())
}

/// Saved maps include materialized inverse navigation. Their authored surface is the
/// declared contract, as in saved-to-transient cloning. Mutable maps are authored state
/// and retain undeclared names so they cannot evade the fixed Commit check.
pub(crate) fn authored_targets(
    source: &HolonReference,
    reader: &ProspectiveDescriptorReader,
) -> Result<Vec<(RelationshipName, HolonReference)>, AssessmentReadError> {
    let source = reader.select(source)?;
    let declared = if matches!(source, HolonReference::Smart(_)) {
        let DescribingTypeResolution::Unique(descriptor) =
            resolve_describing_type_with_reader(&source, reader)?
        else {
            return Err(
                HolonError::MissingDescribedBy { holon: source.reference_id_string() }.into()
            );
        };
        let contributions = ContractContributions::resolve_with_reader(&descriptor, reader)?;
        Some(
            contributions
                .relationships
                .iter()
                .map(|member| {
                    match member.member.property_value(CorePropertyTypeName::TypeName)? {
                        Some(core_types::BaseValue::StringValue(name)) => Ok(name.to_string()),
                        _ => Err(HolonError::EmptyField("TypeName".into())),
                    }
                })
                .collect::<Result<HashSet<_>, HolonError>>()?,
        )
    } else {
        None
    };
    let mut edges = source.all_related_holons()?.iter();
    edges.sort_by_key(|(name, _)| name.to_string());
    let mut result = Vec::new();
    for (name, members) in edges {
        if declared.as_ref().is_some_and(|names| !names.contains(&name.to_string())) {
            continue;
        }
        let members = members
            .read()
            .map_err(|error| HolonError::FailedToAcquireLock(error.to_string()))?
            .get_members()
            .to_vec();
        result.extend(members.into_iter().map(|target| (name.clone(), target)));
    }
    Ok(result)
}

fn run(
    invocation: ValidationInvocation<'_>,
    collector: &mut ValidationCollector,
    rule: CoreValidationRuleName,
    code: &str,
) -> Result<RuleOutcome, HolonError> {
    let ValidationInvocation::Schema { binding, path, products } = invocation else {
        return Err(HolonError::InvalidParameter(
            "Schema rule requires prepared aggregate products".into(),
        ));
    };
    for message in products.diagnostics.get(&rule).into_iter().flatten() {
        rule_violation(collector, binding, path, code, message.clone())?;
    }
    Ok(RuleOutcome::Continue)
}
pub(crate) fn schema_dependencies_acyclic(
    i: ValidationInvocation<'_>,
    c: &mut ValidationCollector,
) -> Result<RuleOutcome, HolonError> {
    run(i, c, CoreValidationRuleName::SchemaDependenciesAcyclic, "DS-SCHEMA-001")
}
pub(crate) fn cross_schema_dependencies_declared(
    i: ValidationInvocation<'_>,
    c: &mut ValidationCollector,
) -> Result<RuleOutcome, HolonError> {
    run(i, c, CoreValidationRuleName::CrossSchemaDependenciesDeclared, "DS-SCHEMA-002")
}
