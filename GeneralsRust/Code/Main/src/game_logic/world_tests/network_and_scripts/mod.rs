//! Host GameLogic tests — `network_and_scripts`.
#![allow(unused_imports, non_snake_case, unused_variables, dead_code)]
use super::super::*;
use super::helpers::*;

fn drain_script_act_b_queues() {
    let _ = gamelogic::scripting::take_host_script_named_attitude_requests();
    let _ = gamelogic::scripting::take_host_script_stopping_distance_requests();
    let _ = gamelogic::scripting::take_host_script_force_select_requests();
    let _ = gamelogic::scripting::take_host_script_player_misc_requests();
    let _ = gamelogic::scripting::take_host_script_garrison_enter_requests();
}

// Behavior-named suites keep each test file below the 4k LOC ceiling.
mod host_queries_and_visibility;
mod script_repair_and_locomotion;
mod scripts_and_capture;
