//! Test-only isolation for intentionally ambient Recorder compatibility calls.
//! Production and explicit-owner Recorder tests do not use this helper.

pub(super) fn run(module: &str, name: &str, body: impl FnOnce()) {
    #[cfg(target_arch = "wasm32")]
    {
        let _ = (module, name);
        body();
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        const CHILD: &str = "GENERALS_LEGACY_RECORDER_FIXTURE_CHILD";
        let module = module
            .strip_prefix(concat!(env!("CARGO_CRATE_NAME"), "::"))
            .expect("fixture module starts with the current crate name");
        let exact = format!("{module}::{name}");
        if std::env::var(CHILD).ok().as_deref() == Some(exact.as_str()) {
            // Even constructor-only fixtures read legacy map/seed defaults.
            // Never let a future compatibility file operation reach user data.
            let temporary = tempfile::tempdir().expect("private fixture user data");
            {
                let global = crate::common::ini::ini_game_data::ensure_global_data();
                let mut data = global.write();
                data.set_path_user_data(temporary.path().to_string_lossy().into_owned());
                data.base_stats_dir = temporary
                    .path()
                    .join("Stats")
                    .to_string_lossy()
                    .into_owned();
            }
            body();
            return;
        }
        // Files avoid pipe backpressure while retaining diagnostics for bounded
        // child execution, including a regression that deadlocks a global lock.
        let stdout = tempfile::NamedTempFile::new().expect("child stdout");
        let stderr = tempfile::NamedTempFile::new().expect("child stderr");
        let mut child =
            std::process::Command::new(std::env::current_exe().expect("current test executable"))
                .arg("--exact")
                .arg(&exact)
                .arg("--nocapture")
                .arg("--test-threads=1")
                .env(CHILD, &exact)
                .stdout(std::process::Stdio::from(stdout.reopen().unwrap()))
                .stderr(std::process::Stdio::from(stderr.reopen().unwrap()))
                .spawn()
                .expect("spawn exact Recorder fixture");
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(30);
        let mut timed_out = false;
        let status = loop {
            if let Some(status) = child.try_wait().expect("poll Recorder fixture") {
                break status;
            }
            if std::time::Instant::now() >= deadline {
                timed_out = true;
                let _ = child.kill();
                break child.wait().expect("reap timed-out Recorder fixture");
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        };
        let stdout = std::fs::read_to_string(stdout.path()).expect("read child stdout");
        let stderr = std::fs::read_to_string(stderr.path()).expect("read child stderr");
        assert!(
            !timed_out && status.success(),
            "exact Recorder fixture {exact} failed: {status}; timeout={timed_out}\n{stdout}\n{stderr}"
        );
        assert!(
            stdout.contains("running 1 test\n")
                && stdout.contains(&format!("test {exact} ... ok"))
                && stdout.contains("test result: ok. 1 passed; 0 failed;"),
            "exact Recorder fixture must execute one passing test, not zero matches: {exact}\n{stdout}\n{stderr}"
        );
    }
}
