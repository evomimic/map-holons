// shared_crates/holons_loader/src/loader_ref_resolver.rs
//
// Pass-2 (Resolver): Transform queued LoaderRelationshipReference holons into
// concrete writes on staged holons. Implements the multi‑pass, graph‑driven
// inverse handling policy:
//   Pass-2a: write DescribedBy first
//   Pass-2b: write Extends next so descriptor ancestry is available
//   Pass-2c: write InverseOf next so inverse RTDs can map to declared RTDs
//   Pass-2d: resolve remaining relationships via fixed-point iteration
//
// Design goals:
// - Self‑contained, self‑describing code with explicit invariants
// - No global/in‑memory inverse name index; resolution is graph‑proven
// - Non‑fatal errors are accumulated; the controller decides commit policy
// - Deduplicate within the resolver run: (source, declared_name, target)
// - Never invent inline holons here (no new instance staging):
//   only write to already staged holons or stage new versions of saved ones
//
// Safety guardrails:
// - DescribedBy must target exactly one descriptor
// - Bootstrap relationships are selected by name before the type graph is queryable
// - If a declared name for an inverse cannot be proven via type graph → error

use std::collections::HashSet;
use std::rc::Rc;
use std::sync::Arc;
use tracing::debug;

use core_types::type_kinds::TypeKind;
use holons_prelude::prelude::*;

use crate::errors::ErrorWithContext;

/// Outcome of Pass-2: counts successful writes and collects non-fatal errors.
#[derive(Debug, Default)]
pub struct ResolverOutcome {
    /// Total number of links scheduled on staged holons
    pub links_created: i64,
    /// Non-fatal errors encountered during resolution
    pub errors: Vec<ErrorWithContext>,
}

/// Stable identity for per-run relationship deduplication.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct RelationshipEdgeKey {
    /// Best-effort stable identifier for the write source (id > vkey > key > fallback)
    source_identifier: String,
    /// Declared (forward) relationship name
    relationship_name: RelationshipName,
    /// Best-effort stable identifier for the write target
    target_identifier: String,
}

/// Batched declared-orientation write prepared after all per-target classification succeeds.
struct DeclaredRelationshipWrite {
    staged_source: StagedReference,
    targets: Vec<HolonReference>,
    edge_keys: Vec<RelationshipEdgeKey>,
}

/// Single inverse-orientation write prepared after all per-target classification succeeds.
struct InverseRelationshipWrite {
    staged_source: StagedReference,
    declared_name: RelationshipName,
    target: HolonReference,
    edge_key: RelationshipEdgeKey,
}

/// Per-run resolver state. Holds data we want to compute once and reuse.
/// Start small (just the saved index), but this scales well if we add
/// metrics, feature flags, or lazy fetches later.
///
/// Note: saved local-key fallback is currently implemented by lazily caching
/// `get_all_holons()` on the first staged miss because targeted saved lookup
/// by key does not exist yet. Replace this cache once saved key lookup is available.
pub struct ResolverState {
    /// Interim snapshot of *saved* holons for key-based fallback lookups.
    /// We fetch it at most once per resolver run.
    saved_index: Option<Rc<HolonCollection>>,
}

impl ResolverState {
    /// Create a fresh state with no pre-fetched saved index.
    /// Use `ensure_saved_index(...)` to populate it on demand.
    pub fn new() -> Self {
        Self { saved_index: None }
    }

    /// Ensure we have a saved holon index available.
    /// If already present, this is a no-op. Otherwise, it fetches all saved holons
    /// once via the TransactionContext and stores the collection for this resolver run.
    ///
    /// This is an interim implementation until the lookup layer supports targeted
    /// saved lookup by key.
    pub fn ensure_saved_index(
        &mut self,
        context: &Arc<TransactionContext>,
    ) -> Result<(), HolonError> {
        if self.saved_index.is_some() {
            return Ok(());
        }
        let collection = context.lookup().get_all_holons()?;
        self.saved_index = Some(Rc::new(collection));
        Ok(())
    }

    /// Get a reference to the saved index, if present.
    pub fn saved_index(&self) -> Option<&Rc<HolonCollection>> {
        self.saved_index.as_ref()
    }
}

/// Public resolver entry point.
pub struct LoaderRefResolver;

impl LoaderRefResolver {
    /// Resolve all queued LoaderRelationshipReference holons into concrete writes on staged holons.
    ///
    /// Multi-pass orchestration (deterministic):
    ///   1) Pass-2a: DescribedBy → with_descriptor()
    ///   2) Pass-2b: Extends → add_related_holons()
    ///   3) Pass-2c: InverseOf → add_related_holons()
    ///   4) Pass-2d: process remaining relationship references
    pub fn resolve_relationships(
        context: &Arc<TransactionContext>,
        queued_relationship_references: Vec<TransientReference>,
    ) -> Result<ResolverOutcome, HolonError> {
        let mut outcome = ResolverOutcome::default();
        let mut seen_relationship_edge_keys: HashSet<RelationshipEdgeKey> = HashSet::new();
        let mut resolver_state = ResolverState::new();

        // ── Pass-2a: ensure all descriptors are set (enables type graph walks later)
        Self::pass_2a_write_described_by_by_name(
            context,
            &mut resolver_state,
            &queued_relationship_references,
            &mut seen_relationship_edge_keys,
            &mut outcome,
        );

        // ── Pass-2b: write Extends edges so descriptor ancestry is queryable
        Self::pass_2b_write_extends_by_name(
            context,
            &mut resolver_state,
            &queued_relationship_references,
            &mut seen_relationship_edge_keys,
            &mut outcome,
        );

        // ── Pass-2c: write InverseOf edges so inverse RTDs can point to declared RTDs
        Self::pass_2c_write_inverse_of_by_name(
            context,
            &mut resolver_state,
            &queued_relationship_references,
            &mut seen_relationship_edge_keys,
            &mut outcome,
        );

        // ── Unified worklist for pass-2d: everything not handled by bootstrap passes.
        let deferred_queue: Vec<TransientReference> = queued_relationship_references
            .into_iter()
            .filter(|lrr| {
                !Self::is_described_by_by_name(lrr)
                    && !Self::is_extends_by_name(lrr)
                    && !Self::is_inverse_of_by_name(lrr)
            })
            .collect();

        let (created, errors, remaining) = Self::process_remaining_references(
            context,
            &mut resolver_state,
            deferred_queue,
            &mut seen_relationship_edge_keys,
        );

        outcome.links_created += created;
        outcome.errors.extend(errors);

        // Any remaining items could not resolve within the retry budget → explicit errors
        for relationship_reference in remaining {
            let msg = format!(
                "LoaderRelationshipReference could not be resolved after fixed-point retries: {}",
                Self::brief_lrr_summary(&relationship_reference)
            );
            outcome.errors.push(Self::error_with_context(
                &relationship_reference,
                HolonError::InvalidType(msg),
            ));
        }

        debug!(
            "Pass-2 complete: links_created={}, errors={}",
            outcome.links_created,
            outcome.errors.len()
        );

        Ok(outcome)
    }

    // ─────────────────────────────────────────────────────────────────────
    // Pass partitioning / predicates
    // ─────────────────────────────────────────────────────────────────────

    /// Returns true if the LRR’s relationship name equals `relationship_name`.
    fn has_relationship_name(
        relationship_reference: &TransientReference,
        relationship_name: &RelationshipName,
    ) -> bool {
        let relationship_name_property: PropertyName =
            CorePropertyTypeName::RelationshipName.as_property_name();
        match relationship_reference.property_value(&relationship_name_property) {
            Ok(Some(BaseValue::StringValue(MapString(s)))) => {
                &s.to_relationship_name() == relationship_name
            }
            _ => false,
        }
    }

    /// Returns true if the LRR's relationship name is DescribedBy.
    fn is_described_by_by_name(relationship_reference: &TransientReference) -> bool {
        let described_by = CoreRelationshipTypeName::DescribedBy.as_relationship_name();
        Self::has_relationship_name(relationship_reference, &described_by)
    }

    /// Returns true if the LRR's relationship name is Extends.
    fn is_extends_by_name(relationship_reference: &TransientReference) -> bool {
        let extends = CoreRelationshipTypeName::Extends.as_relationship_name();
        Self::has_relationship_name(relationship_reference, &extends)
    }

    /// Returns true if the LRR's relationship name is InverseOf.
    fn is_inverse_of_by_name(relationship_reference: &TransientReference) -> bool {
        let inverse_of = CoreRelationshipTypeName::InverseOf.as_relationship_name();
        Self::has_relationship_name(relationship_reference, &inverse_of)
    }

    // ─────────────────────────────────────────────────────────────────────
    // Pass-2a: DescribedBy bootstrap
    // ─────────────────────────────────────────────────────────────────────

    /// Writes all DescribedBy edges by name; enforces exactly one target.
    fn pass_2a_write_described_by_by_name(
        context: &Arc<TransactionContext>,
        resolver_state: &mut ResolverState,
        queue: &[TransientReference],
        seen: &mut HashSet<RelationshipEdgeKey>,
        outcome: &mut ResolverOutcome,
    ) {
        let described_by = CoreRelationshipTypeName::DescribedBy.as_relationship_name();

        let described_by_refs: Vec<_> =
            queue.iter().filter(|reference| Self::is_described_by_by_name(reference)).collect();
        debug!("Pass 2A: Processing {} DescribedBy relationships", described_by_refs.len());

        for relationship_reference in described_by_refs {
            debug!(
                "[resolver] BEFORE resolve_endpoints: {}, source_loader_key={:?}",
                Self::brief_lrr_summary(relationship_reference),
                Self::source_loader_key_of_lrr(relationship_reference).map(|k| k.0),
            );
            match Self::resolve_endpoints(context, resolver_state, relationship_reference) {
                Ok((source_endpoint, mut target_endpoints)) => {
                    // Enforce exactly one target for DescribedBy
                    if target_endpoints.len() != 1 {
                        outcome.errors.push(Self::error_with_context(
                            relationship_reference,
                            HolonError::InvalidRelationship(
                                described_by.to_string(),
                                "DescribedBy relationship must have exactly one target".into(),
                            ),
                        ));
                        continue;
                    }

                    // Resolve staged write source (the LRR source in declared orientation)
                    let staged_source =
                        match Self::resolve_staged_write_source(context, &source_endpoint) {
                            Ok(s) => s,
                            Err(e) => {
                                outcome
                                    .errors
                                    .push(Self::error_with_context(relationship_reference, e));
                                continue;
                            }
                        };

                    // Dedupe key: (source, DescribedBy, descriptor)
                    let edge_key = Self::make_edge_key(
                        &HolonReference::Staged(staged_source.clone()),
                        &described_by,
                        &target_endpoints[0],
                    );
                    if !seen.insert(edge_key) {
                        debug!("Duplicate DescribedBy skipped (bootstrap)");
                        continue;
                    }

                    // Perform the write using with_descriptor()
                    match Self::write_relationship(
                        staged_source,
                        &described_by,
                        target_endpoints.split_off(0), // exactly one
                    ) {
                        Ok(n) => {
                            outcome.links_created += n;
                            debug!(
                                "[resolver] AFTER write_relationship(DescribedBy): links_created={}",
                                n
                            );
                        }
                        Err(e) => {
                            outcome.errors.push(Self::error_with_context(relationship_reference, e))
                        }
                    }
                }
                Err(e) => outcome.errors.push(Self::error_with_context(relationship_reference, e)),
            }
        }
    }

    // ─────────────────────────────────────────────────────────────────────
    // Pass-2b/2c: type-graph bootstrap relationships
    // ─────────────────────────────────────────────────────────────────────

    /// Writes Extends edges by name so descriptor ancestry is available to later passes.
    fn pass_2b_write_extends_by_name(
        context: &Arc<TransactionContext>,
        resolver_state: &mut ResolverState,
        queue: &[TransientReference],
        seen: &mut HashSet<RelationshipEdgeKey>,
        outcome: &mut ResolverOutcome,
    ) {
        let extends = CoreRelationshipTypeName::Extends.as_relationship_name();
        Self::write_bootstrap_relationships_by_name(
            context,
            resolver_state,
            queue,
            seen,
            outcome,
            &extends,
            Self::is_extends_by_name,
            "Pass 2B",
        );
    }

    /// Writes InverseOf edges by name so inverse RTDs can point to declared RTDs.
    fn pass_2c_write_inverse_of_by_name(
        context: &Arc<TransactionContext>,
        resolver_state: &mut ResolverState,
        queue: &[TransientReference],
        seen: &mut HashSet<RelationshipEdgeKey>,
        outcome: &mut ResolverOutcome,
    ) {
        let inverse_of = CoreRelationshipTypeName::InverseOf.as_relationship_name();
        Self::write_bootstrap_relationships_by_name(
            context,
            resolver_state,
            queue,
            seen,
            outcome,
            &inverse_of,
            Self::is_inverse_of_by_name,
            "Pass 2C",
        );
    }

    /// Writes bootstrap relationships that are required before schema-aware classification.
    fn write_bootstrap_relationships_by_name(
        context: &Arc<TransactionContext>,
        resolver_state: &mut ResolverState,
        queue: &[TransientReference],
        seen: &mut HashSet<RelationshipEdgeKey>,
        outcome: &mut ResolverOutcome,
        relationship_name: &RelationshipName,
        predicate: fn(&TransientReference) -> bool,
        pass_label: &str,
    ) {
        let bootstrap_refs: Vec<_> =
            queue.iter().filter(|reference| predicate(reference)).collect();
        debug!(
            "{}: Processing {} {} relationships",
            pass_label,
            bootstrap_refs.len(),
            relationship_name.0
        );

        for relationship_reference in bootstrap_refs {
            debug!(
                "[resolver] BEFORE resolve_endpoints: {}, source_loader_key={:?}",
                Self::brief_lrr_summary(relationship_reference),
                Self::source_loader_key_of_lrr(relationship_reference).map(|k| k.0),
            );
            match Self::resolve_endpoints(context, resolver_state, relationship_reference) {
                Ok((source_endpoint, target_endpoints)) => {
                    if let Err(error) = Self::validate_bootstrap_relationship_targets(
                        relationship_name,
                        target_endpoints.len(),
                    ) {
                        outcome
                            .errors
                            .push(Self::error_with_context(relationship_reference, error));
                        continue;
                    }

                    let staged_source =
                        match Self::resolve_staged_write_source(context, &source_endpoint) {
                            Ok(s) => s,
                            Err(e) => {
                                outcome
                                    .errors
                                    .push(Self::error_with_context(relationship_reference, e));
                                continue;
                            }
                        };

                    // Deduplicate per (source, relationship name, each target)
                    let mut unique_targets: Vec<HolonReference> =
                        Vec::with_capacity(target_endpoints.len());
                    let source_ref = HolonReference::Staged(staged_source.clone());
                    for target in target_endpoints.into_iter() {
                        let edge_key = Self::make_edge_key(&source_ref, relationship_name, &target);
                        if seen.insert(edge_key) {
                            unique_targets.push(target);
                        } else {
                            debug!("Duplicate {} skipped (bootstrap)", relationship_name.0);
                        }
                    }

                    match Self::write_relationship(staged_source, relationship_name, unique_targets)
                    {
                        Ok(n) => outcome.links_created += n,
                        Err(e) => {
                            outcome.errors.push(Self::error_with_context(relationship_reference, e))
                        }
                    }
                }
                Err(e) => outcome.errors.push(Self::error_with_context(relationship_reference, e)),
            }
        }
    }

    fn validate_bootstrap_relationship_targets(
        relationship_name: &RelationshipName,
        target_count: usize,
    ) -> Result<(), HolonError> {
        let extends = CoreRelationshipTypeName::Extends.as_relationship_name();
        let inverse_of = CoreRelationshipTypeName::InverseOf.as_relationship_name();

        if (*relationship_name == extends || *relationship_name == inverse_of) && target_count != 1
        {
            return Err(HolonError::InvalidRelationship(
                relationship_name.to_string(),
                format!(
                    "{} relationship must have exactly one target; found {}",
                    relationship_name, target_count
                ),
            ));
        }

        Ok(())
    }

    // ─────────────────────────────────────────────────────────────────────
    // Pass-2d: Process remaining relationship references
    // ─────────────────────────────────────────────────────────────────────

    /// After bootstrap passes, process all remaining references together.
    /// Removes successes & fatals; retains only deferrables; stops at fixed point.
    fn process_remaining_references(
        context: &Arc<TransactionContext>,
        resolver_state: &mut ResolverState,
        mut remaining_queue: Vec<TransientReference>,
        seen: &mut HashSet<RelationshipEdgeKey>,
    ) -> (i64, Vec<ErrorWithContext>, Vec<TransientReference>) {
        let mut errors: Vec<ErrorWithContext> = Vec::new();
        let mut total_links_created = 0i64;
        debug!("Processing REMAINING_REFERENCES");
        // Conservative upper bound; usually we break by fixed-point first.
        // At least 2 passes to allow for progress.
        let mut passes_remaining = (remaining_queue.len() + 1).max(2);

        while passes_remaining > 0 {
            let mut links_created_this_pass = 0i64;
            let mut pass_fatal_errors: Vec<ErrorWithContext> = Vec::new();

            // Filter in place using retain() and true/false return values.
            remaining_queue.retain(|relationship_reference| {
                // Skip anything already handled by bootstrap passes (defensive; unified_queue already filtered)
                if Self::is_described_by_by_name(relationship_reference)
                    || Self::is_extends_by_name(relationship_reference)
                    || Self::is_inverse_of_by_name(relationship_reference)
                {
                    return false;
                }

                // Pass-2d runs after DescribedBy and Extends are available for type-graph classification.
                let resolution_result = Self::try_resolve_by_type_graph(
                    context,
                    resolver_state,
                    relationship_reference,
                    seen,
                );

                match resolution_result {
                    Ok(n) => {
                        links_created_this_pass += n;
                        // success or dedup (n may be 0) → drop from queue
                        false
                    }
                    Err(e) if Self::is_deferrable_error(&e) => {
                        // keep for next pass
                        true
                    }
                    Err(e) => {
                        // fatal → record and drop
                        pass_fatal_errors.push(Self::error_with_context(relationship_reference, e));
                        false
                    }
                }
            });

            total_links_created += links_created_this_pass;
            errors.extend(pass_fatal_errors);
            passes_remaining -= 1;

            if links_created_this_pass == 0 {
                break; // fixed point reached
            }
        }

        (total_links_created, errors, remaining_queue)
    }

    /// Follow InverseOf from an inverse RTD to its declared RTD and return the declared name.
    fn declared_name_from_inverse_type_descriptor(
        inverse_name: &RelationshipName,
        relationship_type_descriptor: &HolonReference,
    ) -> Result<RelationshipName, HolonError> {
        let inverse_of = CoreRelationshipTypeName::InverseOf.as_relationship_name();
        let declared_handle = relationship_type_descriptor.related_holons(&inverse_of)?;
        debug!("[resolver] found declared TypeDescriptor");

        let related_members: Vec<HolonReference> = {
            let guard = declared_handle.read().map_err(|_| {
                HolonError::FailedToBorrow("InverseOf collection read lock poisoned".into())
            })?;
            guard.get_members().clone()
        };
        if related_members.is_empty() {
            return Err(HolonError::InvalidRelationship(
                "InverseOf".into(),
                format!("No InverseOf target from relationship '{}'", inverse_name.0),
            ));
        }

        debug!("[resolver] declared type descriptor type-gated filtering");
        let mut valid_targets = Vec::new();
        for candidate in related_members {
            if Self::is_relationship_type_kind(&candidate) {
                valid_targets.push(candidate);
            }
        }

        match valid_targets.len() {
            1 => {
                // We return the **declared relationship name** (TypeName is correct here).
                let type_name_prop: PropertyName =
                    CorePropertyTypeName::TypeName.as_property_name();
                let declared_type_name =
                    Self::read_string_property(&valid_targets[0], &type_name_prop)?;
                Ok(declared_type_name.to_relationship_name())
            }
            0 => Err(HolonError::InvalidType(format!(
                "InverseOf targets for relationship '{}' did not include a RelationshipTypeDescriptor",
                inverse_name.0
            ))),
            n => Err(HolonError::DuplicateError(
                "inverse mapping".into(),
                format!(
                    "Multiple RelationshipTypeDescriptor targets ({}) via InverseOf for relationship '{}'",
                    n, inverse_name.0
                ),
            )),
        }
    }

    // ─────────────────────────────────────────────────────────────────────
    // Endpoint + type-graph helpers
    // ─────────────────────────────────────────────────────────────────────

    /// Extracts the relationship name from an LRR.
    fn extract_relationship_metadata(
        relationship_reference: &TransientReference,
    ) -> Result<RelationshipName, HolonError> {
        let relationship_name_property: PropertyName =
            CorePropertyTypeName::RelationshipName.as_property_name();

        let relationship_value =
            relationship_reference.property_value(&relationship_name_property)?.ok_or_else(
                || HolonError::EmptyField("LoaderRelationshipReference.RelationshipName".into()),
            )?;

        let relationship_name = match relationship_value {
            BaseValue::StringValue(MapString(text)) => text.to_relationship_name(),
            other => {
                return Err(HolonError::UnexpectedValueType(
                    format!("{:?}", other),
                    "String".into(),
                ));
            }
        };

        Ok(relationship_name)
    }

    /// Resolve LoaderHolonReference endpoints to actual holon references.
    /// Ensures exactly one `ReferenceSource` and ≥1 `ReferenceTarget`;
    /// Returns (source_holon, target_holons) where each has been dereferenced
    /// from its LoaderHolonReference wrapper.
    fn resolve_endpoints(
        context: &Arc<TransactionContext>,
        resolver_state: &mut ResolverState,
        relationship_reference: &TransientReference,
    ) -> Result<(HolonReference, Vec<HolonReference>), HolonError> {
        let source_relationship = CoreRelationshipTypeName::ReferenceSource;
        let target_relationship = CoreRelationshipTypeName::ReferenceTarget;

        // Get LoaderHolonReference wrappers (not the actual holons yet)
        let source_refs_handle = relationship_reference.related_holons(source_relationship)?;
        let target_refs_handle = relationship_reference.related_holons(target_relationship)?;

        // NOTE: Safe to hold these read locks in resolver paths; parser-produced bundles are immutable during Pass-2.
        let source_guard = source_refs_handle.read().map_err(|_| {
            HolonError::FailedToBorrow("Source collection read lock poisoned".into())
        })?;
        let source_loader_refs = source_guard.get_members(); // &Vec<HolonReference>

        let target_guard = target_refs_handle.read().map_err(|_| {
            HolonError::FailedToBorrow("Target collection read lock poisoned".into())
        })?;
        let target_loader_refs = target_guard.get_members(); // &Vec<HolonReference>

        debug!(
            "[resolver] LRR endpoints: sources={}, targets={}",
            source_loader_refs.len(),
            target_loader_refs.len()
        );

        // Validate cardinality
        // Exactly one ReferenceSource
        match source_loader_refs.len() {
            1 => {}
            0 => {
                return Err(HolonError::EmptyField(
                    "LoaderRelationshipReference.ReferenceSource".into(),
                ));
            }
            n => {
                return Err(HolonError::DuplicateError(
                    "ReferenceSource".into(),
                    format!("{n} found"),
                ));
            }
        }

        // At least one ReferenceTarget
        if target_loader_refs.is_empty() {
            return Err(HolonError::EmptyField(
                "LoaderRelationshipReference.ReferenceTarget".into(),
            ));
        }

        // Dereference: LoaderHolonReference → actual HolonReference
        let source_holon =
            Self::resolve_loader_holon_reference(context, resolver_state, &source_loader_refs[0])?;
        debug!(
            "[resolver]   resolved source holon = {}",
            Self::best_identifier_for_dedupe(&source_holon)
        );

        let mut target_holons = Vec::with_capacity(target_loader_refs.len());
        for loader_ref in target_loader_refs.iter() {
            let resolved =
                Self::resolve_loader_holon_reference(context, resolver_state, loader_ref)?;
            debug!(
                "[resolver]   resolved target holon = {}",
                Self::best_identifier_for_dedupe(&resolved)
            );
            target_holons.push(resolved);
        }

        Ok((source_holon, target_holons))
    }

    /// Dereference a LoaderHolonReference to the actual holon it points to.
    ///
    /// Resolution order (per spec):
    /// 1. `holon_key` → prefer staged holon via Nursery, then fall back to saved by key
    /// 2. (Future) `holon_id` → saved holon by ID
    /// 3. (Future) `proxy_key`/`proxy_id` → external holon via proxy
    ///
    /// Note: the saved fallback currently uses a lazily populated per-run snapshot
    /// of all saved holons because targeted saved lookup by key is not available yet.
    fn resolve_loader_holon_reference(
        context: &Arc<TransactionContext>,
        resolver_state: &mut ResolverState,
        loader_ref: &HolonReference,
    ) -> Result<HolonReference, HolonError> {
        // Property names from LoaderHolonReference schema
        let holon_key_property = CorePropertyTypeName::HolonKey.as_property_name();
        // let holon_id_property = CorePropertyTypeName::HolonId.as_property_name(); // to be used with holon id lookup below

        // Try holon_key first:
        //   1) prefer staged holons in the current import
        //   2) fall back to already-saved local holons by key
        if let Some(BaseValue::StringValue(key)) = loader_ref.property_value(&holon_key_property)? {
            debug!("[resolver] dereference LHR by holon_key='{}'", key.0);
            // Use the convenience API for the single expected staged match.
            match context.lookup().get_staged_holon_by_base_key(&key) {
                Ok(staged) => {
                    debug!("[resolver]   → FOUND staged holon for key='{}'", key.0);
                    return Ok(HolonReference::Staged(staged));
                }
                Err(HolonError::HolonNotFound(_)) => {
                    debug!(
                        "[resolver]   → NO staged holon for key='{}'; trying saved fallback",
                        key.0
                    );
                }
                Err(e) => {
                    // Propagate duplicate/borrow/etc. from staged lookup.
                    debug!("[resolver]   → lookup for key='{}' failed with: {:?}", key.0, e);
                    return Err(e);
                }
            }

            // Interim saved fallback: fetch all saved holons once on the first staged miss,
            // then reuse that snapshot until targeted saved lookup by key exists.
            if resolver_state.saved_index().is_none() {
                debug!(
                    "Staged miss for holon key '{}'; fetching saved holons via get_all_holons()",
                    key.0
                );
                resolver_state.ensure_saved_index(context)?;
            }

            if let Some(saved_collection) = resolver_state.saved_index() {
                match saved_collection.get_by_key(&key) {
                    Ok(Some(saved_reference)) => {
                        debug!("[resolver]   → FOUND saved holon for key='{}'", key.0);
                        return Ok(saved_reference);
                    }
                    Ok(None) => {
                        debug!("[resolver]   → NO saved holon for key='{}'", key.0);
                    }
                    Err(error) => return Err(error),
                }
            }

            // Key was present, but neither staged nor saved lookup found a match yet → deferrable.
            return Err(HolonError::HolonNotFound(format!(
                "staged or saved holon with key '{}'",
                key.0
            )));
        }

        // TODO: un-comment when saved holon fetch by ID is implemented (we need a MapBytes BaseValue variant)
        // Try holon_id (saved)
        // if let Some(BaseValue::BytesValue(id_bytes)) =
        //     loader_ref.property_value(&holon_id_property)?
        // {
        //     // Convert MapBytes to HolonId
        //     let holon_id = HolonId::try_from(id_bytes.0.as_slice()).map_err(|e| {
        //         HolonError::InvalidParameter(format!("Invalid holon_id bytes: {}", e))
        //     })?;
        //
        //     // Return a SmartReference (saved holon)
        //     return Ok(HolonReference::Smart(SmartReference::new_from_id(holon_id)));
        // }

        // TODO: proxy_key / proxy_id resolution for external references

        debug!("[resolver] dereference LHR: no HolonKey property present");
        Err(HolonError::EmptyField(
            "LoaderHolonReference has no holon_key(holon_id not yet supported); cannot dereference"
                .into(),
        ))
    }

    /// Finds the first relationship type descriptor matching an endpoint pair's effective types.
    ///
    /// Endpoint descriptors are searched across their `Extends` ancestors in
    /// source-major, target-minor order, so matches closer to the concrete
    /// source descriptor win before widening the source type.
    fn find_relationship_type_for_endpoints(
        context: &Arc<TransactionContext>,
        resolver_state: &mut ResolverState,
        relationship_name: &RelationshipName,
        source_endpoint: &HolonReference,
        target_endpoint: &HolonReference,
    ) -> Result<Option<(HolonReference, RelationshipDirection)>, HolonError> {
        let source_ancestors = effective_descriptor_lineage(source_endpoint)?;
        let target_ancestors = effective_descriptor_lineage(target_endpoint)?;
        let key_property_name = CorePropertyTypeName::Key.as_property_name();
        let target_descriptor_keys = Self::keyed_descriptor_ancestors(
            &target_ancestors,
            &key_property_name,
            "target",
            relationship_name,
        )?;

        // Search endpoint type pairs from most-specific source outward.
        for source_ancestor in source_ancestors.iter() {
            let Some(source_descriptor_key) = Self::optional_descriptor_key(
                source_ancestor,
                &key_property_name,
                "source",
                relationship_name,
            )?
            else {
                continue;
            };

            for target_descriptor_key in target_descriptor_keys.iter() {
                let canonical_key = MapString(format!(
                    "({})-[{}]->({})",
                    source_descriptor_key.0, relationship_name.0, target_descriptor_key.0
                ));

                if let Some(relationship_type_descriptor) =
                    Self::find_relationship_type_by_key(context, resolver_state, &canonical_key)?
                {
                    let direction = classify_relationship_direction(&relationship_type_descriptor)?;
                    debug!(
                        "[resolver] found RelationshipType key '{}' with direction {:?}",
                        canonical_key.0, direction
                    );
                    return Ok(Some((relationship_type_descriptor, direction)));
                }
            }
        }

        Ok(None)
    }

    fn keyed_descriptor_ancestors(
        ancestors: &[HolonReference],
        key_property_name: &PropertyName,
        endpoint_role: &str,
        relationship_name: &RelationshipName,
    ) -> Result<Vec<MapString>, HolonError> {
        let mut keyed_ancestors = Vec::new();

        for ancestor in ancestors.iter() {
            if let Some(key) = Self::optional_descriptor_key(
                ancestor,
                key_property_name,
                endpoint_role,
                relationship_name,
            )? {
                keyed_ancestors.push(key);
            }
        }

        Ok(keyed_ancestors)
    }

    fn optional_descriptor_key(
        descriptor: &HolonReference,
        key_property_name: &PropertyName,
        endpoint_role: &str,
        relationship_name: &RelationshipName,
    ) -> Result<Option<MapString>, HolonError> {
        match Self::read_string_property(descriptor, key_property_name) {
            Ok(key) => Ok(Some(key)),
            Err(HolonError::EmptyField(_)) | Err(HolonError::UnexpectedValueType(_, _)) => {
                debug!(
                    "[resolver] skipping {} descriptor ancestor without usable Key while resolving relationship '{}'",
                    endpoint_role, relationship_name.0
                );
                Ok(None)
            }
            Err(error) => Err(error),
        }
    }

    /// Look up a relationship type descriptor by its canonical key
    /// `"(SourceType.HolonType)-[RelationshipName]->(TargetType.HolonType)"`.
    /// Preference order:
    ///   1) Staged (Nursery) lookup by base key
    ///   2) Saved fallback via a pre-fetched HolonCollection and `get_by_key`
    /// Returns `Ok(None)` if not found in either place.
    fn find_relationship_type_by_key(
        context: &Arc<TransactionContext>,
        resolver_state: &mut ResolverState,
        canonical_key: &MapString,
    ) -> Result<Option<HolonReference>, HolonError> {
        debug!("[resolver] looking up RelationshipType by key '{}'", canonical_key.0);

        // 1) Prefer staged (Nursery) lookup by base key through the holon operations API.
        match context.lookup().get_staged_holons_by_base_key(canonical_key) {
            Ok(staged_candidates) => match staged_candidates.len() {
                1 => {
                    let staged = staged_candidates.into_iter().next().unwrap();
                    debug!(
                        "[resolver]   → FOUND staged RelationshipType for key '{}'",
                        canonical_key.0
                    );
                    return Ok(Some(HolonReference::Staged(staged)));
                }
                n if n > 1 => {
                    return Err(HolonError::DuplicateError(
                        "relationship type by key".into(),
                        n.to_string(),
                    ));
                }
                _ => { /* fall through to saved fallback */ }
            },
            Err(HolonError::HolonNotFound(_)) => {
                debug!(
                    "[resolver]   → NO staged RelationshipType for key '{}'; trying saved fallback",
                    canonical_key.0
                );
            }
            Err(error) => return Err(error),
        }

        // 2) Saved fallback: lazily fetch the saved index on first staged miss.
        if resolver_state.saved_index().is_none() {
            debug!(
                "Staged miss for relationship type '{}'; fetching saved holons via get_all_holons()",
                canonical_key.0
            );
            resolver_state.ensure_saved_index(context)?; // one-time fetch per run
        }

        if let Some(saved_collection) = resolver_state.saved_index() {
            match saved_collection.get_by_key(canonical_key) {
                Ok(Some(saved_reference)) => return Ok(Some(saved_reference)),
                Ok(None) => { /* not present in saved */ }
                Err(error) => return Err(error),
            }
        }

        Ok(None)
    }

    /// Returns true if the holon's `InstanceTypeKind == "TypeKind.Relationship"`.
    fn is_relationship_type_kind(holon_reference: &HolonReference) -> bool {
        debug!("[resolver] entering is_relationship_type_kind");

        let property_name: PropertyName = CorePropertyTypeName::InstanceTypeKind.as_property_name();
        let expected = TypeKind::Relationship.as_schema_key();

        match holon_reference.property_value(&property_name) {
            Ok(Some(PropertyValue::StringValue(MapString(actual)))) => actual == expected,

            _ => false,
        }
    }

    // ─────────────────────────────────────────────────────────────────────
    // Writing + dedupe + worklist
    // ─────────────────────────────────────────────────────────────────────

    /// Ensures a writable staged source (promote saved → staged if policy allows).
    fn resolve_staged_write_source(
        context: &Arc<TransactionContext>,
        write_source_endpoint: &HolonReference,
    ) -> Result<StagedReference, HolonError> {
        // 1) If the endpoint already corresponds to a staged holon, use it (prefer versioned key).
        if let HolonReference::Staged(s) = write_source_endpoint {
            return Ok(s.clone());
        }
        if let Ok(versioned_key) = write_source_endpoint.versioned_key() {
            // Short read lock to check by versioned key
            if let Ok(staged_ref) =
                { context.lookup().get_staged_holon_by_versioned_key(&versioned_key) }
            {
                return Ok(staged_ref);
            }
        }

        // Try base key as a secondary staged lookup.
        if let Ok(Some(base_key)) = write_source_endpoint.key() {
            let staged_matches = context.lookup().get_staged_holons_by_base_key(&base_key)?;

            match staged_matches.len() {
                1 => {
                    // Extra defensive check to avoid panics if the vector is somehow empty.
                    let mut iter = staged_matches.into_iter();
                    return if let Some(single) = iter.next() {
                        Ok(single)
                    } else {
                        Err(HolonError::Misc(
                            "Internal error: staged_matches reported len() == 1 but contained no elements"
                                .into(),
                        ))
                    };
                }
                n if n > 1 => {
                    return Err(HolonError::DuplicateError(
                        "write source by base key".into(),
                        n.to_string(),
                    ));
                }
                _ => {
                    // not staged by base key; try promotion next
                }
            }
        }

        // 2) Promotion path: saved → stage a new version (requires HolonId).
        if let Ok(saved_id) = write_source_endpoint.holon_id() {
            let staged_reference = context.mutation().stage_new_version_from_id(saved_id)?;
            return Ok(staged_reference);
        }

        // 3) No staged match and no saved identity → not supported in Pass-2.
        Err(HolonError::InvalidParameter(
            "Write source is not staged, and no saved identity (holon_id) available to stage a new version. Inline/embedded instance creation is not supported in Pass-2.".into(),
        ))
    }

    /// Performs the actual write:
    /// - DescribedBy: exactly one target → `with_descriptor`
    /// - Others: batch → `add_related_holons`
    fn write_relationship(
        mut staged_source: StagedReference,
        declared_relationship_name: &RelationshipName,
        mut write_targets: Vec<HolonReference>,
    ) -> Result<i64, HolonError> {
        let is_descriptor = *declared_relationship_name
            == CoreRelationshipTypeName::DescribedBy.as_relationship_name();

        if is_descriptor {
            return match write_targets.len() {
                0 => Ok(0), // nothing to do (likely deduped earlier)
                1 => {
                    // Exactly one descriptor: attach it
                    staged_source.with_descriptor(write_targets.remove(0))?;
                    Ok(1)
                }
                _ => {
                    Err(HolonError::InvalidRelationship(
                        declared_relationship_name.to_string(),
                        "DescribedBy target was duplicate or ambiguous; expected exactly one unique target"
                            .into(),
                    ))
                }
            };
        }

        // Non-descriptor relationships: add the whole batch (if any)
        if write_targets.is_empty() {
            return Ok(0);
        }

        let number_of_targets = write_targets.len() as i64;
        staged_source.add_related_holons(declared_relationship_name.clone(), write_targets)?;

        Ok(number_of_targets)
    }

    /// Builds a stable dedupe key for (source, relationship, target).
    fn make_edge_key(
        source_ref: &HolonReference,
        relationship_name: &RelationshipName,
        target_ref: &HolonReference,
    ) -> RelationshipEdgeKey {
        RelationshipEdgeKey {
            source_identifier: Self::best_identifier_for_dedupe(source_ref),
            relationship_name: relationship_name.clone(),
            target_identifier: Self::best_identifier_for_dedupe(target_ref),
        }
    }

    /// Provenance prefix used only for key-like identifiers (not for HolonId).
    #[inline]
    fn provenance_prefix(reference: &HolonReference) -> &'static str {
        match reference {
            HolonReference::Staged(_) => "staged:",
            HolonReference::Smart(_) => "saved:",
            HolonReference::Transient(_) => "transient:",
        }
    }

    /// Best-effort identifier for dedupe/diagnostics:
    /// 1) Prefer HolonId (no provenance prefix) so staged/saved of the *same* holon dedupe together.
    /// 2) Fall back to versioned_key (prefixed with provenance).
    /// 3) Fall back to base key (prefixed with provenance).
    /// 4) Final fallback includes provenance as well.
    fn best_identifier_for_dedupe(reference: &HolonReference) -> String {
        // If we can resolve a HolonId, that’s the canonical identity across staged/saved.
        if let Ok(id) = reference.holon_id() {
            return format!("id:{id}");
        }

        // Otherwise we’re in key territory—prefix to avoid staged/saved collisions.
        let prefix = Self::provenance_prefix(reference);

        if let Ok(vk) = reference.versioned_key() {
            return format!("{prefix}vkey:{vk}");
        }
        if let Ok(Some(k)) = reference.key() {
            return format!("{prefix}key:{k}");
        }

        format!("{prefix}<no-id>")
    }

    /// Resolve one remaining relationship reference by classifying each endpoint pair.
    fn try_resolve_by_type_graph(
        context: &Arc<TransactionContext>,
        resolver_state: &mut ResolverState,
        relationship_reference: &TransientReference,
        seen_relationship_edge_keys: &mut HashSet<RelationshipEdgeKey>,
    ) -> Result<i64, HolonError> {
        debug!("[resolver] Entering try_resolve_by_type_graph");

        let relationship_name = Self::extract_relationship_metadata(relationship_reference)?;
        let described_by = CoreRelationshipTypeName::DescribedBy.as_relationship_name();
        let extends = CoreRelationshipTypeName::Extends.as_relationship_name();
        let inverse_of = CoreRelationshipTypeName::InverseOf.as_relationship_name();

        // Bootstrap relationships are handled before type-graph resolution.
        if relationship_name == described_by
            || relationship_name == extends
            || relationship_name == inverse_of
        {
            return Ok(0);
        }

        debug!(
            "[resolver] BEFORE resolve_endpoints: {}, source_loader_key={:?}",
            Self::brief_lrr_summary(relationship_reference),
            Self::source_loader_key_of_lrr(relationship_reference).map(|k| k.0),
        );
        let (source_endpoint, target_endpoints) =
            Self::resolve_endpoints(context, resolver_state, relationship_reference)?;

        let mut declared_target_candidates: Vec<HolonReference> = Vec::new();
        let mut inverse_write_candidates: Vec<(RelationshipName, HolonReference)> = Vec::new();

        // Classification phase: prove every target before mutating relationships.
        for target_endpoint in target_endpoints {
            // Classify this endpoint pair; heterogeneous targets may resolve differently.
            let Some((relationship_type_descriptor, relationship_direction)) =
                Self::find_relationship_type_for_endpoints(
                    context,
                    resolver_state,
                    &relationship_name,
                    &source_endpoint,
                    &target_endpoint,
                )?
            else {
                return Err(HolonError::HolonNotFound(format!(
                    "RelationshipType for relationship '{}' between endpoint descriptors",
                    relationship_name.0
                )));
            };

            match relationship_direction {
                RelationshipDirection::Declared => {
                    declared_target_candidates.push(target_endpoint);
                }
                RelationshipDirection::Inverse => {
                    // Inverse orientation: use the matched RTD instead of walking the type graph again.
                    let declared_name = Self::declared_name_from_inverse_type_descriptor(
                        &relationship_name,
                        &relationship_type_descriptor,
                    )
                    .map_err(|e| {
                        HolonError::InvalidType(format!(
                            "inverse LRR ({}): {}",
                            Self::brief_lrr_summary(relationship_reference),
                            e
                        ))
                    })?;
                    inverse_write_candidates.push((declared_name, target_endpoint));
                }
            }
        }

        let mut planned_edge_keys: HashSet<RelationshipEdgeKey> = HashSet::new();
        let declared_write = Self::plan_declared_relationship_write(
            context,
            &relationship_name,
            &source_endpoint,
            declared_target_candidates,
            seen_relationship_edge_keys,
            &mut planned_edge_keys,
        )?;
        let inverse_writes = Self::plan_inverse_relationship_writes(
            context,
            &source_endpoint,
            inverse_write_candidates,
            seen_relationship_edge_keys,
            &mut planned_edge_keys,
        )?;

        // Write phase: execute only after all endpoint pairs and write sources resolved.
        let mut created_link_count = 0i64;
        if let Some(declared_write) = declared_write {
            created_link_count += Self::write_relationship(
                declared_write.staged_source,
                &relationship_name,
                declared_write.targets,
            )?;
            for edge_key in declared_write.edge_keys {
                seen_relationship_edge_keys.insert(edge_key);
            }
        }

        for inverse_write in inverse_writes {
            debug!(
                "Attempting to write inverse→declared relationship: declared_name={}",
                inverse_write.declared_name.0
            );
            created_link_count += Self::write_relationship(
                inverse_write.staged_source.clone(),
                &inverse_write.declared_name,
                vec![inverse_write.target.clone()],
            )?;

            seen_relationship_edge_keys.insert(inverse_write.edge_key);

            debug!(
                "Created relationship (inverse→declared): source={}, rel={}, target={}",
                Self::best_identifier_for_dedupe(&HolonReference::Staged(
                    inverse_write.staged_source
                )),
                inverse_write.declared_name,
                Self::best_identifier_for_dedupe(&inverse_write.target),
            );
        }

        Ok(created_link_count)
    }

    /// Plan a batched declared write and dedupe it without mutating the global seen set.
    fn plan_declared_relationship_write(
        context: &Arc<TransactionContext>,
        relationship_name: &RelationshipName,
        source_endpoint: &HolonReference,
        target_candidates: Vec<HolonReference>,
        seen_relationship_edge_keys: &HashSet<RelationshipEdgeKey>,
        planned_edge_keys: &mut HashSet<RelationshipEdgeKey>,
    ) -> Result<Option<DeclaredRelationshipWrite>, HolonError> {
        if target_candidates.is_empty() {
            return Ok(None);
        }

        let staged_source = Self::resolve_staged_write_source(context, source_endpoint)?;
        let source_reference = HolonReference::Staged(staged_source.clone());
        let mut targets = Vec::new();
        let mut edge_keys = Vec::new();

        for target in target_candidates {
            let edge_key = Self::make_edge_key(&source_reference, relationship_name, &target);
            if seen_relationship_edge_keys.contains(&edge_key)
                || !planned_edge_keys.insert(edge_key.clone())
            {
                debug!("Duplicate relationship skipped (declared)");
                continue;
            }

            targets.push(target);
            edge_keys.push(edge_key);
        }

        if targets.is_empty() {
            return Ok(None);
        }

        Ok(Some(DeclaredRelationshipWrite { staged_source, targets, edge_keys }))
    }

    /// Plan flipped inverse writes and dedupe them without mutating the global seen set.
    fn plan_inverse_relationship_writes(
        context: &Arc<TransactionContext>,
        source_endpoint: &HolonReference,
        inverse_write_candidates: Vec<(RelationshipName, HolonReference)>,
        seen_relationship_edge_keys: &HashSet<RelationshipEdgeKey>,
        planned_edge_keys: &mut HashSet<RelationshipEdgeKey>,
    ) -> Result<Vec<InverseRelationshipWrite>, HolonError> {
        let mut planned_writes = Vec::new();

        for (declared_name, write_source_endpoint) in inverse_write_candidates {
            let staged_source = Self::resolve_staged_write_source(context, &write_source_endpoint)?;
            let declared_source_reference = HolonReference::Staged(staged_source.clone());
            let target = source_endpoint.clone();
            let edge_key = Self::make_edge_key(&declared_source_reference, &declared_name, &target);
            if seen_relationship_edge_keys.contains(&edge_key)
                || !planned_edge_keys.insert(edge_key.clone())
            {
                debug!("Duplicate relationship skipped (inverse→declared)");
                continue;
            }

            planned_writes.push(InverseRelationshipWrite {
                staged_source,
                declared_name,
                target,
                edge_key,
            });
        }

        Ok(planned_writes)
    }

    // ─────────────────────────────────────────────────────────────────────
    // Low-level helpers
    // ─────────────────────────────────────────────────────────────────────

    /// Read a required string property from a holon reference.
    fn read_string_property(
        holon: &HolonReference,
        property_name: &PropertyName,
    ) -> Result<MapString, HolonError> {
        match holon.property_value(property_name)? {
            Some(BaseValue::StringValue(s)) => Ok(s),
            Some(other) => {
                Err(HolonError::UnexpectedValueType(format!("{:?}", other), "String".into()))
            }
            None => Err(HolonError::EmptyField(property_name.to_string())),
        }
    }

    /// Short diagnostic summary for a LoaderRelationshipReference.
    fn brief_lrr_summary(lrr: &TransientReference) -> String {
        let name = Self::extract_relationship_metadata(lrr)
            .unwrap_or_else(|_| RelationshipName(MapString("<unknown>".into())));
        format!("name={}", name)
    }

    /// Deferrable errors are those that might succeed after earlier writes land.
    /// Treat missing endpoints/keys and not-found lookups as deferrable; schema violations are not.
    fn is_deferrable_error(err: &HolonError) -> bool {
        match err {
            HolonError::EmptyField(_) => true,
            HolonError::HolonNotFound(_) => true,
            HolonError::FailedToBorrow(_) => true,
            HolonError::InvalidParameter(_) => true, // e.g., write-source not staged *yet*
            _ => false,
        }
    }

    /// Extract the LoaderHolon key from the LRR's ReferenceSource (if present).
    fn source_loader_key_of_lrr(lrr: &TransientReference) -> Option<MapString> {
        let source_rel = CoreRelationshipTypeName::ReferenceSource.as_relationship_name();
        let handle = lrr.related_holons(source_rel).ok()?;
        let guard = handle.read().ok()?;
        let first = guard.get_members().get(0)?;
        // The ReferenceSource points to a LoaderHolonReference which carries HolonKey.
        let key_prop = CorePropertyTypeName::HolonKey.as_property_name();
        match first.property_value(&key_prop).ok()? {
            Some(BaseValue::StringValue(k)) if !k.0.is_empty() => Some(k),
            _ => None,
        }
    }

    /// Wrap a HolonError with contextual loader key (if available).
    fn error_with_context(lrr: &TransientReference, err: HolonError) -> ErrorWithContext {
        let key = Self::source_loader_key_of_lrr(lrr);
        ErrorWithContext { error: err, source_loader_key: key }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use core_types::LocalId;
    use holons_core::core_shared_objects::space_manager::HolonSpaceManager;
    use holons_core::core_shared_objects::{Holon, ServiceRoutingPolicy};
    use holons_core::HolonServiceApi;
    use std::any::Any;

    #[derive(Debug)]
    struct TestHolonService;

    fn unreachable_in_loader_ref_resolver_tests<T>() -> Result<T, HolonError> {
        Err(HolonError::NotImplemented("TestHolonService".to_string()))
    }

    impl HolonServiceApi for TestHolonService {
        fn as_any(&self) -> &dyn Any {
            self
        }

        fn commit_internal(
            &self,
            _context: &Arc<TransactionContext>,
            _staged_references: &[StagedReference],
        ) -> Result<TransientReference, HolonError> {
            unreachable_in_loader_ref_resolver_tests()
        }

        fn delete_holon_internal(
            &self,
            _context: &Arc<TransactionContext>,
            _local_id: &LocalId,
        ) -> Result<(), HolonError> {
            unreachable_in_loader_ref_resolver_tests()
        }

        fn fetch_all_related_holons_internal(
            &self,
            _context: &Arc<TransactionContext>,
            _source_id: &HolonId,
        ) -> Result<RelationshipMap, HolonError> {
            unreachable_in_loader_ref_resolver_tests()
        }

        fn fetch_holon_internal(
            &self,
            _context: &Arc<TransactionContext>,
            _id: &HolonId,
        ) -> Result<Holon, HolonError> {
            unreachable_in_loader_ref_resolver_tests()
        }

        fn fetch_related_holons_internal(
            &self,
            _context: &Arc<TransactionContext>,
            _source_id: &HolonId,
            _relationship_name: &RelationshipName,
        ) -> Result<HolonCollection, HolonError> {
            unreachable_in_loader_ref_resolver_tests()
        }

        fn get_all_holons_internal(
            &self,
            _context: &Arc<TransactionContext>,
        ) -> Result<HolonCollection, HolonError> {
            Ok(HolonCollection::new_saved())
        }

        fn load_holons_internal(
            &self,
            _context: &Arc<TransactionContext>,
            _bundle: TransientReference,
        ) -> Result<TransientReference, HolonError> {
            unreachable_in_loader_ref_resolver_tests()
        }
    }

    fn build_context() -> Arc<TransactionContext> {
        let holon_service: Arc<dyn HolonServiceApi> = Arc::new(TestHolonService);
        let space_manager = Arc::new(HolonSpaceManager::new_with_managers(
            None,
            holon_service,
            None,
            ServiceRoutingPolicy::BlockExternal,
        ));

        space_manager
            .get_transaction_manager()
            .open_new_transaction(Arc::clone(&space_manager))
            .expect("test transaction should open")
    }

    fn new_holon(
        context: &Arc<TransactionContext>,
        key: &str,
    ) -> Result<TransientReference, HolonError> {
        context.mutation().new_holon(Some(MapString(key.to_string())))
    }

    fn new_descriptor(
        context: &Arc<TransactionContext>,
        key: &str,
        type_name: &str,
        type_kind: TypeKind,
    ) -> Result<TransientReference, HolonError> {
        let mut descriptor = new_holon(context, key)?;
        descriptor
            .with_property_value(CorePropertyTypeName::TypeName, type_name)?
            .with_property_value(CorePropertyTypeName::IsAbstractType, false)?
            .with_property_value(
                CorePropertyTypeName::InstanceTypeKind,
                type_kind.as_schema_key(),
            )?;
        Ok(descriptor)
    }

    fn stage(
        context: &Arc<TransactionContext>,
        transient_reference: TransientReference,
    ) -> Result<HolonReference, HolonError> {
        Ok(HolonReference::Staged(context.mutation().stage_new_holon(transient_reference)?))
    }

    fn self_described_type_descriptor(
        context: &Arc<TransactionContext>,
    ) -> Result<HolonReference, HolonError> {
        let mut staged_type_descriptor = context.mutation().stage_new_holon(new_descriptor(
            context,
            "TypeDescriptor.HolonType",
            "TypeDescriptor",
            TypeKind::Holon,
        )?)?;
        let type_descriptor_reference = HolonReference::Staged(staged_type_descriptor.clone());
        staged_type_descriptor.with_descriptor(type_descriptor_reference.clone())?;
        Ok(type_descriptor_reference)
    }

    fn relationship_direction_meta(
        context: &Arc<TransactionContext>,
        direction_type_name: CoreHolonTypeName,
    ) -> Result<HolonReference, HolonError> {
        let type_name = direction_type_name.as_holon_name();
        let descriptor = new_descriptor(
            context,
            &type_name.to_string(),
            &type_name.to_string(),
            TypeKind::Relationship,
        )?;
        stage(context, descriptor)
    }

    fn typed_instance(
        context: &Arc<TransactionContext>,
        key: &str,
        descriptor: HolonReference,
    ) -> Result<HolonReference, HolonError> {
        let mut instance = new_holon(context, key)?;
        instance.add_related_holons(CoreRelationshipTypeName::DescribedBy, vec![descriptor])?;
        stage(context, instance)
    }

    fn relationship_type_descriptor(
        context: &Arc<TransactionContext>,
        relationship_name: &str,
        source_descriptor_key: &str,
        target_descriptor_key: &str,
        direction_meta: HolonReference,
    ) -> Result<HolonReference, HolonError> {
        let canonical_key = format!(
            "({})-[{}]->({})",
            source_descriptor_key, relationship_name, target_descriptor_key
        );
        let mut relationship_type =
            new_descriptor(context, &canonical_key, relationship_name, TypeKind::Relationship)?;
        relationship_type
            .add_related_holons(CoreRelationshipTypeName::Extends, vec![direction_meta])?;
        stage(context, relationship_type)
    }

    fn loader_holon_reference(
        context: &Arc<TransactionContext>,
        key: &str,
    ) -> Result<TransientReference, HolonError> {
        let mut loader_reference = new_holon(context, &format!("LoaderHolonReference.{}", key))?;
        loader_reference.with_property_value(
            CorePropertyTypeName::HolonKey,
            BaseValue::StringValue(MapString(key.to_string())),
        )?;
        Ok(loader_reference)
    }

    fn loader_relationship_reference(
        context: &Arc<TransactionContext>,
        relationship_name: &str,
        source_key: &str,
        target_keys: &[&str],
    ) -> Result<TransientReference, HolonError> {
        let mut relationship_reference = new_holon(
            context,
            &format!("LoaderRelationshipReference.{}.{}", source_key, relationship_name),
        )?;
        relationship_reference.with_property_value(
            CorePropertyTypeName::RelationshipName,
            BaseValue::StringValue(MapString(relationship_name.to_string())),
        )?;

        let source_reference = loader_holon_reference(context, source_key)?;
        let mut target_references = Vec::with_capacity(target_keys.len());
        for target_key in target_keys {
            target_references
                .push(HolonReference::Transient(loader_holon_reference(context, target_key)?));
        }

        relationship_reference.add_related_holons(
            CoreRelationshipTypeName::ReferenceSource,
            vec![HolonReference::Transient(source_reference)],
        )?;
        relationship_reference
            .add_related_holons(CoreRelationshipTypeName::ReferenceTarget, target_references)?;

        Ok(relationship_reference)
    }

    #[test]
    fn find_relationship_type_for_endpoints_returns_concrete_pair_hit() -> Result<(), HolonError> {
        let context = build_context();
        let mut resolver_state = ResolverState::new();
        let declared_meta =
            relationship_direction_meta(&context, CoreHolonTypeName::DeclaredRelationshipType)?;
        let source_descriptor = stage(
            &context,
            new_descriptor(&context, "SourceType", "SourceType", TypeKind::Holon)?,
        )?;
        let target_descriptor = stage(
            &context,
            new_descriptor(&context, "TargetType", "TargetType", TypeKind::Holon)?,
        )?;
        let expected_relationship_type = relationship_type_descriptor(
            &context,
            "Owns",
            "SourceType",
            "TargetType",
            declared_meta,
        )?;
        let source_endpoint = typed_instance(&context, "source-instance", source_descriptor)?;
        let target_endpoint = typed_instance(&context, "target-instance", target_descriptor)?;

        let (relationship_type_descriptor, direction) =
            LoaderRefResolver::find_relationship_type_for_endpoints(
                &context,
                &mut resolver_state,
                &RelationshipName(MapString("Owns".to_string())),
                &source_endpoint,
                &target_endpoint,
            )?
            .expect("concrete endpoint pair should resolve");

        assert_eq!(direction, RelationshipDirection::Declared);
        assert_eq!(
            relationship_type_descriptor.reference_id_string(),
            expected_relationship_type.reference_id_string()
        );

        Ok(())
    }

    #[test]
    fn find_relationship_type_for_endpoints_walks_abstract_ancestors_and_skips_missing_keys(
    ) -> Result<(), HolonError> {
        let context = build_context();
        let mut resolver_state = ResolverState::new();
        let inverse_meta =
            relationship_direction_meta(&context, CoreHolonTypeName::InverseRelationshipType)?;
        let abstract_source = stage(
            &context,
            new_descriptor(&context, "AbstractSource", "AbstractSource", TypeKind::Holon)?,
        )?;
        let abstract_target = stage(
            &context,
            new_descriptor(&context, "AbstractTarget", "AbstractTarget", TypeKind::Holon)?,
        )?;

        let mut keyless_source_parent = new_descriptor(
            &context,
            "KeylessSourceParent",
            "KeylessSourceParent",
            TypeKind::Holon,
        )?;
        keyless_source_parent
            .add_related_holons(CoreRelationshipTypeName::Extends, vec![abstract_source.clone()])?;
        let mut keyless_source_parent =
            context.mutation().stage_new_holon(keyless_source_parent)?;
        keyless_source_parent.remove_property_value(CorePropertyTypeName::Key)?;
        let keyless_source_parent = HolonReference::Staged(keyless_source_parent);

        let mut concrete_source =
            new_descriptor(&context, "ConcreteSource", "ConcreteSource", TypeKind::Holon)?;
        concrete_source
            .add_related_holons(CoreRelationshipTypeName::Extends, vec![keyless_source_parent])?;
        let concrete_source = stage(&context, concrete_source)?;

        let mut concrete_target =
            new_descriptor(&context, "ConcreteTarget", "ConcreteTarget", TypeKind::Holon)?;
        concrete_target
            .add_related_holons(CoreRelationshipTypeName::Extends, vec![abstract_target.clone()])?;
        let concrete_target = stage(&context, concrete_target)?;

        let expected_relationship_type = relationship_type_descriptor(
            &context,
            "VariantOf",
            "AbstractSource",
            "AbstractTarget",
            inverse_meta,
        )?;
        let source_endpoint = typed_instance(&context, "source-instance", concrete_source)?;
        let target_endpoint = typed_instance(&context, "target-instance", concrete_target)?;

        let (relationship_type_descriptor, direction) =
            LoaderRefResolver::find_relationship_type_for_endpoints(
                &context,
                &mut resolver_state,
                &RelationshipName(MapString("VariantOf".to_string())),
                &source_endpoint,
                &target_endpoint,
            )?
            .expect("abstract ancestor endpoint pair should resolve");

        assert_eq!(direction, RelationshipDirection::Inverse);
        assert_eq!(
            relationship_type_descriptor.reference_id_string(),
            expected_relationship_type.reference_id_string()
        );

        Ok(())
    }

    #[test]
    fn find_relationship_type_for_endpoints_uses_type_descriptor_fallback_for_descriptor_sources(
    ) -> Result<(), HolonError> {
        let context = build_context();
        let mut resolver_state = ResolverState::new();
        let declared_meta =
            relationship_direction_meta(&context, CoreHolonTypeName::DeclaredRelationshipType)?;

        let type_descriptor = stage(
            &context,
            new_descriptor(
                &context,
                "TypeDescriptor.HolonType",
                "TypeDescriptor",
                TypeKind::Holon,
            )?,
        )?;
        let holon_type =
            stage(&context, new_descriptor(&context, "HolonType", "HolonType", TypeKind::Holon)?)?;
        let mut schema_type =
            new_descriptor(&context, "Schema.HolonType", "Schema", TypeKind::Holon)?;
        schema_type.add_related_holons(
            CoreRelationshipTypeName::DescribedBy,
            vec![type_descriptor.clone()],
        )?;
        schema_type.add_related_holons(CoreRelationshipTypeName::Extends, vec![holon_type])?;
        let schema_type = stage(&context, schema_type)?;

        let expected_relationship_type = relationship_type_descriptor(
            &context,
            "ComponentOf",
            "TypeDescriptor.HolonType",
            "Schema.HolonType",
            declared_meta,
        )?;
        let schema_endpoint =
            typed_instance(&context, "MAP Core Schema-v0.0.5", schema_type.clone())?;

        let (relationship_type_descriptor, direction) =
            LoaderRefResolver::find_relationship_type_for_endpoints(
                &context,
                &mut resolver_state,
                &RelationshipName(MapString("ComponentOf".to_string())),
                &schema_type,
                &schema_endpoint,
            )?
            .expect("descriptor endpoint should match generic TypeDescriptor RTD");

        assert_eq!(direction, RelationshipDirection::Declared);
        assert_eq!(
            relationship_type_descriptor.reference_id_string(),
            expected_relationship_type.reference_id_string()
        );

        Ok(())
    }

    #[test]
    fn find_relationship_type_for_endpoints_prefers_concrete_descriptor_rtd_before_fallback(
    ) -> Result<(), HolonError> {
        let context = build_context();
        let mut resolver_state = ResolverState::new();
        let declared_meta =
            relationship_direction_meta(&context, CoreHolonTypeName::DeclaredRelationshipType)?;

        let type_descriptor = stage(
            &context,
            new_descriptor(
                &context,
                "TypeDescriptor.HolonType",
                "TypeDescriptor",
                TypeKind::Holon,
            )?,
        )?;
        let mut schema_type =
            new_descriptor(&context, "Schema.HolonType", "Schema", TypeKind::Holon)?;
        schema_type
            .add_related_holons(CoreRelationshipTypeName::DescribedBy, vec![type_descriptor])?;
        let schema_type = stage(&context, schema_type)?;

        relationship_type_descriptor(
            &context,
            "ComponentOf",
            "TypeDescriptor.HolonType",
            "Schema.HolonType",
            declared_meta.clone(),
        )?;
        let expected_relationship_type = relationship_type_descriptor(
            &context,
            "ComponentOf",
            "Schema.HolonType",
            "Schema.HolonType",
            declared_meta,
        )?;
        let schema_endpoint =
            typed_instance(&context, "MAP Core Schema-v0.0.5", schema_type.clone())?;

        let (relationship_type_descriptor, direction) =
            LoaderRefResolver::find_relationship_type_for_endpoints(
                &context,
                &mut resolver_state,
                &RelationshipName(MapString("ComponentOf".to_string())),
                &schema_type,
                &schema_endpoint,
            )?
            .expect("concrete descriptor RTD should resolve first");

        assert_eq!(direction, RelationshipDirection::Declared);
        assert_eq!(
            relationship_type_descriptor.reference_id_string(),
            expected_relationship_type.reference_id_string()
        );

        Ok(())
    }

    #[test]
    fn find_relationship_type_for_endpoints_matches_generic_instance_relationships_for_rtd_targets(
    ) -> Result<(), HolonError> {
        let context = build_context();
        let mut resolver_state = ResolverState::new();
        let declared_meta =
            relationship_direction_meta(&context, CoreHolonTypeName::DeclaredRelationshipType)?;

        let type_descriptor = stage(
            &context,
            new_descriptor(
                &context,
                "TypeDescriptor.HolonType",
                "TypeDescriptor",
                TypeKind::Holon,
            )?,
        )?;
        let mut schema_type =
            new_descriptor(&context, "Schema.HolonType", "Schema", TypeKind::Holon)?;
        schema_type.add_related_holons(
            CoreRelationshipTypeName::DescribedBy,
            vec![type_descriptor.clone()],
        )?;
        let schema_type = stage(&context, schema_type)?;

        let expected_relationship_type = relationship_type_descriptor(
            &context,
            "InstanceRelationships",
            "TypeDescriptor.HolonType",
            "DeclaredRelationshipType",
            declared_meta.clone(),
        )?;
        let mut depends_on = new_descriptor(
            &context,
            "(Schema.HolonType)-[DependsOn]->(Schema.HolonType)",
            "DependsOn",
            TypeKind::Relationship,
        )?;
        depends_on
            .add_related_holons(CoreRelationshipTypeName::DescribedBy, vec![type_descriptor])?;
        depends_on.add_related_holons(CoreRelationshipTypeName::Extends, vec![declared_meta])?;
        let depends_on = stage(&context, depends_on)?;

        let (relationship_type_descriptor, direction) =
            LoaderRefResolver::find_relationship_type_for_endpoints(
                &context,
                &mut resolver_state,
                &RelationshipName(MapString("InstanceRelationships".to_string())),
                &schema_type,
                &depends_on,
            )?
            .expect("relationship RTD target should match generic InstanceRelationships RTD");

        assert_eq!(direction, RelationshipDirection::Declared);
        assert_eq!(
            relationship_type_descriptor.reference_id_string(),
            expected_relationship_type.reference_id_string()
        );

        Ok(())
    }

    #[test]
    fn find_relationship_type_for_endpoints_matches_type_descriptor_endpoint_without_holon_anchor(
    ) -> Result<(), HolonError> {
        let context = build_context();
        let mut resolver_state = ResolverState::new();
        let declared_meta =
            relationship_direction_meta(&context, CoreHolonTypeName::DeclaredRelationshipType)?;
        let type_descriptor = self_described_type_descriptor(&context)?;

        let expected_relationship_type = relationship_type_descriptor(
            &context,
            "SourceType",
            "DeclaredRelationshipType",
            "TypeDescriptor.HolonType",
            declared_meta.clone(),
        )?;
        let mut implements_dance = new_descriptor(
            &context,
            "(TypeDescriptor.HolonType)-[ImplementsDance]->(DanceImplementation.HolonType)",
            "ImplementsDance",
            TypeKind::Relationship,
        )?;
        implements_dance.add_related_holons(
            CoreRelationshipTypeName::DescribedBy,
            vec![type_descriptor.clone()],
        )?;
        implements_dance
            .add_related_holons(CoreRelationshipTypeName::Extends, vec![declared_meta])?;
        let implements_dance = stage(&context, implements_dance)?;

        let (relationship_type_descriptor, direction) =
            LoaderRefResolver::find_relationship_type_for_endpoints(
                &context,
                &mut resolver_state,
                &RelationshipName(MapString("SourceType".to_string())),
                &implements_dance,
                &type_descriptor,
            )?
            .expect("TypeDescriptor endpoint should match generic TypeDescriptor RTD");

        assert_eq!(direction, RelationshipDirection::Declared);
        assert_eq!(
            relationship_type_descriptor.reference_id_string(),
            expected_relationship_type.reference_id_string()
        );

        Ok(())
    }

    #[test]
    fn find_relationship_type_for_endpoints_returns_none_when_no_pair_matches(
    ) -> Result<(), HolonError> {
        let context = build_context();
        let mut resolver_state = ResolverState::new();
        let source_descriptor = stage(
            &context,
            new_descriptor(&context, "SourceType", "SourceType", TypeKind::Holon)?,
        )?;
        let target_descriptor = stage(
            &context,
            new_descriptor(&context, "TargetType", "TargetType", TypeKind::Holon)?,
        )?;
        let source_endpoint = typed_instance(&context, "source-instance", source_descriptor)?;
        let target_endpoint = typed_instance(&context, "target-instance", target_descriptor)?;

        let result = LoaderRefResolver::find_relationship_type_for_endpoints(
            &context,
            &mut resolver_state,
            &RelationshipName(MapString("MissingRelationship".to_string())),
            &source_endpoint,
            &target_endpoint,
        )?;

        assert!(result.is_none());

        Ok(())
    }

    #[test]
    fn validate_bootstrap_relationship_targets_rejects_multiple_extends_targets() {
        let extends = CoreRelationshipTypeName::Extends.as_relationship_name();

        assert!(matches!(
            LoaderRefResolver::validate_bootstrap_relationship_targets(&extends, 2),
            Err(HolonError::InvalidRelationship(name, _)) if name == "Extends"
        ));
    }

    #[test]
    fn validate_bootstrap_relationship_targets_rejects_missing_inverse_of_target() {
        let inverse_of = CoreRelationshipTypeName::InverseOf.as_relationship_name();

        assert!(matches!(
            LoaderRefResolver::validate_bootstrap_relationship_targets(&inverse_of, 0),
            Err(HolonError::InvalidRelationship(name, _)) if name == "InverseOf"
        ));
    }

    #[test]
    fn try_resolve_by_type_graph_does_not_write_partial_declared_targets() -> Result<(), HolonError>
    {
        let context = build_context();
        let mut resolver_state = ResolverState::new();
        let mut seen_relationship_edge_keys = HashSet::new();
        let declared_meta =
            relationship_direction_meta(&context, CoreHolonTypeName::DeclaredRelationshipType)?;
        let source_descriptor = stage(
            &context,
            new_descriptor(&context, "SourceType", "SourceType", TypeKind::Holon)?,
        )?;
        let target_descriptor = stage(
            &context,
            new_descriptor(&context, "TargetType", "TargetType", TypeKind::Holon)?,
        )?;
        let missing_target_descriptor = stage(
            &context,
            new_descriptor(&context, "MissingTargetType", "MissingTargetType", TypeKind::Holon)?,
        )?;

        relationship_type_descriptor(&context, "Owns", "SourceType", "TargetType", declared_meta)?;
        let source_endpoint = typed_instance(&context, "source-instance", source_descriptor)?;
        let _target_endpoint = typed_instance(&context, "target-instance", target_descriptor)?;
        let _missing_target_endpoint =
            typed_instance(&context, "missing-target-instance", missing_target_descriptor)?;
        let relationship_reference = loader_relationship_reference(
            &context,
            "Owns",
            "source-instance",
            &["target-instance", "missing-target-instance"],
        )?;

        let result = LoaderRefResolver::try_resolve_by_type_graph(
            &context,
            &mut resolver_state,
            &relationship_reference,
            &mut seen_relationship_edge_keys,
        );

        assert!(matches!(result, Err(HolonError::HolonNotFound(_))));
        assert!(seen_relationship_edge_keys.is_empty());

        let relationship_name = RelationshipName(MapString("Owns".to_string()));
        let related_handle = source_endpoint.related_holons(&relationship_name)?;
        let related_members = related_handle
            .read()
            .map_err(|_| HolonError::FailedToBorrow("Owns collection read lock poisoned".into()))?
            .get_members()
            .clone();
        assert!(related_members.is_empty());

        Ok(())
    }
}
