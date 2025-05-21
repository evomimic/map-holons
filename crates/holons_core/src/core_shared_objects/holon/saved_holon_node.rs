use derive_new::new;
use hdi::prelude::Record;
use serde::{Deserialize, Serialize};
use shared_types_holon::{LocalId, MapInteger};




#[derive(new, Debug, Clone, Serialize, Deserialize, Eq, PartialEq)]
pub struct SavedHolonNode {
    saved_node: Record,
}

impl SavedHolonNode {
    /// Retrieves the `LocalId` from the underlying `saved_node`. 
    pub fn get_local_id(&self) -> LocalId {
            LocalId(self.saved_node.action_address().clone())
    }
}

// #[cfg(test)]
// mod tests {
//     use super::*;
//     use hdi::prelude::ActionHash;


//     /// Utility function to create a mock `Record`.
//     fn mock_record(hash_str: &str) -> Record {
//         let action_hash = ActionHash::from_raw_36(hash_str.as_bytes().to_vec());
//         Record::new(action_hash)
//     }

//     #[test]
//     fn get_local_id_returns_local_id_when_saved_node_is_present() {
//         // Arrange
//         let record = mock_record("valid_hash");
//         let saved_holon_node = SavedHolonNode {
//             saved_node: Some(record.clone()),
//         };

//         // Act
//         let result = saved_holon_node.get_local_id();

//         // Assert
//         assert!(result.is_ok());
//         assert_eq!(result.unwrap(), LocalId(record.action_address().clone()));
//     }

//     #[test]
//     fn get_local_id_returns_error_when_saved_node_is_none() {
//         // Arrange
//         let saved_holon_node = SavedHolonNode {
//             saved_node: None,
//         };

//         // Act
//         let result = saved_holon_node.get_local_id();

//         // Assert
//         assert!(result.is_err());
//         assert!(matches!(result, Err(HolonError::HolonNotFound(_))));
//     }

//     #[test]
//     fn get_local_id_produces_correct_error_message() {
//         // Arrange
//         let saved_holon_node = SavedHolonNode {
//             saved_node: None,
//         };

//         // Act
//         let result = saved_holon_node.get_local_id();

//         // Assert
//         match result {
//             Err(HolonError::HolonNotFound(msg)) => {
//                 assert_eq!(msg, "SavedHolonNode is empty");
//             }
//             _ => panic!("Expected HolonNotFound error with correct message"),
//         }
//     }

//     #[test]
//     fn get_local_id_with_different_records_produces_correct_ids() {
//         // Arrange
//         let record1 = mock_record("hash_one");
//         let record2 = mock_record("hash_two");

//         let node1 = SavedHolonNode {
//             saved_node: Some(record1.clone()),
//         };

//         let node2 = SavedHolonNode {
//             saved_node: Some(record2.clone()),
//         };

//         // Act & Assert
//         assert_eq!(node1.get_local_id().unwrap(), LocalId(record1.action_address().clone()));
//         assert_eq!(node2.get_local_id().unwrap(), LocalId(record2.action_address().clone()));
//         assert_ne!(node1.get_local_id().unwrap(), node2.get_local_id().unwrap());
//     }
// }