use std::sync::Arc;

use base_types::MapString;
use core_types::{ContentSet, HolonError, HolonId, LocalId};
use holons_boundary::{
    DanceRequestWire, DanceV2InvocationWire, HolonReferenceWire, SmartReferenceWire,
    TransientReferenceWire,
};
use holons_core::core_shared_objects::transactions::{TransactionContext, TxId};
use serde::{Deserialize, Serialize};

use map_commands_contract::{
    TransactionAction, TransactionCommand, VisualizerKind, VisualizerOwner,
    VisualizerSelectionRequest,
};

/// Transaction-scoped wire command.
///
/// Carries a TxId for binding to a live transaction context at runtime.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TransactionCommandWire {
    pub tx_id: TxId,
    pub action: TransactionActionWire,
}

/// Wire-level transaction actions.
///
/// Flat enum per the MAP Commands spec. Policy classification is enforced by
/// `CommandLifecyclePolicy` at runtime, not by enum structure.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum TransactionActionWire {
    /// Commits the transaction.
    Commit,

    /// Explicitly release retained transaction evidence.
    Dispose,

    /// Verify the captured target before acquiring sources.
    CheckLoadTarget {
        space: HolonReferenceWire,
    },

    /// Undoes the last mutation in this transaction.
    UndoLast,

    /// Redoes the last undone mutation in this transaction.
    RedoLast,

    /// Undoes mutations up to the specified marker.
    UndoToMarker {
        marker_id: String,
    },

    /// Redoes mutations up to the specified marker.
    RedoToMarker {
        marker_id: String,
    },

    /// Prepares a transient load request without executing it.
    PrepareHolons {
        content_set: ContentSet,
    },
    /// Loads holons from uploaded/imported file content.
    LoadHolons {
        content_set: ContentSet,
    },

    /// Executes the retained legacy dance ingress within this transaction.
    ///
    /// This wire shape stays operational for compatibility, including
    /// old-world query traversal dances, but is not the preferred
    /// foundation for new command-surface work.
    Dance(DanceRequestWire),

    /// Executes the canonical new-world dance ingress within this transaction.
    DanceV2 {
        invocation: DanceV2InvocationWire,
    },

    SelectVisualizer(VisualizerSelectionRequestWire),
    DiscoverVisualizers {
        request: VisualizerSelectionRequestWire,
        current_selection: Option<HolonReferenceWire>,
    },
    ChooseVisualizer {
        request: VisualizerSelectionRequestWire,
        candidate: HolonReferenceWire,
    },
    SelectVisualizerUsage {
        request: VisualizerSelectionRequestWire,
        selected: HolonReferenceWire,
    },
    RecordVisualizerUse {
        request: VisualizerSelectionRequestWire,
        selected: HolonReferenceWire,
        usage: HolonReferenceWire,
        origin: VisualizerChoiceOriginWire,
    },
    SelectCollectionVisualizer {
        collection: crate::DescribedHolonCollectionWire,
        parent_visualizer: HolonReferenceWire,
        slot: HolonReferenceWire,
    },

    /// Fetches verified bytes for an opaque artifact capability issued by a
    /// materialization Dance in this transaction.
    FetchArtifact {
        handle: MapString,
    },

    // ── Lookup actions ───────────────────────────────────────────────
    /// `get_all_holons()` → `HolonCollection`
    GetAllHolons,

    /// Saved Nursery members projected without cached/staged properties.
    GetCommittedHolons,

    /// `get_saved_holon_by_key(key)` → `SmartReference`
    GetSavedHolonByBaseKey {
        key: MapString,
    },

    /// `get_staged_holon_by_base_key(key)` → `StagedReference`
    GetStagedHolonByBaseKey {
        key: MapString,
    },

    /// `get_staged_holons_by_base_key(key)` → `Vec<StagedReference>`
    ///
    /// This remains the deliberate reference-shaped plural exception.
    GetStagedHolonsByBaseKey {
        key: MapString,
    },

    /// `get_staged_holon_by_versioned_key(key)` → `StagedReference`
    GetStagedHolonByVersionedKey {
        key: MapString,
    },

    /// `get_transient_holon_by_base_key(key)` → `TransientReference`
    GetTransientHolonByBaseKey {
        key: MapString,
    },

    /// `get_transient_holon_by_versioned_key(key)` → `TransientReference`
    GetTransientHolonByVersionedKey {
        key: MapString,
    },

    /// `staged_count()` → `i64`
    GetStagedCount,

    /// `transient_count()` → `i64`
    GetTransientCount,

    // ── Mutation actions ─────────────────────────────────────────────
    /// `new_holon(key)` → `TransientReference`
    NewHolon {
        key: Option<MapString>,
    },

    /// `stage_new_holon(source)` → `StagedReference`
    StageNewHolon {
        source: TransientReferenceWire,
    },

    /// `stage_new_from_clone(original, new_key)` → `StagedReference`
    StageNewFromClone {
        original: HolonReferenceWire,
        new_key: MapString,
    },

    /// `stage_new_version(current_version)` → `StagedReference`
    StageNewVersion {
        current_version: SmartReferenceWire,
    },

    /// `stage_new_version_from_id(holon_id)` → `StagedReference`
    StageNewVersionFromId {
        holon_id: HolonId,
    },

    /// `delete_holon(local_id)` → `()`
    DeleteHolon {
        local_id: LocalId,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum VisualizerKindWire {
    Canvas,
    Node,
    RootedNavigation,
    Collection,
    PropertyMap,
    Property,
    Value,
    /// Composes action slots for an affording holon.
    ActionBar,
    /// Selects an individual action using its Dance descriptor as subject.
    Action,
}

/// Wire form of the current, Holon-backed selection request ingress.
///
/// The wire shape is deliberately request-based even though the current
/// PropertyMap uses the owning holon as its subject. Property and Value use the
/// resolved PropertyDescriptor holon, preserving descriptor provenance across
/// this transport boundary.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct VisualizerSelectionRequestWire {
    pub subject: HolonReferenceWire,
    pub requested_kind: VisualizerKindWire,
    pub owner: VisualizerOwnerWire,
    pub slot: HolonReferenceWire,
    pub theme: HolonReferenceWire,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum VisualizerOwnerWire {
    Visualizer(HolonReferenceWire),
    Dancer(HolonReferenceWire),
}

impl VisualizerSelectionRequestWire {
    pub fn bind(
        self,
        context: &Arc<TransactionContext>,
    ) -> Result<VisualizerSelectionRequest, HolonError> {
        Ok(VisualizerSelectionRequest {
            subject: self.subject.bind(context)?,
            requested_kind: self.requested_kind.into(),
            owner: match self.owner {
                VisualizerOwnerWire::Visualizer(owner) => {
                    VisualizerOwner::Visualizer(owner.bind(context)?)
                }
                VisualizerOwnerWire::Dancer(owner) => VisualizerOwner::Dancer(owner.bind(context)?),
            },
            slot: self.slot.bind(context)?,
            theme: self.theme.bind(context)?,
        })
    }
}

impl From<VisualizerKindWire> for VisualizerKind {
    fn from(value: VisualizerKindWire) -> Self {
        match value {
            VisualizerKindWire::Canvas => Self::Canvas,
            VisualizerKindWire::Node => Self::Node,
            VisualizerKindWire::RootedNavigation => Self::RootedNavigation,
            VisualizerKindWire::Collection => Self::Collection,
            VisualizerKindWire::PropertyMap => Self::PropertyMap,
            VisualizerKindWire::Property => Self::Property,
            VisualizerKindWire::Value => Self::Value,
            VisualizerKindWire::ActionBar => Self::ActionBar,
            VisualizerKindWire::Action => Self::Action,
        }
    }
}

impl From<VisualizerKind> for VisualizerKindWire {
    fn from(value: VisualizerKind) -> Self {
        match value {
            VisualizerKind::Canvas => Self::Canvas,
            VisualizerKind::Node => Self::Node,
            VisualizerKind::RootedNavigation => Self::RootedNavigation,
            VisualizerKind::Collection => Self::Collection,
            VisualizerKind::PropertyMap => Self::PropertyMap,
            VisualizerKind::Property => Self::Property,
            VisualizerKind::Value => Self::Value,
            VisualizerKind::ActionBar => Self::ActionBar,
            VisualizerKind::Action => Self::Action,
        }
    }
}

// ── Binding ─────────────────────────────────────────────────────────

impl TransactionCommandWire {
    /// Binds a transaction wire command to its domain equivalent.
    ///
    /// Requires a pre-resolved `Arc<TransactionContext>` (looked up from
    /// `RuntimeSession.active_transactions` by the caller).
    pub fn bind(self, context: Arc<TransactionContext>) -> Result<TransactionCommand, HolonError> {
        let action = self.action.bind(&context)?;
        Ok(TransactionCommand { context, action })
    }
}

impl TransactionActionWire {
    fn bind(self, context: &Arc<TransactionContext>) -> Result<TransactionAction, HolonError> {
        match self {
            TransactionActionWire::Dispose => Ok(TransactionAction::Dispose),
            TransactionActionWire::CheckLoadTarget { space } => {
                Ok(TransactionAction::CheckLoadTarget { space: space.bind(context)? })
            }
            TransactionActionWire::Commit => Ok(TransactionAction::Commit),
            TransactionActionWire::UndoLast => Ok(TransactionAction::UndoLast),
            TransactionActionWire::RedoLast => Ok(TransactionAction::RedoLast),
            TransactionActionWire::UndoToMarker { marker_id } => {
                Ok(TransactionAction::UndoToMarker { marker_id })
            }
            TransactionActionWire::RedoToMarker { marker_id } => {
                Ok(TransactionAction::RedoToMarker { marker_id })
            }
            TransactionActionWire::PrepareHolons { content_set } => {
                Ok(TransactionAction::PrepareHolons { content_set })
            }
            TransactionActionWire::LoadHolons { content_set } => {
                Ok(TransactionAction::LoadHolons { content_set })
            }
            TransactionActionWire::Dance(request_wire) => {
                Ok(TransactionAction::Dance(request_wire.bind(context)?))
            }
            TransactionActionWire::DanceV2 { invocation } => {
                Ok(TransactionAction::DanceV2 { invocation: invocation.bind(context)? })
            }
            TransactionActionWire::SelectVisualizerUsage { request, selected } => {
                Ok(TransactionAction::SelectVisualizerUsage {
                    request: request.bind(context)?,
                    selected: selected.bind(context)?,
                })
            }
            TransactionActionWire::RecordVisualizerUse { request, selected, usage, origin } => {
                Ok(TransactionAction::RecordVisualizerUse {
                    request: request.bind(context)?,
                    selected: selected.bind(context)?,
                    usage: usage.bind(context)?,
                    origin: origin.into(),
                })
            }
            TransactionActionWire::SelectCollectionVisualizer {
                collection,
                parent_visualizer,
                slot,
            } => Ok(TransactionAction::SelectCollectionVisualizer {
                collection: map_commands_contract::DescribedHolonCollection {
                    members: collection.members.bind(context)?,
                    element_type: collection.element_type.bind(context)?,
                },
                parent_visualizer: parent_visualizer.bind(context)?,
                slot: slot.bind(context)?,
            }),
            TransactionActionWire::SelectVisualizer(request) => {
                Ok(TransactionAction::SelectVisualizer { request: request.bind(context)? })
            }
            TransactionActionWire::DiscoverVisualizers { request, current_selection } => {
                Ok(TransactionAction::DiscoverVisualizers {
                    request: request.bind(context)?,
                    current_selection: current_selection
                        .map(|value| value.bind(context))
                        .transpose()?,
                })
            }
            TransactionActionWire::ChooseVisualizer { request, candidate } => {
                Ok(TransactionAction::ChooseVisualizer {
                    request: request.bind(context)?,
                    candidate: candidate.bind(context)?,
                })
            }
            TransactionActionWire::FetchArtifact { handle } => {
                Ok(TransactionAction::FetchArtifact { handle })
            }
            // Lookup actions — no context binding needed
            TransactionActionWire::GetAllHolons => Ok(TransactionAction::GetAllHolons),
            TransactionActionWire::GetSavedHolonByBaseKey { key } => {
                Ok(TransactionAction::GetSavedHolonByBaseKey { key })
            }
            TransactionActionWire::GetStagedHolonByBaseKey { key } => {
                Ok(TransactionAction::GetStagedHolonByBaseKey { key })
            }
            TransactionActionWire::GetStagedHolonsByBaseKey { key } => {
                Ok(TransactionAction::GetStagedHolonsByBaseKey { key })
            }
            TransactionActionWire::GetStagedHolonByVersionedKey { key } => {
                Ok(TransactionAction::GetStagedHolonByVersionedKey { key })
            }
            TransactionActionWire::GetTransientHolonByBaseKey { key } => {
                Ok(TransactionAction::GetTransientHolonByBaseKey { key })
            }
            TransactionActionWire::GetTransientHolonByVersionedKey { key } => {
                Ok(TransactionAction::GetTransientHolonByVersionedKey { key })
            }
            TransactionActionWire::GetCommittedHolons => Ok(TransactionAction::GetCommittedHolons),
            TransactionActionWire::GetStagedCount => Ok(TransactionAction::GetStagedCount),
            TransactionActionWire::GetTransientCount => Ok(TransactionAction::GetTransientCount),

            // Mutation actions — some require context binding
            TransactionActionWire::NewHolon { key } => Ok(TransactionAction::NewHolon { key }),
            TransactionActionWire::StageNewHolon { source } => {
                Ok(TransactionAction::StageNewHolon { source: source.bind(context)? })
            }
            TransactionActionWire::StageNewFromClone { original, new_key } => {
                Ok(TransactionAction::StageNewFromClone {
                    original: original.bind(context)?,
                    new_key,
                })
            }
            TransactionActionWire::StageNewVersion { current_version } => {
                Ok(TransactionAction::StageNewVersion {
                    current_version: current_version.bind(context)?,
                })
            }
            TransactionActionWire::StageNewVersionFromId { holon_id } => {
                Ok(TransactionAction::StageNewVersionFromId { holon_id })
            }
            TransactionActionWire::DeleteHolon { local_id } => {
                Ok(TransactionAction::DeleteHolon { local_id })
            }
        }
    }
}

/// Explicit choice is distinguished from automatic and exploratory presentation.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum VisualizerChoiceOriginWire {
    Automatic,
    Explicit,
    Exploratory,
}

impl From<VisualizerChoiceOriginWire> for map_commands_contract::VisualizerChoiceOrigin {
    fn from(value: VisualizerChoiceOriginWire) -> Self {
        match value {
            VisualizerChoiceOriginWire::Automatic => Self::Automatic,
            VisualizerChoiceOriginWire::Explicit => Self::Explicit,
            VisualizerChoiceOriginWire::Exploratory => Self::Exploratory,
        }
    }
}
