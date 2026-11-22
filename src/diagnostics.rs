use crate::json::Value;
use serde_json::json;

pub fn invalid_type(
    expected: &str,
    value: Option<&Value>,
    path: serde_json::Value,
) -> serde_json::Value {
    let received = value.map_or("undefined", Value::kind);
    json!({"expected":expected, "code":"invalid_type", "path":path, "message":format!("Invalid input: expected {expected}, received {received}")})
}

pub fn issues(errors: Vec<serde_json::Value>) -> String {
    serde_json::to_string_pretty(&errors).unwrap()
}
