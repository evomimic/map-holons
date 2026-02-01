use std::sync::{Arc, RwLock};
// Removed `RefCell`/`Rc` in favor of thread-safe types

use super::Holon;
use crate::core_shared_objects::transactions::TransactionContext;
use crate::{HolonCacheAccess, HolonCacheManager, HolonCollection, RelationshipMap};
use core_types::{HolonError, HolonId, RelationshipName};

#[derive(Debug)]
pub struct CacheRequestRouter {
    local_cache_manager: Arc<RwLock<HolonCacheManager>>, // Thread-safe local cache manager
    cache_routing_policy: ServiceRoutingPolicy,          // Determines how requests are routed
                                                         //outbound_proxies: Option<Arc<BTreeMap<String, Box<dyn HolonCacheAccess>>>>, // Optional external proxies for remote spaces
}

impl CacheRequestRouter {
    /// Creates a new `CacheRequestRouter` instance.
    pub fn new(
        local_cache_manager: Arc<RwLock<HolonCacheManager>>,
        cache_routing_policy: ServiceRoutingPolicy,
        //outbound_proxies: Option<Arc<BTreeMap<String, Box<dyn HolonCacheAccess>>>>,
    ) -> Self {
        Self { local_cache_manager, cache_routing_policy }
    }

    /// Determines the service route for a given `HolonId` based on the provided `ServiceRoutingPolicy`.
    ///
    /// # Parameters
    /// - `id`: The identifier of the holon to be routed.
    /// - `policy`: The routing policy that governs how requests for the holon should be handled.
    ///
    /// # Returns
    /// - `Ok(ServiceRoute)` if a valid route can be determined based on the `HolonId` and `ServiceRoutingPolicy`.
    /// - `Err(HolonError::InvalidParameter)` if the request is invalid based on the policy.
    ///
    /// # Errors
    /// - Returns `HolonError::InvalidParameter` if the `HolonId` is external and the policy is `BlockExternal`.
    /// - Returns `HolonError::InvalidParameter` if the required `OutboundProxy` is not found for the given space.
    pub fn get_request_route(
        id: &HolonId,
        policy: &ServiceRoutingPolicy,
    ) -> Result<ServiceRoute, HolonError> {
        match id {
            // Case 1: If the HolonId is Local, always return `ServiceRoute::Local`.
            HolonId::Local(_) => Ok(ServiceRoute::Local),

            // Case 2: If the HolonId is External, handle based on the policy.
            HolonId::External(_external_id) => match policy {
                // BlockExternal: Reject requests for external holons.
                ServiceRoutingPolicy::BlockExternal => Err(HolonError::InvalidParameter(
                    "This request is invalid for External HolonId's".to_string(),
                )),

                // Combined: Treat all external holons as local (fallback behavior).
                ServiceRoutingPolicy::Combined => Ok(ServiceRoute::Local),

                // ProxyExternal: Look up the proxy for the space_id and return Proxy.
                ServiceRoutingPolicy::ProxyExternal => {
                    Err(HolonError::NotImplemented(
                        "Service Routing is not implemented for External HolonId's".to_string(),
                    ))
                    // if let Some(proxy) = self
                    //     .outbound_proxies
                    //     .as_ref()
                    //     .and_then(|proxies| proxies.get(&external_id.space_id))
                    // {
                    //     Ok(ServiceRoute::Proxy(proxy))
                    // } else {
                    //     Err(HolonError::InvalidParameter(
                    //         "No outbound proxy found for the given ExternalId's space_id",
                    //     ))
                    // }
                }
            },
        }
    }
}
impl HolonCacheAccess for CacheRequestRouter {
    /// Retrieves a mutable reference (`Arc<RwLock<Holon>`) to the `Holon` identified by `holon_id`.
    /// Delegates to the `local_cache_manager` if the `ServiceRoute` is `Local`.
    /// Returns an error if the route is not `Local` or cannot be resolved.
    fn get_rc_holon(&self, holon_id: &HolonId) -> Result<Arc<RwLock<Holon>>, HolonError> {
        // Determine the routing policy for the request
        match CacheRequestRouter::get_request_route(holon_id, &self.cache_routing_policy)? {
            ServiceRoute::Local => {
                // Delegate to the local cache manager through a read lock
                self.local_cache_manager
                    .read()
                    .map_err(|e| {
                        HolonError::FailedToAcquireLock(format!(
                            "Cache manager read lock poisoned: {}",
                            e
                        ))
                    })?
                    .get_rc_holon(holon_id)
            } // ServiceRoute::Proxy(_) => {
              //     // Handle proxy-based requests (if supported in the future)
              //     Err(HolonError::NotImplemented(
              //         "Proxy-based cache access is not yet implemented.",
              //     ))
              // }
        }
    }

    /// Retrieves a collection of `Holon`s related to the `source_holon_id` by a given `relationship_name`.
    /// Delegates to the `local_cache_manager` if the `ServiceRoute` is `Local`.
    /// Returns a thread-safe `Arc<RwLock<HolonCollection>>` or an error if not `Local`.
    fn get_related_holons(
        &self,
        context: &TransactionContext,
        source_holon_id: &HolonId,
        relationship_name: &RelationshipName,
    ) -> Result<Arc<RwLock<HolonCollection>>, HolonError> {
        // Determine the routing policy for the request
        match CacheRequestRouter::get_request_route(source_holon_id, &self.cache_routing_policy)? {
            ServiceRoute::Local => {
                // Delegate to the local cache manager through a read lock
                self.local_cache_manager
                    .read()
                    .map_err(|e| {
                        HolonError::FailedToAcquireLock(format!(
                            "Cache manager read lock poisoned: {}",
                            e
                        ))
                    })?
                    .get_related_holons(context, source_holon_id, relationship_name)
            } // ServiceRoute::Proxy(_) => {
              //     // Handle proxy-based requests (if supported in the future)
              //     Err(HolonError::NotImplemented(
              //         "Proxy-based related holon access is not yet implemented.",
              //     ))
              // }
        }
    }

    fn get_all_related_holons(
        &self,
        context: &TransactionContext,
        source_holon_id: &HolonId,
    ) -> Result<RelationshipMap, HolonError> {
        // Determine the routing policy for the request
        match CacheRequestRouter::get_request_route(source_holon_id, &self.cache_routing_policy)? {
            ServiceRoute::Local => {
                // Delegate to the local cache manager through a read lock
                self.local_cache_manager
                    .read()
                    .map_err(|e| {
                        HolonError::FailedToAcquireLock(format!(
                            "Cache manager read lock poisoned: {}",
                            e
                        ))
                    })?
                    .get_all_related_holons(context, source_holon_id)
            }
        }
    }
}

/// Specifies the routing policy for handling holon service requests in the `HolonSpaceManager`.
///
/// The `ServiceRoutingPolicy` determines how the `HolonSpaceManager` should handle requests
/// for holons that may be external to the local space. Different policies can be used to
/// control whether external holon requests are allowed, how they are resolved, and whether
/// proxies are used to delegate these requests.
///
/// # Variants
///
/// - `BlockExternal`:
///     - Requests for holons that are not local to the current space are denied.
///     - This policy ensures that only holons within the local space are accessible,
///       effectively blocking any interaction with external spaces.
///
/// - `Combined`:
///     - Supports both local and external holons seamlessly.
///     - Requests for external holons are resolved internally, either by querying other services
///       or delegating to a mechanism such as an outbound proxy (if available).
///     - This policy provides a hybrid approach that does not explicitly block or proxy external holons.
///
/// - `ProxyExternal`:
///     - Requests for holons that are external to the local space are routed through an `OutboundProxy`.
///     - Proxies are injected into the `HolonSpaceManager` and used to resolve holons from
///       other spaces.
///     - This policy is suitable for environments where communication with external spaces
///       is required, but access should be explicitly mediated through proxies.
///
/// # Benefits of Using `ServiceRoutingPolicy`
///
/// - Encapsulates different routing strategies in a single unified type.
/// - Enables clean and extensible configurations for managing holon requests.
/// - Simplifies decision-making in the `HolonSpaceManager` by delegating routing behavior
///   to the selected policy.
///
/// # Extensibility
///
/// If additional routing strategies are needed in the future, simply add a new variant to this enum
/// and update the `HolonSpaceManager` logic to handle the new case. This design promotes
/// clean separation of concerns and ensures that routing decisions remain configurable.
#[derive(Clone, Debug)]
pub enum ServiceRoutingPolicy {
    BlockExternal, // Reject requests for external holons
    Combined,      // Support both local and external holons
    ProxyExternal, // Proxy requests for external holons to an injected OutboundProxy
}

pub enum ServiceRoute {
    Local,
    // Proxy(&OutboundSpaceProxy), TODO: implement OutboundSpaceProxy
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_thread_safe<T: Send + Sync>() {}

    #[test]
    fn cache_request_router_is_thread_safe() {
        assert_thread_safe::<CacheRequestRouter>();
    }
}
