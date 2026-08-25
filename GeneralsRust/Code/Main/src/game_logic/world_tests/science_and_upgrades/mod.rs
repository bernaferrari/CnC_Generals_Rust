//! Host GameLogic tests — `science_and_upgrades`.
#![allow(unused_imports, non_snake_case, unused_variables, dead_code)]
use super::super::*;
use super::helpers::*;

fn wave21_guard_weapon() -> Weapon {
    Weapon {
        range: 400.0,
        can_target_ground: true,
        can_target_air: true,
        ..Default::default()
    }
}

fn mark_guard_scan_due(logic: &mut GameLogic, id: crate::game_logic::ObjectId) {
    logic.guard_next_enemy_scan.insert(id, logic.frame);
}

// Behavior-named suites keep each test file below the 4k LOC ceiling.
mod guards_and_retaliation;
mod player_events_and_crates;
mod upgrades_power_capture;
