use std::collections::HashMap;
use std::sync::Arc;

use crate::setup::provider_integration::ProviderIntegration;
use crate::setup::providers::{holochain::HolochainProvider, ipfs::IpfsProvider, local::LocalProvider};

pub struct ProviderRegistry {
    providers: HashMap<&'static str, Arc<dyn ProviderIntegration>>,
}

impl ProviderRegistry {
    pub fn new() -> Self {
        Self {
            providers: HashMap::new(),
        }
    }

    pub fn with_defaults() -> Self {
        let mut registry = Self::new();
        registry.register(Arc::new(HolochainProvider::new()));
        registry.register(Arc::new(LocalProvider::new()));
        registry.register(Arc::new(IpfsProvider::new()));
        registry
    }

    pub fn register(&mut self, provider: Arc<dyn ProviderIntegration>) {
        self.providers.insert(provider.provider_type(), provider);
    }

    pub fn get(&self, provider_type: &str) -> Option<&Arc<dyn ProviderIntegration>> {
        self.providers.get(provider_type)
    }
}
