use crate::commit_manager::CommitManager;
use std::rc::Rc;

use derive_new::new;

#[derive(new, Clone)]
pub struct Context {
    pub commit_manager: Option<Rc<CommitManager>>, // TODO: arc_swap
}

// impl Context {
//     pub fn new(commit_manager: Option<Rc<CommitManager>>) -> Context {
//         Context { commit_manager }
//     }
// }
