//! Run with `cargo run -p generals_main --features internal --bin native_rank_science_probe`.
#[cfg(all(feature = "internal", feature = "game_client", not(target_arch = "wasm32")))]
fn main() -> anyhow::Result<()> {
    const CHILD: &str = "GENERALS_RANK_PROBE_CHILD";
    const ROOT: &str = "GENERALS_RANK_PROBE_PRIVATE_ROOT";
    const SOURCE: &str = "GENERALS_RANK_PROBE_SOURCE_ROOT";
    if std::env::var_os(CHILD).is_some() {
        let root = std::path::PathBuf::from(std::env::var_os(ROOT).ok_or_else(|| anyhow::anyhow!("private root required"))?);
        let source = std::path::PathBuf::from(std::env::var_os(SOURCE).ok_or_else(|| anyhow::anyhow!("source root required"))?);
        // Path admission writes Common GameData before the Engine is constructed.
        // Keep that real bootstrap owner alive through the child run.
        let _common = game_engine::common::common_services::CommonServicesScope::new();
        generals_main::cnc_game_engine::prepare_replay_probe_private_paths(&root, &source)?;
        return run_child();
    }
    let source = std::env::current_dir()?.canonicalize()?;
    let nonce = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH)?.as_nanos();
    let root = (0..1024).find_map(|attempt| {
        let path = std::env::temp_dir().join(format!("generals-native-rank-probe-{}-{nonce}-{attempt}", std::process::id()));
        match std::fs::create_dir(&path) {
            Ok(()) => Some(Ok(path)),
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => None,
            Err(error) => Some(Err(error)),
        }
    }).ok_or_else(|| anyhow::anyhow!("unable to reserve private root"))??.canonicalize()?;
    std::fs::create_dir(root.join("cwd"))?;
    std::fs::create_dir(root.join("userdata"))?;
    let mut child = std::process::Command::new(std::env::current_exe()?);
    child.current_dir(root.join("cwd")).env(CHILD, "1").env(ROOT, &root).env(SOURCE, &source);
    if std::env::var_os("GENERALS_ASSETS_DIR").is_none() {
        child.env("GENERALS_ASSETS_DIR", &source);
    }
    eprintln!("native rank probe private output: {}", root.display());
    let status = child.status()?;
    anyhow::ensure!(status.success(), "native rank probe child failed: {status}");
    Ok(())
}

#[cfg(all(feature = "internal", feature = "game_client", not(target_arch = "wasm32")))]
#[tokio::main]
async fn run_child() -> anyhow::Result<()> {
    generals_main::cnc_game_engine::run_native_rank_science_probe()
}

#[cfg(not(all(feature = "internal", feature = "game_client", not(target_arch = "wasm32"))))]
fn main() {
    eprintln!("native_rank_science_probe requires native internal and game_client features");
    std::process::exit(2);
}
