//! Temporary boundary shim crate for wire types.
//!
//! This crate currently re-exports existing wire types from `holons_core` so
//! call sites can migrate imports first without behavior changes. The concrete
//! wire implementations can then be moved here incrementally.

mod dance_request_wire;
mod dance_response_wire;
mod holon_collection_wire;
mod holon_reference_wire;
mod query_wire;
mod smart_reference_wire;
mod staged_reference_wire;
mod transient_reference_wire;

pub mod reference_layer {
    pub use crate::holon_reference_wire::HolonReferenceWire;
    pub use crate::smart_reference_wire::SmartReferenceWire;
    pub use crate::staged_reference_wire::StagedReferenceWire;
    pub use crate::transient_reference_wire::TransientReferenceWire;
}

pub mod core_shared_objects {
    pub use holons_core::core_shared_objects::holon_pool::SerializableHolonPool;
    pub use holons_core::core_shared_objects::{
        HolonCollectionWire, HolonWire, StagedHolonWire, StagedRelationshipMapWire,
        TransientHolonWire, TransientRelationshipMapWire,
    };
}

pub mod query_layer {
    pub use holons_core::query_layer::{NodeCollectionWire, NodeWire, QueryPathMapWire};
}

pub mod dances {
    pub use holons_core::dances::dance_request::{
        DanceRequestWire, DanceTypeWire, RequestBodyWire,
    };
    pub use holons_core::dances::dance_response::{DanceResponseWire, ResponseBodyWire};
    pub use holons_core::dances::SessionState;
}

pub use core_shared_objects::*;
pub use dances::*;
pub use query_layer::*;
pub use reference_layer::*;
