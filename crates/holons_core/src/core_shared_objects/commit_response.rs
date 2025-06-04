use crate::HolonError;
use shared_types_holon::{LocalId, MapInteger, MapString};
use super::holon::{Holon, HolonBehavior, SavedHolon};

#[derive(Debug, Eq, PartialEq, Clone)]
pub struct CommitResponse {
    pub status: CommitRequestStatus,
    pub commits_attempted: MapInteger,
    // could the order of these Vec cause challenges with identifying Holons in relation to their staged_index?
    pub saved_holons: Vec<SavedHolon>, // should this be indexed? where else used?
    pub abandoned_holons: Vec<Holon>, // should this be indexed?
}
#[derive(Debug, Eq, PartialEq, Clone)]
/// *Complete* means all staged holons have been committed and staged_holons cleared
///
/// *Incomplete* means one or more of the staged_holons could not be committed.
/// For details, iterate through the staged_holons vector.
/// Holon's with a `Saved` status have been committed,
/// Holon's with a `New` or `Changed` state had error(s), see the Holon's errors vector for details
pub enum CommitRequestStatus {
    Complete,
    Incomplete,
}
impl CommitResponse {
    /// This helper method returns true if the supplied CommitResponse indicates that the commit
    /// was complete and false otherwise
    pub fn is_complete(&self) -> bool {
        match self.status {
            CommitRequestStatus::Complete => true,
            CommitRequestStatus::Incomplete => false,
        }
    }
    pub fn find_local_id_by_key(&self, k: &MapString) -> Result<LocalId, HolonError> {
        for holon in &self.saved_holons {
            if let Some(key) = holon.get_key()? {
                // Check if the key matches the given key `k`
                if &key == k {
                    // Return the LocalId if a match is found
                    return holon.get_local_id();
                }
            }
        }
        // Return an error if no matching Holon is found
        Err(HolonError::HolonNotFound(format!(
            "No saved Holon with key {:?} was found in commit response",
            k.to_string(),
        )))
    }
}
