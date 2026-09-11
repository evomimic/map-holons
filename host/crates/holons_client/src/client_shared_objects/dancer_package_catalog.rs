use std::collections::{HashMap, HashSet};
use std::path::{Component, Path, PathBuf};
use std::sync::{Arc, Mutex};

use base_types::MapString;
use core_types::{ContentSet, FileData, HolonError};
use holons_core::core_shared_objects::transactions::TransactionContext;
use holons_core::reference_layer::{ReadableHolon, WritableHolon};
use serde::Deserialize;

/// Host-owned catalog of locally bundled Dancer packages.
///
/// The catalog maps semantic package identities to packaged resources. It
/// deliberately knows nothing about individual Dance names or Visualizers.
#[derive(Debug, Clone)]
pub struct DancerPackageCatalog {
    packages: Arc<HashMap<MapString, PathBuf>>,
    activated: Arc<Mutex<HashSet<MapString>>>,
}

#[derive(Debug, Deserialize)]
struct DancerPackageManifest {
    package_identity: String,
    schema_imports: Vec<String>,
    required_holons: Vec<String>,
    #[serde(default)]
    offered_theme_keys: Vec<String>,
}

#[derive(Debug)]
struct PackageActivationContent {
    content_set: ContentSet,
    offered_theme_keys: Vec<MapString>,
}

impl DancerPackageCatalog {
    pub fn new(package_root: PathBuf) -> Self {
        let mut packages = HashMap::new();
        packages
            .insert(MapString::from("SpaceNavigator.Dancer"), package_root.join("space-navigator"));
        Self { packages: Arc::new(packages), activated: Arc::new(Mutex::new(HashSet::new())) }
    }

    pub fn development_default() -> Self {
        Self::new(
            PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("../../conductora/resources/house-troupe"),
        )
    }

    /// Loads a package once per host runtime using an isolated transaction.
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

        let package_directory = self.packages.get(package_identity).ok_or_else(|| {
            HolonError::InvalidParameter(format!(
                "No locally bundled Dancer package resolves semantic identity `{package_identity}`"
            ))
        })?;
        let package_content = package_content_set(package_directory, package_identity)?;
        let isolated_context = context.open_isolated_transaction()?;
        futures_executor::block_on(holons_loader_client::load_holons_from_files(
            isolated_context,
            package_content.content_set,
        ))?;
        establish_theme_offers(context, package_content.offered_theme_keys)?;
        activated.insert(package_identity.clone());
        Ok(())
    }
}

fn package_content_set(
    package_directory: &Path,
    requested_identity: &MapString,
) -> Result<PackageActivationContent, HolonError> {
    let manifest_path = package_directory.join("package.json");
    let manifest: DancerPackageManifest =
        serde_json::from_slice(&std::fs::read(&manifest_path).map_err(|error| {
            HolonError::InvalidState(format!(
                "Unable to read Dancer package manifest {}: {error}",
                manifest_path.display()
            ))
        })?)
        .map_err(|error| {
            HolonError::InvalidState(format!(
                "Unable to parse Dancer package manifest {}: {error}",
                manifest_path.display()
            ))
        })?;

    if manifest.package_identity != requested_identity.0
        || manifest.schema_imports.is_empty()
        || manifest.required_holons.is_empty()
    {
        return Err(HolonError::InvalidState(format!(
            "Dancer package manifest {} is incomplete or incompatible",
            manifest_path.display()
        )));
    }

    let mut files_to_load = Vec::with_capacity(manifest.schema_imports.len());
    for relative_path in manifest.schema_imports {
        let relative_path = Path::new(&relative_path);
        if relative_path.is_absolute()
            || relative_path
                .components()
                .any(|component| !matches!(component, Component::Normal(_)))
        {
            return Err(HolonError::InvalidParameter(format!(
                "Dancer package manifest contains unsafe schema import path {}",
                relative_path.display()
            )));
        }
        let path = package_directory.join(relative_path);
        let raw_contents = std::fs::read_to_string(&path).map_err(|error| {
            HolonError::InvalidState(format!(
                "Unable to read Dancer package import {}: {error}",
                path.display()
            ))
        })?;
        files_to_load.push(FileData {
            filename: relative_path.to_string_lossy().into_owned(),
            raw_contents,
        });
    }

    Ok(PackageActivationContent {
        content_set: ContentSet { files_to_load },
        offered_theme_keys: manifest.offered_theme_keys.into_iter().map(MapString::from).collect(),
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
    fn development_package_manifest_resolves_to_one_schema_import() {
        let catalog = DancerPackageCatalog::development_default();
        let package_directory = catalog
            .packages
            .get(&MapString::from("SpaceNavigator.Dancer"))
            .expect("Space Navigator package entry");

        let package_content =
            package_content_set(package_directory, &MapString::from("SpaceNavigator.Dancer"))
                .expect("packaged Space Navigator resources");

        assert_eq!(package_content.content_set.files_to_load.len(), 1);
        assert_eq!(package_content.content_set.files_to_load[0].filename, "imports/schema.json");
        assert!(package_content.content_set.files_to_load[0]
            .raw_contents
            .contains("SpaceNavigator.Dancer"));
        assert_eq!(
            package_content.offered_theme_keys,
            vec![MapString::from("SpaceNavigator.DefaultTheme")]
        );
    }

    #[test]
    fn package_manifest_rejects_a_mismatched_semantic_identity() {
        let catalog = DancerPackageCatalog::development_default();
        let package_directory = catalog
            .packages
            .get(&MapString::from("SpaceNavigator.Dancer"))
            .expect("Space Navigator package entry");

        let error = package_content_set(package_directory, &MapString::from("Other.Dancer"))
            .expect_err("manifest identity must match the requested package");

        assert!(format!("{error}").contains("incomplete or incompatible"));
    }
}
