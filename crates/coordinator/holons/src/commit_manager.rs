// use hdk::prelude::*;

use std::collections::BTreeMap;
// use std::sync::Arc;

use crate::holon_errors::HolonError;
use crate::holon_types::Holon;
use shared_types_holon::MapString;

#[derive(Debug, Eq, PartialEq)]
pub struct CommitResponse {
    pub status: CommitRequestStatusCode,
    pub description: MapString,
    pub errors: Option<Vec<CommitError>>,
}

#[derive(Debug, Eq, PartialEq)]
pub enum CommitRequestStatusCode {
    Success,
    Error,
}

#[derive(Debug, Eq, PartialEq)]
pub struct CommitError {
    pub holon_key: MapString,
    pub error_code: HolonError,
}

#[derive(Default, Clone, Debug)]
pub struct CommitManager {
    pub staged_holons: BTreeMap<MapString, Holon>,
    // pub staged_holons: BTreeMap<String, Arc<Holon>>, // <Key, immutable reference>
}

impl CommitManager {
    pub fn stage(&mut self, key: MapString, holon: Holon) {
        self.staged_holons.insert(key, holon);
    }

    pub fn get_by_key(&self, key: MapString) -> Option<&Holon> {
        self.staged_holons.get(&key)
    }
    /*
    // pub fn get_by_key(&self, key: String) -> Option<Arc<Holon>> {
    //     let fetch_option = self.staged_holons.get(&key);
    //     if let Some(holon) = fetch_option {
    //         Some(holon.clone())
    //     }
    //     else { None }
    // }
     */

    // pub fn commit(&mut self) -> Result<Vec<HolonErrorCase>, CommitManagerError> {
    pub fn commit(&mut self) -> CommitResponse {
        let mut errors: Vec<CommitError> = Vec::new();
        for (k, v) in self.clone().staged_holons.iter() {
            let result = v.clone().commit();
            match result {
                Ok(_) => {
                    self.staged_holons.remove(k.into());
                }
                Err(e) => {
                    let commit_error = CommitError {
                        holon_key: k.clone(),
                        error_code: e,
                    };
                    errors.push(commit_error);
                }
            }
        }
        let error_count = errors.len();
        let commit_response = if errors.is_empty() {
            CommitResponse {
                status: CommitRequestStatusCode::Success,
                description: MapString("All holons successfully committed".to_string()),
                errors: None,
            }
        } else {
            CommitResponse {
                status: CommitRequestStatusCode::Error,
                description: MapString(format!("Error committing {:?} holons", error_count)),
                errors: Some(errors),
            }
        };
        commit_response
    }
}
