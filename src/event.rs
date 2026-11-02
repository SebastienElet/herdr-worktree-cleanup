use crate::json::{Document, Value};
use serde_json::json;

pub struct Worktree {
    pub checkout_path: String,
    pub is_linked_worktree: bool,
    pub repo_root: String,
}

pub fn parse(input: &str) -> Result<Option<Worktree>, String> {
    let document = Document::parse(input)?;
    let mut current = Some(document.root());
    let mut path = Vec::new();
    for (key, optional) in [("data", false), ("workspace", true), ("worktree", true)] {
        if !matches!(current, Some(Value::Object(_))) {
            return Err(issues(vec![invalid_type("object", current, &path)]));
        }
        current = document.property(current.unwrap(), key);
        path.push(key);
        if optional && current.is_none() {
            return Ok(None);
        }
    }
    if !matches!(current, Some(Value::Object(_))) {
        return Err(issues(vec![invalid_type("object", current, &path)]));
    }
    let worktree = current.unwrap();
    let mut errors = Vec::new();
    for (key, expected) in [
        ("checkout_path", "string"),
        ("is_linked_worktree", "boolean"),
        ("repo_root", "string"),
    ] {
        let value = document.property(worktree, key);
        let mut field_path = path.clone();
        field_path.push(key);
        if value.map(Value::kind) != Some(expected) {
            errors.push(invalid_type(expected, value, &field_path));
        }
        if expected == "string" {
            let origin = match value {
                Some(Value::String(s)) if s.is_empty() => {
                    Some(("string", "to have >=1 characters"))
                }
                Some(Value::Array(items)) if items.is_empty() => {
                    Some(("array", "to have >=1 items"))
                }
                Some(value @ Value::Object(_))
                    if document.property(value, "length").is_some_and(
                        |length| !matches!(document.number(length), Ok(n) if n >= 1.0),
                    ) =>
                {
                    document.number(document.property(value, "length").unwrap())?;
                    Some(("unknown", "to be >=1"))
                }
                _ => None,
            };
            if let Some((origin, message)) = origin {
                errors.push(json!({"origin":origin, "code":"too_small", "minimum":1, "inclusive":true, "path":field_path, "message":format!("Too small: expected {origin} {message}")}));
            }
        }
    }
    if !errors.is_empty() {
        return Err(issues(errors));
    }
    let Some(Value::String(checkout_path)) = document.property(worktree, "checkout_path") else {
        unreachable!()
    };
    let Some(Value::Bool(is_linked_worktree)) = document.property(worktree, "is_linked_worktree")
    else {
        unreachable!()
    };
    let Some(Value::String(repo_root)) = document.property(worktree, "repo_root") else {
        unreachable!()
    };
    Ok(Some(Worktree {
        checkout_path: checkout_path.clone(),
        is_linked_worktree: *is_linked_worktree,
        repo_root: repo_root.clone(),
    }))
}

fn invalid_type(expected: &str, value: Option<&Value>, path: &[&str]) -> serde_json::Value {
    let received = value.map_or("undefined", Value::kind);
    json!({"expected":expected, "code":"invalid_type", "path":path, "message":format!("Invalid input: expected {expected}, received {received}")})
}

fn issues(errors: Vec<serde_json::Value>) -> String {
    serde_json::to_string_pretty(&errors).unwrap()
}
