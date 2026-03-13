use core_types::HolonError;
use serde::{Deserialize, Serialize};

use super::MapCommandWire;
use super::MapResultWire;

/// Opaque request identifier assigned by the TypeScript client.
///
/// Echoed back in MapIpcResponse so the client can correlate responses.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, PartialOrd, Ord)]
pub struct RequestId(u64);

impl RequestId {
    pub fn new(id: u64) -> Self {
        Self(id)
    }

    pub fn value(&self) -> u64 {
        self.0
    }
}

/// Canonical IPC request envelope for MAP Commands.
///
/// This is the only inbound type accepted by `dispatch_map_command`.
/// It carries a client-assigned request id and a structural command.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MapIpcRequest {
    pub request_id: RequestId,
    pub command: MapCommandWire,
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
