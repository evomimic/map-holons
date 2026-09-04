// shared_crates/holons_loader/src/loader_ref_resolver.rs
//
// Pass-2 (Resolver): Transform queued LoaderRelationshipReference holons into
// concrete writes on staged holons. Implements the multi-pass, graph-driven
// declared relationship authoring policy:
//   Pass-2a: write DescribedBy first
//   Pass-2b: write Extends next so descriptor ancestry is available
//   Pass-2c: resolve remaining authored relationships once
//
// Design goals:
// - Self-contained, self-describing code with explicit invariants
// - No global/in-memory relationship name index; resolution is graph-proven
// - Non-fatal errors are accumulated; the controller decides commit policy
// - Deduplicate within the resolver run: (source, declared_name, target)
// - Never invent inline holons here (no new instance staging):
//   only write to already staged holons or stage new versions of saved ones
//
// Safety guardrails:
// - DescribedBy must target exactly one descriptor
// - Bootstrap relationships are selected by name before the type graph is queryable
// - Instance relationships must be authored in declared orientation

use std::collections::HashSet;
use std::rc::Rc;
use std::sync::Arc;
use tracing::debug;

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
    ///   1) Pass-2a: DescribedBy -> with_descriptor()
    ///   2) Pass-2b: Extends -> add_related_holons_ungoverned()
    ///   3) Pass-2c: process remaining declared relationship references
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

        // ── Unified worklist for pass-2c: everything not handled by bootstrap passes.
        let deferred_queue: Vec<TransientReference> = queued_relationship_references
            .into_iter()
            .filter(|lrr| !Self::is_described_by_by_name(lrr) && !Self::is_extends_by_name(lrr))
            .collect();

        let (created, errors) = Self::process_remaining_references(
            context,
            &mut resolver_state,
            deferred_queue,
            &mut seen_relationship_edge_keys,
        );

        outcome.links_created += created;
        outcome.errors.extend(errors);

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
    // Pass-2b: type-graph bootstrap relationships
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

        if *relationship_name == extends && target_count != 1 {
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
    // Pass-2c: Process remaining relationship references
    // ─────────────────────────────────────────────────────────────────────

    /// After bootstrap passes, resolve each remaining authored relationship once.
    /// Every input holon was staged before Pass 2 and `DescribedBy`/`Extends`
    /// were written in the preceding passes, so a later Pass 2c write cannot
    /// make a failed named reference become resolvable.
    fn process_remaining_references(
        context: &Arc<TransactionContext>,
        resolver_state: &mut ResolverState,
        queue: Vec<TransientReference>,
        seen: &mut HashSet<RelationshipEdgeKey>,
    ) -> (i64, Vec<ErrorWithContext>) {
        let mut errors: Vec<ErrorWithContext> = Vec::new();
        let mut total_links_created = 0i64;
        for relationship_reference in queue {
            if Self::is_described_by_by_name(&relationship_reference)
                || Self::is_extends_by_name(&relationship_reference)
            {
                continue;
            }
            match Self::try_resolve_by_type_graph(
                context,
                resolver_state,
                &relationship_reference,
                seen,
            ) {
                Ok(created) => total_links_created += created,
                Err(error) => errors.push(Self::error_with_context(&relationship_reference, error)),
            }
        }
        (total_links_created, errors)
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
    /// - Others: batch → `add_related_holons_ungoverned`
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
        staged_source
            .add_related_holons_ungoverned(declared_relationship_name.clone(), write_targets)?;

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

        // Bootstrap relationships are handled before type-graph resolution.
        if relationship_name == described_by || relationship_name == extends {
            return Ok(0);
        }

        debug!(
            "[resolver] BEFORE resolve_endpoints: {}, source_loader_key={:?}",
            Self::brief_lrr_summary(relationship_reference),
            Self::source_loader_key_of_lrr(relationship_reference).map(|k| k.0),
        );
        let (source_endpoint, target_endpoints) =
            Self::resolve_endpoints(context, resolver_state, relationship_reference)?;

        let mut planned_edge_keys: HashSet<RelationshipEdgeKey> = HashSet::new();
        let declared_write = Self::plan_declared_relationship_write(
            context,
            &relationship_name,
            &source_endpoint,
            target_endpoints,
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

    // ─────────────────────────────────────────────────────────────────────
    // Low-level helpers
    // ─────────────────────────────────────────────────────────────────────

    /// Short diagnostic summary for a LoaderRelationshipReference.
    fn brief_lrr_summary(lrr: &TransientReference) -> String {
        let name = Self::extract_relationship_metadata(lrr)
            .unwrap_or_else(|_| RelationshipName(MapString("<unknown>".into())));
        format!("name={}", name)
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
