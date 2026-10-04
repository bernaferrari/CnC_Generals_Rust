//! Actual engine regression; event-loop construction must run on the main thread.
//! cargo run --locked -p generals_main --features internal --bin snow_anim2d_owner_probe
#[cfg(all(feature = "internal", not(target_arch = "wasm32")))]
#[tokio::main]
async fn main() -> anyhow::Result<()> {
    generals_main::cnc_game_engine::run_snow_anim2d_owner_probe()
}

#[cfg(not(all(feature = "internal", not(target_arch = "wasm32"))))]
fn main() {
    eprintln!("snow_anim2d_owner_probe requires native --features internal");
    std::process::exit(2);
}
