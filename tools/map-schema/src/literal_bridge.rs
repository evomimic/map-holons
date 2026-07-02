use map_schema_semantic::{LiteralObject, LiteralValue};
use serde_json::{Map, Number, Value};

pub fn json_map_to_literal_object(map: &Map<String, Value>) -> LiteralObject {
    let mut object = LiteralObject::new();
    for (key, value) in map {
        object.insert(key.clone(), json_value_to_literal(value));
    }
    object
}

pub fn literal_object_to_json_map(object: &LiteralObject) -> Map<String, Value> {
    let mut map = Map::new();
    for (key, value) in object.iter() {
        map.insert(key.clone(), literal_value_to_json(value));
    }
    map
}

pub fn json_value_to_literal(value: &Value) -> LiteralValue {
    match value {
        Value::Null => LiteralValue::Null,
        Value::Bool(value) => LiteralValue::Boolean(*value),
        Value::Number(number) => {
            if let Some(int) = number.as_i64() {
                LiteralValue::Integer(int)
            } else {
                LiteralValue::Number(number.to_string())
            }
        }
        Value::String(value) => LiteralValue::String(value.clone()),
        Value::Array(values) => {
            LiteralValue::Array(values.iter().map(json_value_to_literal).collect())
        }
        Value::Object(map) => LiteralValue::Object(json_map_to_literal_object(map)),
    }
}

pub fn literal_value_to_json(value: &LiteralValue) -> Value {
    match value {
        LiteralValue::Null => Value::Null,
        LiteralValue::Boolean(value) => Value::Bool(*value),
        LiteralValue::Integer(value) => Value::Number(Number::from(*value)),
        LiteralValue::Number(value) => {
            if let Ok(int) = value.parse::<i64>() {
                Value::Number(Number::from(int))
            } else if let Ok(float) = value.parse::<f64>() {
                Number::from_f64(float)
                    .map(Value::Number)
                    .unwrap_or_else(|| Value::String(value.clone()))
            } else {
                Value::String(value.clone())
            }
        }
        LiteralValue::String(value) => Value::String(value.clone()),
        LiteralValue::Array(values) => {
            Value::Array(values.iter().map(literal_value_to_json).collect())
        }
        LiteralValue::Object(object) => Value::Object(literal_object_to_json_map(object)),
    }
}

pub fn render_literal_value(value: &LiteralValue) -> String {
    match value {
        LiteralValue::Null => "null".to_string(),
        LiteralValue::Boolean(value) => value.to_string(),
        LiteralValue::Integer(value) => value.to_string(),
        LiteralValue::Number(value) => value.clone(),
        LiteralValue::String(value) => {
            serde_json::to_string(value).unwrap_or_else(|_| "\"\"".to_string())
        }
        LiteralValue::Array(values) => {
            let rendered = values.iter().map(render_literal_value).collect::<Vec<_>>();
            format!("[{}]", rendered.join(", "))
        }
        LiteralValue::Object(object) => {
            let rendered = object
                .iter()
                .map(|(key, value)| {
                    format!(
                        "{}: {}",
                        serde_json::to_string(key).unwrap_or_else(|_| "\"\"".to_string()),
                        render_literal_value(value)
                    )
                })
                .collect::<Vec<_>>();
            format!("{{{}}}", rendered.join(", "))
        }
    }
}
