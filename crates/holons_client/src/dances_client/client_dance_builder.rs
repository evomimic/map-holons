use base_types::BaseValue::StringValue;
use base_types::MapString;
use core_types::{HolonError, PropertyName};
use holon_dance_builders::{
    build_commit_dance_request, build_get_all_holons_dance_request,
    build_get_holon_by_id_dance_request,
    stage_new_holon_dance::build_stage_new_holon_dance_request,
};
use holons_core::dances::RequestBody;
use holons_core::{
    core_shared_objects::{Holon, ReadableHolonState},
    dances::DanceRequest,
    new_holon, HolonsContextBehavior,
};

use crate::shared_types::map_request::MapRequest;

pub struct ClientDanceBuilder;

const PERMITTED_OPS: &[&str] = &[
    "abandon_staged_changes",
    "add_related_holons",
    "commit",
    "delete_holon",
    "get_all_holons",
    "get_holon_by_id",
    "load_core_schema",
    "query_relationships",
    "remove_properties",
    "remove_related_holons",
    "stage_new_from_clone",
    "stage_new_holon",
    "stage_new_version",
    "with_properties",
];

impl ClientDanceBuilder {
    pub fn permitted_operations() -> Vec<&'static str> {
        PERMITTED_OPS.to_vec()
    }

    pub fn validate_and_execute(
        context: &dyn HolonsContextBehavior,
        request: &MapRequest,
    ) -> Result<DanceRequest, HolonError> {
        Self::validate_request(request)?;

        match request.name.as_str() {
            "abandon_staged_changes" => Self::abandon_staged_changes_dance(context, request),
            "add_related_holons" => Self::add_related_holons_dance(context, request),
            "commit" => Self::commit_dance(context, request),
            "delete_holon" => Self::delete_holon_dance(context, request),
            "get_all_holons" => Self::get_all_holons_dance(context, request),
            "get_holon_by_id" => Self::get_holon_by_id_dance(context, request),
            "load_core_schema" => Self::load_core_schema_dance(context, request),
            "query_relationships" => Self::query_relationships_dance(context, request),
            "remove_properties" => Self::remove_properties_dance(context, request),
            "remove_related_holons" => Self::remove_related_holons_dance(context, request),
            "stage_new_from_clone" => Self::stage_new_from_clone_dance(context, request),
            "stage_new_holon" => Self::stage_new_holon_dance(context, request),
            "stage_new_version" => Self::stage_new_version_dance(context, request),
            "with_properties" => Self::with_properties_dance(context, request),
            _ => Err(HolonError::NotImplemented(format!("Operation '{}' not found", request.name))),
        }
    }

    pub fn validate_request(request: &MapRequest) -> Result<(), HolonError> {
        if PERMITTED_OPS.contains(&request.name.as_str()) {
            Ok(())
        } else {
            Err(HolonError::NotImplemented(format!(
                "Operation '{}' is not permitted. Allowed operations: {}",
                request.name,
                PERMITTED_OPS.join(", ")
            )))
        }
    }
    pub fn abandon_staged_changes_dance(
        _context: &dyn HolonsContextBehavior,
        _request: &MapRequest,
    ) -> Result<DanceRequest, HolonError> {
        todo!()
    }

    pub fn add_related_holons_dance(
        _context: &dyn HolonsContextBehavior,
        _request: &MapRequest,
    ) -> Result<DanceRequest, HolonError> {
        todo!()
    }

    pub fn commit_dance(
        _context: &dyn HolonsContextBehavior,
        _request: &MapRequest,
    ) -> Result<DanceRequest, HolonError> {
        return build_commit_dance_request();
    }
    pub fn delete_holon_dance(
        _context: &dyn HolonsContextBehavior,
        _request: &MapRequest,
    ) -> Result<DanceRequest, HolonError> {
        todo!()
    }
    pub fn get_all_holons_dance(
        _context: &dyn HolonsContextBehavior,
        _request: &MapRequest,
    ) -> Result<DanceRequest, HolonError> {
        return build_get_all_holons_dance_request();
    }
    pub fn get_holon_by_id_dance(
        _context: &dyn HolonsContextBehavior,
        request: &MapRequest,
    ) -> Result<DanceRequest, HolonError> {
        match &request.body {
            RequestBody::HolonId(holon_id) => {
                return build_get_holon_by_id_dance_request(holon_id.clone())
            }
            _ => {
                return Err(HolonError::InvalidParameter(
                    "Missing HolonId in request body for get_holon_by_id".into(),
                ))
            }
        }
    }
    pub fn load_core_schema_dance(
        _context: &dyn HolonsContextBehavior,
        _request: &MapRequest,
    ) -> Result<DanceRequest, HolonError> {
        todo!()
    }
    pub fn query_relationships_dance(
        _context: &dyn HolonsContextBehavior,
        _request: &MapRequest,
    ) -> Result<DanceRequest, HolonError> {
        todo!()
    }
    pub fn remove_properties_dance(
        _context: &dyn HolonsContextBehavior,
        _request: &MapRequest,
    ) -> Result<DanceRequest, HolonError> {
        todo!()
    }
    pub fn remove_related_holons_dance(
        _context: &dyn HolonsContextBehavior,
        _request: &MapRequest,
    ) -> Result<DanceRequest, HolonError> {
        todo!()
    }
    pub fn stage_new_from_clone_dance(
        _context: &dyn HolonsContextBehavior,
        _request: &MapRequest,
    ) -> Result<DanceRequest, HolonError> {
        todo!()
    }

    pub fn stage_new_holon_dance(
        context: &dyn HolonsContextBehavior,
        request: &MapRequest,
    ) -> Result<DanceRequest, HolonError> {
        match &request.body {
            RequestBody::Holon(holon) => {
                let key = Self::extract_holon_key(&holon)?;
                let transient_ref = new_holon(context, Some(key))?;
                return build_stage_new_holon_dance_request(transient_ref.clone());
            }
            _ => {
                return Err(HolonError::InvalidParameter(
                    "Missing holon reference for stage_new_holon".into(),
                ))
            }
        }
    }

    pub fn stage_new_version_dance(
        _context: &dyn HolonsContextBehavior,
        _request: &MapRequest,
    ) -> Result<DanceRequest, HolonError> {
        todo!()
    }
    pub fn with_properties_dance(
        _context: &dyn HolonsContextBehavior,
        _request: &MapRequest,
    ) -> Result<DanceRequest, HolonError> {
        todo!()
    }

    pub fn extract_holon_key(holon: &Holon) -> Result<MapString, HolonError> {
        let key_property = holon.property_value(&PropertyName(MapString("key".to_string())))?;

        // Convert PropertyValue to MapString
        let key = match key_property {
            Some(StringValue(map_string)) => map_string,
            Some(other) => {
                return Err(HolonError::InvalidParameter(format!(
                    "Expected StringValue for key, got: {:?}",
                    other
                )))
            }
            None => return Err(HolonError::HolonNotFound("Key property not found".into())),
        };
        Ok(key)
    }
}
// Methods for building client requests would go here
