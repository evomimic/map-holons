use crate::shared_types::holon_space::HolonSpace;
use base_types::MapString;
use core_types::{ContentSet, HolonError, HolonId, PropertyMap, RelationshipName};
use holons_boundary::{
    DanceTypeWire, HolonReferenceWire, HolonWire, StagedReferenceWire, TransientReferenceWire,
};
use holons_core::core_shared_objects::TransientManagerAccess;
use holons_core::{
    core_shared_objects::{transactions::TransactionContext, Holon},
    dances::DanceType,
    query_layer::QueryExpression,
    reference_layer::TransientReference,
    HolonReference, StagedReference,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

/// Runtime request body (may contain tx-bound references).
///
/// This type must not be deserialized across IPC boundaries because it may contain
/// tx-bound references. Use `MapRequestBodyWire` for IPC and call `bind(context)` at ingress.
#[derive(Debug, Clone, PartialEq)]
pub enum MapRequestBody {
    None,
    Holon(Holon),
    TargetHolons(RelationshipName, Vec<HolonReference>),
    TransientReference(TransientReference),
    HolonId(HolonId),
    ParameterValues(PropertyMap),
    StagedRef(StagedReference),
    QueryExpression(QueryExpression),
    LoadHolons(ContentSet),
}

/// IPC-safe wire-form request body.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub enum MapRequestBodyWire {
    None,
    Holon(HolonWire),
    TargetHolons(RelationshipName, Vec<HolonReferenceWire>),
    TransientReference(TransientReferenceWire),
    HolonId(HolonId),
    ParameterValues(PropertyMap),
    StagedRef(StagedReferenceWire),
    QueryExpression(QueryExpression),
    LoadHolons(ContentSet),
}

impl MapRequestBody {
    pub fn new() -> Self {
        Self::None // Assuming 'None' is the default variant
    }

    pub fn new_holon(holon: Holon) -> Self {
        Self::Holon(holon)
    }

    pub fn new_parameter_values(parameters: PropertyMap) -> Self {
        Self::ParameterValues(parameters)
    }

    pub fn new_target_holons(
        relationship_name: RelationshipName,
        holons_to_add: Vec<HolonReference>,
    ) -> Self {
        Self::TargetHolons(relationship_name, holons_to_add)
    }

    pub fn new_staged_reference(staged_reference: StagedReference) -> Self {
        Self::StagedRef(staged_reference)
    }

    pub fn new_query_expression(query_expression: QueryExpression) -> Self {
        Self::QueryExpression(query_expression)
    }
    pub fn new_load_holons(content_set: ContentSet) -> Self {
        Self::LoadHolons(content_set)
    }
}

#[derive(Debug, Clone)]
pub struct MapRequest {
    pub name: String, // unique key within the (single) dispatch table
    pub req_type: DanceType,
    pub body: MapRequestBody,
    pub space: HolonSpace,
}

/// IPC-safe wire-form map request.
///
/// This is the context-free shape that may be decoded at IPC boundaries.
/// Convert to runtime via `bind(context)`.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct MapRequestWire {
    pub name: String, // unique key within the (single) dispatch table
    pub req_type: DanceTypeWire,
    pub body: MapRequestBodyWire,
    pub space: HolonSpace,
}

impl MapRequest {
    pub fn new_for_reference(reference: TransientReference) -> Self {
        let name = "get_all_holons".to_string();
        let req_type = DanceType::Standalone;
        let body = MapRequestBody::TransientReference(reference);
        let space = HolonSpace::default();
        Self { name, req_type, body, space }
    }
    pub fn test_for_stage_new_holon() -> Self {
        let name = "stage_new_holon".to_string();
        let req_type = DanceType::Standalone;
        let context = crate::init_client_context(None);
        let transient_ref = context
            .get_transient_behavior_service()
            .create_empty(MapString("my_key".to_string()))
            .unwrap();
        let locked_holon =
            context.transient_manager().get_holon_by_id(&transient_ref.temporary_id()).unwrap();
        let actual_holon = locked_holon.read().unwrap().clone();
        let body = MapRequestBody::new_holon(actual_holon);
        //holon.with_property_value(property_name, value)?;
        let space = HolonSpace::default();
        Self { name, req_type, body, space }
    }
}

impl MapRequestWire {
    // ---------------------------------------------------------------------
    // Binding
    // ---------------------------------------------------------------------

    /// Binds a wire request to the supplied transaction, validating `tx_id`
    /// for all embedded references and producing a runtime `MapRequest`.
    pub fn bind(self, context: &Arc<TransactionContext>) -> Result<MapRequest, HolonError> {
        Ok(MapRequest {
            name: self.name,
            req_type: self.req_type.bind(context)?,
            body: self.body.bind(context)?,
            space: self.space,
        })
    }
}

impl MapRequestBodyWire {
    pub fn bind(self, context: &Arc<TransactionContext>) -> Result<MapRequestBody, HolonError> {
        match self {
            MapRequestBodyWire::None => Ok(MapRequestBody::None),
            MapRequestBodyWire::Holon(holon_wire) => {
                Ok(MapRequestBody::Holon(holon_wire.bind(context)?))
            }
            MapRequestBodyWire::TargetHolons(name, wires) => {
                let mut refs = Vec::with_capacity(wires.len());
                for wire in wires {
                    refs.push(wire.bind(context)?);
                }
                Ok(MapRequestBody::TargetHolons(name, refs))
            }
            MapRequestBodyWire::TransientReference(wire) => {
                Ok(MapRequestBody::TransientReference(wire.bind(context)?))
            }
            MapRequestBodyWire::HolonId(id) => Ok(MapRequestBody::HolonId(id)),
            MapRequestBodyWire::ParameterValues(values) => {
                Ok(MapRequestBody::ParameterValues(values))
            }
            MapRequestBodyWire::StagedRef(wire) => {
                Ok(MapRequestBody::StagedRef(wire.bind(context)?))
            }
            MapRequestBodyWire::QueryExpression(query) => {
                Ok(MapRequestBody::QueryExpression(query))
            }
            MapRequestBodyWire::LoadHolons(content_set) => {
                Ok(MapRequestBody::LoadHolons(content_set))
            }
        }
    }
}

impl From<&MapRequest> for MapRequestWire {
    fn from(request: &MapRequest) -> Self {
        Self {
            name: request.name.clone(),
            req_type: DanceTypeWire::from(&request.req_type),
            body: MapRequestBodyWire::from(&request.body),
            space: request.space.clone(),
        }
    }
}

impl From<&MapRequestBody> for MapRequestBodyWire {
    fn from(body: &MapRequestBody) -> Self {
        match body {
            MapRequestBody::None => MapRequestBodyWire::None,
            MapRequestBody::Holon(holon) => MapRequestBodyWire::Holon(HolonWire::from(holon)),
            MapRequestBody::TargetHolons(name, holons) => MapRequestBodyWire::TargetHolons(
                name.clone(),
                holons.iter().map(HolonReferenceWire::from).collect(),
            ),
            MapRequestBody::TransientReference(reference) => {
                MapRequestBodyWire::TransientReference(TransientReferenceWire::from(reference))
            }
            MapRequestBody::HolonId(id) => MapRequestBodyWire::HolonId(id.clone()),
            MapRequestBody::ParameterValues(values) => {
                MapRequestBodyWire::ParameterValues(values.clone())
            }
            MapRequestBody::StagedRef(reference) => {
                MapRequestBodyWire::StagedRef(StagedReferenceWire::from(reference))
            }
            MapRequestBody::QueryExpression(query) => {
                MapRequestBodyWire::QueryExpression(query.clone())
            }
            MapRequestBody::LoadHolons(content_set) => {
                MapRequestBodyWire::LoadHolons(content_set.clone())
            }
        }
    }
}

/*        let holon = TransientHolon::with_fields(
    MapInteger(1),
    HolonState::Mutable,
    ValidationState::ValidationRequired,
    // None,
    property_map,
    TransientRelationshipMap::new_empty(),
    None,
); */
