//! Holochain adapter for descriptor-independent HolonNode envelope validation.

use hdi::prelude::*;
use holochain_serialized_bytes::{decode, encode};
use integrity_core_types::{HolonNodeModel, PvlMalformedReason, PvlViolation};
use shared_validation::{validate_holon_node_decoded, validate_holon_node_size};

use crate::HolonNode;

const HOLON_NODE_ENTRY_DEF_INDEX: EntryDefIndex = EntryDefIndex(0);

/// Result of preparing the HolonNode envelope carried by an operation.
#[derive(Debug, PartialEq, Eq)]
pub enum HolonNodeEnvelope {
    /// The operation does not carry a HolonNode app entry.
    NotApplicable,
    /// The entry passed all envelope rules and is ready for lifecycle validation.
    Valid(HolonNodeModel),
    /// The entry failed a deterministic PVL envelope rule.
    Invalid(PvlViolation),
}

/// Extracts and validates a HolonNode entry before flattened-op decoding.
pub fn prepare_holon_node_envelope(op: &Op) -> ExternResult<HolonNodeEnvelope> {
    let Some(raw) = holon_node_entry_bytes(op) else {
        return Ok(HolonNodeEnvelope::NotApplicable);
    };

    Ok(match run_holon_node_envelope(raw)? {
        Ok(model) => HolonNodeEnvelope::Valid(model),
        Err(violation) => HolonNodeEnvelope::Invalid(violation),
    })
}

/// Locates the stored inner app-entry payload without decoding it.
fn holon_node_entry_bytes(op: &Op) -> Option<&[u8]> {
    let (entry_type, entry) = match op {
        Op::StoreEntry(store_entry) => (store_entry.action.hashed.entry_type(), &store_entry.entry),
        Op::StoreRecord(store_record) => {
            (store_record.record.action().entry_type()?, store_record.record.entry().as_option()?)
        }
        Op::RegisterUpdate(register_update) => {
            (&register_update.update.hashed.entry_type, register_update.new_entry.as_ref()?)
        }
        _ => return None,
    };

    // HolonNode is the sole app entry in this integrity zome and therefore entry-def index 0.
    // The integrity callback already scopes the zome index; keep this constant aligned if another
    // app entry is ever added or the declaration order changes.
    match entry_type {
        EntryType::App(app_entry_def)
            if app_entry_def.entry_index == HOLON_NODE_ENTRY_DEF_INDEX => {}
        _ => return None,
    }

    Some(entry.as_app_entry()?.as_ref().bytes())
}

/// Runs the consensus-ordered envelope pipeline against stored inner-entry bytes.
///
/// The outer result is reserved for serialization/callback failures. The inner result carries
/// deterministic PVL violations that the integrity zome maps explicitly to `Invalid`.
fn run_holon_node_envelope(raw: &[u8]) -> ExternResult<Result<HolonNodeModel, PvlViolation>> {
    // Rejecting size first bounds work even when the payload is malformed or expensive to decode.
    if let Err(violation) = validate_holon_node_size(raw.len()) {
        return Ok(Err(violation));
    }

    let node: HolonNode = match decode(raw) {
        Ok(node) => node,
        Err(_) => {
            return Ok(Err(PvlViolation::MalformedHolonNode {
                reason: PvlMalformedReason::DecodeFailed,
            }))
        }
    };
    let model = HolonNodeModel::from(node);

    // Canonicalize the model rather than the outer EntryTypes enum: Holochain stores these inner
    // bytes, and comparing at this boundary preserves alternate-encoding evidence lost by decode.
    let canonical = encode(&model).map_err(|error| wasm_error!(error))?;
    if let Err(violation) = validate_holon_node_decoded(raw, &canonical, &model) {
        return Ok(Err(violation));
    }

    Ok(Ok(model))
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use base_types::{BaseValue, MapBoolean, MapBytes, MapEnumValue, MapInteger, MapString};
    use holochain_serialized_bytes::UnsafeBytes;
    use integrity_core_types::{LocalId, PropertyMap, PropertyName};
    use serde::ser::{SerializeMap, SerializeStruct};
    use serde::Serialize;
    use shared_validation::pvl_limits_v1::{MAX_HOLON_NODE_BYTES, MAX_PROPERTY_COUNT};

    use super::*;

    fn property_name(value: &str) -> PropertyName {
        PropertyName(MapString(value.to_string()))
    }

    fn canonical_node(property_map: PropertyMap) -> HolonNode {
        HolonNode::new(None, property_map)
    }

    fn run(raw: &[u8]) -> Result<HolonNodeModel, PvlViolation> {
        run_holon_node_envelope(raw).expect("test values should not fail canonical serialization")
    }

    #[derive(Clone, Copy, Debug)]
    enum HolonNodeOpForm {
        StoreEntryCreate,
        StoreEntryUpdate,
        RegisterUpdate,
        StoreRecordCreate,
        StoreRecordUpdate,
    }

    const HOLON_NODE_OP_FORMS: [HolonNodeOpForm; 5] = [
        HolonNodeOpForm::StoreEntryCreate,
        HolonNodeOpForm::StoreEntryUpdate,
        HolonNodeOpForm::RegisterUpdate,
        HolonNodeOpForm::StoreRecordCreate,
        HolonNodeOpForm::StoreRecordUpdate,
    ];

    fn app_entry(raw: Vec<u8>) -> Entry {
        Entry::App(AppEntryBytes(SerializedBytes::from(UnsafeBytes::from(raw))))
    }

    fn app_entry_type() -> EntryType {
        EntryType::App(AppEntryDef::new(
            HOLON_NODE_ENTRY_DEF_INDEX,
            ZomeIndex(0),
            EntryVisibility::Public,
        ))
    }

    fn create_action() -> Create {
        Create {
            author: AgentPubKey::from_raw_36(vec![0; 36]),
            timestamp: Timestamp::from_micros(1),
            action_seq: 1,
            prev_action: ActionHash::from_raw_36(vec![1; 36]),
            entry_type: app_entry_type(),
            entry_hash: EntryHash::from_raw_36(vec![2; 36]),
            weight: EntryRateWeight::default(),
        }
    }

    fn update_action() -> Update {
        Update {
            author: AgentPubKey::from_raw_36(vec![0; 36]),
            timestamp: Timestamp::from_micros(2),
            action_seq: 2,
            prev_action: ActionHash::from_raw_36(vec![3; 36]),
            original_action_address: ActionHash::from_raw_36(vec![4; 36]),
            original_entry_address: EntryHash::from_raw_36(vec![5; 36]),
            entry_type: app_entry_type(),
            entry_hash: EntryHash::from_raw_36(vec![6; 36]),
            weight: EntryRateWeight::default(),
        }
    }

    fn signed_entry_creation_action(
        action: EntryCreationAction,
    ) -> SignedHashed<EntryCreationAction> {
        SignedHashed::with_presigned(
            HoloHashed::with_pre_hashed(action, ActionHash::from_raw_36(vec![7; 36])),
            Signature([0; SIGNATURE_BYTES]),
        )
    }

    fn signed_update(update: Update) -> SignedHashed<Update> {
        SignedHashed::with_presigned(
            HoloHashed::with_pre_hashed(update, ActionHash::from_raw_36(vec![8; 36])),
            Signature([0; SIGNATURE_BYTES]),
        )
    }

    fn signed_action(action: Action) -> SignedActionHashed {
        SignedHashed::with_presigned(
            HoloHashed::with_pre_hashed(action, ActionHash::from_raw_36(vec![9; 36])),
            Signature([0; SIGNATURE_BYTES]),
        )
    }

    fn op_with_raw_entry(form: HolonNodeOpForm, raw: Vec<u8>) -> Op {
        let entry = app_entry(raw);

        match form {
            HolonNodeOpForm::StoreEntryCreate => Op::StoreEntry(StoreEntry {
                action: signed_entry_creation_action(EntryCreationAction::Create(create_action())),
                entry,
            }),
            HolonNodeOpForm::StoreEntryUpdate => Op::StoreEntry(StoreEntry {
                action: signed_entry_creation_action(EntryCreationAction::Update(update_action())),
                entry,
            }),
            HolonNodeOpForm::RegisterUpdate => Op::RegisterUpdate(RegisterUpdate {
                update: signed_update(update_action()),
                new_entry: Some(entry),
            }),
            HolonNodeOpForm::StoreRecordCreate => Op::StoreRecord(StoreRecord {
                record: Record::new(signed_action(Action::Create(create_action())), Some(entry)),
            }),
            HolonNodeOpForm::StoreRecordUpdate => Op::StoreRecord(StoreRecord {
                record: Record::new(signed_action(Action::Update(update_action())), Some(entry)),
            }),
        }
    }

    #[test]
    fn all_five_op_forms_select_inner_bytes_and_use_the_public_envelope_seam() {
        for form in HOLON_NODE_OP_FORMS {
            let node = canonical_node(BTreeMap::from([(
                property_name("op-form"),
                BaseValue::StringValue(MapString(format!("{form:?}"))),
            )]));
            let expected_model = HolonNodeModel::from(node.clone());
            let raw = encode(&node).unwrap();
            let op = op_with_raw_entry(form, raw.clone());

            assert_eq!(
                holon_node_entry_bytes(&op),
                Some(raw.as_slice()),
                "{form:?} selected the wrong inner app-entry bytes"
            );
            assert_eq!(
                prepare_holon_node_envelope(&op).unwrap(),
                HolonNodeEnvelope::Valid(expected_model),
                "{form:?} did not complete the shared envelope pipeline"
            );
        }
    }

    #[test]
    fn all_five_op_forms_reject_oversized_payloads_before_later_processing() {
        let expected = HolonNodeEnvelope::Invalid(PvlViolation::HolonNodeTooLarge {
            actual_bytes: 262_145,
            max_bytes: 262_144,
        });

        for form in HOLON_NODE_OP_FORMS {
            let raw = vec![0xc1; MAX_HOLON_NODE_BYTES + 1];
            let op = op_with_raw_entry(form, raw);

            // In particular, StoreRecordUpdate carries an arbitrary original-action address.
            // Preparation must reject its current entry without attempting dependency resolution.
            assert_eq!(
                prepare_holon_node_envelope(&op).unwrap(),
                expected,
                "{form:?} did not apply the raw-size rule through the public seam"
            );
        }
    }

    #[test]
    fn non_holon_node_and_non_entry_ops_are_not_applicable() {
        let mut other_entry_create = create_action();
        other_entry_create.entry_type = EntryType::App(AppEntryDef::new(
            EntryDefIndex(1),
            ZomeIndex(0),
            EntryVisibility::Public,
        ));
        let other_entry_op = Op::StoreEntry(StoreEntry {
            action: signed_entry_creation_action(EntryCreationAction::Create(other_entry_create)),
            entry: app_entry(vec![0xc1]),
        });

        let delete = Delete {
            author: AgentPubKey::from_raw_36(vec![0; 36]),
            timestamp: Timestamp::from_micros(3),
            action_seq: 3,
            prev_action: ActionHash::from_raw_36(vec![10; 36]),
            deletes_address: ActionHash::from_raw_36(vec![11; 36]),
            deletes_entry_address: EntryHash::from_raw_36(vec![12; 36]),
            weight: RateWeight::default(),
        };
        let non_entry_op = Op::RegisterDelete(RegisterDelete {
            delete: SignedHashed::with_presigned(
                HoloHashed::with_pre_hashed(delete, ActionHash::from_raw_36(vec![13; 36])),
                Signature([0; SIGNATURE_BYTES]),
            ),
        });

        for op in [other_entry_op, non_entry_op] {
            assert_eq!(holon_node_entry_bytes(&op), None);
            assert_eq!(prepare_holon_node_envelope(&op).unwrap(), HolonNodeEnvelope::NotApplicable);
        }
    }

    #[test]
    fn model_encoding_matches_guest_inner_entry_encoding() {
        let empty = HolonNode::new(None, PropertyMap::new());
        assert_eq!(encode(&empty).unwrap(), encode(&HolonNodeModel::from(empty.clone())).unwrap());

        let properties = BTreeMap::from([
            (property_name("z-bytes"), BaseValue::BytesValue(MapBytes(vec![1, 2, 3]))),
            (property_name("a-string"), BaseValue::StringValue(MapString("value".into()))),
            (property_name("m-bool"), BaseValue::BooleanValue(MapBoolean(true))),
            (property_name("c-int"), BaseValue::IntegerValue(MapInteger(-42))),
            (
                property_name("e-enum"),
                BaseValue::EnumValue(MapEnumValue(MapString("Active".into()))),
            ),
        ]);
        let representative = HolonNode::new(Some(LocalId(vec![7; 39])), properties);

        assert_eq!(
            encode(&representative).unwrap(),
            encode(&HolonNodeModel::from(representative)).unwrap()
        );
    }

    #[test]
    fn stored_app_entry_payload_matches_guest_encoding() {
        let node = canonical_node(BTreeMap::from([(
            property_name("name"),
            BaseValue::StringValue(MapString("stored".into())),
        )]));
        let entry = Entry::app(SerializedBytes::try_from(&node).unwrap()).unwrap();
        let stored =
            entry.as_app_entry().expect("entry was constructed as an app entry").as_ref().bytes();
        let encoded = encode(&node).unwrap();

        assert_eq!(stored.as_slice(), encoded.as_slice());
    }

    #[test]
    fn accepts_canonical_holon_node_bytes() {
        let node = canonical_node(BTreeMap::from([(
            property_name("name"),
            BaseValue::StringValue(MapString("canonical".into())),
        )]));
        let raw = encode(&node).unwrap();

        assert_eq!(run(&raw), Ok(HolonNodeModel::from(node)));
    }

    #[test]
    fn maps_decode_failures_to_the_fixed_malformed_reason() {
        assert_eq!(
            run(&[0xc1]),
            Err(PvlViolation::MalformedHolonNode { reason: PvlMalformedReason::DecodeFailed })
        );
    }

    #[test]
    fn rejects_duplicate_property_keys_as_non_canonical() {
        let raw = encode(&NodeWithProperties(DuplicateProperties)).unwrap();

        assert_non_canonical(&raw);
    }

    #[test]
    fn rejects_reordered_property_map_as_non_canonical() {
        let raw = encode(&NodeWithProperties(ReorderedProperties)).unwrap();

        assert_non_canonical(&raw);
    }

    #[test]
    fn rejects_non_canonical_integer_width() {
        let node = canonical_node(BTreeMap::from([(
            property_name("integer"),
            BaseValue::IntegerValue(MapInteger(1)),
        )]));
        let mut raw = encode(&node).unwrap();
        assert_eq!(
            raw.pop(),
            Some(1),
            "canonical integer value should be the terminal positive fixint"
        );
        raw.extend_from_slice(&[0xd3, 0, 0, 0, 0, 0, 0, 0, 1]);

        assert_non_canonical(&raw);
    }

    #[test]
    fn rejects_numeric_and_string_boolean_substitutes_during_typed_decode() {
        let numeric = encode(&NodeWithProperties(BTreeMap::from([(
            property_name("boolean"),
            ForgedNativeValue::new(1, "BooleanValue", 1_u8),
        )])))
        .unwrap();
        assert_decode_failed(&numeric);

        let string = encode(&NodeWithProperties(BTreeMap::from([(
            property_name("boolean"),
            ForgedNativeValue::new(1, "BooleanValue", "true"),
        )])))
        .unwrap();
        assert_decode_failed(&string);
    }

    #[test]
    fn rejects_overflowing_and_wrongly_typed_integer_values_during_typed_decode() {
        let overflowing = encode(&NodeWithProperties(BTreeMap::from([(
            property_name("integer"),
            ForgedNativeValue::new(2, "IntegerValue", u64::MAX),
        )])))
        .unwrap();
        assert_decode_failed(&overflowing);

        let wrongly_typed = encode(&NodeWithProperties(BTreeMap::from([(
            property_name("integer"),
            ForgedNativeValue::new(2, "IntegerValue", "1"),
        )])))
        .unwrap();
        assert_decode_failed(&wrongly_typed);
    }

    #[test]
    fn rejects_ignored_extra_field_as_non_canonical() {
        let raw = encode(&NodeWithExtraField {
            original_id: None,
            property_map: PropertyMap::new(),
            ignored: true,
        })
        .unwrap();

        assert_non_canonical(&raw);
    }

    #[test]
    fn rejects_oversized_bytes_before_decode() {
        let raw = vec![0xc1; MAX_HOLON_NODE_BYTES + 1];

        assert_eq!(
            run(&raw),
            Err(PvlViolation::HolonNodeTooLarge { actual_bytes: 262_145, max_bytes: 262_144 })
        );
    }

    #[test]
    fn rejects_more_than_the_property_limit() {
        let properties = (0..=MAX_PROPERTY_COUNT)
            .map(|index| {
                (
                    property_name(&format!("property-{index:03}")),
                    BaseValue::BooleanValue(MapBoolean(true)),
                )
            })
            .collect();
        let raw = encode(&canonical_node(properties)).unwrap();

        assert_eq!(
            run(&raw),
            Err(PvlViolation::TooManyProperties { actual_count: 257, max_count: 256 })
        );
    }

    fn assert_non_canonical(raw: &[u8]) {
        assert_eq!(
            run(raw),
            Err(PvlViolation::MalformedHolonNode {
                reason: PvlMalformedReason::NonCanonicalEncoding,
            })
        );
    }

    fn assert_decode_failed(raw: &[u8]) {
        assert_eq!(
            run(raw),
            Err(PvlViolation::MalformedHolonNode { reason: PvlMalformedReason::DecodeFailed })
        );
    }

    #[derive(Debug)]
    struct NodeWithProperties<T>(T);

    impl<T: Serialize> Serialize for NodeWithProperties<T> {
        fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
            let mut node = serializer.serialize_struct("HolonNode", 2)?;
            node.serialize_field("original_id", &Option::<LocalId>::None)?;
            node.serialize_field("property_map", &self.0)?;
            node.end()
        }
    }

    #[derive(Debug)]
    struct ForgedNativeValue<T> {
        variant_index: u32,
        variant_name: &'static str,
        value: T,
    }

    impl<T> ForgedNativeValue<T> {
        fn new(variant_index: u32, variant_name: &'static str, value: T) -> Self {
            Self { variant_index, variant_name, value }
        }
    }

    impl<T: Serialize> Serialize for ForgedNativeValue<T> {
        fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
            serializer.serialize_newtype_variant(
                "BaseValue",
                self.variant_index,
                self.variant_name,
                &self.value,
            )
        }
    }

    #[derive(Debug)]
    struct DuplicateProperties;

    impl Serialize for DuplicateProperties {
        fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
            let mut map = serializer.serialize_map(Some(2))?;
            let key = property_name("duplicate");
            map.serialize_entry(&key, &BaseValue::BooleanValue(MapBoolean(false)))?;
            map.serialize_entry(&key, &BaseValue::BooleanValue(MapBoolean(true)))?;
            map.end()
        }
    }

    #[derive(Debug)]
    struct ReorderedProperties;

    impl Serialize for ReorderedProperties {
        fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
            let mut map = serializer.serialize_map(Some(2))?;
            map.serialize_entry(
                &property_name("z-last"),
                &BaseValue::BooleanValue(MapBoolean(true)),
            )?;
            map.serialize_entry(
                &property_name("a-first"),
                &BaseValue::BooleanValue(MapBoolean(false)),
            )?;
            map.end()
        }
    }

    #[derive(Debug, Serialize)]
    struct NodeWithExtraField {
        original_id: Option<LocalId>,
        property_map: PropertyMap,
        ignored: bool,
    }
}
