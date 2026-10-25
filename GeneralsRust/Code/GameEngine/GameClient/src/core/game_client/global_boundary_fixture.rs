//! Isolation for tests whose public API deliberately uses application globals.
//! Each child runs one unchanged test body against a private working directory.
#[cfg(not(target_arch = "wasm32"))]
pub(crate) fn run_in_private_child(module: &str, name: &str) -> bool {
    const MARKER: &str = "GENERALS_CLIENT_GLOBAL_BOUNDARY_CHILD";
    let module = module
        .split_once("::")
        .expect("test module has crate prefix")
        .1;
    let exact = format!("{module}::{name}");
    if std::env::var(MARKER).as_deref() == Ok(exact.as_str()) {
        let root = std::env::current_dir()
            .expect("private child cwd")
            .join("UserData");
        std::fs::create_dir_all(&root).expect("private child userdata");
        game_engine::common::ini::ini_game_data::ensure_global_data()
            .write()
            .set_path_user_data(root.to_string_lossy().into_owned());
        return false;
    }
    let directory = tempfile::tempdir().expect("private compatibility fixture directory");
    let mut child = std::process::Command::new(std::env::current_exe().expect("test executable"))
        .args(["--exact", &exact, "--test-threads=1", "--nocapture"])
        .env(MARKER, &exact)
        .current_dir(directory.path())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .expect("isolated compatibility test child");
    // Each reader owns its pipe; output cannot fill a pipe and look like a
    // global-boundary deadlock. No shared mutable reader state is introduced.
    let readers = [
        child
            .stdout
            .take()
            .map(|pipe| std::thread::spawn(move || read_output(pipe))),
        child
            .stderr
            .take()
            .map(|pipe| std::thread::spawn(move || read_output(pipe))),
    ];
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(25);
    let mut timed_out = false;
    let status = loop {
        if let Some(status) = child.try_wait().expect("compatibility child status") {
            break status;
        }
        if std::time::Instant::now() >= deadline {
            timed_out = true;
            // Reap even when the child exits between the status check and kill.
            let _ = child.kill();
            break child.wait().expect("reap compatibility child");
        }
        std::thread::sleep(std::time::Duration::from_millis(25));
    };
    let [stdout, stderr] = readers.map(|reader| {
        reader
            .expect("piped child output")
            .join()
            .expect("child output reader panicked")
            .expect("read child output")
    });
    let stdout = String::from_utf8_lossy(&stdout);
    let stderr = String::from_utf8_lossy(&stderr);
    assert!(
        !timed_out,
        "compatibility test exceeded 25 seconds: {exact}\n{stdout}\n{stderr}"
    );
    assert!(
        stdout.contains("running 1 test"),
        "exact child must execute one test: {stdout}\n{stderr}"
    );
    assert!(
        status.success() && stdout.contains("test result: ok. 1 passed; 0 failed; 0 ignored;"),
        "isolated compatibility test failed: {exact}\n{stdout}\n{stderr}"
    );
    true
}

#[cfg(target_arch = "wasm32")]
pub(super) fn run_in_private_child(_module: &str, _name: &str) -> bool {
    false
}

#[cfg(not(target_arch = "wasm32"))]
fn read_output(mut pipe: impl std::io::Read) -> std::io::Result<Vec<u8>> {
    let mut output = Vec::new();
    pipe.read_to_end(&mut output)?;
    Ok(output)
}
