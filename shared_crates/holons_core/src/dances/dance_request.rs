use crate::core_shared_objects::{Holon, HolonWire, ReadableHolonState};
use crate::reference_layer::TransientReference;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use crate::core_shared_objects::transactions::TransactionContext;
use crate::dances::SessionState;
use crate::query_layer::{NodeCollection, NodeCollectionWire, QueryExpression};
use crate::{
    HolonReference, HolonReferenceWire, StagedReference, StagedReferenceWire,
    TransientReferenceWire,
};
use base_types::MapString;
use core_types::{HolonError, HolonId, LocalId, PropertyMap, RelationshipName};

/// Runtime dance request (tx-bound, execution-capable).
///
/// This type must not be deserialized across IPC boundaries because it may contain
/// tx-bound references. Use `DanceRequestWire` for IPC and call `bind(context)` at ingress.
#[derive(Debug, Clone, PartialEq)]
pub struct DanceRequest {
    pub dance_name: MapString, // unique key within the (single) dispatch table
    pub dance_type: DanceType,
    pub body: RequestBody,
    pub state: Option<SessionState>,
    //pub descriptor: Option<HolonReference>, // space_id+holon_id of DanceDescriptor
}

/// IPC-safe wire-form dance request.
///
/// This is the context-free shape that may be decoded at IPC boundaries.
/// Convert to runtime via `bind(context)`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DanceRequestWire {
    pub dance_name: MapString,
    pub dance_type: DanceTypeWire,
    pub body: RequestBodyWire,
    pub state: Option<SessionState>,
}

/// Runtime dance type (may contain tx-bound references).
#[derive(Debug, Clone, PartialEq)]
pub enum DanceType {
    Standalone,                    // i.e., a dance not associated with a specific holon
    QueryMethod(NodeCollection), // a read-only dance originated from a specific, already persisted, holon
    CommandMethod(HolonReference), // a mutating method operating on a HolonReference
    CloneMethod(HolonReference), // a specific method for cloning a Holon
    NewVersionMethod(HolonId), // a SmartReference only method for cloning a Holon as new version by linking to the original Holon it was cloned from via PREDECESSOR relationship
    DeleteMethod(LocalId),
}

/// IPC-safe wire-form dance type.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub enum DanceTypeWire {
    Standalone,
    QueryMethod(NodeCollectionWire),
    CommandMethod(HolonReferenceWire),
    CloneMethod(HolonReferenceWire),
    NewVersionMethod(HolonId),
    DeleteMethod(LocalId),
}

/// Runtime request body (may contain tx-bound references).
#[derive(Debug, Clone, PartialEq)]
pub enum RequestBody {
    None,
    Holon(Holon),
    TargetHolons(RelationshipName, Vec<HolonReference>),
    TransientReference(TransientReference),
    HolonId(HolonId),
    ParameterValues(PropertyMap),
    StagedRef(StagedReference),
    QueryExpression(QueryExpression),
}

/// IPC-safe wire-form request body.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub enum RequestBodyWire {
    None,
    Holon(HolonWire),
    TargetHolons(RelationshipName, Vec<HolonReferenceWire>),
    TransientReference(TransientReferenceWire),
    HolonId(HolonId),
    ParameterValues(PropertyMap),
    StagedRef(StagedReferenceWire),
    QueryExpression(QueryExpression),
}

impl RequestBody {
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
    pub fn summarize(&self) -> String {
        match &self {
            RequestBody::Holon(holon) => format!("  Holon summary: {}", holon.summarize()),
            RequestBody::TargetHolons(relationship_name, holons) => format!(
                "  relationship: {:?}, {{\n    holon_references: {:?} }} ",
                relationship_name, holons
            ),
            RequestBody::HolonId(holon_id) => format!("  HolonId: {:?}", holon_id),

            _ => format!("{:#?}", self), // Use full debug for other response bodies
        }
    }
}

impl DanceRequest {
    pub fn new(
        dance_name: MapString,
        dance_type: DanceType,
        body: RequestBody,
        state: Option<SessionState>,
    ) -> Self {
        Self {
            dance_name,
            dance_type,
            body,
            state: state.or(Some(SessionState::default())), // Default if None
        }
    }
    /// Gets a reference to the session state, or `None` if not set.
    pub fn get_state(&self) -> Option<&SessionState> {
        self.state.as_ref()
    }

    /// Gets a mutable reference to the session state, or `None` if not set.
    pub fn get_state_mut(&mut self) -> Option<&mut SessionState> {
        self.state.as_mut()
    }

    /// Summarizes the DanceRequest for logging purposes.
    ///
    /// Handles cases where session state is `None` by providing a placeholder message.
    pub fn summarize(&self) -> String {
        format!(
            "DanceRequest {{ \n  dance_name: {:?}, dance_type: {:?}, \n  body: {}, \n  state: {} }}\n",
            self.dance_name.to_string(),
            self.dance_type,
            self.body.summarize(),
            self.state
                .as_ref()
                .map_or_else(|| "None".to_string(), |state| state.summarize()),
        )
    }
}

impl DanceRequestWire {
    // ---------------------------------------------------------------------
    // Binding
    // ---------------------------------------------------------------------

    /// Binds a wire request to the supplied transaction, validating `tx_id`
    /// for all embedded references and producing a runtime `DanceRequest`.
    pub fn bind(self, context: &Arc<TransactionContext>) -> Result<DanceRequest, HolonError> {
        Ok(DanceRequest {
            dance_name: self.dance_name,
            dance_type: self.dance_type.bind(context)?,
            body: self.body.bind(context)?,
            state: self.state,
        })
    }

    // ---------------------------------------------------------------------
    // Accessors (IPC-safe inspection only)
    // ---------------------------------------------------------------------

    /// Returns the dance name for dispatch lookup.
    pub fn dance_name(&self) -> &MapString {
        &self.dance_name
    }

    /// Returns the wire-level dance type.
    ///
    /// Useful for validation, logging, or routing decisions at IPC boundaries.
    pub fn dance_type(&self) -> &DanceTypeWire {
        &self.dance_type
    }

    /// Returns the wire-level request body.
    pub fn body(&self) -> &RequestBodyWire {
        &self.body
    }

    /// Returns the session state, if present.
    pub fn get_state(&self) -> Option<&SessionState> {
        self.state.as_ref()
    }

    /// Returns a cloned session state.
    ///
    /// This is intentionally explicit to make cloning at boundaries obvious.
    pub fn cloned_state(&self) -> Option<SessionState> {
        self.state.clone()
    }

    /// Returns `true` if the request carries session state.
    pub fn has_state(&self) -> bool {
        self.state.is_some()
    }

    // ---------------------------------------------------------------------
    // Logging / diagnostics
    // ---------------------------------------------------------------------

    /// Summarizes the IPC-safe wire request for logging purposes.
    pub fn summarize(&self) -> String {
        let state_summary =
            self.state.as_ref().map_or_else(|| "None".to_string(), |state| state.summarize());

        format!(
            "DanceRequestWire {{ \n  dance_name: {:?}, dance_type: {}, \n  body: {}, \n  state: {} }}\n",
            self.dance_name.to_string(),
            self.dance_type.summarize(),
            self.body.summarize(),
            state_summary,
        )
    }
}

impl DanceTypeWire {
    pub fn bind(self, context: &Arc<TransactionContext>) -> Result<DanceType, HolonError> {
        match self {
            DanceTypeWire::Standalone => Ok(DanceType::Standalone),
            DanceTypeWire::QueryMethod(node_collection_wire) => {
                Ok(DanceType::QueryMethod(node_collection_wire.bind(context)?))
            }
            DanceTypeWire::CommandMethod(reference_wire) => {
                Ok(DanceType::CommandMethod(HolonReference::bind(reference_wire, context)?))
            }
            DanceTypeWire::CloneMethod(reference_wire) => {
                Ok(DanceType::CloneMethod(HolonReference::bind(reference_wire, context)?))
            }
            DanceTypeWire::NewVersionMethod(holon_id) => Ok(DanceType::NewVersionMethod(holon_id)),
            DanceTypeWire::DeleteMethod(local_id) => Ok(DanceType::DeleteMethod(local_id)),
        }
    }

    /// Summarizes the wire dance type for logging purposes.
    pub fn summarize(&self) -> String {
        match self {
            DanceTypeWire::Standalone => "Standalone".to_string(),

            DanceTypeWire::QueryMethod(node_collection_wire) => {
                // Keep this compact; NodeCollectionWire can be large.
                format!("QueryMethod(NodeCollectionWire: {:#?})", node_collection_wire)
            }

            DanceTypeWire::CommandMethod(reference_wire) => {
                format!("CommandMethod({:?})", reference_wire)
            }

            DanceTypeWire::CloneMethod(reference_wire) => {
                format!("CloneMethod({:?})", reference_wire)
            }

            DanceTypeWire::NewVersionMethod(holon_id) => {
                format!("NewVersionMethod({:?})", holon_id)
            }

            DanceTypeWire::DeleteMethod(local_id) => {
                format!("DeleteMethod({:?})", local_id)
            }
        }
    }
}

impl RequestBodyWire {
    pub fn bind(self, context: &Arc<TransactionContext>) -> Result<RequestBody, HolonError> {
        match self {
            RequestBodyWire::None => Ok(RequestBody::None),
            RequestBodyWire::Holon(holon_wire) => {
                Ok(RequestBody::Holon(holon_wire.bind(Arc::clone(context))?))
            }
            RequestBodyWire::TargetHolons(name, wires) => {
                let mut refs = Vec::with_capacity(wires.len());
                for w in wires {
                    refs.push(HolonReference::bind(w, context)?);
                }
                Ok(RequestBody::TargetHolons(name, refs))
            }
            RequestBodyWire::TransientReference(w) => {
                Ok(RequestBody::TransientReference(TransientReference::bind(w, context)?))
            }
            RequestBodyWire::HolonId(id) => Ok(RequestBody::HolonId(id)),
            RequestBodyWire::ParameterValues(p) => Ok(RequestBody::ParameterValues(p)),
            RequestBodyWire::StagedRef(w) => {
                Ok(RequestBody::StagedRef(StagedReference::bind(w, context)?))
            }
            RequestBodyWire::QueryExpression(q) => Ok(RequestBody::QueryExpression(q)),
        }
    }

    /// Summarizes the wire request body for logging purposes.
    ///
    /// Intentionally avoids overly-verbose Debug dumps for common cases.
    pub fn summarize(&self) -> String {
        match self {
            RequestBodyWire::None => "  None".to_string(),

            RequestBodyWire::Holon(holon_wire) => {
                // If HolonWire has summarize(), prefer it. Otherwise fallback to Debug.
                // (HolonWire::bind exists, but we intentionally do not bind here.)
                format!("  HolonWire: {:#?}", holon_wire)
            }

            RequestBodyWire::TargetHolons(relationship_name, holons) => format!(
                "  relationship: {:?}, {{\n    holon_references: {:?}\n  }}",
                relationship_name, holons
            ),

            RequestBodyWire::TransientReference(reference_wire) => {
                format!("  TransientReferenceWire: {:?}", reference_wire)
            }

            RequestBodyWire::HolonId(holon_id) => format!("  HolonId: {:?}", holon_id),

            RequestBodyWire::ParameterValues(parameters) => {
                format!("  ParameterValues: keys={:?}", parameters.keys().collect::<Vec<_>>())
            }

            RequestBodyWire::StagedRef(reference_wire) => {
                format!("  StagedReferenceWire: {:?}", reference_wire)
            }

            RequestBodyWire::QueryExpression(query_expression) => {
                format!("  QueryExpression: {:#?}", query_expression)
            }
        }
    }
}

impl From<&DanceRequest> for DanceRequestWire {
    fn from(request: &DanceRequest) -> Self {
        Self {
            dance_name: request.dance_name.clone(),
            dance_type: DanceTypeWire::from(&request.dance_type),
            body: RequestBodyWire::from(&request.body),
            state: request.state.clone(),
        }
    }
}

impl From<&DanceType> for DanceTypeWire {
    fn from(dance_type: &DanceType) -> Self {
        match dance_type {
            DanceType::Standalone => DanceTypeWire::Standalone,
            DanceType::QueryMethod(node_collection) => {
                DanceTypeWire::QueryMethod(NodeCollectionWire::from(node_collection))
            }
            DanceType::CommandMethod(reference) => {
                DanceTypeWire::CommandMethod(HolonReferenceWire::from(reference))
            }
            DanceType::CloneMethod(reference) => {
                DanceTypeWire::CloneMethod(HolonReferenceWire::from(reference))
            }
            DanceType::NewVersionMethod(holon_id) => {
                DanceTypeWire::NewVersionMethod(holon_id.clone())
            }
            DanceType::DeleteMethod(local_id) => DanceTypeWire::DeleteMethod(local_id.clone()),
        }
    }
}

impl From<&RequestBody> for RequestBodyWire {
    fn from(body: &RequestBody) -> Self {
        match body {
            RequestBody::None => RequestBodyWire::None,
            RequestBody::Holon(holon) => RequestBodyWire::Holon(HolonWire::from(holon)),
            RequestBody::TargetHolons(relationship_name, references) => {
                RequestBodyWire::TargetHolons(
                    relationship_name.clone(),
                    references.iter().map(HolonReferenceWire::from).collect(),
                )
            }
            RequestBody::TransientReference(reference) => {
                RequestBodyWire::TransientReference(TransientReferenceWire::from(reference))
            }
            RequestBody::HolonId(holon_id) => RequestBodyWire::HolonId(holon_id.clone()),
            RequestBody::ParameterValues(parameters) => {
                RequestBodyWire::ParameterValues(parameters.clone())
            }
            RequestBody::StagedRef(reference) => {
                RequestBodyWire::StagedRef(StagedReferenceWire::from(reference))
            }
            RequestBody::QueryExpression(query_expression) => {
                RequestBodyWire::QueryExpression(query_expression.clone())
            }
        }
    }
}
