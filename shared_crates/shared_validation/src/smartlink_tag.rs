use std::{collections::BTreeMap, error::Error, fmt};

use base_types::{BaseValue, MapBoolean, MapBytes, MapEnumValue, MapInteger, MapString};
use core_types::{HolonId, LocalId, OutboundProxyId, PropertyMap, PropertyName, RelationshipName};

/// The stable byte marker that identifies a SmartLink tag.
pub const SMARTLINK_HEADER_BYTES: [u8; 3] = [226, 130, 183];

/// The first version of the length-prefixed SmartLink tag format.
pub const SMARTLINK_TAG_FORMAT_VERSION_V2: u8 = 2;

/// Holochain hashes use a fixed 39-byte raw representation.
pub const HOLOCHAIN_HASH_BYTES: usize = 39;

const LOCAL_REFERENCE_TYPE: u8 = 0;
const EXTERNAL_REFERENCE_TYPE: u8 = 1;
const NO_FORWARD_LINK_PROVENANCE: u8 = 0;
const HAS_FORWARD_LINK_PROVENANCE: u8 = 1;
const STRING_VALUE_KIND: u8 = 0;
const BOOLEAN_VALUE_KIND: u8 = 1;
const INTEGER_VALUE_KIND: u8 = 2;
const ENUM_VALUE_KIND: u8 = 3;
const BYTES_VALUE_KIND: u8 = 4;
// Empty property names and values are valid, so the minimum is a name length, kind, and value length.
const MIN_PROPERTY_RECORD_BYTES: usize = 9;

/// The byte-level SmartLink data recovered from a link tag.
///
/// The local target id is carried by Holochain's link action, not by this tag.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct LinkTagObject {
    pub relationship_name: String,
    pub proxy_id: Option<OutboundProxyId>,
    pub forward_link_provenance: Option<LocalId>,
    pub smart_property_values: Option<PropertyMap>,
}

/// Temporary decode and encoding failures for the SmartLink byte contract.
///
/// PR 6 maps these structural failures onto the PVL violation model.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SmartLinkTagError {
    InvalidHeader,
    UnsupportedVersion(u8),
    UnexpectedEnd,
    LengthOverflow,
    InvalidReferenceType(u8),
    InvalidProvenanceMarker(u8),
    InvalidValueKind(u8),
    InvalidBooleanValue,
    InvalidIntegerLength(usize),
    InvalidUtf8(&'static str),
    InvalidHashLength { field: &'static str, actual: usize },
    InvalidPropertyCount,
    NonCanonicalPropertyOrder,
    TrailingData,
}

impl fmt::Display for SmartLinkTagError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidHeader => write!(formatter, "SmartLink tag is missing its header"),
            Self::UnsupportedVersion(version) => {
                write!(formatter, "unsupported SmartLink tag version {version}")
            }
            Self::UnexpectedEnd => write!(formatter, "SmartLink tag ended unexpectedly"),
            Self::LengthOverflow => write!(formatter, "SmartLink tag field length overflows u32"),
            Self::InvalidReferenceType(value) => {
                write!(formatter, "invalid SmartLink reference type {value}")
            }
            Self::InvalidProvenanceMarker(value) => {
                write!(formatter, "invalid SmartLink provenance marker {value}")
            }
            Self::InvalidValueKind(value) => {
                write!(formatter, "invalid SmartLink value kind {value}")
            }
            Self::InvalidBooleanValue => write!(formatter, "invalid SmartLink boolean value"),
            Self::InvalidIntegerLength(length) => {
                write!(formatter, "invalid SmartLink integer length {length}")
            }
            Self::InvalidUtf8(field) => write!(formatter, "SmartLink {field} is not valid UTF-8"),
            Self::InvalidHashLength { field, actual } => {
                write!(
                    formatter,
                    "SmartLink {field} must be {HOLOCHAIN_HASH_BYTES} bytes, got {actual}"
                )
            }
            Self::InvalidPropertyCount => write!(formatter, "invalid SmartLink property count"),
            Self::NonCanonicalPropertyOrder => {
                write!(formatter, "SmartLink properties are not in canonical order")
            }
            Self::TrailingData => write!(formatter, "SmartLink tag contains trailing data"),
        }
    }
}

impl Error for SmartLinkTagError {}

/// Encodes the deterministic relationship-name prefix used by Holochain tag-prefix queries.
pub fn encode_link_tag_prolog(
    relationship_name: &RelationshipName,
) -> Result<Vec<u8>, SmartLinkTagError> {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(&SMARTLINK_HEADER_BYTES);
    bytes.push(SMARTLINK_TAG_FORMAT_VERSION_V2);
    append_len_prefixed(&mut bytes, relationship_name.0 .0.as_bytes())?;
    Ok(bytes)
}

/// Encodes a version-2 SmartLink tag using only substrate-independent bytes and MAP types.
///
/// `Some(empty_map)` is normalized to the same canonical representation as `None`.
pub fn encode_link_tag(
    relationship_name: &RelationshipName,
    to_address: &HolonId,
    property_values: Option<&PropertyMap>,
    forward_link_provenance: Option<&LocalId>,
) -> Result<Vec<u8>, SmartLinkTagError> {
    let mut bytes = encode_link_tag_prolog(relationship_name)?;

    match to_address {
        HolonId::Local(_) => bytes.push(LOCAL_REFERENCE_TYPE),
        HolonId::External(external_id) => {
            bytes.push(EXTERNAL_REFERENCE_TYPE);
            append_hash(&mut bytes, &external_id.space_id.0, "proxy id")?;
        }
    }

    match forward_link_provenance {
        Some(action_hash) => {
            bytes.push(HAS_FORWARD_LINK_PROVENANCE);
            append_hash(&mut bytes, action_hash, "forward-link provenance")?;
        }
        None => bytes.push(NO_FORWARD_LINK_PROVENANCE),
    }

    let property_count = property_values.map_or(0, PropertyMap::len);
    append_u32(&mut bytes, property_count)?;
    if let Some(property_map) = property_values {
        for (property_name, value) in property_map {
            append_len_prefixed(&mut bytes, property_name.0 .0.as_bytes())?;
            let (value_kind, value_bytes) = encode_value(value);
            bytes.push(value_kind);
            append_len_prefixed(&mut bytes, &value_bytes)?;
        }
    }

    Ok(bytes)
}

/// Decodes a version-2 SmartLink tag.
pub fn decode_link_tag(bytes: &[u8]) -> Result<LinkTagObject, SmartLinkTagError> {
    let mut cursor = ByteCursor::new(bytes);
    if cursor.read_exact(SMARTLINK_HEADER_BYTES.len())? != SMARTLINK_HEADER_BYTES {
        return Err(SmartLinkTagError::InvalidHeader);
    }

    let version = cursor.read_u8()?;
    if version != SMARTLINK_TAG_FORMAT_VERSION_V2 {
        return Err(SmartLinkTagError::UnsupportedVersion(version));
    }

    let relationship_name = read_utf8(&mut cursor, "relationship name")?;
    let proxy_id = match cursor.read_u8()? {
        LOCAL_REFERENCE_TYPE => None,
        EXTERNAL_REFERENCE_TYPE => {
            Some(OutboundProxyId(LocalId(cursor.read_exact(HOLOCHAIN_HASH_BYTES)?.to_vec())))
        }
        value => return Err(SmartLinkTagError::InvalidReferenceType(value)),
    };

    let forward_link_provenance = match cursor.read_u8()? {
        NO_FORWARD_LINK_PROVENANCE => None,
        HAS_FORWARD_LINK_PROVENANCE => {
            Some(LocalId(cursor.read_exact(HOLOCHAIN_HASH_BYTES)?.to_vec()))
        }
        value => return Err(SmartLinkTagError::InvalidProvenanceMarker(value)),
    };

    let property_count = cursor.read_u32()? as usize;
    if property_count > cursor.remaining() / MIN_PROPERTY_RECORD_BYTES {
        return Err(SmartLinkTagError::InvalidPropertyCount);
    }

    let mut property_map = BTreeMap::new();
    for _ in 0..property_count {
        let property_name = PropertyName(MapString(read_utf8(&mut cursor, "property name")?));
        // Canonical tags are strictly ascending; the map's maximum key covers every prior key.
        if property_map.last_key_value().is_some_and(|(previous, _)| previous >= &property_name) {
            return Err(SmartLinkTagError::NonCanonicalPropertyOrder);
        }
        let value_kind = cursor.read_u8()?;
        let value_bytes = cursor.read_len_prefixed()?;
        let value = decode_value(value_kind, value_bytes)?;
        property_map.insert(property_name, value);
    }

    if !cursor.is_empty() {
        return Err(SmartLinkTagError::TrailingData);
    }

    Ok(LinkTagObject {
        relationship_name,
        proxy_id,
        forward_link_provenance,
        smart_property_values: (!property_map.is_empty()).then_some(property_map),
    })
}

fn append_hash(
    target: &mut Vec<u8>,
    value: &LocalId,
    field: &'static str,
) -> Result<(), SmartLinkTagError> {
    if value.0.len() != HOLOCHAIN_HASH_BYTES {
        return Err(SmartLinkTagError::InvalidHashLength { field, actual: value.0.len() });
    }
    target.extend_from_slice(&value.0);
    Ok(())
}

fn append_len_prefixed(target: &mut Vec<u8>, value: &[u8]) -> Result<(), SmartLinkTagError> {
    append_u32(target, value.len())?;
    target.extend_from_slice(value);
    Ok(())
}

fn append_u32(target: &mut Vec<u8>, value: usize) -> Result<(), SmartLinkTagError> {
    let value = u32::try_from(value).map_err(|_| SmartLinkTagError::LengthOverflow)?;
    target.extend_from_slice(&value.to_be_bytes());
    Ok(())
}

fn encode_value(value: &BaseValue) -> (u8, Vec<u8>) {
    match value {
        BaseValue::StringValue(value) => (STRING_VALUE_KIND, value.0.as_bytes().to_vec()),
        BaseValue::BooleanValue(value) => (BOOLEAN_VALUE_KIND, vec![u8::from(value.0)]),
        BaseValue::IntegerValue(value) => (INTEGER_VALUE_KIND, value.0.to_be_bytes().to_vec()),
        BaseValue::EnumValue(value) => (ENUM_VALUE_KIND, value.0 .0.as_bytes().to_vec()),
        BaseValue::BytesValue(value) => (BYTES_VALUE_KIND, value.0.clone()),
    }
}

fn decode_value(value_kind: u8, value: &[u8]) -> Result<BaseValue, SmartLinkTagError> {
    match value_kind {
        STRING_VALUE_KIND => {
            Ok(BaseValue::StringValue(MapString(read_utf8_bytes(value, "string property value")?)))
        }
        BOOLEAN_VALUE_KIND => match value {
            [0] => Ok(BaseValue::BooleanValue(MapBoolean(false))),
            [1] => Ok(BaseValue::BooleanValue(MapBoolean(true))),
            _ => Err(SmartLinkTagError::InvalidBooleanValue),
        },
        INTEGER_VALUE_KIND => {
            let integer: [u8; 8] = value
                .try_into()
                .map_err(|_| SmartLinkTagError::InvalidIntegerLength(value.len()))?;
            Ok(BaseValue::IntegerValue(MapInteger(i64::from_be_bytes(integer))))
        }
        ENUM_VALUE_KIND => Ok(BaseValue::EnumValue(MapEnumValue(MapString(read_utf8_bytes(
            value,
            "enum property value",
        )?)))),
        BYTES_VALUE_KIND => Ok(BaseValue::BytesValue(MapBytes(value.to_vec()))),
        value => Err(SmartLinkTagError::InvalidValueKind(value)),
    }
}

fn read_utf8(
    cursor: &mut ByteCursor<'_>,
    field: &'static str,
) -> Result<String, SmartLinkTagError> {
    read_utf8_bytes(cursor.read_len_prefixed()?, field)
}

fn read_utf8_bytes(value: &[u8], field: &'static str) -> Result<String, SmartLinkTagError> {
    String::from_utf8(value.to_vec()).map_err(|_| SmartLinkTagError::InvalidUtf8(field))
}

struct ByteCursor<'a> {
    bytes: &'a [u8],
    offset: usize,
}

impl<'a> ByteCursor<'a> {
    fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, offset: 0 }
    }

    fn read_u8(&mut self) -> Result<u8, SmartLinkTagError> {
        Ok(self.read_exact(1)?[0])
    }

    fn read_u32(&mut self) -> Result<u32, SmartLinkTagError> {
        let value: [u8; 4] = self
            .read_exact(4)?
            .try_into()
            .expect("a four-byte slice converts to a four-byte array");
        Ok(u32::from_be_bytes(value))
    }

    fn read_len_prefixed(&mut self) -> Result<&'a [u8], SmartLinkTagError> {
        let length = self.read_u32()? as usize;
        self.read_exact(length)
    }

    fn read_exact(&mut self, length: usize) -> Result<&'a [u8], SmartLinkTagError> {
        let end = self.offset.checked_add(length).ok_or(SmartLinkTagError::UnexpectedEnd)?;
        let value = self.bytes.get(self.offset..end).ok_or(SmartLinkTagError::UnexpectedEnd)?;
        self.offset = end;
        Ok(value)
    }

    fn remaining(&self) -> usize {
        self.bytes.len() - self.offset
    }

    fn is_empty(&self) -> bool {
        self.offset == self.bytes.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use core_types::ExternalId;

    fn raw_hash(seed: u8) -> LocalId {
        let mut bytes = vec![seed; HOLOCHAIN_HASH_BYTES];
        bytes[7] = 0;
        LocalId(bytes)
    }

    fn relationship_name() -> RelationshipName {
        RelationshipName(MapString("RelatedTo".to_string()))
    }

    fn all_value_kinds() -> PropertyMap {
        BTreeMap::from([
            (
                PropertyName(MapString("String".to_string())),
                BaseValue::StringValue(MapString("text\0value".to_string())),
            ),
            (
                PropertyName(MapString("Boolean".to_string())),
                BaseValue::BooleanValue(MapBoolean(true)),
            ),
            (
                PropertyName(MapString("Integer".to_string())),
                BaseValue::IntegerValue(MapInteger(-1)),
            ),
            (
                PropertyName(MapString("Enum".to_string())),
                BaseValue::EnumValue(MapEnumValue(MapString("Active".to_string()))),
            ),
            (
                PropertyName(MapString("Bytes".to_string())),
                BaseValue::BytesValue(MapBytes(vec![0, 1, 0, 255])),
            ),
        ])
    }

    #[test]
    fn round_trips_all_value_kinds_and_raw_hashes() {
        let proxy_id = OutboundProxyId(raw_hash(10));
        let target =
            HolonId::External(ExternalId { space_id: proxy_id.clone(), local_id: raw_hash(11) });
        let properties = all_value_kinds();
        let provenance = raw_hash(12);

        let bytes =
            encode_link_tag(&relationship_name(), &target, Some(&properties), Some(&provenance))
                .expect("v2 tag should encode");
        let decoded = decode_link_tag(&bytes).expect("v2 tag should decode");

        assert_eq!(decoded.relationship_name, "RelatedTo");
        assert_eq!(decoded.proxy_id, Some(proxy_id));
        assert_eq!(decoded.forward_link_provenance, Some(provenance));
        assert_eq!(decoded.smart_property_values, Some(properties));
    }

    #[test]
    fn normalizes_empty_property_maps_to_the_absent_form() {
        let target = HolonId::Local(raw_hash(1));
        let empty_properties = PropertyMap::new();

        let absent = encode_link_tag(&relationship_name(), &target, None, None)
            .expect("absent properties should encode");
        let empty = encode_link_tag(&relationship_name(), &target, Some(&empty_properties), None)
            .expect("empty properties should encode");

        assert_eq!(absent, empty);
        assert_eq!(
            decode_link_tag(&empty).expect("empty tag should decode").smart_property_values,
            None
        );
    }

    #[test]
    fn encodes_equivalent_property_maps_to_identical_tags() {
        let target = HolonId::Local(raw_hash(2));
        let first_property =
            (PropertyName(MapString("Alpha".to_string())), BaseValue::IntegerValue(MapInteger(1)));
        let second_property = (
            PropertyName(MapString("Beta".to_string())),
            BaseValue::BytesValue(MapBytes(vec![0, 2, 0])),
        );

        let mut ascending = PropertyMap::new();
        ascending.insert(first_property.0.clone(), first_property.1.clone());
        ascending.insert(second_property.0.clone(), second_property.1.clone());

        let mut descending = PropertyMap::new();
        descending.insert(second_property.0, second_property.1);
        descending.insert(first_property.0, first_property.1);

        let ascending_tag = encode_link_tag(&relationship_name(), &target, Some(&ascending), None)
            .expect("ascending properties should encode");
        let descending_tag =
            encode_link_tag(&relationship_name(), &target, Some(&descending), None)
                .expect("descending properties should encode");

        assert_eq!(ascending_tag, descending_tag);
    }

    #[test]
    fn prolog_is_deterministic_and_distinguishes_relationship_names() {
        let first = encode_link_tag_prolog(&relationship_name()).expect("prolog should encode");
        let second = encode_link_tag_prolog(&relationship_name()).expect("prolog should encode");
        let different =
            encode_link_tag_prolog(&RelationshipName(MapString("Contains".to_string())))
                .expect("prolog should encode");

        let tag = encode_link_tag(
            &relationship_name(),
            &HolonId::Local(raw_hash(2)),
            Some(&all_value_kinds()),
            None,
        )
        .expect("tag should encode");

        assert_eq!(first, second);
        assert_ne!(first, different);
        assert!(tag.starts_with(&first));
    }

    #[test]
    fn rejects_non_39_byte_proxy_ids() {
        let target = HolonId::External(ExternalId {
            space_id: OutboundProxyId(LocalId(vec![0; HOLOCHAIN_HASH_BYTES - 1])),
            local_id: raw_hash(3),
        });

        assert_eq!(
            encode_link_tag(&relationship_name(), &target, None, None),
            Err(SmartLinkTagError::InvalidHashLength {
                field: "proxy id",
                actual: HOLOCHAIN_HASH_BYTES - 1,
            })
        );
    }

    #[test]
    fn rejects_malformed_boolean_values() {
        let mut bytes = encode_link_tag(
            &relationship_name(),
            &HolonId::Local(raw_hash(4)),
            Some(&BTreeMap::from([(
                PropertyName(MapString("Boolean".to_string())),
                BaseValue::BooleanValue(MapBoolean(true)),
            )])),
            None,
        )
        .expect("tag should encode");

        *bytes.last_mut().expect("encoded boolean has a value byte") = 2;

        assert_eq!(decode_link_tag(&bytes), Err(SmartLinkTagError::InvalidBooleanValue));
    }

    #[test]
    fn rejects_non_canonical_property_order() {
        let mut bytes = encode_link_tag_prolog(&relationship_name()).expect("prolog should encode");
        bytes.push(LOCAL_REFERENCE_TYPE);
        bytes.push(NO_FORWARD_LINK_PROVENANCE);
        append_u32(&mut bytes, 2).expect("property count should fit");

        append_test_property(
            &mut bytes,
            "Beta",
            &BaseValue::StringValue(MapString("second".to_string())),
        );
        append_test_property(
            &mut bytes,
            "Alpha",
            &BaseValue::StringValue(MapString("first".to_string())),
        );

        assert_eq!(decode_link_tag(&bytes), Err(SmartLinkTagError::NonCanonicalPropertyOrder));
    }

    #[test]
    fn rejects_truncated_tags() {
        let mut bytes = SMARTLINK_HEADER_BYTES.to_vec();
        bytes.push(SMARTLINK_TAG_FORMAT_VERSION_V2);

        assert_eq!(decode_link_tag(&bytes), Err(SmartLinkTagError::UnexpectedEnd));
    }

    #[test]
    fn rejects_trailing_data() {
        let mut bytes =
            encode_link_tag(&relationship_name(), &HolonId::Local(raw_hash(5)), None, None)
                .expect("tag should encode");
        bytes.push(0);

        assert_eq!(decode_link_tag(&bytes), Err(SmartLinkTagError::TrailingData));
    }

    #[test]
    fn rejects_legacy_v1_tags() {
        let mut legacy_tag = SMARTLINK_HEADER_BYTES.to_vec();
        legacy_tag.extend_from_slice(b"RelatedTo\0");

        assert_eq!(decode_link_tag(&legacy_tag), Err(SmartLinkTagError::UnsupportedVersion(b'R')));
    }

    fn append_test_property(target: &mut Vec<u8>, name: &str, value: &BaseValue) {
        append_len_prefixed(target, name.as_bytes()).expect("test property name should fit");
        let (value_kind, value_bytes) = encode_value(value);
        target.push(value_kind);
        append_len_prefixed(target, &value_bytes).expect("test property value should fit");
    }
}
