//! Child-process isolation for tests that intentionally exercise Common globals.

/// Run one global-state test in its own test-harness process.
///
/// Pass the calling test module path (usually `module_path!()`), the exact test
/// leaf, and a caller-specific environment marker. In the child, the marker
/// makes the test body proceed; in the parent, this waits for exactly that one
/// test and returns `true` so the parent test body can return without touching
/// shared globals.
#[cfg(not(target_arch = "wasm32"))]
pub(crate) fn run(module_path: &str, leaf: &str, child_marker: &str, fixture_name: &str) -> bool {
    let module = module_path.split_once("::").unwrap().1;
    let exact = format!("{module}::{leaf}");
    if std::env::var_os(child_marker).as_deref() == Some(std::ffi::OsStr::new(&exact)) {
        return false;
    }
    let mut child = std::process::Command::new(std::env::current_exe().unwrap())
        .arg("--exact")
        .arg(&exact)
        .arg("--nocapture")
        .env(child_marker, &exact)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .unwrap();
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(25);
    loop {
        if child.try_wait().unwrap().is_some() {
            let output = child.wait_with_output().unwrap();
            assert!(
                output.status.success(),
                "actual {fixture_name} fixture child {exact} failed: {}\n{}\n{}",
                output.status,
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            );
            let stdout = String::from_utf8_lossy(&output.stdout);
            assert!(
                stdout.contains("running 1 test")
                    && stdout.contains("test result: ok. 1 passed; 0 failed"),
                "actual {fixture_name} fixture child {exact} must execute exactly one test: {stdout}"
            );
            return true;
        }
        if std::time::Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            panic!("actual {fixture_name} fixture child {exact} exceeded 25 seconds");
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
}

#[cfg(target_arch = "wasm32")]
pub(crate) fn run(
    _module_path: &str,
    _leaf: &str,
    _child_marker: &str,
    _fixture_name: &str,
) -> bool {
    false
}
