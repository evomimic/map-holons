use crate::cache_manager::HolonCacheManager;
use crate::commit_manager::CommitManager;
use std::cell::RefCell;

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
    pub fn init_context(commit_manager: CommitManager, cache_manager: HolonCacheManager) -> HolonsContext {
        HolonsContext {
            commit_manager: RefCell::from(commit_manager),
            cache_manager: RefCell::from(cache_manager),
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
