use crate::{HolonReferenceWire, RowSetWire, RowWire};
use core_types::HolonError;
use holons_core::core_shared_objects::transactions::TransactionContext;
use holons_core::query_layer::{
    QueryDiagnostic, QueryRequest, QueryResult, QueryResultData, QuerySpec, Value,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

/// Wire-form query request envelope.
///
/// This is the adapter-facing contract carried across the client/host boundary
/// before binding into the shared query substrate.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct QueryRequestWire {
    pub target_refs: Vec<HolonReferenceWire>,
    pub query: QuerySpec,
    pub parameters: Option<RowWire>,
}

impl QueryRequestWire {
    pub fn new(
        target_refs: Vec<HolonReferenceWire>,
        query: QuerySpec,
        parameters: Option<RowWire>,
    ) -> Self {
        Self { target_refs, query, parameters }
    }

    pub fn bind(self, context: &Arc<TransactionContext>) -> Result<QueryRequest, HolonError> {
        let mut target_refs = Vec::with_capacity(self.target_refs.len());
        for target_ref in self.target_refs {
            target_refs.push(target_ref.bind(context)?);
        }

        Ok(QueryRequest::new(target_refs, self.query, self.parameters.map(RowWire::bind)))
    }
}

/// Wire-form materialized query result envelope.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct QueryResultWire {
    pub data: Option<QueryResultDataWire>,
    pub diagnostics: Vec<QueryDiagnosticWire>,
}

impl QueryResultWire {
    pub fn new(data: Option<QueryResultDataWire>, diagnostics: Vec<QueryDiagnosticWire>) -> Self {
        Self { data, diagnostics }
    }
}

/// Wire-form query result payload kinds.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum QueryResultDataWire {
    Value(Value),
    Row(RowWire),
    RowSet(RowSetWire),
}

/// Wire-form non-fatal query diagnostics.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct QueryDiagnosticWire {
    pub code: String,
    pub message: String,
}

impl QueryDiagnosticWire {
    pub fn new(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self { code: code.into(), message: message.into() }
    }
}

impl From<&QueryResult> for QueryResultWire {
    fn from(result: &QueryResult) -> Self {
        Self {
            data: result.data.as_ref().map(QueryResultDataWire::from),
            diagnostics: result.diagnostics.iter().map(QueryDiagnosticWire::from).collect(),
        }
    }
}

impl From<&QueryResultData> for QueryResultDataWire {
    fn from(data: &QueryResultData) -> Self {
        match data {
            QueryResultData::Value(value) => Self::Value(value.clone()),
            QueryResultData::Row(row) => Self::Row(RowWire::from(row)),
            QueryResultData::RowSet(rowset) => Self::RowSet(RowSetWire::from(rowset)),
        }
    }
}

impl From<&QueryDiagnostic> for QueryDiagnosticWire {
    fn from(diagnostic: &QueryDiagnostic) -> Self {
        Self::new(diagnostic.code.clone(), diagnostic.message.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::{QueryRequestWire, QueryResultWire};
    use base_types::{BaseValue, MapString};
    use core_types::RelationshipName;
    use holons_core::query_layer::{
        QueryDiagnostic, QueryExpression, QueryResult, QueryResultData, QuerySpec, Row, RowSet,
    };
    use serde_json::{json, to_value};
    use std::collections::BTreeMap;

    #[test]
    fn query_result_wire_serializes_materialized_rowset_payload() {
        let result = QueryResult::new(
            Some(QueryResultData::RowSet(RowSet::new(vec![Row::new(BTreeMap::from([(
                "title".to_string(),
                BaseValue::StringValue(MapString("alpha".to_string())),
            )]))]))),
            vec![QueryDiagnostic::new("ok", "shape stabilized")],
        );

        assert_eq!(
            to_value(QueryResultWire::from(&result)).expect("serialize query result wire"),
            json!({
                "data": {
                    "RowSet": {
                        "rows": [
                            { "title": { "StringValue": "alpha" } }
                        ]
                    }
                },
                "diagnostics": [
                    { "code": "ok", "message": "shape stabilized" }
                ]
            })
        );
    }

    #[test]
    fn query_request_wire_serializes_legacy_shape_without_eager_projection_requirements() {
        let request = QueryRequestWire::new(
            vec![],
            QuerySpec::LegacyRelationshipTraversal(QueryExpression::new(RelationshipName(
                MapString("children".to_string()),
            ))),
            Some(
                Row::new(BTreeMap::from([(
                    "status".to_string(),
                    BaseValue::StringValue(MapString("Active".to_string())),
                )]))
                .into(),
            ),
        );

        assert_eq!(
            to_value(&request).expect("serialize query request wire"),
            json!({
                "target_refs": [],
                "query": {
                    "LegacyRelationshipTraversal": {
                        "relationship_name": "children"
                    }
                },
                "parameters": {
                    "status": { "StringValue": "Active" }
                }
            })
        );
    }
}
