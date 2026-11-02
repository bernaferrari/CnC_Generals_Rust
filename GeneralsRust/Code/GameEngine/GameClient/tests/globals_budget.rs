//! Fails CI if GameClient (or peer crates tracked by the counter) gain mutable process statics.
#[test]
fn mutable_process_statics_do_not_rise() {
    let manifest_dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    // GameClient/ -> GameEngine/ -> Code/ -> GeneralsRust/
    let generals_rust = manifest_dir
        .ancestors()
        .nth(3)
        .expect("GameClient is under GeneralsRust/Code/GameEngine");
    let script = generals_rust.join("scripts/count_globals.sh");
    let baseline = generals_rust.join("scripts/globals_baseline.json");
    assert!(script.is_file(), "missing {}", script.display());
    assert!(baseline.is_file(), "missing {}", baseline.display());
    let status = std::process::Command::new(&script)
        .arg("--compare")
        .arg(&baseline)
        .current_dir(generals_rust)
        .status()
        .expect("run count_globals.sh");
    assert!(
        status.success(),
        "mutable process-wide static count rose; see scripts/count_globals.sh output"
    );
}
