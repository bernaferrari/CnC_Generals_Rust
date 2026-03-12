#[cfg(not(feature = "dev-tools"))]
fn main() {
    eprintln!("Enable the 'dev-tools' feature to build and run mouse_selection_test.");
}

#[cfg(feature = "dev-tools")]
use generals_main::game_logic::{GameLogic, GameMode, Team};
#[cfg(feature = "dev-tools")]
use generals_main::{RtsInputSystem, UnitInputHandler};
#[cfg(feature = "dev-tools")]
use glam::Vec3;
#[cfg(feature = "dev-tools")]
use std::sync::{Arc, Mutex};

#[cfg(feature = "dev-tools")]
fn main() -> anyhow::Result<()> {
    env_logger::init();

    let mut logic = GameLogic::new();
    logic.start_new_game(GameMode::Skirmish);
    let _ = logic.load_map("demo_map");
    let _ = logic.create_object("USA_Ranger", Team::USA, Vec3::new(-10.0, 0.0, -10.0));
    let _ = logic.create_object("USA_Ranger", Team::USA, Vec3::new(-8.0, 0.0, -12.0));

    let game_logic = Arc::new(Mutex::new(logic));
    let mut input = RtsInputSystem::new();
    let mut handler = UnitInputHandler::new((1280.0, 720.0), Team::USA, 0);

    let rt = tokio::runtime::Runtime::new()?;
    rt.block_on(handler.process_input(&mut input, &game_logic));

    println!(
        "mouse_selection_test initialized successfully (selected={} hovered={:?})",
        handler.get_selected_objects().len(),
        handler.get_hovered_object()
    );

    Ok(())
}
