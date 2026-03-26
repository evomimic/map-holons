use std::collections::HashMap;
use std::sync::Arc;

use crate::setup::provider_integration::ProviderIntegration;
use crate::setup::providers::catalog::{ProviderDescriptor, PROVIDER_CATALOG};

pub struct ProviderRegistry {
    providers: HashMap<&'static str, Arc<dyn ProviderIntegration>>,
}

impl ProviderRegistry {
    pub fn new() -> Self {
        Self {
            providers: HashMap::new(),
        }
    }

    pub fn with_provider_types(provider_types: &[&str]) -> Self {
        let mut registry = Self::new();
        for provider_type in provider_types {
            if let Some(descriptor) = Self::find_descriptor(provider_type) {
                registry.register((descriptor.factory)());
            } else {
                tracing::warn!(
                    "[PROVIDER REGISTRY] No default integration registered for provider type '{}'",
                    provider_type
                );
            }
        }
        registry
    }

    fn find_descriptor(provider_type: &str) -> Option<&'static ProviderDescriptor> {
        PROVIDER_CATALOG.iter().find(|d| d.provider_type == provider_type)
    }

    pub fn register(&mut self, provider: Arc<dyn ProviderIntegration>) {
        self.providers.insert(provider.provider_type(), provider);
    }

    pub fn get(&self, provider_type: &str) -> Option<&Arc<dyn ProviderIntegration>> {
        self.providers.get(provider_type)
    }
}
