use base_types::{MapInteger, MapString};
use core_types::HolonError;
use serde::{Deserialize, Serialize};

use super::MapCommandWire;
use super::MapResultWire;

/// Opaque request identifier assigned by the TypeScript client.
///
/// Echoed back in MapIpcResponse so the client can correlate responses.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RequestId(pub MapInteger);

impl RequestId {
    pub fn new(id: i64) -> Self {
        Self(MapInteger(id))
    }

    pub fn value(&self) -> i64 {
        self.0 .0
    }
}

/// Identifies a user marker for undo/redo grouping.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MarkerId(pub MapString);

/// Per-request options controlling dispatch behavior.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RequestOptions {
    /// Groups this command into a marker for undo/redo.
    pub marker_id: Option<MarkerId>,
    /// Human-readable label for the marker (shown in undo UI).
    pub marker_label: Option<String>,
    /// When true, snapshot pool state after mutation (no-op until Phase 2.3).
    pub snapshot_after: bool,
    /// When true, disables undo for this request.
    pub disable_undo: bool,
}

/// Canonical IPC request envelope for MAP Commands.
///
/// This is the only inbound type accepted by `dispatch_map_command`.
/// It carries a client-assigned request id and a structural command.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MapIpcRequest {
    pub request_id: RequestId,
    pub command: MapCommandWire,
    pub options: RequestOptions,
}

/// Canonical IPC response envelope for MAP Commands.
///
/// Returned by `dispatch_map_command`. The request_id echoes the
/// originating request for client-side correlation.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MapIpcResponse {
    pub request_id: RequestId,
    pub result: Result<MapResultWire, HolonError>,
}
