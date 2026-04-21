use client_shared_types::map_request::{MapRequest, MapRequestBody};
use base_types::BaseValue::StringValue;
use base_types::MapString;
use core_types::{HolonError, PropertyMap, PropertyName};
use holon_dance_builders::{
    build_commit_dance_request, build_get_all_holons_dance_request,
    build_get_holon_by_id_dance_request,
};
use holons_core::core_shared_objects::transactions::TransactionContext;
use holons_core::dances::DanceRequest;
use std::sync::Arc;

pub struct ClientDanceBuilder;

const PERMITTED_OPS: &[&str] = &[
    "commit",
    "delete_holon",
    "get_all_holons",
    "get_holon_by_id",
    "load_holons",
    "query_relationships",
];

impl ClientDanceBuilder {
    pub fn _permitted_operations() -> Vec<&'static str> {
        PERMITTED_OPS.to_vec()
    }

    pub fn validate_and_execute(
        context: &Arc<TransactionContext>,
        request: &MapRequest,
    ) -> Result<DanceRequest, HolonError> {
        Self::validate_request(request)?;

        match request.name.as_str() {
            "commit" => Self::commit_dance(context, request),
            "delete_holon" => Self::delete_holon_dance(context, request),
            "get_all_holons" => Self::get_all_holons_dance(),
            "get_holon_by_id" => Self::get_holon_by_id_dance(context, request),
            "load_holons" => Self::load_holons_dance(context, request),
            "query_relationships" => Self::query_relationships_dance(context, request),
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
    pub fn commit_dance(
        _context: &Arc<TransactionContext>,
        _request: &MapRequest,
    ) -> Result<DanceRequest, HolonError> {
        return build_commit_dance_request();
    }
    pub fn delete_holon_dance(
        _context: &Arc<TransactionContext>,
        _request: &MapRequest,
    ) -> Result<DanceRequest, HolonError> {
        todo!()
    }
    pub fn get_all_holons_dance(//_context: &Arc<TransactionContext>,
        //_request: &MapRequest,
    ) -> Result<DanceRequest, HolonError> {
        return build_get_all_holons_dance_request();
    }
    pub fn get_holon_by_id_dance(
        _context: &Arc<TransactionContext>,
        request: &MapRequest,
    ) -> Result<DanceRequest, HolonError> {
        match &request.body {
            MapRequestBody::HolonId(holon_id) => {
                return build_get_holon_by_id_dance_request(holon_id.clone())
            }
            _ => {
                return Err(HolonError::InvalidParameter(
                    "Missing HolonId in request body for get_holon_by_id".into(),
                ))
            }
        }
    }
    pub fn load_holons_dance(
        _context: &Arc<TransactionContext>,
        request: &MapRequest,
    ) -> Result<DanceRequest, HolonError> {
        match &request.body {
            MapRequestBody::TransientReference(transient_ref) => {
                return holon_dance_builders::load_holons_dance::build_load_holons_dance_request(
                    transient_ref.clone(),
                );
            }
            _ => {
                return Err(HolonError::InvalidParameter(
                    "Missing Content data in request body for load_holons".into(),
                ))
            }
        }
    }

    pub fn query_relationships_dance(
        _context: &Arc<TransactionContext>,
        _request: &MapRequest,
    ) -> Result<DanceRequest, HolonError> {
        todo!()
    }

    //helpers

    fn extract_holon_key(props: &PropertyMap) -> Result<MapString, HolonError> {
        let key_property = props.get(&PropertyName(MapString("key".to_string())));

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
        Ok(key.clone())
    }
}
