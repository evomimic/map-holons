use std::path::PathBuf;

/// High-level client responsible for validating, parsing, and loading holons
/// from JSON import files via the HolonLoaderDance.
///
/// This sits entirely on the native client side (not in the guest).
pub struct HolonLoaderClient<C: ConductorDanceCaller> {
    /// Shared holons context (nursery, transient manager, etc.).
    pub context: Arc<dyn HolonsContextBehavior>,
    /// Service for executing dances against the conductor.
    pub dance_service: DanceCallService<C>,
    /// Static configuration for this loader client (paths, keys, etc.).
    pub config: LoaderClientConfig,
}

impl<C: ConductorDanceCaller> HolonLoaderClient<C> {
    /// Construct a new `HolonLoaderClient` with the given context, dance service, and config.
    pub fn new(
        context: Arc<dyn HolonsContextBehavior>,
        dance_service: DanceCallService<C>,
        config: LoaderClientConfig,
    ) -> Self;

    /// Primary entry point for the host side Holon Loader.
    ///
    /// This function is intended to be called from `ReceptorFactory::load_holons(...)`.
    /// It will:
    /// 1. Validate each import file against the loader JSON Schema.
    /// 2. Parse all files into a single `HolonLoadSet`.
    /// 3. Invoke the `load_holons` dance with a `TransientReference → HolonLoadSet`.
    /// 4. Return the resulting `HolonLoadResponse` as a transient holon.
    ///
    /// If validation fails, no dance is invoked.
    /// If parsing fails, no dance is invoked.
    ///
    /// On success, this returns the `HolonLoadResponse` transient holon produced by the guest.
    pub async fn load_holons_from_files(
        &self,
        import_file_paths: Vec<PathBuf>,
    ) -> Result<holons_core::TransientHolon, HolonError>;
}
