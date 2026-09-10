use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::Mutex;

use base_types::{BaseValue, MapBytes, MapString};
use core_types::HolonError;
use holons_core::core_shared_objects::transactions::TransactionContext;
use holons_core::dances::MaterializedVisualizer;
use holons_core::reference_layer::{HolonReference, ReadableHolon, WritableHolon};
use sha2::{Digest, Sha256};
use type_names::{
    CorePropertyTypeName, DahnPropertyTypeName, DahnRelationshipTypeName, ToPropertyName,
};
use uuid::Uuid;

/// Host-Authoritative artifact backend for MaterializeVisualizer.
///
/// The lookup table is keyed by the selected Visualizer's stable semantic key;
/// local filenames do not cross the Dance response boundary.
#[derive(Debug, Clone)]
pub struct DahnMaterializer {
    artifact_root: PathBuf,
    issued_artifacts: Arc<Mutex<HashMap<MapString, IssuedArtifact>>>,
}

#[derive(Debug)]
struct IssuedArtifact {
    transaction_id: u64,
    bytes: MapBytes,
}

impl DahnMaterializer {
    pub fn new(artifact_root: PathBuf) -> Self {
        Self { artifact_root, issued_artifacts: Arc::new(Mutex::new(HashMap::new())) }
    }

    pub fn development_default() -> Self {
        Self::new(
            PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("../../conductora/resources/dahn-visualizers"),
        )
    }

    /// Resolves and verifies the selected Visualizer through this host service's
    /// local artifact backend, then returns a transient opaque artifact handle.
    pub fn materialize(
        &self,
        context: &Arc<TransactionContext>,
        visualizer: &HolonReference,
    ) -> Result<HolonReference, HolonError> {
        let visualizer_key = visualizer.key()?.ok_or_else(|| {
            HolonError::NotImplemented(
                "Materialize requires a selected Visualizer with a stable key".to_string(),
            )
        })?;
        let implementation = require_single_implementation(visualizer)?;
        let expected_digest = required_string_property(
            &implementation,
            DahnPropertyTypeName::VisualizerArtifactDigest,
        )?;
        let module_format = required_string_property(
            &implementation,
            DahnPropertyTypeName::VisualizerModuleFormat,
        )?;
        let entrypoint =
            required_string_property(&implementation, CorePropertyTypeName::Entrypoint)?;
        let artifact = self.artifact_for(&visualizer_key)?;
        let bytes = fs::read(&artifact).map_err(|error| {
            HolonError::NotImplemented(format!(
                "Unable to materialize Visualizer `{visualizer_key}` from {}: {error}",
                artifact.display()
            ))
        })?;
        let actual_digest = MapString(format!("sha256:{}", hex::encode(Sha256::digest(&bytes))));
        if actual_digest != expected_digest {
            return Err(HolonError::InvalidState(format!(
                "Visualizer artifact digest mismatch for `{visualizer_key}`: expected {expected_digest}, found {actual_digest}"
            )));
        }

        let handle = MapString(format!("artifact:{}", Uuid::new_v4()));
        self.issued_artifacts
            .lock()
            .map_err(|error| HolonError::FailedToAcquireLock(format!("{error}")))?
            .insert(
                handle.clone(),
                IssuedArtifact { transaction_id: context.tx_id().value(), bytes: MapBytes(bytes) },
            );

        let descriptor = context
            .lookup()
            .get_saved_holon_by_key(&MapString::from("MaterializedVisualizer.Projection"))?;
        let mut body =
            context.mutation().new_holon(Some(MapString::from("materialized-visualizer")))?;
        body.with_descriptor(HolonReference::from(descriptor))?;
        let mut materialized = MaterializedVisualizer::new(HolonReference::from(body))?;
        materialized.set_artifact_handle(handle)?;
        materialized.set_module_format(module_format)?;
        materialized.set_entrypoint(entrypoint)?;
        Ok(materialized.into_inner())
    }

    /// Consumes a handle issued to this transaction and returns the already
    /// verified artifact bytes. Handles cannot be reused or replayed in a
    /// different transaction.
    pub fn fetch_artifact(
        &self,
        context: &Arc<TransactionContext>,
        handle: &MapString,
    ) -> Result<MapBytes, HolonError> {
        let issued = self
            .issued_artifacts
            .lock()
            .map_err(|error| HolonError::FailedToAcquireLock(format!("{error}")))?
            .remove(handle)
            .ok_or_else(|| {
                HolonError::InvalidParameter("Unknown or expired artifact handle".into())
            })?;

        if issued.transaction_id != context.tx_id().value() {
            return Err(HolonError::InvalidParameter(
                "Artifact handle was issued to a different transaction".into(),
            ));
        }

        Ok(issued.bytes)
    }

    fn artifact_for(&self, visualizer_key: &MapString) -> Result<PathBuf, HolonError> {
        let filename = match visualizer_key.0.as_str() {
            "SpaceNavigator.CanvasVisualizer" => "space-navigator.js",
            "GenericHolonNodeVisualizer.NodeVisualizer" => "generic-holon-node.js",
            "TableCollectionVisualizer.CollectionVisualizer" => "table-collection.js",
            key => {
                return Err(HolonError::NotImplemented(format!(
                    "No local artifact is configured for selected Visualizer `{key}`"
                )))
            }
        };
        Ok(self.artifact_root.join(Path::new(filename)))
    }
}

fn require_single_implementation(
    visualizer: &HolonReference,
) -> Result<HolonReference, HolonError> {
    let implementations = visualizer.related_holons(DahnRelationshipTypeName::ImplementedBy)?;
    let members = implementations
        .read()
        .map_err(|error| HolonError::FailedToAcquireLock(format!("{error}")))?
        .get_members()
        .clone();

    match members.as_slice() {
        [implementation] => Ok(implementation.clone()),
        [] => Err(HolonError::MissingRequiredRelationship {
            relationship: DahnRelationshipTypeName::ImplementedBy
                .as_relationship_name()
                .to_string(),
            descriptor: visualizer.summarize()?,
        }),
        _ => Err(HolonError::MultipleRelatedHolons {
            relationship: DahnRelationshipTypeName::ImplementedBy
                .as_relationship_name()
                .to_string(),
            descriptor: visualizer.summarize()?,
            count: members.len(),
        }),
    }
}

fn required_string_property<T: ToPropertyName>(
    holon: &HolonReference,
    property_name: T,
) -> Result<MapString, HolonError> {
    let property_name = property_name.to_property_name();
    match holon.property_value(property_name.clone())? {
        Some(BaseValue::StringValue(value)) => Ok(value),
        Some(other) => {
            Err(HolonError::UnexpectedValueType(format!("{other:?}"), "String".to_string()))
        }
        None => Err(HolonError::EmptyField(property_name.0 .0)),
    }
}

#[cfg(test)]
mod tests {
    use super::DahnMaterializer;
    use base_types::MapString;

    #[test]
    fn resolves_local_artifacts_from_visualizer_semantic_keys() {
        let materializer = DahnMaterializer::new("/artifacts".into());

        assert_eq!(
            materializer
                .artifact_for(&MapString::from("GenericHolonNodeVisualizer.NodeVisualizer"))
                .expect("known bootstrap visualizer")
                .to_string_lossy(),
            "/artifacts/generic-holon-node.js"
        );
    }

    #[test]
    fn refuses_unselected_or_unknown_visualizer_keys() {
        let materializer = DahnMaterializer::new("/artifacts".into());

        assert!(materializer.artifact_for(&MapString::from("untrusted.visualizer")).is_err());
    }
}
