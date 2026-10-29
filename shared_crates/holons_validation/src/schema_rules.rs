//! Schema rule products over the bounded Commit workset.
use crate::{
    handlers::rule_violation, schema_view::ProspectiveSchema, RuleOutcome, ValidationCollector,
    ValidationInvocation,
};
use core_types::{HolonError, RelationshipName};
use holons_core::{
    core_shared_objects::transactions::TransactionContext,
    descriptors::{resolve_describing_type, schema_dependencies},
    ContractContributions, DefinitionIdentity, DescribingTypeResolution, HolonReference,
    ReadableHolon,
};
use std::{
    collections::{BTreeMap, HashMap, HashSet},
    sync::Arc,
};
use type_names::CoreValidationRuleName;

/// Prepared once per affected Schema; bound handlers never repeat graph traversal.
#[derive(Default)]
pub struct SchemaRuleProducts {
    diagnostics: BTreeMap<CoreValidationRuleName, Vec<String>>,
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
    schemas: &[ProspectiveSchema],
) -> Result<HashMap<DefinitionIdentity, Option<HolonReference>>, HolonError> {
    // Cycle memoization is per exact Schema definition.
    let mut results = HashMap::new();
    let mut active = HashSet::new();
    for view in schemas {
        let mut stack = vec![(view.schema.clone(), false, Vec::<HolonReference>::new())];
        while let Some((schema, leaving, dependencies)) = stack.pop() {
            let id = DefinitionIdentity::for_reference(&schema, context)?;
            if leaving {
                active.remove(&id);
                let mut witness = results.get(&id).cloned().flatten();
                for dependency in dependencies {
                    let dependency_id = DefinitionIdentity::for_reference(&dependency, context)?;
                    witness = witness.or_else(|| results.get(&dependency_id).cloned().flatten());
                }
                results.insert(id, witness);
            } else if active.contains(&id) {
                results.insert(id, Some(schema));
            } else if !results.contains_key(&id) {
                let dependencies = schema_dependencies(&schema)?;
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

/// Saved maps include materialized inverse navigation. Their authored surface is the
/// declared contract, as in saved-to-transient cloning. Mutable maps are authored state
/// and retain undeclared names so they cannot evade the fixed Commit check.
pub(crate) fn authored_targets(
    source: &HolonReference,
) -> Result<Vec<(RelationshipName, HolonReference)>, HolonError> {
    if let HolonReference::Smart(saved) = source {
        let DescribingTypeResolution::Unique(descriptor) = resolve_describing_type(source)? else {
            return Err(
                HolonError::MissingDescribedBy { holon: source.reference_id_string() }.into()
            );
        };
        let contract = ContractContributions::resolve(&descriptor)?;
        let names = crate::descriptor_package::declared_names(&contract)?;
        saved.prepare_relationships(&names)?;
        let mut result = Vec::new();
        for name in names {
            let members = source.related_holons(&name)?;
            let members =
                members.read().map_err(|e| HolonError::FailedToAcquireLock(e.to_string()))?;
            result
                .extend(members.get_members().iter().cloned().map(|target| (name.clone(), target)));
        }
        return Ok(result);
    }
    let mut edges = source.all_related_holons()?.iter();
    edges.sort_by_key(|(name, _)| name.to_string());
    let mut result = Vec::new();
    for (name, members) in edges {
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
