use serde_json::{Value, json};
use std::{
    fs,
    path::{Path, PathBuf},
    process::{Command, Output},
};
use tempfile::{Builder, TempDir};

fn isolated(command: &mut Command) -> &mut Command {
    for key in [
        "GIT_DIR",
        "GIT_WORK_TREE",
        "GIT_INDEX_FILE",
        "GIT_COMMON_DIR",
        "GIT_CONFIG_COUNT",
        "GIT_OBJECT_DIRECTORY",
        "GIT_ALTERNATE_OBJECT_DIRECTORIES",
    ] {
        command.env_remove(key);
    }
    command
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
}

fn git(directory: &Path, args: &[&str]) -> String {
    let output = isolated(
        Command::new("git")
            .args([
                "-c",
                "user.email=test@example.com",
                "-c",
                "user.name=Test",
                "-c",
                "commit.gpgSign=false",
                "-c",
                "core.hooksPath=/dev/null",
                "-C",
            ])
            .arg(directory)
            .args(args),
    )
    .output()
    .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).unwrap().trim().to_owned()
}

struct Repository {
    _root: TempDir,
    repository: PathBuf,
    checkout: PathBuf,
}

impl Repository {
    fn new() -> Self {
        let root = Builder::new().prefix("herdr cleanup ").tempdir().unwrap();
        let repository = root.path().join("repository");
        let checkout = root.path().join("checkout with spaces");
        fs::create_dir(&repository).unwrap();
        git(&repository, &["init", "-q", "-b", "main"]);
        git(
            &repository,
            &["commit", "-q", "--allow-empty", "-m", "initial"],
        );
        git(
            &repository,
            &[
                "worktree",
                "add",
                "-q",
                "-b",
                "feature",
                checkout.to_str().unwrap(),
            ],
        );
        Self {
            _root: root,
            repository,
            checkout,
        }
    }

    fn event(&self) -> Value {
        json!({"event":"workspace_closed", "data":{"type":"workspace_closed", "workspace_id":"w1", "workspace":{"workspace_id":"w1", "label":"feature", "worktree":{"checkout_path":self.checkout, "is_linked_worktree":true, "repo_key":self.repository.join(".git"), "repo_name":"repository", "repo_root":self.repository}}}})
    }

    fn invoke(&self) -> Output {
        invoke(&self.event().to_string(), &[])
    }

    fn branch_survives(&self) {
        assert!(git(&self.repository, &["branch", "--list", "feature"]).contains("feature"));
    }
}

fn command(event: &str, args: &[&str]) -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_herdr-worktree-cleanup"));
    isolated(&mut command)
        .env("HERDR_PLUGIN_EVENT_JSON", event)
        .args(args);
    command
}

fn invoke(event: &str, args: &[&str]) -> Output {
    command(event, args).output().unwrap()
}

fn assert_result(output: &Output, status: i32, stdout: &str) {
    assert_eq!(output.status.code(), Some(status), "{output:?}");
    assert_eq!(String::from_utf8_lossy(&output.stdout), stdout);
    assert!(output.stderr.is_empty(), "{output:?}");
}

#[test]
fn clean_checkout_is_removed_and_branch_is_kept() {
    for visibility in ["normal", "no"] {
        let repo = Repository::new();
        git(
            &repo.repository,
            &["config", "status.showUntrackedFiles", visibility],
        );
        assert_result(
            &repo.invoke(),
            0,
            &format!("removed {}\n", repo.checkout.display()),
        );
        assert!(!repo.checkout.exists());
        repo.branch_survives();
    }
}

#[test]
fn untracked_files_survive_even_when_hidden_by_git_configuration() {
    for visibility in ["normal", "no"] {
        let repo = Repository::new();
        git(
            &repo.repository,
            &["config", "status.showUntrackedFiles", visibility],
        );
        let notes = repo.checkout.join("notes.txt");
        fs::write(&notes, "work in progress\n").unwrap();
        let output = repo.invoke();
        assert_eq!(output.status.code(), Some(1));
        assert!(String::from_utf8_lossy(&output.stdout).starts_with("failed: "));
        assert!(output.stderr.is_empty());
        assert_eq!(fs::read_to_string(notes).unwrap(), "work in progress\n");
        repo.branch_survives();
    }
}

#[test]
fn tracked_changes_survive_staged_or_unstaged() {
    for staged in [false, true] {
        let repo = Repository::new();
        let file = repo.checkout.join("tracked.txt");
        fs::write(&file, "committed\n").unwrap();
        git(&repo.checkout, &["add", "."]);
        git(&repo.checkout, &["commit", "-qm", "tracked"]);
        fs::write(&file, "modified\n").unwrap();
        if staged {
            git(&repo.checkout, &["add", "."]);
        }
        assert_eq!(repo.invoke().status.code(), Some(1));
        assert_eq!(fs::read_to_string(file).unwrap(), "modified\n");
    }
}

#[test]
fn locked_checkout_and_deployed_symlink_survive() {
    let repo = Repository::new();
    let source = repo.checkout.join("skill.md");
    let deployed = repo.repository.join("deployed-skill.md");
    fs::write(&source, "fixture skill\n").unwrap();
    git(&repo.checkout, &["add", "skill.md"]);
    git(&repo.checkout, &["commit", "-qm", "fixture skill"]);
    std::os::unix::fs::symlink(&source, &deployed).unwrap();
    git(
        &repo.repository,
        &[
            "worktree",
            "lock",
            "--reason",
            "deployment source",
            repo.checkout.to_str().unwrap(),
        ],
    );
    let output = repo.invoke();
    assert_eq!(output.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&output.stdout).starts_with("failed: "));
    assert_eq!(fs::read_link(&deployed).unwrap(), source);
    assert_eq!(fs::read_to_string(deployed).unwrap(), "fixture skill\n");
    repo.branch_survives();
}

#[test]
fn ignored_files_disappear_with_an_otherwise_clean_checkout() {
    let repo = Repository::new();
    git(
        &repo.repository,
        &["config", "status.showUntrackedFiles", "no"],
    );
    fs::write(
        repo.repository.join(".git/info/exclude"),
        "ignored-notes.txt\n",
    )
    .unwrap();
    fs::write(
        repo.checkout.join("ignored-notes.txt"),
        "disposable ignored notes\n",
    )
    .unwrap();
    assert_result(
        &repo.invoke(),
        0,
        &format!("removed {}\n", repo.checkout.display()),
    );
    assert!(!repo.checkout.exists());
}

#[test]
fn primary_checkout_is_skipped() {
    let repo = Repository::new();
    let mut event = repo.event();
    event["data"]["workspace"]["worktree"]["checkout_path"] = json!(repo.repository);
    event["data"]["workspace"]["worktree"]["is_linked_worktree"] = json!(false);
    assert_result(
        &invoke(&event.to_string(), &[]),
        0,
        "skipped: workspace is not a linked worktree\n",
    );
    assert!(repo.repository.join(".git").exists());
    assert!(repo.checkout.exists());
}

#[test]
fn detached_head_and_its_unreferenced_commits_survive() {
    let repo = Repository::new();
    git(&repo.checkout, &["checkout", "-q", "--detach"]);
    git(
        &repo.checkout,
        &["commit", "-q", "--allow-empty", "-m", "unreferenced"],
    );
    let commit = git(&repo.checkout, &["rev-parse", "HEAD"]);
    assert_result(
        &repo.invoke(),
        0,
        "skipped: checkout HEAD is detached and its commits would be lost\n",
    );
    assert_eq!(git(&repo.checkout, &["rev-parse", "HEAD"]), commit);
}

#[test]
fn already_removed_checkout_is_skipped() {
    let repo = Repository::new();
    fs::remove_dir_all(&repo.checkout).unwrap();
    assert_result(&repo.invoke(), 0, "skipped: checkout no longer exists\n");
}

#[test]
fn foreign_checkout_is_refused_by_git_and_preserved() {
    let repo = Repository::new();
    let foreign = Repository::new();
    let mut event = repo.event();
    event["data"]["workspace"]["worktree"]["repo_root"] = json!(foreign.repository);
    let output = invoke(&event.to_string(), &[]);
    assert_eq!(output.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&output.stdout).starts_with("failed: "));
    assert!(repo.checkout.exists());
    repo.branch_survives();
}

#[test]
fn event_diagnostics_match_the_typescript_reference() {
    let fixtures: Value = serde_json::from_str(include_str!("fixtures/events.json")).unwrap();
    for case in fixtures.as_array().unwrap() {
        let input = case["input"].as_str().unwrap();
        let output = invoke(input, &[]);
        assert_eq!(
            String::from_utf8_lossy(&output.stdout),
            case["stdout"].as_str().unwrap(),
            "input: {input:?}"
        );
        assert_eq!(
            output.status.code().map(i64::from),
            case["status"].as_i64(),
            "input: {input:?}"
        );
        assert!(output.stderr.is_empty(), "{output:?}");
    }
    let output = Command::new(env!("CARGO_BIN_EXE_herdr-worktree-cleanup"))
        .env_remove("HERDR_PLUGIN_EVENT_JSON")
        .output()
        .unwrap();
    assert_result(&output, 1, "failed: JSON Parse error: Unexpected EOF\n");
}

fn docker(
    repo: &Repository,
    mode: &str,
    ids: &str,
    ps_status: &str,
    stop_status: &str,
    delay: &str,
) -> (Output, String) {
    use std::os::unix::fs::PermissionsExt;
    let bin = repo._root.path().join("substituted-bin");
    fs::create_dir(&bin).unwrap();
    std::os::unix::fs::symlink("/usr/bin/git", bin.join("git")).unwrap();
    if ps_status != "missing" {
        let script = bin.join("docker");
        fs::write(&script, "#!/bin/sh\nprintf '[%s]\\n' \"$@\" >> \"$DOCKER_LOG\"\nprintf '\\n' >> \"$DOCKER_LOG\"\nif [ \"$DOCKER_DELAY\" = \"$1\" ]; then\n  printf '%s' \"$$\" > \"$DOCKER_PID\"\n  exec /bin/sleep 60\nfi\nif [ \"$1\" = ps ]; then\n  printf '%s' \"$DOCKER_IDS\"\n  exit \"$DOCKER_PS_STATUS\"\nfi\nexit \"$DOCKER_STOP_STATUS\"\n").unwrap();
        fs::set_permissions(script, fs::Permissions::from_mode(0o755)).unwrap();
    }
    let log = repo._root.path().join("docker.log");
    let pid = repo._root.path().join("docker.pid");
    let args: Vec<&str> = mode.split(' ').collect();
    let output = command(&repo.event().to_string(), &args)
        .env("PATH", bin)
        .env("DOCKER_LOG", &log)
        .env("DOCKER_PID", &pid)
        .env("DOCKER_IDS", ids)
        .env("DOCKER_PS_STATUS", ps_status)
        .env("DOCKER_STOP_STATUS", stop_status)
        .env("DOCKER_DELAY", delay)
        .output()
        .unwrap();
    if pid.exists() {
        let id = fs::read_to_string(pid).unwrap();
        assert!(
            !Command::new("/bin/kill")
                .args(["-0", &id])
                .output()
                .unwrap()
                .status
                .success(),
            "timed-out Docker child survived"
        );
    }
    (output, fs::read_to_string(log).unwrap_or_default())
}

fn listed(repo: &Repository) -> String {
    format!(
        "[ps]\n[-q]\n[--filter]\n[label=com.docker.compose.project.working_dir={}]\n\n",
        repo.checkout.display()
    )
}

#[test]
fn stop_uses_only_ids_from_the_exact_compose_path_filter() {
    let repo = Repository::new();
    let (output, log) = docker(
        &repo,
        "--docker-cleanup=stop",
        "  aaaaaaaaaaaa\nbbbbbbbbbbbb\n",
        "0",
        "0",
        "",
    );
    assert_result(
        &output,
        0,
        &format!("removed {}\n", repo.checkout.display()),
    );
    assert_eq!(
        log,
        format!(
            "{}[stop]\n[--timeout]\n[10]\n[aaaaaaaaaaaa]\n[bbbbbbbbbbbb]\n\n",
            listed(&repo)
        )
    );
}

#[test]
fn default_off_unknown_flags_and_split_stop_never_invoke_docker() {
    for mode in [
        "",
        "--docker-cleanup=off",
        "--docker-cleanup=invalid",
        "--docker-cleanup stop",
        "--unknown",
    ] {
        let repo = Repository::new();
        let (output, log) = docker(&repo, mode, "aaaaaaaaaaaa", "0", "0", "");
        assert_result(
            &output,
            0,
            &format!("removed {}\n", repo.checkout.display()),
        );
        assert!(log.is_empty());
    }
}

#[test]
fn exact_stop_flag_wins_even_when_off_is_also_present() {
    let repo = Repository::new();
    let (output, log) = docker(
        &repo,
        "--docker-cleanup=stop --docker-cleanup=off",
        "",
        "0",
        "0",
        "",
    );
    assert!(output.status.success());
    assert_eq!(log, listed(&repo));
}

#[test]
fn empty_or_invalid_ids_never_reach_stop() {
    for ids in [
        "",
        " \n",
        "invalid ID",
        "AAAAAAAAAAAA",
        "aaaaaaaaaaa",
        "aaaaaaaaaaaa\n\nbbbbbbbbbbbb",
        "aaaaaaaaaaaa\r\nbbbbbbbbbbbb",
        "aaaaaaaaaaaa\n--help",
        "aaaaaaaaaaaa\nbad",
        &"a".repeat(65),
    ] {
        let repo = Repository::new();
        let (output, log) = docker(&repo, "--docker-cleanup=stop", ids, "0", "0", "");
        assert_result(
            &output,
            0,
            &format!("removed {}\n", repo.checkout.display()),
        );
        assert_eq!(log, listed(&repo), "ids: {ids:?}");
    }
}

#[test]
fn full_length_ids_are_accepted() {
    let repo = Repository::new();
    let id = "a".repeat(64);
    let (output, log) = docker(&repo, "--docker-cleanup=stop", &id, "0", "0", "");
    assert!(output.status.success());
    assert_eq!(
        log,
        format!("{}[stop]\n[--timeout]\n[10]\n[{id}]\n\n", listed(&repo))
    );
}

#[test]
fn missing_unavailable_or_failed_docker_cannot_prevent_removal() {
    for (ps, stop) in [("missing", "0"), ("1", "0"), ("0", "1")] {
        let repo = Repository::new();
        let (output, log) = docker(&repo, "--docker-cleanup=stop", "aaaaaaaaaaaa", ps, stop, "");
        assert_result(
            &output,
            0,
            &format!("removed {}\n", repo.checkout.display()),
        );
        let expected = match ps {
            "missing" => String::new(),
            "1" => listed(&repo),
            _ => format!(
                "{}[stop]\n[--timeout]\n[10]\n[aaaaaaaaaaaa]\n\n",
                listed(&repo)
            ),
        };
        assert_eq!(log, expected);
    }
}

#[test]
fn docker_timeout_kills_and_reaps_the_command_then_removes_checkout() {
    for delay in ["ps", "stop"] {
        let repo = Repository::new();
        let started = std::time::Instant::now();
        let (output, log) = docker(
            &repo,
            "--docker-cleanup=stop",
            "aaaaaaaaaaaa",
            "0",
            "0",
            delay,
        );
        assert_result(
            &output,
            0,
            &format!("removed {}\n", repo.checkout.display()),
        );
        assert!(started.elapsed() < std::time::Duration::from_secs(25));
        let expected = if delay == "ps" {
            listed(&repo)
        } else {
            format!(
                "{}[stop]\n[--timeout]\n[10]\n[aaaaaaaaaaaa]\n\n",
                listed(&repo)
            )
        };
        assert_eq!(log, expected);
    }
}

#[test]
fn docker_still_runs_before_git_refuses_dirty_locked_or_detached_checkouts() {
    for state in ["dirty", "locked", "detached"] {
        let repo = Repository::new();
        match state {
            "dirty" => fs::write(repo.checkout.join("notes.txt"), "keep").unwrap(),
            "locked" => {
                git(
                    &repo.repository,
                    &["worktree", "lock", repo.checkout.to_str().unwrap()],
                );
            }
            _ => {
                git(&repo.checkout, &["checkout", "-q", "--detach"]);
            }
        }
        let (output, log) = docker(&repo, "--docker-cleanup=stop", "aaaaaaaaaaaa", "0", "0", "");
        assert_eq!(
            output.status.code(),
            Some(if state == "detached" { 0 } else { 1 })
        );
        assert_eq!(
            log,
            format!(
                "{}[stop]\n[--timeout]\n[10]\n[aaaaaaaaaaaa]\n\n",
                listed(&repo)
            )
        );
        assert!(repo.checkout.exists());
    }
}

#[test]
fn git_launch_failures_refuse_removal_and_preserve_checkout() {
    use std::os::unix::fs::PermissionsExt;
    for disappears in [false, true] {
        let repo = Repository::new();
        let bin = repo._root.path().join("missing-git-bin");
        fs::create_dir(&bin).unwrap();
        if disappears {
            let executable = bin.join("git");
            fs::write(&executable, "#!/bin/sh\n/bin/rm \"$0\"\nexit 0\n").unwrap();
            fs::set_permissions(executable, fs::Permissions::from_mode(0o755)).unwrap();
        }
        let output = command(&repo.event().to_string(), &[])
            .env("PATH", bin)
            .output()
            .unwrap();
        assert_eq!(output.status.code(), Some(1));
        assert!(String::from_utf8_lossy(&output.stdout).starts_with("failed: "));
        assert!(output.stderr.is_empty());
        assert!(repo.checkout.exists());
        repo.branch_survives();
    }
}

#[test]
fn excessive_array_coercion_returns_a_structured_failure() {
    for depth in [5000, 30000] {
        let input = format!(
            "{{\"data\":{{\"workspace\":{{\"worktree\":{{\"checkout_path\":{{\"length\":{}0{}}},\"repo_root\":\"/none\",\"is_linked_worktree\":false}}}}}}}}",
            "[".repeat(depth),
            "]".repeat(depth)
        );
        assert_result(
            &invoke(&input, &[]),
            1,
            "failed: Maximum call stack size exceeded.\n",
        );
    }
}

#[test]
fn array_length_coercion_has_a_deterministic_boundary() {
    let unrelated = format!(
        "{{\"data\":{{}},\"extra\":{}0{}}}",
        "[".repeat(5000),
        "]".repeat(5000)
    );
    assert_result(
        &invoke(&unrelated, &[]),
        0,
        "skipped: event carries no worktree snapshot\n",
    );

    for depth in [4096, 4097] {
        let input = format!(
            "{{\"data\":{{\"workspace\":{{\"worktree\":{{\"checkout_path\":{{\"length\":{}0{}}},\"repo_root\":\"/none\",\"is_linked_worktree\":false}}}}}}}}",
            "[".repeat(depth),
            "]".repeat(depth)
        );
        let output = invoke(&input, &[]);
        if depth == 4097 {
            assert_result(&output, 1, "failed: Maximum call stack size exceeded.\n");
        } else {
            assert_eq!(output.status.code(), Some(1));
            assert!(
                String::from_utf8_lossy(&output.stdout)
                    .contains("Invalid input: expected string, received object")
            );
            assert!(!String::from_utf8_lossy(&output.stdout).contains("Maximum call stack"));
            assert!(output.stderr.is_empty());
        }
    }
}
