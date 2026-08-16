//! Storage-layer holon node persistence and exact-version retrieval (Storage SL2, Issue #607).
//!
//! This module owns the translation between MAP's write intent and Holochain's action model.
//! Callers declare *what kind of write this is* — begin a lineage, or add a version to one —
//! and this layer decides that a root is a `Create` and a version is an `Update` addressed at
//! the lineage's root `Create`. Nothing above this boundary sees an `Action`, a `Record`, or an
//! `original_action_address`; they arrive as `VersionMetadata` derived from the record itself,
//! never from entry content.
//!
//! Reads are exact-version: `get_holon` returns the record you named, not the head of its
//! lineage. Head selection, revision-history traversal, and version-graph walking are
//! deliberately absent — they are later storage work, and folding them in here would make an
//! exact read ambiguous.
//!
//! One deliberate non-decision, recorded so it is not mistaken for an oversight: `PublishRoot`
//! indexes the new holon under `AllHolonNodes`; `PublishVersion` does not. The lineage root is
//! already indexed, and one index entry per version would make a get-all return every version of
//! every holon as a separate top-level result.
//!
//! Versions are authored as native root-addressed Holochain updates and nothing else: Holochain's
//! own update graph already records that an update happened, so there is no parallel link index
//! to keep in step with it.

use core_types::{
    resolve_shared_lineage_root, HolonError, HolonWriteRequest, LineageId, StoredHolonNode,
    VersionMetadata,
};
use hdk::prelude::*;
use holochain_serialized_bytes::encode;
use holons_guest_integrity::{type_conversions::*, HolonNode, ALL_HOLON_NODES_PATH};
use holons_integrity::{EntryTypes, LinkTypes};
use integrity_core_types::{short_hex, HolonNodeModel, LocalId};
use shared_validation::{validate_holon_node_decoded, validate_holon_node_size};

// ---------------------------------------------------------------------------
// Read: exact-version retrieval
// ---------------------------------------------------------------------------

/// Returns the exact persisted version named by `local_id`, or `None` when it is absent.
///
/// Reads the exact record — never the head of its lineage. A record that exists but cannot be
/// classified fails rather than reporting absence: see `decode_stored_holon_node`.
pub fn get_holon(local_id: &LocalId) -> Result<Option<StoredHolonNode>, HolonError> {
    let action_hash = try_action_hash_from_local_id(local_id)?;
    let Some(record) =
        get(action_hash, GetOptions::default()).map_err(holon_error_from_wasm_error)?
    else {
        return Ok(None);
    };

    Ok(Some(decode_stored_holon_node(&record)?))
}

/// Returns one positional slot per requested id, preserving order and duplicates.
///
/// `None` means "absent from the DHT". A malformed id, an unsupported action, or a malformed
/// entry fails the whole call rather than degrading to `None`, so a decoding defect can never
/// be mistaken for a missing holon.
pub fn get_holons(local_ids: &[LocalId]) -> Result<Vec<Option<StoredHolonNode>>, HolonError> {
    if local_ids.is_empty() {
        return Ok(Vec::new());
    }

    let get_inputs = local_ids
        .iter()
        .map(|local_id| {
            Ok(GetInput::new(
                try_action_hash_from_local_id(local_id)?.into(),
                GetOptions::default(),
            ))
        })
        .collect::<Result<Vec<GetInput>, HolonError>>()?;

    // The host answers a batched get positionally: one slot per input, in order, `None` for
    // anything it could not find. That is exactly the contract this function owes its callers,
    // so the slots are mapped through rather than flattened.
    let records =
        HDK.with(|hdk| hdk.borrow().get(get_inputs)).map_err(holon_error_from_wasm_error)?;

    records
        .into_iter()
        .map(|slot| slot.map(|record| decode_stored_holon_node(&record)).transpose())
        .collect()
}

/// Returns version metadata for `action_hash`, or `None` when the record is absent or is not a
/// holon node.
///
/// Post-commit signalling asks "is this a holon node, and if so what lineage is it in?" about an
/// arbitrary action, so it must stay quiet for the many actions that are not holon nodes. It stays
/// quiet only for those: a record that *claims* to be a holon node and will not decode still
/// fails, because silently emitting no signal would turn corruption into a missing event.
pub fn try_version_metadata_for_action(
    action_hash: &ActionHash,
) -> Result<Option<VersionMetadata>, HolonError> {
    let Some(record) =
        get(action_hash.clone(), GetOptions::default()).map_err(holon_error_from_wasm_error)?
    else {
        return Ok(None);
    };

    match classify_record(&record)? {
        RecordClassification::NotAHolonNode => Ok(None),
        RecordClassification::HolonNode(_) => Ok(Some(version_metadata_from_record(&record)?)),
    }
}

// ---------------------------------------------------------------------------
// Write: intent-driven action selection
// ---------------------------------------------------------------------------

/// Persists holon node content, selecting the substrate action from the request variant.
///
/// `PublishRoot` authors a `Create` and begins a lineage. `PublishVersion` resolves the lineage
/// root shared by every predecessor and authors an `Update` against that root, so every version
/// in a lineage sits exactly one hop from the same `Create` — a version of a version is still
/// rooted at the original, not at its immediate predecessor.
///
/// Immediate-predecessor ordering is not a storage concern. It is carried above this layer by
/// `Predecessor`/`Successor` SmartLinks, which is why `predecessor_ids` informs the lineage
/// decision here but is not itself persisted.
pub fn persist_holon(request: HolonWriteRequest) -> Result<StoredHolonNode, HolonError> {
    match request {
        HolonWriteRequest::PublishRoot { holon_node } => {
            preflight_holon_node(&holon_node)?;

            let action_hash =
                create_entry(&EntryTypes::HolonNode(HolonNode::from(holon_node.clone())))
                    .map_err(holon_error_from_wasm_error)?;

            index_under_all_holon_nodes(&action_hash)?;

            let version_metadata = VersionMetadata::root(local_id_from_action_hash(action_hash));
            debug!(
                "persist_holon: PublishRoot -> create_entry, version_id={}",
                short_hex(&version_metadata.version_id, 8)
            );

            Ok(StoredHolonNode::new(holon_node, version_metadata))
        }

        HolonWriteRequest::PublishVersion { holon_node, predecessor_ids } => {
            preflight_holon_node(&holon_node)?;

            let lineage_id = resolve_lineage_root_for_predecessors(&predecessor_ids)?;
            let root_hash = try_action_hash_from_local_id(lineage_id.as_local_id())?;

            let action_hash = update_entry(
                root_hash,
                &EntryTypes::HolonNode(HolonNode::from(holon_node.clone())),
            )
            .map_err(holon_error_from_wasm_error)?;

            let version_metadata = VersionMetadata::derived(
                local_id_from_action_hash(action_hash),
                lineage_id.clone(),
            );
            debug!(
                "persist_holon: PublishVersion -> update_entry rooted at {}, {} predecessor(s), version_id={}",
                lineage_id,
                predecessor_ids.len(),
                short_hex(&version_metadata.version_id, 8)
            );

            Ok(StoredHolonNode::new(holon_node, version_metadata))
        }
    }
}

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

/// Applies descriptor-independent PVL to the canonical HolonNode value intended for authoring.
///
/// The raw and canonical byte slices are identical by construction, so the canonicality
/// comparison inside `validate_holon_node_decoded` is intentionally vacuous at this typed
/// coordinator boundary. Reusing that function is still important because it owns the
/// established property-count and property-value validation pipeline.
///
/// This preflight is not Integrity-adapter parity. Integrity starts with arbitrary peer-supplied
/// bytes and additionally classifies flattened operations, checks substrate action facts, and
/// resolves dependencies whose availability can produce `UnresolvedDependencies`. Passing this
/// check therefore does not guarantee conductor acceptance.
///
/// `create_entry` and `update_entry` serialize their typed values internally, so this function
/// cannot submit the bytes it measured. The encoding-parity tests in `holon_node_envelope` pin
/// the required equivalence among `HolonNodeModel`, the guest `HolonNode`, and stored app-entry
/// payload bytes.
fn preflight_holon_node(model: &HolonNodeModel) -> Result<(), HolonError> {
    let serialized = encode(model).map_err(|error| HolonError::InvalidWireFormat {
        wire_type: "HolonNodeModel".into(),
        reason: format!("canonical serialization failed: {error}"),
    })?;

    validate_holon_node_size(serialized.len()).map_err(HolonError::PvlViolation)?;
    validate_holon_node_decoded(&serialized, &serialized, model).map_err(HolonError::PvlViolation)
}

/// What a persisted record turns out to be, from this layer's point of view.
///
/// The distinction that matters is *not a holon node* versus *a holon node that will not decode*.
/// The first is an ordinary answer — plenty of records are links or deletes. The second is
/// corruption, and must never be reported as the first.
enum RecordClassification {
    /// Carries a decodable holon node entry.
    HolonNode(HolonNodeModel),
    /// A well-formed record that simply is not a holon node: it carries no entry, or carries an
    /// entry belonging to another type.
    NotAHolonNode,
}

/// Projects a persisted record into the storage-boundary form.
///
/// Fails explicitly on anything that is not an intact holon node version, and distinguishes the
/// three ways that can happen:
///
/// - the action is not one MAP authors for a holon node (`Create` begins a lineage, `Update`
///   extends one; anything else is unsupported)
/// - the record carries no holon node entry at all
/// - the record claims a holon node entry that will not decode — corruption
///
/// The action is classified *first*, so a record that carries no entry because of its action kind
/// — a link or a delete — is reported as an unsupported action rather than as malformed content.
/// Reporting the wrong one sends whoever is debugging to the wrong place.
pub(crate) fn decode_stored_holon_node(record: &Record) -> Result<StoredHolonNode, HolonError> {
    let version_metadata = version_metadata_from_record(record)?;

    match classify_record(record)? {
        RecordClassification::HolonNode(holon_node) => {
            Ok(StoredHolonNode::new(holon_node, version_metadata))
        }
        RecordClassification::NotAHolonNode => Err(HolonError::RecordConversion(format!(
            "Record {:?} does not carry a HolonNode entry",
            version_metadata.version_id
        ))),
    }
}

/// Derives version metadata from a record's action, independent of its entry content.
fn version_metadata_from_record(record: &Record) -> Result<VersionMetadata, HolonError> {
    let version_id = local_id_from_action_hash(record.action_address().clone());

    match record.action() {
        Action::Create(_) => Ok(VersionMetadata::root(version_id)),
        Action::Update(update) => Ok(VersionMetadata::derived(
            version_id,
            LineageId(local_id_from_action_hash(update.original_action_address.clone())),
        )),
        other => Err(HolonError::RecordConversion(format!(
            "Unsupported action kind for HolonNode at {:?}: {:?}",
            version_id,
            other.action_type()
        ))),
    }
}

/// Classifies a record's entry, failing only when it claims to be a holon node but will not decode.
///
/// The scoped entry type is consulted before decoding, so "this entry belongs to another type" is
/// answered without guessing from a failed deserialization. That is what lets a decode failure
/// mean corruption and nothing else.
fn classify_record(record: &Record) -> Result<RecordClassification, HolonError> {
    let Some(entry) = record.entry().as_option() else {
        return Ok(RecordClassification::NotAHolonNode);
    };

    let Some(EntryType::App(AppEntryDef { zome_index, entry_index, .. })) =
        record.action().entry_type()
    else {
        return Ok(RecordClassification::NotAHolonNode);
    };

    // A decode failure here is corruption, not a type mismatch: the scoped indices already say
    // this entry belongs to this zome's entry definitions.
    let decoded = EntryTypes::deserialize_from_type(zome_index.clone(), entry_index.clone(), entry)
        .map_err(|error| {
            HolonError::RecordConversion(format!(
                "HolonNode entry at {:?} could not be decoded: {}",
                local_id_from_action_hash(record.action_address().clone()),
                error
            ))
        })?;

    Ok(match decoded {
        Some(EntryTypes::HolonNode(holon_node)) => {
            RecordClassification::HolonNode(HolonNodeModel::from(holon_node))
        }
        None => RecordClassification::NotAHolonNode,
    })
}

/// Loads each predecessor and returns the single lineage root they share.
///
/// Predecessors are loaded through `get_holons` so they pass the same decoding rule as any
/// other read: a predecessor that is not a holon node fails here, with a storage-layer message,
/// rather than being caught later and more opaquely by integrity validation.
fn resolve_lineage_root_for_predecessors(
    predecessor_ids: &[LocalId],
) -> Result<LineageId, HolonError> {
    let slots = get_holons(predecessor_ids)?;

    let predecessors = predecessor_ids
        .iter()
        .zip(slots)
        .map(|(local_id, slot)| {
            slot.map(|stored| stored.version_metadata).ok_or_else(|| {
                HolonError::HolonNotFound(format!(
                    "Predecessor {:?} is not persisted, so no lineage can be resolved for it",
                    local_id
                ))
            })
        })
        .collect::<Result<Vec<VersionMetadata>, HolonError>>()?;

    resolve_shared_lineage_root(&predecessors).inspect_err(|error| {
        warn!("persist_holon: PublishVersion rejected — {}", error);
    })
}

/// Adds the new holon to the space-wide holon index.
fn index_under_all_holon_nodes(action_hash: &ActionHash) -> Result<(), HolonError> {
    let path = Path::from(ALL_HOLON_NODES_PATH);
    let base = path.path_entry_hash().map_err(holon_error_from_wasm_error)?;

    create_link(base, action_hash.clone(), LinkTypes::AllHolonNodes, ())
        .map_err(holon_error_from_wasm_error)?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use base_types::{BaseValue, MapBoolean, MapString};
    use integrity_core_types::{PropertyMap, PropertyName, PvlViolation};

    fn property_name(value: impl Into<String>) -> PropertyName {
        PropertyName(MapString(value.into()))
    }

    #[test]
    fn preflight_rejects_257_properties_with_the_stable_pvl_error() {
        let property_map = (0..257)
            .map(|index| {
                (
                    property_name(format!("property-{index}")),
                    BaseValue::BooleanValue(MapBoolean(true)),
                )
            })
            .collect::<PropertyMap>();
        let model = HolonNodeModel::new(property_map);

        let error = preflight_holon_node(&model).unwrap_err();

        assert_eq!(
            error,
            HolonError::PvlViolation(PvlViolation::TooManyProperties {
                actual_count: 257,
                max_count: 256,
            })
        );
        assert_eq!(error.to_string(), "MAP-PVL-1101: property count exceeds 256");
    }

    #[test]
    fn preflight_rejects_an_empty_property_name_with_the_stable_pvl_error() {
        let model = HolonNodeModel::new(PropertyMap::from([(
            property_name(""),
            BaseValue::BooleanValue(MapBoolean(true)),
        )]));

        let error = preflight_holon_node(&model).unwrap_err();

        assert_eq!(error, HolonError::PvlViolation(PvlViolation::EmptyPropertyName));
        assert_eq!(error.to_string(), "MAP-PVL-1102: property name is empty");
    }
}
