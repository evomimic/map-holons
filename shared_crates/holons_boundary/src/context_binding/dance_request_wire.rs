use crate::{HolonReferenceWire, StagedReferenceWire, TransientReferenceWire};
use core_types::{HolonError, HolonId, LocalId, PropertyMap, RelationshipName};
use holons_core::core_shared_objects::transactions::TransactionContext;
use holons_core::core_shared_objects::HolonWire;
use holons_core::dances::{DanceRequest, DanceType, RequestBody, SessionState};
use holons_core::query_layer::{NodeCollectionWire, QueryExpression};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

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

    /// Returns the session_state state, if present.
    pub fn get_state(&self) -> Option<&SessionState> {
        self.state.as_ref()
    }

    /// Returns a cloned session_state state.
    ///
    /// This is intentionally explicit to make cloning at boundaries obvious.
    pub fn cloned_state(&self) -> Option<SessionState> {
        self.state.clone()
    }

    /// Returns `true` if the request carries session_state state.
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
                Ok(DanceType::CommandMethod(reference_wire.bind(context)?))
            }
            DanceTypeWire::CloneMethod(reference_wire) => {
                Ok(DanceType::CloneMethod(reference_wire.bind(context)?))
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
                    refs.push(w.bind(context)?);
                }
                Ok(RequestBody::TargetHolons(name, refs))
            }
            RequestBodyWire::TransientReference(w) => {
                Ok(RequestBody::TransientReference(w.bind(context)?))
            }
            RequestBodyWire::HolonId(id) => Ok(RequestBody::HolonId(id)),
            RequestBodyWire::ParameterValues(p) => Ok(RequestBody::ParameterValues(p)),
            RequestBodyWire::StagedRef(w) => Ok(RequestBody::StagedRef(w.bind(context)?)),
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
