//! AIPlayer / AISkirmish C++ phase-order residual tests.

pub use super::*;

#[test]
fn aiplayer_update_with_frame_cpp_phase_order() {
    // C++ AIPlayer::update (AIPlayer.cpp): doBaseBuilding → checkReadyTeams →
    // checkQueuedTeams → doTeamBuilding → doUpgradesAndSkills → updateBridgeRepair.
    let src = gamelogic::ai::ai_player::AI_PLAYER_SRC;
    let i = src
        .find("pub fn update_with_frame")
        .expect("update_with_frame");
    let window = &src[i..i.saturating_add(2200).min(src.len())];
    let base = window
        .find("self.do_base_building()?")
        .expect("do_base_building");
    let ready = window
        .find("self.check_ready_teams()?")
        .expect("check_ready");
    let queued = window
        .find("self.check_queued_teams()?")
        .expect("check_queued");
    let team = window.find("self.do_team_building()?").expect("do_team");
    let upg = window
        .find("self.do_upgrades_and_skills()?")
        .expect("upgrades");
    let bridge = window.find("self.update_bridge_repair()?").expect("bridge");
    assert!(
        base < ready && ready < queued && queued < team && team < upg && upg < bridge,
        "update_with_frame must match C++ AIPlayer::update phase order (got base={base} ready={ready} queued={queued} team={team} upg={upg} bridge={bridge})"
    );
    // Attack residual must not interrupt the C++ phase block.
    if let Some(atk) = window.find("self.process_attack_decisions") {
        assert!(
            atk > bridge,
            "process_attack_decisions is host residual and must run after C++ phase block"
        );
    }
}

#[test]
fn aiplayer_check_ready_queued_cpp_parity_surface() {
    // C++ AIPlayer.cpp checkReadyTeams / checkQueuedTeams surface in Rust.
    let src = gamelogic::ai::ai_player::AI_PLAYER_SRC;
    assert!(
        src.contains("AIPlayer::checkReadyTeams")
            && src.contains("60 * LOGICFRAMES_PER_SECOND")
            && src.contains("set_active()")
            && src.contains("AIPlayer::checkQueuedTeams")
            && src.contains("is_build_time_expired")
            && src.contains("push_front(team)")
            && src.contains("is_minimum_built")
            && src.contains("are_builds_complete")
            && src.contains("clear_team_flags")
            && src.contains("join_team_reinforcement"),
        "check_ready/queued must port C++ activation, expiry, prepend, joinTeam residual"
    );
    // Must NOT only move build→ready inside check_ready_teams (old wrong port).
    let i = src.find("fn check_ready_teams").expect("check_ready_teams");
    let end = (i + 3500).min(src.len());
    let window = &src[i..end];
    assert!(
        !window.contains("team_build_queue.remove") || window.contains("team_ready_queue.remove"),
        "check_ready_teams must drain ready queue (C++), not only promote build queue"
    );
    assert!(
        window.contains("team_ready_queue.remove") || window.contains("team_ready_queue"),
        "check_ready_teams must iterate ready queue"
    );
}

#[test]
fn aiplayer_do_base_team_delay_cpp() {
    // C++ AIPlayer.cpp doBaseBuilding / doTeamBuilding delay rechecks.
    let src = gamelogic::ai::ai_player::AI_PLAYER_SRC;
    assert!(
        src.contains("BUILD_DELAY_RECHECK_FRAMES: u32 = 2 * LOGICFRAMES_PER_SECOND")
            && src.contains("TEAM_DELAY_RECHECK_FRAMES: u32 = 5 * LOGICFRAMES_PER_SECOND")
            && src.contains("self.build_delay = BUILD_DELAY_RECHECK_FRAMES")
            && src.contains("self.team_delay = TEAM_DELAY_RECHECK_FRAMES")
            && src.contains("let _ = self.queue_units()")
            && src.contains("C++ `AIPlayer::doBaseBuilding`")
            && src.contains("C++ `AIPlayer::doTeamBuilding`"),
        "do_base/do_team must port C++ 2s/5s delay + queueUnits cadence"
    );
}

#[test]
fn aiplayer_process_base_building_cpp() {
    let src = gamelogic::ai::ai_player::AI_PLAYER_SRC;
    assert!(
        src.contains("C++ `AIPlayer::processBaseBuilding`")
            && src.contains("arm_structure_timer_after_build")
            && src.contains("rebuild_delay_frames")
            && src.contains("structures_poor_mod")
            && src.contains("structures_wealthy_mod"),
        "process_base_building must arm structureTimer with C++ wealth mods"
    );
}

#[test]
fn aiplayer_select_team_to_build_cpp() {
    let src = gamelogic::ai::ai_player::AI_PLAYER_SRC;
    assert!(
        src.contains("C++ `AIPlayer::selectTeamToBuild`")
            && src.contains("select_team_to_reinforce(hi_pri)")
            && src.contains("game_logic_random_value")
            && src.contains("build_specific_ai_team(team_name, false)")
            && src.contains("arm_team_timer_after_build")
            && src.contains("is_possible_to_build_team(team_name, true)"),
        "selectTeamToBuild must hiPri-filter, reinforce first, random pick, arm teamTimer"
    );
}

#[test]
fn aiplayer_select_team_to_reinforce_cpp() {
    let src = gamelogic::ai::ai_player::AI_PLAYER_SRC;
    assert!(
        src.contains("C++ `AIPlayer::selectTeamToReinforce`")
            && src.contains("automatically_reinforce")
            && src.contains("try_to_recruit")
            && src.contains("self.team_delay = 0"),
        "selectTeamToReinforce must auto-reinforce with recruit/train + teamDelay shortcut"
    );
}

#[test]
fn team_evaluate_production_condition_cpp() {
    let team = gamelogic::team::TEAM_SRC;
    let ai = gamelogic::ai::ai_player::AI_PLAYER_SRC;
    assert!(
        team.contains("evaluate_production_condition")
            && team.contains("production_condition_runtime")
            && team.contains("always_false")
            && team.contains("evaluate_conditions")
            && ai.contains("evaluate_production_condition()"),
        "TeamPrototype::evaluateProductionCondition + isAGoodIdea wire required"
    );
}

#[test]
fn aiplayer_queue_units_cpp() {
    let src = gamelogic::ai::ai_player::AI_PLAYER_SRC;
    assert!(
        src.contains("C++ `AIPlayer::queueUnits`")
            && src.contains("try_to_recruit")
            && src.contains("while order.is_waiting_to_build()")
            && src.contains("ai_move_to_position")
            && src.contains("ai_idle"),
        "queueUnits must tryToRecruit then startTraining (C++)"
    );
}

#[test]
fn aiplayer_is_possible_to_build_team_cpp() {
    let src = gamelogic::ai::ai_player::AI_PLAYER_SRC;
    assert!(
        src.contains("C++ `AIPlayer::isPossibleToBuildTeam`")
            && src.contains("any_idle")
            && src.contains("max_units as f32 + min_units as f32")
            && src.contains("team_resources_to_build"),
        "isPossibleToBuildTeam C++ factory/cost semantics required"
    );
}

#[test]
fn aiplayer_start_training_cpp() {
    let src = gamelogic::ai::ai_player::AI_PLAYER_SRC;
    assert!(
        src.contains("C++ `AIPlayer::startTraining`")
            && src.contains("queue_unit_with_production_id")
            && src.contains("request_unique_unit_production_id")
            && src.contains("C++ `AIPlayer::findFactory`")
            && src.contains("get_build_list"),
        "startTraining/findFactory must queueCreateUnit via build-list factories"
    );
}

#[test]
fn aiplayer_on_unit_produced_cpp() {
    let src = gamelogic::ai::ai_player::AI_PLAYER_SRC;
    assert!(
        src.contains("C++ `AIPlayer::onUnitProduced`")
            && src.contains("self.team_delay = 0")
            && src.contains("set_force_wanting_state")
            && src.contains("structure_timer = 1"),
        "onUnitProduced must setTeam/delays/supply/dozer like C++"
    );
}

#[test]
fn aiplayer_build_structure_with_dozer_cpp() {
    let src = gamelogic::ai::ai_player::AI_PLAYER_SRC;
    assert!(
        src.contains("C++ `AIPlayer::buildStructureWithDozer`")
            && src.contains("set_build_task")
            && src.contains("build_structure_with_dozer"),
        "buildStructureWithDozer must be wired for USE_DOZER base building"
    );
}

#[test]
fn aiplayer_new_map_and_build_structure_now_cpp() {
    let src = gamelogic::ai::ai_player::AI_PLAYER_SRC;
    assert!(
        src.contains("C++ `AIPlayer::newMap`")
            && src.contains("C++ `AIPlayer::buildStructureNow`")
            && src.contains("C++ `AIPlayer::checkForSupplyCenter`")
            && src.contains("is_initially_built")
            && src.contains("set_desired_gatherers(desired + 1)"),
        "newMap/buildStructureNow/checkForSupplyCenter C++ paths required"
    );
}

#[test]
fn aiplayer_queue_supply_truck_cpp() {
    let src = gamelogic::ai::ai_player::AI_PLAYER_SRC;
    assert!(
        src.contains("C++ `AIPlayer::queueSupplyTruck`")
            && src.contains("truck_in_queue")
            && src.contains("queue_one_harvester_at_factory")
            && src.contains("try_reattach_loose_harvester"),
        "queueSupplyTruck C++ gatherer path required"
    );
}

#[test]
fn aiplayer_find_dozer_queue_supply_safe_cpp() {
    let src = gamelogic::ai::ai_player::AI_PLAYER_SRC;
    assert!(
        src.contains("C++ `AIPlayer::findDozer`")
            && src.contains("C++ `AIPlayer::queueDozer`")
            && src.contains("C++ `AIPlayer::isSupplySourceAttacked`")
            && src.contains("C++ `AIPlayer::isSupplySourceSafe`")
            && src.contains("C++ `AIPlayer::guardSupplyCenter`")
            && src.contains("is_currently_ferrying_supplies")
            && src.contains("start_training_internal"),
        "findDozer/queueDozer/supply safety C++ paths required"
    );
}

#[test]
fn aiplayer_compute_superweapon_cpp() {
    let src = gamelogic::ai::ai_player::AI_PLAYER_SRC;
    assert!(
        src.contains("C++ `AIPlayer::computeSuperweaponTarget`")
            && src.contains("C++ `AIPlayer::getPlayerSuperweaponValue`")
            && src.contains("Fine tune: C++ uses (x-5) for BOTH axes")
            && src.contains("FSBaseDefense"),
        "superweapon targeting C++ parity required"
    );
}

#[test]
fn aiplayer_build_upgrade_repair_cpp() {
    let src = gamelogic::ai::ai_player::AI_PLAYER_SRC;
    assert!(
        src.contains("C++ `AIPlayer::buildUpgrade`")
            && src.contains("C++ `AIPlayer::buildBySupplies`")
            && src.contains("C++ `AIPlayer::repairStructure`")
            && src.contains("C++ `AIPlayer::updateBridgeRepair`")
            && src.contains("add_to_priority_build_list")
            && src.contains("AiCommandType::Repair"),
        "buildUpgrade/buildBySupplies/repair bridge C++ paths required"
    );
}

#[test]
fn aiplayer_find_supply_nearest_cpp() {
    let src = gamelogic::ai::ai_player::AI_PLAYER_SRC;
    assert!(
        src.contains("C++ `AIPlayer::findSupplyCenter`")
            && src.contains("C++ `AIPlayer::buildSpecificBuildingNearestTeam`")
            && src.contains("C++ `AIPlayer::calcClosestConstructionZoneLocation`")
            && src.contains("C++ `AIPlayer::onStructureProduced`")
            && src.contains("dist_sqr * 0.4")
            && src.contains("RebuildHole"),
        "findSupplyCenter/nearest/onStructure C++ paths required"
    );
}

#[test]
fn aiplayer_build_recruit_team_cpp() {
    let src = gamelogic::ai::ai_player::AI_PLAYER_SRC;
    assert!(
        src.contains("C++ `AIPlayer::buildSpecificAITeam`")
            && src.contains("C++ `AIPlayer::recruitSpecificAITeam`")
            && src.contains("create_inactive_team")
            && src.contains("team_ready_queue.push_front")
            && src.contains("get_can_build_units"),
        "buildSpecificAITeam/recruitSpecificAITeam C++ paths required"
    );
}

#[test]
fn aiskirmish_base_defense_new_map_cpp() {
    let src = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../GameEngine/GameLogic/src/ai/skirmish_player.rs"
    ));
    assert!(
        src.contains("C++ `AISkirmishPlayer::buildAIBaseDefenseStructure`")
            && src.contains("C++ `AISkirmishPlayer::newMap`")
            && src.contains("add_to_priority_build_list")
            && src.contains("build_structure_now_at_public"),
        "skirmish defense/newMap C++ paths required"
    );
}

#[test]
fn aiskirmish_process_base_building_cpp() {
    let src = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../GameEngine/GameLogic/src/ai/skirmish_player.rs"
    ));
    assert!(
        src.contains("build_structure_with_dozer")
            && src.contains("ResumeConstruction")
            && src.contains("processBaseBuilding"),
        "skirmish processBaseBuilding dozer path required"
    );
}

#[test]
fn aiskirmish_select_team_cpp() {
    let src = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../GameEngine/GameLogic/src/ai/skirmish_player.rs"
    ));
    assert!(
        src.contains("selectTeamToBuild")
            && src.contains("is_a_good_idea_to_build_team(proto.get_name()")
            && src.contains("select_team_to_reinforce(min_priority)"),
        "skirmish selectTeam C++ delegation required"
    );
}

#[test]
fn aiskirmish_update_phase_cpp() {
    let src = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../GameEngine/GameLogic/src/ai/skirmish_player.rs"
    ));
    assert!(
        src.contains("C++ `AISkirmishPlayer::update`")
            && src.contains("check_ready_teams")
            && src.contains("do_base_building()")
            && src.contains("do_team_building()")
            && src.contains("check_queued_teams")
            && src.contains("update_bridge_repair"),
        "skirmish update C++ phase order required"
    );
}
