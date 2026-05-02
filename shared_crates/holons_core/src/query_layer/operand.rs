use base_types::BaseValue;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Shared scalar/domain operand payload.
///
/// This is the operand-facing name for MAP's existing scalar value family.
/// It intentionally preserves the current `BaseValue` wire shape so adjacent
/// contracts can converge on shared vocabulary without introducing new scalar
/// encodings in this slice.
pub type Value = BaseValue;

/// A single row-shaped projection.
///
/// Rows are string-keyed result objects whose fields are projection labels, not
/// descriptor-resolved property names. Row values are always scalar/domain
/// `Value` payloads; nested row or rowset operands are intentionally excluded
/// from this foundational family.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct Row(pub BTreeMap<String, Value>);

impl Row {
    pub fn new(fields: BTreeMap<String, Value>) -> Self {
        Self(fields)
    }

    pub fn fields(&self) -> &BTreeMap<String, Value> {
        &self.0
    }
}

/// An ordered collection of row-shaped projections.
///
/// `RowSet` is collection-shaped only: it preserves row order, but does not
/// define planner, descriptor, cursor, streaming, or distributed semantics.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct RowSet {
    pub rows: Vec<Row>,
}

impl RowSet {
    pub fn new(rows: Vec<Row>) -> Self {
        Self { rows }
    }

    pub fn new_empty() -> Self {
        Self { rows: Vec::new() }
    }
}

#[cfg(test)]
mod tests {
    use super::{Row, RowSet};
    use base_types::{BaseValue, MapBoolean, MapInteger, MapString};
    use serde_json::{from_str, json, to_value};
    use std::collections::BTreeMap;

    #[test]
    fn row_serializes_with_string_projection_labels() {
        let row = Row::new(BTreeMap::from([
            ("title".to_string(), BaseValue::StringValue(MapString("alpha".to_string()))),
            ("rank".to_string(), BaseValue::IntegerValue(MapInteger(7))),
        ]));

        let json = to_value(&row).expect("serialize row");
        assert_eq!(
            json,
            json!({
                "rank": { "IntegerValue": 7 },
                "title": { "StringValue": "alpha" }
            })
        );
    }

    #[test]
    fn rowset_serializes_as_ordered_rows() {
        let rowset = RowSet::new(vec![
            Row::new(BTreeMap::from([(
                "title".to_string(),
                BaseValue::StringValue(MapString("alpha".to_string())),
            )])),
            Row::new(BTreeMap::from([(
                "published".to_string(),
                BaseValue::BooleanValue(MapBoolean(true)),
            )])),
        ]);

        let json = to_value(&rowset).expect("serialize rowset");
        assert_eq!(
            json,
            json!({
                "rows": [
                    { "title": { "StringValue": "alpha" } },
                    { "published": { "BooleanValue": true } }
                ]
            })
        );
    }

    #[test]
    fn empty_row_and_rowset_round_trip() {
        let row = Row::default();
        let rowset = RowSet::new_empty();

        let row_json = serde_json::to_string(&row).expect("serialize empty row");
        let rowset_json = serde_json::to_string(&rowset).expect("serialize empty rowset");

        assert_eq!(serde_json::from_str::<Row>(&row_json).expect("deserialize row"), row);
        assert_eq!(
            serde_json::from_str::<RowSet>(&rowset_json).expect("deserialize rowset"),
            rowset
        );
    }

    #[test]
    fn row_rejects_nested_row_shapes() {
        let invalid_row = r#"{"child":{"title":{"StringValue":"nested"}}}"#;
        let error = from_str::<Row>(invalid_row).expect_err("nested row should fail");

        assert!(
            error.to_string().contains("unknown variant")
                || error.to_string().contains("invalid type"),
            "unexpected error: {error}"
        );
    }
}
