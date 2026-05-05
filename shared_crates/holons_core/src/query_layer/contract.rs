use crate::reference_layer::HolonReference;
use serde::{Deserialize, Serialize};

use super::{QueryExpression, Row, RowSet, Value};

/// Substrate-facing query request envelope.
///
/// This is the canonical runtime contract carried below ingress adapters such
/// as Commands. It intentionally allows query execution to retain richer
/// holon-bound state internally rather than requiring eager row materialization.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QueryRequest {
    pub target_refs: Vec<HolonReference>,
    pub query: QuerySpec,
    pub parameters: Option<Row>,
}

impl QueryRequest {
    pub fn new(
        target_refs: Vec<HolonReference>,
        query: QuerySpec,
        parameters: Option<Row>,
    ) -> Self {
        Self { target_refs, query, parameters }
    }
}

/// Query request shape discriminator.
///
/// `PRO2` stabilizes only the envelope direction. The request body remains
/// intentionally narrow so later semantic slices can expand it without
/// requiring a new ingress path.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum QuerySpec {
    LegacyRelationshipTraversal(QueryExpression),
}

/// Materialized query result envelope returned by the shared substrate.
///
/// This is a boundary/result shape. Internal execution may retain richer
/// bindings and materialize these payloads only when a contract or operator
/// requires them.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct QueryResult {
    pub data: Option<QueryResultData>,
    pub diagnostics: Vec<QueryDiagnostic>,
}

impl QueryResult {
    pub fn new(data: Option<QueryResultData>, diagnostics: Vec<QueryDiagnostic>) -> Self {
        Self { data, diagnostics }
    }
}

/// Materialized result payload kinds for query boundaries.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum QueryResultData {
    Value(Value),
    Row(Row),
    RowSet(RowSet),
}

/// Non-fatal query diagnostics emitted alongside a materialized result.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct QueryDiagnostic {
    pub code: String,
    pub message: String,
}

impl QueryDiagnostic {
    pub fn new(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self { code: code.into(), message: message.into() }
    }
}

#[cfg(test)]
mod tests {
    use super::{QueryDiagnostic, QueryResult, QueryResultData, QuerySpec};
    use crate::query_layer::{QueryExpression, Row, RowSet};
    use base_types::{BaseValue, MapString};
    use core_types::RelationshipName;
    use serde_json::{json, to_value};
    use std::collections::BTreeMap;

    #[test]
    fn query_result_serializes_rowset_payload_and_diagnostics() {
        let result = QueryResult::new(
            Some(QueryResultData::RowSet(RowSet::new(vec![Row::new(BTreeMap::from([(
                "title".to_string(),
                BaseValue::StringValue(MapString("alpha".to_string())),
            )]))]))),
            vec![QueryDiagnostic::new("legacy_bridge", "using legacy query substrate")],
        );

        assert_eq!(
            to_value(&result).expect("serialize query result"),
            json!({
                "data": {
                    "RowSet": {
                        "rows": [
                            { "title": { "StringValue": "alpha" } }
                        ]
                    }
                },
                "diagnostics": [
                    {
                        "code": "legacy_bridge",
                        "message": "using legacy query substrate"
                    }
                ]
            })
        );
    }

    #[test]
    fn query_spec_serializes_legacy_relationship_traversal() {
        let spec = QuerySpec::LegacyRelationshipTraversal(QueryExpression::new(RelationshipName(
            MapString("children".to_string()),
        )));

        assert_eq!(
            to_value(&spec).expect("serialize query spec"),
            json!({
                "LegacyRelationshipTraversal": {
                    "relationship_name": "children"
                }
            })
        );
    }
}
