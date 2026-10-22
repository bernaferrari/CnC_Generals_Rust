//! Main-thread native failure-handler acceptance; requires licensed content.
#[cfg(all(
    feature = "internal",
    feature = "game_client",
    not(target_arch = "wasm32")
))]
#[tokio::main]
async fn main() -> anyhow::Result<()> {
    generals_main::cnc_game_engine::run_terminal_load_owner_probe()
}
#[cfg(not(all(
    feature = "internal",
    feature = "game_client",
    not(target_arch = "wasm32")
)))]
fn main() {
    eprintln!("terminal_load_owner_probe requires native internal+game_client");
    std::process::exit(2);
}
