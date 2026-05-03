use holons_core::query_layer::{Row, RowSet};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Wire-form row operand.
///
/// This is a context-free projection object keyed by projection label strings.
/// Values retain the existing shared scalar encoding via `BaseValue`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct RowWire(pub BTreeMap<String, base_types::BaseValue>);

impl RowWire {
    pub fn new(fields: BTreeMap<String, base_types::BaseValue>) -> Self {
        Self(fields)
    }

    pub fn bind(self) -> Row {
        Row::new(self.0)
    }
}

/// Wire-form ordered collection of rows.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct RowSetWire {
    pub rows: Vec<RowWire>,
}

impl RowSetWire {
    pub fn new(rows: Vec<RowWire>) -> Self {
        Self { rows }
    }

    pub fn bind(self) -> RowSet {
        RowSet::new(self.rows.into_iter().map(RowWire::bind).collect())
    }
}

impl From<&Row> for RowWire {
    fn from(row: &Row) -> Self {
        Self::new(row.0.clone())
    }
}

impl From<&RowSet> for RowSetWire {
    fn from(rowset: &RowSet) -> Self {
        Self::new(rowset.rows.iter().map(RowWire::from).collect())
    }
}

#[cfg(test)]
mod tests {
    use super::{RowSetWire, RowWire};
    use base_types::{BaseValue, MapInteger, MapString};
    use holons_core::query_layer::{Row, RowSet};
    use serde_json::{json, to_value};
    use std::collections::BTreeMap;

    #[test]
    fn row_wire_binds_to_runtime_row() {
        let wire = RowWire::new(BTreeMap::from([(
            "title".to_string(),
            BaseValue::StringValue(MapString("alpha".to_string())),
        )]));
        let runtime = wire.clone().bind();

        assert_eq!(
            runtime.clone(),
            Row::new(BTreeMap::from([(
                "title".to_string(),
                BaseValue::StringValue(MapString("alpha".to_string())),
            )]))
        );
        assert_eq!(RowWire::from(&runtime), wire);
    }

    #[test]
    fn rowset_wire_serializes_with_expected_shape() {
        let rowset = RowSet::new(vec![
            Row::new(BTreeMap::from([(
                "rank".to_string(),
                BaseValue::IntegerValue(MapInteger(3)),
            )])),
            Row::new(BTreeMap::from([(
                "title".to_string(),
                BaseValue::StringValue(MapString("beta".to_string())),
            )])),
        ]);

        let json = to_value(RowSetWire::from(&rowset)).expect("serialize rowset wire");
        assert_eq!(
            json,
            json!({
                "rows": [
                    { "rank": { "IntegerValue": 3 } },
                    { "title": { "StringValue": "beta" } }
                ]
            })
        );
    }
}
