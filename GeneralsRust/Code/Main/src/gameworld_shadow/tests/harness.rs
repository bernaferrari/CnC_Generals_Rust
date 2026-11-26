//! Helpers that exercise the installed Main body directly.

/// Run a libtest case that requires process isolation in a fresh process, rather than
/// mutating the environment shared by the test runner and native libraries.
/// Returns true in the parent after the exact child test has passed.
pub(crate) fn isolated_shadow_case(module: &str, test: &str) -> bool {
    const MARKER: &str = "GENERALS_ISOLATED_SHADOW_TEST";
    let module = module.split_once("::").expect("test module path").1;
    let name = format!("{module}::{test}");
    if std::env::var(MARKER).as_deref() == Ok(name.as_str()) {
        return false;
    }

    let output = std::process::Command::new(std::env::current_exe().expect("test executable"))
        .args(["--exact", &name, "--nocapture", "--test-threads=1"])
        .env(MARKER, &name)
        .output()
        .expect("launch isolated shadow test");
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success()
            && stdout.contains("running 1 test")
            && stdout.contains("1 passed; 0 failed"),
        "isolated test {name} failed or did not run: {}\n{stdout}\n{stderr}",
        output.status
    );
    true
}
