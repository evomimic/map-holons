use holons_core::{
    dances::{DanceResponse, ResponseBody, ResponseStatusCode, SessionState},
    HolonReference,
};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct MapResponse {
    pub space_id: String,
    pub status_code: ResponseStatusCode,
    pub description: String,
    pub body: ResponseBody,
    pub descriptor: Option<HolonReference>, // space_id+holon_id of DanceDescriptor
    pub state: Option<SessionState>,
}

impl MapResponse {
    pub fn new_from_dance_response(space_id: String, danceresponse: DanceResponse) -> Self {
        Self {
            space_id,
            status_code: danceresponse.status_code,
            description: danceresponse.description.to_string(),
            body: danceresponse.body,
            descriptor: danceresponse.descriptor,
            state: danceresponse.state,
        }
    }
}
