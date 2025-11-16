use holons_client::shared_types::base_receptor::BaseReceptor;
use std::sync::Mutex;

/// Registry for collecting `ReceptorConfig` entries from different setup modules
#[derive(Default)]
pub struct ReceptorConfigRegistry {
    configs: Mutex<Vec<BaseReceptor>>,
}

impl ReceptorConfigRegistry {
    /// Create an empty registry
    pub fn new() -> Self {
        Self {
            configs: Mutex::new(Vec::new())
        }
    }

    /// Register a receptor config
    pub fn register(&self, config: BaseReceptor) {
        self.configs.lock().unwrap().push(config);
    }

    /// Retrieve all registered receptor configs
    pub fn all(&self) -> Vec<BaseReceptor> {
        let mut configs = self.configs.lock().unwrap().clone();
        Self::ensure_local_receptor_first(&mut configs);
        configs
    }

        /// Ensure the local receptor is first in the vector for priority processing
    fn ensure_local_receptor_first(configs: &mut Vec<BaseReceptor>) {
        if configs.is_empty() {
            return;
        }
        // Find the index of the local receptor
        let local_index = configs.iter().position(|config| {
            config.receptor_type == "local"
        });

        // If found and not already first, move it to the front
        if let Some(index) = local_index {
            if index != 0 {
                let local_config = configs.remove(index);
                configs.insert(0, local_config);
            }
        } else {
            tracing::error!("[REGISTRY] No local receptor found in configs");
        }
    }
}