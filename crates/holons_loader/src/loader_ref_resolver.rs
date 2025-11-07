// crates/holons_loader/src/loader_ref_resolver.rs
//
// Pass-2 (Resolver): Transform queued LoaderRelationshipReference holons into
// concrete writes on staged holons. Implements the multi‑pass, graph‑driven
// inverse handling policy:
//   Pass-2a: write DescribedBy (declared) first
//   Pass-2b: write InverseOf (declared) next (no endpoint prefilter)
//   Pass-2c: resolve remaining relationships via fixed-point iteration
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
// - Only trust InverseOf links whose endpoints are relationship type descriptors
// - If a declared name for an inverse cannot be proven via type graph → error

use std::collections::HashSet;
use std::rc::Rc;
use tracing::debug;

use core_types::type_kinds::TypeKind;
use holons_prelude::prelude::*;

/// Outcome of Pass-2: counts successful writes and collects non-fatal errors.
#[derive(Debug, Default)]
pub struct ResolverOutcome {
    /// Total number of links scheduled on staged holons
    pub links_created: i64,
    /// Non-fatal errors encountered during resolution
    pub errors: Vec<HolonError>,
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

/// Per-run resolver state. Holds data we want to compute once and reuse.
/// Start small (just the saved index), but this scales well if we add
/// metrics, feature flags, or lazy fetches later.
pub struct ResolverState {
    /// Optional snapshot of *saved* holons for key-based lookups.
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
    /// If already present, this is a no-op. Otherwise, it attempts to fetch
    /// all holons once via the HolonOperationsApi and stores the collection.
    pub fn ensure_saved_index(
        &mut self,
        context: &dyn HolonsContextBehavior,
    ) -> Result<(), HolonError> {
        if self.saved_index.is_some() {
            return Ok(());
        }
        let collection = get_all_holons(context)?;
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
    ///   1) Pass-2a: declared DescribedBy → with_descriptor()
    ///   2) Pass-2b: declared InverseOf → add_related_holons() (no prefilter)
    ///   3) Pass-2c: process remaining relationship references
    pub fn resolve_relationships(
        context: &dyn HolonsContextBehavior,
        queued_relationship_references: Vec<TransientReference>,
    ) -> Result<ResolverOutcome, HolonError> {
        let mut outcome = ResolverOutcome::default();
        let mut seen_relationship_edge_keys: HashSet<RelationshipEdgeKey> = HashSet::new();
        let mut resolver_state = ResolverState::new();

        // ── Pass-2a: ensure all descriptors are set (enables type graph walks later)
        Self::pass_2a_write_described_by_declared(
            context,
            &queued_relationship_references,
            &mut seen_relationship_edge_keys,
            &mut outcome,
        );

        // ── Pass-2b: write any declared InverseOf edges
        Self::pass_2b_write_inverse_of_declared(
            context,
            &queued_relationship_references,
            &mut seen_relationship_edge_keys,
            &mut outcome,
        );

        // ── Unified worklist for pass-2c: everything that is NOT (declared DescribedBy) and NOT (declared InverseOf)
        let deferred_queue: Vec<TransientReference> = queued_relationship_references
            .into_iter()
            .filter(|lrr| {
                !Self::is_described_by_declared(context, lrr)
                    && !Self::is_inverse_of_declared(context, lrr)
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
            outcome.errors.push(HolonError::InvalidType(format!(
                "LoaderRelationshipReference could not be resolved after fixed-point retries: {}",
                Self::brief_lrr_summary(context, &relationship_reference)
            )));
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

    /// Returns true if the LRR is declared (IsDeclared = true). Errors default to false.
    fn is_declared(
        context: &dyn HolonsContextBehavior,
        relationship_reference: &TransientReference,
    ) -> bool {
        let is_declared_property: PropertyName =
            CorePropertyTypeName::IsDeclared.as_property_name();
        match relationship_reference.property_value(context, &is_declared_property) {
            Ok(Some(BaseValue::BooleanValue(b))) => b.0,
            _ => false,
        }
    }

    /// Returns true if the LRR’s relationship name equals `relationship_name`.
    fn has_relationship_name(
        context: &dyn HolonsContextBehavior,
        relationship_reference: &TransientReference,
        relationship_name: &RelationshipName,
    ) -> bool {
        let relationship_name_property: PropertyName =
            CorePropertyTypeName::RelationshipName.as_property_name();
        match relationship_reference.property_value(context, &relationship_name_property) {
            Ok(Some(BaseValue::StringValue(MapString(s)))) => {
                &s.to_relationship_name() == relationship_name
            }
            _ => false,
        }
    }

    /// Returns true if the LRR is a declared DescribedBy relationship.
    fn is_described_by_declared(
        context: &dyn HolonsContextBehavior,
        relationship_reference: &TransientReference,
    ) -> bool {
        let described_by = CoreRelationshipTypeName::DescribedBy.as_relationship_name();
        Self::is_declared(context, relationship_reference)
            && Self::has_relationship_name(context, relationship_reference, &described_by)
    }

    /// Returns true if the LRR is a declared InverseOf relationship.
    fn is_inverse_of_declared(
        context: &dyn HolonsContextBehavior,
        relationship_reference: &TransientReference,
    ) -> bool {
        let inverse_of = CoreRelationshipTypeName::InverseOf.as_relationship_name();
        Self::is_declared(context, relationship_reference)
            && Self::has_relationship_name(context, relationship_reference, &inverse_of)
    }

    // ─────────────────────────────────────────────────────────────────────
    // Pass-2a: DescribedBy (declared)
    // ─────────────────────────────────────────────────────────────────────

    /// Writes all declared DescribedBy edges; enforces exactly one target.
    fn pass_2a_write_described_by_declared(
        context: &dyn HolonsContextBehavior,
        queue: &[TransientReference],
        seen: &mut HashSet<RelationshipEdgeKey>,
        outcome: &mut ResolverOutcome,
    ) {
        let described_by = CoreRelationshipTypeName::DescribedBy.as_relationship_name();

        let described_by_refs: Vec<_> =
            queue.iter().filter(|r| Self::is_described_by_declared(context, r)).collect();
        debug!("Pass 2A: Processing {} DescribedBy relationships", described_by_refs.len());

        for relationship_reference in described_by_refs {
            match Self::resolve_endpoints(context, relationship_reference) {
                Ok((source_endpoint, mut target_endpoints)) => {
                    // Enforce exactly one target for DescribedBy
                    if target_endpoints.len() != 1 {
                        outcome.errors.push(HolonError::InvalidRelationship(
                            described_by.to_string(),
                            "DescribedBy requires exactly one target".into(),
                        ));
                        continue;
                    }

                    // Resolve staged write source (the LRR source in declared orientation)
                    let staged_source =
                        match Self::resolve_staged_write_source(context, &source_endpoint) {
                            Ok(s) => s,
                            Err(e) => {
                                outcome.errors.push(e);
                                continue;
                            }
                        };

                    // Dedupe key: (source, DescribedBy, descriptor)
                    let edge_key = Self::make_edge_key(
                        context,
                        &HolonReference::Staged(staged_source.clone()),
                        &described_by,
                        &target_endpoints[0],
                    );
                    if !seen.insert(edge_key) {
                        debug!("Duplicate DescribedBy skipped (declared)");
                        continue;
                    }

                    // Perform the write using with_descriptor()
                    match Self::write_relationship(
                        context,
                        staged_source,
                        &described_by,
                        target_endpoints.split_off(0), // exactly one
                    ) {
                        Ok(n) => outcome.links_created += n,
                        Err(e) => outcome.errors.push(e),
                    }
                }
                Err(e) => outcome.errors.push(e),
            }
        }
    }

    // ─────────────────────────────────────────────────────────────────────
    // Pass-2b: InverseOf (declared)
    // ─────────────────────────────────────────────────────────────────────

    /// Writes all declared InverseOf edges (no endpoint prefilter).
    fn pass_2b_write_inverse_of_declared(
        context: &dyn HolonsContextBehavior,
        queue: &[TransientReference],
        seen: &mut HashSet<RelationshipEdgeKey>,
        outcome: &mut ResolverOutcome,
    ) {
        let inverse_of = CoreRelationshipTypeName::InverseOf.as_relationship_name();
        let inverse_of_refs: Vec<_> = queue
            .iter()
            .filter(|reference| Self::is_inverse_of_declared(context, reference))
            .collect();
        debug!("Pass 2B: Processing {} InverseOf relationships", inverse_of_refs.len());

        for relationship_reference in inverse_of_refs {
            match Self::resolve_endpoints(context, relationship_reference) {
                Ok((source_endpoint, target_endpoints)) => {
                    let staged_source =
                        match Self::resolve_staged_write_source(context, &source_endpoint) {
                            Ok(s) => s,
                            Err(e) => {
                                outcome.errors.push(e);
                                continue;
                            }
                        };

                    // Deduplicate per (source, InverseOf, each target)
                    let mut unique_targets: Vec<HolonReference> =
                        Vec::with_capacity(target_endpoints.len());
                    let source_ref = HolonReference::Staged(staged_source.clone());
                    for target in target_endpoints.into_iter() {
                        let edge_key =
                            Self::make_edge_key(context, &source_ref, &inverse_of, &target);
                        if seen.insert(edge_key) {
                            unique_targets.push(target);
                        } else {
                            debug!("Duplicate InverseOf skipped (declared)");
                        }
                    }

                    match Self::write_relationship(
                        context,
                        staged_source,
                        &inverse_of,
                        unique_targets,
                    ) {
                        Ok(n) => outcome.links_created += n,
                        Err(e) => outcome.errors.push(e),
                    }
                }
                Err(e) => outcome.errors.push(e),
            }
        }
    }

    // ─────────────────────────────────────────────────────────────────────
    // Pass-2c: Process remaining relationship references
    // ─────────────────────────────────────────────────────────────────────

    /// After 2a/2b, process all remaining references together.
    /// Removes successes & fatals; retains only deferrables; stops at fixed point.
    fn process_remaining_references(
        context: &dyn HolonsContextBehavior,
        resolver_state: &mut ResolverState,
        mut remaining_queue: Vec<TransientReference>,
        seen: &mut HashSet<RelationshipEdgeKey>,
    ) -> (i64, Vec<HolonError>, Vec<TransientReference>) {
        let mut errors = Vec::new();
        let mut total_links_created = 0i64;
        debug!("Processing REMAINING_REFERENCES");
        // Conservative upper bound; usually we break by fixed-point first.
        // At least 2 passes to allow for progress.
        let mut passes_remaining = (remaining_queue.len() + 1).max(2);

        while passes_remaining > 0 {
            let mut links_created_this_pass = 0i64;
            let mut pass_fatal_errors = Vec::new();

            // Filter in place using retain() and true/false return values.
            remaining_queue.retain(|relationship_reference| {
                // Skip anything already handled by 2a/2b (defensive; unified_queue already filtered)
                if Self::is_described_by_declared(context, relationship_reference)
                    || Self::is_inverse_of_declared(context, relationship_reference)
                {
                    return false;
                }

                let is_declared = Self::is_declared(context, relationship_reference);
                let resolution_result = if is_declared {
                    Self::try_declared_single_resolve(context, relationship_reference, seen)
                } else {
                    Self::try_inverse_single_resolve(
                        context,
                        resolver_state,
                        relationship_reference,
                        seen,
                    )
                };

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
                        pass_fatal_errors.push(e);
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

    /// Resolve the declared relationship name for a single inverse LRR via type-gated graph walk.
    fn declared_name_for_inverse(
        context: &dyn HolonsContextBehavior,
        resolver_state: &mut ResolverState,
        inverse_name: &RelationshipName,
        src_endpoint: &HolonReference,
        dst_endpoint: &HolonReference,
    ) -> Result<RelationshipName, HolonError> {
        debug!("[resolver] entering declared_name_for_inverse for inverse '{}'", inverse_name.0);

        // 1) Resolve endpoint type descriptors (instances → follow DescribedBy; types pass through).
        let src_type_td = Self::resolve_type_descriptor(context, src_endpoint)?;
        let dst_type_td = Self::resolve_type_descriptor(context, dst_endpoint)?;
        debug!("[resolver] TypeDescriptors resolved for endpoints of inverse '{}'", inverse_name.0);

        // 2) Build the **canonical key** for the *inverse* RTD using descriptor Keys.
        let key_prop: PropertyName = CorePropertyTypeName::Key.as_property_name();
        let src_desc_key = Self::read_string_property(context, &src_type_td, &key_prop)?;
        let dst_desc_key = Self::read_string_property(context, &dst_type_td, &key_prop)?;
        let inverse_key =
            MapString(format!("({})-[{}]->({})", src_desc_key.0, inverse_name.0, dst_desc_key.0));
        debug!("[resolver] looking up RelationshipType by key '{}'", inverse_key.0);

        // 3) Locate the inverse RTD by canonical key (prefer staged).
        let inverse_reltype =
            match Self::find_relationship_type_by_key(context, resolver_state, &inverse_key)? {
                Some(h) => h,
                None => {
                    return Err(HolonError::HolonNotFound(format!(
                        "RelationshipType for key '{}'",
                        inverse_key.0
                    )));
                }
            };
        debug!("[resolver] found RelationshipType for key '{}'", inverse_key.0);

        // 4) Follow InverseOf from the inverse RTD to the declared RTD.
        let inverse_of = CoreRelationshipTypeName::InverseOf.as_relationship_name();
        let declared_handle = inverse_reltype.related_holons(context, &inverse_of)?;
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
                format!("No InverseOf target from relationship type key '{}'", inverse_key.0),
            ));
        }

        debug!("[resolver] declared type descriptor type-gated filtering");
        let mut valid_targets = Vec::new();
        for candidate in related_members {
            if Self::is_relationship_type_kind(context, &candidate) {
                valid_targets.push(candidate);
            }
        }

        match valid_targets.len() {
            1 => {
                // We return the **declared relationship name** (TypeName is correct here).
                let type_name_prop: PropertyName =
                    CorePropertyTypeName::TypeName.as_property_name();
                let declared_type_name =
                    Self::read_string_property(context, &valid_targets[0], &type_name_prop)?;
                Ok(declared_type_name.to_relationship_name())
            }
            0 => Err(HolonError::InvalidType(format!(
                "InverseOf targets for key '{}' did not include a RelationshipTypeDescriptor",
                inverse_key.0
            ))),
            n => Err(HolonError::DuplicateError(
                "inverse mapping".into(),
                format!(
                    "Multiple RelationshipTypeDescriptor targets ({}) via InverseOf for key '{}'",
                    n, inverse_key.0
                ),
            )),
        }
    }

    // ─────────────────────────────────────────────────────────────────────
    // Endpoint + type-graph helpers
    // ─────────────────────────────────────────────────────────────────────

    /// Extracts (relationship_name, is_declared) from an LRR.
    fn extract_relationship_metadata(
        context: &dyn HolonsContextBehavior,
        relationship_reference: &TransientReference,
    ) -> Result<(RelationshipName, bool), HolonError> {
        let relationship_name_property: PropertyName =
            CorePropertyTypeName::RelationshipName.as_property_name();
        let is_declared_property: PropertyName =
            CorePropertyTypeName::IsDeclared.as_property_name();

        let relationship_value = relationship_reference
            .property_value(context, &relationship_name_property)?
            .ok_or_else(|| {
                HolonError::EmptyField("LoaderRelationshipReference.RelationshipName".into())
            })?;

        let relationship_name = match relationship_value {
            BaseValue::StringValue(MapString(text)) => text.to_relationship_name(),
            other => {
                return Err(HolonError::UnexpectedValueType(
                    format!("{:?}", other),
                    "String".into(),
                ))
            }
        };

        let is_declared_value =
            relationship_reference.property_value(context, &is_declared_property)?.ok_or_else(
                || HolonError::EmptyField("LoaderRelationshipReference.IsDeclared".into()),
            )?;

        let is_declared_flag: bool = match is_declared_value {
            BaseValue::BooleanValue(inner) => inner.0,
            other => {
                return Err(HolonError::UnexpectedValueType(format!("{:?}", other), "bool".into()))
            }
        };

        Ok((relationship_name, is_declared_flag))
    }

    /// Resolve LoaderHolonReference endpoints to actual holon references.
    /// Ensures exactly one `ReferenceSource` and ≥1 `ReferenceTarget`;
    /// Returns (source_holon, target_holons) where each has been dereferenced
    /// from its LoaderHolonReference wrapper.
    fn resolve_endpoints(
        context: &dyn HolonsContextBehavior,
        relationship_reference: &TransientReference,
    ) -> Result<(HolonReference, Vec<HolonReference>), HolonError> {
        let source_relationship = CoreRelationshipTypeName::ReferenceSource.as_relationship_name();
        let target_relationship = CoreRelationshipTypeName::ReferenceTarget.as_relationship_name();

        // Get LoaderHolonReference wrappers (not the actual holons yet)
        let source_refs_handle =
            relationship_reference.related_holons(context, source_relationship)?;
        let target_refs_handle =
            relationship_reference.related_holons(context, target_relationship)?;

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
                ))
            }
            n => {
                return Err(HolonError::DuplicateError(
                    "ReferenceSource".into(),
                    format!("{n} found"),
                ))
            }
        }

        // At least one ReferenceTarget
        if target_loader_refs.is_empty() {
            return Err(HolonError::EmptyField(
                "LoaderRelationshipReference.ReferenceTarget".into(),
            ));
        }

        // Dereference: LoaderHolonReference → actual HolonReference
        let source_holon = Self::resolve_loader_holon_reference(context, &source_loader_refs[0])?;
        debug!(
            "[resolver]   resolved source holon = {}",
            Self::best_identifier_for_dedupe(context, &source_holon)
        );

        let mut target_holons = Vec::with_capacity(target_loader_refs.len());
        for loader_ref in target_loader_refs.iter() {
            let resolved = Self::resolve_loader_holon_reference(context, loader_ref)?;
            debug!(
                "[resolver]   resolved target holon = {}",
                Self::best_identifier_for_dedupe(context, &resolved)
            );
            target_holons.push(resolved);
        }

        Ok((source_holon, target_holons))
    }

    /// Dereference a LoaderHolonReference to the actual holon it points to.
    ///
    /// Resolution order (per spec):
    /// 1. `holon_key` → staged holon via Nursery
    /// 2. `holon_id` → saved holon by ID
    /// 3. (Future) `proxy_key`/`proxy_id` → external holon via proxy
    fn resolve_loader_holon_reference(
        context: &dyn HolonsContextBehavior,
        loader_ref: &HolonReference,
    ) -> Result<HolonReference, HolonError> {
        // Property names from LoaderHolonReference schema
        let holon_key_property = CorePropertyTypeName::HolonKey.as_property_name();
        let holon_id_property = CorePropertyTypeName::HolonId.as_property_name();

        // Try holon_key first (local staged)
        if let Some(BaseValue::StringValue(key)) =
            loader_ref.property_value(context, &holon_key_property)?
        {
            debug!("[resolver] dereference LHR by holon_key='{}'", key.0);
            // Use the convenience API for single expected match
            return match get_staged_holon_by_base_key(context, &key) {
                Ok(staged) => {
                    debug!("[resolver]   → FOUND staged holon for key='{}'", key.0);
                    Ok(HolonReference::Staged(staged))
                }
                Err(HolonError::HolonNotFound(_)) => {
                    // Key was present, but nothing staged yet → deferrable
                    debug!("[resolver]   → NO staged holon for key='{}' (HolonNotFound)", key.0);
                    Err(HolonError::HolonNotFound(format!("staged holon with key '{}'", key.0)))
                }
                Err(e) => {
                    // Propagate duplicate/borrow/etc.
                    debug!("[resolver]   → lookup for key='{}' failed with: {:?}", key.0, e);
                    Err(e)
                }
            };
        }

        // TODO: un-comment when saved holon fetch by ID is implemented (we need a MapBytes BaseValue variant)
        // Try holon_id (saved)
        // if let Some(BaseValue::BytesValue(id_bytes)) =
        //     loader_ref.property_value(context, &holon_id_property)?
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

    /// Resolve the *type descriptor* for an endpoint:
    /// - If the endpoint is an instance holon, follow its single `DescribedBy` relationship to obtain
    ///   the concrete type descriptor holon.
    /// - If the endpoint is already a type descriptor holon, return it unchanged (do not follow
    ///   to the meta TypeDescriptor).
    /// - If no `DescribedBy` relationship is found and the endpoint is not clearly a descriptor,
    ///   return an `EmptyField` error because after Pass‑2a every holon should have a resolved descriptor;
    ///   missing descriptors at this stage are unexpected and indicate malformed input or earlier failure.
    fn resolve_type_descriptor(
        context: &dyn HolonsContextBehavior,
        endpoint: &HolonReference,
    ) -> Result<HolonReference, HolonError> {
        let described_by = CoreRelationshipTypeName::DescribedBy.as_relationship_name();
        let type_name_prop: PropertyName = CorePropertyTypeName::TypeName.as_property_name();
        let type_descriptor_name = CoreHolonTypeName::TypeDescriptor.as_holon_name();

        // Read `DescribedBy` targets (propagate access errors).
        // NOTE: Safe to hold this read lock in resolver paths; bundles & RTD sets are immutable during Pass-2.
        let related_handle = endpoint.related_holons(context, &described_by)?;
        let related_guard = related_handle.read().map_err(|_| {
            HolonError::FailedToBorrow("DescribedBy collection read lock poisoned".into())
        })?;
        let described_members = related_guard.get_members(); // &Vec<HolonReference>

        match described_members.len() {
            0 => Err(HolonError::EmptyField("DescribedBy".into())),
            1 => {
                let candidate_ref = &described_members[0];

                // If the `DescribedBy` target is *meta* TypeDescriptor, endpoint is a TypeDescriptor instance.
                if let Ok(candidate_type_name) =
                    Self::read_string_property(context, candidate_ref, &type_name_prop)
                {
                    if candidate_type_name == type_descriptor_name {
                        // Endpoint is itself a TypeDescriptor (do not climb to meta)
                        return Ok(endpoint.clone());
                    }
                }

                // Otherwise the candidate is the concrete type descriptor for the instance endpoint.
                Ok(candidate_ref.clone())
            }
            _ => Err(HolonError::DuplicateError(
                "DescribedBy".into(),
                "Expected exactly one descriptor target".into(),
            )),
        }
    }

    /// Look up a relationship type descriptor by its canonical key
    /// `"(SourceType.HolonType)-[RelationshipName]->(TargetType.HolonType)"`.
    /// Preference order:
    ///   1) Staged (Nursery) lookup by base key
    ///   2) Saved fallback via a pre-fetched HolonCollection and `get_by_key`
    /// Returns `Ok(None)` if not found in either place.
    fn find_relationship_type_by_key(
        context: &dyn HolonsContextBehavior,
        resolver_state: &mut ResolverState,
        canonical_key: &MapString,
    ) -> Result<Option<HolonReference>, HolonError> {
        debug!("[resolver] looking up RelationshipType by key '{}'", canonical_key.0);

        // 1) Prefer staged (Nursery) lookup by base key.
        let staging_service_handle = context.get_space_manager().get_staging_service();
        let staged_candidates = {
            let guard = staging_service_handle.read().map_err(|_| {
                HolonError::FailedToBorrow("Staging service read lock poisoned".into())
            })?;
            guard.get_staged_holons_by_base_key(canonical_key)?
        };

        match staged_candidates.len() {
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

    /// Returns true if the holon's `InstanceTypeKind == "Relationship"`.
    fn is_relationship_type_kind(
        context: &dyn HolonsContextBehavior,
        holon_reference: &HolonReference,
    ) -> bool {
        debug!("[resolver] entering is_relationship_type_kind");

        let property_name: PropertyName = CorePropertyTypeName::InstanceTypeKind.as_property_name();
        let expected = TypeKind::Relationship.to_string();

        match holon_reference.property_value(context, &property_name) {
            Ok(Some(PropertyValue::StringValue(MapString(actual)))) => actual == expected,

            _ => false,
        }
    }

    // ─────────────────────────────────────────────────────────────────────
    // Writing + dedupe + worklist
    // ─────────────────────────────────────────────────────────────────────

    /// Ensures a writable staged source (promote saved → staged if policy allows).
    fn resolve_staged_write_source(
        context: &dyn HolonsContextBehavior,
        write_source_endpoint: &HolonReference,
    ) -> Result<StagedReference, HolonError> {
        let staging_service_handle = context.get_space_manager().get_staging_service();

        // 1) If the endpoint already corresponds to a staged holon, use it (prefer versioned key).
        if let HolonReference::Staged(s) = write_source_endpoint {
            return Ok(s.clone());
        }
        if let Ok(versioned_key) = write_source_endpoint.versioned_key(context) {
            // Short read lock to check by versioned key
            if let Ok(staged_ref) = {
                let guard = staging_service_handle.read().map_err(|_| {
                    HolonError::FailedToBorrow("Staging service read lock poisoned".into())
                })?;
                guard.get_staged_holon_by_versioned_key(&versioned_key)
            } {
                return Ok(staged_ref);
            }
        }

        // Try base key as a secondary staged lookup.
        if let Ok(Some(base_key)) = write_source_endpoint.key(context) {
            // Read lock just long enough to fetch the list
            let staged_matches = {
                let guard = staging_service_handle.read().map_err(|_| {
                    HolonError::FailedToBorrow("Staging service read lock poisoned".into())
                })?;
                guard.get_staged_holons_by_base_key(&base_key)?
            };

            match staged_matches.len() {
                1 => return Ok(staged_matches.into_iter().next().unwrap()),
                n if n > 1 => {
                    return Err(HolonError::DuplicateError(
                        "write source by base key".into(),
                        n.to_string(),
                    ))
                }
                _ => {
                    // not staged by base key; try promotion next
                }
            }
        }

        // 2) Promotion path: saved → stage a new version (requires HolonId).
        if let Ok(saved_id) = write_source_endpoint.holon_id(context) {
            let smart_reference = SmartReference::new_from_id(saved_id);
            let staged_reference = stage_new_version(context, smart_reference)?;
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
        context: &dyn HolonsContextBehavior,
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
                    staged_source.with_descriptor(context, write_targets.remove(0))?;
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
        staged_source.add_related_holons(
            context,
            declared_relationship_name.clone(),
            write_targets,
        )?;

        Ok(number_of_targets)
    }

    /// Builds a stable dedupe key for (source, relationship, target).
    fn make_edge_key(
        context: &dyn HolonsContextBehavior,
        source_ref: &HolonReference,
        relationship_name: &RelationshipName,
        target_ref: &HolonReference,
    ) -> RelationshipEdgeKey {
        RelationshipEdgeKey {
            source_identifier: Self::best_identifier_for_dedupe(context, source_ref),
            relationship_name: relationship_name.clone(),
            target_identifier: Self::best_identifier_for_dedupe(context, target_ref),
        }
    }

    /// Best-effort identifier for dedupe/diagnostics: id > versioned_key > key > "<no-id>".
    fn best_identifier_for_dedupe(
        context: &dyn HolonsContextBehavior,
        reference: &HolonReference,
    ) -> String {
        if let Ok(id) = reference.holon_id(context) {
            return format!("id:{id}");
        }
        if let Ok(vk) = reference.versioned_key(context) {
            return format!("vkey:{vk}");
        }
        if let Ok(Some(k)) = reference.key(context) {
            return format!("key:{k}");
        }
        "<no-id>".to_string()
    }

    /// Handle a single DECLARED (non-InverseOf, non-DescribedBy) reference.
    /// Returns number of links created (may be 0 if dedup) or an error.
    fn try_declared_single_resolve(
        context: &dyn HolonsContextBehavior,
        relationship_reference: &TransientReference,
        seen: &mut HashSet<RelationshipEdgeKey>,
    ) -> Result<i64, HolonError> {
        debug!("[resolver] Entering try_declared_single_resolve");
        // Fast skips if caller forgot to prefilter
        if !Self::is_declared(context, relationship_reference)
            || Self::is_described_by_declared(context, relationship_reference)
            || Self::is_inverse_of_declared(context, relationship_reference)
        {
            return Ok(0);
        }

        let (declared_relationship_name, _is_declared) =
            Self::extract_relationship_metadata(context, relationship_reference)?;
        let described_by = CoreRelationshipTypeName::DescribedBy.as_relationship_name();
        let inverse_of = CoreRelationshipTypeName::InverseOf.as_relationship_name();

        // Defensive: exclude the two handled in 2a/2b
        if declared_relationship_name == described_by || declared_relationship_name == inverse_of {
            return Ok(0);
        }

        let (source_endpoint, target_endpoints) =
            Self::resolve_endpoints(context, relationship_reference)?;

        let staged_source = Self::resolve_staged_write_source(context, &source_endpoint)?;

        // Dedupe per (source, declared_name, each target)
        let source_ref = HolonReference::Staged(staged_source.clone());
        let mut unique_targets: Vec<HolonReference> = Vec::new();
        for target in target_endpoints.into_iter() {
            let edge_key =
                Self::make_edge_key(context, &source_ref, &declared_relationship_name, &target);
            if seen.insert(edge_key) {
                unique_targets.push(target);
            }
        }

        Self::write_relationship(
            context,
            staged_source,
            &declared_relationship_name,
            unique_targets,
        )
    }

    /// Handle a single INVERSE (IsDeclared=false) reference via type-gated graph walk.
    /// Returns number of links created (sum across flipped targets) or an error
    /// if *no* targets could be processed (fatal). Deferrables should be returned as Err(deferrable).
    fn try_inverse_single_resolve(
        context: &dyn HolonsContextBehavior,
        resolver_state: &mut ResolverState,
        relationship_reference: &TransientReference,
        seen: &mut HashSet<RelationshipEdgeKey>,
    ) -> Result<i64, HolonError> {
        debug!("[resolver] Entering try_inverse_single_resolve");
        if Self::is_declared(context, relationship_reference) {
            return Ok(0); // not an inverse item
        }

        let (inverse_name, _flag) =
            Self::extract_relationship_metadata(context, relationship_reference)?;
        let (src_endpoint, target_endpoints) =
            Self::resolve_endpoints(context, relationship_reference)?;

        let mut created_link_count = 0i64;

        // Precompute declared target identifier for logging
        let declared_target_identifier = Self::best_identifier_for_dedupe(context, &src_endpoint);

        for target_endpoint in target_endpoints.into_iter() {
            // Derive declared relationship name from type graph
            let declared_name = Self::declared_name_for_inverse(
                context,
                resolver_state,
                &inverse_name,
                &src_endpoint,
                &target_endpoint,
            )?;

            // In declared orientation, each original target becomes the write source
            let staged_source = Self::resolve_staged_write_source(context, &target_endpoint)?;

            // Per-edge dedupe across (declared_source, declared_name, declared_target)
            let declared_source_ref = HolonReference::Staged(staged_source.clone());
            let edge_key =
                Self::make_edge_key(context, &declared_source_ref, &declared_name, &src_endpoint);
            if !seen.insert(edge_key) {
                debug!("Duplicate relationship skipped (inverse→declared)");
                continue;
            }

            // Perform the flipped write: declared_source −[declared_name]→ declared_target (original src)
            debug!(
                "Attempting to write inverse→declared relationship: declared_name={}",
                declared_name.0
            );
            created_link_count += Self::write_relationship(
                context,
                staged_source,
                &declared_name,
                vec![src_endpoint.clone()],
            )?;

            debug!(
                "Created relationship (inverse→declared): source={}, rel={}, target={}",
                Self::best_identifier_for_dedupe(context, &declared_source_ref),
                declared_name,
                declared_target_identifier,
            );
        }

        Ok(created_link_count)
    }

    // ─────────────────────────────────────────────────────────────────────
    // Low-level helpers
    // ─────────────────────────────────────────────────────────────────────

    /// Read a required string property from a holon reference.
    fn read_string_property(
        context: &dyn HolonsContextBehavior,
        holon: &HolonReference,
        property_name: &PropertyName,
    ) -> Result<MapString, HolonError> {
        match holon.property_value(context, property_name)? {
            Some(BaseValue::StringValue(s)) => Ok(s),
            Some(other) => {
                Err(HolonError::UnexpectedValueType(format!("{:?}", other), "String".into()))
            }
            None => Err(HolonError::EmptyField(property_name.to_string())),
        }
    }

    /// Short diagnostic summary for a LoaderRelationshipReference.
    fn brief_lrr_summary(context: &dyn HolonsContextBehavior, lrr: &TransientReference) -> String {
        let (name, is_decl) = Self::extract_relationship_metadata(context, lrr)
            .unwrap_or_else(|_| (RelationshipName(MapString("<unknown>".into())), false));
        format!("name={}, declared={}", name, is_decl)
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
}
