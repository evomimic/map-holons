use hdk::prelude::*;
// use serde::Serialize;

use crate::staging_area::StagingArea;
use hdi::hdk_entry_helper;
use holons::cache_manager::HolonCacheManager;
use holons::context::HolonsContext;
use holons::holon_reference::HolonReference;

/// SessionState provides a way to distinguish information associated with a specific request from
/// state info that is just being maintained via the ping pong process. This also should make it
/// easier to evolve to token-based state management approach where, say, the state token is
/// actually a reference into the ephemeral store.
#[hdk_entry_helper]
#[derive(Clone, Eq, PartialEq)]
pub struct SessionState {
    staging_area: StagingArea,
    local_holon_space: Option<HolonReference>,
}

impl SessionState {
    pub fn empty() -> Self {
        Self { staging_area: StagingArea::empty(), local_holon_space: None }
    }
    pub fn new(staging_area: StagingArea, local_holon_space: Option<HolonReference>) -> Self {
        Self { staging_area, local_holon_space }
    }
    pub fn get_local_holon_space(&self) -> Option<HolonReference> {
        self.local_holon_space.clone()
    }
    pub fn get_staging_area(&self) -> &StagingArea {
        &self.staging_area
    }
    pub fn get_staging_area_mut(&mut self) -> &mut StagingArea {
        &mut self.staging_area
    }
    pub fn init_context_from_state(&self) -> HolonsContext {
        let commit_manager = self.staging_area.to_commit_manager();

        let local_holon_space = self.get_local_holon_space();
        //info!("initializing context");
        HolonsContext::init_context(commit_manager, HolonCacheManager::new(), local_holon_space)
    }
    pub fn restore_session_state_from_context(context: &HolonsContext) -> SessionState {
        SessionState::new(
            StagingArea::from_commit_manager(&context.commit_manager.borrow()),
            context.get_local_holon_space(),
        )
    }
    // /// This function constructs a (Smart variant) of a HolonReference to the HolonSpace from the
    // /// holon space id stored in SessionState (or None, if local_holon_space is None)
    // fn get_local_holon_space_reference(&self) -> Option<HolonReference> {
    //     // Check if local_holon_space is Some, then construct a HolonReference
    //     self.local_holon_space.as_ref().map(|holon_space_id| {
    //         // Convert HolonSpaceId to LocalId
    //         let local_id = LocalId::from(holon_space_id.0.clone());
    //
    //         // Create the SmartReference with the Local variant of HolonId (no smart properties)
    //         let smart_reference = SmartReference::new(HolonId::Local(local_id),None);
    //
    //         // Return a HolonReference::Smart variant
    //         HolonReference::Smart(smart_reference)
    //     })
    // }

    pub fn set_local_holon_space(&mut self, local_holon_space: Option<HolonReference>) {
        self.local_holon_space = local_holon_space;
    }
    pub fn set_staging_area(&mut self, staging_area: StagingArea) {
        self.staging_area = staging_area;
    }
}
