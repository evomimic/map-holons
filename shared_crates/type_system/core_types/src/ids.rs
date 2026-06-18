use integrity_core_types::{short_hash, HolonError, LocalId};
use serde::{Deserialize, Serialize};
use std::fmt;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct OutboundProxyId(pub LocalId);

impl From<LocalId> for OutboundProxyId {
    fn from(action_hash: LocalId) -> Self {
        OutboundProxyId(action_hash)
    }
}

impl fmt::Display for OutboundProxyId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match short_hash(&self.0, 6) {
            Ok(s) => write!(f, "{}", s),
            Err(_) => write!(f, "<invalid utf-8>"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct ExternalId {
    pub space_id: OutboundProxyId,
    pub local_id: LocalId,
}

impl From<(OutboundProxyId, LocalId)> for ExternalId {
    fn from((space_id, local_id): (OutboundProxyId, LocalId)) -> Self {
        ExternalId { space_id, local_id }
    }
}

impl fmt::Display for ExternalId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}::{}", self.space_id, self.local_id)
    }
}

/// Unique identifier for non-persisted Holons, RFC4122 UUID specification.
#[derive(Default, Debug, Clone, Serialize, Deserialize, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct TemporaryId(pub Uuid);

impl fmt::Display for TemporaryId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let id = self.0.hyphenated().to_string();
        let short = id.get(..8).unwrap_or(&id);
        write!(f, "{short}")
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum HolonId {
    Local(LocalId),
    External(ExternalId),
}

impl From<LocalId> for HolonId {
    fn from(local_id: LocalId) -> Self {
        HolonId::Local(local_id)
    }
}

impl From<(OutboundProxyId, LocalId)> for HolonId {
    fn from((space_id, local_id): (OutboundProxyId, LocalId)) -> Self {
        HolonId::External(ExternalId { space_id, local_id })
    }
}

impl HolonId {
    /// Encodes this holon id using the canonical byte representation for
    /// `HolonIdValueType`.
    pub fn to_canonical_bytes(&self) -> Vec<u8> {
        let mut bytes = Vec::new();
        match self {
            HolonId::Local(local_id) => {
                bytes.push(HOLON_ID_LOCAL_TAG);
                append_len_prefixed(&mut bytes, &local_id.0);
            }
            HolonId::External(external_id) => {
                bytes.push(HOLON_ID_EXTERNAL_TAG);
                append_len_prefixed(&mut bytes, &(external_id.space_id.0).0);
                append_len_prefixed(&mut bytes, &external_id.local_id.0);
            }
        }
        bytes
    }

    /// Decodes a holon id from the canonical byte representation for
    /// `HolonIdValueType`.
    pub fn from_canonical_bytes(bytes: &[u8]) -> Result<Self, HolonError> {
        let mut cursor = ByteCursor::new(bytes);
        let tag = cursor.read_tag()?;
        let holon_id = match tag {
            HOLON_ID_LOCAL_TAG => HolonId::Local(cursor.read_local_id()?),
            HOLON_ID_EXTERNAL_TAG => {
                let space_id = OutboundProxyId(cursor.read_local_id()?);
                let local_id = cursor.read_local_id()?;
                HolonId::External(ExternalId { space_id, local_id })
            }
            other => {
                return Err(HolonError::InvalidParameter(format!(
                    "Invalid HolonId bytes tag: {other}"
                )))
            }
        };
        cursor.finish()?;
        Ok(holon_id)
    }

    pub fn is_local(&self) -> bool {
        matches!(self, HolonId::Local(_))
    }

    pub fn is_external(&self) -> bool {
        matches!(self, HolonId::External(_))
    }

    pub fn local_id(&self) -> &LocalId {
        match self {
            HolonId::Local(l) => l,
            HolonId::External(e) => &e.local_id,
        }
    }

    pub fn external_id(&self) -> Option<&ExternalId> {
        if let HolonId::External(e) = self {
            Some(e)
        } else {
            None
        }
    }
}

const HOLON_ID_LOCAL_TAG: u8 = 0;
const HOLON_ID_EXTERNAL_TAG: u8 = 1;

fn append_len_prefixed(target: &mut Vec<u8>, value: &[u8]) {
    target.extend_from_slice(&(value.len() as u32).to_be_bytes());
    target.extend_from_slice(value);
}

struct ByteCursor<'a> {
    bytes: &'a [u8],
    offset: usize,
}

impl<'a> ByteCursor<'a> {
    fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, offset: 0 }
    }

    fn read_tag(&mut self) -> Result<u8, HolonError> {
        let tag = *self.bytes.get(self.offset).ok_or_else(|| {
            HolonError::InvalidParameter("HolonId bytes are missing a tag".to_string())
        })?;
        self.offset += 1;
        Ok(tag)
    }

    fn read_local_id(&mut self) -> Result<LocalId, HolonError> {
        let length = self.read_u32()? as usize;
        let end = self.offset.checked_add(length).ok_or_else(|| {
            HolonError::InvalidParameter("HolonId byte length overflow".to_string())
        })?;
        if end > self.bytes.len() {
            return Err(HolonError::InvalidParameter(
                "HolonId bytes ended before the declared segment length".to_string(),
            ));
        }
        let value = self.bytes[self.offset..end].to_vec();
        self.offset = end;
        Ok(LocalId(value))
    }

    fn read_u32(&mut self) -> Result<u32, HolonError> {
        let end = self.offset.checked_add(4).ok_or_else(|| {
            HolonError::InvalidParameter("HolonId byte length offset overflow".to_string())
        })?;
        if end > self.bytes.len() {
            return Err(HolonError::InvalidParameter(
                "HolonId bytes ended before a segment length".to_string(),
            ));
        }
        let mut length = [0_u8; 4];
        length.copy_from_slice(&self.bytes[self.offset..end]);
        self.offset = end;
        Ok(u32::from_be_bytes(length))
    }

    fn finish(&self) -> Result<(), HolonError> {
        if self.offset == self.bytes.len() {
            return Ok(());
        }
        Err(HolonError::InvalidParameter("HolonId bytes contain trailing data".to_string()))
    }
}

impl fmt::Display for HolonId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            HolonId::Local(l) => write!(f, "Local({})", l),
            HolonId::External(e) => write!(f, "External({})", e),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn canonical_bytes_round_trip_local_holon_id() {
        let expected = HolonId::Local(LocalId(vec![4, 5, 6]));

        let decoded = HolonId::from_canonical_bytes(&expected.to_canonical_bytes())
            .expect("local HolonId should decode");

        assert_eq!(decoded, expected);
    }

    #[test]
    fn canonical_bytes_round_trip_external_holon_id() {
        let expected = HolonId::External(ExternalId {
            space_id: OutboundProxyId(LocalId(vec![1, 2, 3])),
            local_id: LocalId(vec![4, 5, 6]),
        });

        let decoded = HolonId::from_canonical_bytes(&expected.to_canonical_bytes())
            .expect("external HolonId should decode");

        assert_eq!(decoded, expected);
    }

    #[test]
    fn canonical_bytes_rejects_invalid_tag() {
        let error = HolonId::from_canonical_bytes(&[255])
            .expect_err("invalid HolonId tag should be rejected");

        assert!(matches!(
            error,
            HolonError::InvalidParameter(message)
                if message.contains("Invalid HolonId bytes tag")
        ));
    }
}
