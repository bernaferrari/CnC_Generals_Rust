//! Behavior suite extracted from `production_and_mobs`: production queues, dozer
//! and supply docks, repair pads, path extra distance, and ignore management.
use super::*;

fn assert_reserved_dock_approach_path(
    logic: &GameLogic,
    dock_id: crate::game_logic::ObjectId,
    docker_id: crate::game_logic::ObjectId,
    start: Vec3,
    dock_pos: Vec3,
    dock_radius: f32,
) {
    let (_, queue) = logic
        .host_dock_approach_queue_snapshot()
        .into_iter()
        .find(|(id, queue)| *id == dock_id && queue.index_of(docker_id).is_some())
        .expect("dock must retain the docker's registered approach slot");
    let slot = queue.index_of(docker_id).expect("reserved approach slot") as usize;
    let registered = queue.approach_world_position(slot, start, dock_pos, dock_radius);
    let unit = logic.host_object(docker_id).expect("docker");
    assert!(
        unit.is_approach_path,
        "dock request must keep approach mode"
    );
    assert_eq!(unit.requested_destination, Some(registered));
    assert!(unit.ignored_obstacle_id.is_none());
    let end = *unit.movement.path.last().expect("approach path endpoint");
    let endpoint_cell = logic.pathfinding_system.grid.world_to_grid(end);
    assert!(
        logic.pathfinding_system.grid.valid_movement_position(
            endpoint_cell,
            gamelogic::ai::pathfind_astar::PathfindLayerEnum::Ground,
            unit.locomotor_surfaces,
            unit.crusher_level > 0,
            0,
        ),
        "closest approach endpoint must be a legal ground cell, end={end:?}"
    );
    assert!(
        end.distance(registered) < start.distance(registered),
        "path endpoint must make progress toward registered approach {registered:?}, got {end:?}"
    );
    assert!(
        unit.movement.path.windows(2).all(|segment| logic
            .pathfinding_system
            .line_passable_for_surfaces(segment[0], segment[1], unit.locomotor_surfaces)),
        "dock approach path must not cross the dock obstacle: {:?}",
        unit.movement.path
    );
}

#[test]
fn control_bar_queue_slot_cancel_releases_player_upgrade_state() {
    use crate::game_logic::{
        Player, Resources, ThingTemplate,
        buildings::{BuildingData, BuildingType},
    };

    const UPGRADE: &str = "Upgrade_AmericaSupplyLines";
    let cost = Resources {
        supplies: 800,
        power: 0,
    };

    let mut logic = GameLogic::new();
    let mut player = Player::new(0, Team::USA, "USA", true);
    player.resources.supplies = 5_000;
    logic.add_player(player);

    let mut producer_template = ThingTemplate::new("TestBarracks");
    producer_template
        .add_kind_of(KindOf::Structure)
        .add_kind_of(KindOf::Selectable)
        .set_health(1_000.0);
    logic
        .templates
        .insert("TestBarracks".to_string(), producer_template);
    let producer = logic
        .create_object("TestBarracks", Team::USA, Vec3::ZERO)
        .expect("producer");
    let building = logic.host_object_mut(producer).expect("producer object");
    building.building_data = Some(BuildingData::new(BuildingType::Barracks));

    // Queue research through the coupled player + producer state that the
    // ControlBar's build-queue icon later cancels by slot.
    assert!(
        logic
            .get_player_mut(0)
            .expect("player")
            .queue_upgrade(UPGRADE, &cost)
    );
    assert!(
        logic
            .host_object_mut(producer)
            .and_then(|object| object.building_data.as_mut())
            .expect("building data")
            .add_upgrade_to_queue(UPGRADE.to_string(), 30.0, cost.clone())
    );
    assert_eq!(
        logic.get_player(0).expect("player").effective_supplies(),
        4_200
    );

    // This is the Main authority endpoint used by HostControlBarRequest::QueueCancel.
    assert!(logic.cancel_production_at_index(producer, 0));

    let player = logic.get_player(0).expect("player after cancellation");
    assert!(
        !player.has_queued_upgrade(UPGRADE),
        "C++ cancelUpgrade clears the player's IN_PRODUCTION state"
    );
    assert_eq!(player.effective_supplies(), 5_000, "research cost refunded");
    assert!(
        logic
            .host_object(producer)
            .and_then(|object| object.building_data.as_ref())
            .is_some_and(|building| building.production_queue.is_empty())
    );
}
#[test]
fn cancel_production_refunds_controlling_player_not_first_same_team() {
    // C++ ProductionUpdate.cpp:316 / :456 — cancelUpgrade / cancelUnitCreate
    // deposit to getObject()->getControllingPlayer(), never the first
    // same-faction PlayerList slot.
    let mut game_logic = GameLogic::new();
    ensure_test_player_for_team(&mut game_logic, Team::USA);
    ensure_test_barracks_template(&mut game_logic);
    ensure_test_infantry_template(&mut game_logic);

    let mut teammate = Player::new(4, Team::USA, "USA2", true);
    teammate.resources.supplies = 80_000;
    teammate.power_available = 100;
    game_logic.add_player(teammate);

    // Pin ownership to whoever get_player_by_team would *not* pick so a
    // first-same-team refund cannot accidentally look correct.
    let first_same_team = game_logic
        .get_player_by_team(Team::USA)
        .expect("USA player")
        .id;
    let owner_id = if first_same_team == 0 { 4 } else { 0 };
    let other_id = first_same_team;
    let owner_before = game_logic
        .get_player(owner_id)
        .expect("owner")
        .effective_supplies();
    let other_before = game_logic
        .get_player(other_id)
        .expect("other same-team player")
        .effective_supplies();

    let barracks_id = game_logic
        .create_object_for_player("TestBarracks", owner_id, Vec3::ZERO)
        .expect("barracks should be created for the controlling player");
    assert!(game_logic.enqueue_production(barracks_id, "TestInfantry".to_string()));
    let charged = owner_before.saturating_sub(
        game_logic
            .get_player(owner_id)
            .expect("owner after enqueue")
            .effective_supplies(),
    );
    assert!(charged > 0, "enqueue must charge the controlling player");
    assert_eq!(
        game_logic
            .get_player(other_id)
            .expect("other after enqueue")
            .effective_supplies(),
        other_before,
        "enqueue must not charge the first same-team player"
    );

    assert!(game_logic.cancel_production(barracks_id, "TestInfantry".to_string()));
    assert_eq!(
        game_logic
            .get_player(owner_id)
            .expect("owner after cancel")
            .effective_supplies(),
        owner_before,
        "C++ getControllingPlayer refund must restore the owner"
    );
    assert_eq!(
        game_logic
            .get_player(other_id)
            .expect("other after cancel")
            .effective_supplies(),
        other_before,
        "first same-team player must not receive the cancel refund"
    );
}

#[test]
fn exact_player_template_production_maps_keep_cpp_namekey_semantics() {
    use crate::game_logic::host_upgrade_module_residuals::apply_production_cost_factor;
    use game_engine::common::game_common::VeterancyLevel as CommonVeterancyLevel;
    use game_engine::common::name_key_generator::NameKeyGenerator;
    use game_engine::common::rts::player_template::PlayerTemplate;
    use gamelogic::world::entities::production_total_logic_frames;

    // C++ Player::init copies these three maps from its selected
    // PlayerTemplate, and Player.cpp subsequently queries them by NAMEKEY of
    // the exact ThingTemplate name.  Exercise the Main adapter without a
    // global-store mutation so parallel tests cannot alter retail templates.
    let mut template = PlayerTemplate::new("TestExactGeneral".to_string());
    template
        .production_cost_changes
        .insert(NameKeyGenerator::name_to_key("ExactUnit"), -0.20);
    template
        .production_time_changes
        .insert(NameKeyGenerator::name_to_key("ExactUnit"), 0.25);
    template.production_veterancy_levels.insert(
        NameKeyGenerator::name_to_key("ExactUnit"),
        CommonVeterancyLevel::Elite,
    );

    assert!(
        (PlayerTemplateIdentity::production_cost_factor_for_template(&template, "ExactUnit")
            - 0.80)
            .abs()
            < f32::EPSILON
    );
    assert!(
        (PlayerTemplateIdentity::production_time_factor_for_template(&template, "ExactUnit")
            - 1.25)
            .abs()
            < f32::EPSILON
    );
    assert_eq!(
        apply_production_cost_factor(101, 0.80),
        80,
        "C++ calcCostToBuild returns Int after the real General multiplier"
    );

    // C++ calcTimeToBuild does `Int(build_time * 30)` *before* multiplying
    // the PlayerTemplate factor, then truncates again before its later
    // low-power division.  The old seconds-first route completed this 2.099s
    // item in 78 frames; retail's two integer boundaries yield 77.
    let exact_time_seconds = GameLogic::cpp_build_time_seconds_from_factor(
        2.099,
        PlayerTemplateIdentity::production_time_factor_for_template(&template, "ExactUnit"),
    );
    assert_eq!(
        GameLogic::cpp_build_time_frames_from_factor(2.099, 1.25),
        77,
        "C++ applies the General factor to the integer base-frame count"
    );
    assert_eq!(
        production_total_logic_frames(exact_time_seconds, false, 1.0),
        77,
        "General production-time factor must apply after the base 30-FPS truncation"
    );
    assert_eq!(
        production_total_logic_frames(exact_time_seconds, false, 0.5),
        154,
        "low-power division must remain after the authored PlayerTemplate frame truncation"
    );
    let frame_boundary_seconds = GameLogic::cpp_build_time_seconds_from_factor(4.2, 1.0);
    assert_eq!(
        production_total_logic_frames(frame_boundary_seconds, false, 1.0),
        125,
        "seconds carrier must not round a valid C++ frame count down before queue completion"
    );
    assert_eq!(
        PlayerTemplateIdentity::production_veterancy_for_template(&template, "ExactUnit"),
        VeterancyLevel::Elite,
    );
    assert_eq!(
        PlayerTemplateIdentity::production_veterancy_for_template(&template, "OtherUnit"),
        VeterancyLevel::Rookie,
        "C++ Player::getProductionVeterancyLevel defaults to LEVEL_FIRST"
    );
}

#[test]
fn dozer_construction_rate_applies_handicap_buildtime() {
    // hq-mzfv5: C++ calcTimeToBuild multiplies Handicap::BUILDTIME before power.
    use crate::game_logic::{KindOf, Player, ThingTemplate};
    let mut logic = GameLogic::new();
    let mut player = Player::new(0, Team::USA, "P0", true);
    player.map_side.handicap_build_time_buildings = 2.0;
    player.power_produced = 10;
    player.power_consumed = 0;
    logic.add_player(player);

    let mut pad = ThingTemplate::new("HandicapPad");
    pad.add_kind_of(KindOf::Structure)
        .add_kind_of(KindOf::Selectable)
        .set_health(1_000.0);
    pad.build_time = 10.0;
    logic.templates.insert("HandicapPad".into(), pad);

    let mut dozer_tpl = ThingTemplate::new("HandicapDozer");
    dozer_tpl
        .add_kind_of(KindOf::Vehicle)
        .add_kind_of(KindOf::Dozer)
        .set_health(200.0);
    logic.templates.insert("HandicapDozer".into(), dozer_tpl);

    let pad_id = logic
        .create_object_for_player("HandicapPad", 0, Vec3::ZERO)
        .expect("pad");
    let dozer = logic
        .create_object_for_player("HandicapDozer", 0, Vec3::new(5.0, 0.0, 0.0))
        .expect("dozer");
    {
        let obj = logic.host_object_mut(pad_id).expect("pad");
        obj.set_status_under_construction(true);
        obj.construction_percent = 0.0;
        obj.builder_id = Some(dozer);
    }
    {
        let obj = logic.host_object_mut(dozer).expect("dozer");
        obj.set_target(Some(pad_id));
        obj.set_ai_state(AIState::Constructing);
        obj.status.moving = false;
        obj.dozer_dock_action = Some(obj.get_position());
    }

    logic.update_construction(&[pad_id], 1.0);
    let progress = logic.host_object(pad_id).unwrap().construction_percent;
    // 10s * 30 = 300 frames * 2.0 handicap = 600 frames → 30/600 = 0.05 / sec.
    assert!(
        (progress - 0.05).abs() < 0.001,
        "hq-mzfv5: handicap BUILDTIME must scale dozer construction, got {progress}"
    );
}

#[test]
fn arrived_moving_dozer_builds_at_the_dock() {
    use crate::game_logic::{KindOf, Player, ThingTemplate};
    let mut logic = GameLogic::new();
    logic.add_player(Player::new(0, Team::USA, "P0", true));
    let mut pad = ThingTemplate::new("ArrivedPad");
    pad.add_kind_of(KindOf::Structure)
        .add_kind_of(KindOf::Selectable)
        .set_health(1_000.0);
    pad.build_time = 10.0;
    logic.templates.insert("ArrivedPad".into(), pad);
    let mut dozer_tpl = ThingTemplate::new("ArrivedDozer");
    dozer_tpl
        .add_kind_of(KindOf::Vehicle)
        .add_kind_of(KindOf::Dozer)
        .set_health(200.0);
    logic.templates.insert("ArrivedDozer".into(), dozer_tpl);
    let pad_id = logic
        .create_object_for_player("ArrivedPad", 0, Vec3::ZERO)
        .expect("pad");
    let dozer = logic
        .create_object_for_player("ArrivedDozer", 0, Vec3::new(40.0, 0.0, 0.0))
        .expect("dozer");
    logic.dozer_new_task_build(dozer, pad_id);
    let dock = logic
        .host_object(dozer)
        .and_then(|d| d.dozer_dock_action)
        .expect("dock");
    {
        let obj = logic.host_object_mut(pad_id).expect("pad");
        obj.set_status_under_construction(true);
        obj.builder_id = Some(dozer);
    }
    {
        let obj = logic.host_object_mut(dozer).expect("dozer");
        obj.set_position(dock);
        obj.set_target(Some(pad_id));
        obj.set_ai_state(AIState::Moving);
        obj.movement.path.clear();
        obj.waiting_for_path = false;
    }
    logic.update_construction(&[pad_id], 1.0);
    let progress = logic.host_object(pad_id).unwrap().construction_percent;
    assert!(
        progress > 0.0,
        "an arrived dozer still tagged Moving must build, got {progress}"
    );
}

#[test]
fn simulation_step_builds_when_dozer_is_at_the_pad() {
    use crate::game_logic::{KindOf, Player, ThingTemplate};
    let mut logic = GameLogic::new();
    logic.add_player(Player::new(0, Team::USA, "P0", true));
    let mut pad = ThingTemplate::new("SimPad");
    pad.add_kind_of(KindOf::Structure)
        .add_kind_of(KindOf::Selectable)
        .set_health(1_000.0);
    pad.build_time = 10.0;
    logic.templates.insert("SimPad".into(), pad);
    let mut dozer_tpl = ThingTemplate::new("SimDozer");
    dozer_tpl
        .add_kind_of(KindOf::Vehicle)
        .add_kind_of(KindOf::Dozer)
        .set_health(200.0);
    logic.templates.insert("SimDozer".into(), dozer_tpl);
    let pad_id = logic
        .create_object_for_player("SimPad", 0, Vec3::ZERO)
        .expect("pad");
    let dozer = logic
        .create_object_for_player("SimDozer", 0, Vec3::ZERO)
        .expect("dozer");
    logic.dozer_new_task_build(dozer, pad_id);
    let dock = logic
        .host_object(dozer)
        .and_then(|d| d.dozer_dock_action)
        .unwrap_or(Vec3::ZERO);
    {
        let obj = logic.host_object_mut(pad_id).expect("pad");
        obj.set_status_under_construction(true);
        obj.builder_id = Some(dozer);
    }
    {
        let obj = logic.host_object_mut(dozer).expect("dozer");
        obj.set_position(dock);
        obj.set_target(Some(pad_id));
        obj.set_ai_state(AIState::Constructing);
        obj.movement.path.clear();
        obj.waiting_for_path = false;
    }
    for _ in 0..30 {
        logic.update();
    }
    let progress = logic.host_object(pad_id).unwrap().construction_percent;
    assert!(
        progress > 0.0,
        "a simulation step must build while the dozer is at the pad, got {progress}"
    );
}

#[test]
fn simulation_step_finishes_a_short_build() {
    use crate::game_logic::{KindOf, Player, ThingTemplate};
    let mut logic = GameLogic::new();
    logic.add_player(Player::new(0, Team::USA, "P0", true));
    let mut pad = ThingTemplate::new("FinishPad");
    pad.add_kind_of(KindOf::Structure)
        .add_kind_of(KindOf::Selectable)
        .set_health(1_000.0);
    pad.build_time = 0.1;
    logic.templates.insert("FinishPad".into(), pad);
    let mut dozer_tpl = ThingTemplate::new("FinishDozer");
    dozer_tpl
        .add_kind_of(KindOf::Vehicle)
        .add_kind_of(KindOf::Dozer)
        .set_health(200.0);
    logic.templates.insert("FinishDozer".into(), dozer_tpl);
    let pad_id = logic
        .create_object_for_player("FinishPad", 0, Vec3::ZERO)
        .expect("pad");
    let dozer = logic
        .create_object_for_player("FinishDozer", 0, Vec3::ZERO)
        .expect("dozer");
    logic.dozer_new_task_build(dozer, pad_id);
    let action = logic
        .host_object(dozer)
        .and_then(|d| d.dozer_dock_action)
        .expect("dozer_new_task_build stores the action dock");
    let building_pos = logic.host_object(pad_id).expect("pad").get_position();
    let end_dock = crate::game_logic::host_repair::dozer_end_dock_position(action, building_pos);
    {
        let obj = logic.host_object_mut(pad_id).expect("pad");
        obj.set_status_under_construction(true);
        obj.builder_id = Some(dozer);
    }
    {
        let obj = logic.host_object_mut(dozer).expect("dozer");
        obj.set_position(action);
        obj.set_target(Some(pad_id));
        obj.set_ai_state(AIState::Constructing);
        obj.movement.path.clear();
        obj.waiting_for_path = false;
    }
    let mut at_finish: Option<Vec3> = None;
    for _ in 0..30 {
        logic.update();
        let done = logic
            .host_object(pad_id)
            .is_some_and(|pad| !pad.status.under_construction && pad.construction_percent >= 1.0);
        if done && at_finish.is_none() {
            at_finish = logic
                .host_object(dozer)
                .and_then(|dozer_obj| dozer_obj.movement.path.last().copied());
        }
    }
    let pad = logic.host_object(pad_id).unwrap();
    assert!(
        !pad.status.under_construction && pad.construction_percent >= 1.0,
        "a short build must finish, percent={} under={}",
        pad.construction_percent,
        pad.status.under_construction
    );
    let path_end = at_finish.expect("completion-frame path to the end dock");
    assert!(
        path_end.distance(end_dock) < 15.0,
        "completion-frame path must end at the end dock, path_end={path_end:?} end_dock={end_dock:?}"
    );
    let unit = logic.host_object(dozer).expect("dozer");
    assert!(
        unit.get_position().distance(action) > 1.0,
        "pos={:?} ai={:?} vel={:?} ignore={:?} target={:?} path={:?}",
        unit.get_position(),
        unit.ai_state,
        unit.movement.velocity,
        unit.ignored_obstacle_id,
        unit.movement.target_position,
        unit.movement.path
    );
    assert_eq!(
        unit.dozer_task_build_target, None,
        "a finished build must clear the dozer task or construct buttons stay grey"
    );
}

#[test]
fn finished_move_clears_the_ignored_obstacle() {
    use crate::game_logic::{KindOf, Player, ThingTemplate};
    let mut logic = GameLogic::new();
    logic.add_player(Player::new(0, Team::USA, "P0", true));
    let mut tpl = ThingTemplate::new("Mover");
    tpl.add_kind_of(KindOf::Vehicle).set_health(100.0);
    logic.templates.insert("Mover".into(), tpl);
    let id = logic
        .create_object_for_player("Mover", 0, Vec3::ZERO)
        .expect("unit");
    let building = logic
        .create_object_for_player("Mover", 0, Vec3::new(40.0, 0.0, 0.0))
        .expect("building");
    {
        let unit = logic.host_object_mut(id).expect("unit");
        unit.ignored_obstacle_id = Some(building);
        unit.set_ai_state(AIState::Repairing);
        unit.movement.path.push(Vec3::new(10.0, 0.0, 0.0));
        unit.stop_moving();
        assert_eq!(
            unit.ignored_obstacle_id,
            Some(building),
            "a pack or repair stop must keep the building ignore"
        );
        assert_eq!(unit.ai_state, AIState::Repairing);
    }
    {
        let unit = logic.host_object_mut(id).expect("unit");
        unit.set_ai_state(AIState::Moving);
        unit.set_position(Vec3::ZERO);
        unit.movement.path = vec![Vec3::ZERO];
        unit.movement.current_path_index = 0;
        unit.movement.target_position = Some(Vec3::ZERO);
        unit.ignored_obstacle_id = Some(building);
        unit.update_movement(1.0 / 30.0);
        assert_eq!(unit.ignored_obstacle_id, None);
        assert_eq!(unit.ai_state, AIState::Idle);
        assert!(unit.movement.path.is_empty());
    }
    {
        let unit = logic.host_object_mut(id).expect("unit");
        let near = Vec3::new(1.0, 0.0, 0.0);
        unit.set_ai_state(AIState::Moving);
        unit.set_position(Vec3::ZERO);
        unit.movement.velocity = Vec3::ZERO;
        unit.movement.path = vec![near];
        unit.movement.current_path_index = 0;
        unit.movement.target_position = Some(near);
        unit.ignored_obstacle_id = Some(building);
        unit.update_movement(1.0 / 30.0);
        assert_eq!(
            unit.ignored_obstacle_id, None,
            "arriving inside close-enough distance must drop the ignore"
        );
    }
}

#[test]
fn repair_abort_clears_the_ignored_obstacle() {
    use crate::game_logic::{KindOf, Player, ThingTemplate};
    let mut logic = GameLogic::new();
    logic.add_player(Player::new(0, Team::USA, "P0", true));
    let mut tpl = ThingTemplate::new("NotADozer");
    tpl.add_kind_of(KindOf::Vehicle).set_health(100.0);
    logic.templates.insert("NotADozer".into(), tpl);
    let id = logic
        .create_object_for_player("NotADozer", 0, Vec3::ZERO)
        .expect("unit");
    let building = logic
        .create_object_for_player("NotADozer", 0, Vec3::new(40.0, 0.0, 0.0))
        .expect("building");
    {
        let unit = logic.host_object_mut(id).expect("unit");
        unit.set_order_target(Some(building));
        unit.set_ai_state(AIState::Repairing);
        unit.ignored_obstacle_id = Some(building);
    }
    logic.update_support_states_for_test(&[id], 1.0 / 30.0);
    let unit = logic.host_object(id).expect("unit");
    assert_eq!(
        unit.ignored_obstacle_id, None,
        "a unit that cannot repair must drop the building ignore"
    );
    assert!(unit.target.is_none());
}

#[test]
fn full_truck_returns_to_the_supply_center_approach() {
    use crate::game_logic::host_structure_economy_residual::VALUE_PER_SUPPLY_BOX;
    use crate::game_logic::{
        DockKind, KindOf, Player, SupplyTruckMetadata, SupplyTruckState, ThingTemplate,
    };
    let mut logic = GameLogic::new();
    logic.add_player(Player::new(0, Team::USA, "P0", true));
    let mut warehouse_t = ThingTemplate::new("SupplyWarehouse");
    warehouse_t.add_kind_of(KindOf::Structure).set_health(500.0);
    warehouse_t.dock_kind = DockKind::SupplyWarehouse;
    logic
        .templates
        .insert("SupplyWarehouse".into(), warehouse_t);
    let mut center_t = ThingTemplate::new("AmericaSupplyCenter");
    center_t
        .add_kind_of(KindOf::Structure)
        .add_kind_of(KindOf::SupplyCenter)
        .set_health(500.0);
    center_t.dock_kind = DockKind::SupplyCenter;
    logic
        .templates
        .insert("AmericaSupplyCenter".into(), center_t);
    let mut truck_t = ThingTemplate::new("AmericaSupplyTruck");
    truck_t.add_kind_of(KindOf::Vehicle).set_health(200.0);
    truck_t.supply_truck_metadata = Some(SupplyTruckMetadata {
        max_boxes: 1,
        warehouse_scan_distance: 200.0,
        warehouse_delay_frames: 0,
        center_delay_frames: 0,
        upgraded_supply_boost: 0,
    });
    logic.templates.insert("AmericaSupplyTruck".into(), truck_t);
    let center_pos = Vec3::new(200.0, 0.0, 0.0);
    let warehouse = logic
        .create_object_for_player("SupplyWarehouse", 0, Vec3::ZERO)
        .expect("warehouse");
    let center = logic
        .create_object_for_player("AmericaSupplyCenter", 0, center_pos)
        .expect("center");
    let truck = logic
        .create_object_for_player("AmericaSupplyTruck", 0, Vec3::ZERO)
        .expect("truck");
    {
        let src = logic.host_object_mut(warehouse).expect("warehouse");
        src.set_stored_supplies(VALUE_PER_SUPPLY_BOX as u32);
        src.dock_active_docker = Some(truck);
        src.selection_radius = 40.0;
    }
    {
        let pad = logic.host_object_mut(center).expect("center");
        pad.set_status_under_construction(false);
        pad.construction_percent = 1.0;
        pad.selection_radius = 40.0;
    }
    {
        let unit = logic.host_object_mut(truck).expect("truck");
        unit.set_order_target(Some(warehouse));
        unit.set_ai_state(AIState::Gathering);
        unit.set_stored_supplies(VALUE_PER_SUPPLY_BOX as u32);
        unit.supply_truck_state = SupplyTruckState::DockingWarehouse;
        unit.supply_truck_next_dock_action_frame = 0;
    }
    logic.update_support_states_for_test(&[truck], 1.0 / 30.0);
    let unit = logic.host_object(truck).expect("truck");
    assert_eq!(unit.ai_state, AIState::ReturningResources);
    let end = unit
        .movement
        .path
        .last()
        .copied()
        .expect("installed return path");
    let approach = Vec3::new(180.0, 0.0, 0.0);
    assert!(
        end.distance(approach) < 1.0,
        "path must end at the dock approach {approach:?}, end={end:?} path={:?}",
        unit.movement.path
    );
    assert!(unit.ignored_obstacle_id.is_none());
    logic
        .host_object_mut(warehouse)
        .expect("warehouse")
        .status
        .destroyed = true;
    let mut next_t = ThingTemplate::new("NextSupplyWarehouse");
    next_t
        .add_kind_of(KindOf::Structure)
        .add_kind_of(KindOf::SupplySource)
        .set_health(500.0);
    next_t.dock_kind = DockKind::SupplyWarehouse;
    logic.templates.insert("NextSupplyWarehouse".into(), next_t);
    let next = logic
        .create_object_for_player("NextSupplyWarehouse", 0, Vec3::new(300.0, 0.0, 0.0))
        .expect("next warehouse");
    {
        let src = logic.host_object_mut(next).expect("next");
        src.set_stored_supplies(VALUE_PER_SUPPLY_BOX as u32);
        src.selection_radius = 40.0;
        src.set_status_under_construction(false);
        src.construction_percent = 1.0;
    }
    {
        let unit = logic.host_object_mut(truck).expect("truck");
        unit.set_position(end);
        unit.movement.path.clear();
        unit.movement.target_position = None;
        unit.supply_truck_state = SupplyTruckState::Wanting;
    }
    let start = logic.players.get(&0).expect("player").resources.supplies;
    for _ in 0..4 {
        logic.update_support_states_for_test(&[truck], 1.0 / 30.0);
    }
    let cash = logic.players.get(&0).expect("player").resources.supplies;
    assert_eq!(
        cash,
        start + VALUE_PER_SUPPLY_BOX as u32,
        "a truck on the dock approach must pay the box"
    );
    let back = logic.host_object(truck).expect("truck");
    assert_eq!(back.stored_resources.supplies, 0);
    assert_eq!(back.ai_state, AIState::Gathering);
    assert_ne!(back.ai_state, AIState::Attacking);
    let back_end = back
        .movement
        .path
        .last()
        .copied()
        .expect("path to the next warehouse");
    assert!(
        back_end.distance(Vec3::new(280.0, 0.0, 0.0)) < 1.0,
        "the next warehouse must be approached at its dock, end={back_end:?} path={:?}",
        back.movement.path
    );
}

#[test]
fn empty_truck_regroups_outside_the_supply_center() {
    use crate::game_logic::host_structure_economy_residual::VALUE_PER_SUPPLY_BOX;
    use crate::game_logic::{DockKind, KindOf, Player, SupplyTruckMetadata, ThingTemplate};
    let mut logic = GameLogic::new();
    logic.add_player(Player::new(0, Team::USA, "P0", true));
    let mut center_t = ThingTemplate::new("RegroupCenter");
    center_t
        .add_kind_of(KindOf::Structure)
        .add_kind_of(KindOf::SupplyCenter)
        .set_health(500.0);
    center_t.dock_kind = DockKind::SupplyCenter;
    logic.templates.insert("RegroupCenter".into(), center_t);
    let mut truck_t = ThingTemplate::new("RegroupTruck");
    truck_t.add_kind_of(KindOf::Vehicle).set_health(200.0);
    truck_t.supply_truck_metadata = Some(SupplyTruckMetadata {
        max_boxes: 1,
        warehouse_scan_distance: 500.0,
        warehouse_delay_frames: 0,
        center_delay_frames: 0,
        upgraded_supply_boost: 0,
    });
    logic.templates.insert("RegroupTruck".into(), truck_t);
    let center_pos = Vec3::new(200.0, 0.0, 0.0);
    let center = logic
        .create_object_for_player("RegroupCenter", 0, center_pos)
        .expect("center");
    let truck = logic
        .create_object_for_player("RegroupTruck", 0, Vec3::ZERO)
        .expect("truck");
    {
        let pad = logic.host_object_mut(center).expect("center");
        pad.selection_radius = 40.0;
        pad.set_status_under_construction(false);
        pad.construction_percent = 1.0;
    }
    {
        let unit = logic.host_object_mut(truck).expect("truck");
        unit.set_ai_state(AIState::Idle);
        unit.supply_truck_force_pending = true;
        unit.set_stored_supplies(VALUE_PER_SUPPLY_BOX as u32);
    }
    logic.update_support_states_for_test(&[truck], 1.0 / 30.0);
    let loaded = logic.host_object(truck).expect("truck");
    assert_eq!(loaded.ai_state, AIState::ReturningResources);
    let to_center = loaded.movement.path.last().copied().expect("center path");
    assert!(
        to_center.distance(Vec3::new(180.0, 0.0, 0.0)) < 1.0,
        "wanting with a box must dock the approach, end={to_center:?}"
    );
    {
        let unit = logic.host_object_mut(truck).expect("truck");
        unit.set_ai_state(AIState::Idle);
        unit.set_stored_supplies(0);
        unit.supply_truck_force_pending = true;
        unit.set_status_moving(false);
        unit.movement.path.clear();
        unit.movement.target_position = None;
        unit.ignored_obstacle_id = Some(center);
    }
    let mut terrain = crate::game_logic::terrain::TerrainData::flat(
        Vec3::new(-1000.0, 0.0, -1000.0),
        Vec3::new(1000.0, 0.0, 1000.0),
    );
    terrain.add_water_polygon(vec![(247, -3), (253, -3), (253, 3), (247, 3)], 10.0);
    logic.terrain = Some(terrain);
    logic.update_support_states_for_test(&[truck], 1.0 / 30.0);
    let regrouped = logic.host_object(truck).expect("truck");
    assert_eq!(regrouped.ai_state, AIState::Moving);
    assert_eq!(
        regrouped.supply_truck_state,
        crate::game_logic::SupplyTruckState::Regrouping
    );
    assert!(regrouped.ignored_obstacle_id.is_none());
    let regroup = regrouped
        .movement
        .path
        .last()
        .copied()
        .expect("regroup path");
    let from_center = regroup.distance(center_pos);
    assert!(
        from_center >= 45.0 - 0.1 && from_center <= 100.0,
        "regroup must clear the building by a 5-unit sphere and stay within 100, end={regroup:?}"
    );
    let in_water =
        regroup.x >= 247.0 && regroup.x <= 253.0 && regroup.z >= -3.0 && regroup.z <= 3.0;
    assert!(
        !in_water,
        "the first +X ring is underwater and must be skipped, end={regroup:?}"
    );
    {
        let unit = logic.host_object_mut(truck).expect("truck");
        unit.set_position(Vec3::new(150.0, 0.0, 0.0));
        unit.set_ai_state(AIState::Idle);
        unit.set_status_moving(false);
        unit.set_stored_supplies(0);
        unit.supply_truck_force_pending = true;
        unit.movement.path.clear();
        unit.movement.target_position = None;
        unit.ignored_obstacle_id = Some(center);
    }
    logic.update_support_states_for_test(&[truck], 1.0 / 30.0);
    let close = logic.host_object(truck).expect("truck");
    assert!(
        close.movement.path.is_empty(),
        "a truck already inside the bounding-sphere gap must not path, path={:?}",
        close.movement.path
    );
    assert!(close.ignored_obstacle_id.is_none());
    assert_eq!(
        close.supply_truck_state,
        crate::game_logic::SupplyTruckState::Regrouping
    );
}

#[test]
fn repair_approach_crosses_the_buildings_obstacle() {
    use crate::game_logic::pathfinding::GridPos;
    use crate::game_logic::{KindOf, Player, ThingTemplate};

    let mut logic = GameLogic::new();
    logic.add_player(Player::new(0, Team::USA, "P0", true));
    let mut pad = ThingTemplate::new("RepairPad");
    pad.add_kind_of(KindOf::Structure).set_health(1_000.0);
    logic.templates.insert("RepairPad".into(), pad);
    let mut dozer_tpl = ThingTemplate::new("RepairDozer");
    dozer_tpl
        .add_kind_of(KindOf::Vehicle)
        .add_kind_of(KindOf::Dozer)
        .set_health(200.0);
    logic.templates.insert("RepairDozer".into(), dozer_tpl);
    let from = Vec3::new(10.0, 0.0, 10.0);
    let to = Vec3::new(160.0, 0.0, 10.0);
    let pad_id = logic
        .create_object_for_player("RepairPad", 0, to)
        .expect("pad");
    let dozer = logic
        .create_object_for_player("RepairDozer", 0, from)
        .expect("dozer");
    {
        let building = logic.host_object_mut(pad_id).expect("pad");
        building.health.current = 100.0;
        building.set_status_under_construction(false);
        building.construction_percent = 1.0;
    }
    let wall_x = {
        let grid = &logic.pathfinding_system.grid;
        let start_cell = grid.world_to_grid(from);
        let goal_cell = grid.world_to_grid(to);
        (start_cell.x + goal_cell.x) / 2
    };
    let height = logic.pathfinding_system.grid.height();
    for y in 0..height {
        logic.pathfinding_system.grid.set_cell_obstacle_owned(
            GridPos::new(wall_x, y),
            false,
            false,
            pad_id.0,
            None,
            None,
        );
    }
    {
        let obj = logic.host_object_mut(dozer).expect("dozer");
        obj.set_target(Some(pad_id));
        obj.set_ai_state(AIState::Repairing);
        obj.movement.path.clear();
        obj.waiting_for_path = false;
    }
    logic.force_map_loaded_for_path_test(true);
    logic.update();
    logic.process_pathfind_queue();
    let (state, path) = {
        let obj = logic.host_object(dozer).expect("dozer");
        (obj.ai_state.clone(), obj.movement.path.clone())
    };
    assert_eq!(state, AIState::Repairing);
    let xs: Vec<i32> = path
        .iter()
        .map(|wp| logic.pathfinding_system.grid.world_to_grid(*wp).x)
        .collect();
    let crosses = xs.windows(2).any(|w| {
        let lo = w[0].min(w[1]);
        let hi = w[0].max(w[1]);
        lo <= wall_x && wall_x <= hi
    });
    assert!(
        crosses,
        "repair approach must ignore the building and cross its obstacle, xs={xs:?} wall={wall_x}"
    );
}

#[test]
fn finished_repair_releases_the_dozer() {
    use crate::game_logic::{KindOf, Player, ThingTemplate};
    let mut logic = GameLogic::new();
    logic.add_player(Player::new(0, Team::USA, "P0", true));
    let mut pad = ThingTemplate::new("HealPad");
    pad.add_kind_of(KindOf::Structure).set_health(200.0);
    logic.templates.insert("HealPad".into(), pad);
    let mut dozer_tpl = ThingTemplate::new("HealDozer");
    dozer_tpl
        .add_kind_of(KindOf::Vehicle)
        .add_kind_of(KindOf::Dozer)
        .set_health(200.0);
    logic.templates.insert("HealDozer".into(), dozer_tpl);
    let pad_id = logic
        .create_object_for_player("HealPad", 0, Vec3::ZERO)
        .expect("pad");
    let dozer = logic
        .create_object_for_player("HealDozer", 0, Vec3::ZERO)
        .expect("dozer");
    {
        let building = logic.host_object_mut(pad_id).expect("pad");
        building.set_status_under_construction(false);
        building.construction_percent = 1.0;
        building.health.current = building.health.maximum - 1.0;
    }
    {
        let unit = logic.host_object_mut(dozer).expect("dozer");
        unit.set_order_target(Some(pad_id));
        unit.set_ai_state(AIState::Repairing);
        unit.dozer_task_repair_target = Some(pad_id);
        unit.ignored_obstacle_id = Some(pad_id);
        unit.movement.path = vec![Vec3::new(20.0, 0.0, 0.0)];
        unit.set_status_moving(true);
    }
    logic.frame = 10;
    for _ in 0..8 {
        logic.update_support_states_for_test(&[dozer], 1.0);
        logic.frame += 1;
    }
    let building = logic.host_object(pad_id).expect("pad");
    assert!(
        building.health.current >= building.health.maximum - 0.01,
        "in-range dozer must finish the repair, hp={}",
        building.health.current
    );
    let unit = logic.host_object(dozer).expect("dozer");
    assert_eq!(unit.ai_state, AIState::Idle);
    assert!(unit.ignored_obstacle_id.is_none());
    assert!(unit.movement.path.is_empty());
    assert!(unit.dozer_task_repair_target.is_none());
}

#[test]
fn dead_repair_target_releases_the_dozer() {
    use crate::game_logic::{KindOf, Player, ThingTemplate};
    let mut logic = GameLogic::new();
    logic.add_player(Player::new(0, Team::USA, "P0", true));
    let mut pad = ThingTemplate::new("DeadPad");
    pad.add_kind_of(KindOf::Structure).set_health(200.0);
    logic.templates.insert("DeadPad".into(), pad);
    let mut dozer_tpl = ThingTemplate::new("DeadDozer");
    dozer_tpl
        .add_kind_of(KindOf::Vehicle)
        .add_kind_of(KindOf::Dozer)
        .set_health(200.0);
    logic.templates.insert("DeadDozer".into(), dozer_tpl);
    let pad_id = logic
        .create_object_for_player("DeadPad", 0, Vec3::ZERO)
        .expect("pad");
    let dozer = logic
        .create_object_for_player("DeadDozer", 0, Vec3::ZERO)
        .expect("dozer");
    {
        let building = logic.host_object_mut(pad_id).expect("pad");
        building.health.current = 0.0;
        building.status.destroyed = true;
        building.set_status_under_construction(false);
        building.construction_percent = 1.0;
    }
    {
        let unit = logic.host_object_mut(dozer).expect("dozer");
        unit.set_order_target(Some(pad_id));
        unit.set_ai_state(AIState::Repairing);
        unit.dozer_task_repair_target = Some(pad_id);
        unit.ignored_obstacle_id = Some(pad_id);
        unit.movement.path = vec![Vec3::new(30.0, 0.0, 0.0)];
        unit.set_status_moving(true);
    }
    logic.update_support_states_for_test(&[dozer], 1.0 / 30.0);
    let unit = logic.host_object(dozer).expect("dozer");
    assert_eq!(unit.ai_state, AIState::Idle);
    assert!(unit.ignored_obstacle_id.is_none());
    assert!(unit.movement.path.is_empty());
    assert!(unit.dozer_task_repair_target.is_none());
}

#[test]
fn repair_without_a_target_does_not_keep_the_task() {
    use crate::game_logic::{KindOf, Player, ThingTemplate};
    let mut logic = GameLogic::new();
    logic.add_player(Player::new(0, Team::USA, "P0", true));
    let mut pad = ThingTemplate::new("OrphanPad");
    pad.add_kind_of(KindOf::Structure).set_health(200.0);
    logic.templates.insert("OrphanPad".into(), pad);
    let mut dozer_tpl = ThingTemplate::new("OrphanDozer");
    dozer_tpl
        .add_kind_of(KindOf::Vehicle)
        .add_kind_of(KindOf::Dozer)
        .set_health(200.0);
    logic.templates.insert("OrphanDozer".into(), dozer_tpl);
    let pad_id = logic
        .create_object_for_player("OrphanPad", 0, Vec3::new(80.0, 0.0, 0.0))
        .expect("pad");
    let dozer = logic
        .create_object_for_player("OrphanDozer", 0, Vec3::ZERO)
        .expect("dozer");
    {
        let building = logic.host_object_mut(pad_id).expect("pad");
        building.health.current = 50.0;
        building.set_status_under_construction(false);
        building.construction_percent = 1.0;
    }
    {
        let unit = logic.host_object_mut(dozer).expect("dozer");
        unit.set_ai_state(AIState::Repairing);
        unit.dozer_task_repair_target = Some(pad_id);
        unit.ignored_obstacle_id = Some(pad_id);
        unit.movement.path = vec![Vec3::new(40.0, 0.0, 0.0)];
        unit.set_status_moving(true);
    }
    logic.update_support_states_for_test(&[dozer], 1.0 / 30.0);
    let unit = logic.host_object(dozer).expect("dozer");
    assert_eq!(unit.ai_state, AIState::Idle);
    assert!(unit.dozer_task_repair_target.is_none());
    assert!(unit.ignored_obstacle_id.is_none());
    assert!(unit.movement.path.is_empty());
    assert!(unit.target.is_none());
}

#[test]
fn seeker_without_a_pad_goes_idle() {
    use crate::game_logic::{KindOf, Player, ThingTemplate};
    let mut logic = GameLogic::new();
    logic.add_player(Player::new(0, Team::USA, "P0", true));
    let mut tpl = ThingTemplate::new("SeekTank");
    tpl.add_kind_of(KindOf::Vehicle).set_health(100.0);
    logic.templates.insert("SeekTank".into(), tpl);
    let id = logic
        .create_object_for_player("SeekTank", 0, Vec3::ZERO)
        .expect("tank");
    {
        let unit = logic.host_object_mut(id).expect("tank");
        unit.set_ai_state(AIState::SeekingRepair);
        unit.ignored_obstacle_id = Some(id);
        unit.movement.path = vec![Vec3::new(40.0, 0.0, 0.0)];
        unit.set_status_moving(true);
        unit.health.current = 50.0;
    }
    logic.update_support_states_for_test(&[id], 1.0 / 30.0);
    let unit = logic.host_object(id).expect("tank");
    assert_eq!(unit.ai_state, AIState::Idle);
    assert!(unit.ignored_obstacle_id.is_none());
    assert!(unit.movement.path.is_empty());
    assert!(unit.target.is_none());
}

#[test]
fn full_vehicle_leaves_the_repair_pad_for_its_rally() {
    use crate::game_logic::{BuildingData, BuildingType, KindOf, Player, ThingTemplate};
    let mut logic = GameLogic::new();
    logic.add_player(Player::new(0, Team::USA, "P0", true));
    let mut pad_t = ThingTemplate::new("RallyPad");
    pad_t
        .add_kind_of(KindOf::Structure)
        .add_kind_of(KindOf::RepairPad)
        .set_health(500.0);
    logic.templates.insert("RallyPad".into(), pad_t);
    let mut tank_t = ThingTemplate::new("FixedTank");
    tank_t.add_kind_of(KindOf::Vehicle).set_health(100.0);
    logic.templates.insert("FixedTank".into(), tank_t);
    let pad = logic
        .create_object_for_player("RallyPad", 0, Vec3::ZERO)
        .expect("pad");
    let tank = logic
        .create_object_for_player("FixedTank", 0, Vec3::ZERO)
        .expect("tank");
    let rally = Vec3::new(80.0, 0.0, 0.0);
    {
        let building = logic.host_object_mut(pad).expect("pad");
        let mut data = BuildingData::new(BuildingType::PowerPlant);
        data.rally_point = Some(rally);
        building.building_data = Some(data);
        building.set_status_under_construction(false);
        building.construction_percent = 1.0;
    }
    {
        let unit = logic.host_object_mut(tank).expect("tank");
        // C++ requestPath requires an active locomotor with nonzero valid
        // surfaces; synthetic vehicles default to an immobile fixture.
        unit.cur_locomotor_name = Some("GattlingTankLocomotor".into());
        unit.locomotor_surfaces = crate::game_logic::object::LOCO_SURFACE_GROUND;
        unit.set_order_target(Some(pad));
        unit.set_ai_state(AIState::SeekingRepair);
        unit.health.current = unit.health.maximum;
    }
    {
        let building = logic.host_object(pad).expect("pad");
        assert!(building.is_kind_of(KindOf::RepairPad), "pad kind missing");
        assert_eq!(
            building.building_data.as_ref().and_then(|b| b.rally_point),
            Some(rally)
        );
        let unit = logic.host_object(tank).expect("tank");
        assert_eq!(unit.target, Some(pad));
        assert!(unit.can_move(), "tank cannot move");
        assert!(
            unit.health.current >= unit.health.maximum - 0.01,
            "hp {} / {}",
            unit.health.current,
            unit.health.maximum
        );
    }
    logic.update_support_states_for_test(&[tank], 1.0 / 30.0);
    let unit = logic.host_object(tank).expect("tank");
    assert!(
        matches!(unit.ai_state, AIState::Moving | AIState::AttackMoving),
        "left for rally in {:?}, path={:?}",
        unit.ai_state,
        unit.movement.path
    );
    let end = unit.movement.path.last().copied().expect("rally path");
    let rally_cell = logic.pathfinding_system.grid.world_to_grid(rally);
    let cpp_adjusted_rally = logic
        .pathfinding_system
        .grid
        .adjust_coord_to_cell(rally_cell, true);
    assert!(
        logic.pathfinding_system.grid.world_to_grid(end) == rally_cell
            && end.distance(cpp_adjusted_rally) < 0.1,
        "ordinary ground path ends at the CPP-adjusted rally cell, end={end:?} expected={cpp_adjusted_rally:?}"
    );
    assert_eq!(unit.ignored_obstacle_id, Some(pad));
}

#[test]
fn passthrough_rally_snapshots_the_pad() {
    use crate::game_logic::{BuildingData, BuildingType, KindOf, Player, ThingTemplate};
    let mut logic = GameLogic::new();
    logic.add_player(Player::new(0, Team::USA, "P0", true));
    let mut pad_t = ThingTemplate::new("SnapPad");
    pad_t
        .add_kind_of(KindOf::Structure)
        .add_kind_of(KindOf::RepairPad)
        .set_health(500.0);
    logic.templates.insert("SnapPad".into(), pad_t);
    let mut tank_t = ThingTemplate::new("SnapTank");
    tank_t.add_kind_of(KindOf::Vehicle).set_health(100.0);
    logic.templates.insert("SnapTank".into(), tank_t);
    let pad = logic
        .create_object_for_player("SnapPad", 0, Vec3::ZERO)
        .expect("pad");
    let tank = logic
        .create_object_for_player("SnapTank", 0, Vec3::ZERO)
        .expect("tank");
    {
        let building = logic.host_object_mut(pad).expect("pad");
        let mut data = BuildingData::new(BuildingType::PowerPlant);
        data.rally_point = Some(Vec3::new(80.0, 0.0, 0.0));
        building.building_data = Some(data);
        building.set_status_under_construction(false);
        building.construction_percent = 1.0;
    }
    {
        let unit = logic.host_object_mut(tank).expect("tank");
        // C++ requestPath requires an active locomotor with nonzero valid
        // surfaces; synthetic vehicles default to an immobile fixture.
        unit.cur_locomotor_name = Some("GattlingTankLocomotor".into());
        unit.locomotor_surfaces = crate::game_logic::object::LOCO_SURFACE_GROUND;
        unit.set_order_target(Some(pad));
        unit.set_ai_state(AIState::SeekingRepair);
        unit.health.current = unit.health.maximum;
    }
    logic.force_map_loaded_for_path_test(true);
    logic.update_support_states_for_test(&[tank], 1.0 / 30.0);
    let queued_ignore: Vec<_> = logic
        .pathfinding_system
        .pending_paths()
        .map(|p| p.ignore_obstacle)
        .collect();
    assert_eq!(queued_ignore, vec![Some(pad)]);
}

#[test]
fn contact_attack_ignores_the_victim() {
    use crate::game_logic::{KindOf, Player, ThingTemplate, Weapon};
    let mut logic = GameLogic::new();
    logic.add_player(Player::new(0, Team::USA, "P0", true));
    logic.add_player(Player::new(1, Team::GLA, "P1", false));
    let mut inf_t = ThingTemplate::new("ContactInf");
    inf_t.add_kind_of(KindOf::Infantry).set_health(100.0);
    logic.templates.insert("ContactInf".into(), inf_t);
    let mut bld_t = ThingTemplate::new("ContactBuilding");
    bld_t
        .add_kind_of(KindOf::Structure)
        .add_kind_of(KindOf::Attackable)
        .set_health(500.0);
    logic.templates.insert("ContactBuilding".into(), bld_t);
    let building = logic
        .create_object_for_player("ContactBuilding", 1, Vec3::new(120.0, 0.0, 0.0))
        .expect("building");
    logic
        .host_object_mut(building)
        .expect("building")
        .selection_radius = 25.0;
    let attacker = logic
        .create_object_for_player("ContactInf", 0, Vec3::ZERO)
        .expect("attacker");
    {
        let unit = logic.host_object_mut(attacker).expect("attacker");
        // C++ attack-path requests use the attacker's active locomotor
        // surfaces; a newly-created test object otherwise has mask zero.
        unit.cur_locomotor_name = Some("BasicHumanLocomotor".into());
        unit.locomotor_surfaces = crate::game_logic::object::LOCO_SURFACE_GROUND;
        unit.weapon = Some(Weapon {
            damage: 10.0,
            range: 5.0,
            min_range: 0.0,
            reload_time: 0.1,
            last_fire_time: -10.0,
            ammo: None,
            clip_size: 0,
            clip_reload_time: 0.0,
            can_target_air: false,
            can_target_ground: true,
            projectile_speed: 0.0,
            pre_attack_delay: 0.0,
            splash_radius: 0.0,
            suspend_fx_frame: 0,
            reloading_clip: false,
            last_bonus_rof: 0.0,
        });
    }
    logic
        .pathfinding_system
        .apply_structure_static_blocks(&logic.objects);
    logic
        .pathfinding_system
        .grid
        .update_dynamic_obstacles(&logic.objects);
    let target_cell = logic
        .pathfinding_system
        .grid
        .world_to_grid(Vec3::new(120.0, 0.0, 0.0));
    assert_eq!(
        logic.pathfinding_system.grid.cell_type(target_cell),
        gamelogic::ai::pathfind_astar::PathfindCellType::Obstacle,
        "fixture must exercise the victim's actual obstacle footprint"
    );
    logic.force_map_loaded_for_path_test(true);
    assert!(logic.assign_unit_attack_path(attacker, Some(building), Vec3::new(120.0, 0.0, 0.0)));
    logic.process_pathfind_queue();
    let unit = logic.host_object(attacker).expect("attacker");
    assert_eq!(unit.ignored_obstacle_id, Some(building));
    let enters_victim_footprint = unit.movement.path.iter().any(|wp| {
        let cell = logic.pathfinding_system.grid.world_to_grid(*wp);
        logic.pathfinding_system.grid.cell_type(cell)
            == gamelogic::ai::pathfind_astar::PathfindCellType::Obstacle
    });
    assert!(
        enters_victim_footprint,
        "contact attack must route into the victim's real obstacle footprint, path={:?}",
        unit.movement.path
    );
    assert!(
        unit.movement
            .path
            .last()
            .is_some_and(|last| last.distance(Vec3::new(120.0, 0.0, 0.0)) < 0.1),
        "contact attack should jam the final node to the victim position: {:?}",
        unit.movement.path
    );
    assert_eq!(unit.path_extra_distance, 100.0);
}

#[test]
fn ranged_attack_does_not_keep_a_stale_ignore() {
    use crate::game_logic::pathfinding::GridPos;
    use crate::game_logic::{KindOf, Player, ThingTemplate, Weapon};
    let mut logic = GameLogic::new();
    logic.add_player(Player::new(0, Team::USA, "P0", true));
    logic.add_player(Player::new(1, Team::GLA, "P1", false));
    let mut inf_t = ThingTemplate::new("RangedInf");
    inf_t.add_kind_of(KindOf::Infantry).set_health(100.0);
    logic.templates.insert("RangedInf".into(), inf_t);
    let mut bld_t = ThingTemplate::new("RangedBuilding");
    bld_t
        .add_kind_of(KindOf::Structure)
        .add_kind_of(KindOf::Attackable)
        .set_health(500.0);
    logic.templates.insert("RangedBuilding".into(), bld_t);
    let mut block_t = ThingTemplate::new("StaleBlocker");
    block_t.add_kind_of(KindOf::Structure).set_health(500.0);
    logic.templates.insert("StaleBlocker".into(), block_t);
    let building = logic
        .create_object_for_player("RangedBuilding", 1, Vec3::new(120.0, 0.0, 0.0))
        .expect("building");
    let blocker = logic
        .create_object_for_player("StaleBlocker", 1, Vec3::new(0.0, 0.0, 80.0))
        .expect("blocker");
    let attacker = logic
        .create_object_for_player("RangedInf", 0, Vec3::ZERO)
        .expect("attacker");
    {
        let unit = logic.host_object_mut(attacker).expect("attacker");
        unit.weapon = Some(Weapon {
            damage: 10.0,
            range: 80.0,
            min_range: 0.0,
            reload_time: 0.1,
            last_fire_time: -10.0,
            ammo: None,
            clip_size: 0,
            clip_reload_time: 0.0,
            can_target_air: false,
            can_target_ground: true,
            projectile_speed: 0.0,
            pre_attack_delay: 0.0,
            splash_radius: 0.0,
            suspend_fx_frame: 0,
            reloading_clip: false,
            last_bonus_rof: 0.0,
        });
    }
    let wall_x = {
        let grid = &logic.pathfinding_system.grid;
        let start = grid.world_to_grid(Vec3::ZERO);
        let goal = grid.world_to_grid(Vec3::new(120.0, 0.0, 0.0));
        (start.x + goal.x) / 2
    };
    let height = logic.pathfinding_system.grid.height();
    for y in 0..height {
        logic.pathfinding_system.grid.set_cell_obstacle_owned(
            GridPos::new(wall_x, y),
            false,
            false,
            blocker.0,
            None,
            None,
        );
    }
    logic.pathfinding_system.set_ignore_obstacle(Some(blocker));
    assert!(logic.assign_unit_attack_path(attacker, Some(building), Vec3::new(120.0, 0.0, 0.0)));
    assert_eq!(logic.pathfinding_system.ignore_obstacle(), None);
    let unit = logic.host_object(attacker).expect("attacker");
    assert_eq!(
        unit.ignored_obstacle_id, None,
        "firing scan should succeed; the fail fallback is not this case"
    );
    assert_eq!(unit.path_extra_distance, 0.0);
    let end = unit.movement.path.last().copied().expect("firing path");
    let end_x = logic.pathfinding_system.grid.world_to_grid(end).x;
    assert!(
        end_x < wall_x,
        "ranged firing path must stop short of the wall, end_x={end_x} wall={wall_x} path={:?}",
        unit.movement.path
    );
}

#[test]
fn intermediate_path_keeps_the_next_segment_distance() {
    use crate::game_logic::{KindOf, Player, ThingTemplate};
    let mut logic = GameLogic::new();
    logic.add_player(Player::new(0, Team::USA, "P0", true));
    let mut inf_t = ThingTemplate::new("PathExtraInf");
    inf_t.add_kind_of(KindOf::Infantry).set_health(100.0);
    logic.templates.insert("PathExtraInf".into(), inf_t);
    let id = logic
        .create_object_for_player("PathExtraInf", 0, Vec3::ZERO)
        .expect("unit");
    let unit = logic.host_object_mut(id).expect("unit");
    unit.movement.path = vec![
        Vec3::ZERO,
        Vec3::new(10.0, 0.0, 0.0),
        Vec3::new(10.0, 0.0, 30.0),
        Vec3::new(50.0, 0.0, 30.0),
    ];
    unit.movement.current_path_index = 1;
    unit.refresh_follow_path_extra_distance();
    assert!((unit.path_extra_distance - 70.0).abs() < 0.01);
    unit.path_extra_distance = 100.0;
    unit.is_attack_path = false;
    unit.refresh_follow_path_extra_distance();
    assert!((unit.path_extra_distance - 70.0).abs() < 0.01);
    unit.path_extra_distance = 100.0;
    unit.is_attack_path = true;
    unit.refresh_follow_path_extra_distance();
    assert!((unit.path_extra_distance - 100.0).abs() < 0.01);
}

#[test]
fn exact_waypoint_path_sums_five_links() {
    use crate::game_logic::{KindOf, Player, ThingTemplate};
    let mut logic = GameLogic::new();
    logic.add_player(Player::new(0, Team::USA, "P0", true));
    let mut inf_t = ThingTemplate::new("WayExtraInf");
    inf_t.add_kind_of(KindOf::Infantry).set_health(100.0);
    logic.templates.insert("WayExtraInf".into(), inf_t);
    let id = logic
        .create_object_for_player("WayExtraInf", 0, Vec3::ZERO)
        .expect("unit");
    let points: Vec<Vec3> = (0..7)
        .map(|i| Vec3::new(i as f32 * 10.0, 0.0, 0.0))
        .collect();
    let unit = logic.host_object_mut(id).expect("unit");
    unit.movement.path = points;
    unit.movement.current_path_index = 0;
    unit.is_exact_path = true;
    unit.is_attack_path = false;
    unit.path_extra_distance = 100.0;
    unit.refresh_follow_path_extra_distance();
    assert!((unit.path_extra_distance - 51.0).abs() < 0.01);
}

#[test]
fn non_final_hop_keeps_the_obstacle_corner() {
    use crate::game_logic::pathfinding::GridPos;
    use crate::game_logic::{KindOf, Player, ThingTemplate};
    let mut logic = GameLogic::new();
    logic.add_player(Player::new(0, Team::USA, "P0", true));
    let mut inf_t = ThingTemplate::new("HopInf");
    inf_t.add_kind_of(KindOf::Infantry).set_health(100.0);
    logic.templates.insert("HopInf".into(), inf_t);
    let grid = &logic.pathfinding_system.grid;
    let start_cell = grid.world_to_grid(Vec3::ZERO);
    let corner_cell = grid.world_to_grid(Vec3::new(80.0, 0.0, 0.0));
    let corner = grid.grid_to_world(corner_cell);
    let blocked_cell = GridPos::new((start_cell.x + corner_cell.x) / 2, start_cell.y);
    logic.pathfinding_system.grid.set_cell_obstacle_owned(
        blocked_cell,
        false,
        false,
        9,
        None,
        None,
    );
    let unit = logic
        .create_object_for_player("HopInf", 0, Vec3::ZERO)
        .expect("unit");
    {
        let unit = logic.host_object_mut(unit).expect("unit");
        // C++ requestPath asserts that a unit has at least one valid locomotor
        // surface before invoking Pathfinder::findPath.
        unit.cur_locomotor_name = Some("BasicHumanLocomotor".into());
        unit.locomotor_surfaces = crate::game_logic::object::LOCO_SURFACE_GROUND;
    }
    logic.force_map_loaded_for_path_test(true);
    let ok = logic.assign_unit_path(unit, Vec3::new(160.0, 0.0, 0.0), &[corner]);
    logic.process_pathfind_queue();
    let path = logic.host_object(unit).expect("unit").movement.path.clone();
    let keeps_corner = path
        .iter()
        .any(|wp| logic.pathfinding_system.grid.world_to_grid(*wp) == corner_cell);
    let crosses_blocked_cell = path.windows(2).any(|segment| {
        !logic.pathfinding_system.line_passable_for_surfaces(
            segment[0],
            segment[1],
            crate::game_logic::object::LOCO_SURFACE_GROUND,
        )
    });
    let blocked_segments: Vec<_> = path
        .windows(2)
        .filter(|segment| {
            !logic.pathfinding_system.line_passable_for_surfaces(
                segment[0],
                segment[1],
                crate::game_logic::object::LOCO_SURFACE_GROUND,
            )
        })
        .map(|segment| {
            (
                segment[0],
                segment[1],
                logic.pathfinding_system.grid.world_to_grid(segment[0]),
                logic.pathfinding_system.grid.world_to_grid(segment[1]),
            )
        })
        .collect();
    let detours_around_blocker = path
        .iter()
        .any(|wp| logic.pathfinding_system.grid.world_to_grid(*wp).y != start_cell.y);
    assert!(
        ok && keeps_corner && detours_around_blocker && !crosses_blocked_cell,
        "the non-final hop must keep its reachable corner while routing around the blocker, \
         ok={ok} path={path:?} corner={corner:?} blocked={blocked_cell:?} \
         unpassable_segments={blocked_segments:?}"
    );
}

#[test]
fn live_corner_refreshes_the_extra_distance() {
    use crate::game_logic::{KindOf, Player, ThingTemplate};
    let mut logic = GameLogic::new();
    logic.add_player(Player::new(0, Team::USA, "P0", true));
    let mut inf_t = ThingTemplate::new("LiveExtraInf");
    inf_t.add_kind_of(KindOf::Infantry).set_health(100.0);
    logic.templates.insert("LiveExtraInf".into(), inf_t);
    let id = logic
        .create_object_for_player("LiveExtraInf", 0, Vec3::new(40.0, 0.0, 0.0))
        .expect("unit");
    {
        let unit = logic.host_object_mut(id).expect("unit");
        unit.movement.path = vec![
            Vec3::ZERO,
            Vec3::new(10.0, 0.0, 0.0),
            Vec3::new(50.0, 0.0, 0.0),
            Vec3::new(90.0, 0.0, 0.0),
            Vec3::new(130.0, 0.0, 0.0),
        ];
        unit.movement.current_path_index = 1;
        unit.movement.target_position = Some(Vec3::new(10.0, 0.0, 0.0));
        unit.movement.max_speed = 0.0;
        unit.path_extra_distance = 0.0;
        unit.set_status_moving(true);
        unit.set_ai_state(crate::game_logic::AIState::Moving);
    }
    logic.update_movement_for_test(&[id], 1.0 / 30.0);
    let unit = logic.host_object(id).expect("unit");
    assert_eq!(unit.movement.current_path_index, 2);
    assert!(
        (unit.path_extra_distance - 80.0).abs() < 0.01,
        "live corner must keep the next segment plus the one after, extra={}",
        unit.path_extra_distance
    );
}

#[test]
fn click_move_sets_follow_path_extra_distance() {
    use crate::game_logic::{KindOf, Player, ThingTemplate};
    let mut logic = GameLogic::new();
    logic.add_player(Player::new(0, Team::USA, "P0", true));
    let mut inf_t = ThingTemplate::new("ClickExtraInf");
    inf_t.add_kind_of(KindOf::Infantry).set_health(100.0);
    logic.templates.insert("ClickExtraInf".into(), inf_t);
    let id = logic
        .create_object_for_player("ClickExtraInf", 0, Vec3::ZERO)
        .expect("unit");
    {
        let unit = logic.host_object_mut(id).expect("unit");
        unit.path_extra_distance = 100.0;
        unit.is_attack_path = true;
        unit.is_exact_path = true;
    }
    logic.force_map_loaded_for_path_test(true);
    logic.move_object_with_pathfinding_for_test(id, Vec3::new(200.0, 0.0, 0.0), None);
    let unit = logic.host_object(id).expect("unit");
    assert!(!unit.is_attack_path, "a click move is not a contact attack");
    assert!(
        !unit.is_exact_path,
        "a click move is not an exact waypoint path"
    );
    let path = &unit.movement.path;
    let index = unit.movement.current_path_index;
    let expected = if index + 1 < path.len() {
        let goal = path[index];
        let next = path[index + 1];
        let dx = next.x - goal.x;
        let dz = next.z - goal.z;
        let mut seg = (dx * dx + dz * dz).sqrt();
        if index + 2 < path.len() {
            seg += 40.0;
        }
        seg
    } else {
        0.0
    };
    assert!(
        (unit.path_extra_distance - expected).abs() < 0.01,
        "click move must replace a leftover contact 100, extra={} expected={expected} path={path:?}",
        unit.path_extra_distance
    );
}

#[test]
fn aircraft_quick_path_restores_a_false_adjust_flag() {
    use crate::game_logic::{KindOf, Player, ThingTemplate};
    let mut logic = GameLogic::new();
    logic.add_player(Player::new(0, Team::USA, "P0", true));
    let mut jet_t = ThingTemplate::new("AdjustJet");
    jet_t.add_kind_of(KindOf::Aircraft).set_health(100.0);
    logic.templates.insert("AdjustJet".into(), jet_t);
    let id = logic
        .create_object_for_player("AdjustJet", 0, Vec3::new(0.0, 30.0, 0.0))
        .expect("jet");
    {
        let jet = logic.host_object_mut(id).expect("jet");
        jet.locomotor_surfaces = crate::game_logic::object::LOCO_SURFACE_AIR;
        jet.loco_appearance = crate::game_logic::LocomotorAppearance::Wings;
    }
    logic.pathfinding_system.set_adjust_goal(false);
    assert!(logic.assign_unit_path_for_test(id, Vec3::new(80.0, 30.0, 0.0), &[]));
    assert!(
        !logic.pathfinding_system.adjusts_goal(),
        "aircraft quick path must restore the caller's false adjust flag"
    );
}

#[test]
fn queued_waypoint_drops_the_exact_path_formula() {
    use crate::game_logic::{KindOf, Player, ThingTemplate};
    let mut logic = GameLogic::new();
    logic.add_player(Player::new(0, Team::USA, "P0", true));
    let mut inf_t = ThingTemplate::new("QueueExtraInf");
    inf_t.add_kind_of(KindOf::Infantry).set_health(100.0);
    logic.templates.insert("QueueExtraInf".into(), inf_t);
    let id = logic
        .create_object_for_player("QueueExtraInf", 0, Vec3::ZERO)
        .expect("unit");
    {
        let unit = logic.host_object_mut(id).expect("unit");
        unit.movement.path = vec![Vec3::ZERO, Vec3::new(40.0, 0.0, 0.0)];
        unit.movement.current_path_index = 1;
        unit.is_exact_path = true;
        unit.path_extra_distance = 100.0;
        unit.set_status_moving(true);
    }
    logic.force_map_loaded_for_path_test(true);
    assert!(logic.append_unit_waypoint_for_test(id, Vec3::new(120.0, 0.0, 0.0)));
    let unit = logic.host_object(id).expect("unit");
    assert!(!unit.is_exact_path);
    let path = &unit.movement.path;
    let index = unit.movement.current_path_index;
    let expected = if index + 1 < path.len() {
        let goal = path[index];
        let next = path[index + 1];
        let dx = next.x - goal.x;
        let dz = next.z - goal.z;
        let mut seg = (dx * dx + dz * dz).sqrt();
        if index + 2 < path.len() {
            seg += 40.0;
        }
        seg
    } else {
        0.0
    };
    assert!(
        (unit.path_extra_distance - expected).abs() < 0.01,
        "queued waypoint must use the follow-path distance, extra={} expected={expected} path={path:?}",
        unit.path_extra_distance
    );
}

#[test]
fn residual_path_installs_drop_the_exact_path_formula() {
    use crate::game_logic::{KindOf, Player, ThingTemplate};
    let mut logic = GameLogic::new();
    logic.add_player(Player::new(0, Team::USA, "P0", true));
    let mut inf_t = ThingTemplate::new("ResidualExtraInf");
    inf_t.add_kind_of(KindOf::Infantry).set_health(100.0);
    logic.templates.insert("ResidualExtraInf".into(), inf_t);
    let id = logic
        .create_object_for_player("ResidualExtraInf", 0, Vec3::ZERO)
        .expect("unit");
    {
        let unit = logic.host_object_mut(id).expect("unit");
        unit.is_exact_path = true;
        unit.path_extra_distance = 100.0;
        unit.request_path(
            Vec3::new(30.0, 0.0, 0.0),
            Some(vec![
                Vec3::ZERO,
                Vec3::new(10.0, 0.0, 0.0),
                Vec3::new(30.0, 0.0, 0.0),
            ]),
        );
        assert!(!unit.is_exact_path);
        assert_eq!(unit.movement.current_path_index, 0);
        assert_eq!(
            unit.movement.target_position,
            Some(Vec3::new(10.0, 0.0, 0.0))
        );
        assert!((unit.path_extra_distance - 50.0).abs() < 0.01);
    }
    logic.update_movement_for_test(&[id], 1.0 / 30.0);
    {
        let unit = logic.host_object(id).expect("unit");
        assert_eq!(unit.movement.current_path_index, 1);
        assert_eq!(
            unit.movement.target_position,
            Some(Vec3::new(10.0, 0.0, 0.0))
        );
        assert!(
            unit.get_position().x >= 0.0,
            "movement did not jump behind the path start"
        );
    }
    {
        let unit = logic.host_object_mut(id).expect("unit");
        unit.is_exact_path = true;
        unit.path_extra_distance = 100.0;
        unit.apply_move_away_path(
            id,
            &[
                Vec3::ZERO,
                Vec3::new(20.0, 0.0, 0.0),
                Vec3::new(50.0, 0.0, 0.0),
            ],
        );
        assert!(!unit.is_exact_path);
        assert!((unit.path_extra_distance - 30.0).abs() < 0.01);
    }
}

#[test]
fn projectile_precise_z_starts_on_the_last_segment() {
    use crate::game_logic::{KindOf, Player, ThingTemplate};
    let mut logic = GameLogic::new();
    logic.add_player(Player::new(0, Team::USA, "P0", true));
    let mut shell_t = ThingTemplate::new("Shell");
    shell_t.add_kind_of(KindOf::Projectile).set_health(1.0);
    logic.templates.insert("Shell".into(), shell_t);
    let id = logic
        .create_object_for_player("Shell", 0, Vec3::ZERO)
        .expect("shell");
    {
        let shell = logic.host_object_mut(id).expect("shell");
        shell.movement.path = vec![
            Vec3::ZERO,
            Vec3::new(10.0, 0.0, 0.0),
            Vec3::new(30.0, 0.0, 0.0),
        ];
        shell.movement.current_path_index = 1;
        shell.refresh_follow_path_extra_distance();
        assert!(!shell.precise_z_pos);
        shell.movement.current_path_index = 2;
        shell.refresh_follow_path_extra_distance();
        assert!(shell.precise_z_pos);
        shell.stop_moving();
        assert!(!shell.precise_z_pos);
    }
}

#[test]
fn extra_distance_uses_the_easy_goal_metric() {
    use crate::game_logic::{KindOf, Player, ThingTemplate};
    let mut logic = GameLogic::new();
    logic.add_player(Player::new(0, Team::USA, "P0", true));
    let mut inf_t = ThingTemplate::new("EasyDistInf");
    inf_t.add_kind_of(KindOf::Infantry).set_health(100.0);
    logic.templates.insert("EasyDistInf".into(), inf_t);
    let id = logic
        .create_object_for_player("EasyDistInf", 0, Vec3::ZERO)
        .expect("unit");
    let here = Vec3::ZERO;
    let middle = Vec3::new(20.0, 0.0, 0.0);
    let last = Vec3::new(20.0, 0.0, 20.0);
    {
        let unit = logic.host_object_mut(id).expect("unit");
        unit.cur_locomotor_name = Some("BasicHumanLocomotor".into());
        unit.movement.path = vec![here, middle, last];
        unit.movement.current_path_index = 1;
        unit.path_extra_distance = 0.0;
    }
    logic.register_ground_path_goal(id, last);
    let unit = logic.host_object_mut(id).expect("unit");
    let strict = unit.host_locomotor_distance_to_goal(here, last);
    unit.path_extra_distance = 80.0;
    let easy = unit.host_locomotor_distance_to_goal(here, last);
    assert!(
        (strict - 40.0).abs() < 0.01,
        "ground locomotion measures the routed path to the last node, got {strict}"
    );
    assert!(
        easy < strict,
        "extra distance must be the shorter flight to the last node, strict={strict} easy={easy}"
    );
}

#[test]
fn attack_path_request_preserves_the_exact_path_flag() {
    use crate::game_logic::{KindOf, Player, ThingTemplate};
    let mut logic = GameLogic::new();
    logic.add_player(Player::new(0, Team::USA, "P0", true));
    let mut inf_t = ThingTemplate::new("AttackReqInf");
    inf_t.add_kind_of(KindOf::Infantry).set_health(100.0);
    logic.templates.insert("AttackReqInf".into(), inf_t);
    let id = logic
        .create_object_for_player("AttackReqInf", 0, Vec3::ZERO)
        .expect("unit");
    let unit = logic.host_object_mut(id).expect("unit");
    unit.is_exact_path = true;
    assert!(unit.begin_request_attack_path(None, Vec3::new(40.0, 0.0, 0.0), 10));
    assert!(unit.is_attack_path);
    assert!(
        unit.is_exact_path,
        "CPP requestAttackPath changes attack-path state but preserves the exact-path flag"
    );
}

#[test]
fn saved_object_keeps_path_extra_distance() {
    use crate::game_logic::{KindOf, Player, ThingTemplate};
    let mut logic = GameLogic::new();
    logic.add_player(Player::new(0, Team::USA, "P0", true));
    let mut inf_t = ThingTemplate::new("SaveExtraInf");
    inf_t.add_kind_of(KindOf::Infantry).set_health(100.0);
    logic.templates.insert("SaveExtraInf".into(), inf_t);
    let id = logic
        .create_object_for_player("SaveExtraInf", 0, Vec3::ZERO)
        .expect("unit");
    let envelope = {
        let unit = logic.host_object_mut(id).expect("unit");
        unit.path_extra_distance = 80.0;
        unit.is_exact_path = true;
        unit.is_attack_path = false;
        unit.entity_lifecycle_envelope()
    };
    {
        let unit = logic.host_object_mut(id).expect("unit");
        unit.path_extra_distance = 0.0;
        unit.is_exact_path = false;
        unit.is_attack_path = true;
        unit.entity_apply_lifecycle_envelope(&envelope)
            .expect("apply");
        assert!((unit.path_extra_distance - 80.0).abs() < 0.01);
        assert!(unit.is_exact_path);
        assert!(!unit.is_attack_path);
    }
}

#[test]
fn live_arrival_clears_the_ignored_obstacle() {
    use crate::game_logic::{KindOf, ObjectId, Player, ThingTemplate};
    let mut logic = GameLogic::new();
    logic.add_player(Player::new(0, Team::USA, "P0", true));
    let mut inf_t = ThingTemplate::new("ArriveInf");
    inf_t.add_kind_of(KindOf::Infantry).set_health(100.0);
    logic.templates.insert("ArriveInf".into(), inf_t);
    let goal = Vec3::new(10.0, 0.0, 0.0);
    let id = logic
        .create_object_for_player("ArriveInf", 0, goal)
        .expect("unit");
    {
        let unit = logic.host_object_mut(id).expect("unit");
        unit.movement.path = vec![Vec3::ZERO, goal];
        unit.movement.current_path_index = 1;
        unit.movement.target_position = Some(goal);
        unit.movement.max_speed = 0.0;
        unit.ignored_obstacle_id = Some(ObjectId(9));
        unit.queue_for_path_frames = 60;
        unit.set_status_moving(true);
        unit.set_ai_state(crate::game_logic::AIState::Moving);
    }
    logic.update_movement_for_test(&[id], 1.0 / 30.0);
    let unit = logic.host_object(id).expect("unit");
    assert!(unit.movement.path.is_empty() || !unit.status.moving);
    assert_eq!(unit.ignored_obstacle_id, None);
    assert_eq!(unit.queue_for_path_frames, 0);
}

#[test]
fn live_air_hold_clears_the_ignore_and_the_queue() {
    use crate::game_logic::{KindOf, LocomotorAppearance, ObjectId, Player, ThingTemplate};
    let mut logic = GameLogic::new();
    logic.add_player(Player::new(0, Team::USA, "P0", true));
    let mut jet_t = ThingTemplate::new("HoldJet");
    jet_t.add_kind_of(KindOf::Aircraft).set_health(100.0);
    logic.templates.insert("HoldJet".into(), jet_t);
    let goal = Vec3::new(10.0, 20.0, 0.0);
    let id = logic
        .create_object_for_player("HoldJet", 0, goal)
        .expect("jet");
    {
        let unit = logic.host_object_mut(id).expect("jet");
        unit.loco_appearance = LocomotorAppearance::Wings;
        unit.movement.path = vec![Vec3::ZERO, goal];
        unit.movement.current_path_index = 1;
        unit.movement.target_position = Some(goal);
        unit.movement.max_speed = 0.0;
        unit.ignored_obstacle_id = Some(ObjectId(9));
        unit.queue_for_path_frames = 60;
        unit.can_path_through_units = true;
        unit.set_precise_z_pos(true);
        unit.locomotor_goal_type = crate::game_logic::LocoGoalType::PositionOnPath;
        unit.set_status_moving(true);
        unit.set_ai_state(crate::game_logic::AIState::Moving);
    }
    logic.update_movement_for_test(&[id], 1.0 / 30.0);
    let unit = logic.host_object(id).expect("jet");
    assert!(unit.movement.path.is_empty());
    assert_eq!(unit.ignored_obstacle_id, None);
    assert_eq!(unit.queue_for_path_frames, 0);
    assert!(!unit.can_path_through_units);
    assert!(!unit.precise_z_pos);
    assert!(!unit.status.moving);
    assert_eq!(
        unit.locomotor_goal_type,
        crate::game_logic::LocoGoalType::None
    );
}

#[test]
fn far_arrival_snaps_the_current_cell_not_the_goal() {
    use crate::game_logic::pathfinding::GridPos;
    use crate::game_logic::{KindOf, Player, ThingTemplate};
    let mut logic = GameLogic::new();
    logic.add_player(Player::new(0, Team::USA, "P0", true));
    let mut inf_t = ThingTemplate::new("FarArrive");
    inf_t.add_kind_of(KindOf::Infantry).set_health(100.0);
    logic.templates.insert("FarArrive".into(), inf_t);
    let here = Vec3::ZERO;
    let goal = Vec3::new(20.0, 0.0, 0.0);
    let (_, center) = crate::game_logic::PathfindingGrid::radius_and_center(
        8.0,
        logic.pathfinding_system.grid.grid_size(),
    );
    let stand = logic
        .pathfinding_system
        .grid
        .cell_for_unit_position(here, center);
    logic.pathfinding_system.grid.set_cell_obstacle_owned(
        GridPos::new(stand.x, stand.y),
        false,
        false,
        9,
        None,
        None,
    );
    let id = logic
        .create_object_for_player("FarArrive", 0, here)
        .expect("unit");
    {
        let unit = logic.host_object_mut(id).expect("unit");
        unit.selection_radius = 8.0;
        unit.cur_locomotor_name = Some("BasicHumanLocomotor".into());
        unit.movement.path = vec![here, goal];
        unit.movement.current_path_index = 1;
        unit.movement.target_position = Some(goal);
        unit.movement.max_speed = 0.0;
        unit.close_enough_dist = Some(30.0);
        unit.final_position = Vec3::new(999.0, 0.0, 999.0);
        unit.do_final_position = true;
        unit.set_status_moving(true);
        unit.set_ai_state(crate::game_logic::AIState::Moving);
    }
    logic.register_ground_path_goal(id, goal);
    logic.update_movement_for_test(&[id], 1.0 / 30.0);
    let unit = logic.host_object(id).expect("unit");
    assert!(!unit.do_final_position);
    let planted = logic
        .pathfinding_system
        .grid
        .world_to_grid(unit.final_position);
    assert_eq!(
        planted, stand,
        "snapPosition stays on the blocked stand cell, planted={planted:?} stand={stand:?}"
    );
}

#[test]
fn close_arrival_plants_the_goal_cell_not_the_raw_point() {
    use crate::game_logic::pathfinding::GridPos;
    use crate::game_logic::{KindOf, Player, ThingTemplate};
    let mut logic = GameLogic::new();
    logic.add_player(Player::new(0, Team::USA, "P0", true));
    let mut inf_t = ThingTemplate::new("CloseArrive");
    inf_t.add_kind_of(KindOf::Infantry).set_health(100.0);
    logic.templates.insert("CloseArrive".into(), inf_t);
    let here = Vec3::ZERO;
    let raw = Vec3::new(2.0, 7.0, 1.0);
    let id = logic
        .create_object_for_player("CloseArrive", 0, here)
        .expect("unit");
    {
        let unit = logic.host_object_mut(id).expect("unit");
        unit.selection_radius = 8.0;
        unit.cur_locomotor_name = Some("BasicHumanLocomotor".into());
        unit.movement.path = vec![here, raw];
        unit.movement.current_path_index = 1;
        unit.movement.target_position = Some(raw);
        unit.movement.max_speed = 0.0;
        unit.close_enough_dist = Some(30.0);
        unit.final_position = Vec3::new(999.0, 0.0, 999.0);
        unit.do_final_position = true;
        unit.set_status_moving(true);
        unit.set_ai_state(crate::game_logic::AIState::Moving);
    }
    logic.register_ground_path_goal(id, raw);
    logic.update_movement_for_test(&[id], 1.0 / 30.0);
    let unit = logic.host_object(id).expect("unit");
    assert!(!unit.do_final_position);
    let grid = &logic.pathfinding_system.grid;
    let (_, center) = crate::game_logic::PathfindingGrid::radius_and_center(8.0, grid.grid_size());
    let registered_goal = GridPos::new(unit.pathfind_goal_cell.0, unit.pathfind_goal_cell.1);
    let expected = grid.adjust_coord_to_ground_cell(registered_goal, center);
    let planted = unit.final_position;
    assert!(
        (planted.x - expected.x).abs() < 0.01 && (planted.z - expected.z).abs() < 0.01,
        "close arrival must plant the goal cell, planted={planted:?} expected={expected:?} raw={raw:?}"
    );
    assert!(
        (planted.y - expected.y).abs() < 0.01,
        "keep the cell height, planted={planted:?} expected={expected:?}"
    );
    assert!(
        (planted.x - raw.x).abs() > 0.01 || (planted.z - raw.z).abs() > 0.01,
        "the raw waypoint is not the cell point"
    );
}

#[test]
fn repair_tick_that_fills_the_vehicle_leaves_for_the_rally() {
    use crate::game_logic::{BuildingData, BuildingType, KindOf, Player, ThingTemplate};
    let mut logic = GameLogic::new();
    logic.add_player(Player::new(0, Team::USA, "P0", true));
    let mut pad_t = ThingTemplate::new("FillPad");
    pad_t
        .add_kind_of(KindOf::Structure)
        .add_kind_of(KindOf::RepairPad)
        .set_health(500.0);
    logic.templates.insert("FillPad".into(), pad_t);
    let mut tank_t = ThingTemplate::new("FillTank");
    tank_t.add_kind_of(KindOf::Vehicle).set_health(100.0);
    logic.templates.insert("FillTank".into(), tank_t);
    let pad = logic
        .create_object_for_player("FillPad", 0, Vec3::ZERO)
        .expect("pad");
    let tank = logic
        .create_object_for_player("FillTank", 0, Vec3::ZERO)
        .expect("tank");
    let rally = Vec3::new(90.0, 0.0, 0.0);
    {
        let building = logic.host_object_mut(pad).expect("pad");
        let mut data = BuildingData::new(BuildingType::RepairPad);
        data.rally_point = Some(rally);
        building.building_data = Some(data);
        building.dock_active_docker = Some(tank);
        building.set_status_under_construction(false);
        building.construction_percent = 1.0;
    }
    {
        let unit = logic.host_object_mut(tank).expect("tank");
        unit.set_order_target(Some(pad));
        unit.set_ai_state(AIState::SeekingRepair);
        unit.health.current = unit.health.maximum - 1.0;
    }
    logic.update_support_states_for_test(&[tank], 30.0);
    let unit = logic.host_object(tank).expect("tank");
    assert!(
        unit.health.current >= unit.health.maximum - 0.01,
        "the tick must finish the heal, hp={}",
        unit.health.current
    );
    assert!(
        matches!(unit.ai_state, AIState::Moving | AIState::AttackMoving),
        "filled vehicle left in {:?}, path={:?}",
        unit.ai_state,
        unit.movement.path
    );
    let end = unit.movement.path.last().copied().expect("rally path");
    let rally_cell = logic.pathfinding_system.grid.world_to_grid(rally);
    let cpp_adjusted_rally = logic
        .pathfinding_system
        .grid
        .adjust_coord_to_cell(rally_cell, true);
    assert!(
        logic.pathfinding_system.grid.world_to_grid(end) == rally_cell
            && end.distance(cpp_adjusted_rally) < 0.1,
        "ordinary ground path ends at the CPP-adjusted rally cell, end={end:?} expected={cpp_adjusted_rally:?}"
    );
    assert_eq!(unit.ignored_obstacle_id, Some(pad));
}

#[test]
fn repair_tick_that_fills_without_a_rally_goes_idle() {
    use crate::game_logic::{KindOf, Player, ThingTemplate};
    let mut logic = GameLogic::new();
    logic.add_player(Player::new(0, Team::USA, "P0", true));
    let mut pad_t = ThingTemplate::new("NoRallyPad");
    pad_t
        .add_kind_of(KindOf::Structure)
        .add_kind_of(KindOf::RepairPad)
        .set_health(500.0);
    logic.templates.insert("NoRallyPad".into(), pad_t);
    let mut tank_t = ThingTemplate::new("NoRallyTank");
    tank_t.add_kind_of(KindOf::Vehicle).set_health(100.0);
    logic.templates.insert("NoRallyTank".into(), tank_t);
    let pad = logic
        .create_object_for_player("NoRallyPad", 0, Vec3::ZERO)
        .expect("pad");
    let tank = logic
        .create_object_for_player("NoRallyTank", 0, Vec3::ZERO)
        .expect("tank");
    {
        let building = logic.host_object_mut(pad).expect("pad");
        building.dock_active_docker = Some(tank);
        building.set_status_under_construction(false);
        building.construction_percent = 1.0;
    }
    {
        let unit = logic.host_object_mut(tank).expect("tank");
        unit.set_order_target(Some(pad));
        unit.set_ai_state(AIState::SeekingRepair);
        unit.health.current = unit.health.maximum - 1.0;
        unit.ignored_obstacle_id = Some(pad);
        unit.movement.path = vec![Vec3::new(10.0, 0.0, 0.0)];
    }
    logic.update_support_states_for_test(&[tank], 30.0);
    let unit = logic.host_object(tank).expect("tank");
    assert!(unit.health.current >= unit.health.maximum - 0.01);
    assert_eq!(unit.ai_state, AIState::Idle);
    assert!(unit.ignored_obstacle_id.is_none());
    assert!(unit.movement.path.is_empty());
}

#[test]
fn aborted_capture_clears_the_building_ignore() {
    use crate::game_logic::{KindOf, Player, ThingTemplate};
    let mut logic = GameLogic::new();
    logic.add_player(Player::new(0, Team::USA, "P0", true));
    let mut inf_t = ThingTemplate::new("Capturer");
    inf_t.add_kind_of(KindOf::Infantry).set_health(100.0);
    logic.templates.insert("Capturer".into(), inf_t);
    let mut bld_t = ThingTemplate::new("CaptureMe");
    bld_t.add_kind_of(KindOf::Structure).set_health(500.0);
    logic.templates.insert("CaptureMe".into(), bld_t);
    let building = logic
        .create_object_for_player("CaptureMe", 0, Vec3::new(40.0, 0.0, 0.0))
        .expect("building");
    let infantry = logic
        .create_object_for_player("Capturer", 0, Vec3::ZERO)
        .expect("infantry");
    {
        let unit = logic.host_object_mut(infantry).expect("infantry");
        unit.set_ai_state(AIState::Capturing);
        unit.ignored_obstacle_id = Some(building);
        unit.movement.path = vec![Vec3::new(20.0, 0.0, 0.0)];
        unit.set_status_moving(true);
    }
    logic.update_support_states_for_test(&[infantry], 1.0 / 30.0);
    let unit = logic.host_object(infantry).expect("infantry");
    assert_eq!(unit.ai_state, AIState::Idle);
    assert!(unit.ignored_obstacle_id.is_none());
    assert!(unit.movement.path.is_empty());
    assert!(unit.target.is_none());
}

#[test]
fn dropped_special_ability_clears_the_building_ignore() {
    use crate::game_logic::{KindOf, Player, ThingTemplate};
    let mut logic = GameLogic::new();
    logic.add_player(Player::new(0, Team::USA, "P0", true));
    let mut inf_t = ThingTemplate::new("Hacker");
    inf_t.add_kind_of(KindOf::Infantry).set_health(100.0);
    logic.templates.insert("Hacker".into(), inf_t);
    let mut bld_t = ThingTemplate::new("HackMe");
    bld_t.add_kind_of(KindOf::Structure).set_health(500.0);
    logic.templates.insert("HackMe".into(), bld_t);
    let building = logic
        .create_object_for_player("HackMe", 0, Vec3::new(30.0, 0.0, 0.0))
        .expect("building");
    let hacker = logic
        .create_object_for_player("Hacker", 0, Vec3::ZERO)
        .expect("hacker");
    {
        let unit = logic.host_object_mut(hacker).expect("hacker");
        unit.set_ai_state(AIState::SpecialAbility);
        unit.ignored_obstacle_id = Some(building);
        unit.movement.path = vec![Vec3::new(15.0, 0.0, 0.0)];
        unit.set_status_moving(true);
    }
    logic.update_support_states_for_test(&[hacker], 1.0 / 30.0);
    let unit = logic.host_object(hacker).expect("hacker");
    assert_eq!(unit.ai_state, AIState::Idle);
    assert!(unit.ignored_obstacle_id.is_none());
    assert!(unit.movement.path.is_empty());
}

#[test]
fn finished_hacker_channel_clears_the_building_ignore() {
    use crate::game_logic::{KindOf, PendingSpecialAbility, Player, ThingTemplate};
    let mut logic = GameLogic::new();
    logic.add_player(Player::new(0, Team::USA, "P0", true));
    let mut inf_t = ThingTemplate::new("HackerFinish");
    inf_t.add_kind_of(KindOf::Infantry).set_health(100.0);
    logic.templates.insert("HackerFinish".into(), inf_t);
    let mut bld_t = ThingTemplate::new("HackFinish");
    bld_t.add_kind_of(KindOf::Structure).set_health(500.0);
    logic.templates.insert("HackFinish".into(), bld_t);
    let building = logic
        .create_object_for_player("HackFinish", 0, Vec3::new(30.0, 0.0, 0.0))
        .expect("building");
    let hacker = logic
        .create_object_for_player("HackerFinish", 0, Vec3::ZERO)
        .expect("hacker");
    logic.pending_special_abilities.insert(
        hacker,
        PendingSpecialAbility::HackerDisableBuilding {
            target_id: building,
        },
    );
    {
        let unit = logic.host_object_mut(hacker).expect("hacker");
        unit.set_ai_state(AIState::SpecialAbility);
        unit.ignored_obstacle_id = Some(building);
        unit.movement.path = vec![Vec3::new(12.0, 0.0, 0.0)];
        unit.set_status_moving(true);
    }
    logic.update_support_states_for_test(&[hacker], 1.0 / 30.0);
    let unit = logic.host_object(hacker).expect("hacker");
    assert_eq!(unit.ai_state, AIState::Idle);
    assert!(unit.ignored_obstacle_id.is_none());
    assert!(unit.movement.path.is_empty());
    assert!(!logic.pending_special_abilities.contains_key(&hacker));
}

#[test]
fn queued_hacker_path_stays_special_ability() {
    use crate::game_logic::{KindOf, Player, ThingTemplate};
    let mut logic = GameLogic::new();
    logic.add_player(Player::new(0, Team::USA, "P0", true));
    let mut inf_t = ThingTemplate::new("HackerQueue");
    inf_t.add_kind_of(KindOf::Infantry).set_health(100.0);
    logic.templates.insert("HackerQueue".into(), inf_t);
    let mut bld_t = ThingTemplate::new("HackQueue");
    bld_t.add_kind_of(KindOf::Structure).set_health(500.0);
    logic.templates.insert("HackQueue".into(), bld_t);
    let building = logic
        .create_object_for_player("HackQueue", 0, Vec3::new(80.0, 0.0, 0.0))
        .expect("building");
    let hacker = logic
        .create_object_for_player("HackerQueue", 0, Vec3::ZERO)
        .expect("hacker");
    {
        let unit = logic.host_object_mut(hacker).expect("hacker");
        unit.set_ai_state(AIState::SpecialAbility);
        unit.ignored_obstacle_id = Some(building);
    }
    logic.force_map_loaded_for_path_test(true);
    assert!(logic.assign_unit_path_ignoring(
        hacker,
        Vec3::new(80.0, 0.0, 0.0),
        &[],
        Some(building),
    ));
    {
        let unit = logic.host_object(hacker).expect("hacker");
        assert_eq!(unit.ai_state, AIState::SpecialAbility);
        assert!(unit.waiting_for_path);
    }
    logic.process_pathfind_queue();
    let unit = logic.host_object(hacker).expect("hacker");
    assert_eq!(
        unit.ai_state,
        AIState::SpecialAbility,
        "installing the hacker path must not look like a new move order"
    );
    assert!(!unit.movement.path.is_empty() || unit.waiting_for_path);
    assert_eq!(unit.ignored_obstacle_id, Some(building));
}

#[test]
fn reissued_hacker_move_keeps_special_ability_and_the_building() {
    use crate::game_logic::pathfinding::GridPos;
    use crate::game_logic::{KindOf, Player, ThingTemplate};
    let mut logic = GameLogic::new();
    logic.add_player(Player::new(0, Team::USA, "P0", true));
    let mut inf_t = ThingTemplate::new("HackerReplay");
    inf_t.add_kind_of(KindOf::Infantry).set_health(100.0);
    logic.templates.insert("HackerReplay".into(), inf_t);
    let mut bld_t = ThingTemplate::new("HackReplay");
    bld_t.add_kind_of(KindOf::Structure).set_health(500.0);
    logic.templates.insert("HackReplay".into(), bld_t);
    let building = logic
        .create_object_for_player("HackReplay", 0, Vec3::new(90.0, 0.0, 0.0))
        .expect("building");
    let hacker = logic
        .create_object_for_player("HackerReplay", 0, Vec3::ZERO)
        .expect("hacker");
    {
        let unit = logic.host_object_mut(hacker).expect("hacker");
        unit.set_ai_state(AIState::SpecialAbility);
        unit.set_order_target(Some(building));
        unit.pending_move = Some(Vec3::new(90.0, 0.0, 0.0));
    }
    let wall_x = {
        let grid = &logic.pathfinding_system.grid;
        let start = grid.world_to_grid(Vec3::ZERO);
        let goal = grid.world_to_grid(Vec3::new(90.0, 0.0, 0.0));
        (start.x + goal.x) / 2
    };
    let height = logic.pathfinding_system.grid.height();
    for y in 0..height {
        logic.pathfinding_system.grid.set_cell_obstacle_owned(
            GridPos::new(wall_x, y),
            false,
            false,
            building.0,
            None,
            None,
        );
    }
    logic.force_map_loaded_for_path_test(true);
    logic.reissue_pending_moves();
    logic.process_pathfind_queue();
    let unit = logic.host_object(hacker).expect("hacker");
    assert_eq!(unit.ai_state, AIState::SpecialAbility);
    assert_eq!(unit.ignored_obstacle_id, Some(building));
    assert!(unit.pending_move.is_none());
    let xs: Vec<i32> = unit
        .movement
        .path
        .iter()
        .map(|wp| logic.pathfinding_system.grid.world_to_grid(*wp).x)
        .collect();
    let crosses = xs.windows(2).any(|w| {
        let lo = w[0].min(w[1]);
        let hi = w[0].max(w[1]);
        lo <= wall_x && wall_x <= hi
    });
    assert!(
        crosses,
        "replayed hacker path must ignore the building, xs={xs:?} wall={wall_x}"
    );
}

#[test]
fn reissued_attack_crosses_the_target_building() {
    use crate::game_logic::pathfinding::GridPos;
    use crate::game_logic::{KindOf, Player, ThingTemplate};
    let mut logic = GameLogic::new();
    logic.add_player(Player::new(0, Team::USA, "P0", true));
    logic.add_player(Player::new(1, Team::GLA, "P1", false));
    let mut inf_t = ThingTemplate::new("AttackReplay");
    inf_t.add_kind_of(KindOf::Infantry).set_health(100.0);
    logic.templates.insert("AttackReplay".into(), inf_t);
    let mut bld_t = ThingTemplate::new("AttackBuilding");
    bld_t.add_kind_of(KindOf::Structure).set_health(500.0);
    logic.templates.insert("AttackBuilding".into(), bld_t);
    let building = logic
        .create_object_for_player("AttackBuilding", 1, Vec3::new(90.0, 0.0, 0.0))
        .expect("building");
    let attacker = logic
        .create_object_for_player("AttackReplay", 0, Vec3::ZERO)
        .expect("attacker");
    {
        let unit = logic.host_object_mut(attacker).expect("attacker");
        unit.set_ai_state(AIState::Attacking);
        unit.set_order_target(Some(building));
        unit.pending_move = Some(Vec3::new(90.0, 0.0, 0.0));
    }
    let wall_x = {
        let grid = &logic.pathfinding_system.grid;
        let start = grid.world_to_grid(Vec3::ZERO);
        let goal = grid.world_to_grid(Vec3::new(90.0, 0.0, 0.0));
        (start.x + goal.x) / 2
    };
    let height = logic.pathfinding_system.grid.height();
    for y in 0..height {
        logic.pathfinding_system.grid.set_cell_obstacle_owned(
            GridPos::new(wall_x, y),
            false,
            false,
            building.0,
            None,
            None,
        );
    }
    logic.force_map_loaded_for_path_test(true);
    logic.reissue_pending_moves();
    logic.process_pathfind_queue();
    let unit = logic.host_object(attacker).expect("attacker");
    assert_eq!(unit.ai_state, AIState::Attacking);
    assert_eq!(unit.ignored_obstacle_id, Some(building));
    let xs: Vec<i32> = unit
        .movement
        .path
        .iter()
        .map(|wp| logic.pathfinding_system.grid.world_to_grid(*wp).x)
        .collect();
    let crosses = xs.windows(2).any(|w| {
        let lo = w[0].min(w[1]);
        let hi = w[0].max(w[1]);
        lo <= wall_x && wall_x <= hi
    });
    assert!(
        crosses,
        "replayed attack must ignore the target building, xs={xs:?} wall={wall_x}"
    );
}

#[test]
fn reissued_gather_paths_to_the_approach() {
    use crate::game_logic::{KindOf, Player, ThingTemplate};
    let mut logic = GameLogic::new();
    logic.add_player(Player::new(0, Team::USA, "P0", true));
    let mut truck_t = ThingTemplate::new("GatherReplay");
    truck_t.add_kind_of(KindOf::Vehicle).set_health(100.0);
    logic.templates.insert("GatherReplay".into(), truck_t);
    let mut wh_t = ThingTemplate::new("GatherWarehouse");
    wh_t.add_kind_of(KindOf::Structure).set_health(500.0);
    logic.templates.insert("GatherWarehouse".into(), wh_t);
    let warehouse = logic
        .create_object_for_player("GatherWarehouse", 0, Vec3::new(90.0, 0.0, 0.0))
        .expect("warehouse");
    let truck = logic
        .create_object_for_player("GatherReplay", 0, Vec3::ZERO)
        .expect("truck");
    logic
        .host_object_mut(truck)
        .expect("truck")
        .cur_locomotor_name = Some("SupplyTruckLocomotor".into());
    logic
        .host_object_mut(truck)
        .expect("truck")
        .locomotor_surfaces = crate::game_logic::object::LOCO_SURFACE_GROUND;
    let approach = Vec3::new(70.0, 0.0, 0.0);
    {
        let unit = logic.host_object_mut(truck).expect("truck");
        unit.set_ai_state(AIState::Gathering);
        unit.preferred_dock_id = Some(warehouse);
        unit.pending_move = Some(approach);
        // Xfer restores this request kind alongside the queued destination.
        unit.requested_destination = Some(approach);
        unit.is_final_goal = true;
        unit.is_approach_path = true;
    }
    logic.force_map_loaded_for_path_test(true);
    logic.reissue_pending_moves();
    logic.process_pathfind_queue();
    let unit = logic.host_object(truck).expect("truck");
    assert_eq!(unit.ai_state, AIState::Gathering);
    assert!(unit.ignored_obstacle_id.is_none());
    assert!(
        unit.is_approach_path,
        "reissued dock request keeps its saved mode"
    );
    assert_eq!(unit.requested_destination, Some(approach));
    let end = unit.movement.path.last().copied().expect("approach path");
    assert!(
        logic.pathfinding_system.grid.valid_movement_position(
            logic.pathfinding_system.grid.world_to_grid(end),
            gamelogic::ai::pathfind_astar::PathfindLayerEnum::Ground,
            unit.locomotor_surfaces,
            unit.crusher_level > 0,
            0,
        ) && end.distance(approach) < Vec3::ZERO.distance(approach),
        "replay must reach a legal path point closer to the saved dock approach, end={end:?}"
    );
}

#[test]
fn gather_click_paths_to_the_warehouse_approach() {
    use crate::game_logic::{DockKind, KindOf, Player, ThingTemplate};
    let mut logic = GameLogic::new();
    logic.add_player(Player::new(0, Team::USA, "P0", true));
    let mut truck_t = ThingTemplate::new("GatherClick");
    truck_t.add_kind_of(KindOf::Vehicle).set_health(100.0);
    logic.templates.insert("GatherClick".into(), truck_t);
    let mut wh_t = ThingTemplate::new("ClickWarehouse");
    wh_t.add_kind_of(KindOf::Structure).set_health(500.0);
    wh_t.dock_kind = DockKind::SupplyWarehouse;
    logic.templates.insert("ClickWarehouse".into(), wh_t);
    let warehouse_pos = Vec3::new(90.0, 0.0, 0.0);
    let warehouse = logic
        .create_object_for_player("ClickWarehouse", 0, warehouse_pos)
        .expect("warehouse");
    let truck = logic
        .create_object_for_player("GatherClick", 0, Vec3::ZERO)
        .expect("truck");
    logic
        .host_object_mut(truck)
        .expect("truck")
        .cur_locomotor_name = Some("SupplyTruckLocomotor".into());
    logic
        .host_object_mut(truck)
        .expect("truck")
        .locomotor_surfaces = crate::game_logic::object::LOCO_SURFACE_GROUND;
    logic
        .host_object_mut(warehouse)
        .expect("warehouse")
        .selection_radius = 40.0;
    logic.force_map_loaded_for_path_test(true);
    assert!(logic.unit_command_dock_at_supply_warehouse(truck, warehouse));
    logic.process_pathfind_queue();
    let unit = logic.host_object(truck).expect("truck");
    assert_eq!(unit.ai_state, AIState::Gathering);
    assert_reserved_dock_approach_path(&logic, warehouse, truck, Vec3::ZERO, warehouse_pos, 40.0);
}

#[test]
fn stunned_gather_click_holds_the_approach() {
    use crate::game_logic::{DockKind, KindOf, Player, ThingTemplate};
    let mut logic = GameLogic::new();
    logic.add_player(Player::new(0, Team::USA, "P0", true));
    let mut truck_t = ThingTemplate::new("StunnedGather");
    truck_t.add_kind_of(KindOf::Vehicle).set_health(100.0);
    logic.templates.insert("StunnedGather".into(), truck_t);
    let mut wh_t = ThingTemplate::new("StunnedWarehouse");
    wh_t.add_kind_of(KindOf::Structure).set_health(500.0);
    wh_t.dock_kind = DockKind::SupplyWarehouse;
    logic.templates.insert("StunnedWarehouse".into(), wh_t);
    let warehouse = logic
        .create_object_for_player("StunnedWarehouse", 0, Vec3::new(90.0, 0.0, 0.0))
        .expect("warehouse");
    let truck = logic
        .create_object_for_player("StunnedGather", 0, Vec3::ZERO)
        .expect("truck");
    logic
        .host_object_mut(warehouse)
        .expect("warehouse")
        .selection_radius = 40.0;
    logic
        .host_object_mut(truck)
        .expect("truck")
        .shock_stun_frames = 100;
    assert!(logic.unit_command_dock_at_supply_warehouse(truck, warehouse));
    let unit = logic.host_object(truck).expect("truck");
    let held = unit.pending_move.expect("held approach");
    assert!(
        held.distance(Vec3::new(70.0, 0.0, 0.0)) < 1.0,
        "a stunned gather must hold the approach, not the center, held={held:?}"
    );
    assert!(unit.ignored_obstacle_id.is_none());
    assert_eq!(unit.ai_state, AIState::Gathering);
}

#[test]
fn return_click_paths_to_the_supply_center_approach() {
    use crate::game_logic::{DockKind, KindOf, Player, ThingTemplate};
    let mut logic = GameLogic::new();
    logic.add_player(Player::new(0, Team::USA, "P0", true));
    let mut truck_t = ThingTemplate::new("ReturnClick");
    truck_t.add_kind_of(KindOf::Vehicle).set_health(100.0);
    logic.templates.insert("ReturnClick".into(), truck_t);
    let mut center_t = ThingTemplate::new("ReturnCenter");
    center_t
        .add_kind_of(KindOf::Structure)
        .add_kind_of(KindOf::SupplyCenter)
        .set_health(500.0);
    center_t.dock_kind = DockKind::SupplyCenter;
    logic.templates.insert("ReturnCenter".into(), center_t);
    let center = logic
        .create_object_for_player("ReturnCenter", 0, Vec3::new(90.0, 0.0, 0.0))
        .expect("center");
    let truck = logic
        .create_object_for_player("ReturnClick", 0, Vec3::ZERO)
        .expect("truck");
    logic
        .host_object_mut(truck)
        .expect("truck")
        .cur_locomotor_name = Some("SupplyTruckLocomotor".into());
    logic
        .host_object_mut(truck)
        .expect("truck")
        .locomotor_surfaces = crate::game_logic::object::LOCO_SURFACE_GROUND;
    {
        let pad = logic.host_object_mut(center).expect("center");
        pad.selection_radius = 40.0;
        pad.set_status_under_construction(false);
        pad.construction_percent = 1.0;
    }
    logic.force_map_loaded_for_path_test(true);
    assert!(logic.unit_command_return_supplies(truck, center));
    logic.process_pathfind_queue();
    let unit = logic.host_object(truck).expect("truck");
    assert_eq!(unit.ai_state, AIState::ReturningResources);
    assert_reserved_dock_approach_path(
        &logic,
        center,
        truck,
        Vec3::ZERO,
        Vec3::new(90.0, 0.0, 0.0),
        40.0,
    );
}

#[test]
fn stunned_return_click_holds_the_approach() {
    use crate::game_logic::{DockKind, KindOf, Player, ThingTemplate};
    let mut logic = GameLogic::new();
    logic.add_player(Player::new(0, Team::USA, "P0", true));
    let mut truck_t = ThingTemplate::new("StunnedReturn");
    truck_t.add_kind_of(KindOf::Vehicle).set_health(100.0);
    logic.templates.insert("StunnedReturn".into(), truck_t);
    let mut center_t = ThingTemplate::new("StunnedReturnCenter");
    center_t
        .add_kind_of(KindOf::Structure)
        .add_kind_of(KindOf::SupplyCenter)
        .set_health(500.0);
    center_t.dock_kind = DockKind::SupplyCenter;
    logic
        .templates
        .insert("StunnedReturnCenter".into(), center_t);
    let center = logic
        .create_object_for_player("StunnedReturnCenter", 0, Vec3::new(90.0, 0.0, 0.0))
        .expect("center");
    let truck = logic
        .create_object_for_player("StunnedReturn", 0, Vec3::ZERO)
        .expect("truck");
    {
        let pad = logic.host_object_mut(center).expect("center");
        pad.selection_radius = 40.0;
        pad.set_status_under_construction(false);
        pad.construction_percent = 1.0;
    }
    logic
        .host_object_mut(truck)
        .expect("truck")
        .shock_stun_frames = 100;
    assert!(logic.unit_command_return_supplies(truck, center));
    let unit = logic.host_object(truck).expect("truck");
    let held = unit.pending_move.expect("held approach");
    assert!(
        held.distance(Vec3::new(70.0, 0.0, 0.0)) < 1.0,
        "a stunned return must hold the approach, not the center, held={held:?}"
    );
    assert!(unit.ignored_obstacle_id.is_none());
    assert_eq!(unit.ai_state, AIState::ReturningResources);
}

#[test]
fn guard_object_adjusts_off_an_impassable_post() {
    use crate::game_logic::pathfinding::GridPos;
    use crate::game_logic::{GuardMode, KindOf, Player, ThingTemplate};
    use gamelogic::ai::pathfind_astar::PathfindCellType;
    let mut logic = GameLogic::new();
    logic.add_player(Player::new(0, Team::USA, "P0", true));
    let mut inf_t = ThingTemplate::new("GuardInf");
    inf_t.add_kind_of(KindOf::Infantry).set_health(100.0);
    logic.templates.insert("GuardInf".into(), inf_t);
    let mut bld_t = ThingTemplate::new("GuardPost");
    bld_t.add_kind_of(KindOf::Structure).set_health(500.0);
    logic.templates.insert("GuardPost".into(), bld_t);
    let post = Vec3::new(90.0, 0.0, 0.0);
    let building = logic
        .create_object_for_player("GuardPost", 0, post)
        .expect("post");
    let guard = logic
        .create_object_for_player("GuardInf", 0, Vec3::ZERO)
        .expect("guard");
    let blocked = logic.pathfinding_system.grid.world_to_grid(post);
    logic
        .pathfinding_system
        .grid
        .set_cell_type(blocked, PathfindCellType::Obstacle);
    let expected = logic
        .pathfinding_system
        .grid
        .adjust_destination(
            blocked,
            crate::game_logic::object::LOCO_SURFACE_GROUND,
            false,
            400,
        )
        .expect("adjustDestination cell");
    assert_ne!(expected, blocked);
    logic.force_map_loaded_for_path_test(true);
    assert!(logic.unit_command_guard_object(guard, building));
    logic.process_pathfind_queue();
    let unit = logic.host_object(guard).expect("guard");
    assert_eq!(unit.ai_state, AIState::GuardingObject);
    let end_cell = logic
        .pathfinding_system
        .grid
        .world_to_grid(unit.movement.path.last().copied().expect("guard path"));
    assert_eq!(
        end_cell, expected,
        "guard_object must use adjustDestination"
    );
    let guard2 = logic
        .create_object_for_player("GuardInf", 0, Vec3::new(0.0, 0.0, 20.0))
        .expect("guard2");
    let expected2 = logic
        .pathfinding_system
        .grid
        .adjust_destination(
            blocked,
            crate::game_logic::object::LOCO_SURFACE_GROUND,
            false,
            400,
        )
        .expect("second adjustDestination cell");
    assert!(logic.unit_command_guard_full(guard2, None, Some(building), 100.0, GuardMode::Normal,));
    logic.process_pathfind_queue();
    let unit2 = logic.host_object(guard2).expect("guard2");
    assert_eq!(unit2.ai_state, AIState::GuardingObject);
    let end2 = logic
        .pathfinding_system
        .grid
        .world_to_grid(unit2.movement.path.last().copied().expect("full path"));
    assert_eq!(end2, expected2, "guard_full must use adjustDestination");
    assert_ne!(end2, blocked);
}

#[test]
fn attack_exit_clears_the_victim_building_ignore() {
    use crate::game_logic::{KindOf, Player, ThingTemplate};
    let mut logic = GameLogic::new();
    logic.add_player(Player::new(0, Team::USA, "P0", true));
    logic.add_player(Player::new(1, Team::GLA, "P1", false));
    let mut inf_t = ThingTemplate::new("AttackExit");
    inf_t.add_kind_of(KindOf::Infantry).set_health(100.0);
    logic.templates.insert("AttackExit".into(), inf_t);
    let mut bld_t = ThingTemplate::new("AttackExitBuilding");
    bld_t.add_kind_of(KindOf::Structure).set_health(500.0);
    logic.templates.insert("AttackExitBuilding".into(), bld_t);
    let building = logic
        .create_object_for_player("AttackExitBuilding", 1, Vec3::new(40.0, 0.0, 0.0))
        .expect("building");
    let attacker = logic
        .create_object_for_player("AttackExit", 0, Vec3::ZERO)
        .expect("attacker");
    {
        let unit = logic.host_object_mut(attacker).expect("attacker");
        unit.set_ai_state(AIState::Attacking);
        unit.set_order_target(Some(building));
        unit.ignored_obstacle_id = Some(building);
    }
    logic
        .host_object_mut(building)
        .expect("building")
        .status
        .destroyed = true;
    logic.attack_state_exit(attacker);
    let unit = logic.host_object(attacker).expect("attacker");
    assert!(unit.ignored_obstacle_id.is_none());
    assert_eq!(unit.target, Some(building));
}

#[test]
fn live_attack_move_keeps_the_building_ignore() {
    use crate::game_logic::{KindOf, Player, ThingTemplate};
    let mut logic = GameLogic::new();
    logic.add_player(Player::new(0, Team::USA, "P0", true));
    logic.add_player(Player::new(1, Team::GLA, "P1", false));
    let mut inf_t = ThingTemplate::new("AttackMoveKeep");
    inf_t.add_kind_of(KindOf::Infantry).set_health(100.0);
    logic.templates.insert("AttackMoveKeep".into(), inf_t);
    let mut bld_t = ThingTemplate::new("LiveAttackBuilding");
    bld_t.add_kind_of(KindOf::Structure).set_health(500.0);
    logic.templates.insert("LiveAttackBuilding".into(), bld_t);
    let building = logic
        .create_object_for_player("LiveAttackBuilding", 1, Vec3::new(40.0, 0.0, 0.0))
        .expect("building");
    let attacker = logic
        .create_object_for_player("AttackMoveKeep", 0, Vec3::ZERO)
        .expect("attacker");
    {
        let unit = logic.host_object_mut(attacker).expect("attacker");
        unit.set_ai_state(AIState::AttackMoving);
        unit.set_order_target(Some(building));
        unit.ignored_obstacle_id = Some(building);
        unit.requested_destination = Some(Vec3::new(80.0, 0.0, 0.0));
        unit.movement.path = vec![Vec3::new(80.0, 0.0, 0.0)];
    }
    logic.attack_state_exit(attacker);
    let unit = logic.host_object(attacker).expect("attacker");
    assert_eq!(unit.ignored_obstacle_id, Some(building));
    assert_eq!(unit.movement.path, vec![Vec3::new(80.0, 0.0, 0.0)]);
}

#[test]
fn sole_tick_completion_walks_the_dozer_off_the_pad() {
    use crate::game_logic::{KindOf, Player, ThingTemplate, host_construction_ready_log};
    use crate::gameworld_shadow::{
        begin_shadow_coupled_tick, end_shadow_coupled_tick,
        gameworld_construction_sole_tick_enabled,
    };

    let _guard = crate::gameworld_shadow::authority_env_lock();
    let prev_sh = std::env::var_os("GENERALS_GAMEWORLD_SHADOW");
    crate::env_compat::set_var("GENERALS_GAMEWORLD_SHADOW", "1");
    host_construction_ready_log::clear();

    let mut logic = GameLogic::new();
    logic.add_player(Player::new(0, Team::USA, "P0", true));
    logic.set_construction_authority(true);
    let mut pad = ThingTemplate::new("SolePad");
    pad.add_kind_of(KindOf::Structure).set_health(1_000.0);
    logic.templates.insert("SolePad".into(), pad);
    let mut dozer_tpl = ThingTemplate::new("SoleDozer");
    dozer_tpl
        .add_kind_of(KindOf::Vehicle)
        .add_kind_of(KindOf::Dozer)
        .set_health(200.0);
    logic.templates.insert("SoleDozer".into(), dozer_tpl);
    let pad_id = logic
        .create_object_for_player("SolePad", 0, Vec3::ZERO)
        .expect("pad");
    let dozer = logic
        .create_object_for_player("SoleDozer", 0, Vec3::ZERO)
        .expect("dozer");
    logic.dozer_new_task_build(dozer, pad_id);
    let action = logic
        .host_object(dozer)
        .and_then(|d| d.dozer_dock_action)
        .expect("dozer_new_task_build stores the action dock");
    {
        let obj = logic.host_object_mut(pad_id).expect("pad");
        obj.set_status_under_construction(true);
        obj.construction_percent = 1.0;
        obj.builder_id = Some(dozer);
    }
    {
        let obj = logic.host_object_mut(dozer).expect("dozer");
        obj.set_position(action);
        obj.set_target(Some(pad_id));
        obj.set_ai_state(AIState::Constructing);
        obj.movement.path.clear();
        obj.waiting_for_path = false;
    }

    begin_shadow_coupled_tick();
    assert!(gameworld_construction_sole_tick_enabled());
    host_construction_ready_log::record(pad_id, 1.0);
    logic.host_apply_construction_completions_after_ready_writeback();
    end_shadow_coupled_tick();

    let building_pos = logic.host_object(pad_id).expect("pad").get_position();
    let end_dock = crate::game_logic::host_repair::dozer_end_dock_position(action, building_pos);
    {
        let unit = logic.host_object(dozer).expect("dozer");
        let path_end = unit.movement.path.last().copied();
        assert_eq!(
            unit.ignored_obstacle_id,
            Some(pad_id),
            "sole-tick exit must ignore the finished building"
        );
        assert!(
            path_end.is_some_and(|p| p.distance(end_dock) < 15.0),
            "sole-tick exit must aim at the end dock, path={:?} end={end_dock:?}",
            unit.movement.path
        );
    }
    for _ in 0..40 {
        logic.update();
    }
    let unit = logic.host_object(dozer).expect("dozer");
    assert!(
        unit.get_position().distance(action) > 1.0,
        "sole-tick dozer must leave the pad, pos={:?} ai={:?} vel={:?} ignore={:?} target={:?} path={:?}",
        unit.get_position(),
        unit.ai_state,
        unit.movement.velocity,
        unit.ignored_obstacle_id,
        unit.movement.target_position,
        unit.movement.path
    );

    match prev_sh {
        Some(v) => crate::env_compat::set_var("GENERALS_GAMEWORLD_SHADOW", v),
        None => crate::env_compat::remove_var("GENERALS_GAMEWORLD_SHADOW"),
    }
}

#[test]
fn player_stop_clears_a_building_ignore() {
    use crate::game_logic::{KindOf, Player, ThingTemplate};
    let mut logic = GameLogic::new();
    logic.add_player(Player::new(0, Team::USA, "P0", true));
    let mut inf_t = ThingTemplate::new("StopInf");
    inf_t.add_kind_of(KindOf::Infantry).set_health(100.0);
    logic.templates.insert("StopInf".into(), inf_t);
    let mut bld_t = ThingTemplate::new("StopBuilding");
    bld_t.add_kind_of(KindOf::Structure).set_health(500.0);
    logic.templates.insert("StopBuilding".into(), bld_t);
    let building = logic
        .create_object_for_player("StopBuilding", 0, Vec3::new(40.0, 0.0, 0.0))
        .expect("building");
    let unit_id = logic
        .create_object_for_player("StopInf", 0, Vec3::ZERO)
        .expect("unit");
    {
        let unit = logic.host_object_mut(unit_id).expect("unit");
        unit.set_ai_state(AIState::Attacking);
        unit.ignored_obstacle_id = Some(building);
        unit.movement.path = vec![Vec3::new(40.0, 0.0, 0.0)];
        unit.set_status_moving(true);
    }
    assert!(logic.unit_command_stop(unit_id));
    let unit = logic.host_object(unit_id).expect("unit");
    assert_eq!(unit.ai_state, AIState::Idle);
    assert!(unit.ignored_obstacle_id.is_none());
    assert!(unit.movement.path.is_empty());
}

#[test]
fn aircraft_stop_clears_a_building_ignore() {
    use crate::game_logic::{KindOf, Player, ThingTemplate};
    let mut logic = GameLogic::new();
    logic.add_player(Player::new(0, Team::USA, "P0", true));
    let mut jet_t = ThingTemplate::new("StopJet");
    jet_t
        .add_kind_of(KindOf::Vehicle)
        .add_kind_of(KindOf::Aircraft)
        .set_health(200.0);
    logic.templates.insert("StopJet".into(), jet_t);
    let mut bld_t = ThingTemplate::new("StopJetBuilding");
    bld_t.add_kind_of(KindOf::Structure).set_health(500.0);
    logic.templates.insert("StopJetBuilding".into(), bld_t);
    let building = logic
        .create_object_for_player("StopJetBuilding", 0, Vec3::new(40.0, 0.0, 0.0))
        .expect("building");
    let jet = logic
        .create_object_for_player("StopJet", 0, Vec3::new(0.0, 30.0, 0.0))
        .expect("jet");
    {
        let unit = logic.host_object_mut(jet).expect("jet");
        unit.status.airborne_target = true;
        unit.ignored_obstacle_id = Some(building);
        unit.movement.path = vec![Vec3::new(40.0, 30.0, 0.0)];
        unit.set_status_moving(true);
    }
    assert!(logic.unit_command_stop(jet));
    let unit = logic.host_object(jet).expect("jet");
    assert!(unit.ignored_obstacle_id.is_none());
    assert!(unit.movement.path.is_empty());
}

#[test]
fn player_move_clears_a_building_ignore() {
    use crate::game_logic::{KindOf, Player, ThingTemplate};
    let mut logic = GameLogic::new();
    logic.add_player(Player::new(0, Team::USA, "P0", true));
    let mut inf_t = ThingTemplate::new("MoveInf");
    inf_t.add_kind_of(KindOf::Infantry).set_health(100.0);
    logic.templates.insert("MoveInf".into(), inf_t);
    let mut bld_t = ThingTemplate::new("MoveBuilding");
    bld_t.add_kind_of(KindOf::Structure).set_health(500.0);
    logic.templates.insert("MoveBuilding".into(), bld_t);
    let building = logic
        .create_object_for_player("MoveBuilding", 0, Vec3::new(40.0, 0.0, 0.0))
        .expect("building");
    let unit_id = logic
        .create_object_for_player("MoveInf", 0, Vec3::ZERO)
        .expect("unit");
    {
        let unit = logic.host_object_mut(unit_id).expect("unit");
        unit.set_ai_state(AIState::Attacking);
        unit.ignored_obstacle_id = Some(building);
    }
    assert!(logic.unit_command_move_to(unit_id, Vec3::new(30.0, 0.0, 10.0)));
    let unit = logic.host_object(unit_id).expect("unit");
    assert_eq!(unit.ai_state, AIState::Moving);
    assert!(unit.ignored_obstacle_id.is_none());
}

#[test]
fn carrier_idle_replaces_the_jet_building_ignore_with_the_carrier() {
    use crate::game_logic::host_flight_deck::{
        HostFlightDeckCommand, HostFlightDeckSpace, HostFlightDeckState,
    };
    use crate::game_logic::{FlightDeckMetadata, KindOf, Player, ThingTemplate};
    let mut logic = GameLogic::new();
    logic.add_player(Player::new(0, Team::USA, "P0", true));
    let mut carrier_t = ThingTemplate::new("StopCarrier");
    carrier_t.add_kind_of(KindOf::Structure).set_health(2000.0);
    carrier_t.flight_deck = Some(FlightDeckMetadata {
        payload_template: "StopJet".into(),
        num_rows: 1,
        num_cols: 1,
        approach_height: 50.0,
        landing_deck_height_offset: 0.0,
        heal_amount_per_second: 0.0,
        cleanup_frames: 1,
        human_follow_frames: 1,
        replacement_frames: 1,
        dock_animation_frames: 1,
        launch_wave_frames: 1,
        launch_ramp_frames: 1,
        lower_ramp_frames: 1,
        catapult_fire_frames: 1,
        catapult_system: [None, None],
    });
    logic.templates.insert("StopCarrier".into(), carrier_t);
    let mut jet_t = ThingTemplate::new("DeckJet");
    jet_t
        .add_kind_of(KindOf::Vehicle)
        .add_kind_of(KindOf::Aircraft)
        .set_health(200.0);
    logic.templates.insert("DeckJet".into(), jet_t);
    let mut bld_t = ThingTemplate::new("JetAttackBuilding");
    bld_t.add_kind_of(KindOf::Structure).set_health(500.0);
    logic.templates.insert("JetAttackBuilding".into(), bld_t);
    let carrier = logic
        .create_object_for_player("StopCarrier", 0, Vec3::ZERO)
        .expect("carrier");
    let jet = logic
        .create_object_for_player("DeckJet", 0, Vec3::new(20.0, 30.0, 0.0))
        .expect("jet");
    let building = logic
        .create_object_for_player("JetAttackBuilding", 0, Vec3::new(80.0, 0.0, 0.0))
        .expect("building");
    {
        let unit = logic.host_object_mut(jet).expect("jet");
        unit.status.airborne_target = true;
        unit.ignored_obstacle_id = Some(building);
        unit.set_ai_state(AIState::Attacking);
    }
    let mut state = HostFlightDeckState::default();
    state.got_info = true;
    state.spaces.push(HostFlightDeckSpace {
        object_id: Some(jet),
        position: Vec3::new(5.0, 0.0, 0.0),
        orientation: 0.0,
        runway: 0,
    });
    logic.flight_decks.insert(carrier, state);
    assert!(logic.flight_deck_ai_do_command(carrier, HostFlightDeckCommand::Idle, None, None));
    let unit = logic.host_object(jet).expect("jet");
    assert_eq!(unit.ai_state, AIState::Entering);
    assert_eq!(unit.ignored_obstacle_id, Some(carrier));
}

#[test]
fn leaving_enter_clears_the_carrier_ignore() {
    use crate::game_logic::{KindOf, Player, ThingTemplate};
    let mut logic = GameLogic::new();
    logic.add_player(Player::new(0, Team::USA, "P0", true));
    let mut jet_t = ThingTemplate::new("LeaveEnterJet");
    jet_t.add_kind_of(KindOf::Aircraft).set_health(100.0);
    logic.templates.insert("LeaveEnterJet".into(), jet_t);
    let mut deck_t = ThingTemplate::new("LeaveEnterDeck");
    deck_t.add_kind_of(KindOf::Structure).set_health(500.0);
    logic.templates.insert("LeaveEnterDeck".into(), deck_t);
    let deck = logic
        .create_object_for_player("LeaveEnterDeck", 0, Vec3::ZERO)
        .expect("deck");
    let jet = logic
        .create_object_for_player("LeaveEnterJet", 0, Vec3::new(10.0, 20.0, 0.0))
        .expect("jet");
    {
        let unit = logic.host_object_mut(jet).expect("jet");
        unit.set_ai_state(AIState::Entering);
        unit.ignored_obstacle_id = Some(deck);
        unit.set_ai_state(AIState::Docked);
    }
    let unit = logic.host_object(jet).expect("jet");
    assert_eq!(unit.ai_state, AIState::Docked);
    assert!(unit.ignored_obstacle_id.is_none());
}

#[test]
fn queued_enter_path_keeps_the_carrier_ignore() {
    use crate::game_logic::{KindOf, Player, ThingTemplate};
    let mut logic = GameLogic::new();
    logic.add_player(Player::new(0, Team::USA, "P0", true));
    let mut jet_t = ThingTemplate::new("EnterQueueJet");
    jet_t.add_kind_of(KindOf::Aircraft).set_health(100.0);
    logic.templates.insert("EnterQueueJet".into(), jet_t);
    let mut deck_t = ThingTemplate::new("EnterQueueDeck");
    deck_t.add_kind_of(KindOf::Structure).set_health(500.0);
    logic.templates.insert("EnterQueueDeck".into(), deck_t);
    let deck = logic
        .create_object_for_player("EnterQueueDeck", 0, Vec3::new(80.0, 0.0, 0.0))
        .expect("deck");
    let jet = logic
        .create_object_for_player("EnterQueueJet", 0, Vec3::ZERO)
        .expect("jet");
    {
        let unit = logic.host_object_mut(jet).expect("jet");
        unit.set_ai_state(AIState::Entering);
        unit.ignored_obstacle_id = Some(deck);
        unit.set_order_target(Some(deck));
    }
    logic.force_map_loaded_for_path_test(true);
    assert!(logic.assign_unit_path_ignoring(jet, Vec3::new(80.0, 0.0, 0.0), &[], Some(deck)));
    logic.process_pathfind_queue();
    let unit = logic.host_object(jet).expect("jet");
    assert_eq!(unit.ai_state, AIState::Entering);
    assert_eq!(unit.ignored_obstacle_id, Some(deck));
    assert!(!unit.movement.path.is_empty() || unit.waiting_for_path);
}

#[test]
fn reissued_enter_crosses_the_carrier() {
    use crate::game_logic::pathfinding::GridPos;
    use crate::game_logic::{KindOf, Player, ThingTemplate};
    let mut logic = GameLogic::new();
    logic.add_player(Player::new(0, Team::USA, "P0", true));
    let mut jet_t = ThingTemplate::new("EnterReplayJet");
    jet_t.add_kind_of(KindOf::Aircraft).set_health(100.0);
    logic.templates.insert("EnterReplayJet".into(), jet_t);
    let mut deck_t = ThingTemplate::new("EnterReplayDeck");
    deck_t.add_kind_of(KindOf::Structure).set_health(500.0);
    logic.templates.insert("EnterReplayDeck".into(), deck_t);
    let deck = logic
        .create_object_for_player("EnterReplayDeck", 0, Vec3::new(90.0, 0.0, 0.0))
        .expect("deck");
    let jet = logic
        .create_object_for_player("EnterReplayJet", 0, Vec3::ZERO)
        .expect("jet");
    {
        let unit = logic.host_object_mut(jet).expect("jet");
        unit.set_ai_state(AIState::Entering);
        unit.ignored_obstacle_id = Some(deck);
        unit.set_order_target(Some(deck));
        unit.pending_move = Some(Vec3::new(90.0, 0.0, 0.0));
    }
    let wall_x = {
        let grid = &logic.pathfinding_system.grid;
        let start = grid.world_to_grid(Vec3::ZERO);
        let goal = grid.world_to_grid(Vec3::new(90.0, 0.0, 0.0));
        (start.x + goal.x) / 2
    };
    let height = logic.pathfinding_system.grid.height();
    for y in 0..height {
        logic.pathfinding_system.grid.set_cell_obstacle_owned(
            GridPos::new(wall_x, y),
            false,
            false,
            deck.0,
            None,
            None,
        );
    }
    logic.force_map_loaded_for_path_test(true);
    logic.reissue_pending_moves();
    logic.process_pathfind_queue();
    let unit = logic.host_object(jet).expect("jet");
    assert_eq!(unit.ai_state, AIState::Entering);
    assert_eq!(unit.ignored_obstacle_id, Some(deck));
    let xs: Vec<i32> = unit
        .movement
        .path
        .iter()
        .map(|wp| logic.pathfinding_system.grid.world_to_grid(*wp).x)
        .collect();
    let crosses = xs.windows(2).any(|w| {
        let lo = w[0].min(w[1]);
        let hi = w[0].max(w[1]);
        lo <= wall_x && wall_x <= hi
    });
    assert!(
        crosses,
        "replayed enter must ignore the carrier, xs={xs:?} wall={wall_x}"
    );
}

#[test]
fn cliff_cell_picks_the_cliff_member_when_the_set_changes() {
    use crate::game_logic::host_upgrade_module_residuals::{
        AuthoredLocomotorSet, HostLocomotorSetKind,
    };
    use crate::game_logic::{KindOf, Player, ThingTemplate};
    use gamelogic::ai::pathfind_astar::PathfindCellType;

    let mut logic = GameLogic::new();
    logic.add_player(Player::new(0, Team::USA, "P0", true));
    let mut template = ThingTemplate::new("CliffBike");
    template.add_kind_of(KindOf::Vehicle);
    template.authored_locomotor_sets = Some(vec![AuthoredLocomotorSet {
        kind: HostLocomotorSetKind::Normal,
        members: vec![
            "CombatBikeGroundLocomotor".into(),
            "CombatBikeCliffLocomotor".into(),
        ],
    }]);
    logic.templates.insert("CliffBike".into(), template);
    let id = logic
        .create_object_for_player("CliffBike", 0, Vec3::ZERO)
        .expect("bike");
    let cell = logic.pathfinding_system.grid.world_to_grid(Vec3::ZERO);
    logic
        .pathfinding_system
        .grid
        .set_cell_type(cell, PathfindCellType::Cliff);
    assert!(logic.apply_unit_locomotor_set(id, "normal"));
    let bike = logic.host_object(id).expect("bike");
    assert_eq!(
        bike.cur_locomotor_name.as_deref(),
        Some("CombatBikeCliffLocomotor"),
        "a cliff cell must bind the cliff member before the next movement frame"
    );
}
#[test]
fn garrison_enter_ignores_the_bunker() {
    use crate::game_logic::{
        BuildingData, BuildingType, ContainAdmission, ContainModuleKind, KindOf, Player,
        ThingTemplate,
    };
    let mut logic = GameLogic::new();
    logic.add_player(Player::new(0, Team::USA, "P0", true));
    let mut inf_t = ThingTemplate::new("GarrisonInf");
    inf_t.add_kind_of(KindOf::Infantry).set_health(100.0);
    inf_t.transport_slot_count = Some(1);
    logic.templates.insert("GarrisonInf".into(), inf_t);
    let mut bunker_t = ThingTemplate::new("GarrisonBunker");
    bunker_t.add_kind_of(KindOf::Structure).set_health(800.0);
    bunker_t.contain_module.kind = ContainModuleKind::Garrison;
    bunker_t.contain_module.admission = ContainAdmission::InfantryOnly;
    logic.templates.insert("GarrisonBunker".into(), bunker_t);
    let bunker = logic
        .create_object_for_player("GarrisonBunker", 0, Vec3::new(40.0, 0.0, 0.0))
        .expect("bunker");
    let infantry = logic
        .create_object_for_player("GarrisonInf", 0, Vec3::ZERO)
        .expect("infantry");
    {
        let building = logic.host_object_mut(bunker).expect("bunker");
        let mut data = BuildingData::new(BuildingType::PowerPlant);
        data.max_garrison = 4;
        building.building_data = Some(data);
        building.set_status_under_construction(false);
        building.construction_percent = 1.0;
    }
    use crate::game_logic::pathfinding::GridPos;
    let wall_x = {
        let grid = &logic.pathfinding_system.grid;
        let start = grid.world_to_grid(Vec3::ZERO);
        let goal = grid.world_to_grid(Vec3::new(40.0, 0.0, 0.0));
        (start.x + goal.x) / 2
    };
    let height = logic.pathfinding_system.grid.height();
    for y in 0..height {
        logic.pathfinding_system.grid.set_cell_obstacle_owned(
            GridPos::new(wall_x, y),
            false,
            false,
            bunker.0,
            None,
            None,
        );
    }
    logic.force_map_loaded_for_path_test(true);
    assert!(
        logic.try_guard_enter_or_hijack(infantry, bunker, false, Team::USA),
        "infantry should be allowed to enter the bunker"
    );
    logic.process_pathfind_queue();
    let unit = logic.host_object(infantry).expect("infantry");
    assert_eq!(unit.ai_state, AIState::Entering);
    assert_eq!(unit.ignored_obstacle_id, Some(bunker));
    let xs: Vec<i32> = unit
        .movement
        .path
        .iter()
        .map(|wp| logic.pathfinding_system.grid.world_to_grid(*wp).x)
        .collect();
    let crosses = xs.windows(2).any(|w| {
        let lo = w[0].min(w[1]);
        let hi = w[0].max(w[1]);
        lo <= wall_x && wall_x <= hi
    });
    assert!(
        crosses,
        "garrison enter must path through the bunker column, xs={xs:?} wall={wall_x}"
    );
    logic.update_support_states_for_test(&[infantry], 1.0 / 30.0);
    logic.process_pathfind_queue();
    let unit = logic.host_object(infantry).expect("infantry");
    assert_eq!(unit.ai_state, AIState::Entering);
    let xs: Vec<i32> = unit
        .movement
        .path
        .iter()
        .map(|wp| logic.pathfinding_system.grid.world_to_grid(*wp).x)
        .collect();
    let crosses = xs.windows(2).any(|w| {
        let lo = w[0].min(w[1]);
        let hi = w[0].max(w[1]);
        lo <= wall_x && wall_x <= hi
    });
    assert!(
        crosses,
        "the next enter tick must still path through the bunker, xs={xs:?} wall={wall_x}"
    );
}

#[test]
fn hijack_paths_through_the_vehicle() {
    use crate::game_logic::pathfinding::GridPos;
    use crate::game_logic::{KindOf, Player, ThingTemplate};
    let mut logic = GameLogic::new();
    logic.add_player(Player::new(0, Team::USA, "P0", true));
    logic.add_player(Player::new(1, Team::GLA, "P1", false));
    let mut inf_t = ThingTemplate::new("HijackInf");
    inf_t.add_kind_of(KindOf::Infantry).set_health(100.0);
    logic.templates.insert("HijackInf".into(), inf_t);
    let mut veh_t = ThingTemplate::new("HijackTruck");
    veh_t
        .add_kind_of(KindOf::Vehicle)
        .add_kind_of(KindOf::Attackable)
        .set_health(200.0);
    logic.templates.insert("HijackTruck".into(), veh_t);
    let vehicle = logic
        .create_object_for_player("HijackTruck", 1, Vec3::new(90.0, 0.0, 0.0))
        .expect("vehicle");
    let infantry = logic
        .create_object_for_player("HijackInf", 0, Vec3::ZERO)
        .expect("infantry");
    let wall_x = {
        let grid = &logic.pathfinding_system.grid;
        let start = grid.world_to_grid(Vec3::ZERO);
        let goal = grid.world_to_grid(Vec3::new(90.0, 0.0, 0.0));
        (start.x + goal.x) / 2
    };
    let height = logic.pathfinding_system.grid.height();
    for y in 0..height {
        logic.pathfinding_system.grid.set_cell_obstacle_owned(
            GridPos::new(wall_x, y),
            false,
            false,
            vehicle.0,
            None,
            None,
        );
    }
    logic.force_map_loaded_for_path_test(true);
    assert!(logic.try_guard_enter_or_hijack(infantry, vehicle, true, Team::USA));
    logic.process_pathfind_queue();
    let unit = logic.host_object(infantry).expect("infantry");
    assert_eq!(unit.ai_state, AIState::SpecialAbility);
    assert_eq!(unit.ignored_obstacle_id, Some(vehicle));
    let xs: Vec<i32> = unit
        .movement
        .path
        .iter()
        .map(|wp| logic.pathfinding_system.grid.world_to_grid(*wp).x)
        .collect();
    let crosses = xs.windows(2).any(|w| {
        let lo = w[0].min(w[1]);
        let hi = w[0].max(w[1]);
        lo <= wall_x && wall_x <= hi
    });
    assert!(
        crosses,
        "hijack must path through the vehicle column, xs={xs:?} wall={wall_x}"
    );
}

#[test]
fn non_hijacker_at_the_vehicle_is_released() {
    use crate::game_logic::{KindOf, PendingSpecialAbility, Player, ThingTemplate};
    let mut logic = GameLogic::new();
    logic.add_player(Player::new(0, Team::USA, "P0", true));
    logic.add_player(Player::new(1, Team::GLA, "P1", false));
    let mut inf_t = ThingTemplate::new("PlainInfantry");
    inf_t.add_kind_of(KindOf::Infantry).set_health(100.0);
    logic.templates.insert("PlainInfantry".into(), inf_t);
    let mut veh_t = ThingTemplate::new("StealMe");
    veh_t
        .add_kind_of(KindOf::Vehicle)
        .add_kind_of(KindOf::Attackable)
        .set_health(200.0);
    logic.templates.insert("StealMe".into(), veh_t);
    let vehicle = logic
        .create_object_for_player("StealMe", 1, Vec3::ZERO)
        .expect("vehicle");
    let infantry = logic
        .create_object_for_player("PlainInfantry", 0, Vec3::ZERO)
        .expect("infantry");
    logic.pending_special_abilities.insert(
        infantry,
        PendingSpecialAbility::Hijack { target_id: vehicle },
    );
    {
        let unit = logic.host_object_mut(infantry).expect("infantry");
        unit.set_ai_state(AIState::SpecialAbility);
        unit.set_order_target(Some(vehicle));
        unit.ignored_obstacle_id = Some(vehicle);
        unit.movement.path = vec![Vec3::ZERO];
        unit.set_status_moving(true);
    }
    logic.update_support_states_for_test(&[infantry], 1.0 / 30.0);
    let unit = logic.host_object(infantry).expect("infantry");
    assert_eq!(unit.ai_state, AIState::Idle);
    assert!(unit.ignored_obstacle_id.is_none());
    assert!(unit.movement.path.is_empty());
    assert!(!logic.pending_special_abilities.contains_key(&infantry));
    assert!(!logic.host_object(vehicle).expect("vehicle").status.hijacked);
}

#[test]
fn rejected_car_bomb_releases_the_infantry() {
    use crate::game_logic::{KindOf, PendingSpecialAbility, Player, ThingTemplate};
    let mut logic = GameLogic::new();
    logic.add_player(Player::new(0, Team::USA, "P0", true));
    logic.add_player(Player::new(1, Team::GLA, "P1", false));
    let mut inf_t = ThingTemplate::new("BombInfantry");
    inf_t.add_kind_of(KindOf::Infantry).set_health(100.0);
    logic.templates.insert("BombInfantry".into(), inf_t);
    let mut boat_t = ThingTemplate::new("BombBoat");
    boat_t
        .add_kind_of(KindOf::Vehicle)
        .add_kind_of(KindOf::Boat)
        .add_kind_of(KindOf::Attackable)
        .set_health(200.0);
    logic.templates.insert("BombBoat".into(), boat_t);
    let boat = logic
        .create_object_for_player("BombBoat", 1, Vec3::ZERO)
        .expect("boat");
    let infantry = logic
        .create_object_for_player("BombInfantry", 0, Vec3::ZERO)
        .expect("infantry");
    logic
        .pending_special_abilities
        .insert(infantry, PendingSpecialAbility::CarBomb { target_id: boat });
    {
        let unit = logic.host_object_mut(infantry).expect("infantry");
        unit.set_ai_state(AIState::SpecialAbility);
        unit.set_order_target(Some(boat));
        unit.ignored_obstacle_id = Some(boat);
        unit.movement.path = vec![Vec3::ZERO];
        unit.set_status_moving(true);
    }
    logic.update_support_states_for_test(&[infantry], 1.0 / 30.0);
    let unit = logic.host_object(infantry).expect("infantry");
    assert_eq!(unit.ai_state, AIState::Idle);
    assert!(unit.ignored_obstacle_id.is_none());
    assert!(unit.movement.path.is_empty());
    assert!(!logic.pending_special_abilities.contains_key(&infantry));
}

#[test]
fn rejected_disguise_releases_the_infantry() {
    use crate::game_logic::{KindOf, PendingSpecialAbility, Player, ThingTemplate};
    let mut logic = GameLogic::new();
    logic.add_player(Player::new(0, Team::USA, "P0", true));
    logic.add_player(Player::new(1, Team::GLA, "P1", false));
    let mut inf_t = ThingTemplate::new("DisguiseInfantry");
    inf_t.add_kind_of(KindOf::Infantry).set_health(100.0);
    logic.templates.insert("DisguiseInfantry".into(), inf_t);
    let mut jet_t = ThingTemplate::new("DisguiseJet");
    jet_t
        .add_kind_of(KindOf::Vehicle)
        .add_kind_of(KindOf::Aircraft)
        .add_kind_of(KindOf::Attackable)
        .set_health(200.0);
    logic.templates.insert("DisguiseJet".into(), jet_t);
    let jet = logic
        .create_object_for_player("DisguiseJet", 1, Vec3::ZERO)
        .expect("jet");
    let infantry = logic
        .create_object_for_player("DisguiseInfantry", 0, Vec3::ZERO)
        .expect("infantry");
    logic.pending_special_abilities.insert(
        infantry,
        PendingSpecialAbility::DisguiseAsVehicle { target_id: jet },
    );
    {
        let unit = logic.host_object_mut(infantry).expect("infantry");
        unit.set_ai_state(AIState::SpecialAbility);
        unit.set_order_target(Some(jet));
        unit.ignored_obstacle_id = Some(jet);
        unit.movement.path = vec![Vec3::ZERO];
        unit.set_status_moving(true);
    }
    logic.update_support_states_for_test(&[infantry], 1.0 / 30.0);
    let unit = logic.host_object(infantry).expect("infantry");
    assert_eq!(unit.ai_state, AIState::Idle);
    assert!(unit.ignored_obstacle_id.is_none());
    assert!(unit.movement.path.is_empty());
    assert!(!logic.pending_special_abilities.contains_key(&infantry));
}

#[test]
fn sabotage_of_a_vehicle_releases_the_infantry() {
    use crate::game_logic::{KindOf, PendingSpecialAbility, Player, ThingTemplate};
    let mut logic = GameLogic::new();
    logic.add_player(Player::new(0, Team::USA, "P0", true));
    logic.add_player(Player::new(1, Team::GLA, "P1", false));
    let mut inf_t = ThingTemplate::new("SaboteurInf");
    inf_t.add_kind_of(KindOf::Infantry).set_health(100.0);
    logic.templates.insert("SaboteurInf".into(), inf_t);
    let mut veh_t = ThingTemplate::new("SabotageTruck");
    veh_t
        .add_kind_of(KindOf::Vehicle)
        .add_kind_of(KindOf::Attackable)
        .set_health(200.0);
    logic.templates.insert("SabotageTruck".into(), veh_t);
    let vehicle = logic
        .create_object_for_player("SabotageTruck", 1, Vec3::ZERO)
        .expect("vehicle");
    let infantry = logic
        .create_object_for_player("SaboteurInf", 0, Vec3::ZERO)
        .expect("infantry");
    logic.pending_special_abilities.insert(
        infantry,
        PendingSpecialAbility::Sabotage { target_id: vehicle },
    );
    {
        let unit = logic.host_object_mut(infantry).expect("infantry");
        unit.set_ai_state(AIState::SpecialAbility);
        unit.set_order_target(Some(vehicle));
        unit.ignored_obstacle_id = Some(vehicle);
        unit.movement.path = vec![Vec3::ZERO];
        unit.set_status_moving(true);
    }
    logic.update_support_states_for_test(&[infantry], 1.0 / 30.0);
    let unit = logic.host_object(infantry).expect("infantry");
    assert_eq!(unit.ai_state, AIState::Idle);
    assert!(unit.ignored_obstacle_id.is_none());
    assert!(unit.movement.path.is_empty());
    assert!(!logic.pending_special_abilities.contains_key(&infantry));
}

#[test]
fn hijack_followup_tick_still_crosses_the_vehicle() {
    use crate::game_logic::pathfinding::GridPos;
    use crate::game_logic::{KindOf, PendingSpecialAbility, Player, ThingTemplate};
    let mut logic = GameLogic::new();
    logic.add_player(Player::new(0, Team::USA, "P0", true));
    logic.add_player(Player::new(1, Team::GLA, "P1", false));
    let mut inf_t = ThingTemplate::new("HijackFollow");
    inf_t.add_kind_of(KindOf::Infantry).set_health(100.0);
    logic.templates.insert("HijackFollow".into(), inf_t);
    let mut veh_t = ThingTemplate::new("HijackFollowTruck");
    veh_t
        .add_kind_of(KindOf::Vehicle)
        .add_kind_of(KindOf::Attackable)
        .set_health(200.0);
    logic.templates.insert("HijackFollowTruck".into(), veh_t);
    let vehicle = logic
        .create_object_for_player("HijackFollowTruck", 1, Vec3::new(120.0, 0.0, 0.0))
        .expect("vehicle");
    let infantry = logic
        .create_object_for_player("HijackFollow", 0, Vec3::ZERO)
        .expect("infantry");
    logic.pending_special_abilities.insert(
        infantry,
        PendingSpecialAbility::Hijack { target_id: vehicle },
    );
    {
        let unit = logic.host_object_mut(infantry).expect("infantry");
        unit.set_ai_state(AIState::SpecialAbility);
        unit.set_order_target(Some(vehicle));
    }
    let wall_x = {
        let grid = &logic.pathfinding_system.grid;
        let start = grid.world_to_grid(Vec3::ZERO);
        let goal = grid.world_to_grid(Vec3::new(120.0, 0.0, 0.0));
        (start.x + goal.x) / 2
    };
    let height = logic.pathfinding_system.grid.height();
    for y in 0..height {
        logic.pathfinding_system.grid.set_cell_obstacle_owned(
            GridPos::new(wall_x, y),
            false,
            false,
            vehicle.0,
            None,
            None,
        );
    }
    logic.force_map_loaded_for_path_test(true);
    logic.update_support_states_for_test(&[infantry], 1.0 / 30.0);
    logic.process_pathfind_queue();
    let unit = logic.host_object(infantry).expect("infantry");
    assert_eq!(unit.ai_state, AIState::SpecialAbility);
    assert_eq!(unit.ignored_obstacle_id, Some(vehicle));
    let xs: Vec<i32> = unit
        .movement
        .path
        .iter()
        .map(|wp| logic.pathfinding_system.grid.world_to_grid(*wp).x)
        .collect();
    let crosses = xs.windows(2).any(|w| {
        let lo = w[0].min(w[1]);
        let hi = w[0].max(w[1]);
        lo <= wall_x && wall_x <= hi
    });
    assert!(
        crosses,
        "hijack follow-up must path through the vehicle, xs={xs:?} wall={wall_x}"
    );
}

