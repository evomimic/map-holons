// use hdk::prelude::*;

use std::collections::BTreeMap;
// use std::sync::Arc;

use crate::holon_errors::HolonError;
use crate::holon_types::Holon;
use shared_types_holon::MapString;

// use thiserror::Error;

// #[hdk_entry_helper]
// #[derive(Error, Eq, PartialEq)]
// pub enum CommitManagerError {
//     #[error("Error during commit: {0}")]
//     CommitError(String),
// }

// pub struct HolonErrorCase {
//     pub key: String,
//     pub holon: Holon,
//     // pub holon: Arc<Holon>,
//     pub error: HolonError,
// }

#[derive(Debug)]
pub struct CommitResponse {
    pub status: CommitRequestStatusCode,
    pub description: MapString,
    pub errors: Option<Vec<CommitError>>,
}

#[derive(Debug)]
pub enum CommitRequestStatusCode {
    Success,
    Error,
}

#[derive(Debug)]
pub struct CommitError {
    pub holon_key: MapString,
    pub error_code: HolonError,
    // pub description: MapString,
}

#[derive(Default, Clone, Debug)]
pub struct CommitManager {
    pub staged_holons: BTreeMap<String, Holon>,
    // pub staged_holons: BTreeMap<String, Arc<Holon>>, // <Key, immutable reference>
}

impl CommitManager {
    pub fn stage(&mut self, key: String, holon: Holon) {
        self.staged_holons.insert(key, holon);
    }

    pub fn get_by_key(&self, key: String) -> Option<&Holon> {
        self.staged_holons.get(&key)
    }
    // pub fn get_by_key(&self, key: String) -> Option<Arc<Holon>> {
    //     let fetch_option = self.staged_holons.get(&key);
    //     if let Some(holon) = fetch_option {
    //         Some(holon.clone())
    //     }
    //     else { None }
    // }

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
                        holon_key: MapString(k.to_string()),
                        error_code: e,
                        // description: MapString("".to_string()),
                    };
                    errors.push(commit_error);
                }
            }
        }
        let error_count = errors.len();
        if errors.is_empty() {
            let commit_response = CommitResponse {
                status: CommitRequestStatusCode::Success,
                description: MapString("All holons successfully committed".to_string()),
                errors: None,
            };
            return commit_response;
        } else {
            let commit_response = CommitResponse {
                status: CommitRequestStatusCode::Success,
                description: MapString(format!("Error committing {:?} holons", error_count)),
                errors: Some(errors),
            };
            return commit_response;
        };
    }
}

// let mut commit_count = 0;
// let mut errors: Vec<HolonErrorCase> = Vec::new();
// /*  Arc:
// // for (k, v) in self.staged_holons.iter() {
// //     let holon = v.as_ref();
// //     let result = holon.commit();
// //     match result {
// //         Ok(holon) => {
// //             self.staged_holons.insert(k.to_string(), Arc::new(holon));
// //             commit_count += 1;
// //         }
// //         Err(e) => {
// //             let holon_error_case = HolonErrorCase {
// //                 key: k.to_string(),
// //                 holon: *v,
// //                 error: e,
// //             };
// //             errors.push(holon_error_case);
// //         }
// //     }
// */
// for (k, v) in self.clone().staged_holons.iter() {
//     let result = v.clone().commit();
//     match result {
//         Ok(holon) => {
//             self.staged_holons.insert(k.to_string(), holon);
//             commit_count += 1;
//         }
//         Err(e) => {
//             let holon_error_case = HolonErrorCase {
//                 key: k.to_string(),
//                 holon: v.clone(),
//                 error: e,
//             };
//             errors.push(holon_error_case);
//         }
//     }
// }
// if commit_count == 0 {
//     return Err(CommitManagerError::CommitError(
//         "all holons failed to commit".to_string(),
//     ));
// } else {
//     return Ok(errors);
// }
