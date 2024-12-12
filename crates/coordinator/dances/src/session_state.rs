use hdk::prelude::*;
// use serde::Serialize;

use hdi::hdk_entry_helper;
// use holons::helpers::summarize_holons;
use crate::staging_area::StagingArea;
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

    pub fn set_local_holon_space(&mut self, local_holon_space: Option<HolonReference>) {
        self.local_holon_space = local_holon_space;
    }
    pub fn set_staging_area(&mut self, staging_area: StagingArea) {
        self.staging_area = staging_area;
    }

    // Method to summarize the SessionState
    pub fn summarize(&self) -> String {
        format!(
            "\n   local_holon_space: {:?}, \n  staging_area: {} }}",
            self.local_holon_space,
            self.staging_area.summarize(),
        )
    }
}
