//! Fails CI if a sealed module regains mutable process globals or stub markers.
#[test]
fn sealed_modules_stay_clean() {
    let manifest_dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    // GameClient/ -> GameEngine/ -> Code/ -> GeneralsRust/
    let generals_rust = manifest_dir
        .ancestors()
        .nth(3)
        .expect("GameClient is under GeneralsRust/Code/GameEngine");
    let script = generals_rust.join("scripts/check_sealed_modules.sh");
    let list = generals_rust.join("sealed_modules.toml");
    assert!(script.is_file(), "missing {}", script.display());
    assert!(list.is_file(), "missing {}", list.display());
    let status = std::process::Command::new(&script)
        .current_dir(generals_rust)
        .status()
        .expect("run check_sealed_modules.sh");
    assert!(
        status.success(),
        "sealed module gate failed; see scripts/check_sealed_modules.sh output"
    );
}
