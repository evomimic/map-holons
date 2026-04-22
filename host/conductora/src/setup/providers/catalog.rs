use std::sync::Arc;

use crate::setup::{
    provider_integration::ProviderIntegration,
    providers::{holochain::HolochainProvider, ipfs::IpfsProvider, local::LocalProvider},
};

pub struct ProviderDescriptor {
    pub provider_type: &'static str,
    pub factory: fn() -> Arc<dyn ProviderIntegration>,
}

fn make_holochain_provider() -> Arc<dyn ProviderIntegration> {
    Arc::new(HolochainProvider::new())
}

fn make_ipfs_provider() -> Arc<dyn ProviderIntegration> {
    Arc::new(IpfsProvider::new())
}

fn make_local_provider() -> Arc<dyn ProviderIntegration> {
    Arc::new(LocalProvider::new())
}

pub const PROVIDER_CATALOG: &[ProviderDescriptor] = &[
    ProviderDescriptor { provider_type: "holochain", factory: make_holochain_provider },
    ProviderDescriptor { provider_type: "ipfs", factory: make_ipfs_provider },
    ProviderDescriptor { provider_type: "local", factory: make_local_provider },
];
