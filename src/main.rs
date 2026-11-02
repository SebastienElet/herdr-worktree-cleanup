mod docker;
mod event;
mod json;

use std::{
    env,
    path::Path,
    process::{Command, ExitCode, Stdio},
};

fn git(directory: &str, args: &[&str]) -> std::io::Result<std::process::Output> {
    Command::new("git")
        .arg("-C")
        .arg(directory)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .output()
}

fn remove(input: &str, stop_docker: bool) -> Result<String, String> {
    let Some(worktree) = event::parse(input)? else {
        return Ok("skipped: event carries no worktree snapshot".to_owned());
    };
    if !worktree.is_linked_worktree {
        return Ok("skipped: workspace is not a linked worktree".to_owned());
    }
    if !Path::new(&worktree.checkout_path).exists() {
        return Ok("skipped: checkout no longer exists".to_owned());
    }
    if stop_docker {
        docker::cleanup(&worktree.checkout_path);
    }
    let head = git(
        &worktree.checkout_path,
        &["symbolic-ref", "--quiet", "HEAD"],
    )
    .map_err(|e| e.to_string())?;
    if !head.status.success() {
        return Ok("skipped: checkout HEAD is detached and its commits would be lost".to_owned());
    }
    let removal = git(
        &worktree.repo_root,
        &[
            "-c",
            "status.showUntrackedFiles=all",
            "worktree",
            "remove",
            &worktree.checkout_path,
        ],
    )
    .map_err(|e| e.to_string())?;
    if !removal.status.success() {
        return Err(json::trim(&String::from_utf8_lossy(&removal.stderr)).to_owned());
    }
    Ok(format!("removed {}", worktree.checkout_path))
}

fn main() -> ExitCode {
    match remove(
        &env::var("HERDR_PLUGIN_EVENT_JSON").unwrap_or_default(),
        env::args_os().any(|arg| arg == "--docker-cleanup=stop"),
    ) {
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
