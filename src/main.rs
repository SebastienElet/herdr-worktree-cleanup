mod diagnostics;
mod docker;
mod event;
mod ignored;
mod json;

use std::{
    env,
    path::Path,
    process::{Command, ExitCode, Stdio},
};

fn git(directory: &str, args: &[&str]) -> std::io::Result<std::process::Output> {
    let mut command = Command::new("git");
    for variable in [
        "GIT_ALTERNATE_OBJECT_DIRECTORIES",
        "GIT_CONFIG",
        "GIT_CONFIG_PARAMETERS",
        "GIT_CONFIG_COUNT",
        "GIT_OBJECT_DIRECTORY",
        "GIT_DIR",
        "GIT_WORK_TREE",
        "GIT_IMPLICIT_WORK_TREE",
        "GIT_GRAFT_FILE",
        "GIT_INDEX_FILE",
        "GIT_NO_REPLACE_OBJECTS",
        "GIT_REPLACE_REF_BASE",
        "GIT_PREFIX",
        "GIT_SHALLOW_FILE",
        "GIT_COMMON_DIR",
    ] {
        command.env_remove(variable);
    }
    command
        .arg("-C")
        .arg(directory)
        .args(args)
        .stdin(Stdio::null())
        .output()
}

fn remove(input: &str, stop_docker: bool, disposable: &[String]) -> Result<String, String> {
    let Some(worktree) = event::parse(input)? else {
        return Ok("skipped: event carries no worktree snapshot".to_owned());
    };
    if !worktree.is_linked_worktree {
        return Ok("skipped: workspace is not a linked worktree".to_owned());
    }
    let checkout = Path::new(&worktree.checkout_path);
    if !checkout.is_absolute() {
        return Err("checkout path must be absolute".to_owned());
    }
    if !checkout.exists() {
        return Ok("skipped: checkout no longer exists".to_owned());
    }
    let head = git(
        &worktree.checkout_path,
        &["symbolic-ref", "--quiet", "HEAD"],
    )
    .map_err(|e| e.to_string())?;
    if !head.status.success() {
        return Ok("skipped: checkout HEAD is detached and its commits would be lost".to_owned());
    }
    ignored::protect(&worktree.checkout_path, disposable)?;
    if stop_docker {
        docker::cleanup(&worktree.checkout_path);
    }
    let removal = git(
        &worktree.repo_root,
        &[
            "-c",
            "status.showUntrackedFiles=all",
            "worktree",
            "remove",
            "--",
            &worktree.checkout_path,
        ],
    )
    .map_err(|e| e.to_string())?;
    if !removal.status.success() {
        return Err(json::trim(&String::from_utf8_lossy(&removal.stderr)).to_owned());
    }
    Ok(format!("removed {}", worktree.checkout_path))
}

fn remove_from_command_line() -> Result<String, String> {
    let mut stop_docker = false;
    let mut disposable_option = None;
    for argument in env::args_os().skip(1) {
        let Some(argument) = argument.to_str() else {
            if argument
                .as_encoded_bytes()
                .starts_with(ignored::OPTION.as_bytes())
            {
                return Err("invalid disposable ignored directories: expected UTF-8".to_owned());
            }
            continue;
        };
        if argument == "--docker-cleanup=stop" {
            stop_docker = true;
        }
        if argument.starts_with(ignored::OPTION) {
            let value = argument
                .strip_prefix(ignored::OPTION)
                .and_then(|value| value.strip_prefix('='));
            if disposable_option.is_some() || value.is_none() {
                return Err(
                    "invalid disposable ignored directories: use one JSON array option".to_owned(),
                );
            }
            disposable_option = value.map(str::to_owned);
        }
    }
    let disposable = ignored::disposable_directories(disposable_option.as_deref())
        .map_err(|reason| format!("invalid disposable ignored directories: {reason}"))?;
    remove(
        &env::var("HERDR_PLUGIN_EVENT_JSON").unwrap_or_default(),
        stop_docker,
        &disposable,
    )
}

fn main() -> ExitCode {
    match remove_from_command_line() {
        Ok(message) => {
            println!("{message}");
            ExitCode::SUCCESS
        }
        Err(reason) => {
            println!("failed: {reason}");
            ExitCode::FAILURE
        }
    }
}
