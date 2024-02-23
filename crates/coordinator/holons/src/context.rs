use std::cell::RefCell;
use crate::commit_manager::CommitManager;

use derive_new::new;

#[derive(Clone)]
pub struct HolonsContext {
    pub commit_manager: RefCell<CommitManager>,
}

impl HolonsContext {
    pub fn new() -> HolonsContext {
        HolonsContext {
            commit_manager: CommitManager::new().into()
            }
        }
    // pub fn set_commit_manager(&mut self, commit_manager: CommitManager) {
    //     self.commit_manager = commit_manager.clone();
    //     return
    // }
}

// impl Context {
//     pub fn new(commit_manager: Option<Rc<CommitManager>>) -> Context {
//         Context { commit_manager }
//     }
// }
