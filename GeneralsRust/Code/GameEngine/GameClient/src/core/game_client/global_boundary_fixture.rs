//! Isolation for tests whose public API deliberately uses application globals.
//! Each child runs one unchanged test body in a fresh process. Keep the parent's
//! cwd so retail `Data/` / archive probes still resolve; only UserData is private.
#[cfg(not(target_arch = "wasm32"))]
use std::path::{Path, PathBuf};
#[cfg(not(target_arch = "wasm32"))]
fn ensure_retail_assets_env() {
    // Prefer a root that actually has loose ParticleSystem.ini (extracted BIG).
    // A pre-set GENERALS_ASSETS_DIR pointing at a retail install without INI is
    // not sufficient for particle/FX admission tests.
    let particle_rel = Path::new("Data/INI/ParticleSystem.ini");
    if let Ok(root) = std::env::var("GENERALS_ASSETS_DIR") {
        if Path::new(&root).join(particle_rel).is_file() {
            return;
        }
    }
    let candidates = [
        PathBuf::from("windows_game/extracted_big_files_v2/INIZH"),
        PathBuf::from("../windows_game/extracted_big_files_v2/INIZH"),
        PathBuf::from("../../windows_game/extracted_big_files_v2/INIZH"),
        PathBuf::from("../../../windows_game/extracted_big_files_v2/INIZH"),
        PathBuf::from("../../../../windows_game/extracted_big_files_v2/INIZH"),
        PathBuf::from(
            "/Users/bernardoferrari/Downloads/CnC_Generals_Zero_Hour-main/windows_game/extracted_big_files_v2/INIZH",
        ),
        PathBuf::from("windows_game/Command & Conquer Generals Zero Hour"),
        PathBuf::from("../windows_game/Command & Conquer Generals Zero Hour"),
        PathBuf::from("../../windows_game/Command & Conquer Generals Zero Hour"),
        PathBuf::from("../../../../windows_game/Command & Conquer Generals Zero Hour"),
        PathBuf::from(
            "/Users/bernardoferrari/Downloads/CnC_Generals_Zero_Hour-main/windows_game/Command & Conquer Generals Zero Hour",
        ),
    ];
    for path in &candidates {
        if path.join(particle_rel).is_file() || path.is_dir() {
            // Prefer the first candidate that carries ParticleSystem.ini; else
            // keep searching for a dir that does before falling back.
            if !path.join(particle_rel).is_file() {
                continue;
            }
            // SAFETY: test-only env seed before child spawn / ambient host.
            unsafe {
                std::env::set_var("GENERALS_ASSETS_DIR", &path);
                std::env::set_var("GENERALS_RETAIL_DIR", &path);
            }
            return;
        }
    }
    // Last resort: any existing retail/install dir for non-particle probes.
    for path in &candidates {
        if path.is_dir() {
            // SAFETY: test-only env seed before child spawn / ambient host.
            unsafe {
                std::env::set_var("GENERALS_ASSETS_DIR", &path);
                std::env::set_var("GENERALS_RETAIL_DIR", &path);
            }
            return;
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn retail_particle_ini_available() -> bool {
    ensure_retail_assets_env();
    let relative = Path::new("Data/INI/ParticleSystem.ini");
    if relative.is_file() {
        return true;
    }
    if let Ok(root) = std::env::var("GENERALS_ASSETS_DIR") {
        if Path::new(&root).join("Data/INI/ParticleSystem.ini").is_file() {
            return true;
        }
    }
    // Extracted BIG layout used by local checkouts. Cargo --lib cwd is the
    // GameClient package dir, so walk up to GeneralsRust/windows_game as well.
    for path in [
        Path::new("windows_game/extracted_big_files_v2/INIZH/Data/INI/ParticleSystem.ini"),
        Path::new("../windows_game/extracted_big_files_v2/INIZH/Data/INI/ParticleSystem.ini"),
        Path::new("../../windows_game/extracted_big_files_v2/INIZH/Data/INI/ParticleSystem.ini"),
        Path::new("../../../windows_game/extracted_big_files_v2/INIZH/Data/INI/ParticleSystem.ini"),
        Path::new("../../../../windows_game/extracted_big_files_v2/INIZH/Data/INI/ParticleSystem.ini"),
        Path::new("/Users/bernardoferrari/Downloads/CnC_Generals_Zero_Hour-main/windows_game/extracted_big_files_v2/INIZH/Data/INI/ParticleSystem.ini"),
    ] {
        if path.is_file() {
            return true;
        }
    }
    if let Ok(root) = std::env::var("GENERALS_ASSETS_DIR") {
        let root = Path::new(&root);
        for candidate in [
            root.join("../extracted_big_files_v2/INIZH/Data/INI/ParticleSystem.ini"),
            root.join("../../extracted_big_files_v2/INIZH/Data/INI/ParticleSystem.ini"),
        ] {
            if candidate.is_file() {
                return true;
            }
        }
    }
    false
}

#[cfg(not(target_arch = "wasm32"))]
pub(crate) fn run_in_private_child(module: &str, name: &str) -> bool {
    const MARKER: &str = "GENERALS_CLIENT_GLOBAL_BOUNDARY_CHILD";
    let module = module
        .split_once("::")
        .expect("test module has crate prefix")
        .1;
    let exact = format!("{module}::{name}");
    if std::env::var(MARKER).as_deref() == Ok(exact.as_str()) {
        ensure_retail_assets_env();
        crate::test_support::ensure_ambient_host();
        let root = std::env::temp_dir().join(format!(
            "generals_client_userdata_{}_{}",
            std::process::id(),
            exact.replace(":", "_")
        ));
        std::fs::create_dir_all(&root).expect("private child userdata");
        game_engine::common::ini::ini_game_data::ensure_global_data()
            .write()
            .set_path_user_data(root.to_string_lossy().into_owned());
        return false;
    }
    if !retail_particle_ini_available() {
        eprintln!(
            "skipping isolated compatibility test (retail ParticleSystem.ini absent): {exact}"
        );
        return true;
    }
    ensure_retail_assets_env();
    let parent_cwd = std::env::current_dir().expect("parent cwd");
    let mut child_cmd = std::process::Command::new(std::env::current_exe().expect("test executable"));
    child_cmd
        .args(["--exact", &exact, "--test-threads=1", "--nocapture"])
        .env(MARKER, &exact)
        .current_dir(&parent_cwd)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());
    if let Ok(assets) = std::env::var("GENERALS_ASSETS_DIR") {
        child_cmd.env("GENERALS_ASSETS_DIR", &assets);
        child_cmd.env("GENERALS_RETAIL_DIR", &assets);
    }
    let mut child = child_cmd
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
