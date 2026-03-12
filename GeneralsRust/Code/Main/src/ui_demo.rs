#[cfg(not(feature = "dev-tools"))]
fn main() {
    eprintln!("Enable the 'dev-tools' feature to build and run ui_demo.");
}

#[cfg(feature = "dev-tools")]
fn main() -> anyhow::Result<()> {
    env_logger::init();
    println!("ui_demo smoke test: UI runtime is integrated via the main 'generals' binary.");
    Ok(())
}
