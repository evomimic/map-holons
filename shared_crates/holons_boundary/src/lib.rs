//! Temporary boundary shim crate for wire types.
//!
//! This crate currently re-exports existing wire types from `holons_core` so
//! call sites can migrate imports first without behavior changes. The concrete
//! wire implementations can then be moved here incrementally.

mod context_binding;
pub mod envelopes;
pub mod session_state;

pub mod reference_layer {
    pub use crate::context_binding::holon_reference_wire::HolonReferenceWire;
    pub use crate::context_binding::smart_reference_wire::SmartReferenceWire;
    pub use crate::context_binding::staged_reference_wire::StagedReferenceWire;
    pub use crate::context_binding::transient_reference_wire::TransientReferenceWire;
}

pub mod core_shared_objects {
    pub use crate::session_state::SerializableHolonPool;
    pub use crate::context_binding::{
        HolonCollectionWire, HolonWire, StagedHolonWire, StagedRelationshipMapWire,
        TransientHolonWire, TransientRelationshipMapWire,
    };
}

pub mod query_layer {
    pub use crate::context_binding::{NodeCollectionWire, NodeWire, QueryPathMapWire};
}

pub mod dances {
    pub use crate::context_binding::{DanceRequestWire, DanceTypeWire, RequestBodyWire};
    pub use crate::context_binding::{DanceResponseWire, ResponseBodyWire};
    pub use crate::session_state::SessionStateWire;
}

pub use core_shared_objects::*;
pub use dances::*;
pub use envelopes::*;
pub use query_layer::*;
pub use reference_layer::*;
pub use session_state::*;
