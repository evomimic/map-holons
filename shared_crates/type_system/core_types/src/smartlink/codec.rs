use std::{collections::BTreeSet, error::Error, fmt};

use base_types::{BaseValue, MapBoolean, MapBytes, MapEnumValue, MapInteger, MapString};

use crate::{
    CanonicalKey, CanonicalKeyPrefix, DecodedSmartLinkTag, ExternalId, HolonId, LocalId,
    OccurrenceId, OutboundProxyId, PropertyMap, PropertyName, RelationshipName, SmartLinkTagInput,
    TargetPropertyCacheCandidate, MAP_SMARTLINK_V1_MAX_BYTES, SMARTLINK_V1_PACKING_BUDGET_BYTES,
};

/// Stable marker identifying MAP SmartLink tags.
pub const SMARTLINK_HEADER_BYTES: [u8; 3] = [0xE2, 0x82, 0xB7];
/// Payload version implemented by this codec.
pub const SMARTLINK_TAG_VERSION_V1: u8 = 1;
/// Raw byte width of a Holochain action hash.
pub const HOLOCHAIN_ACTION_HASH_BYTES: usize = 39;

const NUL: u8 = 0;
const EXTERNAL_TARGET_FLAG: u8 = 1 << 0;
const OCCURRENCE_ID_FLAG: u8 = 1 << 1;
const KNOWN_FLAGS: u8 = EXTERNAL_TARGET_FLAG | OCCURRENCE_ID_FLAG;
const RELATIONSHIP_PROPERTIES_SECTION: u8 = 1;
const TARGET_PROPERTIES_SECTION: u8 = 2;
const STRING_VALUE_TYPE: u8 = 1;
const BOOLEAN_VALUE_TYPE: u8 = 2;
const INTEGER_VALUE_TYPE: u8 = 3;
const ENUM_VALUE_TYPE: u8 = 4;
const BYTES_VALUE_TYPE: u8 = 5;

/// Structural or packing failure for the SmartLink Tag v1 byte contract.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SmartLinkTagError {
    TagTooLarge { actual: usize, maximum: usize },
    PackingBudgetTooLarge { budget: usize, maximum: usize },
    MandatoryContentExceedsBudget { actual: usize, budget: usize },
    InvalidHeader,
    MissingDelimiter(&'static str),
    InvalidUtf8(&'static str),
    ContainsNul(&'static str),
    UnsupportedVersion(u8),
    UnknownFlags(u8),
    InvalidHashLength { field: &'static str, actual: usize },
    LengthOverflow(&'static str),
    UnexpectedEnd(&'static str),
    UnknownSectionType(u8),
    DuplicateSection(u8),
    NonCanonicalSectionOrder,
    EmptySection(u8),
    SectionBoundaryCrossing,
    NonCanonicalPropertyOrder,
    DuplicateCacheCandidate(String),
    UnknownValueType(u8),
    InvalidBooleanValue,
    InvalidIntegerLength(usize),
}

impl fmt::Display for SmartLinkTagError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TagTooLarge { actual, maximum } => {
                write!(f, "SmartLink tag is {actual} bytes; maximum is {maximum}")
            }
            Self::PackingBudgetTooLarge { budget, maximum } => {
                write!(f, "SmartLink packing budget {budget} exceeds maximum {maximum}")
            }
            Self::MandatoryContentExceedsBudget { actual, budget } => write!(
                f,
                "mandatory SmartLink content is {actual} bytes; packing budget is {budget}"
            ),
            Self::InvalidHeader => write!(f, "invalid SmartLink header"),
            Self::MissingDelimiter(field) => write!(f, "SmartLink {field} delimiter is missing"),
            Self::InvalidUtf8(field) => write!(f, "SmartLink {field} is not valid UTF-8"),
            Self::ContainsNul(field) => write!(f, "SmartLink {field} contains NUL"),
            Self::UnsupportedVersion(version) => {
                write!(f, "unsupported SmartLink payload version {version}")
            }
            Self::UnknownFlags(flags) => {
                write!(f, "SmartLink flags contain reserved bits: {flags:#04x}")
            }
            Self::InvalidHashLength { field, actual } => write!(
                f,
                "SmartLink {field} must be {HOLOCHAIN_ACTION_HASH_BYTES} bytes, got {actual}"
            ),
            Self::LengthOverflow(field) => write!(f, "SmartLink {field} exceeds its u16 length"),
            Self::UnexpectedEnd(field) => write!(f, "SmartLink ended while reading {field}"),
            Self::UnknownSectionType(section) => {
                write!(f, "unknown SmartLink property section {section}")
            }
            Self::DuplicateSection(section) => {
                write!(f, "duplicate SmartLink property section {section}")
            }
            Self::NonCanonicalSectionOrder => {
                write!(f, "SmartLink property sections are not in canonical order")
            }
            Self::EmptySection(section) => {
                write!(f, "SmartLink property section {section} is empty")
            }
            Self::SectionBoundaryCrossing => {
                write!(f, "SmartLink property entry crosses its section boundary")
            }
            Self::NonCanonicalPropertyOrder => {
                write!(f, "SmartLink properties are not in canonical order")
            }
            Self::DuplicateCacheCandidate(name) => {
                write!(f, "duplicate SmartLink cache candidate {name}")
            }
            Self::UnknownValueType(value_type) => {
                write!(f, "unknown SmartLink value type {value_type}")
            }
            Self::InvalidBooleanValue => write!(f, "invalid SmartLink boolean encoding"),
            Self::InvalidIntegerLength(length) => {
                write!(f, "SmartLink integer has noncanonical length {length}")
            }
        }
    }
}

impl Error for SmartLinkTagError {}

/// Encodes a SmartLink Tag v1 using the active writer packing budget.
pub fn encode_smartlink_tag(input: &SmartLinkTagInput) -> Result<Vec<u8>, SmartLinkTagError> {
    encode_smartlink_tag_with_budget(input, SMARTLINK_V1_PACKING_BUDGET_BYTES)
}

/// Encodes a SmartLink Tag v1 using an explicit writer-policy budget.
///
/// This entry point makes packing policy testable without changing wire-format
/// validity. The budget may not exceed the Tag v1 validity ceiling.
pub fn encode_smartlink_tag_with_budget(
    input: &SmartLinkTagInput,
    budget: usize,
) -> Result<Vec<u8>, SmartLinkTagError> {
    if budget > MAP_SMARTLINK_V1_MAX_BYTES {
        return Err(SmartLinkTagError::PackingBudgetTooLarge {
            budget,
            maximum: MAP_SMARTLINK_V1_MAX_BYTES,
        });
    }

    validate_target_hashes(&input.target_id)?;
    validate_prefix_segment("relationship name", relationship_bytes(&input.relationship_name))?;

    let mut seen_candidates = BTreeSet::new();
    for candidate in &input.target_property_cache_candidates {
        let name = candidate.property_name.0 .0.clone();
        if !seen_candidates.insert(candidate.property_name.clone()) {
            return Err(SmartLinkTagError::DuplicateCacheCandidate(name));
        }
    }

    let mut admitted = PropertyMap::new();
    let mandatory = encode_selected(input, &admitted)?;
    if mandatory.len() > budget {
        return Err(SmartLinkTagError::MandatoryContentExceedsBudget {
            actual: mandatory.len(),
            budget,
        });
    }

    let mut encoded = mandatory;
    for TargetPropertyCacheCandidate { property_name, value } in
        &input.target_property_cache_candidates
    {
        // An unrepresentable optional entry cannot fit any valid v1 packing budget.
        if encode_property_entry(property_name, value).is_err() {
            continue;
        }
        admitted.insert(property_name.clone(), value.clone());
        match encode_selected(input, &admitted) {
            Ok(candidate_encoding) if candidate_encoding.len() <= budget => {
                encoded = candidate_encoding;
            }
            Ok(_) | Err(SmartLinkTagError::LengthOverflow(_)) => {
                admitted.remove(property_name);
            }
            Err(error) => return Err(error),
        }
    }

    Ok(encoded)
}

/// Decodes Tag v1 bytes using the Holochain link target as local target identity.
pub fn decode_smartlink_tag(
    bytes: &[u8],
    link_target: LocalId,
) -> Result<DecodedSmartLinkTag, SmartLinkTagError> {
    if bytes.len() > MAP_SMARTLINK_V1_MAX_BYTES {
        return Err(SmartLinkTagError::TagTooLarge {
            actual: bytes.len(),
            maximum: MAP_SMARTLINK_V1_MAX_BYTES,
        });
    }
    validate_hash("link target", &link_target)?;

    let mut cursor = ByteCursor::new(bytes);
    if cursor.read_exact(SMARTLINK_HEADER_BYTES.len(), "header")? != SMARTLINK_HEADER_BYTES {
        return Err(SmartLinkTagError::InvalidHeader);
    }

    let relationship_name =
        RelationshipName(MapString(read_delimited_utf8(&mut cursor, "relationship name")?));
    // Delimiter scanning excludes NUL from the parsed value; construction keeps
    // the CanonicalKey invariant explicit at this boundary.
    let canonical_key = CanonicalKey::new(read_delimited_utf8(&mut cursor, "canonical key")?)
        .map_err(|_| SmartLinkTagError::ContainsNul("canonical key"))?;

    let version = cursor.read_u8("payload version")?;
    if version != SMARTLINK_TAG_VERSION_V1 {
        return Err(SmartLinkTagError::UnsupportedVersion(version));
    }
    let flags = cursor.read_u8("payload flags")?;
    if flags & !KNOWN_FLAGS != 0 {
        return Err(SmartLinkTagError::UnknownFlags(flags));
    }

    let target_id = if flags & EXTERNAL_TARGET_FLAG != 0 {
        let proxy =
            LocalId(cursor.read_exact(HOLOCHAIN_ACTION_HASH_BYTES, "outbound proxy id")?.to_vec());
        HolonId::External(ExternalId { space_id: OutboundProxyId(proxy), local_id: link_target })
    } else {
        HolonId::Local(link_target)
    };

    let occurrence_id = if flags & OCCURRENCE_ID_FLAG != 0 {
        let value: [u8; 16] = cursor
            .read_exact(16, "occurrence id")?
            .try_into()
            .expect("a 16-byte slice converts to a 16-byte array");
        Some(OccurrenceId(value))
    } else {
        None
    };

    let mut relationship_property_values = PropertyMap::new();
    let mut target_property_values = PropertyMap::new();
    let mut previous_section = None;
    while !cursor.is_empty() {
        let section_type = cursor.read_u8("section type")?;
        if !matches!(section_type, RELATIONSHIP_PROPERTIES_SECTION | TARGET_PROPERTIES_SECTION) {
            return Err(SmartLinkTagError::UnknownSectionType(section_type));
        }
        if previous_section == Some(section_type) {
            return Err(SmartLinkTagError::DuplicateSection(section_type));
        }
        if previous_section.is_some_and(|previous| previous > section_type) {
            return Err(SmartLinkTagError::NonCanonicalSectionOrder);
        }
        previous_section = Some(section_type);

        let section_length = cursor.read_u16("section length")? as usize;
        if section_length == 0 {
            return Err(SmartLinkTagError::EmptySection(section_type));
        }
        let section_bytes = cursor
            .read_exact(section_length, "section payload")
            .map_err(|_| SmartLinkTagError::SectionBoundaryCrossing)?;
        let properties = decode_property_section(section_bytes)?;
        match section_type {
            RELATIONSHIP_PROPERTIES_SECTION => relationship_property_values = properties,
            TARGET_PROPERTIES_SECTION => target_property_values = properties,
            _ => unreachable!("section type was checked"),
        }
    }

    Ok(DecodedSmartLinkTag {
        target_id,
        relationship_name,
        canonical_key,
        occurrence_id,
        relationship_property_values,
        target_property_values,
    })
}

/// Constructs the relationship-only query prefix from the Tag v1 grammar.
pub fn smartlink_relationship_prefix(
    relationship_name: &RelationshipName,
) -> Result<Vec<u8>, SmartLinkTagError> {
    let mut bytes = SMARTLINK_HEADER_BYTES.to_vec();
    append_prefix_segment(&mut bytes, "relationship name", relationship_bytes(relationship_name))?;
    bytes.push(NUL);
    Ok(bytes)
}

/// Constructs a relationship plus canonical-key-prefix query prefix.
pub fn smartlink_key_prefix(
    relationship_name: &RelationshipName,
    key_prefix: &CanonicalKeyPrefix,
) -> Result<Vec<u8>, SmartLinkTagError> {
    let mut bytes = smartlink_relationship_prefix(relationship_name)?;
    bytes.extend_from_slice(key_prefix.as_str().as_bytes());
    Ok(bytes)
}

/// Constructs a relationship plus exact-canonical-key query prefix.
pub fn smartlink_exact_key_prefix(
    relationship_name: &RelationshipName,
    canonical_key: &CanonicalKey,
) -> Result<Vec<u8>, SmartLinkTagError> {
    let mut bytes = smartlink_relationship_prefix(relationship_name)?;
    bytes.extend_from_slice(canonical_key.as_str().as_bytes());
    bytes.push(NUL);
    Ok(bytes)
}

fn encode_selected(
    input: &SmartLinkTagInput,
    target_properties: &PropertyMap,
) -> Result<Vec<u8>, SmartLinkTagError> {
    let mut bytes = smartlink_exact_key_prefix(&input.relationship_name, &input.canonical_key)?;
    bytes.push(SMARTLINK_TAG_VERSION_V1);

    let mut flags = 0;
    if input.target_id.is_external() {
        flags |= EXTERNAL_TARGET_FLAG;
    }
    if input.occurrence_id.is_some() {
        flags |= OCCURRENCE_ID_FLAG;
    }
    bytes.push(flags);

    if let HolonId::External(external_id) = &input.target_id {
        bytes.extend_from_slice((external_id.space_id.0).as_bytes());
    }
    if let Some(occurrence_id) = input.occurrence_id {
        bytes.extend_from_slice(&occurrence_id.0);
    }

    append_property_section(
        &mut bytes,
        RELATIONSHIP_PROPERTIES_SECTION,
        &input.relationship_property_values,
    )?;
    append_property_section(&mut bytes, TARGET_PROPERTIES_SECTION, target_properties)?;
    Ok(bytes)
}

fn append_property_section(
    target: &mut Vec<u8>,
    section_type: u8,
    properties: &PropertyMap,
) -> Result<(), SmartLinkTagError> {
    if properties.is_empty() {
        return Ok(());
    }
    let mut payload = Vec::new();
    for (name, value) in properties {
        payload.extend_from_slice(&encode_property_entry(name, value)?);
    }
    target.push(section_type);
    append_u16(target, payload.len(), "section payload")?;
    target.extend_from_slice(&payload);
    Ok(())
}

fn encode_property_entry(
    name: &PropertyName,
    value: &BaseValue,
) -> Result<Vec<u8>, SmartLinkTagError> {
    let mut bytes = Vec::new();
    append_u16(&mut bytes, name.0 .0.len(), "property name")?;
    bytes.extend_from_slice(name.0 .0.as_bytes());
    let (value_type, value_bytes) = encode_value(value);
    bytes.push(value_type);
    append_u16(&mut bytes, value_bytes.len(), "property value")?;
    bytes.extend_from_slice(&value_bytes);
    Ok(bytes)
}

fn decode_property_section(bytes: &[u8]) -> Result<PropertyMap, SmartLinkTagError> {
    let mut cursor = ByteCursor::new(bytes);
    let mut properties = PropertyMap::new();
    while !cursor.is_empty() {
        let name_length = cursor
            .read_u16("property name length")
            .map_err(|_| SmartLinkTagError::SectionBoundaryCrossing)?
            as usize;
        let name_bytes = cursor
            .read_exact(name_length, "property name")
            .map_err(|_| SmartLinkTagError::SectionBoundaryCrossing)?;
        let name = PropertyName(MapString(read_utf8(name_bytes, "property name")?));
        if properties.last_key_value().is_some_and(|(previous, _)| previous >= &name) {
            return Err(SmartLinkTagError::NonCanonicalPropertyOrder);
        }
        let value_type = cursor
            .read_u8("property value type")
            .map_err(|_| SmartLinkTagError::SectionBoundaryCrossing)?;
        let value_length = cursor
            .read_u16("property value length")
            .map_err(|_| SmartLinkTagError::SectionBoundaryCrossing)?
            as usize;
        let value_bytes = cursor
            .read_exact(value_length, "property value")
            .map_err(|_| SmartLinkTagError::SectionBoundaryCrossing)?;
        properties.insert(name, decode_value(value_type, value_bytes)?);
    }
    Ok(properties)
}

fn encode_value(value: &BaseValue) -> (u8, Vec<u8>) {
    match value {
        BaseValue::StringValue(value) => (STRING_VALUE_TYPE, value.0.as_bytes().to_vec()),
        BaseValue::BooleanValue(value) => (BOOLEAN_VALUE_TYPE, vec![u8::from(value.0)]),
        BaseValue::IntegerValue(value) => (INTEGER_VALUE_TYPE, value.0.to_be_bytes().to_vec()),
        BaseValue::EnumValue(value) => (ENUM_VALUE_TYPE, value.0 .0.as_bytes().to_vec()),
        BaseValue::BytesValue(value) => (BYTES_VALUE_TYPE, value.0.clone()),
    }
}

fn decode_value(value_type: u8, bytes: &[u8]) -> Result<BaseValue, SmartLinkTagError> {
    match value_type {
        STRING_VALUE_TYPE => {
            Ok(BaseValue::StringValue(MapString(read_utf8(bytes, "string property value")?)))
        }
        BOOLEAN_VALUE_TYPE => match bytes {
            [0] => Ok(BaseValue::BooleanValue(MapBoolean(false))),
            [1] => Ok(BaseValue::BooleanValue(MapBoolean(true))),
            _ => Err(SmartLinkTagError::InvalidBooleanValue),
        },
        INTEGER_VALUE_TYPE => {
            let integer: [u8; 8] = bytes
                .try_into()
                .map_err(|_| SmartLinkTagError::InvalidIntegerLength(bytes.len()))?;
            Ok(BaseValue::IntegerValue(MapInteger(i64::from_be_bytes(integer))))
        }
        ENUM_VALUE_TYPE => Ok(BaseValue::EnumValue(MapEnumValue(MapString(read_utf8(
            bytes,
            "enum property value",
        )?)))),
        BYTES_VALUE_TYPE => Ok(BaseValue::BytesValue(MapBytes(bytes.to_vec()))),
        other => Err(SmartLinkTagError::UnknownValueType(other)),
    }
}

fn relationship_bytes(relationship_name: &RelationshipName) -> &[u8] {
    relationship_name.0 .0.as_bytes()
}

fn append_prefix_segment(
    target: &mut Vec<u8>,
    field: &'static str,
    bytes: &[u8],
) -> Result<(), SmartLinkTagError> {
    validate_prefix_segment(field, bytes)?;
    target.extend_from_slice(bytes);
    Ok(())
}

fn validate_prefix_segment(field: &'static str, bytes: &[u8]) -> Result<(), SmartLinkTagError> {
    if bytes.contains(&NUL) {
        return Err(SmartLinkTagError::ContainsNul(field));
    }
    Ok(())
}

fn validate_target_hashes(target_id: &HolonId) -> Result<(), SmartLinkTagError> {
    validate_hash("target action hash", target_id.local_id())?;
    if let HolonId::External(external_id) = target_id {
        validate_hash("outbound proxy id", &external_id.space_id.0)?;
    }
    Ok(())
}

fn validate_hash(field: &'static str, value: &LocalId) -> Result<(), SmartLinkTagError> {
    if value.as_bytes().len() != HOLOCHAIN_ACTION_HASH_BYTES {
        return Err(SmartLinkTagError::InvalidHashLength { field, actual: value.as_bytes().len() });
    }
    Ok(())
}

fn append_u16(
    target: &mut Vec<u8>,
    value: usize,
    field: &'static str,
) -> Result<(), SmartLinkTagError> {
    let value = u16::try_from(value).map_err(|_| SmartLinkTagError::LengthOverflow(field))?;
    target.extend_from_slice(&value.to_be_bytes());
    Ok(())
}

fn read_delimited_utf8(
    cursor: &mut ByteCursor<'_>,
    field: &'static str,
) -> Result<String, SmartLinkTagError> {
    let bytes = cursor.read_until(NUL).ok_or(SmartLinkTagError::MissingDelimiter(field))?;
    read_utf8(bytes, field)
}

fn read_utf8(bytes: &[u8], field: &'static str) -> Result<String, SmartLinkTagError> {
    String::from_utf8(bytes.to_vec()).map_err(|_| SmartLinkTagError::InvalidUtf8(field))
}

struct ByteCursor<'a> {
    bytes: &'a [u8],
    offset: usize,
}

impl<'a> ByteCursor<'a> {
    fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, offset: 0 }
    }

    fn read_u8(&mut self, field: &'static str) -> Result<u8, SmartLinkTagError> {
        Ok(self.read_exact(1, field)?[0])
    }

    fn read_u16(&mut self, field: &'static str) -> Result<u16, SmartLinkTagError> {
        let bytes: [u8; 2] = self
            .read_exact(2, field)?
            .try_into()
            .expect("a two-byte slice converts to a two-byte array");
        Ok(u16::from_be_bytes(bytes))
    }

    fn read_exact(
        &mut self,
        length: usize,
        field: &'static str,
    ) -> Result<&'a [u8], SmartLinkTagError> {
        let end = self.offset.checked_add(length).ok_or(SmartLinkTagError::UnexpectedEnd(field))?;
        let value =
            self.bytes.get(self.offset..end).ok_or(SmartLinkTagError::UnexpectedEnd(field))?;
        self.offset = end;
        Ok(value)
    }

    fn read_until(&mut self, delimiter: u8) -> Option<&'a [u8]> {
        let relative_end =
            self.bytes.get(self.offset..)?.iter().position(|byte| *byte == delimiter)?;
        let start = self.offset;
        self.offset += relative_end + 1;
        self.bytes.get(start..start + relative_end)
    }

    fn is_empty(&self) -> bool {
        self.offset == self.bytes.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    fn hash(seed: u8) -> LocalId {
        let mut bytes = vec![seed; HOLOCHAIN_ACTION_HASH_BYTES];
        bytes[7] = 0;
        LocalId(bytes)
    }

    fn relationship(name: &str) -> RelationshipName {
        RelationshipName(MapString(name.to_string()))
    }

    fn property_name(name: &str) -> PropertyName {
        PropertyName(MapString(name.to_string()))
    }

    fn string(value: &str) -> BaseValue {
        BaseValue::StringValue(MapString(value.to_string()))
    }

    fn bytes(value: Vec<u8>) -> BaseValue {
        BaseValue::BytesValue(MapBytes(value))
    }

    fn local_input() -> SmartLinkTagInput {
        SmartLinkTagInput {
            target_id: HolonId::Local(hash(1)),
            relationship_name: relationship("RelatedTo"),
            canonical_key: CanonicalKey::new("target-key").unwrap(),
            occurrence_id: None,
            relationship_property_values: PropertyMap::new(),
            target_property_cache_candidates: Vec::new(),
        }
    }

    fn all_values() -> PropertyMap {
        BTreeMap::from([
            (property_name("Boolean"), BaseValue::BooleanValue(MapBoolean(true))),
            (property_name("Bytes"), bytes(vec![0, 1, 0, 255])),
            (
                property_name("Enum"),
                BaseValue::EnumValue(MapEnumValue(MapString("Active".to_string()))),
            ),
            (property_name("Integer"), BaseValue::IntegerValue(MapInteger(-42))),
            (property_name("String"), string("text\0value")),
        ])
    }

    #[test]
    fn round_trips_local_target_all_scalars_and_separate_sections() {
        let mut input = local_input();
        input.relationship_property_values = all_values();
        input.target_property_cache_candidates = vec![TargetPropertyCacheCandidate {
            property_name: property_name("TargetName"),
            value: string("cached"),
        }];

        let encoded = encode_smartlink_tag(&input).unwrap();
        let decoded = decode_smartlink_tag(&encoded, hash(1)).unwrap();

        assert_eq!(decoded.target_id, input.target_id);
        assert_eq!(decoded.relationship_name, input.relationship_name);
        assert_eq!(decoded.canonical_key, input.canonical_key);
        assert_eq!(decoded.relationship_property_values, all_values());
        assert_eq!(
            decoded.target_property_values,
            BTreeMap::from([(property_name("TargetName"), string("cached"))])
        );
    }

    #[test]
    fn round_trips_external_routing_and_occurrence_identity() {
        let mut input = local_input();
        input.target_id =
            HolonId::External(ExternalId { space_id: OutboundProxyId(hash(2)), local_id: hash(3) });
        input.occurrence_id = Some(OccurrenceId([9; 16]));

        let encoded = encode_smartlink_tag(&input).unwrap();
        let decoded = decode_smartlink_tag(&encoded, hash(3)).unwrap();

        assert_eq!(decoded.target_id, input.target_id);
        assert_eq!(decoded.occurrence_id, input.occurrence_id);
    }

    #[test]
    fn round_trips_empty_canonical_key() {
        let mut input = local_input();
        input.canonical_key = CanonicalKey::new("").unwrap();

        let encoded = encode_smartlink_tag(&input).unwrap();
        let decoded = decode_smartlink_tag(&encoded, hash(1)).unwrap();

        assert_eq!(decoded.canonical_key.as_str(), "");
        assert!(encoded.starts_with(
            &smartlink_exact_key_prefix(&input.relationship_name, &input.canonical_key).unwrap()
        ));
    }

    #[test]
    fn constructs_all_prefix_forms_including_empty_exact_key() {
        let name = relationship("Contains");
        let relationship_prefix = smartlink_relationship_prefix(&name).unwrap();
        assert_eq!(
            relationship_prefix,
            [SMARTLINK_HEADER_BYTES.as_slice(), b"Contains\0"].concat()
        );

        let key_prefix =
            smartlink_key_prefix(&name, &CanonicalKeyPrefix::new("abc").unwrap()).unwrap();
        assert_eq!(key_prefix, [relationship_prefix.as_slice(), b"abc"].concat());

        let exact = smartlink_exact_key_prefix(&name, &CanonicalKey::new("abc").unwrap()).unwrap();
        assert_eq!(exact, [relationship_prefix.as_slice(), b"abc\0"].concat());

        let empty = smartlink_exact_key_prefix(&name, &CanonicalKey::new("").unwrap()).unwrap();
        assert!(empty.ends_with(b"Contains\0\0"));
    }

    #[test]
    fn rejects_nul_in_relationship_name_and_invalid_hash_widths() {
        let mut input = local_input();
        input.relationship_name = relationship("bad\0name");
        assert_eq!(
            encode_smartlink_tag(&input),
            Err(SmartLinkTagError::ContainsNul("relationship name"))
        );

        input.relationship_name = relationship("Valid");
        input.target_id = HolonId::Local(LocalId(vec![0; 38]));
        assert!(matches!(
            encode_smartlink_tag(&input),
            Err(SmartLinkTagError::InvalidHashLength { .. })
        ));
        assert!(matches!(
            decode_smartlink_tag(&[0; 8], LocalId(vec![0; 38])),
            Err(SmartLinkTagError::InvalidHashLength { .. })
        ));
    }

    #[test]
    fn packs_candidates_by_priority_but_encodes_admitted_names_canonically() {
        let mut input = local_input();
        input.target_property_cache_candidates = vec![
            TargetPropertyCacheCandidate {
                property_name: property_name("Zulu"),
                value: bytes(vec![7; 90]),
            },
            TargetPropertyCacheCandidate {
                property_name: property_name("Alpha"),
                value: string("fits"),
            },
        ];
        let mandatory_len = encode_selected(&input, &PropertyMap::new()).unwrap().len();
        let alpha_entry_len =
            encode_property_entry(&property_name("Alpha"), &string("fits")).unwrap().len();
        let budget = mandatory_len + 3 + alpha_entry_len;

        let encoded = encode_smartlink_tag_with_budget(&input, budget).unwrap();
        let decoded = decode_smartlink_tag(&encoded, hash(1)).unwrap();

        assert_eq!(
            decoded.target_property_values,
            BTreeMap::from([(property_name("Alpha"), string("fits"))])
        );
    }

    #[test]
    fn rejects_duplicate_cache_candidates_before_packing() {
        let mut input = local_input();
        input.target_property_cache_candidates = vec![
            TargetPropertyCacheCandidate {
                property_name: property_name("Same"),
                value: string("first"),
            },
            TargetPropertyCacheCandidate {
                property_name: property_name("Same"),
                value: string("second"),
            },
        ];

        assert_eq!(
            encode_smartlink_tag(&input),
            Err(SmartLinkTagError::DuplicateCacheCandidate("Same".to_string()))
        );
    }

    #[test]
    fn equivalent_property_maps_produce_identical_bytes() {
        let mut first = local_input();
        first.relationship_property_values.insert(property_name("Beta"), string("2"));
        first.relationship_property_values.insert(property_name("Alpha"), string("1"));
        let mut second = local_input();
        second.relationship_property_values.insert(property_name("Alpha"), string("1"));
        second.relationship_property_values.insert(property_name("Beta"), string("2"));

        assert_eq!(encode_smartlink_tag(&first).unwrap(), encode_smartlink_tag(&second).unwrap());
    }

    #[test]
    fn mandatory_content_must_fit_and_budget_cannot_exceed_ceiling() {
        let input = local_input();
        let mandatory = encode_selected(&input, &PropertyMap::new()).unwrap();
        assert_eq!(
            encode_smartlink_tag_with_budget(&input, mandatory.len() - 1),
            Err(SmartLinkTagError::MandatoryContentExceedsBudget {
                actual: mandatory.len(),
                budget: mandatory.len() - 1,
            })
        );
        assert!(matches!(
            encode_smartlink_tag_with_budget(&input, MAP_SMARTLINK_V1_MAX_BYTES + 1),
            Err(SmartLinkTagError::PackingBudgetTooLarge { .. })
        ));
    }

    #[test]
    fn enforces_packing_boundary_and_decode_ceiling_independently() {
        let mut input = local_input();
        let base_len = encode_selected(&input, &PropertyMap::new()).unwrap().len();
        let entry_overhead =
            encode_property_entry(&property_name("x"), &bytes(Vec::new())).unwrap().len();
        let value_len = MAP_SMARTLINK_V1_MAX_BYTES - base_len - 3 - entry_overhead;
        input.target_property_cache_candidates.push(TargetPropertyCacheCandidate {
            property_name: property_name("x"),
            value: bytes(vec![0; value_len]),
        });

        let exact = encode_smartlink_tag_with_budget(&input, MAP_SMARTLINK_V1_MAX_BYTES).unwrap();
        assert_eq!(exact.len(), MAP_SMARTLINK_V1_MAX_BYTES);
        assert!(decode_smartlink_tag(&exact, hash(1)).is_ok());

        let lower_budget =
            encode_smartlink_tag_with_budget(&input, MAP_SMARTLINK_V1_MAX_BYTES - 1).unwrap();
        assert_eq!(lower_budget.len(), base_len);

        let mut oversized = exact;
        oversized.push(0);
        assert!(matches!(
            decode_smartlink_tag(&oversized, hash(1)),
            Err(SmartLinkTagError::TagTooLarge { .. })
        ));
    }

    #[test]
    fn rejects_bad_header_delimiters_version_and_reserved_flags() {
        let valid = encode_smartlink_tag(&local_input()).unwrap();

        let mut bad_header = valid.clone();
        bad_header[0] = 0;
        assert_eq!(
            decode_smartlink_tag(&bad_header, hash(1)),
            Err(SmartLinkTagError::InvalidHeader)
        );

        let missing_delimiter = [SMARTLINK_HEADER_BYTES.as_slice(), b"Relationship"].concat();
        assert_eq!(
            decode_smartlink_tag(&missing_delimiter, hash(1)),
            Err(SmartLinkTagError::MissingDelimiter("relationship name"))
        );

        let payload_offset = smartlink_exact_key_prefix(
            &local_input().relationship_name,
            &local_input().canonical_key,
        )
        .unwrap()
        .len();
        let mut bad_version = valid.clone();
        bad_version[payload_offset] = 2;
        assert_eq!(
            decode_smartlink_tag(&bad_version, hash(1)),
            Err(SmartLinkTagError::UnsupportedVersion(2))
        );
        let mut bad_flags = valid;
        bad_flags[payload_offset + 1] = 0x80;
        assert_eq!(
            decode_smartlink_tag(&bad_flags, hash(1)),
            Err(SmartLinkTagError::UnknownFlags(0x80))
        );
    }

    #[test]
    fn rejects_unknown_empty_duplicate_and_out_of_order_sections() {
        let base = encode_smartlink_tag(&local_input()).unwrap();

        let mut unknown = base.clone();
        unknown.extend_from_slice(&[3, 0, 1, 0]);
        assert_eq!(
            decode_smartlink_tag(&unknown, hash(1)),
            Err(SmartLinkTagError::UnknownSectionType(3))
        );

        let mut empty = base.clone();
        empty.extend_from_slice(&[1, 0, 0]);
        assert_eq!(decode_smartlink_tag(&empty, hash(1)), Err(SmartLinkTagError::EmptySection(1)));

        let entry = encode_property_entry(&property_name("a"), &string("v")).unwrap();
        let mut duplicate = base.clone();
        append_raw_section(&mut duplicate, 1, &entry);
        append_raw_section(&mut duplicate, 1, &entry);
        assert_eq!(
            decode_smartlink_tag(&duplicate, hash(1)),
            Err(SmartLinkTagError::DuplicateSection(1))
        );

        let mut reversed = base;
        append_raw_section(&mut reversed, 2, &entry);
        append_raw_section(&mut reversed, 1, &entry);
        assert_eq!(
            decode_smartlink_tag(&reversed, hash(1)),
            Err(SmartLinkTagError::NonCanonicalSectionOrder)
        );
    }

    #[test]
    fn rejects_noncanonical_properties_and_scalar_encodings() {
        let base = encode_smartlink_tag(&local_input()).unwrap();
        let beta = encode_property_entry(&property_name("Beta"), &string("b")).unwrap();
        let alpha = encode_property_entry(&property_name("Alpha"), &string("a")).unwrap();
        let mut unordered = base.clone();
        append_raw_section(&mut unordered, 1, &[beta, alpha].concat());
        assert_eq!(
            decode_smartlink_tag(&unordered, hash(1)),
            Err(SmartLinkTagError::NonCanonicalPropertyOrder)
        );

        let duplicate_entry = encode_property_entry(&property_name("Same"), &string("v")).unwrap();
        let mut duplicate = base.clone();
        append_raw_section(&mut duplicate, 1, &[duplicate_entry.clone(), duplicate_entry].concat());
        assert_eq!(
            decode_smartlink_tag(&duplicate, hash(1)),
            Err(SmartLinkTagError::NonCanonicalPropertyOrder)
        );

        let mut invalid_boolean_entry = Vec::new();
        append_u16(&mut invalid_boolean_entry, 1, "test").unwrap();
        invalid_boolean_entry.extend_from_slice(b"b");
        invalid_boolean_entry.push(BOOLEAN_VALUE_TYPE);
        append_u16(&mut invalid_boolean_entry, 1, "test").unwrap();
        invalid_boolean_entry.push(2);
        let mut invalid_boolean = base.clone();
        append_raw_section(&mut invalid_boolean, 1, &invalid_boolean_entry);
        assert_eq!(
            decode_smartlink_tag(&invalid_boolean, hash(1)),
            Err(SmartLinkTagError::InvalidBooleanValue)
        );

        let mut invalid_integer_entry = Vec::new();
        append_u16(&mut invalid_integer_entry, 1, "test").unwrap();
        invalid_integer_entry.extend_from_slice(b"i");
        invalid_integer_entry.push(INTEGER_VALUE_TYPE);
        append_u16(&mut invalid_integer_entry, 1, "test").unwrap();
        invalid_integer_entry.push(0);
        let mut invalid_integer = base;
        append_raw_section(&mut invalid_integer, 1, &invalid_integer_entry);
        assert_eq!(
            decode_smartlink_tag(&invalid_integer, hash(1)),
            Err(SmartLinkTagError::InvalidIntegerLength(1))
        );
    }

    #[test]
    fn rejects_section_boundary_crossing_and_prior_development_formats() {
        let mut crossing = encode_smartlink_tag(&local_input()).unwrap();
        crossing.extend_from_slice(&[1, 0, 4, 0, 10, b'a', 1]);
        assert_eq!(
            decode_smartlink_tag(&crossing, hash(1)),
            Err(SmartLinkTagError::SectionBoundaryCrossing)
        );

        let legacy = [SMARTLINK_HEADER_BYTES.as_slice(), b"RelatedTo\0L\0"].concat();
        assert!(decode_smartlink_tag(&legacy, hash(1)).is_err());

        let interim_v2 =
            [SMARTLINK_HEADER_BYTES.as_slice(), &[2, 0, 0, 0, 9], b"RelatedTo"].concat();
        assert!(decode_smartlink_tag(&interim_v2, hash(1)).is_err());

        let mut trailing = encode_smartlink_tag(&local_input()).unwrap();
        trailing.push(0);
        assert_eq!(
            decode_smartlink_tag(&trailing, hash(1)),
            Err(SmartLinkTagError::UnknownSectionType(0))
        );
    }

    #[test]
    fn rejects_invalid_utf8_unknown_values_and_truncated_fixed_width_fields() {
        let invalid_relationship = [SMARTLINK_HEADER_BYTES.as_slice(), &[0xff, 0]].concat();
        assert_eq!(
            decode_smartlink_tag(&invalid_relationship, hash(1)),
            Err(SmartLinkTagError::InvalidUtf8("relationship name"))
        );

        let missing_key_delimiter = [SMARTLINK_HEADER_BYTES.as_slice(), b"Rel\0key"].concat();
        assert_eq!(
            decode_smartlink_tag(&missing_key_delimiter, hash(1)),
            Err(SmartLinkTagError::MissingDelimiter("canonical key"))
        );

        let mut unknown_value_entry = Vec::new();
        append_u16(&mut unknown_value_entry, 1, "test").unwrap();
        unknown_value_entry.extend_from_slice(b"x");
        unknown_value_entry.push(99);
        append_u16(&mut unknown_value_entry, 0, "test").unwrap();
        let mut unknown_value = encode_smartlink_tag(&local_input()).unwrap();
        append_raw_section(&mut unknown_value, 1, &unknown_value_entry);
        assert_eq!(
            decode_smartlink_tag(&unknown_value, hash(1)),
            Err(SmartLinkTagError::UnknownValueType(99))
        );

        let mut external =
            smartlink_exact_key_prefix(&relationship("Rel"), &CanonicalKey::new("key").unwrap())
                .unwrap();
        external.extend_from_slice(&[SMARTLINK_TAG_VERSION_V1, EXTERNAL_TARGET_FLAG]);
        external.extend_from_slice(&[0; HOLOCHAIN_ACTION_HASH_BYTES - 1]);
        assert_eq!(
            decode_smartlink_tag(&external, hash(1)),
            Err(SmartLinkTagError::UnexpectedEnd("outbound proxy id"))
        );

        let mut occurrence =
            smartlink_exact_key_prefix(&relationship("Rel"), &CanonicalKey::new("key").unwrap())
                .unwrap();
        occurrence.extend_from_slice(&[SMARTLINK_TAG_VERSION_V1, OCCURRENCE_ID_FLAG]);
        occurrence.extend_from_slice(&[0; 15]);
        assert_eq!(
            decode_smartlink_tag(&occurrence, hash(1)),
            Err(SmartLinkTagError::UnexpectedEnd("occurrence id"))
        );
    }

    fn append_raw_section(target: &mut Vec<u8>, section_type: u8, payload: &[u8]) {
        target.push(section_type);
        append_u16(target, payload.len(), "test section").unwrap();
        target.extend_from_slice(payload);
    }
}
