use derive_new::new;
use std::collections::{HashMap, HashSet};
use std::sync::RwLock;
use std::{fmt, sync::Arc};
use tracing::info;
use type_names::relationship_names::{CoreRelationshipTypeName, ToRelationshipName};

use crate::core_shared_objects::holon::StagedState;
use crate::core_shared_objects::transactions::{
    TransactionContext, TransactionContextHandle, TxId,
};
use crate::descriptors::{
    effective_relationship_declaration, inheritance::described_by_descriptor, RelationshipDirection,
};
use crate::reference_layer::readable_impl::ReadableHolonImpl;
use crate::reference_layer::writable_impl::WritableHolonImpl;
use crate::{
    core_shared_objects::holon::HolonCloneModel,
    reference_layer::{HolonReference, ReadableHolon, TransientReference},
};
// Provides methods for creating/transient holons
use crate::{
    core_shared_objects::{
        holon::{holon_utils::EssentialHolonContent, state::AccessType},
        transient_holon_manager::ToHolonCloneModel,
        Holon, HolonCollection, ReadableHolonState, WriteableHolonState,
    },
    RelationshipMap,
};
use base_types::{BaseValue, MapString};
use core_types::{
    HolonError, HolonId, HolonNodeModel, PropertyMap, PropertyName, PropertyValue,
    RelationshipName, TemporaryId, ValidationError,
};

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
enum DuplicatePolicy {
    Enforced { allows_duplicates: bool },
    Ungoverned,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
struct RelationshipMutationPolicy {
    note_definitional: Option<bool>,
    duplicate_policy: DuplicatePolicy,
}

#[derive(new, Debug, Clone)]
pub struct StagedReference {
    context_handle: TransactionContextHandle,
    id: TemporaryId, // the position of the holon with CommitManager's staged_holons vector
}

/// Capability token allowing nursery access only from this module.
pub(crate) struct StagedRefAccessKey(());

impl StagedRefAccessKey {
    fn new() -> Self {
        Self(())
    }
}

impl StagedReference {
    /// Marks the underlying StagedHolon that is referenced as 'Abandoned'
    ///
    /// Prevents a commit from taking place and restricts Holon to read-only access.
    ///
    /// # Arguments
    /// * `context` - A reference to a `TransactionContext`.
    pub fn abandon_staged_changes(
        &self,
        _context: &Arc<TransactionContext>,
    ) -> Result<(), HolonError> {
        // Get access to the source holon
        let rc_holon = self.get_rc_holon()?;

        // Borrow the holon mutably
        let mut holon_mut = rc_holon.write().map_err(|e| {
            HolonError::FailedToAcquireLock(format!("Failed to acquire write lock on holon: {}", e))
        })?;

        match &mut *holon_mut {
            Holon::Staged(staged_holon) => {
                staged_holon.abandon_staged_changes()?;
            }
            _ => {
                unreachable!()
            }
        }

        Ok(())
    }

    /// Creates a new `StagedReference` from a given TemporaryId without validation.
    ///
    /// # Arguments
    /// * `id` - A TemporaryId
    ///
    /// # Returns
    /// A new `StagedReference` wrapping the provided id.
    pub fn from_temporary_id(context_handle: TransactionContextHandle, id: &TemporaryId) -> Self {
        StagedReference { context_handle, id: id.clone() }
    }

    /// Retrieves the underlying Holon handle for commit operations.
    ///
    /// This method simply delegates to the internal `get_rc_holon()` and exists
    /// so that guest commit functions can obtain the referenced holon for
    /// persistence.  It will later be restricted to the `guest` feature.
    pub fn get_holon_to_commit(
        &self,
        _context: &Arc<TransactionContext>,
    ) -> Result<Arc<RwLock<Holon>>, HolonError> {
        self.get_rc_holon()
    }

    /// Retrieves a shared reference to the holon with interior mutability.
    ///
    /// # Arguments
    /// * `context` - A reference to a `TransactionContext`.
    ///
    /// # Returns
    ///  Result<Arc<RwLock<Holon>>, HolonError>
    ///
    fn get_rc_holon(&self) -> Result<Arc<RwLock<Holon>>, HolonError> {
        // Get NurseryAccess
        let nursery_access =
            self.context_handle.context().nursery_access(StagedRefAccessKey::new());

        // Retrieve the holon by its temporaryId
        let rc_holon = nursery_access.get_holon_by_id(&self.id)?;

        Ok(rc_holon)
    }

    pub fn is_in_state(
        &self,
        _context: &Arc<TransactionContext>,
        check_state: StagedState,
    ) -> Result<bool, HolonError> {
        let rc_holon = self.get_rc_holon()?;

        let holon = rc_holon
            .read()
            .map_err(|e| {
                HolonError::FailedToAcquireLock(format!(
                    "Failed to acquire read lock on nursery: {}",
                    e
                ))
            })?
            .clone();
        let current_state = match holon {
            Holon::Staged(holon) => Ok(holon.get_staged_state()),
            _ => Err(HolonError::InvalidType(
                "StagedReference should point to a StagedHolon".to_string(),
            )),
        }?;
        if current_state == check_state {
            return Ok(true);
        } else {
            Ok(false)
        }
    }

    /// Returns whether this staged holon has been committed.
    ///
    /// Unlike [`Self::is_in_state`], this matches any `StagedState::Committed`
    /// without requiring the exact committed `LocalId`.
    pub fn is_committed(&self) -> Result<bool, HolonError> {
        let rc_holon = self.get_rc_holon()?;

        let holon = rc_holon
            .read()
            .map_err(|e| {
                HolonError::FailedToAcquireLock(format!(
                    "Failed to acquire read lock on nursery: {}",
                    e
                ))
            })?
            .clone();
        match holon {
            Holon::Staged(holon) => {
                Ok(matches!(holon.get_staged_state(), StagedState::Committed(_)))
            }
            _ => Err(HolonError::InvalidType(
                "StagedReference should point to a StagedHolon".to_string(),
            )),
        }
    }

    // ****** Accessors ******

    /// Returns the temporary id for this staged holon.
    pub fn temporary_id(&self) -> TemporaryId {
        self.id.clone()
    }

    /// Returns the transaction id this reference is bound to.
    pub fn tx_id(&self) -> TxId {
        self.context_handle.tx_id()
    }

    // Simple string representations for errors/logging
    pub fn reference_kind_string(&self) -> String {
        "StagedReference".to_string()
    }

    pub fn reference_id_string(&self) -> String {
        format!("TemporaryId={}", self.id)
    }

    fn staged_state(&self) -> Result<StagedState, HolonError> {
        let rc_holon = self.get_rc_holon()?;
        let holon = rc_holon.read().map_err(|e| {
            HolonError::FailedToAcquireLock(format!(
                "Failed to acquire read lock on staged holon: {}",
                e
            ))
        })?;

        match &*holon {
            Holon::Staged(staged_holon) => Ok(staged_holon.get_staged_state()),
            _ => Err(HolonError::InvalidType(
                "StagedReference should point to a StagedHolon".to_string(),
            )),
        }
    }

    fn relationship_mutation_policy(
        &self,
        relationship_name: &RelationshipName,
    ) -> Result<RelationshipMutationPolicy, HolonError> {
        let staged_state = self.staged_state()?;

        let source_ref = HolonReference::Staged(self.clone());
        let relationship_descriptor = match effective_relationship_declaration(
            &source_ref,
            relationship_name.clone(),
        ) {
            Ok(descriptor) => descriptor,
            Err(
                original_error @ HolonError::DescriptorDeclarationNotFound {
                    kind: _,
                    name: _,
                    descriptor: _,
                },
            ) => {
                let source_descriptor = described_by_descriptor(&source_ref)?;
                if staged_state == StagedState::ForCreate && source_descriptor.is_none() {
                    return Ok(RelationshipMutationPolicy {
                        note_definitional: None,
                        duplicate_policy: DuplicatePolicy::Ungoverned,
                    });
                }

                if source_descriptor.is_some() {
                    match source_ref
                        .holon_descriptor()?
                        .allows_relationship(relationship_name.clone())
                    {
                        Ok(qualified)
                            if qualified.descriptor_direction == RelationshipDirection::Inverse =>
                        {
                            return Err(HolonError::ValidationError(
                                    ValidationError::RelationshipError(format!(
                                        "Relationship '{}' is an inverse relationship and cannot be staged as ordinary mutation input. Stage the declared relationship from the declared source endpoint instead.",
                                        relationship_name
                                    )),
                                ));
                        }
                        Ok(_) => return Err(original_error),
                        Err(_) => return Err(original_error),
                    }
                }

                return Err(original_error);
            }
            Err(err) => return Err(err),
        };

        let duplicate_policy = DuplicatePolicy::Enforced {
            allows_duplicates: relationship_descriptor.allows_duplicates()?,
        };
        let note_definitional = if staged_state == StagedState::ForCreate {
            None
        } else {
            Some(relationship_descriptor.is_definitional()?)
        };

        Ok(RelationshipMutationPolicy { note_definitional, duplicate_policy })
    }

    fn classify_relationship_removal(
        &self,
        relationship_name: &RelationshipName,
    ) -> Result<Option<bool>, HolonError> {
        if self.staged_state()? == StagedState::ForCreate {
            return Ok(None);
        }

        let source_ref = HolonReference::Staged(self.clone());
        let relationship_descriptor =
            effective_relationship_declaration(&source_ref, relationship_name.clone())?;

        Ok(Some(relationship_descriptor.is_definitional()?))
    }

    /// Filters add entries for a duplicate-disallowed relationship against the staged
    /// collection: exact repeats (same reference identity) are dropped as idempotent
    /// no-ops, while a same-key entry with a different identity fails with
    /// `DuplicateError`.
    ///
    /// This check sees only the staged collection, so it is authoritative for
    /// `ForCreate` sources but best-effort for `ForUpdate*` sources, whose prior
    /// relationships live in the persisted graph. Commit-time SmartLink suppression
    /// is the authoritative duplicate guard against persisted links (issue #516).
    fn filter_duplicate_disallowed_entries(
        &self,
        relationship_name: &RelationshipName,
        entries: Vec<(HolonReference, Option<MapString>)>,
    ) -> Result<Vec<(HolonReference, Option<MapString>)>, HolonError> {
        let collection_arc = self.related_holons(relationship_name)?;
        let collection = collection_arc.read().map_err(|e| {
            HolonError::FailedToAcquireLock(format!(
                "Failed to acquire read lock on holon collection: {}",
                e
            ))
        })?;

        let mut seen_reference_ids = HashSet::new();
        let mut key_to_reference_id = HashMap::new();
        for member in collection.get_members() {
            let reference_id = member.reference_id_string();
            seen_reference_ids.insert(reference_id.clone());
            if let Some(key) = member.key()? {
                key_to_reference_id.entry(key).or_insert(reference_id);
            }
        }
        drop(collection);

        let mut filtered = Vec::new();
        for (reference, key) in entries {
            let reference_id = reference.reference_id_string();
            if seen_reference_ids.contains(&reference_id) {
                continue;
            }

            if let Some(key) = &key {
                if let Some(existing_id) = key_to_reference_id.get(key) {
                    if existing_id != &reference_id {
                        return Err(HolonError::DuplicateError(
                            relationship_name.to_string(),
                            format!(
                                "Duplicate target key '{}' resolves to non-identical references: existing {}, requested {}",
                                key.0, existing_id, reference_id
                            ),
                        ));
                    }
                }
                key_to_reference_id.insert(key.clone(), reference_id.clone());
            }

            seen_reference_ids.insert(reference_id);
            filtered.push((reference, key));
        }

        Ok(filtered)
    }

    fn related_holons_with_keys(
        holons: Vec<HolonReference>,
    ) -> Result<Vec<(HolonReference, Option<MapString>)>, HolonError> {
        holons
            .into_iter()
            .map(|h| {
                let key = h.key()?;
                Ok((h, key))
            })
            .collect()
    }

    fn add_related_holons_with_classification(
        &self,
        relationship_name: RelationshipName,
        entries: Vec<(HolonReference, Option<MapString>)>,
        policy: RelationshipMutationPolicy,
    ) -> Result<(), HolonError> {
        let entries = match policy.duplicate_policy {
            DuplicatePolicy::Enforced { allows_duplicates: false } => {
                self.filter_duplicate_disallowed_entries(&relationship_name, entries)?
            }
            DuplicatePolicy::Enforced { allows_duplicates: true } | DuplicatePolicy::Ungoverned => {
                entries
            }
        };

        if entries.is_empty() {
            return Ok(());
        }

        let rc_holon = self.get_rc_holon()?;
        let mut holon_mut = rc_holon.write().map_err(|e| {
            HolonError::FailedToAcquireLock(format!(
                "Failed to acquire write lock on staged holon: {}",
                e
            ))
        })?;

        match &mut *holon_mut {
            Holon::Staged(staged_holon) => {
                staged_holon.add_related_holons_with_keys(relationship_name, entries)?;
                if let Some(is_definitional) = policy.note_definitional {
                    staged_holon.note_relationship_mutation(is_definitional)?;
                }
            }
            _ => {
                return Err(HolonError::InvalidType(
                    "StagedReference should point to a StagedHolon".to_string(),
                ))
            }
        }

        Ok(())
    }

    fn remove_related_holons_with_classification(
        &self,
        relationship_name: &RelationshipName,
        entries: Vec<(HolonReference, Option<MapString>)>,
        is_definitional: Option<bool>,
    ) -> Result<(), HolonError> {
        let rc_holon = self.get_rc_holon()?;
        let mut holon_mut = rc_holon.write().map_err(|e| {
            HolonError::FailedToAcquireLock(format!(
                "Failed to acquire write lock on staged holon: {}",
                e
            ))
        })?;

        match &mut *holon_mut {
            Holon::Staged(staged_holon) => {
                staged_holon.remove_related_holons_with_keys(relationship_name, entries)?;
                if let Some(is_definitional) = is_definitional {
                    staged_holon.note_relationship_mutation(is_definitional)?;
                }
            }
            _ => {
                return Err(HolonError::InvalidType(
                    "StagedReference should point to a StagedHolon".to_string(),
                ))
            }
        }

        Ok(())
    }

    fn add_related_holons_without_classification(
        &self,
        relationship_name: RelationshipName,
        holons: Vec<HolonReference>,
    ) -> Result<(), HolonError> {
        let holons_with_keys = Self::related_holons_with_keys(holons)?;
        self.add_related_holons_with_classification(
            relationship_name,
            holons_with_keys,
            RelationshipMutationPolicy {
                note_definitional: None,
                duplicate_policy: DuplicatePolicy::Ungoverned,
            },
        )
    }

    /// Adds relationship targets without descriptor-policy classification.
    ///
    /// This is a narrowly scoped escape hatch for descriptor-graph construction
    /// paths, such as loader Pass-2 bootstrap writes, where the caller already
    /// owns relationship validation and deduplication but the self-describing
    /// relationship declarations may not exist yet. Ordinary runtime mutation
    /// must use [`crate::reference_layer::WritableHolon::add_related_holons`]
    /// so #516 duplicate policy, metadata failures, and inverse-name rejection
    /// are enforced.
    pub fn add_related_holons_ungoverned<T: ToRelationshipName>(
        &mut self,
        relationship_name: T,
        holons: Vec<HolonReference>,
    ) -> Result<&mut Self, HolonError> {
        self.add_related_holons_without_classification(
            relationship_name.to_relationship_name(),
            holons,
        )?;
        Ok(self)
    }

    fn remove_related_holons_without_classification(
        &self,
        relationship_name: &RelationshipName,
        holons: Vec<HolonReference>,
    ) -> Result<(), HolonError> {
        let holons_with_keys = Self::related_holons_with_keys(holons)?;
        self.remove_related_holons_with_classification(relationship_name, holons_with_keys, None)
    }
}

impl fmt::Display for StagedReference {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "StagedReference({})", self.reference_id_string())
    }
}

impl ReadableHolonImpl for StagedReference {
    fn clone_holon_impl(&self) -> Result<TransientReference, HolonError> {
        self.is_accessible(AccessType::Clone)?;
        let rc_holon = self.get_rc_holon()?;
        let holon_clone_model = rc_holon
            .read()
            .map_err(|e| {
                HolonError::FailedToAcquireLock(format!(
                    "Failed to acquire read lock on staged holon: {}",
                    e
                ))
            })?
            .holon_clone_model();

        let cloned_holon_transient_reference =
            self.context_handle.context().new_transient_from_clone_model(holon_clone_model)?;

        Ok(cloned_holon_transient_reference)
    }

    fn all_related_holons_impl(&self) -> Result<RelationshipMap, HolonError> {
        self.is_accessible(AccessType::Read)?;
        let rc_holon = self.get_rc_holon()?;
        let borrowed_holon = rc_holon.read().map_err(|e| {
            HolonError::FailedToAcquireLock(format!(
                "Failed to acquire read lock on staged holon: {}",
                e
            ))
        })?;

        borrowed_holon.all_related_holons()
    }

    fn holon_id_impl(&self) -> Result<HolonId, HolonError> {
        self.is_accessible(AccessType::Read)?;
        let rc_holon = self.get_rc_holon()?;
        let borrowed_holon = rc_holon.read().map_err(|e| {
            HolonError::FailedToAcquireLock(format!(
                "Failed to acquire read lock on staged holon: {}",
                e
            ))
        })?;

        borrowed_holon.holon_id()
    }

    fn predecessor_impl(&self) -> Result<Option<HolonReference>, HolonError> {
        self.is_accessible(AccessType::Read)?;
        let collection_arc = self.related_holons(CoreRelationshipTypeName::Predecessor)?;
        let collection = collection_arc.read().map_err(|e| {
            HolonError::FailedToAcquireLock(format!(
                "Failed to acquire read lock on holon collection: {}",
                e
            ))
        })?;
        collection.is_accessible(AccessType::Read)?;
        let members = collection.get_members();
        if members.len() > 1 {
            return Err(HolonError::Misc(format!(
                "related_holons for PREDECESSOR returned multiple members: {:#?}",
                members
            )));
        }
        if members.is_empty() {
            Ok(None)
        } else {
            Ok(Some(members[0].clone()))
        }
    }

    fn property_value_impl(
        &self,
        property_name: &PropertyName,
    ) -> Result<Option<PropertyValue>, HolonError> {
        self.is_accessible(AccessType::Read)?;
        let rc_holon = self.get_rc_holon()?;
        let borrowed_holon = rc_holon.read().map_err(|e| {
            HolonError::FailedToAcquireLock(format!(
                "Failed to acquire read lock on staged holon: {}",
                e
            ))
        })?;

        borrowed_holon.property_value(property_name)
    }

    fn key_impl(&self) -> Result<Option<MapString>, HolonError> {
        self.is_accessible(AccessType::Read)?;
        let rc_holon = self.get_rc_holon()?;
        let borrowed_holon = rc_holon.read().map_err(|e| {
            HolonError::FailedToAcquireLock(format!(
                "Failed to acquire read lock on staged holon: {}",
                e
            ))
        })?;

        Ok(borrowed_holon.key())
    }

    fn related_holons_impl(
        &self,
        relationship_name: &RelationshipName,
    ) -> Result<Arc<RwLock<HolonCollection>>, HolonError> {
        self.is_accessible(AccessType::Read)?;
        let rc_holon = self.get_rc_holon()?;
        let holon = rc_holon.read().map_err(|e| {
            HolonError::FailedToAcquireLock(format!(
                "Failed to acquire read lock on staged holon: {}",
                e
            ))
        })?;

        holon.related_holons(relationship_name)
    }

    fn versioned_key_impl(&self) -> Result<MapString, HolonError> {
        self.is_accessible(AccessType::Read)?;
        let holon = self.get_rc_holon()?;
        let key = holon
            .read()
            .map_err(|e| {
                HolonError::FailedToAcquireLock(format!(
                    "Failed to acquire read lock on staged holon: {}",
                    e
                ))
            })?
            .versioned_key()?;

        Ok(key)
    }

    fn property_map_impl(&self) -> Result<PropertyMap, HolonError> {
        self.is_accessible(AccessType::Read)?;
        let rc_holon = self.get_rc_holon()?;
        let borrowed_holon = rc_holon.read().map_err(|e| {
            HolonError::FailedToAcquireLock(format!(
                "Failed to acquire read lock on staged holon: {}",
                e
            ))
        })?;

        Ok(borrowed_holon.property_map_clone())
    }

    fn essential_content_impl(&self) -> Result<EssentialHolonContent, HolonError> {
        self.is_accessible(AccessType::Read)?;
        let rc_holon = self.get_rc_holon()?;
        let borrowed_holon = rc_holon.read().map_err(|e| {
            HolonError::FailedToAcquireLock(format!(
                "Failed to acquire read lock on staged holon: {}",
                e
            ))
        })?;

        Ok(borrowed_holon.essential_content())
    }

    fn summarize_impl(&self) -> Result<String, HolonError> {
        self.is_accessible(AccessType::Read)?;
        let rc_holon = self.get_rc_holon()?;
        let borrowed_holon = rc_holon.read().map_err(|e| {
            HolonError::FailedToAcquireLock(format!(
                "Failed to acquire read lock on staged holon: {}",
                e
            ))
        })?;
        Ok(borrowed_holon.summarize())
    }

    fn into_model_impl(&self) -> Result<HolonNodeModel, HolonError> {
        self.is_accessible(AccessType::Read)?;
        let rc_holon = self.get_rc_holon()?;
        let borrowed_holon = rc_holon.read().map_err(|e| {
            HolonError::FailedToAcquireLock(format!(
                "Failed to acquire read lock on staged holon: {}",
                e
            ))
        })?;

        Ok(borrowed_holon.into_node_model())
    }

    fn is_accessible_impl(&self, access_type: AccessType) -> Result<(), HolonError> {
        let rc_holon = self.get_rc_holon()?;
        let holon = rc_holon.read().map_err(|e| {
            HolonError::FailedToAcquireLock(format!(
                "Failed to acquire read lock on staged holon: {}",
                e
            ))
        })?;
        holon.is_accessible(access_type)?;

        Ok(())
    }

    fn is_committed_source_impl(&self) -> Result<bool, HolonError> {
        self.is_committed()
    }

    fn holon_reference_impl(&self) -> HolonReference {
        self.into()
    }
}

impl WritableHolonImpl for StagedReference {
    fn add_related_holons_impl(
        &mut self,
        relationship_name: RelationshipName,
        holons: Vec<HolonReference>,
    ) -> Result<&mut Self, HolonError> {
        self.is_accessible(AccessType::Write)?;
        let policy = self.relationship_mutation_policy(&relationship_name)?;
        // Precompute keys before taking the holon write lock to avoid re-entrant locking on self-edges.
        let holons_with_keys = Self::related_holons_with_keys(holons)?;

        self.add_related_holons_with_classification(relationship_name, holons_with_keys, policy)?;

        Ok(self)
    }

    fn remove_related_holons_impl(
        &mut self,
        relationship_name: RelationshipName,
        holons: Vec<HolonReference>,
    ) -> Result<&mut Self, HolonError> {
        self.is_accessible(AccessType::Write)?;
        let is_definitional = self.classify_relationship_removal(&relationship_name)?;
        let holons_with_keys = Self::related_holons_with_keys(holons)?;
        info!(
            "Removing {:?} related holons from relationship: {:?}",
            holons_with_keys.len(),
            relationship_name
        );
        self.remove_related_holons_with_classification(
            &relationship_name,
            holons_with_keys,
            is_definitional,
        )?;

        Ok(self)
    }

    fn with_property_value_impl(
        &mut self,
        property: PropertyName,
        value: BaseValue,
    ) -> Result<&mut Self, HolonError> {
        self.is_accessible(AccessType::Write)?;
        let rc_holon = self.get_rc_holon()?;
        let mut holon_mut = rc_holon.write().map_err(|e| {
            HolonError::FailedToAcquireLock(format!(
                "Failed to acquire write lock on staged holon: {}",
                e
            ))
        })?;

        holon_mut.with_property_value(property, value)?;

        Ok(self)
    }

    fn remove_property_value_impl(&mut self, name: PropertyName) -> Result<&mut Self, HolonError> {
        self.is_accessible(AccessType::Write)?;
        let rc_holon = self.get_rc_holon()?;
        let mut holon_mut = rc_holon.write().map_err(|e| {
            HolonError::FailedToAcquireLock(format!(
                "Failed to acquire write lock on staged holon: {}",
                e
            ))
        })?;
        holon_mut.remove_property_value(&name)?;

        Ok(self)
    }

    fn with_descriptor_impl(
        &mut self,
        descriptor_reference: HolonReference,
    ) -> Result<(), HolonError> {
        self.is_accessible(AccessType::Write)?;

        // Look up the existing descriptor(s) on THIS holon, not on the descriptor.
        let self_ref = HolonReference::Staged(self.clone());
        let existing_descriptor_option = self_ref.get_descriptor()?;

        if let Some(existing_descriptor) = existing_descriptor_option {
            // Remove the current descriptor edge
            self.remove_related_holons_without_classification(
                &CoreRelationshipTypeName::DescribedBy.as_relationship_name(),
                vec![existing_descriptor.clone()],
            )?;
        }

        // Attach the new descriptor edge
        self.add_related_holons_without_classification(
            CoreRelationshipTypeName::DescribedBy.as_relationship_name(),
            vec![descriptor_reference],
        )?;

        Ok(())
    }

    fn with_predecessor_impl(
        &mut self,
        predecessor_reference_option: Option<HolonReference>, // None passed just removes predecessor
    ) -> Result<(), HolonError> {
        self.is_accessible(AccessType::Write)?;
        let existing_predecessor_option = self.clone().predecessor()?;
        if let Some(predecessor) = existing_predecessor_option {
            self.remove_related_holons_without_classification(
                &CoreRelationshipTypeName::Predecessor.as_relationship_name(),
                vec![predecessor.clone()],
            )?;
        }
        if let Some(predecessor_reference) = predecessor_reference_option {
            self.add_related_holons_without_classification(
                CoreRelationshipTypeName::Predecessor.as_relationship_name(),
                vec![predecessor_reference.clone()],
            )?;
        }

        Ok(())
    }
}

impl ToHolonCloneModel for StagedReference {
    fn holon_clone_model(&self) -> Result<HolonCloneModel, HolonError> {
        self.is_accessible(AccessType::Read)?;
        let rc_holon = self.get_rc_holon()?;
        let holon_clone_model = rc_holon
            .read()
            .map_err(|e| {
                HolonError::FailedToAcquireLock(format!(
                    "Failed to acquire read lock on staged holon: {}",
                    e
                ))
            })?
            .holon_clone_model();

        Ok(holon_clone_model)
    }
}

// ---------- StagedReference equality ----------
//
// Staged holons are transaction-scoped, so the combination of (tx_id, TemporaryId)
// is the stable identity.
impl PartialEq for StagedReference {
    fn eq(&self, other: &Self) -> bool {
        self.context_handle.tx_id() == other.context_handle.tx_id() && self.id == other.id
    }
}

impl Eq for StagedReference {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        core_shared_objects::StagedHolon,
        descriptors::test_support::{
            build_context, core_holon_type_name, new_descriptor_holon, new_holon_type_descriptor,
            new_relationship_descriptor_holon, new_test_holon,
        },
        reference_layer::WritableHolon,
    };
    use core_types::LocalId;
    use type_names::{CoreHolonTypeName, CorePropertyTypeName};

    fn force_staged_reference_for_update(
        context: &Arc<TransactionContext>,
        staged_reference: &StagedReference,
    ) -> Result<(), HolonError> {
        let rc_holon = staged_reference.get_holon_to_commit(context)?;
        let clone_model = {
            let holon = rc_holon.read().map_err(|e| {
                HolonError::FailedToAcquireLock(format!(
                    "Failed to acquire read lock on staged holon: {}",
                    e
                ))
            })?;
            holon.holon_clone_model()
        };
        let update_holon =
            StagedHolon::new_for_update_from_clone_model(clone_model, LocalId(vec![1, 2, 3]))?;

        let mut holon = rc_holon.write().map_err(|e| {
            HolonError::FailedToAcquireLock(format!(
                "Failed to acquire write lock on staged holon: {}",
                e
            ))
        })?;
        *holon = Holon::Staged(update_holon);

        Ok(())
    }

    fn staged_relationship_descriptor(
        context: &Arc<TransactionContext>,
        relationship_name: &str,
        is_definitional: Option<bool>,
    ) -> Result<(StagedReference, StagedReference), HolonError> {
        let source_type = new_holon_type_descriptor(context, "source-type", "SourceType")?;
        let target_type = new_holon_type_descriptor(context, "target-type", "TargetType")?;
        let staged_source_type = context.mutation().stage_new_holon(source_type)?;
        let staged_target_type = context.mutation().stage_new_holon(target_type)?;

        let relationship_descriptor = if let Some(is_definitional) = is_definitional {
            let mut relationship_descriptor = new_relationship_descriptor_holon(
                context,
                "relationship-descriptor",
                relationship_name,
                staged_source_type.clone().into(),
                staged_target_type.clone().into(),
            )?;
            relationship_descriptor
                .with_property_value(CorePropertyTypeName::IsDefinitional, is_definitional)?;
            relationship_descriptor
        } else {
            new_descriptor_holon(
                context,
                "relationship-descriptor",
                relationship_name,
                "Relationship",
            )?
        };
        let staged_relationship_descriptor =
            context.mutation().stage_new_holon(relationship_descriptor)?;

        let mut staged_source_type = staged_source_type;
        staged_source_type.add_related_holons(
            CoreRelationshipTypeName::InstanceRelationships,
            vec![staged_relationship_descriptor.into()],
        )?;

        Ok((staged_source_type, staged_target_type))
    }

    fn staged_relationship_descriptor_with_invalid_is_definitional(
        context: &Arc<TransactionContext>,
        relationship_name: &str,
    ) -> Result<(StagedReference, StagedReference), HolonError> {
        let source_type = new_holon_type_descriptor(context, "source-type", "SourceType")?;
        let target_type = new_holon_type_descriptor(context, "target-type", "TargetType")?;
        let staged_source_type = context.mutation().stage_new_holon(source_type)?;
        let staged_target_type = context.mutation().stage_new_holon(target_type)?;

        let mut relationship_descriptor = new_relationship_descriptor_holon(
            context,
            "relationship-descriptor",
            relationship_name,
            staged_source_type.clone().into(),
            staged_target_type.clone().into(),
        )?;
        relationship_descriptor
            .with_property_value(CorePropertyTypeName::IsDefinitional, "not-a-boolean")?;
        let staged_relationship_descriptor =
            context.mutation().stage_new_holon(relationship_descriptor)?;

        let mut staged_source_type = staged_source_type;
        staged_source_type.add_related_holons(
            CoreRelationshipTypeName::InstanceRelationships,
            vec![staged_relationship_descriptor.into()],
        )?;

        Ok((staged_source_type, staged_target_type))
    }

    fn staged_relationship_descriptor_with_metadata(
        context: &Arc<TransactionContext>,
        relationship_name: &str,
        is_definitional: Option<BaseValue>,
        allows_duplicates: Option<BaseValue>,
    ) -> Result<(StagedReference, StagedReference), HolonError> {
        let source_type = new_holon_type_descriptor(context, "source-type", "SourceType")?;
        let target_type = new_holon_type_descriptor(context, "target-type", "TargetType")?;
        let staged_source_type = context.mutation().stage_new_holon(source_type)?;
        let staged_target_type = context.mutation().stage_new_holon(target_type)?;

        let mut relationship_descriptor = new_descriptor_holon(
            context,
            "relationship-descriptor",
            relationship_name,
            "Relationship",
        )?;
        if let Some(is_definitional) = is_definitional {
            relationship_descriptor.with_property_value_impl(
                CorePropertyTypeName::IsDefinitional.as_property_name(),
                is_definitional,
            )?;
        }
        if let Some(allows_duplicates) = allows_duplicates {
            relationship_descriptor.with_property_value_impl(
                CorePropertyTypeName::AllowsDuplicates.as_property_name(),
                allows_duplicates,
            )?;
        }
        let staged_relationship_descriptor =
            context.mutation().stage_new_holon(relationship_descriptor)?;

        let mut staged_source_type = staged_source_type;
        staged_source_type.add_related_holons(
            CoreRelationshipTypeName::InstanceRelationships,
            vec![staged_relationship_descriptor.into()],
        )?;

        Ok((staged_source_type, staged_target_type))
    }

    struct RelationshipPairFixture {
        target_type: StagedReference,
    }

    fn relationship_pair_fixture(
        context: &Arc<TransactionContext>,
    ) -> Result<RelationshipPairFixture, HolonError> {
        let declared_type = context.mutation().stage_new_holon(new_descriptor_holon(
            context,
            "declared-relationship-type",
            &core_holon_type_name(CoreHolonTypeName::DeclaredRelationshipType),
            "Relationship",
        )?)?;
        let inverse_type = context.mutation().stage_new_holon(new_descriptor_holon(
            context,
            "inverse-relationship-type",
            &core_holon_type_name(CoreHolonTypeName::InverseRelationshipType),
            "Relationship",
        )?)?;
        let mut source_type = context.mutation().stage_new_holon(new_holon_type_descriptor(
            context,
            "book-type",
            "BookType",
        )?)?;
        let mut target_type = context.mutation().stage_new_holon(new_holon_type_descriptor(
            context,
            "person-type",
            "PersonType",
        )?)?;
        let mut declared =
            context.mutation().stage_new_holon(new_relationship_descriptor_holon(
                context,
                "authored-by",
                "AuthoredBy",
                HolonReference::from(&source_type),
                HolonReference::from(&target_type),
            )?)?;
        let mut inverse = context.mutation().stage_new_holon(new_relationship_descriptor_holon(
            context,
            "authors",
            "Authors",
            HolonReference::from(&target_type),
            HolonReference::from(&source_type),
        )?)?;

        declared
            .add_related_holons(CoreRelationshipTypeName::Extends, vec![declared_type.into()])?;
        inverse.add_related_holons(CoreRelationshipTypeName::Extends, vec![inverse_type.into()])?;
        declared
            .add_related_holons(CoreRelationshipTypeName::HasInverse, vec![(&inverse).into()])?;
        inverse
            .add_related_holons(CoreRelationshipTypeName::InverseOf, vec![(&declared).into()])?;
        source_type.add_related_holons(
            CoreRelationshipTypeName::InstanceRelationships,
            vec![(&declared).into()],
        )?;
        target_type
            .add_related_holons(CoreRelationshipTypeName::TargetOf, vec![(&declared).into()])?;

        Ok(RelationshipPairFixture { target_type })
    }

    fn staged_update_source(
        context: &Arc<TransactionContext>,
        source_descriptor: StagedReference,
    ) -> Result<StagedReference, HolonError> {
        let source = new_test_holon(context, "source-instance")?;
        let mut staged_source = context.mutation().stage_new_holon(source)?;
        staged_source.add_related_holons(
            CoreRelationshipTypeName::DescribedBy,
            vec![source_descriptor.into()],
        )?;
        force_staged_reference_for_update(context, &staged_source)?;
        Ok(staged_source)
    }

    fn staged_target(
        context: &Arc<TransactionContext>,
        key: &str,
    ) -> Result<StagedReference, HolonError> {
        context.mutation().stage_new_holon(new_test_holon(context, key)?)
    }

    fn relationship_member_count(
        source: &StagedReference,
        relationship_name: &str,
    ) -> Result<usize, HolonError> {
        let collection = source.related_holons(relationship_name)?;
        let count = collection.read().unwrap().get_members().len();
        Ok(count)
    }

    #[test]
    fn is_committed_matches_any_committed_local_id() -> Result<(), HolonError> {
        let context = build_context();
        let staged = context.mutation().stage_new_holon(new_test_holon(&context, "commit-me")?)?;

        assert!(!staged.is_committed()?);

        let rc_holon = staged.get_holon_to_commit(&context)?;
        {
            let mut holon = rc_holon.write().map_err(|e| {
                HolonError::FailedToAcquireLock(format!(
                    "Failed to acquire write lock on staged holon: {}",
                    e
                ))
            })?;
            match &mut *holon {
                Holon::Staged(staged_holon) => staged_holon.to_committed(LocalId(vec![4, 5, 6]))?,
                _ => unreachable!("stage_new_holon produces a StagedHolon"),
            }
        }

        assert!(staged.is_committed()?);
        assert!(staged.is_in_state(&context, StagedState::Committed(LocalId(vec![4, 5, 6])))?);

        Ok(())
    }

    #[test]
    fn undescribed_for_create_relationship_mutation_keeps_legacy_append() -> Result<(), HolonError>
    {
        let context = build_context();
        let source = new_test_holon(&context, "new-source")?;
        let mut staged_source = context.mutation().stage_new_holon(source)?;
        let target = staged_target(&context, "new-target")?;

        staged_source.add_related_holons("UndeclaredRelationship", vec![target.into()])?;

        assert!(staged_source.is_in_state(&context, StagedState::ForCreate)?);
        Ok(())
    }

    #[test]
    fn described_for_create_unknown_relationship_errors_before_mutation() -> Result<(), HolonError>
    {
        let context = build_context();
        let source_descriptor = context.mutation().stage_new_holon(new_holon_type_descriptor(
            &context,
            "source-type",
            "SourceType",
        )?)?;
        let source = new_test_holon(&context, "new-source")?;
        let mut staged_source = context.mutation().stage_new_holon(source)?;
        staged_source.with_descriptor(source_descriptor.into())?;
        let target = staged_target(&context, "new-target")?;

        let result =
            staged_source.add_related_holons("UndeclaredRelationship", vec![target.into()]);

        assert!(matches!(
            result,
            Err(HolonError::DescriptorDeclarationNotFound { kind, name, .. })
                if kind == "relationship" && name == "UndeclaredRelationship"
        ));
        assert_eq!(relationship_member_count(&staged_source, "UndeclaredRelationship")?, 0);
        Ok(())
    }

    #[test]
    fn ungoverned_add_appends_described_for_create_unresolved_relationship(
    ) -> Result<(), HolonError> {
        let context = build_context();
        let source_descriptor = context.mutation().stage_new_holon(new_holon_type_descriptor(
            &context,
            "source-type",
            "SourceType",
        )?)?;
        let source = new_test_holon(&context, "new-source")?;
        let mut staged_source = context.mutation().stage_new_holon(source)?;
        staged_source.with_descriptor(source_descriptor.into())?;
        let target = staged_target(&context, "new-target")?;

        let ordinary_result =
            staged_source.add_related_holons("UndeclaredRelationship", vec![target.clone().into()]);
        assert!(matches!(
            ordinary_result,
            Err(HolonError::DescriptorDeclarationNotFound { kind, name, .. })
                if kind == "relationship" && name == "UndeclaredRelationship"
        ));
        assert_eq!(relationship_member_count(&staged_source, "UndeclaredRelationship")?, 0);

        staged_source
            .add_related_holons_ungoverned("UndeclaredRelationship", vec![target.into()])?;

        assert_eq!(relationship_member_count(&staged_source, "UndeclaredRelationship")?, 1);
        assert!(staged_source.is_in_state(&context, StagedState::ForCreate)?);
        Ok(())
    }

    #[test]
    fn for_create_remove_skips_descriptor_classification_after_descriptor_added(
    ) -> Result<(), HolonError> {
        let context = build_context();
        let source_descriptor = context.mutation().stage_new_holon(new_holon_type_descriptor(
            &context,
            "source-type",
            "SourceType",
        )?)?;
        let source = new_test_holon(&context, "new-source")?;
        let mut staged_source = context.mutation().stage_new_holon(source)?;
        let target = staged_target(&context, "new-target")?;

        staged_source.add_related_holons("UndeclaredRelationship", vec![target.clone().into()])?;
        staged_source.with_descriptor(source_descriptor.into())?;
        assert_eq!(relationship_member_count(&staged_source, "UndeclaredRelationship")?, 1);

        staged_source.remove_related_holons("UndeclaredRelationship", vec![target.into()])?;

        assert_eq!(relationship_member_count(&staged_source, "UndeclaredRelationship")?, 0);
        assert!(staged_source.is_in_state(&context, StagedState::ForCreate)?);
        Ok(())
    }

    #[test]
    fn duplicate_disallowed_repeated_add_is_idempotent_no_op() -> Result<(), HolonError> {
        let context = build_context();
        let (source_descriptor, _) =
            staged_relationship_descriptor(&context, "AuthoredBy", Some(false))?;
        let mut staged_source = staged_update_source(&context, source_descriptor)?;
        let target = staged_target(&context, "author")?;

        staged_source.add_related_holons("AuthoredBy", vec![target.clone().into()])?;
        assert_eq!(relationship_member_count(&staged_source, "AuthoredBy")?, 1);
        force_staged_reference_for_update(&context, &staged_source)?;

        staged_source.add_related_holons("AuthoredBy", vec![target.into()])?;

        assert_eq!(relationship_member_count(&staged_source, "AuthoredBy")?, 1);
        assert!(staged_source.is_in_state(&context, StagedState::ForUpdate)?);
        Ok(())
    }

    /// Pins the issue #516 §4.6 authority matrix for `ForCreate`: all of a new
    /// holon's relationships are staged, so the mutation-time duplicate check is
    /// authoritative there — the exact repeat is dropped without descriptor
    /// escalation and the source stays `ForCreate`.
    #[test]
    fn duplicate_disallowed_repeated_add_is_idempotent_for_described_for_create_source(
    ) -> Result<(), HolonError> {
        let context = build_context();
        let (source_descriptor, _) =
            staged_relationship_descriptor(&context, "AuthoredBy", Some(false))?;
        let source = new_test_holon(&context, "source-instance")?;
        let mut staged_source = context.mutation().stage_new_holon(source)?;
        staged_source.add_related_holons(
            CoreRelationshipTypeName::DescribedBy,
            vec![source_descriptor.into()],
        )?;
        let target = staged_target(&context, "author")?;

        staged_source.add_related_holons("AuthoredBy", vec![target.clone().into()])?;
        staged_source.add_related_holons("AuthoredBy", vec![target.into()])?;

        assert_eq!(relationship_member_count(&staged_source, "AuthoredBy")?, 1);
        assert!(staged_source.is_in_state(&context, StagedState::ForCreate)?);
        Ok(())
    }

    /// A duplicate declared base name anywhere in the source's effective lineage is
    /// an eager local schema defect; it must surface through ordinary mutation
    /// validation rather than being swallowed or treated as a permissive fallback.
    #[test]
    fn duplicate_inherited_declaration_surfaces_through_mutation() -> Result<(), HolonError> {
        let context = build_context();
        let source_type = new_holon_type_descriptor(&context, "source-type", "SourceType")?;
        let target_type = new_holon_type_descriptor(&context, "target-type", "TargetType")?;
        let staged_source_type = context.mutation().stage_new_holon(source_type)?;
        let staged_target_type = context.mutation().stage_new_holon(target_type)?;

        let first_declaration = new_relationship_descriptor_holon(
            &context,
            "relationship-descriptor-a",
            "AuthoredBy",
            staged_source_type.clone().into(),
            staged_target_type.clone().into(),
        )?;
        let second_declaration = new_relationship_descriptor_holon(
            &context,
            "relationship-descriptor-b",
            "AuthoredBy",
            staged_source_type.clone().into(),
            staged_target_type.into(),
        )?;
        let staged_first = context.mutation().stage_new_holon(first_declaration)?;
        let staged_second = context.mutation().stage_new_holon(second_declaration)?;

        let mut staged_source_type = staged_source_type;
        staged_source_type.add_related_holons(
            CoreRelationshipTypeName::InstanceRelationships,
            vec![staged_first.into(), staged_second.into()],
        )?;

        let mut staged_source = staged_update_source(&context, staged_source_type)?;
        let target = staged_target(&context, "author")?;

        let result = staged_source.add_related_holons("AuthoredBy", vec![target.into()]);

        assert!(matches!(
            result,
            Err(HolonError::DuplicateInheritedDeclaration { kind, name, .. })
                if kind == "relationship" && name == "AuthoredBy"
        ));
        assert_eq!(relationship_member_count(&staged_source, "AuthoredBy")?, 0);
        Ok(())
    }

    #[test]
    fn duplicate_disallowed_same_key_different_reference_errors() -> Result<(), HolonError> {
        let context = build_context();
        let (source_descriptor, _) =
            staged_relationship_descriptor(&context, "AuthoredBy", Some(false))?;
        let mut staged_source = staged_update_source(&context, source_descriptor)?;
        let first_target = staged_target(&context, "author")?;
        let second_target = staged_target(&context, "author")?;

        staged_source.add_related_holons("AuthoredBy", vec![first_target.into()])?;
        force_staged_reference_for_update(&context, &staged_source)?;
        let result = staged_source.add_related_holons("AuthoredBy", vec![second_target.into()]);

        assert!(matches!(
            result,
            Err(HolonError::DuplicateError(relationship, detail))
                if relationship == "AuthoredBy"
                    && detail.contains("Duplicate target key 'author'")
                    && detail.contains("non-identical references")
        ));
        assert_eq!(relationship_member_count(&staged_source, "AuthoredBy")?, 1);
        assert!(staged_source.is_in_state(&context, StagedState::ForUpdate)?);
        Ok(())
    }

    #[test]
    fn duplicate_allowed_relationship_preserves_repeated_targets() -> Result<(), HolonError> {
        let context = build_context();
        let (source_descriptor, _) = staged_relationship_descriptor_with_metadata(
            &context,
            "AuthoredBy",
            Some(BaseValue::BooleanValue(true.into())),
            Some(BaseValue::BooleanValue(true.into())),
        )?;
        let mut staged_source = staged_update_source(&context, source_descriptor)?;
        let target = staged_target(&context, "author")?;

        staged_source.add_related_holons("AuthoredBy", vec![target.clone().into()])?;
        staged_source.add_related_holons("AuthoredBy", vec![target.into()])?;

        assert_eq!(relationship_member_count(&staged_source, "AuthoredBy")?, 2);
        assert!(staged_source.is_in_state(&context, StagedState::ForUpdateNewVersion)?);
        Ok(())
    }

    #[test]
    fn inverse_relationship_name_is_rejected_as_mutation_input() -> Result<(), HolonError> {
        let context = build_context();
        let fixture = relationship_pair_fixture(&context)?;
        let source = new_test_holon(&context, "person-instance")?;
        let mut staged_source = context.mutation().stage_new_holon(source)?;
        staged_source.with_descriptor((&fixture.target_type).into())?;
        let target = staged_target(&context, "book-instance")?;

        let result = staged_source.add_related_holons("Authors", vec![target.into()]);

        assert!(matches!(
            result,
            Err(HolonError::ValidationError(ValidationError::RelationshipError(message)))
                if message.contains("inverse relationship")
                    && message.contains("declared source endpoint")
        ));
        assert_eq!(relationship_member_count(&staged_source, "Authors")?, 0);
        Ok(())
    }

    #[test]
    fn non_definitional_relationship_mutation_sets_graph_only() -> Result<(), HolonError> {
        let context = build_context();
        let (source_descriptor, _) =
            staged_relationship_descriptor(&context, "AuthoredBy", Some(false))?;
        let mut staged_source = staged_update_source(&context, source_descriptor)?;
        let target = staged_target(&context, "author")?;

        staged_source.add_related_holons("AuthoredBy", vec![target.into()])?;

        assert!(staged_source.is_in_state(&context, StagedState::ForUpdateGraphOnly)?);
        Ok(())
    }

    #[test]
    fn definitional_relationship_mutation_sets_new_version() -> Result<(), HolonError> {
        let context = build_context();
        let (source_descriptor, _) =
            staged_relationship_descriptor(&context, "AuthoredBy", Some(true))?;
        let mut staged_source = staged_update_source(&context, source_descriptor)?;
        let target = staged_target(&context, "author")?;

        staged_source.add_related_holons("AuthoredBy", vec![target.into()])?;

        assert!(staged_source.is_in_state(&context, StagedState::ForUpdateNewVersion)?);
        Ok(())
    }

    #[test]
    fn relationship_removal_uses_definitional_classification() -> Result<(), HolonError> {
        let context = build_context();
        let (source_descriptor, _) =
            staged_relationship_descriptor(&context, "AuthoredBy", Some(false))?;
        let mut staged_source = staged_update_source(&context, source_descriptor)?;
        let target = staged_target(&context, "author")?;
        staged_source.add_related_holons("AuthoredBy", vec![target.clone().into()])?;
        force_staged_reference_for_update(&context, &staged_source)?;

        staged_source.remove_related_holons("AuthoredBy", vec![target.into()])?;

        assert!(staged_source.is_in_state(&context, StagedState::ForUpdateGraphOnly)?);
        let collection = staged_source.related_holons("AuthoredBy")?;
        assert!(collection.read().unwrap().get_members().is_empty());
        Ok(())
    }

    #[test]
    fn missing_relationship_descriptor_errors_before_mutation() -> Result<(), HolonError> {
        let context = build_context();
        let (source_descriptor, _) =
            staged_relationship_descriptor(&context, "AuthoredBy", Some(false))?;
        let mut staged_source = staged_update_source(&context, source_descriptor)?;
        let target = staged_target(&context, "author")?;

        let result = staged_source.add_related_holons("MissingRelationship", vec![target.into()]);

        assert!(matches!(
            result,
            Err(HolonError::DescriptorDeclarationNotFound { kind, name, .. })
                if kind == "relationship" && name == "MissingRelationship"
        ));
        assert!(staged_source.is_in_state(&context, StagedState::ForUpdate)?);
        let collection = staged_source.related_holons("MissingRelationship")?;
        assert!(collection.read().unwrap().get_members().is_empty());
        Ok(())
    }

    #[test]
    fn missing_allows_duplicates_errors_before_mutation() -> Result<(), HolonError> {
        let context = build_context();
        let (source_descriptor, _) = staged_relationship_descriptor(&context, "AuthoredBy", None)?;
        let mut staged_source = staged_update_source(&context, source_descriptor)?;
        let target = staged_target(&context, "author")?;

        let result = staged_source.add_related_holons("AuthoredBy", vec![target.into()]);

        assert!(
            matches!(result, Err(HolonError::EmptyField(field)) if field == "AllowsDuplicates")
        );
        assert!(staged_source.is_in_state(&context, StagedState::ForUpdate)?);
        let collection = staged_source.related_holons("AuthoredBy")?;
        assert!(collection.read().unwrap().get_members().is_empty());
        Ok(())
    }

    #[test]
    fn missing_is_definitional_errors_after_duplicate_policy_resolution() -> Result<(), HolonError>
    {
        let context = build_context();
        let (source_descriptor, _) = staged_relationship_descriptor_with_metadata(
            &context,
            "AuthoredBy",
            None,
            Some(BaseValue::BooleanValue(false.into())),
        )?;
        let mut staged_source = staged_update_source(&context, source_descriptor)?;
        let target = staged_target(&context, "author")?;

        let result = staged_source.add_related_holons("AuthoredBy", vec![target.into()]);

        assert!(matches!(result, Err(HolonError::EmptyField(field)) if field == "IsDefinitional"));
        assert!(staged_source.is_in_state(&context, StagedState::ForUpdate)?);
        assert_eq!(relationship_member_count(&staged_source, "AuthoredBy")?, 0);
        Ok(())
    }

    #[test]
    fn invalid_allows_duplicates_errors_before_mutation() -> Result<(), HolonError> {
        let context = build_context();
        let (source_descriptor, _) = staged_relationship_descriptor_with_metadata(
            &context,
            "AuthoredBy",
            Some(BaseValue::BooleanValue(false.into())),
            Some(BaseValue::StringValue(MapString("not-a-boolean".into()))),
        )?;
        let mut staged_source = staged_update_source(&context, source_descriptor)?;
        let target = staged_target(&context, "author")?;

        let result = staged_source.add_related_holons("AuthoredBy", vec![target.into()]);

        assert!(
            matches!(result, Err(HolonError::UnexpectedValueType(_, expected)) if expected == "Boolean")
        );
        assert!(staged_source.is_in_state(&context, StagedState::ForUpdate)?);
        assert_eq!(relationship_member_count(&staged_source, "AuthoredBy")?, 0);
        Ok(())
    }

    #[test]
    fn invalid_is_definitional_errors_before_mutation() -> Result<(), HolonError> {
        let context = build_context();
        let (source_descriptor, _) =
            staged_relationship_descriptor_with_invalid_is_definitional(&context, "AuthoredBy")?;
        let mut staged_source = staged_update_source(&context, source_descriptor)?;
        let target = staged_target(&context, "author")?;

        let result = staged_source.add_related_holons("AuthoredBy", vec![target.into()]);

        assert!(
            matches!(result, Err(HolonError::UnexpectedValueType(_, expected)) if expected == "Boolean")
        );
        assert!(staged_source.is_in_state(&context, StagedState::ForUpdate)?);
        let collection = staged_source.related_holons("AuthoredBy")?;
        assert!(collection.read().unwrap().get_members().is_empty());
        Ok(())
    }
}
