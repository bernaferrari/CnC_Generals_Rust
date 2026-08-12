//! Focused developer regression for GameWorld shadow scheduling during replay fast-forward.
//!
//! Run with:
//! `cargo run -p generals_main --features internal --bin replay_fast_forward_probe`

#[cfg(feature = "internal")]
#[tokio::main]
async fn main() -> anyhow::Result<()> {
    generals_main::cnc_game_engine::run_replay_fast_forward_engine_probe()
}

#[cfg(not(feature = "internal"))]
fn main() {
    eprintln!("replay_fast_forward_probe requires `--features internal`");
    std::process::exit(2);
}
