use crate::cache_manager::HolonCacheManager;
use crate::commit_manager::CommitManager;
use std::cell::RefCell;

#[derive(Clone)]
pub struct HolonsContext {
    pub commit_manager: RefCell<CommitManager>,
    pub cache_manager: RefCell<HolonCacheManager>,
}

impl HolonsContext {
    pub fn new() -> HolonsContext {
        HolonsContext {
            commit_manager: CommitManager::new().into(),
            cache_manager: HolonCacheManager::new().into(),
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
