#![cfg(unix)]

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::process::{Command, Stdio};
use std::time::Duration;

// Spawned only by the isolation test. Its command line deliberately matches
// the old cross-worktree pkill pattern, without launching another game.
#[test]
#[ignore]
fn unrelated_runtime_host() {
    std::thread::sleep(Duration::from_secs(30));
}

#[test]
#[ignore]
fn concurrent_smoke_worker() {
    let results = std::thread::scope(|scope| {
        let launch =
            || generals_main::executable_smoke::run_executable_smoke(Duration::from_secs(1), false);
        let first = scope.spawn(launch);
        let second = scope.spawn(launch);
        [first.join().unwrap(), second.join().unwrap()]
    });
    assert!(results.iter().all(|result| result.process_started));
}

#[test]
fn smoke_owns_only_its_child_and_preserves_an_unrelated_runtime() {
    let dir = tempfile::tempdir().unwrap();
    let fake = dir.path().join("generals");
    let pid_log = dir.path().join("smoke-child-pids");
    let control_log = dir.path().join("smoke-control-paths");
    fs::write(
        &fake,
        b"#!/bin/sh\nprintf '%s\\n' $$ >> \"$SMOKE_CHILD_PID_LOG\"\nfor arg in \"$@\"; do case \"$arg\" in -gpui_control=*) printf '%s\\n' \"$arg\" >> \"$SMOKE_CONTROL_LOG\";; esac; done\nexec sleep 30\n",
    )
    .unwrap();
    fs::set_permissions(&fake, fs::Permissions::from_mode(0o755)).unwrap();

    let mut unrelated = Command::new(std::env::current_exe().unwrap())
        .args([
            "--exact",
            "unrelated_runtime_host",
            "--ignored",
            "--skip=generals-runtime_host=headless",
        ])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    // Give the worker its own environment, so simultaneous tests do not mutate
    // the parent process's environment while other threads are running.
    let worker = Command::new(std::env::current_exe().unwrap())
        .args(["--exact", "concurrent_smoke_worker", "--ignored"])
        .env("GENERALS_RUNTIME_EXE", &fake)
        .env("SMOKE_CHILD_PID_LOG", &pid_log)
        .env("SMOKE_CONTROL_LOG", &control_log)
        .output();
    let survived = unrelated.try_wait().unwrap().is_none();
    let _ = unrelated.kill();
    let _ = unrelated.wait();

    let worker = worker.unwrap();
    assert!(
        worker.status.success(),
        "worker failed: {}",
        String::from_utf8_lossy(&worker.stdout)
    );
    assert!(survived, "smoke cleanup killed an unrelated runtime");
    let pids = fs::read_to_string(pid_log).unwrap();
    assert!(!pids.is_empty());
    let controls = fs::read_to_string(control_log).unwrap();
    let paths: Vec<_> = controls.lines().collect();
    assert!(paths.len() >= 2, "both smoke runs must start a child");
    assert_eq!(paths.len(), pids.lines().count());
    assert_eq!(
        paths.iter().collect::<std::collections::HashSet<_>>().len(),
        paths.len()
    );
    for pid in pids.lines() {
        let pid: libc::pid_t = pid.parse().unwrap();
        // Signal 0 checks for the process without sending a signal.
        assert_ne!(
            unsafe { libc::kill(pid, 0) },
            0,
            "smoke child still alive: {pid}"
        );
    }
}
