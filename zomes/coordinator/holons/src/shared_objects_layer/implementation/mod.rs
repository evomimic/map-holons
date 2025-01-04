pub mod commit_service;
pub mod concrete_holons_context_factory;
pub mod context;
mod holon_cache;
mod holon_cache_manager;
pub mod holon_service;
mod local_holon_resolver;
pub mod nursery;
pub mod property_map;
pub mod smartlink;
pub mod space_manager;
pub mod transient_collection;
//pub use crate::shared_objects_layer::implementation::holons_context_factory::ConcreteHolonsContextFactory as HolonsContextFactory;

pub use concrete_holons_context_factory::ConcreteHolonsContextFactory;
