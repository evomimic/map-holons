use crate::{DanceRequestWire, DanceResponseWire, SessionStateWire};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct InternalDanceRequestEnvelope {
    pub request: DanceRequestWire,
    pub session: Option<SessionStateWire>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct InternalDanceResponseEnvelope {
    pub response: DanceResponseWire,
    pub session: Option<SessionStateWire>,
}
