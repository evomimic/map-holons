pub mod context_behavior;
pub mod holon_collection_api;
pub mod holon_operations_api;
mod holon_readable;
pub mod holon_reference;
pub mod holon_service_api;
mod holon_staging_behavior;
mod holon_writable;
pub mod smart_reference;
mod space_manager_behavior;
pub mod staged_reference;
pub mod transient_collection_behavior;

pub use context_behavior::HolonsContextBehavior;
// pub use factory::init_context_from_session;
pub use holon_collection_api::HolonCollectionApi;
pub use holon_readable::HolonReadable;
pub use holon_reference::HolonReference;
pub use holon_service_api::HolonServiceApi;
pub use holon_staging_behavior::HolonStagingBehavior;
pub use holon_writable::HolonWritable;
pub use smart_reference::SmartReference;
pub use space_manager_behavior::HolonSpaceBehavior;
pub use staged_reference::StagedReference;
pub use transient_collection_behavior::TransientCollectionBehavior;
