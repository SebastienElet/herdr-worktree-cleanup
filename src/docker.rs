use std::{
    fs::File,
    io::{Read, Seek, SeekFrom},
    process::{Command, Stdio},
    thread,
    time::{Duration, Instant},
};

pub fn cleanup(checkout_path: &str) {
    let Some(listed) = run(&[
        "ps",
        "-q",
        "--filter",
        &format!("label=com.docker.compose.project.working_dir={checkout_path}"),
    ]) else {
        return;
    };
    let listed = crate::json::trim(&listed);
    if listed.is_empty() {
        return;
    }
    let ids: Vec<&str> = listed.split('\n').collect();
    if !ids.iter().all(|id| {
        (12..=64).contains(&id.len())
            && id
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    }) {
        return;
    }
    let mut args = vec!["stop", "--timeout", "10"];
    args.extend(ids);
    run(&args);
}

fn run(args: &[&str]) -> Option<String> {
    let mut output: File = tempfile::tempfile().ok()?;
    let mut child = Command::new("docker")
        .args(args)
        .stdin(Stdio::null())
        .stdout(output.try_clone().ok()?)
        .stderr(Stdio::null())
        .spawn()
        .ok()?;
    let deadline = Instant::now() + Duration::from_secs(15);
    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                if !status.success() {
                    return None;
                }
                output.seek(SeekFrom::Start(0)).ok()?;
                let mut bytes = Vec::new();
                output.read_to_end(&mut bytes).ok()?;
                return Some(String::from_utf8_lossy(&bytes).into_owned());
            }
            Ok(None) if Instant::now() < deadline => thread::sleep(Duration::from_millis(5)),
            _ => {
                let _ = child.kill();
                let _ = child.wait();
                return None;
            }
        }
    }
}
