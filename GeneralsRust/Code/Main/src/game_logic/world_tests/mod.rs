//! Host GameLogic unit tests (child of `game_logic.rs` via `#[path]`).
#![allow(unused_imports, non_snake_case, unused_variables, dead_code)]
use super::*;

mod helpers;
use helpers::*;

mod base_defenses;
mod combat_particles_and_economy;
mod crates_and_salvage;
mod network_and_scripts;
mod ocl_and_bombs;
mod parachute_and_rebuild;
mod phase3_produce;
mod pilots_and_movement;
mod production_and_mobs;
mod projectiles_air;
mod scatter_and_chain;
mod science_and_upgrades;
mod shells_and_missiles;
mod strategy_and_stealth;
mod superweapons_and_plans;
mod unit_residuals;
mod vehicles_and_lasers;

#[test]
fn process_destroy_list_runs_inside_each_logic_frame() {
    // C++ GameLogic.cpp:3762 processDestroyList is inside every update(),
    // not once after the fixed-step catch-up batch (hq-x4im).
    let mut logic = GameLogic::new();
    ensure_test_infantry_template(&mut logic);
    ensure_test_player_for_team(&mut logic, Team::USA);
    let id = logic
        .create_object("TestInfantry", Team::USA, glam::Vec3::ZERO)
        .expect("unit");
    logic.mark_object_for_destruction(id, None);
    assert!(logic.get_object(id).is_some(), "queued, not yet removed");
    logic.update_simulation(1.0 / 30.0);
    assert!(
        logic.get_object(id).is_none(),
        "processDestroyList must run inside update_simulation / each fixed step"
    );
}

#[test]
fn victory_conditions_update_inside_each_logic_frame() {
    // C++ GameLogic.cpp:3769 TheVictoryConditions->UPDATE() is inside
    // GameLogic::update, not only PresentationFrame::build (hq-en3j).
    use crate::game_logic::VictoryCondition;
    let mut logic = GameLogic::new();
    ensure_test_infantry_template(&mut logic);
    ensure_test_player_for_team(&mut logic, Team::USA);
    ensure_test_player_for_team(&mut logic, Team::GLA);
    let _usa = logic
        .create_object("TestInfantry", Team::USA, glam::Vec3::new(0.0, 0.0, 0.0))
        .expect("usa");
    let gla = logic
        .create_object("TestInfantry", Team::GLA, glam::Vec3::new(20.0, 0.0, 0.0))
        .expect("gla");
    logic.mark_object_for_destruction(gla, None);
    logic.update_simulation(1.0 / 30.0);
    assert!(
        logic.get_object(gla).is_none(),
        "loser unit must be gone before victory UPDATE"
    );
    let outcome = logic.evaluate_victory_condition();
    assert!(
        matches!(outcome, Some(VictoryCondition::Winner(_))),
        "victory must be decided on the logic frame that processed destroy, got {outcome:?}"
    );
}
