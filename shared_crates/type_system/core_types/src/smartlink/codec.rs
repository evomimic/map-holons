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

/// NUL-delimited prefix fields decoded from the SmartLink Tag v1 grammar.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SmartLinkDelimitedField {
    RelationshipName,
    CanonicalKey,
}

impl SmartLinkDelimitedField {
    const fn as_str(self) -> &'static str {
        match self {
            Self::RelationshipName => "relationship name",
            Self::CanonicalKey => "canonical key",
        }
    }
}

/// UTF-8 fields decoded from the SmartLink Tag v1 grammar.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SmartLinkUtf8Field {
    RelationshipName,
    CanonicalKey,
    PropertyName,
    StringPropertyValue,
    EnumPropertyValue,
}

impl SmartLinkUtf8Field {
    const fn as_str(self) -> &'static str {
        match self {
            Self::RelationshipName => "relationship name",
            Self::CanonicalKey => "canonical key",
            Self::PropertyName => "property name",
            Self::StringPropertyValue => "string property value",
            Self::EnumPropertyValue => "enum property value",
        }
    }
}

impl From<SmartLinkDelimitedField> for SmartLinkUtf8Field {
    fn from(value: SmartLinkDelimitedField) -> Self {
        match value {
            SmartLinkDelimitedField::RelationshipName => Self::RelationshipName,
            SmartLinkDelimitedField::CanonicalKey => Self::CanonicalKey,
        }
    }
}

/// Fixed structural positions at which Tag v1 decoding can exhaust its input.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SmartLinkReadPosition {
    TagHeader,
    PayloadVersion,
    PayloadFlags,
    OutboundProxyId,
    OccurrenceId,
    PropertySectionType,
    PropertySection,
}

impl SmartLinkReadPosition {
    const fn as_str(self) -> &'static str {
        match self {
            Self::TagHeader => "header",
            Self::PayloadVersion => "payload version",
            Self::PayloadFlags => "payload flags",
            Self::OutboundProxyId => "outbound proxy id",
            Self::OccurrenceId => "occurrence id",
            Self::PropertySectionType => "section type",
            Self::PropertySection => "section length",
        }
    }
}

/// Length-prefixed fields emitted by the Tag v1 encoder.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SmartLinkLengthField {
    PropertyName,
    PropertyValue,
    PropertySection,
}

impl SmartLinkLengthField {
    const fn as_str(self) -> &'static str {
        match self {
            Self::PropertyName => "property name",
            Self::PropertyValue => "property value",
            Self::PropertySection => "section payload",
        }
    }
}

/// Endpoint roles whose byte width is enforced before Tag v1 encoding.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SmartLinkEndpointRole {
    TargetActionHash,
    OutboundProxyId,
}

impl SmartLinkEndpointRole {
    const fn as_str(self) -> &'static str {
        match self {
            Self::TargetActionHash => "target action hash",
            Self::OutboundProxyId => "outbound proxy id",
        }
    }
}

/// Writer-input or packing failure for the SmartLink Tag v1 byte contract.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SmartLinkTagEncodeError {
    PackingBudgetTooLarge { budget: usize, maximum: usize },
    MandatoryContentExceedsBudget { actual: usize, budget: usize },
    RelationshipNameContainsNul,
    InvalidEndpointLength { endpoint: SmartLinkEndpointRole, actual: usize },
    LengthOverflow(SmartLinkLengthField),
    DuplicateCacheCandidate(String),
}

impl fmt::Display for SmartLinkTagEncodeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::PackingBudgetTooLarge { budget, maximum } => {
                write!(f, "SmartLink packing budget {budget} exceeds maximum {maximum}")
            }
            Self::MandatoryContentExceedsBudget { actual, budget } => write!(
                f,
                "mandatory SmartLink content is {actual} bytes; packing budget is {budget}"
            ),
            Self::RelationshipNameContainsNul => {
                write!(f, "SmartLink relationship name contains NUL")
            }
            Self::InvalidEndpointLength { endpoint, actual } => write!(
                f,
                "SmartLink {} must be {HOLOCHAIN_ACTION_HASH_BYTES} bytes, got {actual}",
                endpoint.as_str()
            ),
            Self::LengthOverflow(field) => {
                write!(f, "SmartLink {} exceeds its u16 length", field.as_str())
            }
            Self::DuplicateCacheCandidate(name) => {
                write!(f, "duplicate SmartLink cache candidate {name}")
            }
        }
    }
}

impl Error for SmartLinkTagEncodeError {}

/// Peer-byte structural failure reachable while decoding SmartLink Tag v1.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SmartLinkTagDecodeError {
    TagTooLarge { actual: usize, maximum: usize },
    InvalidHeader,
    MissingDelimiter(SmartLinkDelimitedField),
    InvalidUtf8(SmartLinkUtf8Field),
    UnsupportedVersion(u8),
    UnknownFlags(u8),
    InvalidLinkTargetLength { actual: usize },
    UnexpectedEnd(SmartLinkReadPosition),
    UnknownSectionType(u8),
    DuplicateSection(u8),
    NonCanonicalSectionOrder,
    EmptySection(u8),
    SectionBoundaryCrossing,
    NonCanonicalPropertyOrder,
    UnknownValueType(u8),
    InvalidBooleanValue,
    InvalidIntegerLength(usize),
}

impl fmt::Display for SmartLinkTagDecodeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TagTooLarge { actual, maximum } => {
                write!(f, "SmartLink tag is {actual} bytes; maximum is {maximum}")
            }
            Self::InvalidHeader => write!(f, "invalid SmartLink header"),
            Self::MissingDelimiter(field) => {
                write!(f, "SmartLink {} delimiter is missing", field.as_str())
            }
            Self::InvalidUtf8(field) => {
                write!(f, "SmartLink {} is not valid UTF-8", field.as_str())
            }
            Self::UnsupportedVersion(version) => {
                write!(f, "unsupported SmartLink payload version {version}")
            }
            Self::UnknownFlags(flags) => {
                write!(f, "SmartLink flags contain reserved bits: {flags:#04x}")
            }
            Self::InvalidLinkTargetLength { actual } => write!(
                f,
                "SmartLink link target must be {HOLOCHAIN_ACTION_HASH_BYTES} bytes, got {actual}"
            ),
            Self::UnexpectedEnd(position) => {
                write!(f, "SmartLink ended while reading {}", position.as_str())
            }
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

impl Error for SmartLinkTagDecodeError {}

/// Encodes a SmartLink Tag v1 using the active writer packing budget.
pub fn encode_smartlink_tag(input: &SmartLinkTagInput) -> Result<Vec<u8>, SmartLinkTagEncodeError> {
    encode_smartlink_tag_with_budget(input, SMARTLINK_V1_PACKING_BUDGET_BYTES)
}

/// Encodes a SmartLink Tag v1 using an explicit writer-policy budget.
///
/// This entry point makes packing policy testable without changing wire-format
/// validity. The budget may not exceed the Tag v1 validity ceiling.
pub fn encode_smartlink_tag_with_budget(
    input: &SmartLinkTagInput,
    budget: usize,
) -> Result<Vec<u8>, SmartLinkTagEncodeError> {
    if budget > MAP_SMARTLINK_V1_MAX_BYTES {
        return Err(SmartLinkTagEncodeError::PackingBudgetTooLarge {
            budget,
            maximum: MAP_SMARTLINK_V1_MAX_BYTES,
        });
    }

    validate_target_hashes(&input.target_id)?;
    validate_relationship_name_segment(relationship_bytes(&input.relationship_name))?;

    let mut seen_candidates = BTreeSet::new();
    for candidate in &input.target_property_cache_candidates {
        let name = candidate.property_name.0 .0.clone();
        if !seen_candidates.insert(candidate.property_name.clone()) {
            return Err(SmartLinkTagEncodeError::DuplicateCacheCandidate(name));
        }
    }

    let mut admitted = PropertyMap::new();
    let mandatory = encode_selected(input, &admitted)?;
    if mandatory.len() > budget {
        return Err(SmartLinkTagEncodeError::MandatoryContentExceedsBudget {
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
            Ok(_) | Err(SmartLinkTagEncodeError::LengthOverflow(_)) => {
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
) -> Result<DecodedSmartLinkTag, SmartLinkTagDecodeError> {
    if bytes.len() > MAP_SMARTLINK_V1_MAX_BYTES {
        return Err(SmartLinkTagDecodeError::TagTooLarge {
            actual: bytes.len(),
            maximum: MAP_SMARTLINK_V1_MAX_BYTES,
        });
    }
    if link_target.as_bytes().len() != HOLOCHAIN_ACTION_HASH_BYTES {
        return Err(SmartLinkTagDecodeError::InvalidLinkTargetLength {
            actual: link_target.as_bytes().len(),
        });
    }

    let mut cursor = ByteCursor::new(bytes);
    if cursor.read_exact(SMARTLINK_HEADER_BYTES.len(), SmartLinkReadPosition::TagHeader)?
        != SMARTLINK_HEADER_BYTES
    {
        return Err(SmartLinkTagDecodeError::InvalidHeader);
    }

    let relationship_name = RelationshipName(MapString(read_delimited_utf8(
        &mut cursor,
        SmartLinkDelimitedField::RelationshipName,
    )?));
    // The delimiter scan has already proved this UTF-8 segment contains no NUL,
    // so decoding can preserve the CanonicalKey invariant without a fallible
    // construction path or an encoder-only error.
    let canonical_key = CanonicalKey::from_delimited_segment(read_delimited_utf8(
        &mut cursor,
        SmartLinkDelimitedField::CanonicalKey,
    )?);

    let version = cursor.read_u8(SmartLinkReadPosition::PayloadVersion)?;
    if version != SMARTLINK_TAG_VERSION_V1 {
        return Err(SmartLinkTagDecodeError::UnsupportedVersion(version));
    }
    let flags = cursor.read_u8(SmartLinkReadPosition::PayloadFlags)?;
    if flags & !KNOWN_FLAGS != 0 {
        return Err(SmartLinkTagDecodeError::UnknownFlags(flags));
    }

    let target_id = if flags & EXTERNAL_TARGET_FLAG != 0 {
        let proxy = LocalId(
            cursor
                .read_exact(HOLOCHAIN_ACTION_HASH_BYTES, SmartLinkReadPosition::OutboundProxyId)?
                .to_vec(),
        );
        HolonId::External(ExternalId { space_id: OutboundProxyId(proxy), local_id: link_target })
    } else {
        HolonId::Local(link_target)
    };

    let occurrence_id = if flags & OCCURRENCE_ID_FLAG != 0 {
        let value: [u8; 16] = cursor
            .read_exact(16, SmartLinkReadPosition::OccurrenceId)?
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
        let section_type = cursor.read_u8(SmartLinkReadPosition::PropertySectionType)?;
        if !matches!(section_type, RELATIONSHIP_PROPERTIES_SECTION | TARGET_PROPERTIES_SECTION) {
            return Err(SmartLinkTagDecodeError::UnknownSectionType(section_type));
        }
        if previous_section == Some(section_type) {
            return Err(SmartLinkTagDecodeError::DuplicateSection(section_type));
        }
        if previous_section.is_some_and(|previous| previous > section_type) {
            return Err(SmartLinkTagDecodeError::NonCanonicalSectionOrder);
        }
        previous_section = Some(section_type);

        let section_length = cursor.read_u16(SmartLinkReadPosition::PropertySection)? as usize;
        if section_length == 0 {
            return Err(SmartLinkTagDecodeError::EmptySection(section_type));
        }
        let section_bytes =
            cursor.take(section_length).ok_or(SmartLinkTagDecodeError::SectionBoundaryCrossing)?;
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
) -> Result<Vec<u8>, SmartLinkTagEncodeError> {
    let mut bytes = SMARTLINK_HEADER_BYTES.to_vec();
    append_relationship_name_segment(&mut bytes, relationship_bytes(relationship_name))?;
    bytes.push(NUL);
    Ok(bytes)
}

/// Constructs a relationship plus canonical-key-prefix query prefix.
pub fn smartlink_key_prefix(
    relationship_name: &RelationshipName,
    key_prefix: &CanonicalKeyPrefix,
) -> Result<Vec<u8>, SmartLinkTagEncodeError> {
    let mut bytes = smartlink_relationship_prefix(relationship_name)?;
    bytes.extend_from_slice(key_prefix.as_str().as_bytes());
    Ok(bytes)
}

/// Constructs a relationship plus exact-canonical-key query prefix.
pub fn smartlink_exact_key_prefix(
    relationship_name: &RelationshipName,
    canonical_key: &CanonicalKey,
) -> Result<Vec<u8>, SmartLinkTagEncodeError> {
    let mut bytes = smartlink_relationship_prefix(relationship_name)?;
    bytes.extend_from_slice(canonical_key.as_str().as_bytes());
    bytes.push(NUL);
    Ok(bytes)
}

fn encode_selected(
    input: &SmartLinkTagInput,
    target_properties: &PropertyMap,
) -> Result<Vec<u8>, SmartLinkTagEncodeError> {
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
) -> Result<(), SmartLinkTagEncodeError> {
    if properties.is_empty() {
        return Ok(());
    }
    let mut payload = Vec::new();
    for (name, value) in properties {
        payload.extend_from_slice(&encode_property_entry(name, value)?);
    }
    target.push(section_type);
    append_u16(target, payload.len(), SmartLinkLengthField::PropertySection)?;
    target.extend_from_slice(&payload);
    Ok(())
}

fn encode_property_entry(
    name: &PropertyName,
    value: &BaseValue,
) -> Result<Vec<u8>, SmartLinkTagEncodeError> {
    let mut bytes = Vec::new();
    append_u16(&mut bytes, name.0 .0.len(), SmartLinkLengthField::PropertyName)?;
    bytes.extend_from_slice(name.0 .0.as_bytes());
    let (value_type, value_bytes) = encode_value(value);
    bytes.push(value_type);
    append_u16(&mut bytes, value_bytes.len(), SmartLinkLengthField::PropertyValue)?;
    bytes.extend_from_slice(&value_bytes);
    Ok(bytes)
}

fn decode_property_section(bytes: &[u8]) -> Result<PropertyMap, SmartLinkTagDecodeError> {
    let mut cursor = ByteCursor::new(bytes);
    let mut properties = PropertyMap::new();
    while !cursor.is_empty() {
        // Reads within a declared property section intentionally collapse all
        // truncation into one section-boundary error. Their internal positions
        // are not externally observable Tag v1 decode positions.
        let name_length =
            cursor.take_u16().ok_or(SmartLinkTagDecodeError::SectionBoundaryCrossing)? as usize;
        let name_bytes =
            cursor.take(name_length).ok_or(SmartLinkTagDecodeError::SectionBoundaryCrossing)?;
        let name =
            PropertyName(MapString(read_utf8(name_bytes, SmartLinkUtf8Field::PropertyName)?));
        if properties.last_key_value().is_some_and(|(previous, _)| previous >= &name) {
            return Err(SmartLinkTagDecodeError::NonCanonicalPropertyOrder);
        }
        let value_type =
            cursor.take_u8().ok_or(SmartLinkTagDecodeError::SectionBoundaryCrossing)?;
        let value_length =
            cursor.take_u16().ok_or(SmartLinkTagDecodeError::SectionBoundaryCrossing)? as usize;
        let value_bytes =
            cursor.take(value_length).ok_or(SmartLinkTagDecodeError::SectionBoundaryCrossing)?;
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

fn decode_value(value_type: u8, bytes: &[u8]) -> Result<BaseValue, SmartLinkTagDecodeError> {
    match value_type {
        STRING_VALUE_TYPE => Ok(BaseValue::StringValue(MapString(read_utf8(
            bytes,
            SmartLinkUtf8Field::StringPropertyValue,
        )?))),
        BOOLEAN_VALUE_TYPE => match bytes {
            [0] => Ok(BaseValue::BooleanValue(MapBoolean(false))),
            [1] => Ok(BaseValue::BooleanValue(MapBoolean(true))),
            _ => Err(SmartLinkTagDecodeError::InvalidBooleanValue),
        },
        INTEGER_VALUE_TYPE => {
            let integer: [u8; 8] = bytes
                .try_into()
                .map_err(|_| SmartLinkTagDecodeError::InvalidIntegerLength(bytes.len()))?;
            Ok(BaseValue::IntegerValue(MapInteger(i64::from_be_bytes(integer))))
        }
        ENUM_VALUE_TYPE => Ok(BaseValue::EnumValue(MapEnumValue(MapString(read_utf8(
            bytes,
            SmartLinkUtf8Field::EnumPropertyValue,
        )?)))),
        BYTES_VALUE_TYPE => Ok(BaseValue::BytesValue(MapBytes(bytes.to_vec()))),
        other => Err(SmartLinkTagDecodeError::UnknownValueType(other)),
    }
}

fn relationship_bytes(relationship_name: &RelationshipName) -> &[u8] {
    relationship_name.0 .0.as_bytes()
}

fn append_relationship_name_segment(
    target: &mut Vec<u8>,
    bytes: &[u8],
) -> Result<(), SmartLinkTagEncodeError> {
    validate_relationship_name_segment(bytes)?;
    target.extend_from_slice(bytes);
    Ok(())
}

fn validate_relationship_name_segment(bytes: &[u8]) -> Result<(), SmartLinkTagEncodeError> {
    if bytes.contains(&NUL) {
        return Err(SmartLinkTagEncodeError::RelationshipNameContainsNul);
    }
    Ok(())
}

fn validate_target_hashes(target_id: &HolonId) -> Result<(), SmartLinkTagEncodeError> {
    validate_endpoint_hash(SmartLinkEndpointRole::TargetActionHash, target_id.local_id())?;
    if let HolonId::External(external_id) = target_id {
        validate_endpoint_hash(SmartLinkEndpointRole::OutboundProxyId, &external_id.space_id.0)?;
    }
    Ok(())
}

fn validate_endpoint_hash(
    endpoint: SmartLinkEndpointRole,
    value: &LocalId,
) -> Result<(), SmartLinkTagEncodeError> {
    if value.as_bytes().len() != HOLOCHAIN_ACTION_HASH_BYTES {
        return Err(SmartLinkTagEncodeError::InvalidEndpointLength {
            endpoint,
            actual: value.as_bytes().len(),
        });
    }
    Ok(())
}

fn append_u16(
    target: &mut Vec<u8>,
    value: usize,
    field: SmartLinkLengthField,
) -> Result<(), SmartLinkTagEncodeError> {
    let value = u16::try_from(value).map_err(|_| SmartLinkTagEncodeError::LengthOverflow(field))?;
    target.extend_from_slice(&value.to_be_bytes());
    Ok(())
}

fn read_delimited_utf8(
    cursor: &mut ByteCursor<'_>,
    field: SmartLinkDelimitedField,
) -> Result<String, SmartLinkTagDecodeError> {
    let bytes = cursor.read_until(NUL).ok_or(SmartLinkTagDecodeError::MissingDelimiter(field))?;
    read_utf8(bytes, field.into())
}

fn read_utf8(bytes: &[u8], field: SmartLinkUtf8Field) -> Result<String, SmartLinkTagDecodeError> {
    String::from_utf8(bytes.to_vec()).map_err(|_| SmartLinkTagDecodeError::InvalidUtf8(field))
}

struct ByteCursor<'a> {
    bytes: &'a [u8],
    offset: usize,
}

impl<'a> ByteCursor<'a> {
    fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, offset: 0 }
    }

    fn read_u8(&mut self, position: SmartLinkReadPosition) -> Result<u8, SmartLinkTagDecodeError> {
        Ok(self.read_exact(1, position)?[0])
    }

    fn read_u16(
        &mut self,
        position: SmartLinkReadPosition,
    ) -> Result<u16, SmartLinkTagDecodeError> {
        let bytes: [u8; 2] = self
            .read_exact(2, position)?
            .try_into()
            .expect("a two-byte slice converts to a two-byte array");
        Ok(u16::from_be_bytes(bytes))
    }

    fn read_exact(
        &mut self,
        length: usize,
        position: SmartLinkReadPosition,
    ) -> Result<&'a [u8], SmartLinkTagDecodeError> {
        self.take(length).ok_or(SmartLinkTagDecodeError::UnexpectedEnd(position))
    }

    fn take(&mut self, length: usize) -> Option<&'a [u8]> {
        let end = self.offset.checked_add(length)?;
        let value = self.bytes.get(self.offset..end)?;
        self.offset = end;
        Some(value)
    }

    fn take_u8(&mut self) -> Option<u8> {
        Some(self.take(1)?[0])
    }

    fn take_u16(&mut self) -> Option<u16> {
        let bytes: [u8; 2] = self.take(2)?.try_into().ok()?;
        Some(u16::from_be_bytes(bytes))
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
            Err(SmartLinkTagEncodeError::RelationshipNameContainsNul)
        );

        input.relationship_name = relationship("Valid");
        input.target_id = HolonId::Local(LocalId(vec![0; 38]));
        assert!(matches!(
            encode_smartlink_tag(&input),
            Err(SmartLinkTagEncodeError::InvalidEndpointLength { .. })
        ));
        assert!(matches!(
            decode_smartlink_tag(&[0; 8], LocalId(vec![0; 38])),
            Err(SmartLinkTagDecodeError::InvalidLinkTargetLength { .. })
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
            Err(SmartLinkTagEncodeError::DuplicateCacheCandidate("Same".to_string()))
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
            Err(SmartLinkTagEncodeError::MandatoryContentExceedsBudget {
                actual: mandatory.len(),
                budget: mandatory.len() - 1,
            })
        );
        assert!(matches!(
            encode_smartlink_tag_with_budget(&input, MAP_SMARTLINK_V1_MAX_BYTES + 1),
            Err(SmartLinkTagEncodeError::PackingBudgetTooLarge { .. })
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
            Err(SmartLinkTagDecodeError::TagTooLarge { .. })
        ));
    }

    #[test]
    fn rejects_bad_header_delimiters_version_and_reserved_flags() {
        let valid = encode_smartlink_tag(&local_input()).unwrap();

        let mut bad_header = valid.clone();
        bad_header[0] = 0;
        assert_eq!(
            decode_smartlink_tag(&bad_header, hash(1)),
            Err(SmartLinkTagDecodeError::InvalidHeader)
        );

        let missing_delimiter = [SMARTLINK_HEADER_BYTES.as_slice(), b"Relationship"].concat();
        assert_eq!(
            decode_smartlink_tag(&missing_delimiter, hash(1)),
            Err(SmartLinkTagDecodeError::MissingDelimiter(
                SmartLinkDelimitedField::RelationshipName
            ))
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
            Err(SmartLinkTagDecodeError::UnsupportedVersion(2))
        );
        let mut bad_flags = valid;
        bad_flags[payload_offset + 1] = 0x80;
        assert_eq!(
            decode_smartlink_tag(&bad_flags, hash(1)),
            Err(SmartLinkTagDecodeError::UnknownFlags(0x80))
        );
    }

    #[test]
    fn rejects_unknown_empty_duplicate_and_out_of_order_sections() {
        let base = encode_smartlink_tag(&local_input()).unwrap();

        let mut unknown = base.clone();
        unknown.extend_from_slice(&[3, 0, 1, 0]);
        assert_eq!(
            decode_smartlink_tag(&unknown, hash(1)),
            Err(SmartLinkTagDecodeError::UnknownSectionType(3))
        );

        let mut empty = base.clone();
        empty.extend_from_slice(&[1, 0, 0]);
        assert_eq!(
            decode_smartlink_tag(&empty, hash(1)),
            Err(SmartLinkTagDecodeError::EmptySection(1))
        );

        let entry = encode_property_entry(&property_name("a"), &string("v")).unwrap();
        let mut duplicate = base.clone();
        append_raw_section(&mut duplicate, 1, &entry);
        append_raw_section(&mut duplicate, 1, &entry);
        assert_eq!(
            decode_smartlink_tag(&duplicate, hash(1)),
            Err(SmartLinkTagDecodeError::DuplicateSection(1))
        );

        let mut reversed = base;
        append_raw_section(&mut reversed, 2, &entry);
        append_raw_section(&mut reversed, 1, &entry);
        assert_eq!(
            decode_smartlink_tag(&reversed, hash(1)),
            Err(SmartLinkTagDecodeError::NonCanonicalSectionOrder)
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
            Err(SmartLinkTagDecodeError::NonCanonicalPropertyOrder)
        );

        let duplicate_entry = encode_property_entry(&property_name("Same"), &string("v")).unwrap();
        let mut duplicate = base.clone();
        append_raw_section(&mut duplicate, 1, &[duplicate_entry.clone(), duplicate_entry].concat());
        assert_eq!(
            decode_smartlink_tag(&duplicate, hash(1)),
            Err(SmartLinkTagDecodeError::NonCanonicalPropertyOrder)
        );

        let mut invalid_boolean_entry = Vec::new();
        append_u16(&mut invalid_boolean_entry, 1, SmartLinkLengthField::PropertyName).unwrap();
        invalid_boolean_entry.extend_from_slice(b"b");
        invalid_boolean_entry.push(BOOLEAN_VALUE_TYPE);
        append_u16(&mut invalid_boolean_entry, 1, SmartLinkLengthField::PropertyValue).unwrap();
        invalid_boolean_entry.push(2);
        let mut invalid_boolean = base.clone();
        append_raw_section(&mut invalid_boolean, 1, &invalid_boolean_entry);
        assert_eq!(
            decode_smartlink_tag(&invalid_boolean, hash(1)),
            Err(SmartLinkTagDecodeError::InvalidBooleanValue)
        );

        let mut invalid_integer_entry = Vec::new();
        append_u16(&mut invalid_integer_entry, 1, SmartLinkLengthField::PropertyName).unwrap();
        invalid_integer_entry.extend_from_slice(b"i");
        invalid_integer_entry.push(INTEGER_VALUE_TYPE);
        append_u16(&mut invalid_integer_entry, 1, SmartLinkLengthField::PropertyValue).unwrap();
        invalid_integer_entry.push(0);
        let mut invalid_integer = base;
        append_raw_section(&mut invalid_integer, 1, &invalid_integer_entry);
        assert_eq!(
            decode_smartlink_tag(&invalid_integer, hash(1)),
            Err(SmartLinkTagDecodeError::InvalidIntegerLength(1))
        );
    }

    #[test]
    fn rejects_section_boundary_crossing_and_prior_development_formats() {
        let mut crossing = encode_smartlink_tag(&local_input()).unwrap();
        crossing.extend_from_slice(&[1, 0, 4, 0, 10, b'a', 1]);
        assert_eq!(
            decode_smartlink_tag(&crossing, hash(1)),
            Err(SmartLinkTagDecodeError::SectionBoundaryCrossing)
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
            Err(SmartLinkTagDecodeError::UnknownSectionType(0))
        );
    }

    #[test]
    fn rejects_invalid_utf8_unknown_values_and_truncated_fixed_width_fields() {
        let invalid_relationship = [SMARTLINK_HEADER_BYTES.as_slice(), &[0xff, 0]].concat();
        assert_eq!(
            decode_smartlink_tag(&invalid_relationship, hash(1)),
            Err(SmartLinkTagDecodeError::InvalidUtf8(SmartLinkUtf8Field::RelationshipName))
        );

        let missing_key_delimiter = [SMARTLINK_HEADER_BYTES.as_slice(), b"Rel\0key"].concat();
        assert_eq!(
            decode_smartlink_tag(&missing_key_delimiter, hash(1)),
            Err(SmartLinkTagDecodeError::MissingDelimiter(SmartLinkDelimitedField::CanonicalKey))
        );

        let mut unknown_value_entry = Vec::new();
        append_u16(&mut unknown_value_entry, 1, SmartLinkLengthField::PropertyName).unwrap();
        unknown_value_entry.extend_from_slice(b"x");
        unknown_value_entry.push(99);
        append_u16(&mut unknown_value_entry, 0, SmartLinkLengthField::PropertyValue).unwrap();
        let mut unknown_value = encode_smartlink_tag(&local_input()).unwrap();
        append_raw_section(&mut unknown_value, 1, &unknown_value_entry);
        assert_eq!(
            decode_smartlink_tag(&unknown_value, hash(1)),
            Err(SmartLinkTagDecodeError::UnknownValueType(99))
        );

        let mut external =
            smartlink_exact_key_prefix(&relationship("Rel"), &CanonicalKey::new("key").unwrap())
                .unwrap();
        external.extend_from_slice(&[SMARTLINK_TAG_VERSION_V1, EXTERNAL_TARGET_FLAG]);
        external.extend_from_slice(&[0; HOLOCHAIN_ACTION_HASH_BYTES - 1]);
        assert_eq!(
            decode_smartlink_tag(&external, hash(1)),
            Err(SmartLinkTagDecodeError::UnexpectedEnd(SmartLinkReadPosition::OutboundProxyId))
        );

        let mut occurrence =
            smartlink_exact_key_prefix(&relationship("Rel"), &CanonicalKey::new("key").unwrap())
                .unwrap();
        occurrence.extend_from_slice(&[SMARTLINK_TAG_VERSION_V1, OCCURRENCE_ID_FLAG]);
        occurrence.extend_from_slice(&[0; 15]);
        assert_eq!(
            decode_smartlink_tag(&occurrence, hash(1)),
            Err(SmartLinkTagDecodeError::UnexpectedEnd(SmartLinkReadPosition::OccurrenceId))
        );
    }

    #[test]
    fn decoder_reports_every_reachable_unexpected_end_position() {
        let prefix =
            smartlink_exact_key_prefix(&relationship("Rel"), &CanonicalKey::new("key").unwrap())
                .unwrap();
        let mut flags = prefix.clone();
        flags.push(SMARTLINK_TAG_VERSION_V1);
        let mut outbound_proxy = flags.clone();
        outbound_proxy.push(EXTERNAL_TARGET_FLAG);
        outbound_proxy.extend_from_slice(&[0; HOLOCHAIN_ACTION_HASH_BYTES - 1]);
        let mut occurrence = flags.clone();
        occurrence.push(OCCURRENCE_ID_FLAG);
        occurrence.extend_from_slice(&[0; 15]);
        let mut property_section = encode_smartlink_tag(&local_input()).unwrap();
        property_section.extend_from_slice(&[RELATIONSHIP_PROPERTIES_SECTION, 0]);

        let cases = [
            (
                SMARTLINK_HEADER_BYTES[..SMARTLINK_HEADER_BYTES.len() - 1].to_vec(),
                SmartLinkReadPosition::TagHeader,
            ),
            (prefix, SmartLinkReadPosition::PayloadVersion),
            (flags, SmartLinkReadPosition::PayloadFlags),
            (outbound_proxy, SmartLinkReadPosition::OutboundProxyId),
            (occurrence, SmartLinkReadPosition::OccurrenceId),
            (property_section, SmartLinkReadPosition::PropertySection),
        ];

        for (bytes, position) in cases {
            assert_eq!(
                decode_smartlink_tag(&bytes, hash(1)),
                Err(SmartLinkTagDecodeError::UnexpectedEnd(position))
            );
        }
    }

    #[test]
    fn decoder_reports_invalid_utf8_at_every_text_position() {
        let invalid_relationship = [SMARTLINK_HEADER_BYTES.as_slice(), &[0xff, 0]].concat();
        let invalid_canonical_key =
            [SMARTLINK_HEADER_BYTES.as_slice(), b"Rel\0", &[0xff, 0]].concat();
        let base = encode_smartlink_tag(&local_input()).unwrap();

        let property_tag = |value_type: u8, name: &[u8], value: &[u8]| {
            let mut entry = Vec::new();
            append_u16(&mut entry, name.len(), SmartLinkLengthField::PropertyName).unwrap();
            entry.extend_from_slice(name);
            entry.push(value_type);
            append_u16(&mut entry, value.len(), SmartLinkLengthField::PropertyValue).unwrap();
            entry.extend_from_slice(value);
            let mut tag = base.clone();
            append_raw_section(&mut tag, RELATIONSHIP_PROPERTIES_SECTION, &entry);
            tag
        };

        let cases = [
            (invalid_relationship, SmartLinkUtf8Field::RelationshipName),
            (invalid_canonical_key, SmartLinkUtf8Field::CanonicalKey),
            (property_tag(STRING_VALUE_TYPE, &[0xff], b"value"), SmartLinkUtf8Field::PropertyName),
            (
                property_tag(STRING_VALUE_TYPE, b"name", &[0xff]),
                SmartLinkUtf8Field::StringPropertyValue,
            ),
            (
                property_tag(ENUM_VALUE_TYPE, b"name", &[0xff]),
                SmartLinkUtf8Field::EnumPropertyValue,
            ),
        ];

        for (bytes, field) in cases {
            assert_eq!(
                decode_smartlink_tag(&bytes, hash(1)),
                Err(SmartLinkTagDecodeError::InvalidUtf8(field))
            );
        }
    }

    #[test]
    fn section_overruns_are_normalized_at_tag_and_entry_boundaries() {
        let base = encode_smartlink_tag(&local_input()).unwrap();

        let mut tag_level = base.clone();
        tag_level.extend_from_slice(&[RELATIONSHIP_PROPERTIES_SECTION, 0, 4, 0]);
        assert_eq!(
            decode_smartlink_tag(&tag_level, hash(1)),
            Err(SmartLinkTagDecodeError::SectionBoundaryCrossing)
        );

        let mut entry_level = base;
        entry_level.extend_from_slice(&[
            RELATIONSHIP_PROPERTIES_SECTION,
            0,
            4,
            0,
            10,
            b'a',
            STRING_VALUE_TYPE,
        ]);
        assert_eq!(
            decode_smartlink_tag(&entry_level, hash(1)),
            Err(SmartLinkTagDecodeError::SectionBoundaryCrossing)
        );
    }

    #[test]
    fn decoded_canonical_key_construction_has_no_encode_error_channel() {
        let key: CanonicalKey = CanonicalKey::from_delimited_segment("decoded-key".into());
        assert_eq!(key.as_str(), "decoded-key");
    }

    fn append_raw_section(target: &mut Vec<u8>, section_type: u8, payload: &[u8]) {
        target.push(section_type);
        append_u16(target, payload.len(), SmartLinkLengthField::PropertySection).unwrap();
        target.extend_from_slice(payload);
    }
}
