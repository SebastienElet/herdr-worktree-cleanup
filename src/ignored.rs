use crate::{
    diagnostics::{invalid_type, issues},
    git,
    json::{self, Document, Value},
};
use serde_json::json;

pub const OPTION: &str = "--disposable-ignored-directories";
const DEFAULTS: [&str; 3] = ["node_modules", "target", ".moon/cache"];

pub fn disposable_directories(option: Option<&str>) -> Result<Vec<String>, String> {
    let Some(option) = option else {
        return Ok(DEFAULTS.map(str::to_owned).into());
    };
    let document = Document::parse(option)?;
    let root = document.root();
    let Some(entries) = document.elements(root) else {
        return Err(issues(vec![invalid_type("array", Some(root), json!([]))]));
    };
    let mut directories = Vec::new();
    let mut errors = Vec::new();
    for (index, entry) in entries.enumerate() {
        match entry {
            Value::String(directory)
                if !directory.contains(['\\', '\0'])
                    && directory
                        .split('/')
                        .all(|segment| !matches!(segment, "" | "." | "..")) =>
            {
                directories.push(directory.clone());
            }
            Value::String(_) => errors.push(json!({
                "code": "custom",
                "path": [index],
                "message": "expected a relative directory without empty, dot or parent segments",
            })),
            _ => errors.push(invalid_type("string", Some(entry), json!([index]))),
        }
    }
    if errors.is_empty() {
        Ok(directories)
    } else {
        Err(issues(errors))
    }
}

pub fn protect(checkout: &str, disposable: &[String]) -> Result<(), String> {
    let exclusions: Vec<_> = disposable
        .iter()
        .map(|directory| format!(":(top,exclude,literal){directory}/"))
        .collect();
    let mut args = vec![
        "--work-tree",
        checkout,
        "ls-files",
        "--others",
        "--ignored",
        "--exclude-standard",
        "-z",
        "--",
        ".",
    ];
    args.extend(exclusions.iter().map(String::as_str));
    let result =
        git(checkout, &args).map_err(|error| format!("cannot inspect ignored files: {error}"))?;
    let stderr = String::from_utf8_lossy(&result.stderr);
    let warning = json::trim(&stderr);
    if !warning.is_empty() {
        return Err(format!("cannot inspect ignored files: {warning}"));
    }
    if !result.status.success() {
        return Err(format!("cannot inspect ignored files: {}", result.status));
    }
    if !result.stdout.is_empty() && !result.stdout.ends_with(&[0]) {
        return Err("cannot inspect ignored files: incomplete Git output".to_owned());
    }
    if let Some(path) = result
        .stdout
        .split(|byte| *byte == 0)
        .find(|path| !path.is_empty())
    {
        let path = json!(String::from_utf8_lossy(path));
        return Err(format!(
            "protected ignored path {path}; keep the worktree or explicitly declare its directory disposable"
        ));
    }
    Ok(())
}
