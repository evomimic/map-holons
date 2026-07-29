use crate::{HolonError, HolonNodeModel, LocalId};
use integrity_core_types::short_hash;
use serde::{Deserialize, Serialize};
use std::fmt;

/// Permanent identity of a holon's version lineage: the `LocalId` of the lineage-root record.
///
/// A distinct newtype rather than a `LocalId` alias because a lineage id and a version id are
/// both action-hash-shaped `LocalId`s. Only the type system can stop one being passed where the
/// other is expected, and confusing them would silently graft one lineage onto another.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct LineageId(pub LocalId);

impl LineageId {
    /// Borrows the underlying `LocalId`, for callers that need to address the root record.
    pub fn as_local_id(&self) -> &LocalId {
        &self.0
    }

    /// Consumes this lineage id, yielding the underlying `LocalId`.
    pub fn into_local_id(self) -> LocalId {
        self.0
    }
}

impl From<LocalId> for LineageId {
    fn from(value: LocalId) -> Self {
        Self(value)
    }
}

impl From<LineageId> for LocalId {
    fn from(value: LineageId) -> Self {
        value.0
    }
}

impl fmt::Display for LineageId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match short_hash(&self.0, 6) {
            Ok(s) => write!(f, "{}", s),
            Err(_) => write!(f, "<invalid utf-8>"),
        }
    }
}

/// Version facts derived from a persisted record, never from entry content.
///
/// A lineage root carries no `lineage_id`: it *is* the root, and its `version_id` names it.
/// Every other version carries the id of the root it descends from, so lineage membership is a
/// property of the record rather than something the entry body has to remember.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VersionMetadata {
    /// Identifies this exact persisted version.
    pub version_id: LocalId,
    /// The lineage this version belongs to; `None` when this version is itself the root.
    pub lineage_id: Option<LineageId>,
}

impl VersionMetadata {
    /// Describes a version that begins a lineage.
    pub fn root(version_id: LocalId) -> Self {
        Self { version_id, lineage_id: None }
    }

    /// Describes a version that supersedes an existing lineage root.
    pub fn derived(version_id: LocalId, lineage_id: LineageId) -> Self {
        Self { version_id, lineage_id: Some(lineage_id) }
    }

    /// Returns true when this version begins its own lineage.
    pub fn is_lineage_root(&self) -> bool {
        self.lineage_id.is_none()
    }

    /// Resolves the lineage this version belongs to.
    ///
    /// A root is its own lineage; any other version reports the root it descends from. Expressing
    /// the rule here means no caller has to re-derive it, and no caller has to know that a root
    /// records its lineage by omission.
    pub fn lineage_root(&self) -> LineageId {
        self.lineage_id.clone().unwrap_or_else(|| LineageId(self.version_id.clone()))
    }
}

/// A persisted holon node paired with the version facts derived from its record.
///
/// This is what the storage boundary returns in place of a substrate record: decoded content
/// plus the metadata that content can no longer be trusted to carry.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StoredHolonNode {
    pub holon_node: HolonNodeModel,
    pub version_metadata: VersionMetadata,
}

impl StoredHolonNode {
    /// Constructs a stored holon node from decoded content and its record-derived metadata.
    pub fn new(holon_node: HolonNodeModel, version_metadata: VersionMetadata) -> Self {
        Self { holon_node, version_metadata }
    }

    /// Borrows this version's exact identity.
    pub fn version_id(&self) -> &LocalId {
        &self.version_metadata.version_id
    }
}

/// A request to persist holon node content, naming the intent rather than the substrate action.
///
/// Callers declare *what kind of write this is*; the storage layer decides which substrate
/// action expresses it. That split is the whole point of SL2: action selection is a storage
/// concern, and no caller should have to know that a version is written as an update rooted at
/// the lineage's first record.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum HolonWriteRequest {
    /// Begin a new lineage.
    PublishRoot { holon_node: HolonNodeModel },
    /// Add a version to the lineage shared by every predecessor.
    PublishVersion { holon_node: HolonNodeModel, predecessor_ids: Vec<LocalId> },
}

impl HolonWriteRequest {
    /// Borrows the content this request will persist, whichever variant it is.
    pub fn holon_node(&self) -> &HolonNodeModel {
        match self {
            HolonWriteRequest::PublishRoot { holon_node } => holon_node,
            HolonWriteRequest::PublishVersion { holon_node, .. } => holon_node,
        }
    }
}

/// Resolves the single lineage root shared by every supplied predecessor.
///
/// Rejects an empty predecessor set, and rejects any set whose members root at different
/// lineages, so a version-producing write can never silently graft one lineage onto another.
/// Duplicate predecessors and a mix of root and derived predecessors from the *same* lineage are
/// all accepted: they agree on the answer.
pub fn resolve_shared_lineage_root(
    predecessors: &[VersionMetadata],
) -> Result<LineageId, HolonError> {
    let mut predecessors = predecessors.iter();

    let first = predecessors.next().ok_or_else(|| {
        HolonError::InvalidParameter(
            "A version-producing write requires at least one predecessor".to_string(),
        )
    })?;
    let root = first.lineage_root();

    for predecessor in predecessors {
        let candidate = predecessor.lineage_root();
        if candidate != root {
            return Err(HolonError::InvalidParameter(format!(
                "Predecessors resolve to different lineage roots: {} and {}",
                root, candidate
            )));
        }
    }

    Ok(root)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::PropertyMap;

    fn local_id(byte: u8) -> LocalId {
        LocalId(vec![byte; 39])
    }

    fn root_version(byte: u8) -> VersionMetadata {
        VersionMetadata::root(local_id(byte))
    }

    fn derived_version(version_byte: u8, root_byte: u8) -> VersionMetadata {
        VersionMetadata::derived(local_id(version_byte), LineageId(local_id(root_byte)))
    }

    #[test]
    fn lineage_root_of_a_root_version_is_its_own_id() {
        let metadata = root_version(1);

        assert!(metadata.is_lineage_root());
        assert_eq!(metadata.lineage_root(), LineageId(local_id(1)));
    }

    #[test]
    fn lineage_root_of_a_derived_version_is_its_recorded_lineage() {
        let metadata = derived_version(2, 1);

        assert!(!metadata.is_lineage_root());
        assert_eq!(metadata.lineage_root(), LineageId(local_id(1)));
    }

    #[test]
    fn resolve_shared_lineage_root_accepts_a_single_root_predecessor() {
        let resolved = resolve_shared_lineage_root(&[root_version(1)])
            .expect("a lone root predecessor should resolve to itself");

        assert_eq!(resolved, LineageId(local_id(1)));
    }

    #[test]
    fn resolve_shared_lineage_root_accepts_a_single_derived_predecessor() {
        let resolved = resolve_shared_lineage_root(&[derived_version(2, 1)])
            .expect("a derived predecessor should resolve to the root it descends from");

        // A second-generation version stays rooted at the original record, never at its
        // immediate predecessor.
        assert_eq!(resolved, LineageId(local_id(1)));
    }

    #[test]
    fn resolve_shared_lineage_root_accepts_duplicate_and_mixed_predecessors_of_one_lineage() {
        let predecessors =
            [root_version(1), derived_version(2, 1), derived_version(2, 1), derived_version(3, 1)];

        let resolved = resolve_shared_lineage_root(&predecessors)
            .expect("predecessors of one lineage should agree on its root");

        assert_eq!(resolved, LineageId(local_id(1)));
    }

    #[test]
    fn resolve_shared_lineage_root_rejects_predecessors_from_different_lineages() {
        let predecessors = [derived_version(2, 1), derived_version(4, 3)];

        let error = resolve_shared_lineage_root(&predecessors)
            .expect_err("predecessors from different lineages should be rejected");

        assert!(matches!(
            error,
            HolonError::InvalidParameter(message)
                if message.contains("different lineage roots")
        ));
    }

    #[test]
    fn resolve_shared_lineage_root_rejects_a_root_mixed_with_a_foreign_lineage() {
        // The root arrives first, so this proves the comparison is not anchored on the
        // derived-version case alone.
        let predecessors = [root_version(1), derived_version(4, 3)];

        let error = resolve_shared_lineage_root(&predecessors)
            .expect_err("a foreign lineage should be rejected regardless of predecessor order");

        assert!(matches!(error, HolonError::InvalidParameter(_)));
    }

    #[test]
    fn resolve_shared_lineage_root_rejects_an_empty_predecessor_set() {
        let error = resolve_shared_lineage_root(&[])
            .expect_err("a version-producing write needs a predecessor");

        assert!(matches!(
            error,
            HolonError::InvalidParameter(message)
                if message.contains("at least one predecessor")
        ));
    }

    #[test]
    fn holon_write_request_exposes_its_content_for_either_variant() {
        let node = HolonNodeModel::new(None, PropertyMap::new());

        let root = HolonWriteRequest::PublishRoot { holon_node: node.clone() };
        let version = HolonWriteRequest::PublishVersion {
            holon_node: node.clone(),
            predecessor_ids: vec![local_id(1)],
        };

        assert_eq!(root.holon_node(), &node);
        assert_eq!(version.holon_node(), &node);
    }

    #[test]
    fn stored_holon_node_reports_its_exact_version_id() {
        let stored =
            StoredHolonNode::new(HolonNodeModel::new(None, PropertyMap::new()), root_version(7));

        assert_eq!(stored.version_id(), &local_id(7));
    }
}
