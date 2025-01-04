use hdk::prelude::*;
use std::cell::RefCell;
use std::rc::Rc;

use crate::reference_layer::HolonsContextBehavior;
use crate::shared_objects_layer::{
    CommitRequestStatus, CommitResponse, Holon, HolonError, HolonState,
};
use crate::utils::json_adapter::as_json;
use shared_types_holon::{LocalId, MapInteger, MapString};

#[derive(Clone, Debug)]
pub struct CommitService;

impl CommitResponse {
    /// This helper method returns true if the supplied CommitResponse indicates that the commit
    /// was complete and false otherwise
    pub fn is_complete(&self) -> bool {
        match self.status {
            CommitRequestStatus::Complete => true,
            CommitRequestStatus::Incomplete => false,
        }
    }
    pub(crate) fn find_local_id_by_key(&self, k: &MapString) -> Result<LocalId, HolonError> {
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

impl CommitService {
    /// Creates a new instance of `CommitService`.
    pub fn new() -> Self {
        Self
    }
    /// This function attempts to persist the state of all staged_holons AND their relationships.
    ///
    /// The commit is performed in two passes: (1) staged_holons, (2) their relationships.
    ///
    /// In the first pass,
    /// * if a staged_holon commit succeeds,
    ///     * change holon's state to `Saved`
    ///     * populate holon's saved_node
    ///     * add the holon to the saved_nodes vector in the CommitResponse
    /// * if a staged_holon commit fails,
    ///     * leave holon's state unchanged
    ///     * leave holon's saved_node unpopulated
    ///     * push the error into the holon's errors vector
    ///     * do NOT add the holon to the saved_nodes vector in the CommitResponse
    ///
    /// If ANY staged_holon commit fails:
    /// * The 2nd pass (to commit the staged_holon's relationships) is SKIPPED
    /// * the overall return status in the CommitResponse is set to `Incomplete`
    /// * the function returns.
    ///
    /// Otherwise, the 2nd pass is performed.
    /// * If ANY attempt to add a relationship generates an Error, the error is pushed into the
    /// source holon's `errors` vector and processing continues
    ///
    ///
    /// If relationship commits succeed for ALL staged_holons,
    ///     * The space_manager's staged_holons are cleared
    ///     * The Commit Response returns a `Complete` status
    ///
    /// NOTE: The CommitResponse returns clones of any successfully
    /// committed holons, even if the response status is `Incomplete`.
    ///
    pub fn commit(
        &self,
        context: &dyn HolonsContextBehavior,
        staged_holons: &Vec<Rc<RefCell<Holon>>>,
    ) -> Result<CommitResponse, HolonError> {
        debug!("Entering commit...");

        // Initialize the request_status to Complete, assuming all commits will succeed
        // If any commit errors are encountered, reset request_status to `Incomplete`
        let mut response = CommitResponse {
            status: CommitRequestStatus::Complete,
            commits_attempted: MapInteger(0), // staged_holons.len() as i64),
            saved_holons: Vec::new(),
            abandoned_holons: Vec::new(),
        };

        let stage_count = MapInteger(staged_holons.len() as i64);
        if stage_count.0 < 1 {
            info!("Stage empty, nothing to commit!");
            return Ok(response);
        }
        response.commits_attempted = stage_count;

        // FIRST PASS: Commit Staged Holons
        {
            info!("\n\nStarting FIRST PASS... commit staged_holons...");
            for rc_holon in staged_holons {
                trace!(" In commit_service... getting ready to call commit()");
                let outcome = rc_holon.borrow_mut().commit();
                match outcome {
                    Ok(holon) => match holon.state {
                        HolonState::Abandoned => {
                            // should these be indexed?
                            //if !response.abandoned_holons.contains(&holon) {
                            response.abandoned_holons.push(holon);
                            //}
                        }
                        HolonState::Saved => {
                            response.saved_holons.push(holon);
                        }
                        _ => {}
                    },
                    Err(error) => {
                        response.status = CommitRequestStatus::Incomplete;
                        warn!("Attempt to commit holon returned error: {:?}", error.to_string());
                    }
                }
            }
        }

        if response.status == CommitRequestStatus::Incomplete {
            return Ok(response);
        }

        //  SECOND PASS: Commit relationships
        {
            info!("\n\nStarting 2ND PASS... commit relationships for the saved staged_holons...");
            //let commit_manager = context.commit_manager.borrow();
            for rc_holon in staged_holons {
                //commit_manager.staged_holons.clone
                let outcome = rc_holon.borrow_mut().commit_relationships(context);
                if let Err(error) = outcome {
                    rc_holon.borrow_mut().errors.push(error.clone());
                    response.status = CommitRequestStatus::Incomplete;
                    warn!("Attempt to commit relationship returned error: {:?}", error.to_string());
                }
            }
        }

        info!("\n\n VVVVVVVVVVV   SAVED HOLONS AFTER COMMIT VVVVVVVVV\n");
        for saved_holon in &response.saved_holons {
            debug!("{}", as_json(saved_holon));
        }
        Ok(response)
    }
}
