use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use crate::client_shared_objects::client_holon_service::run_future_synchronously;
use base_types::MapString;
use core_types::{ContentSet, FileData, HolonError};
use holons_core::core_shared_objects::transactions::TransactionContext;
use holons_core::reference_layer::{ReadableHolon, WritableHolon};

/// Transitional host-owned catalog of locally bundled Dancer packages.
///
/// This is a bootstrap installer/cache for development and locally bundled
/// assets, not the authority for Dancer discovery or home-Dancer selection.
/// Those decisions belong to DHT-backed Holons and their relationships within
/// the active HolonSpace. The catalog maps semantic package identities to
/// packaged resources and deliberately knows nothing about individual Dance
/// names or Visualizers.
#[derive(Debug, Clone)]
pub struct DancerPackageCatalog {
    packages: Arc<HashMap<MapString, PathBuf>>,
    presentation_package: PathBuf,
    activated: Arc<Mutex<HashSet<MapString>>>,
}

impl DancerPackageCatalog {
    pub fn new(package_root: PathBuf) -> Self {
        let mut packages = HashMap::new();
        packages
            .insert(MapString::from("SpaceNavigator.Dancer"), package_root.join("space-navigator"));
        let presentation_package = package_root
            .parent()
            .unwrap_or(&package_root)
            .join("visualizer-commons/default-presentation");
        Self {
            packages: Arc::new(packages),
            presentation_package,
            activated: Arc::new(Mutex::new(HashSet::new())),
        }
    }

    pub fn development_default() -> Self {
        Self::new(
            PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("../../conductora/resources/house-troupe"),
        )
    }

    /// Loads a locally bundled bootstrap package once per host runtime using
    /// an isolated transaction.
    ///
    /// Loading makes a package's Holons available locally; it does not confer
    /// authority to select the package's Dancer. Runtime selection must read
    /// the active HolonSpace's DHT-backed relationship state.
    pub fn activate(
        &self,
        context: &Arc<TransactionContext>,
        package_identity: &MapString,
    ) -> Result<(), HolonError> {
        let mut activated = self
            .activated
            .lock()
            .map_err(|error| HolonError::FailedToAcquireLock(format!("{error}")))?;
        if activated.contains(package_identity) {
            return Ok(());
        }

        let presentation_identity = MapString::from("DAHN.DefaultPresentation");
        if !activated.contains(&presentation_identity) {
            let presentation = bundled_schema_content(&self.presentation_package)?;
            let isolated_context = context.open_isolated_transaction()?;
            run_future_synchronously(holons_loader_client::load_holons_from_files(
                isolated_context,
                presentation,
            ))?;
            // Preserve the bundled default Theme offer while a general Dancer
            // dependency/activation implementation remains separate work.
            establish_theme_offers(context, vec![MapString::from("DAHN.DefaultTheme")])?;
            activated.insert(presentation_identity);
        }

        let package_directory = self.packages.get(package_identity).ok_or_else(|| {
            HolonError::InvalidParameter(format!(
                "No locally bundled Dancer package resolves semantic identity `{package_identity}`"
            ))
        })?;
        let package_content = bundled_schema_content(package_directory)?;
        let isolated_context = context.open_isolated_transaction()?;
        run_future_synchronously(holons_loader_client::load_holons_from_files(
            isolated_context,
            package_content,
        ))?;
        activated.insert(package_identity.clone());
        Ok(())
    }
}

/// Reads the generated schema for a locally bundled resource directory.
fn bundled_schema_content(directory: &Path) -> Result<ContentSet, HolonError> {
    let path = directory.join("imports/schema.json");
    let raw_contents = std::fs::read_to_string(&path).map_err(|error| {
        HolonError::InvalidState(format!(
            "Unable to read bundled schema import {}: {error}",
            path.display()
        ))
    })?;
    Ok(ContentSet {
        files_to_load: vec![FileData { filename: "imports/schema.json".into(), raw_contents }],
    })
}

/// Stages package-provided Theme offers in the caller's activation transaction.
/// The offer lives in activation state, not Core bootstrap.
fn establish_theme_offers(
    context: &Arc<TransactionContext>,
    offered_theme_keys: Vec<MapString>,
) -> Result<(), HolonError> {
    if offered_theme_keys.is_empty() {
        return Ok(());
    }

    let active_space = context.get_space_holon()?.ok_or_else(|| {
        HolonError::InvalidState(
            "Dancer activation requires an active HolonSpace before offering Themes".into(),
        )
    })?;
    let active_space_id = active_space.holon_id()?;

    for theme_key in offered_theme_keys {
        let theme = context.lookup().get_saved_holon_by_key(&theme_key)?;
        let existing_offers = theme.related_holons("OfferedByHolonSpace")?;
        let already_offered = {
            let offers = existing_offers.read().map_err(|error| {
                HolonError::FailedToAcquireLock(format!(
                    "failed to read existing Theme offers during activation: {error}"
                ))
            })?;
            let mut found = false;
            for offer in &*offers {
                if offer.holon_id()? == active_space_id {
                    found = true;
                    break;
                }
            }
            found
        };

        if already_offered {
            continue;
        }

        let mut staged_theme = context.mutation().stage_new_version(theme)?;
        staged_theme.add_related_holons("OfferedByHolonSpace", vec![active_space.clone()])?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolves_only_known_semantic_package_identities() {
        let catalog = DancerPackageCatalog::new("/packages".into());
        assert!(catalog.packages.contains_key(&MapString::from("SpaceNavigator.Dancer")));
        assert!(!catalog.packages.contains_key(&MapString::from("unknown.Dancer")));
    }

    #[test]
    fn bundled_dancer_and_presentation_imports_are_available() {
        let catalog = DancerPackageCatalog::development_default();
        let dancer_directory = catalog
            .packages
            .get(&MapString::from("SpaceNavigator.Dancer"))
            .expect("Space Navigator bundled resources");
        for (directory, expected_key) in [
            (dancer_directory, "SpaceNavigator.Dancer"),
            (&catalog.presentation_package, "DAHN.DefaultTheme"),
        ] {
            let content = bundled_schema_content(directory).expect("bundled schema import");
            assert_eq!(content.files_to_load.len(), 1);
            assert_eq!(content.files_to_load[0].filename, "imports/schema.json");
            assert!(content.files_to_load[0].raw_contents.contains(expected_key));
        }
    }
}
