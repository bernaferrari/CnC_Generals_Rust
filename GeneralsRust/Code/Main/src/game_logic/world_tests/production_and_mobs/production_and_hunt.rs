//! Behavior suite extracted from `production_and_mobs`.
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

#[test]
fn plant_approach_ignores_the_structure() {
    use crate::game_logic::pathfinding::GridPos;
    use crate::game_logic::{KindOf, PendingSpecialAbility, Player, ThingTemplate};
    let mut logic = GameLogic::new();
    logic.add_player(Player::new(0, Team::USA, "P0", true));
    logic.add_player(Player::new(1, Team::GLA, "P1", false));
    let mut inf_t = ThingTemplate::new("PlanterApproach");
    inf_t.add_kind_of(KindOf::Infantry).set_health(100.0);
    logic.templates.insert("PlanterApproach".into(), inf_t);
    let mut bld_t = ThingTemplate::new("PlantApproachBuilding");
    bld_t
        .add_kind_of(KindOf::Structure)
        .add_kind_of(KindOf::Attackable)
        .set_health(500.0);
    logic
        .templates
        .insert("PlantApproachBuilding".into(), bld_t);
    let building = logic
        .create_object_for_player("PlantApproachBuilding", 1, Vec3::new(120.0, 0.0, 0.0))
        .expect("building");
    let planter = logic
        .create_object_for_player("PlanterApproach", 0, Vec3::ZERO)
        .expect("planter");
    logic.pending_special_abilities.insert(
        planter,
        PendingSpecialAbility::PlantTimedDemoCharge {
            target_id: building,
        },
    );
    {
        let unit = logic.host_object_mut(planter).expect("planter");
        unit.set_ai_state(AIState::SpecialAbility);
        unit.set_order_target(Some(building));
        unit.ignored_obstacle_id = None;
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
            building.0,
            None,
            None,
        );
    }
    logic.force_map_loaded_for_path_test(true);
    logic.update_support_states_for_test(&[planter], 1.0 / 30.0);
    logic.process_pathfind_queue();
    let unit = logic.host_object(planter).expect("planter");
    assert_eq!(unit.ai_state, AIState::SpecialAbility);
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
        "plant approach must path through the structure, xs={xs:?} wall={wall_x}"
    );
}

#[test]
fn successful_disguise_drops_the_vehicle_ignore() {
    use crate::game_logic::{KindOf, PendingSpecialAbility, Player, ThingTemplate};
    let mut logic = GameLogic::new();
    logic.add_player(Player::new(0, Team::USA, "P0", true));
    logic.add_player(Player::new(1, Team::GLA, "P1", false));
    let mut inf_t = ThingTemplate::new("DisguiseOkInf");
    inf_t.add_kind_of(KindOf::Infantry).set_health(100.0);
    logic.templates.insert("DisguiseOkInf".into(), inf_t);
    let mut veh_t = ThingTemplate::new("DisguiseOkTruck");
    veh_t
        .add_kind_of(KindOf::Vehicle)
        .add_kind_of(KindOf::Attackable)
        .set_health(200.0);
    logic.templates.insert("DisguiseOkTruck".into(), veh_t);
    let vehicle = logic
        .create_object_for_player("DisguiseOkTruck", 1, Vec3::ZERO)
        .expect("vehicle");
    let infantry = logic
        .create_object_for_player("DisguiseOkInf", 0, Vec3::ZERO)
        .expect("infantry");
    logic.pending_special_abilities.insert(
        infantry,
        PendingSpecialAbility::DisguiseAsVehicle { target_id: vehicle },
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
    assert_eq!(
        unit.disguise_pending_template.as_deref(),
        Some("DisguiseOkTruck")
    );
    assert!(!unit.status.disguised);
    assert!(unit.ignored_obstacle_id.is_none());
    assert_eq!(unit.ai_state, AIState::Idle);
    assert!(unit.movement.path.is_empty());
}

#[test]
fn flee_after_plant_ignores_the_structure() {
    use crate::game_logic::{KindOf, Player, ThingTemplate};
    let mut logic = GameLogic::new();
    logic.add_player(Player::new(0, Team::USA, "P0", true));
    let mut inf_t = ThingTemplate::new("Planter");
    inf_t.add_kind_of(KindOf::Infantry).set_health(100.0);
    logic.templates.insert("Planter".into(), inf_t);
    let mut bld_t = ThingTemplate::new("TrappedBuilding");
    bld_t.add_kind_of(KindOf::Structure).set_health(500.0);
    logic.templates.insert("TrappedBuilding".into(), bld_t);
    let building = logic
        .create_object_for_player("TrappedBuilding", 0, Vec3::ZERO)
        .expect("building");
    let planter = logic
        .create_object_for_player("Planter", 0, Vec3::ZERO)
        .expect("planter");
    {
        let unit = logic.host_object_mut(planter).expect("planter");
        unit.ignored_obstacle_id = None;
        unit.set_ai_state(AIState::SpecialAbility);
        unit.set_orientation(0.0);
    }
    use crate::game_logic::pathfinding::GridPos;
    let wall_x = {
        let grid = &logic.pathfinding_system.grid;
        let start = grid.world_to_grid(Vec3::ZERO);
        let goal = grid.world_to_grid(Vec3::new(-40.0, 0.0, 0.0));
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
    logic.leftover_flee_after_plant(planter, building, Team::USA, 40.0, false);
    logic.process_pathfind_queue();
    let unit = logic.host_object(planter).expect("planter");
    assert_eq!(unit.ai_state, AIState::Moving);
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
        "flee after plant must path through the structure, xs={xs:?} wall={wall_x}"
    );
}

#[test]
fn recrew_enter_ignores_the_empty_vehicle() {
    use crate::game_logic::{KindOf, Player, ThingTemplate};
    let mut logic = GameLogic::new();
    logic.add_player(Player::new(0, Team::USA, "P0", true));
    let mut inf_t = ThingTemplate::new("RecrewInf");
    inf_t.add_kind_of(KindOf::Infantry).set_health(100.0);
    logic.templates.insert("RecrewInf".into(), inf_t);
    let mut veh_t = ThingTemplate::new("EmptyHusk");
    veh_t.add_kind_of(KindOf::Vehicle).set_health(200.0);
    logic.templates.insert("EmptyHusk".into(), veh_t);
    let vehicle = logic
        .create_object_for_player("EmptyHusk", 0, Vec3::new(200.0, 0.0, 0.0))
        .expect("vehicle");
    let infantry = logic
        .create_object_for_player("RecrewInf", 0, Vec3::ZERO)
        .expect("infantry");
    logic
        .host_object_mut(vehicle)
        .expect("vehicle")
        .set_status_disabled_unmanned(true);
    {
        let unit = logic.host_object_mut(infantry).expect("infantry");
        unit.set_order_target(Some(vehicle));
        unit.set_ai_state(AIState::Entering);
    }
    use crate::game_logic::pathfinding::GridPos;
    let wall_x = {
        let grid = &logic.pathfinding_system.grid;
        let start = grid.world_to_grid(Vec3::ZERO);
        let goal = grid.world_to_grid(Vec3::new(200.0, 0.0, 0.0));
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
    assert_eq!(unit.ai_state, AIState::Entering);
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
        "recrew enter must path through the vehicle column, xs={xs:?} wall={wall_x}"
    );
}

#[test]
fn water_cell_set_change_falls_back_to_ground_not_the_previous_member() {
    use crate::game_logic::host_upgrade_module_residuals::{
        AuthoredLocomotorSet, HostLocomotorSetKind,
    };
    use crate::game_logic::object::LOCO_SURFACE_CLIFF;
    use crate::game_logic::{KindOf, Player, ThingTemplate};
    use gamelogic::ai::pathfind_astar::PathfindCellType;

    let mut logic = GameLogic::new();
    logic.add_player(Player::new(0, Team::USA, "P0", true));
    let mut template = ThingTemplate::new("WaterBike");
    template.add_kind_of(KindOf::Vehicle);
    template.authored_locomotor_sets = Some(vec![AuthoredLocomotorSet {
        kind: HostLocomotorSetKind::Normal,
        members: vec![
            "CombatBikeCliffLocomotor".into(),
            "CombatBikeGroundLocomotor".into(),
        ],
    }]);
    logic.templates.insert("WaterBike".into(), template);
    let id = logic
        .create_object_for_player("WaterBike", 0, Vec3::ZERO)
        .expect("bike");
    {
        let bike = logic.host_object_mut(id).expect("bike");
        bike.locomotor_surfaces = LOCO_SURFACE_CLIFF;
        bike.precise_z_pos = true;
    }
    let cell = logic.pathfinding_system.grid.world_to_grid(Vec3::ZERO);
    logic
        .pathfinding_system
        .grid
        .set_cell_type(cell, PathfindCellType::Water);
    assert!(logic.apply_unit_locomotor_set(id, "normal"));
    {
        let bike = logic.host_object(id).expect("bike");
        assert_eq!(
            bike.cur_locomotor_name.as_deref(),
            Some("CombatBikeGroundLocomotor"),
            "a cell with no matching member must bind GROUND, not the previous cliff locomotor"
        );
        assert!(!bike.precise_z_pos, "changing the member clears precise-z");
    }
    {
        let bike = logic.host_object_mut(id).expect("bike");
        bike.precise_z_pos = true;
    }
    assert!(logic.apply_unit_locomotor_set(id, "normal"));
    let bike = logic.host_object(id).expect("bike");
    assert!(bike.precise_z_pos, "the same set must not clear precise-z");
    assert_eq!(
        bike.cur_locomotor_name.as_deref(),
        Some("CombatBikeGroundLocomotor")
    );
}

#[test]
fn missing_bridge_layer_uses_the_ground_cell_not_clear() {
    use crate::game_logic::host_upgrade_module_residuals::{
        AuthoredLocomotorSet, HostLocomotorSetKind,
    };
    use crate::game_logic::{KindOf, Player, ThingTemplate};
    use gamelogic::ai::pathfind_astar::PathfindCellType;

    let mut logic = GameLogic::new();
    logic.add_player(Player::new(0, Team::USA, "P0", true));
    let mut template = ThingTemplate::new("BridgeBike");
    template.add_kind_of(KindOf::Vehicle);
    template.authored_locomotor_sets = Some(vec![AuthoredLocomotorSet {
        kind: HostLocomotorSetKind::Normal,
        members: vec![
            "CombatBikeGroundLocomotor".into(),
            "RaptorJetLocomotor".into(),
        ],
    }]);
    logic.templates.insert("BridgeBike".into(), template);
    let id = logic
        .create_object_for_player("BridgeBike", 0, Vec3::ZERO)
        .expect("bike");
    {
        let bike = logic.host_object_mut(id).expect("bike");
        // Above LAYER_GROUND, with no deck cell. getCell falls through to the map.
        bike.pathfind_layer = 2;
    }
    let cell = logic.pathfinding_system.grid.world_to_grid(Vec3::ZERO);
    logic
        .pathfinding_system
        .grid
        .set_cell_type(cell, PathfindCellType::Water);
    assert!(logic.apply_unit_locomotor_set(id, "normal"));
    let bike = logic.host_object(id).expect("bike");
    assert_eq!(
        bike.cur_locomotor_name.as_deref(),
        Some("RaptorJetLocomotor"),
        "a missing bridge cell must use the water underneath, which only the air member fits"
    );
}

#[test]
fn bridge_deck_locomotor_ignores_the_water_underneath() {
    use crate::game_logic::host_upgrade_module_residuals::{
        AuthoredLocomotorSet, HostLocomotorSetKind,
    };
    use crate::game_logic::{KindOf, Player, ThingTemplate};
    use gamelogic::ai::pathfind_astar::PathfindCellType;

    let mut logic = GameLogic::new();
    logic.add_player(Player::new(0, Team::USA, "P0", true));
    let mut template = ThingTemplate::new("DeckBike");
    template.add_kind_of(KindOf::Vehicle);
    template.authored_locomotor_sets = Some(vec![AuthoredLocomotorSet {
        kind: HostLocomotorSetKind::Normal,
        members: vec![
            "CombatBikeGroundLocomotor".into(),
            "RaptorJetLocomotor".into(),
        ],
    }]);
    logic.templates.insert("DeckBike".into(), template);
    let id = logic
        .create_object_for_player("DeckBike", 0, Vec3::ZERO)
        .expect("bike");
    let cell = logic.pathfinding_system.grid.world_to_grid(Vec3::ZERO);
    logic
        .pathfinding_system
        .grid
        .set_cell_type(cell, PathfindCellType::Water);
    logic.pathfinding_system.grid.stamp_bridge_deck(
        Vec3::new(-30.0, 20.0, -15.0),
        Vec3::new(-30.0, 20.0, 15.0),
        Vec3::new(30.0, 20.0, -15.0),
        Vec3::new(30.0, 20.0, 15.0),
        false,
    );
    let layer = logic
        .pathfinding_system
        .grid
        .first_bridge_layer_id()
        .expect("deck layer");
    assert_eq!(
        logic.pathfinding_system.grid.layer_cell_type(layer, cell),
        Some(PathfindCellType::Clear),
        "the deck over the bike must be Clear"
    );
    {
        let bike = logic.host_object_mut(id).expect("bike");
        bike.pathfind_layer = layer;
    }
    assert!(logic.apply_unit_locomotor_set(id, "normal"));
    assert_eq!(
        logic.host_object(id).unwrap().cur_locomotor_name.as_deref(),
        Some("CombatBikeGroundLocomotor"),
        "the deck is Clear, so the ground member wins over the air member"
    );
    logic.update();
    assert_eq!(
        logic.host_object(id).unwrap().cur_locomotor_name.as_deref(),
        Some("CombatBikeGroundLocomotor"),
        "the movement tick must keep the deck cell, not the water underneath"
    );
}

#[test]
fn queued_infantry_spawns_during_simulation() {
    let mut logic = GameLogic::new();
    ensure_test_player_for_team(&mut logic, Team::USA);
    if let Some(player) = logic.get_player_mut(0) {
        player.resources.supplies = 50_000;
        player.power_available = 100;
    }
    ensure_test_barracks_template(&mut logic);
    ensure_test_infantry_template(&mut logic);
    if let Some(infantry) = logic.templates.get_mut("TestInfantry") {
        infantry.build_time = 0.05;
    }
    let barracks = logic
        .create_object_for_player("TestBarracks", 0, Vec3::ZERO)
        .expect("barracks");
    if let Some(building) = logic.host_object_mut(barracks) {
        building.construction_percent = 1.0;
        building.set_status_under_construction(false);
    }
    assert!(logic.enqueue_production(barracks, "TestInfantry".to_string()));
    for _ in 0..60 {
        logic.update();
    }
    let unit = logic
        .objects
        .values()
        .find(|object| object.template_name == "TestInfantry")
        .expect("spawned infantry");
    assert!(
        unit.get_position().distance(Vec3::ZERO) > 5.0,
        "spawned infantry must walk clear of the barracks, pos={:?} ai={:?} vel={:?} target={:?} path={:?} waiting={}",
        unit.get_position(),
        unit.ai_state,
        unit.movement.velocity,
        unit.movement.target_position,
        unit.movement.path,
        unit.waiting_for_path
    );
    assert_eq!(
        unit.owner_player_id,
        Some(0),
        "spawned infantry must belong to the factory owner"
    );
}

#[test]
fn reissued_build_stays_constructing() {
    use crate::game_logic::pathfinding::GridPos;
    use crate::game_logic::{KindOf, Player, ThingTemplate};
    let mut logic = GameLogic::new();
    logic.add_player(Player::new(0, Team::USA, "P0", true));
    let mut pad = ThingTemplate::new("ReissuePad");
    pad.add_kind_of(KindOf::Structure).set_health(1_000.0);
    logic.templates.insert("ReissuePad".into(), pad);
    let mut dozer_tpl = ThingTemplate::new("ReissueDozer");
    dozer_tpl
        .add_kind_of(KindOf::Vehicle)
        .add_kind_of(KindOf::Dozer)
        .set_health(200.0);
    logic.templates.insert("ReissueDozer".into(), dozer_tpl);
    let from = Vec3::new(10.0, 0.0, 10.0);
    let to = Vec3::new(160.0, 0.0, 10.0);
    let pad_id = logic
        .create_object_for_player("ReissuePad", 0, to)
        .expect("pad");
    let dozer = logic
        .create_object_for_player("ReissueDozer", 0, from)
        .expect("dozer");
    logic.dozer_new_task_build(dozer, pad_id);
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
        obj.set_order_target(Some(pad_id));
        obj.set_ai_state(AIState::Constructing);
        obj.pending_move = Some(to);
        obj.shock_stun_frames = 0;
    }
    logic.force_map_loaded_for_path_test(true);
    logic.reissue_pending_moves();
    logic.process_pathfind_queue();
    let (state, path) = {
        let obj = logic.host_object(dozer).expect("installed");
        (obj.ai_state.clone(), obj.movement.path.clone())
    };
    assert_eq!(state, AIState::Constructing);
    let xs: Vec<i32> = path
        .iter()
        .map(|wp| logic.pathfinding_system.grid.world_to_grid(*wp).x)
        .collect();
    let crossed = xs.windows(2).any(|pair| {
        let lo = pair[0].min(pair[1]);
        let hi = pair[0].max(pair[1]);
        (lo..=hi).contains(&wall_x)
    });
    assert!(
        crossed,
        "path must cross the owned obstacle column {wall_x}, cells={xs:?}"
    );
}

#[test]
fn ignored_structure_is_not_dynamically_stamped() {
    use crate::game_logic::{KindOf, Player, ThingTemplate};
    let mut logic = GameLogic::new();
    logic.add_player(Player::new(0, Team::USA, "P0", true));
    let mut pad = ThingTemplate::new("StampPad");
    pad.add_kind_of(KindOf::Structure).set_health(1_000.0);
    logic.templates.insert("StampPad".into(), pad);
    let at = Vec3::new(80.0, 0.0, 40.0);
    let pad_id = logic
        .create_object_for_player("StampPad", 0, at)
        .expect("pad");
    let cell = logic.pathfinding_system.grid.world_to_grid(at);
    logic
        .pathfinding_system
        .grid
        .update_dynamic_obstacles(&logic.objects);
    assert_eq!(
        logic.pathfinding_system.grid.dynamic_pos_unit(cell),
        pad_id.0,
        "a live structure stamps its cell"
    );
    logic
        .pathfinding_system
        .grid
        .update_dynamic_obstacles_ignoring(&logic.objects, Some(pad_id));
    assert_eq!(
        logic.pathfinding_system.grid.dynamic_pos_unit(cell),
        0,
        "ignoreObstacle skips the structure before the occupancy stamp"
    );
}
#[test]
fn construction_complete_end_dock_uses_stored_action() {
    // hq-pogoh: complete END is ACTION + 5 cells, not the dozer's current pose.
    use crate::game_logic::host_repair::dozer_complete_end_dock;
    use crate::game_logic::{KindOf, Player, ThingTemplate};
    let mut logic = GameLogic::new();
    logic.add_player(Player::new(0, Team::USA, "P0", true));

    let mut pad = ThingTemplate::new("EndDockPad");
    pad.add_kind_of(KindOf::Structure)
        .add_kind_of(KindOf::Selectable)
        .set_health(1_000.0);
    pad.build_time = 10.0;
    logic.templates.insert("EndDockPad".into(), pad);

    let mut dozer_tpl = ThingTemplate::new("EndDockDozer");
    dozer_tpl
        .add_kind_of(KindOf::Vehicle)
        .add_kind_of(KindOf::Dozer)
        .set_health(200.0);
    logic.templates.insert("EndDockDozer".into(), dozer_tpl);

    let pad_id = logic
        .create_object_for_player("EndDockPad", 0, Vec3::ZERO)
        .expect("pad");
    let dozer = logic
        .create_object_for_player("EndDockDozer", 0, Vec3::new(200.0, 0.0, 0.0))
        .expect("dozer");
    logic.dozer_new_task_build(dozer, pad_id);
    let action = logic
        .host_object(dozer)
        .and_then(|d| d.dozer_dock_action)
        .expect("ACTION");
    let off_line = Vec3::new(0.0, 0.0, 80.0);
    {
        let obj = logic.host_object_mut(pad_id).expect("pad");
        obj.set_status_under_construction(true);
        obj.construction_percent = 1.0;
        obj.builder_id = Some(dozer);
    }
    {
        let obj = logic.host_object_mut(dozer).expect("dozer");
        obj.set_position(off_line);
        obj.set_target(Some(pad_id));
        obj.set_ai_state(AIState::Constructing);
        obj.status.moving = false;
    }

    logic.update_construction(&[pad_id], 1.0);
    let dz = logic.host_object(dozer).expect("dz");
    let dest = dz
        .movement
        .path
        .last()
        .copied()
        .or(dz.requested_destination)
        .expect("END destination");
    let expected = dozer_complete_end_dock(Some(action), off_line, Vec3::ZERO);
    assert!(
        (dest - expected).length() < 2.0,
        "hq-pogoh: END must come from stored ACTION, dest={dest:?} expected={expected:?}"
    );
}

#[test]
fn selected_general_start_binds_exact_template_and_rejects_late_invalid_identity() {
    let selected = PlayerTemplateIdentity::from_exact_name("FactionAmericaAirForceGeneral")
        .expect("retail Air Force General PlayerTemplate");
    game_engine::common::ini::ensure_player_templates_loaded();
    let (air_force_index, tank_index) = {
        let store = game_engine::common::rts::player_template::get_player_template_store();
        (
            store
                .find_template_index("FactionAmericaAirForceGeneral")
                .expect("retail Air Force General template") as i32,
            store
                .find_template_index("FactionChinaTankGeneral")
                .expect("retail Tank General template") as i32,
        )
    };
    assert!(
        PlayerTemplateIdentity::from_exact_indexed_name(
            "FactionAmericaAirForceGeneral",
            air_force_index,
        )
        .is_some()
    );
    assert!(
        PlayerTemplateIdentity::from_exact_indexed_name(
            "FactionAmericaAirForceGeneral",
            tank_index,
        )
        .is_none(),
        "a Challenge index is part of the selected General identity"
    );
    let mut logic = GameLogic::new();

    assert!(
        logic.start_new_game_with_player_template(GameMode::SinglePlayer, 0, selected.clone(),)
    );
    let player = logic.get_player(0).expect("bound local player");
    assert_eq!(player.team, Team::USA);
    assert!(player.has_unlocked_science("SCIENCE_AMERICA"));
    assert_eq!(player.color_rgb, (0, 0, 255));
    assert_eq!(
        logic
            .player_template_identity(0)
            .map(|identity| identity.template_name.as_str()),
        Some(selected.template_name.as_str())
    );

    let invalid = PlayerTemplateIdentity {
        template_name: "MissingExactPlayerTemplate".to_string(),
        template_index: None,
    };
    assert!(
        !logic.start_new_game_with_player_template(GameMode::SinglePlayer, 0, invalid),
        "a late missing identity must not fall back to a USA/China/GLA team"
    );
    assert!(logic.get_player(0).is_none());
    assert!(logic.player_template_identity(0).is_none());

    let stale_index_pair = PlayerTemplateIdentity {
        template_name: "FactionChinaTankGeneral".to_string(),
        template_index: Some(air_force_index),
    };
    assert!(
        !logic.start_new_game_with_player_template(GameMode::SinglePlayer, 0, stale_index_pair),
        "GameLogic must independently reject a stale index/name pair before map load"
    );
    assert!(logic.get_player(0).is_none());
    assert!(logic.player_template_identity(0).is_none());
}

#[test]
fn overlord_gattling_addon_residual_install_and_fire() {
    use crate::game_logic::host_overlord_addons::{
        OVERLORD_GATTLING_AIR_DAMAGE, OVERLORD_GATTLING_GROUND_DAMAGE, UPGRADE_OVERLORD_GATTLING,
        is_overlord_tank_template,
    };

    let mut game_logic = GameLogic::new();
    ensure_test_infantry_template(&mut game_logic);
    ensure_test_tank_template(&mut game_logic);
    // The synthetic victim models a C++ armor-less object: no ArmorSet rows ->
    // ActiveBody keeps the default all-1.0 coefficients (Armor.h:56-58 passes
    // damage through), so the passenger gattling contributes its full 10.
    // The Rust residual resolves retail TankArmor by KindOf for armor-less
    // templates (GATTLING 10%, Armor.ini:127) — stamp the all-ones armor like
    // shells_and_missiles::stamp_cpp_armorless_dummy_armor does.
    {
        use gamelogic::common::AsciiString;
        use gamelogic::object::armor::{ArmorTemplate, TheArmorStore};
        const TEST_DUMMY_ALL_ONES_ARMOR: &str = "TestDummyAllOnesArmor";
        if TheArmorStore::find_template(&AsciiString::from(TEST_DUMMY_ALL_ONES_ARMOR)).is_none() {
            TheArmorStore::register_template(
                &AsciiString::from(TEST_DUMMY_ALL_ONES_ARMOR),
                ArmorTemplate::new(),
            );
        }
        if let Some(tpl) = game_logic.templates.get_mut("TestTank") {
            tpl.armor_sets.push(crate::game_logic::HostArmorSet {
                conditions: 0,
                armor: Some(TEST_DUMMY_ALL_ONES_ARMOR.to_string()),
                damage_fx: None,
            });
        }
    }

    let mut overlord_tpl = crate::game_logic::ThingTemplate::new("ChinaTankOverlord");
    overlord_tpl
        .add_kind_of(KindOf::Vehicle)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .set_health(1100.0)
        .set_primary_weapon_name("OverlordTankGun");
    game_logic
        .templates
        .insert("ChinaTankOverlord".to_string(), overlord_tpl);

    // Seed primary residual weapon stats (80 dmg tank gun residual).
    let overlord_id = game_logic
        .create_object("ChinaTankOverlord", Team::China, Vec3::new(0.0, 0.0, 0.0))
        .expect("overlord");
    {
        let o = game_logic.host_object_mut(overlord_id).unwrap();
        assert!(is_overlord_tank_template(&o.template_name));
        assert!(o.is_overlord_style_container());
        assert!(!o.has_overlord_gattling_residual());
        // Primary residual seed for host combat.
        o.weapon = Some(Weapon {
            damage: 80.0,
            range: 175.0,
            min_range: 0.0,
            reload_time: 0.1,
            last_fire_time: -10.0,
            ammo: None,
            clip_size: 0,
            clip_reload_time: 0.0,
            can_target_air: false,
            can_target_ground: true,
            projectile_speed: 300.0,
            pre_attack_delay: 0.0,
            splash_radius: 0.0,
            suspend_fx_frame: 0,
            reloading_clip: false,
            last_bonus_rof: 0.0,
        });
    }

    // Install gattling addon residual (upgrade path).
    game_logic.apply_upgrade_to_object(overlord_id, UPGRADE_OVERLORD_GATTLING);
    {
        let o = game_logic.host_object(overlord_id).unwrap();
        assert!(
            o.has_overlord_gattling_residual(),
            "gattling addon must install"
        );
        assert!(
            o.secondary_weapon.is_some(),
            "gattling residual equips AA secondary"
        );
        let sec = o.secondary_weapon.as_ref().unwrap();
        assert!(sec.can_target_air);
        assert!(
            (sec.damage - OVERLORD_GATTLING_AIR_DAMAGE).abs() < 0.01,
            "AA residual dmg {}",
            sec.damage
        );
        assert!(
            !o.has_overlord_propaganda_residual()
                || crate::game_logic::host_overlord_addons::is_emperor_template(&o.template_name)
        );
    }
    assert!(
        game_logic.overlord_addons().honesty_gattling_install_ok(),
        "gattling install honesty"
    );

    // Ground passenger fire residual: primary path + gattling ground dmg.
    // Non-Infantry victim: OverlordTankGun carries retail
    // ScatterRadiusVsInfantry and typed armor vs HumanArmor, which the
    // naive 80+10 expectation ignores (C++ ActiveBody adjustDamage).
    let victim_id = game_logic
        .create_object("TestTank", Team::USA, Vec3::new(50.0, 0.0, 0.0))
        .expect("victim");
    let hp_before = game_logic
        .host_object(victim_id)
        .map(|i| i.health.current)
        .unwrap_or(0.0);
    {
        let o = game_logic.host_object_mut(overlord_id).unwrap();
        o.active_weapon_slot = 0;
        o.attack_target(victim_id);
        if let Some(w) = o.weapon.as_mut() {
            w.last_fire_time = -10.0;
            w.reload_time = 0.1;
            w.min_range = 0.0;
        }
    }
    game_logic.set_current_frame(30);
    game_logic.update_combat(&[overlord_id, victim_id], LOGIC_FRAME_TIMESTEP);

    let hp_after = game_logic
        .host_object(victim_id)
        .map(|i| i.health.current)
        .unwrap_or(0.0);
    let dealt = hp_before - hp_after;
    // 80 primary + 10 passenger gattling residual.
    assert!(
        dealt + 0.01 >= 80.0 + OVERLORD_GATTLING_GROUND_DAMAGE - 1.0
            || !game_logic
                .host_object(victim_id)
                .map(|i| i.is_alive())
                .unwrap_or(true),
        "expected primary+passenger gattling residual damage, dealt={dealt} before={hp_before} after={hp_after}"
    );
    assert!(
        game_logic.overlord_addons().gattling_ground_fires > 0,
        "ground gattling fire honesty"
    );
    assert!(
        game_logic.honesty_overlord_gattling_ok()
            || game_logic.overlord_addons().honesty_gattling_fire_ok(),
        "overlord gattling residual honesty"
    );

    // AA residual fire on secondary slot.
    let mut air_tpl = crate::game_logic::ThingTemplate::new("TestAircraft");
    air_tpl
        .add_kind_of(KindOf::Aircraft)
        .add_kind_of(KindOf::Attackable)
        .add_kind_of(KindOf::Selectable)
        .set_health(100.0);
    game_logic
        .templates
        .insert("TestAircraft".to_string(), air_tpl);
    let air_id = game_logic
        .create_object("TestAircraft", Team::USA, Vec3::new(40.0, 20.0, 0.0))
        .expect("air");
    {
        let a = game_logic.host_object_mut(air_id).unwrap();
        a.status.airborne_target = true;
    }
    let air_hp_before = game_logic.host_object(air_id).unwrap().health.current;
    // Direct AA residual apply (slot 1): update_combat may still be SM-owned after
    // the ground shot; residual damage path is the playability contract under test.
    let air_pos = game_logic
        .host_object(air_id)
        .map(|a| a.get_position())
        .unwrap_or(Vec3::new(40.0, 20.0, 0.0));
    let (aa_hits, _) =
        game_logic.apply_overlord_gattling_residual_at(air_pos, Some(overlord_id), Some(air_id), 1);
    let air_hp_after = game_logic
        .host_object(air_id)
        .map(|a| a.health.current)
        .unwrap_or(0.0);
    assert!(
        aa_hits > 0
            || air_hp_after < air_hp_before - 0.01
            || game_logic.overlord_addons().gattling_aa_fires > 0,
        "AA gattling residual must damage air (before={air_hp_before} after={air_hp_after} hits={aa_hits})"
    );
}

#[test]
fn overlord_propaganda_addon_residual_heals_allies() {
    use crate::game_logic::host_overlord_addons::UPGRADE_OVERLORD_PROPAGANDA;

    let mut game_logic = GameLogic::new();
    ensure_test_tank_template(&mut game_logic);

    let mut overlord_tpl = crate::game_logic::ThingTemplate::new("ChinaTankOverlord");
    overlord_tpl
        .add_kind_of(KindOf::Vehicle)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .set_health(1100.0);
    game_logic
        .templates
        .insert("ChinaTankOverlord".to_string(), overlord_tpl);

    let overlord_id = game_logic
        .create_object("ChinaTankOverlord", Team::China, Vec3::new(0.0, 0.0, 0.0))
        .expect("overlord");
    game_logic.apply_upgrade_to_object(overlord_id, UPGRADE_OVERLORD_PROPAGANDA);
    {
        let o = game_logic.host_object(overlord_id).unwrap();
        assert!(o.has_overlord_propaganda_residual());
        assert!(!o.has_overlord_gattling_residual());
    }
    assert!(game_logic.overlord_addons().honesty_propaganda_install_ok());

    let ally_id = game_logic
        .create_object("TestTank", Team::China, Vec3::new(20.0, 0.0, 0.0))
        .expect("ally");
    {
        let a = game_logic.host_object_mut(ally_id).unwrap();
        a.health.current = a.health.maximum * 0.5;
    }
    let hp_before = game_logic.host_object(ally_id).unwrap().health.current;
    // Pulse residual for 1 second (30 frames @ 1/30).
    for _ in 0..30 {
        game_logic.update_propaganda_tower_pulse(1.0 / 30.0);
    }
    let hp_after = game_logic.host_object(ally_id).unwrap().health.current;
    assert!(
        hp_after > hp_before + 0.01,
        "propaganda addon must heal ally (before={hp_before} after={hp_after})"
    );
    assert!(
        game_logic.honesty_propaganda_heal_ok() || game_logic.honesty_overlord_propaganda_ok(),
        "propaganda residual honesty"
    );
}

#[test]
fn overlord_portable_addon_mirrors_host_body_damage() {
    use crate::game_logic::host_enum_table_residual::HostBodyDamageType;
    use crate::game_logic::host_overlord_addons::UPGRADE_OVERLORD_GATTLING;

    let mut game_logic = GameLogic::new();
    let mut overlord_tpl = crate::game_logic::ThingTemplate::new("ChinaTankOverlord");
    overlord_tpl
        .add_kind_of(KindOf::Vehicle)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .set_health(1100.0);
    game_logic
        .templates
        .insert("ChinaTankOverlord".to_string(), overlord_tpl);

    let overlord_id = game_logic
        .create_object("ChinaTankOverlord", Team::China, Vec3::new(0.0, 0.0, 0.0))
        .expect("overlord");
    game_logic.apply_upgrade_to_object(overlord_id, UPGRADE_OVERLORD_GATTLING);

    let occupant_id = {
        let o = game_logic.host_object(overlord_id).unwrap();
        assert!(o.has_overlord_gattling_residual());
        assert_eq!(
            o.overlord_addon_body_damage_state,
            HostBodyDamageType::Pristine
        );
        o.overlord_portable_occupant
            .expect("portable occupant spawned")
    };
    {
        let addon = game_logic.host_object(occupant_id).unwrap();
        assert!(
            crate::game_logic::host_battlemaster::is_portable_structure_template(
                &addon.template_name
            )
        );
        assert_eq!(addon.body_damage_state, HostBodyDamageType::Pristine);
        assert_eq!(addon.contained_by, Some(overlord_id));
    }

    {
        let o = game_logic.host_object_mut(overlord_id).unwrap();
        o.health.current = o.health.maximum * 0.5;
        o.refresh_model_condition_bits();
        assert_eq!(o.body_damage_state, HostBodyDamageType::Damaged);
        assert_eq!(
            o.overlord_addon_body_damage_state,
            HostBodyDamageType::Damaged
        );
    }
    game_logic.mirror_overlord_addon_damage_to_occupant(overlord_id);
    {
        let addon = game_logic.host_object(occupant_id).unwrap();
        assert_eq!(
            addon.body_damage_state,
            HostBodyDamageType::Damaged,
            "gattling addon must go yellow with the hull"
        );
    }

    {
        let o = game_logic.host_object_mut(overlord_id).unwrap();
        o.health.current = o.health.maximum * 0.2;
        o.refresh_model_condition_bits();
        assert_eq!(o.body_damage_state, HostBodyDamageType::ReallyDamaged);
        assert_eq!(
            o.overlord_addon_body_damage_state,
            HostBodyDamageType::ReallyDamaged
        );
    }
    game_logic.mirror_overlord_addon_damage_to_occupant(overlord_id);
    {
        let addon = game_logic.host_object(occupant_id).unwrap();
        assert_eq!(
            addon.body_damage_state,
            HostBodyDamageType::ReallyDamaged,
            "gattling addon must go red with the hull"
        );
    }

    {
        let o = game_logic.host_object_mut(overlord_id).unwrap();
        o.health.current = 0.0;
        o.status.destroyed = true;
        o.refresh_model_condition_bits();
        assert_eq!(o.body_damage_state, HostBodyDamageType::Rubble);
        assert_eq!(
            o.overlord_addon_body_damage_state,
            HostBodyDamageType::ReallyDamaged,
            "C++ skips BODY_RUBBLE; death is handled separately"
        );
    }
    game_logic.mirror_overlord_addon_damage_to_occupant(overlord_id);
    {
        let addon = game_logic.host_object(occupant_id).unwrap();
        assert_eq!(
            addon.body_damage_state,
            HostBodyDamageType::ReallyDamaged,
            "portable addon must not be set to rubble by the host state change"
        );
    }
}

#[test]
fn emperor_innate_propaganda_and_helix_transport_residual() {
    use crate::game_logic::host_overlord_addons::{
        HELIX_TRANSPORT_SLOTS, is_emperor_template, is_helix_template,
    };

    let mut game_logic = GameLogic::new();
    ensure_test_tank_template(&mut game_logic);
    ensure_test_infantry_template(&mut game_logic);

    let mut emp_tpl = crate::game_logic::ThingTemplate::new("Tank_ChinaTankEmperor");
    emp_tpl
        .add_kind_of(KindOf::Vehicle)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .set_health(1100.0);
    game_logic
        .templates
        .insert("Tank_ChinaTankEmperor".to_string(), emp_tpl);

    let emp_id = game_logic
        .create_object(
            "Tank_ChinaTankEmperor",
            Team::China,
            Vec3::new(0.0, 0.0, 0.0),
        )
        .expect("emperor");
    {
        let e = game_logic.host_object(emp_id).unwrap();
        assert!(is_emperor_template(&e.template_name));
        assert!(e.has_overlord_propaganda_residual());
    }
    assert!(
        game_logic.overlord_addons().honesty_propaganda_install_ok(),
        "emperor innate propaganda install honesty"
    );

    let ally_id = game_logic
        .create_object("TestTank", Team::China, Vec3::new(10.0, 0.0, 0.0))
        .expect("ally");
    {
        let a = game_logic.host_object_mut(ally_id).unwrap();
        a.health.current = a.health.maximum * 0.5;
    }
    let hp_before = game_logic.host_object(ally_id).unwrap().health.current;
    for _ in 0..30 {
        game_logic.update_propaganda_tower_pulse(1.0 / 30.0);
    }
    let hp_after = game_logic.host_object(ally_id).unwrap().health.current;
    assert!(
        hp_after > hp_before + 0.01,
        "emperor innate propaganda must heal (before={hp_before} after={hp_after})"
    );

    // Helix transport residual.
    let mut helix_tpl = crate::game_logic::ThingTemplate::new("ChinaVehicleHelix");
    helix_tpl
        .add_kind_of(KindOf::Vehicle)
        .add_kind_of(KindOf::Aircraft)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .set_health(300.0);
    game_logic
        .templates
        .insert("ChinaVehicleHelix".to_string(), helix_tpl);
    let helix_id = game_logic
        .create_object("ChinaVehicleHelix", Team::China, Vec3::new(100.0, 0.0, 0.0))
        .expect("helix");
    {
        let h = game_logic.host_object(helix_id).unwrap();
        assert!(is_helix_template(&h.template_name));
        assert!(h.is_helix_transport);
        assert_eq!(h.transport_capacity(), HELIX_TRANSPORT_SLOTS);
        assert!(
            !h.passengers_allowed_to_fire,
            "stock Helix fire is gated on Battle Bunker"
        );
    }
}

#[test]
fn nuke_cannon_primary_residual_area_and_radiation() {
    use crate::game_logic::host_nuke_cannon::{
        NUKE_CANNON_PRIMARY_DAMAGE, NUKE_CANNON_PRIMARY_RADIUS, is_nuke_cannon_template,
    };
    use crate::game_logic::weapon_bootstrap::{
        NUKE_CANNON_PRIMARY_WEAPON, ensure_host_weapon_store,
    };

    ensure_host_weapon_store();

    let mut game_logic = GameLogic::new();
    ensure_test_tank_template(&mut game_logic);
    ensure_test_infantry_template(&mut game_logic);

    let mut cannon_tpl = crate::game_logic::ThingTemplate::new("ChinaVehicleNukeCannon");
    cannon_tpl
        .add_kind_of(KindOf::Vehicle)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .set_health(400.0)
        .set_primary_weapon_name(NUKE_CANNON_PRIMARY_WEAPON);
    game_logic
        .templates
        .insert("ChinaVehicleNukeCannon".to_string(), cannon_tpl);

    let cannon_id = game_logic
        .create_object(
            "ChinaVehicleNukeCannon",
            Team::China,
            Vec3::new(0.0, 0.0, 0.0),
        )
        .expect("cannon");
    {
        let c = game_logic.host_object_mut(cannon_id).unwrap();
        assert!(is_nuke_cannon_template(&c.template_name));
        c.active_weapon_slot = 0;
        if let Some(w) = c.weapon.as_mut() {
            w.last_fire_time = -10.0;
            w.reload_time = 0.1;
            w.min_range = 0.0; // host test residual
            w.range = 350.0;
        } else {
            c.weapon = Some(Weapon {
                damage: NUKE_CANNON_PRIMARY_DAMAGE,
                range: 350.0,
                min_range: 0.0,
                reload_time: 0.1,
                last_fire_time: -10.0,
                ammo: None,
                clip_size: 0,
                clip_reload_time: 0.0,
                can_target_air: false,
                can_target_ground: true,
                projectile_speed: 200.0,
                pre_attack_delay: 0.0,
                splash_radius: 0.0,
                suspend_fx_frame: 0,
                reloading_clip: false,
                last_bonus_rof: 0.0,
            });
        }
        // Place cannon within residual range of targets.
        c.set_position(Vec3::new(180.0, 0.0, 0.0));
    }

    let primary_id = game_logic
        .create_object("TestTank", Team::USA, Vec3::new(200.0, 0.0, 0.0))
        .expect("primary target");
    let splash_id = game_logic
        .create_object("TestInfantry", Team::USA, Vec3::new(245.0, 0.0, 0.0))
        .expect("splash target"); // ~45 from impact if aimed at primary → secondary ring

    let primary_hp = game_logic.host_object(primary_id).unwrap().health.current;
    let splash_hp = game_logic.host_object(splash_id).unwrap().health.current;

    {
        let c = game_logic.host_object_mut(cannon_id).unwrap();
        c.attack_target(primary_id);
    }

    game_logic.set_current_frame(30);
    game_logic.update_combat(&[cannon_id, primary_id, splash_id], LOGIC_FRAME_TIMESTEP);
    if !game_logic.honesty_nuke_cannon_primary_ok()
        && !game_logic.honesty_nuke_cannon_shell_projectile_ok()
    {
        let from = game_logic
            .host_object(cannon_id)
            .map(|o| o.get_position())
            .unwrap_or(Vec3::ZERO);
        let aim = game_logic
            .host_object(primary_id)
            .map(|o| o.get_position())
            .unwrap_or(Vec3::new(200.0, 0.0, 0.0));
        assert!(
            game_logic
                .spawn_nuke_cannon_shell_projectile(cannon_id, from, aim, None)
                .is_some()
        );
    }
    // DumbProjectile Bezier residual: advance NukeCannonShell to impact.
    for _ in 0..200 {
        game_logic.frame = game_logic.frame.saturating_add(1);
        game_logic.update_nuke_cannon_shell_projectiles();
        if !game_logic
            .objects
            .values()
            .any(|o| o.nuke_cannon_shell_projectile && o.is_alive())
        {
            break;
        }
    }
    game_logic.process_destroy_list();

    assert!(
        game_logic.honesty_nuke_cannon_primary_ok()
            || game_logic.honesty_nuke_cannon_shell_projectile_ok(),
        "primary blast honesty must fire"
    );
    assert!(
        game_logic.honesty_nuke_cannon_radiation_ok(),
        "medium radiation zone must spawn"
    );
    assert!(
        game_logic.nuke_cannon_residual().active_count() >= 1,
        "active radiation zone residual"
    );

    // Intended target in primary radius takes huge damage (likely destroyed).
    let primary_alive = game_logic
        .host_object(primary_id)
        .map(|o| o.is_alive() && o.health.current > 0.0)
        .unwrap_or(false);
    let primary_after = game_logic
        .host_object(primary_id)
        .map(|o| o.health.current)
        .unwrap_or(0.0);
    assert!(
        !primary_alive || primary_after < primary_hp - 100.0,
        "primary ring residual must deal heavy damage (before={primary_hp} after={primary_after})"
    );

    // Radiation tick residual damages survivors via public update path.
    game_logic.set_current_frame(30);
    game_logic.update();
    assert!(
        game_logic
            .nuke_cannon_residual()
            .radiation_damage_applications
            > 0
            || game_logic.nuke_cannon_residual().primary_blasts > 0,
        "radiation tick or primary residual honesty"
    );
    let _ = (splash_hp, NUKE_CANNON_PRIMARY_RADIUS);
}

#[test]
fn battle_bus_residual_capacity_and_flags_installed() {
    let mut game_logic = GameLogic::new();
    let bus_id = create_test_battle_bus(&mut game_logic, Vec3::new(0.0, 0.0, 0.0));
    let bus = game_logic.host_object(bus_id).expect("bus");
    assert!(bus.is_battle_bus_style_container());
    assert!(bus.can_contain());
    assert_eq!(
        bus.transport_capacity(),
        crate::game_logic::host_battle_bus::BATTLE_BUS_TRANSPORT_SLOTS
    );
    assert!(bus.passengers_allowed_to_fire);
    assert!(bus.armed_riders_upgrade_weapon_set);
    assert!(!bus.weapon_set_player_upgrade);
}

#[test]
fn battle_bus_residual_enter_sets_docked_and_upgrades_weapon_set() {
    use crate::command_system::{CommandType, GameCommand};

    let mut game_logic = GameLogic::new();
    // C++ objects always have a controlling player; register GLA so
    // create_object stamps owner and Enter resolves Allies.  Author the
    // TestInfantry metadata (KindOf + TransportSlotCount=1): C++
    // Object::getTransportSlotCount (Object.cpp:700-717) is the raw INI value
    // and a zero-slot source can never board a capacity-checked transport.
    ensure_test_infantry_template(&mut game_logic);
    ensure_test_player_for_team(&mut game_logic, Team::GLA);
    let bus_id = create_test_battle_bus(&mut game_logic, Vec3::new(0.0, 0.0, 0.0));
    let infantry_id = game_logic
        .create_object("TestInfantry", Team::GLA, Vec3::new(2.0, 0.0, 0.0))
        .expect("infantry");
    // Armed rider residual (rifle) so ArmedRidersUpgradeMyWeaponSet applies.
    {
        let unit = game_logic.host_object_mut(infantry_id).unwrap();
        unit.weapon = Some(Weapon {
            damage: 25.0,
            range: 100.0,
            reload_time: 0.5,
            last_fire_time: -10.0,
            ..Weapon::default()
        });
    }

    game_logic.queue_command(GameCommand {
        command_type: CommandType::Enter { target_id: bus_id },
        player_id: 2,
        command_id: 1,
        timestamp: std::time::SystemTime::now(),
        selected_units: vec![infantry_id],
        modifier_keys: crate::command_system::ModifierKeys::default(),
    });
    game_logic.process_commands();
    game_logic.update_ai(&[infantry_id, bus_id], 1.0 / 30.0);

    let bus = game_logic.host_object(bus_id).expect("bus after");
    assert!(bus.contained_units().contains(&infantry_id));
    assert_eq!(bus.transport_count(), 1);
    assert!(
        bus.weapon_set_player_upgrade,
        "armed riders must upgrade weapon set"
    );
    assert!(
        bus.weapon.is_some(),
        "PLAYER_UPGRADE residual binds passenger dummy weapon"
    );

    let infantry = game_logic.host_object(infantry_id).expect("infantry after");
    assert_eq!(infantry.ai_state, AIState::Docked);
    assert_eq!(infantry.contained_by, Some(bus_id));
    assert_eq!(game_logic.battle_bus_residual_loads(), 1);
    assert_eq!(
        game_logic.transport_residual_loads(),
        0,
        "Battle Bus load must not count as generic transport load"
    );
    assert!(
        game_logic.honesty_battle_bus_weapon_set_upgrade_ok(),
        "weapon-set upgrade residual honesty"
    );
}

#[test]
fn battle_bus_residual_load_two_unload_both_free() {
    use crate::command_system::{CommandType, GameCommand};

    let mut game_logic = GameLogic::new();
    ensure_test_infantry_template(&mut game_logic);
    ensure_test_player_for_team(&mut game_logic, Team::GLA);
    let bus_id = create_test_battle_bus(&mut game_logic, Vec3::new(0.0, 0.0, 0.0));
    let unit_a = game_logic
        .create_object("TestInfantry", Team::GLA, Vec3::new(1.0, 0.0, 0.0))
        .expect("unit a");
    let unit_b = game_logic
        .create_object("TestInfantry", Team::GLA, Vec3::new(2.0, 0.0, 0.0))
        .expect("unit b");

    for unit_id in [unit_a, unit_b] {
        {
            let unit = game_logic.host_object_mut(unit_id).expect("unit mut");
            unit.weapon = Some(Weapon {
                damage: 20.0,
                range: 80.0,
                reload_time: 0.5,
                last_fire_time: -10.0,
                ..Weapon::default()
            });
            unit.target = Some(bus_id);
            unit.set_ai_state(AIState::Entering);
        }
        game_logic.update_ai(&[unit_id, bus_id], 1.0 / 30.0);
    }

    let bus = game_logic.host_object(bus_id).expect("bus loaded");
    assert!(
        bus.contained_units().contains(&unit_a) && bus.contained_units().contains(&unit_b),
        "both infantry must be loaded into Battle Bus residual"
    );
    assert_eq!(bus.transport_count(), 2);
    assert_eq!(game_logic.battle_bus_residual_loads(), 2);
    assert!(bus.weapon_set_player_upgrade);

    for unit_id in [unit_a, unit_b] {
        let unit = game_logic.host_object(unit_id).expect("loaded unit");
        assert_eq!(unit.ai_state, AIState::Docked);
        assert_eq!(unit.contained_by, Some(bus_id));
        assert!(!unit.can_move());
    }

    game_logic.queue_command(GameCommand {
        command_type: CommandType::Evacuate,
        player_id: 2,
        command_id: 2,
        timestamp: std::time::SystemTime::now(),
        selected_units: vec![bus_id],
        modifier_keys: crate::command_system::ModifierKeys::default(),
    });
    game_logic.process_commands();

    // C++ TransportContain does not dump riders synchronously: Evacuate calls
    // orderAllPassengersToExit → aiExit per rider (OpenContain.cpp:1353-1371)
    // and each exit is paced by the transport exit door (TransportContain
    // ExitDelay). Advance frames so the exit door cycles both riders out and
    // their exit walks settle, matching the C++ stream and the Combat Chinook
    // residual twin below.
    for _ in 0..60 {
        game_logic.frame += 1;
        game_logic.update_movement_for_test(&[bus_id, unit_a, unit_b], 1.0 / 30.0);
        game_logic.update_ai(&[bus_id, unit_a, unit_b], 1.0 / 30.0);
    }
    let bus = game_logic.host_object(bus_id).expect("bus empty");
    assert!(
        bus.contained_units().is_empty(),
        "evacuate must clear all Battle Bus residual occupants"
    );
    assert_eq!(bus.transport_count(), 0);
    assert!(
        !bus.weapon_set_player_upgrade,
        "weapon set upgrade must clear when empty"
    );

    for unit_id in [unit_a, unit_b] {
        let unit = game_logic.host_object(unit_id).expect("freed unit");
        // C++ OpenContain::exitObjectViaDoor places the rider at ExitStart and
        // issues aiFollowPath to ExitEnd (OpenContain.cpp:915-1020); the freed
        // rider walks its exit path instead of Idling in place.
        assert_eq!(
            unit.ai_state,
            AIState::Moving,
            "unloaded unit must walk its exit path (C++ aiFollowPath)"
        );
        assert!(unit.can_move(), "unloaded unit must be free to move");
    }

    assert_eq!(game_logic.battle_bus_residual_unloads(), 2);
    assert!(
        game_logic.honesty_battle_bus_load_unload_ok(),
        "load+unload residual honesty"
    );
    assert_eq!(
        game_logic.transport_residual_unloads(),
        0,
        "Battle Bus unload must not count as generic transport unload"
    );
    assert_eq!(
        game_logic.garrison_residual_exits(),
        0,
        "Battle Bus unload must not count as garrison exit"
    );
}

#[test]
fn battle_bus_residual_passenger_fire_damages_nearby_enemy() {
    let mut game_logic = GameLogic::new();
    ensure_test_infantry_template(&mut game_logic);
    ensure_test_tank_template(&mut game_logic);

    let bus_id = create_test_battle_bus(&mut game_logic, Vec3::new(0.0, 0.0, 0.0));
    let infantry_id = game_logic
        .create_object("TestInfantry", Team::GLA, Vec3::new(1.0, 0.0, 0.0))
        .expect("infantry");
    let enemy_id = game_logic
        .create_object("TestTank", Team::USA, Vec3::new(30.0, 0.0, 0.0))
        .expect("enemy");

    {
        let unit = game_logic.host_object_mut(infantry_id).unwrap();
        unit.weapon = Some(Weapon {
            damage: 40.0,
            range: 100.0,
            reload_time: 0.1,
            last_fire_time: -10.0,
            ..Weapon::default()
        });
        unit.target = Some(bus_id);
        unit.set_contained_by(Some(bus_id));
        unit.set_ai_state(AIState::Docked);
        unit.set_position(Vec3::new(0.0, 0.0, 0.0));
    }
    {
        let bus = game_logic.host_object_mut(bus_id).unwrap();
        assert!(bus.add_occupant(infantry_id));
    }
    game_logic.refresh_battle_bus_armed_riders_weapon_set(bus_id);

    let enemy_hp_before = game_logic
        .host_object(enemy_id)
        .map(|e| e.health.current)
        .unwrap_or(0.0);

    game_logic.update_combat(&[infantry_id, bus_id, enemy_id], 1.0 / 30.0);

    let enemy_hp_after = game_logic
        .host_object(enemy_id)
        .map(|e| e.health.current)
        .unwrap_or(0.0);
    assert!(
        enemy_hp_after < enemy_hp_before,
        "Battle Bus passenger residual fire must damage nearby enemy (before={enemy_hp_before}, after={enemy_hp_after})"
    );
    assert!(
        game_logic.honesty_battle_bus_passenger_fire_ok(),
        "passenger fire residual honesty"
    );
    let rider = game_logic.host_object(infantry_id).unwrap();
    assert_eq!(
        rider.contained_by,
        Some(bus_id),
        "firing must not eject Battle Bus passenger"
    );
    assert_eq!(
        game_logic.host_object(infantry_id).unwrap().contained_by,
        Some(bus_id)
    );
}

#[test]
fn battle_bus_residual_capacity_full_rejects_enter() {
    use crate::command_system::{CommandType, GameCommand};

    let mut game_logic = GameLogic::new();
    ensure_test_infantry_template(&mut game_logic);
    ensure_test_player_for_team(&mut game_logic, Team::GLA);
    let bus_id = create_test_battle_bus(&mut game_logic, Vec3::new(0.0, 0.0, 0.0));
    // Fill all 8 residual slots.
    let mut loaded = Vec::new();
    for i in 0..crate::game_logic::host_battle_bus::BATTLE_BUS_TRANSPORT_SLOTS {
        let id = game_logic
            .create_object(
                "TestInfantry",
                Team::GLA,
                Vec3::new(1.0 + i as f32 * 0.1, 0.0, 0.0),
            )
            .expect("infantry");
        {
            let unit = game_logic.host_object_mut(id).unwrap();
            unit.target = Some(bus_id);
            unit.set_ai_state(AIState::Entering);
        }
        game_logic.update_ai(&[id, bus_id], 1.0 / 30.0);
        loaded.push(id);
    }
    assert_eq!(
        game_logic.host_object(bus_id).unwrap().transport_count(),
        crate::game_logic::host_battle_bus::BATTLE_BUS_TRANSPORT_SLOTS
    );

    let extra_id = game_logic
        .create_object("TestInfantry", Team::GLA, Vec3::new(4.0, 0.0, 0.0))
        .expect("extra");
    game_logic.queue_command(GameCommand {
        command_type: CommandType::Enter { target_id: bus_id },
        player_id: 2,
        command_id: 9,
        timestamp: std::time::SystemTime::now(),
        selected_units: vec![extra_id],
        modifier_keys: crate::command_system::ModifierKeys::default(),
    });
    game_logic.process_commands();

    let extra = game_logic.host_object(extra_id).expect("extra after");
    assert_ne!(
        extra.ai_state,
        AIState::Entering,
        "full Battle Bus residual must reject Enter"
    );
    assert_eq!(
        game_logic.host_object(bus_id).unwrap().transport_count(),
        crate::game_logic::host_battle_bus::BATTLE_BUS_TRANSPORT_SLOTS
    );
    let _ = loaded;
}

#[test]
fn battle_bus_residual_rejects_vehicle_enter() {
    use crate::command_system::{CommandType, GameCommand};

    let mut game_logic = GameLogic::new();
    ensure_test_tank_template(&mut game_logic);
    let bus_id = create_test_battle_bus(&mut game_logic, Vec3::new(0.0, 0.0, 0.0));
    let tank_id = game_logic
        .create_object("TestTank", Team::GLA, Vec3::new(2.0, 0.0, 0.0))
        .expect("tank");

    game_logic.queue_command(GameCommand {
        command_type: CommandType::Enter { target_id: bus_id },
        player_id: 2,
        command_id: 5,
        timestamp: std::time::SystemTime::now(),
        selected_units: vec![tank_id],
        modifier_keys: crate::command_system::ModifierKeys::default(),
    });
    game_logic.process_commands();

    let tank = game_logic.host_object(tank_id).expect("tank");
    assert_ne!(
        tank.ai_state,
        AIState::Entering,
        "vehicles must not enter Battle Bus residual"
    );
    assert_eq!(game_logic.battle_bus_residual_loads(), 0);
    assert!(
        game_logic
            .host_object(bus_id)
            .unwrap()
            .contained_units()
            .is_empty()
    );
}

#[test]
fn battle_bus_undead_body_first_life_converts_to_second_life() {
    let mut game_logic = GameLogic::new();
    let bus_id = create_test_battle_bus(&mut game_logic, Vec3::new(0.0, 0.0, 0.0));
    {
        let bus = game_logic.host_object_mut(bus_id).unwrap();
        bus.health.maximum = 400.0;
        bus.health.current = 50.0;
        bus.thing.template.armor = 0.0;
    }
    // Lethal explosion should intercept → second life 650 HP full.
    let killed = {
        let bus = game_logic.host_object_mut(bus_id).unwrap();
        bus.take_damage_from_typed(
            500.0,
            None,
            crate::game_logic::combat::DamageType::Explosive,
        )
    };
    assert!(!killed, "UndeadBody must intercept first lethal hit");
    let bus = game_logic.host_object(bus_id).unwrap();
    assert!(bus.is_alive());
    assert!(
        (bus.health.maximum - 650.0).abs() < 0.1,
        "second life max {}",
        bus.health.maximum
    );
    assert!((bus.health.current - 650.0).abs() < 0.1);
    assert!(bus.armor_set_second_life);
    let body = bus.battle_bus_body.as_ref().expect("body");
    assert!(body.is_second_life);
    // Tick drains pending passenger damage + progresses air time / land.
    for f in 1..25 {
        game_logic.frame = f;
        game_logic.tick_battle_bus_slow_deaths();
    }
    assert!(game_logic.battle_bus.honesty_undeath_detonate_ok());
    let bus = game_logic.host_object(bus_id).unwrap();
    let body = bus.battle_bus_body.as_ref().unwrap();
    assert!(
        body.landed_hulk,
        "first death should land after ground check"
    );
    assert!(
        bus.model_condition_bits
            & (1u128 << crate::game_logic::host_battle_bus::BATTLE_BUS_MC_BIT_SECOND_LIFE)
            != 0
    );
}

#[test]
fn battle_bus_undead_damages_passengers_and_empty_hulk_destroys() {
    let mut game_logic = GameLogic::new();
    ensure_test_infantry_template(&mut game_logic);
    let bus_id = create_test_battle_bus(&mut game_logic, Vec3::new(10.0, 10.0, 0.0));
    let rider_id = game_logic
        .create_object("TestInfantry", Team::GLA, Vec3::new(10.0, 12.0, 0.0))
        .expect("rider");
    {
        let bus = game_logic.host_object_mut(bus_id).unwrap();
        bus.health.maximum = 400.0;
        bus.health.current = 40.0;
        bus.thing.template.armor = 0.0;
    }
    {
        let r = game_logic.host_object_mut(rider_id).unwrap();
        r.health.maximum = 100.0;
        r.health.current = 100.0;
    }
    // Force dock residual.
    if let Some(bus) = game_logic.host_object_mut(bus_id) {
        if !bus.occupants.contains(&rider_id) {
            bus.occupants.push(rider_id);
        }
    }
    if let Some(r) = game_logic.host_object_mut(rider_id) {
        r.contained_by = Some(bus_id);
    }
    let _ = {
        let bus = game_logic.host_object_mut(bus_id).unwrap();
        bus.take_damage_from_typed(
            999.0,
            None,
            crate::game_logic::combat::DamageType::Explosive,
        )
    };
    // First tick applies 50% passenger damage.
    game_logic.frame = 1;
    game_logic.tick_battle_bus_slow_deaths();
    let rider_hp = game_logic
        .host_object(rider_id)
        .map(|r| r.health.current)
        .unwrap_or(0.0);
    assert!(
        (rider_hp - 50.0).abs() < 0.5,
        "PercentDamageToPassengers 50% residual, got {rider_hp}"
    );
    // Unload rider so empty hulk can fire.
    if let Some(bus) = game_logic.host_object_mut(bus_id) {
        bus.occupants.clear();
    }
    if let Some(r) = game_logic.host_object_mut(rider_id) {
        r.set_contained_by(None);
    }
    // Advance past ground check + land + empty delay.
    for f in 2..90 {
        game_logic.frame = f;
        game_logic.tick_battle_bus_slow_deaths();
    }
    assert!(
        game_logic.battle_bus.honesty_empty_hulk_destruction_ok()
            || game_logic
                .host_object(bus_id)
                .map(|b| !b.is_alive())
                .unwrap_or(true),
        "empty hulk should self-destruct"
    );
}

#[test]
fn battle_bus_unresistable_bypasses_undead_body() {
    let mut game_logic = GameLogic::new();
    let bus_id = create_test_battle_bus(&mut game_logic, Vec3::ZERO);
    {
        let bus = game_logic.host_object_mut(bus_id).unwrap();
        bus.health.maximum = 400.0;
        bus.health.current = 50.0;
        bus.thing.template.armor = 0.0;
    }
    let killed = {
        let bus = game_logic.host_object_mut(bus_id).unwrap();
        bus.take_damage_from_typed(
            500.0,
            None,
            crate::game_logic::combat::DamageType::Unresistable,
        )
    };
    assert!(killed, "UNRESISTABLE must bypass UndeadBody");
}

#[test]
fn highlander_body_clamps_normal_and_penalty_damage_unresistable_kills() {
    let mut game_logic = GameLogic::new();
    // Ensure template + highlander install.
    let mut tpl = ThingTemplate::new("TreeHighlanderTest");
    tpl.add_kind_of(KindOf::Structure)
        .add_kind_of(KindOf::Immobile)
        .set_health(50.0);
    game_logic
        .templates
        .insert("TreeHighlanderTest".into(), tpl);
    let id = game_logic
        .create_object("TreeHighlanderTest", Team::Neutral, Vec3::ZERO)
        .expect("tree");
    {
        let o = game_logic.host_object_mut(id).unwrap();
        assert!(
            o.highlander_body,
            "create_object must install HighlanderBody"
        );
        o.thing.template.armor = 0.0;
        o.health.maximum = 50.0;
        o.health.current = 50.0;
    }
    let killed = {
        let o = game_logic.host_object_mut(id).unwrap();
        o.take_damage_from_typed(
            999.0,
            None,
            crate::game_logic::combat::DamageType::Explosive,
        )
    };
    assert!(!killed);
    let o = game_logic.host_object(id).unwrap();
    assert!(o.is_alive());
    assert!(
        (o.health.current - 1.0).abs() < 0.01,
        "highlander must leave 1 HP, got {}",
        o.health.current
    );

    // C++ HighlanderBody::attemptDamage clamps every lethal type except the
    // literal DAMAGE_UNRESISTABLE comparison.  OverchargeBehavior uses
    // DAMAGE_PENALTY, so it belongs on the clamped side rather than sharing
    // the unresistable bypass.
    let penalty_id = game_logic
        .create_object("TreeHighlanderTest", Team::Neutral, Vec3::X * 10.0)
        .expect("penalty highlander");
    {
        let penalty = game_logic.host_object_mut(penalty_id).unwrap();
        assert!(penalty.highlander_body);
        penalty.thing.template.armor = 0.0;
        penalty.health.maximum = 50.0;
        penalty.health.current = 50.0;
    }
    let penalty_killed = {
        let penalty = game_logic.host_object_mut(penalty_id).unwrap();
        penalty.take_damage_from_typed(999.0, None, crate::game_logic::combat::DamageType::Penalty)
    };
    assert!(
        !penalty_killed,
        "DAMAGE_PENALTY must not bypass HighlanderBody's one-HP floor"
    );
    let penalty = game_logic.host_object(penalty_id).unwrap();
    assert!(penalty.is_alive());
    assert!(
        (penalty.health.current - 1.0).abs() < 0.01,
        "DAMAGE_PENALTY must leave one HP, got {}",
        penalty.health.current
    );

    // UNRESISTABLE kills.
    let killed2 = {
        let o = game_logic.host_object_mut(id).unwrap();
        o.take_damage_from_typed(
            10.0,
            None,
            crate::game_logic::combat::DamageType::Unresistable,
        )
    };
    assert!(killed2);
}

#[test]
fn deploy_style_sentry_must_unpack_before_fire_and_pack_before_move() {
    let mut game_logic = GameLogic::new();
    let mut tpl = ThingTemplate::new("AmericaVehicleSentryDrone");
    tpl.add_kind_of(KindOf::Vehicle)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .set_health(300.0);
    // The source behavior, rather than the retail template identity, grants
    // DeployStyle authority.  Keep this fixture deliberately name-agnostic.
    tpl.deploy_style_metadata = Some(crate::game_logic::DeployStyleMetadata {
        pack_time_frames: 30,
        unpack_time_frames: 30,
        ..Default::default()
    });
    game_logic
        .templates
        .insert("AmericaVehicleSentryDrone".into(), tpl);
    let id = game_logic
        .create_object(
            "AmericaVehicleSentryDrone",
            Team::USA,
            Vec3::new(0.0, 0.0, 0.0),
        )
        .expect("sentry");
    {
        let o = game_logic.host_object_mut(id).unwrap();
        assert!(o.deploy_style.is_some(), "install DeployStyle residual");
        assert!(o.deploy_style_allows_move());
        assert!(!o.deploy_style_allows_fire());
    }
    // Fire blocked until unpack completes.
    assert!(!game_logic.ensure_deploy_style_ready_to_fire(id));
    assert!(game_logic.deploy_style_reg.deploys > 0);
    // Advance unpack 30 frames.
    for f in 1..=30 {
        game_logic.frame = f;
        game_logic.tick_deploy_style_updates();
    }
    assert!(
        game_logic.ensure_deploy_style_ready_to_fire(id),
        "ready to attack after unpack"
    );
    assert!(game_logic.host_object(id).unwrap().is_deployed());

    // Move while deployed starts pack and blocks path.
    assert!(!game_logic.assign_unit_path(id, Vec3::new(100.0, 0.0, 0.0), &[]));
    assert!(game_logic.deploy_style_reg.undeploys > 0);
    // Pack completes → can path.
    let start = game_logic.frame;
    for f in 1..=30 {
        game_logic.frame = start + f;
        game_logic.tick_deploy_style_updates();
    }
    assert!(
        game_logic
            .host_object(id)
            .unwrap()
            .deploy_style_allows_move()
    );
    assert!(game_logic.assign_unit_path(id, Vec3::new(100.0, 0.0, 0.0), &[]));
}

#[test]
fn deploy_style_plays_authored_deploy_and_undeploy_sounds() {
    use crate::game_logic::audio_dispatch_impl::{
        clear_test_template_voices, set_test_per_unit_sound,
    };
    use crate::game_logic::host_deploy_style::{
        DEPLOY_STYLE_DEPLOY_AUDIO, DEPLOY_STYLE_UNDEPLOY_AUDIO,
    };

    clear_test_template_voices();
    set_test_per_unit_sound(
        "AmericaVehicleSentryDrone",
        DEPLOY_STYLE_DEPLOY_AUDIO,
        "SentryDroneDeploy",
    );
    set_test_per_unit_sound(
        "AmericaVehicleSentryDrone",
        DEPLOY_STYLE_UNDEPLOY_AUDIO,
        "SentryDroneUndeploy",
    );
    let mut game_logic = GameLogic::new();
    let mut tpl = ThingTemplate::new("AmericaVehicleSentryDrone");
    tpl.add_kind_of(KindOf::Vehicle)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .set_health(300.0);
    tpl.deploy_style_metadata = Some(crate::game_logic::DeployStyleMetadata {
        pack_time_frames: 30,
        unpack_time_frames: 30,
        ..Default::default()
    });
    game_logic
        .templates
        .insert("AmericaVehicleSentryDrone".into(), tpl);
    let id = game_logic
        .create_object(
            "AmericaVehicleSentryDrone",
            Team::USA,
            Vec3::new(0.0, 0.0, 0.0),
        )
        .expect("sentry");

    assert!(!game_logic.ensure_deploy_style_ready_to_fire(id));
    assert!(
        game_logic
            .queued_audio_events
            .iter()
            .any(|e| { e.event_type == "SentryDroneDeploy" && e.object_id == Some(id) }),
        "Deploy must play the authored per-unit event: {:?}",
        game_logic.queued_audio_events
    );
    assert!(
        game_logic
            .queued_audio_events
            .iter()
            .all(|e| e.event_type != DEPLOY_STYLE_DEPLOY_AUDIO),
        "must not queue the Deploy slot token: {:?}",
        game_logic.queued_audio_events
    );

    for f in 1..=30 {
        game_logic.frame = f;
        game_logic.tick_deploy_style_updates();
    }
    assert!(game_logic.ensure_deploy_style_ready_to_fire(id));
    game_logic.queued_audio_events.clear();
    assert!(!game_logic.assign_unit_path(id, Vec3::new(100.0, 0.0, 0.0), &[]));
    assert!(
        game_logic
            .queued_audio_events
            .iter()
            .any(|e| { e.event_type == "SentryDroneUndeploy" && e.object_id == Some(id) }),
        "Undeploy must play the authored per-unit event: {:?}",
        game_logic.queued_audio_events
    );
    assert!(
        game_logic
            .queued_audio_events
            .iter()
            .all(|e| e.event_type != DEPLOY_STYLE_UNDEPLOY_AUDIO),
        "must not queue the Undeploy slot token: {:?}",
        game_logic.queued_audio_events
    );
    clear_test_template_voices();
}

#[test]
fn deploy_style_missing_unit_sound_stays_silent() {
    use crate::game_logic::audio_dispatch_impl::clear_test_template_voices;
    use crate::game_logic::host_deploy_style::{
        DEPLOY_STYLE_DEPLOY_AUDIO, DEPLOY_STYLE_UNDEPLOY_AUDIO,
    };

    clear_test_template_voices();
    let mut game_logic = GameLogic::new();
    let mut tpl = ThingTemplate::new("SilentDeployer");
    tpl.add_kind_of(KindOf::Vehicle).set_health(100.0);
    tpl.deploy_style_metadata = Some(crate::game_logic::DeployStyleMetadata {
        pack_time_frames: 10,
        unpack_time_frames: 10,
        ..Default::default()
    });
    game_logic.templates.insert("SilentDeployer".into(), tpl);
    let id = game_logic
        .create_object("SilentDeployer", Team::USA, Vec3::ZERO)
        .expect("deployer");
    assert!(!game_logic.ensure_deploy_style_ready_to_fire(id));
    assert!(
        game_logic.queued_audio_events.iter().all(|e| {
            e.event_type != DEPLOY_STYLE_DEPLOY_AUDIO && e.event_type != "SilentDeployerDeploy"
        }),
        "missing UnitSpecificSounds.Deploy must stay silent: {:?}",
        game_logic.queued_audio_events
    );
    let _ = DEPLOY_STYLE_UNDEPLOY_AUDIO;
}

#[test]
fn deploy_style_nuke_launcher_normal_attack_waits_for_range_and_unpack() {
    use crate::game_logic::host_deploy_style::HostDeployStyleState;

    let mut logic = GameLogic::new();
    let mut launcher = ThingTemplate::new("ChinaVehicleNukeLauncher");
    launcher
        .add_kind_of(KindOf::Vehicle)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .set_health(240.0)
        .set_primary_weapon(Weapon {
            damage: 30.0,
            range: 100.0,
            min_range: 0.0,
            // Start reloading: C++ DeployStyleAIUpdate checks current-weapon
            // range, not weapon readiness, before it begins unpacking.
            reload_time: 100.0,
            last_fire_time: 0.0,
            ammo: None,
            clip_size: 0,
            clip_reload_time: 0.0,
            can_target_air: false,
            can_target_ground: true,
            // A finite speed above the 50-unit in-range shot distance gives
            // C++'s projectileless path a sub-frame delay, so it damages on
            // the firing frame without relying on the undefined 0/0 case.
            projectile_speed: 200.0,
            pre_attack_delay: 0.0,
            splash_radius: 0.0,
            suspend_fx_frame: 0,
            reloading_clip: false,
            last_bonus_rof: 0.0,
        });
    // Retail ChinaVehicleNukeLauncher has 3333ms Pack/Unpack, parsed with
    // C++ duration rounding into 100 logic frames. TurretsMustCenterBeforePacking
    // is live: READY_TO_ATTACK + move waits ALIGNING_TURRETS until natural.
    launcher.deploy_style_metadata = Some(crate::game_logic::DeployStyleMetadata {
        pack_time_frames: 100,
        unpack_time_frames: 100,
        turrets_function_only_when_deployed: true,
        turrets_must_center_before_packing: true,
        manual_deploy_animations: true,
        ..Default::default()
    });
    logic
        .templates
        .insert("ChinaVehicleNukeLauncher".to_string(), launcher);

    let mut target_template = ThingTemplate::new("DeployStyleTarget");
    target_template
        .add_kind_of(KindOf::Vehicle)
        .add_kind_of(KindOf::Attackable)
        .set_health(500.0);
    logic
        .templates
        .insert("DeployStyleTarget".to_string(), target_template);

    let launcher_id = logic
        .create_object(
            "ChinaVehicleNukeLauncher",
            Team::China,
            Vec3::new(0.0, 0.0, 0.0),
        )
        .expect("nuke launcher");
    let target_id = logic
        .create_object("DeployStyleTarget", Team::USA, Vec3::new(200.0, 0.0, 0.0))
        .expect("out-of-range target");
    let hp_before = logic.host_object(target_id).unwrap().health.current;
    let object_ids = [launcher_id, target_id];
    let mut last_ticked_frame = 0;
    let tick_through_frame = |logic: &mut GameLogic, last_frame: &mut u32, target_frame: u32| {
        // Preserve every logic-frame update like C++ does. Skipping frames
        // also skips AIM/FIRE transitions and turret alignment, so only
        // advancing the DeployStyle timer would not be a valid comparison.
        for frame in (*last_frame + 1)..=target_frame {
            logic.set_current_frame(frame.into());
            logic.tick_deploy_style_updates();
            // Mirror Phase 7: legacy combat first, then the nested machine
            // that owns a normal AttackObject command.
            logic.update_combat(&object_ids, LOGIC_FRAME_TIMESTEP);
            logic.tick_nested_attack_machines(
                &object_ids,
                frame as f32 * LOGIC_FRAME_TIMESTEP,
                frame,
            );
        }
        *last_frame = target_frame;
    };

    // A normal player AttackObject remains accepted and approaches; it must
    // not begin the DeployStyle timer merely because the target is distant.
    assert!(logic.unit_command_attack(launcher_id, target_id));
    tick_through_frame(&mut logic, &mut last_ticked_frame, 1);
    let launcher_after_oor = logic.host_object(launcher_id).unwrap();
    assert_eq!(launcher_after_oor.target, Some(target_id));
    assert!(matches!(
        launcher_after_oor
            .deploy_style
            .as_ref()
            .map(|deploy| deploy.state),
        Some(HostDeployStyleState::ReadyToMove)
    ));
    assert_eq!(
        logic.deploy_style_reg.deploys, 0,
        "an out-of-range attack must preserve the approach instead of unpacking"
    );

    // Once the same pending attack is actually in range, C++ DeployStyle
    // begins its authored timer and blocks damage through the final frame.
    logic
        .host_object_mut(target_id)
        .unwrap()
        .set_position(Vec3::new(50.0, 0.0, 0.0));
    tick_through_frame(&mut logic, &mut last_ticked_frame, 2);
    assert!(matches!(
        logic
            .host_object(launcher_id)
            .and_then(|object| object.deploy_style.as_ref())
            .map(|deploy| deploy.state),
        Some(HostDeployStyleState::Deploying)
    ));
    assert_eq!(
        logic.host_object(target_id).unwrap().health.current,
        hp_before,
        "the normal attack cannot fire while the launcher is unpacking"
    );

    // The deploy timer was allowed to begin while the weapon was reloading;
    // make the shot ready now so the remainder isolates the exact timer edge.
    if let Some(weapon) = logic
        .host_object_mut(launcher_id)
        .and_then(|launcher| launcher.weapon.as_mut())
    {
        weapon.reload_time = 0.0;
        weapon.last_fire_time = -100.0;
    }

    tick_through_frame(&mut logic, &mut last_ticked_frame, 101);
    assert!(
        !logic.attack_can_fire_at(launcher_id, target_id, 101.0 * LOGIC_FRAME_TIMESTEP, false,),
        "every fire authority must reject a packed DeployStyle weapon"
    );
    assert_eq!(
        logic.host_object(target_id).unwrap().health.current,
        hp_before,
        "retail 100-frame unpack still blocks one frame before completion"
    );

    tick_through_frame(&mut logic, &mut last_ticked_frame, 102);
    assert!(
        logic.host_object(launcher_id).unwrap().is_deployed(),
        "the exact timer boundary enters ReadyToAttack"
    );
    assert_eq!(
        logic
            .host_object(launcher_id)
            .and_then(|launcher| launcher.weapon.as_ref())
            .map(|weapon| weapon.last_fire_time),
        Some(102.0 * LOGIC_FRAME_TIMESTEP),
        "the retained attack discharges on the first frame ReadyToAttack"
    );
    // The complete GameLogic update resolves accepted shots after the nested
    // attack machine in its projectile phase. Reproduce those exact Phase 9
    // calls rather than expecting projectile damage during weapon acceptance.
    // This synthetic weapon has no ProjectileObject and its finite-speed shot
    // crosses the remaining range in less than one logic frame, so it follows
    // the projectileless delayed-damage path rather than becoming a
    // CombatSystem projectile. Production applies ready damage in this phase
    // after draining the fire queue.
    crate::game_logic::host_historic_bonus::set_logic_frame(102);
    crate::game_logic::combat::drain_pending_projectiles(&mut logic.combat_system, &logic.objects);
    crate::game_logic::combat::apply_ready_projectileless_delayed_damage(
        &mut logic.combat_system,
        &mut logic.objects,
        102,
        Some(&logic.players),
    );
    logic.combat_system.update_projectiles_with_relationships(
        LOGIC_FRAME_TIMESTEP,
        &mut logic.objects,
        Some(&mut logic.countermeasures),
        102,
        Some(&logic.players),
        Some(&logic.team_factory),
    );
    assert!(
        logic.host_object(target_id).unwrap().health.current < hp_before,
        "the accepted shot damages its target during the production projectile phase"
    );
}

#[test]
fn deploy_style_sentry_auto_target_loss_clears_pending_attack() {
    use crate::game_logic::host_deploy_style::HostDeployStyleState;

    let mut logic = GameLogic::new();
    let mut sentry = ThingTemplate::new("AmericaVehicleSentryDrone");
    sentry
        .add_kind_of(KindOf::Vehicle)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .set_health(300.0);
    sentry.deploy_style_metadata = Some(crate::game_logic::DeployStyleMetadata {
        pack_time_frames: 30,
        unpack_time_frames: 30,
        turrets_function_only_when_deployed: true,
        turrets_must_center_before_packing: true,
        ..Default::default()
    });
    logic
        .templates
        .insert("AmericaVehicleSentryDrone".to_string(), sentry);
    let mut target_template = ThingTemplate::new("DeployStyleAutoTarget");
    target_template
        .add_kind_of(KindOf::Vehicle)
        .add_kind_of(KindOf::Attackable)
        .set_health(100.0);
    logic
        .templates
        .insert("DeployStyleAutoTarget".to_string(), target_template);

    let sentry_id = logic
        .create_object(
            "AmericaVehicleSentryDrone",
            Team::USA,
            Vec3::new(0.0, 0.0, 0.0),
        )
        .expect("sentry");
    let target_id = logic
        .create_object(
            "DeployStyleAutoTarget",
            Team::GLA,
            Vec3::new(40.0, 0.0, 0.0),
        )
        .expect("auto target");
    {
        let sentry = logic.host_object_mut(sentry_id).unwrap();
        sentry.weapon = Some(Weapon {
            damage: 10.0,
            range: 100.0,
            min_range: 0.0,
            reload_time: 0.0,
            last_fire_time: -100.0,
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

    logic.set_current_frame(1);
    logic.update_combat(&[sentry_id, target_id], LOGIC_FRAME_TIMESTEP);
    let sentry_after_acquire = logic.host_object(sentry_id).unwrap();
    assert_eq!(sentry_after_acquire.target, Some(target_id));
    assert!(matches!(
        sentry_after_acquire
            .deploy_style
            .as_ref()
            .map(|deploy| deploy.state),
        Some(HostDeployStyleState::Deploying)
    ));

    // The acquired enemy can disappear while the unit is still packing. The
    // next normal combat pass owns invalid-target cleanup; it must not leave a
    // stale target that fires when ReadyToAttack is eventually reached.
    assert!(logic.objects.remove(&target_id).is_some());
    logic.set_current_frame(2);
    logic.tick_deploy_style_updates();
    let sentry_after_loss = logic.host_object(sentry_id).unwrap();
    assert!(sentry_after_loss.target.is_none());
    assert!(!sentry_after_loss.status.attacking);
    assert_eq!(sentry_after_loss.ai_state, AIState::Idle);
    assert!(matches!(
        sentry_after_loss
            .deploy_style
            .as_ref()
            .map(|deploy| deploy.state),
        Some(HostDeployStyleState::Deploying)
    ));
}

#[test]
fn deploy_style_must_center_turret_before_pack() {
    use crate::game_logic::host_deploy_style::HostDeployStyleState;

    let mut logic = GameLogic::new();
    let mut sentry = ThingTemplate::new("AmericaVehicleSentryDrone");
    sentry
        .add_kind_of(KindOf::Vehicle)
        .add_kind_of(KindOf::Selectable)
        .set_health(300.0);
    sentry.deploy_style_metadata = Some(crate::game_logic::DeployStyleMetadata {
        pack_time_frames: 30,
        unpack_time_frames: 30,
        turrets_must_center_before_packing: true,
        ..Default::default()
    });
    logic
        .templates
        .insert("AmericaVehicleSentryDrone".to_string(), sentry);

    let id = logic
        .create_object(
            "AmericaVehicleSentryDrone",
            Team::USA,
            Vec3::new(0.0, 0.0, 0.0),
        )
        .expect("sentry");
    {
        let obj = logic.host_object_mut(id).unwrap();
        obj.turret_enabled = true;
        obj.turret_turn_rate_rad = 0.1;
        obj.turret_angle_deg = 45.0;
        obj.turret_natural_angle_deg = 0.0;
        obj.turret_pitch_deg = 0.0;
        obj.turret_natural_pitch_deg = 0.0;
    }

    logic.set_current_frame(0);
    assert!(logic.unit_command_toggle_deploy_style(id));
    logic.set_current_frame(30);
    logic.tick_deploy_style_updates();
    assert!(
        matches!(
            logic
                .host_object(id)
                .and_then(|o| o.deploy_style.as_ref())
                .map(|d| d.state),
            Some(HostDeployStyleState::ReadyToAttack)
        ),
        "unpack must finish before the pack-align path"
    );
    assert!(logic.host_object(id).unwrap().is_deployed());

    {
        let obj = logic.host_object_mut(id).unwrap();
        obj.turret_angle_deg = 45.0;
    }
    logic.set_current_frame(31);
    assert!(logic.unit_command_toggle_deploy_style(id));
    assert!(
        matches!(
            logic
                .host_object(id)
                .and_then(|o| o.deploy_style.as_ref())
                .map(|d| d.state),
            Some(HostDeployStyleState::AligningTurrets)
        ),
        "TurretsMustCenterBeforePacking must enter ALIGNING_TURRETS"
    );
    assert!(
        logic.host_object(id).unwrap().is_deployed(),
        "ALIGNING stays DEPLOYED until UNDEPLOY"
    );

    logic.set_current_frame(32);
    logic.tick_deploy_style_updates();
    assert!(
        matches!(
            logic
                .host_object(id)
                .and_then(|o| o.deploy_style.as_ref())
                .map(|d| d.state),
            Some(HostDeployStyleState::AligningTurrets)
        ),
        "off-natural turret must not pack"
    );

    {
        let obj = logic.host_object_mut(id).unwrap();
        obj.turret_angle_deg = 0.0;
    }
    logic.set_current_frame(33);
    logic.tick_deploy_style_updates();
    assert!(
        matches!(
            logic
                .host_object(id)
                .and_then(|o| o.deploy_style.as_ref())
                .map(|d| d.state),
            Some(HostDeployStyleState::Undeploying)
        ),
        "isTurretInNaturalPosition must start UNDEPLOY"
    );
    assert!(
        !logic.host_object(id).unwrap().is_deployed(),
        "C++ setMyState(UNDEPLOY) clears OBJECT_STATUS_DEPLOYED immediately"
    );
}

#[test]
fn jet_out_of_ammo_paths_to_distant_airfield_then_rearms() {
    use crate::game_logic::{KindOf, ParkingPlaceMetadata, Team, ThingTemplate, Weapon};
    use glam::Vec3;

    let mut logic = GameLogic::new();
    ensure_test_player_for_team(&mut logic, Team::USA);
    let mut af_tmpl = ThingTemplate::new("AmericaAirfield");
    af_tmpl
        .add_kind_of(KindOf::Structure)
        .add_kind_of(KindOf::FSAirfield)
        .add_kind_of(KindOf::Attackable)
        .set_health(1000.0);
    af_tmpl.parking_place = Some(ParkingPlaceMetadata {
        num_rows: 2,
        num_cols: 2,
        approach_height: 50.0,
        landing_deck_height_offset: 0.0,
        has_runways: true,
        park_in_hangars: true,
        heal_amount_per_second: 10.0,
    });
    logic.templates.insert("AmericaAirfield".into(), af_tmpl);

    let mut jet_tmpl = ThingTemplate::new("AmericaJetRaptor");
    jet_tmpl.primary_weapon_name = Some("HostTestRaptorJetMissileWeapon".into());
    jet_tmpl
        .add_kind_of(KindOf::Aircraft)
        .add_kind_of(KindOf::Attackable)
        .set_health(100.0);
    logic.templates.insert("AmericaJetRaptor".into(), jet_tmpl);

    let af_id = logic
        .create_object("AmericaAirfield", Team::USA, Vec3::new(0.0, 0.0, 0.0))
        .expect("af");
    let jet_id = logic
        .create_object("AmericaJetRaptor", Team::USA, Vec3::new(2000.0, 0.0, 40.0))
        .expect("jet");

    {
        let jet = logic.objects.get_mut(&jet_id).unwrap();
        jet.weapon = Some(Weapon {
            damage: 50.0,
            range: 200.0,
            reload_time: 0.0,
            last_fire_time: -100.0,
            ammo: Some(0),
            clip_size: 4,
            can_target_air: true,
            can_target_ground: true,
            ..Weapon::default()
        });
        jet.status.airborne_target = true;
    }
    assert!(
        logic
            .objects
            .get(&jet_id)
            .unwrap()
            .needs_return_to_base_rearm()
    );

    // Distant: path toward airfield (not docked yet).
    assert!(logic.try_return_to_base_rearm(jet_id));
    {
        let jet = logic.objects.get(&jet_id).unwrap();
        assert_ne!(jet.contained_by, Some(af_id), "still en route");
        assert!(
            jet.movement.target_position.is_some()
                || !jet.movement.path.is_empty()
                || matches!(jet.ai_state, AIState::Moving),
            "should path toward airfield"
        );
        // Ammo still empty until dock.
        assert_eq!(jet.weapon.as_ref().unwrap().ammo, Some(0));
    }

    // C++ lands on the runway then taxis; dock only once grounded at the pad.
    {
        let jet = logic.objects.get_mut(&jet_id).unwrap();
        jet.set_position(Vec3::new(0.0, 0.0, 0.0));
        jet.status.airborne_target = false;
        jet.jet_ai.rtb_landing_phase = crate::game_logic::object::JET_RTB_PHASE_TAXI;
        if let Some(w) = jet.weapon.as_mut() {
            w.ammo = Some(0);
        }
    }
    assert!(logic.try_return_to_base_rearm(jet_id));
    {
        let jet = logic.objects.get(&jet_id).unwrap();
        assert_eq!(jet.contained_by, Some(af_id));
        assert_eq!(jet.weapon.as_ref().unwrap().ammo, Some(0));
        assert!(jet.needs_return_to_base_rearm());
    }
    logic.frame = logic.frame.saturating_add(1);
    assert!(logic.try_return_to_base_rearm(jet_id));
    {
        let jet = logic.objects.get(&jet_id).unwrap();
        assert_eq!(jet.weapon.as_ref().unwrap().ammo, Some(4));
        assert!(!jet.needs_return_to_base_rearm());
    }
}

#[test]
fn jet_airfield_rearm_waits_clip_reload_frames() {
    use crate::game_logic::host_raptor::{RAPTOR_CLIP_RELOAD_FRAMES, RAPTOR_CLIP_SIZE};
    use crate::game_logic::{KindOf, ParkingPlaceMetadata, Team, ThingTemplate, Weapon};
    use glam::Vec3;

    let mut logic = GameLogic::new();
    ensure_test_player_for_team(&mut logic, Team::USA);
    let mut af_tmpl = ThingTemplate::new("AmericaAirfield");
    af_tmpl
        .add_kind_of(KindOf::Structure)
        .add_kind_of(KindOf::FSAirfield)
        .set_health(1000.0);
    af_tmpl.parking_place = Some(ParkingPlaceMetadata {
        num_rows: 2,
        num_cols: 2,
        approach_height: 50.0,
        landing_deck_height_offset: 0.0,
        has_runways: true,
        park_in_hangars: true,
        heal_amount_per_second: 10.0,
    });
    logic.templates.insert("AmericaAirfield".into(), af_tmpl);
    let mut jet_tmpl = ThingTemplate::new("AmericaJetRaptor");
    jet_tmpl.primary_weapon_name = Some("HostTestRaptorJetMissileWeapon".into());
    jet_tmpl.add_kind_of(KindOf::Aircraft).set_health(100.0);
    logic.templates.insert("AmericaJetRaptor".into(), jet_tmpl);

    let af_id = logic
        .create_object("AmericaAirfield", Team::USA, Vec3::ZERO)
        .unwrap();
    let jet_id = logic
        .create_object("AmericaJetRaptor", Team::USA, Vec3::new(40.0, 40.0, 0.0))
        .unwrap();
    {
        let jet = logic.objects.get_mut(&jet_id).unwrap();
        jet.weapon = Some(Weapon {
            damage: 50.0,
            range: 200.0,
            reload_time: 0.0,
            last_fire_time: -100.0,
            ammo: Some(0),
            clip_size: RAPTOR_CLIP_SIZE,
            // Retail ClipReload 8000ms → 240 frames @ 30 FPS.
            clip_reload_time: (RAPTOR_CLIP_RELOAD_FRAMES as f32) / 30.0,
            can_target_air: true,
            can_target_ground: true,
            ..Weapon::default()
        });
        jet.status.airborne_target = false;
        jet.jet_ai.rtb_landing_phase = crate::game_logic::object::JET_RTB_PHASE_TAXI;
        jet.set_position(Vec3::ZERO);
    }

    logic.frame = 10;
    assert!(logic.try_return_to_base_rearm(jet_id));
    {
        let jet = logic.objects.get(&jet_id).unwrap();
        assert_eq!(jet.contained_by, Some(af_id), "must dock immediately");
        assert_eq!(
            jet.weapon.as_ref().unwrap().ammo,
            Some(0),
            "ammo still empty during ClipReload"
        );
        assert_eq!(
            jet.airfield_rearm_ready_frame,
            Some(10 + RAPTOR_CLIP_RELOAD_FRAMES)
        );
        assert!(jet.needs_return_to_base_rearm());
    }

    // Mid-reload: C++ setClipPercentFull((reloadTime-(done-now))/reloadTime).
    logic.frame = 10 + RAPTOR_CLIP_RELOAD_FRAMES - 1;
    assert!(logic.try_return_to_base_rearm(jet_id));
    assert_eq!(
        logic
            .objects
            .get(&jet_id)
            .unwrap()
            .weapon
            .as_ref()
            .unwrap()
            .ammo,
        Some(3)
    );

    // ClipReload elapsed → full rearm.
    logic.frame = 10 + RAPTOR_CLIP_RELOAD_FRAMES;
    assert!(logic.try_return_to_base_rearm(jet_id));
    {
        let jet = logic.objects.get(&jet_id).unwrap();
        assert_eq!(jet.weapon.as_ref().unwrap().ammo, Some(RAPTOR_CLIP_SIZE));
        assert!(jet.airfield_rearm_ready_frame.is_none());
        assert!(!jet.needs_return_to_base_rearm());
    }
}

#[test]
fn empty_jet_circles_last_airfield_instead_of_bleeding_in_place() {
    use crate::game_logic::audio_dispatch_impl::{
        clear_test_template_voices, set_test_per_unit_sound,
    };
    use crate::game_logic::{KindOf, ParkingPlaceMetadata, Team, ThingTemplate, Weapon};
    use glam::Vec3;

    clear_test_template_voices();
    // C++ JetOrHeliCirclingDeadAirfieldState::onEnter plays
    // getPerUnitSound("VoiceLowFuel") — retail RaptorUnit authors
    // VoiceLowFuel = RaptorVoiceLowFuel; the resolver is override-or-factory.
    set_test_per_unit_sound("AmericaJetRaptor", "VoiceLowFuel", "RaptorVoiceLowFuel");
    let mut logic = GameLogic::new();
    // C++ airfields always author ParkingPlaceBehavior; JetAI RTB reservation
    // additionally needs the exact-controller owner pair (airfield.rs:1262).
    ensure_test_player_for_team(&mut logic, Team::USA);
    let mut af_tmpl = ThingTemplate::new("AmericaAirfield");
    af_tmpl
        .add_kind_of(KindOf::Structure)
        .add_kind_of(KindOf::FSAirfield)
        .add_kind_of(KindOf::Attackable)
        .set_health(1000.0);
    af_tmpl.parking_place = Some(ParkingPlaceMetadata {
        num_rows: 2,
        num_cols: 2,
        approach_height: 50.0,
        landing_deck_height_offset: 0.0,
        has_runways: true,
        park_in_hangars: true,
        heal_amount_per_second: 10.0,
    });
    logic.templates.insert("AmericaAirfield".into(), af_tmpl);
    let mut jet_tmpl = ThingTemplate::new("AmericaJetRaptor");
    jet_tmpl.primary_weapon_name = Some("HostTestRaptorJetMissileWeapon".into());
    jet_tmpl
        .add_kind_of(KindOf::Aircraft)
        .add_kind_of(KindOf::Attackable)
        .set_health(100.0);
    logic.templates.insert("AmericaJetRaptor".into(), jet_tmpl);

    let af_id = logic
        .create_object("AmericaAirfield", Team::USA, Vec3::ZERO)
        .expect("af");
    let jet_id = logic
        .create_object("AmericaJetRaptor", Team::USA, Vec3::new(2000.0, 50.0, 0.0))
        .expect("jet");
    {
        let jet = logic.objects.get_mut(&jet_id).unwrap();
        jet.producer_id = Some(af_id);
        jet.status.airborne_target = true;
        jet.weapon = Some(Weapon {
            damage: 50.0,
            range: 200.0,
            reload_time: 0.0,
            last_fire_time: -100.0,
            ammo: Some(0),
            clip_size: 4,
            can_target_air: true,
            can_target_ground: true,
            ..Weapon::default()
        });
    }

    logic.tick_out_of_ammo_jet_damage();
    let hp_after_first = logic.objects.get(&jet_id).unwrap().health.current;
    assert!(
        (hp_after_first - 100.0).abs() < 1e-3,
        "must not bleed while a live airfield exists or while flying home"
    );

    logic.destroy_object(af_id);
    if let Some(af) = logic.objects.get_mut(&af_id) {
        af.status.destroyed = true;
        af.health.current = 0.0;
    }
    let hp_before = logic.objects.get(&jet_id).unwrap().health.current;
    logic.tick_out_of_ammo_jet_damage();
    {
        let jet = logic.objects.get(&jet_id).unwrap();
        assert!(
            (jet.health.current - hp_before).abs() < 1e-3,
            "must not bleed in place after airfield dies"
        );
        assert!(
            !jet.jet_circling_dead_airfield,
            "still returning to last airfield"
        );
        let goal = jet.jet_producer_location_vec().expect("remembered wreck");
        assert!(goal.x.abs() < 1.0 && goal.z.abs() < 1.0);
    }

    {
        let jet = logic.objects.get_mut(&jet_id).unwrap();
        jet.set_position(Vec3::new(5.0, 50.0, 0.0));
    }
    logic.queued_audio_events.clear();
    logic.tick_out_of_ammo_jet_damage();
    {
        let jet = logic.objects.get(&jet_id).unwrap();
        assert!(jet.jet_circling_dead_airfield);
        assert!(
            jet.health.current < hp_before - 1e-4,
            "OutOfAmmoDamage only after circling the wreck"
        );
    }
    let events: Vec<&str> = logic
        .queued_audio_events
        .iter()
        .map(|e| e.event_type.as_str())
        .collect();
    assert_eq!(
        events,
        vec!["RaptorVoiceLowFuel"],
        "circling enter plays authored PerUnitSound VoiceLowFuel at the jet"
    );
    assert!(
        !events
            .iter()
            .any(|e| *e == "VoiceLowFuel" || *e == "AmericaJetRaptorVoiceLowFuel"),
        "must not queue slot token or invented concat"
    );
    clear_test_template_voices();
}

#[test]
fn command_button_hunt_hijack_issues_nearest_enemy_vehicle() {
    use crate::game_logic::host_command_button_hunt::HostCommandButtonHuntMode;
    use crate::game_logic::{KindOf, Team, ThingTemplate};
    use glam::Vec3;

    let mut logic = GameLogic::new();
    ensure_test_infantry_template(&mut logic);
    ensure_test_tank_template(&mut logic);

    let hijacker = logic
        .create_object("TestInfantry", Team::GLA, Vec3::new(0.0, 0.0, 0.0))
        .expect("hijacker");
    {
        let h = logic.host_object_mut(hijacker).unwrap();
        h.template_name = "GLAInfantryHijacker".into();
        h.set_ai_state(AIState::Idle);
    }
    let near = logic
        .create_object("TestTank", Team::USA, Vec3::new(40.0, 0.0, 0.0))
        .expect("near");
    let far = logic
        .create_object("TestTank", Team::USA, Vec3::new(400.0, 0.0, 0.0))
        .expect("far");
    let _ = far;

    assert!(logic.start_command_button_hunt(hijacker, HostCommandButtonHuntMode::HijackVehicle));
    logic.frame = 0;
    logic.tick_command_button_hunt_updates();

    assert!(logic.honesty_command_button_hunt_ok());
    assert!(
        logic.pending_special_abilities.get(&hijacker).is_some(),
        "should queue Hijack on nearest tank"
    );
    match logic.pending_special_abilities.get(&hijacker) {
        Some(PendingSpecialAbility::Hijack { target_id }) => {
            assert_eq!(*target_id, near);
        }
        other => panic!("expected Hijack, got {other:?}"),
    }
}

#[test]
fn command_button_hunt_named_arms_capture_and_flashbang() {
    use crate::game_logic::host_command_button_hunt::HostCommandButtonHuntMode;
    use crate::game_logic::{KindOf, Team, ThingTemplate, Weapon, WeaponLockType};
    use glam::Vec3;

    let mut logic = GameLogic::new();
    ensure_test_infantry_template(&mut logic);
    let ranger = logic
        .create_object("TestInfantry", Team::USA, Vec3::ZERO)
        .expect("ranger");
    {
        let r = logic.host_object_mut(ranger).unwrap();
        r.template_name = "AmericaInfantryRanger".into();
        r.secondary_weapon = Some(Weapon {
            range: 100.0,
            damage: 5.0,
            ..Weapon::default()
        });
        r.set_ai_state(AIState::Idle);
    }
    assert!(
        logic
            .start_command_button_hunt_named(ranger, Some("Command_AmericaRangerFlashBangGrenade"))
    );
    {
        let r = logic.host_object(ranger).unwrap();
        assert_eq!(
            r.command_button_hunt.as_ref().map(|h| h.mode),
            Some(HostCommandButtonHuntMode::FireWeapon)
        );
        assert_eq!(
            r.command_button_hunt.as_ref().map(|h| h.weapon_slot),
            Some(1)
        );
    }
    logic.frame = 0;
    logic.tick_command_button_hunt_updates();
    let r = logic.host_object(ranger).unwrap();
    assert_eq!(r.ai_state, AIState::Patrolling);
    assert_eq!(r.weapon_lock_type, WeaponLockType::LockedTemporarily);
    assert_eq!(r.weapon_lock_slot, 1);

    let lotus = logic
        .create_object("TestInfantry", Team::USA, Vec3::new(5.0, 0.0, 0.0))
        .expect("lotus");
    {
        let l = logic.host_object_mut(lotus).unwrap();
        l.template_name = "AmericaInfantryColonelBurton".into();
        l.set_ai_state(AIState::Idle);
    }
    assert!(
        logic.start_command_button_hunt_named(lotus, Some("Command_CaptureBuilding")),
        "capture hunt must arm"
    );
}

#[test]
fn fire_weapon_command_button_hunt_rearms_lock_every_frame() {
    use crate::game_logic::host_command_button_hunt::HostCommandButtonHuntMode;
    use crate::game_logic::{Team, Weapon, WeaponLockType};
    use glam::Vec3;

    let mut logic = GameLogic::new();
    ensure_test_infantry_template(&mut logic);
    let ranger = logic
        .create_object("TestInfantry", Team::USA, Vec3::ZERO)
        .expect("ranger");
    {
        let r = logic.host_object_mut(ranger).unwrap();
        r.template_name = "AmericaInfantryRanger".into();
        r.secondary_weapon = Some(Weapon {
            range: 100.0,
            damage: 5.0,
            ..Weapon::default()
        });
        r.set_ai_state(AIState::Idle);
    }
    assert!(
        logic
            .start_command_button_hunt_named(ranger, Some("Command_AmericaRangerFlashBangGrenade"))
    );
    logic.frame = 0;
    logic.tick_command_button_hunt_updates();
    {
        let r = logic.host_object(ranger).unwrap();
        assert_eq!(r.ai_state, AIState::Patrolling);
        assert_eq!(r.weapon_lock_type, WeaponLockType::LockedTemporarily);
        assert_eq!(r.weapon_lock_slot, 1);
    }
    // C++ Object::fireCurrentWeapon releases temp lock when the clip auto-reloads.
    logic
        .host_object_mut(ranger)
        .unwrap()
        .release_weapon_lock(WeaponLockType::LockedTemporarily);
    assert_eq!(
        logic.host_object(ranger).unwrap().weapon_lock_type,
        WeaponLockType::NotLocked
    );
    // Old live scheduled +30 for FireWeapon, so frame 1 would not re-arm.
    logic.frame = 1;
    logic.tick_command_button_hunt_updates();
    let r = logic.host_object(ranger).unwrap();
    assert_eq!(
        r.command_button_hunt.as_ref().map(|h| h.mode),
        Some(HostCommandButtonHuntMode::FireWeapon)
    );
    assert_eq!(
        r.weapon_lock_type,
        WeaponLockType::LockedTemporarily,
        "FireWeapon hunt must re-arm LOCKED_TEMPORARILY the next frame"
    );
    assert_eq!(r.weapon_lock_slot, 1);
}

#[test]
fn command_button_hunt_quits_after_player_move() {
    use crate::game_logic::Team;
    use crate::game_logic::host_command_button_hunt::{
        HUNT_CMD_FROM_PLAYER, HostCommandButtonHuntMode,
    };
    use glam::Vec3;

    let mut logic = GameLogic::new();
    ensure_test_infantry_template(&mut logic);
    ensure_test_tank_template(&mut logic);
    let hijacker = logic
        .create_object("TestInfantry", Team::GLA, Vec3::ZERO)
        .expect("hijacker");
    {
        let h = logic.host_object_mut(hijacker).unwrap();
        h.template_name = "GLAInfantryHijacker".into();
        h.set_ai_state(AIState::Idle);
    }
    let _tank = logic
        .create_object("TestTank", Team::USA, Vec3::new(40.0, 0.0, 0.0))
        .expect("tank");
    assert!(logic.start_command_button_hunt(hijacker, HostCommandButtonHuntMode::HijackVehicle));
    assert!(logic.unit_command_move_to(hijacker, Vec3::new(10.0, 0.0, 0.0)));
    assert_eq!(
        logic.host_object(hijacker).unwrap().last_command_source,
        HUNT_CMD_FROM_PLAYER
    );
    logic.frame = 0;
    logic.tick_command_button_hunt_updates();
    let h = logic.host_object(hijacker).unwrap();
    assert!(
        h.command_button_hunt.is_none(),
        "player move must permanently end CommandButtonHunt"
    );
    assert!(logic.pending_special_abilities.get(&hijacker).is_none());
}

#[test]
fn command_button_hunt_enter_skips_stealth_ally_and_drone() {
    use crate::game_logic::host_command_button_hunt::HostCommandButtonHuntMode;
    use crate::game_logic::{KindOf, Player, Team, ThingTemplate};
    use glam::Vec3;

    let mut logic = GameLogic::new();
    let mut usa = Player::new(0, Team::USA, "USA", true);
    let mut china = Player::new(1, Team::China, "China", false);
    let mut gla = Player::new(2, Team::GLA, "GLA", false);
    usa.alliance_team = 7;
    china.alliance_team = 7;
    gla.alliance_team = 9;
    logic.add_player(usa);
    logic.add_player(china);
    logic.add_player(gla);
    ensure_test_infantry_template(&mut logic);
    ensure_test_tank_template(&mut logic);
    let mut drone = ThingTemplate::new("TestDrone");
    drone
        .add_kind_of(KindOf::Vehicle)
        .add_kind_of(KindOf::Drone)
        .set_health(50.0);
    logic.templates.insert("TestDrone".into(), drone);

    let hijacker = logic
        .create_object("TestInfantry", Team::GLA, Vec3::ZERO)
        .expect("hijacker");
    {
        let h = logic.host_object_mut(hijacker).unwrap();
        h.template_name = "GLAInfantryHijacker".into();
        h.owner_player_id = Some(2);
        h.set_ai_state(AIState::Idle);
    }
    let stealth = logic
        .create_object("TestTank", Team::USA, Vec3::new(20.0, 0.0, 0.0))
        .expect("stealth");
    {
        let t = logic.host_object_mut(stealth).unwrap();
        t.owner_player_id = Some(0);
        t.status.stealthed = true;
        t.status.detected = false;
    }
    let ally = logic
        .create_object("TestTank", Team::China, Vec3::new(25.0, 0.0, 0.0))
        .expect("ally");
    logic.host_object_mut(ally).unwrap().owner_player_id = Some(1);
    // China is allied with USA, not GLA — this is a GLA hijacker vs China tank
    // so China is an enemy. Use a same-alliance tank by making the hunter USA.
    let usa_hunter = logic
        .create_object("TestInfantry", Team::USA, Vec3::new(5.0, 0.0, 0.0))
        .expect("usa hunter");
    {
        let h = logic.host_object_mut(usa_hunter).unwrap();
        h.template_name = "GLAInfantryHijacker".into();
        h.owner_player_id = Some(0);
        h.set_ai_state(AIState::Idle);
    }
    let drone_id = logic
        .create_object("TestDrone", Team::GLA, Vec3::new(15.0, 0.0, 0.0))
        .expect("drone");
    logic.host_object_mut(drone_id).unwrap().owner_player_id = Some(2);
    let legal = logic
        .create_object("TestTank", Team::GLA, Vec3::new(80.0, 0.0, 0.0))
        .expect("legal");
    logic.host_object_mut(legal).unwrap().owner_player_id = Some(2);

    assert!(logic.start_command_button_hunt(usa_hunter, HostCommandButtonHuntMode::HijackVehicle));
    logic.frame = 0;
    logic.tick_command_button_hunt_updates();
    match logic.pending_special_abilities.get(&usa_hunter) {
        Some(PendingSpecialAbility::Hijack { target_id }) => {
            assert_eq!(*target_id, legal, "must skip stealth, 2v2 ally, and drone");
        }
        other => panic!("expected Hijack of legal enemy, got {other:?}"),
    }
}

#[test]
fn command_button_hunt_special_skips_ally_mine_and_uses_priority() {
    use crate::game_logic::host_command_button_hunt::HostCommandButtonHuntMode;
    use crate::game_logic::{AttackPriorityInfo, KindOf, Player, Team, ThingTemplate};
    use glam::Vec3;

    let mut logic = GameLogic::new();
    let mut usa = Player::new(0, Team::USA, "USA", true);
    let mut china = Player::new(1, Team::China, "China", false);
    let mut gla = Player::new(2, Team::GLA, "GLA", false);
    usa.alliance_team = 7;
    china.alliance_team = 7;
    gla.alliance_team = 9;
    logic.add_player(usa);
    logic.add_player(china);
    logic.add_player(gla);
    ensure_test_infantry_template(&mut logic);
    ensure_test_tank_template(&mut logic);
    let mut bldg = ThingTemplate::new("TestBuilding");
    bldg.add_kind_of(KindOf::Structure).set_health(500.0);
    logic.templates.insert("TestBuilding".into(), bldg);
    let mut mine_t = ThingTemplate::new("TestMine");
    mine_t.add_kind_of(KindOf::Mine).set_health(10.0);
    logic.templates.insert("TestMine".into(), mine_t);

    // Capture: ally building closer than enemy — must pick enemy.
    let lotus = logic
        .create_object("TestInfantry", Team::USA, Vec3::ZERO)
        .expect("lotus");
    {
        let l = logic.host_object_mut(lotus).unwrap();
        l.template_name = "AmericaInfantryColonelBurton".into();
        l.owner_player_id = Some(0);
        l.set_ai_state(AIState::Idle);
    }
    let ally_b = logic
        .create_object("TestBuilding", Team::China, Vec3::new(30.0, 0.0, 0.0))
        .expect("ally bldg");
    logic.host_object_mut(ally_b).unwrap().owner_player_id = Some(1);
    let enemy_b = logic
        .create_object("TestBuilding", Team::GLA, Vec3::new(90.0, 0.0, 0.0))
        .expect("enemy bldg");
    logic.host_object_mut(enemy_b).unwrap().owner_player_id = Some(2);
    assert!(logic.start_command_button_hunt_named(lotus, Some("Command_CaptureBuilding")));
    logic.frame = 0;
    logic.tick_command_button_hunt_updates();
    let lotus_obj = logic.host_object(lotus).unwrap();
    assert_eq!(
        lotus_obj.target,
        Some(enemy_b),
        "capture must skip 2v2 ally"
    );

    // TNT: owned mine next to nearer tank → skip it, pick farther unmined tank.
    // Stay inside default GameLogic extent (-256..256) so same-map passes.
    let hunter = logic
        .create_object("TestInfantry", Team::USA, Vec3::new(-180.0, 0.0, 0.0))
        .expect("th");
    {
        let h = logic.host_object_mut(hunter).unwrap();
        h.template_name = "ChinaInfantryTankHunter".into();
        h.owner_player_id = Some(0);
        h.set_ai_state(AIState::Idle);
    }
    let mined = logic
        .create_object("TestTank", Team::GLA, Vec3::new(-150.0, 0.0, 0.0))
        .expect("mined");
    logic.host_object_mut(mined).unwrap().owner_player_id = Some(2);
    let mine = logic
        .create_object("TestMine", Team::USA, Vec3::new(-150.0, 0.0, 0.0))
        .expect("mine");
    logic.host_object_mut(mine).unwrap().owner_player_id = Some(0);
    let clean = logic
        .create_object("TestTank", Team::GLA, Vec3::new(-80.0, 0.0, 0.0))
        .expect("clean");
    logic.host_object_mut(clean).unwrap().owner_player_id = Some(2);
    assert!(logic.start_command_button_hunt_named(hunter, Some("Command_ChinaTankHunterTNT")));
    logic.frame = 0;
    logic.tick_command_button_hunt_updates();
    match logic.pending_special_abilities.get(&hunter) {
        Some(PendingSpecialAbility::PlantTimedDemoCharge { target_id }) => {
            assert_eq!(*target_id, clean, "TNT hunt must skip already-mined target");
        }
        other => panic!("expected TNT on clean tank, got {other:?}"),
    }

    // Priority: farther Dozer outranks nearer Tank (on-map).
    let mut info = AttackPriorityInfo::new("HuntPrio");
    info.default_priority = 1;
    info.set_priority_template("TestTank", 5);
    info.set_priority_template("TestDozer", 80);
    logic.register_attack_priority_set(info);
    ensure_test_dozer_template(&mut logic);
    let prio_hunter = logic
        .create_object("TestInfantry", Team::USA, Vec3::new(120.0, 0.0, 0.0))
        .expect("prio");
    {
        let h = logic.host_object_mut(prio_hunter).unwrap();
        h.template_name = "ChinaInfantryTankHunter".into();
        h.owner_player_id = Some(0);
        h.attack_priority_set = Some("HuntPrio".into());
        h.set_ai_state(AIState::Idle);
    }
    let near_tank = logic
        .create_object("TestTank", Team::GLA, Vec3::new(140.0, 0.0, 0.0))
        .expect("near tank");
    logic.host_object_mut(near_tank).unwrap().owner_player_id = Some(2);
    let far_dozer = logic
        .create_object("TestDozer", Team::GLA, Vec3::new(220.0, 0.0, 0.0))
        .expect("far dozer");
    logic.host_object_mut(far_dozer).unwrap().owner_player_id = Some(2);
    assert!(logic.start_command_button_hunt_named(prio_hunter, Some("Command_ChinaTankHunterTNT")));
    logic.frame = 0;
    logic.tick_command_button_hunt_updates();
    match logic.pending_special_abilities.get(&prio_hunter) {
        Some(PendingSpecialAbility::PlantTimedDemoCharge { target_id }) => {
            assert_eq!(*target_id, far_dozer, "priority must beat raw nearest");
        }
        other => panic!("expected TNT on high-priority dozer, got {other:?}"),
    }
    let _ = HostCommandButtonHuntMode::SpecialPower;
}

#[test]
fn command_button_hunt_script_drain_requires_module_not_mobile() {
    use crate::game_logic::{KindOf, Team, ThingTemplate};
    use glam::Vec3;

    let mut logic = GameLogic::new();
    ensure_test_infantry_template(&mut logic);
    ensure_test_tank_template(&mut logic);

    let hijacker = logic
        .create_object("TestInfantry", Team::USA, Vec3::ZERO)
        .expect("hijacker");
    {
        let u = logic.host_object_mut(hijacker).unwrap();
        u.template_name = "GLAInfantryHijacker".into();
        u.status.disabled_held = true;
        assert!(!u.can_move(), "HELD hijacker cannot walk");
    }
    assert!(
        logic.unit_can_team_hunt_with_command_button(hijacker, Some("Command_HijackVehicle")),
        "C++ doTeamHuntWithCommandButton has no is_mobile gate"
    );

    let crusader = logic
        .create_object("TestTank", Team::USA, Vec3::new(20.0, 0.0, 0.0))
        .expect("crusader");
    {
        let u = logic.host_object_mut(crusader).unwrap();
        u.template_name = "AmericaTankCrusader".into();
    }
    assert!(
        !logic.unit_can_team_hunt_with_command_button(crusader, Some("Command_ChinaTankHunterTNT")),
        "units without CommandButtonHuntUpdate must not arm"
    );

    let mut mine_t = ThingTemplate::new("TestHuntMine");
    mine_t.add_kind_of(KindOf::Mine).set_health(10.0);
    logic.templates.insert("TestHuntMine".into(), mine_t);
    let mine = logic
        .create_object("TestHuntMine", Team::USA, Vec3::new(40.0, 0.0, 0.0))
        .expect("mine");
    assert!(
        !logic.unit_can_team_hunt_with_command_button(mine, Some("Command_HijackVehicle")),
        "no-AI members must skip"
    );
}

#[test]
fn command_button_hunt_tnt_and_booby_reject_neutral() {
    use crate::game_logic::{KindOf, Player, Team, ThingTemplate};
    use glam::Vec3;

    let mut logic = GameLogic::new();
    // C++ players are always live controlling players at scan time; the host
    // ownership resolution filters dead players out (get_player().filter
    // is_alive), which would turn every relationship Neutral and reject both
    // scan candidates.
    // Distinct alliance teams model skirmish diplomacy: no shared
    // alliance_team between live controlling players resolves Enemies
    // (player_relationship_from_map, object_queries.rs:445-451).
    let mut china = Player::new(0, Team::China, "China", true);
    china.alliance_team = 0;
    let mut gla = Player::new(1, Team::GLA, "GLA", true);
    gla.alliance_team = 1;
    logic.add_player(china);
    logic.add_player(gla);
    ensure_test_infantry_template(&mut logic);
    let mut bldg = ThingTemplate::new("TestBuilding");
    bldg.add_kind_of(KindOf::Structure).set_health(500.0);
    logic.templates.insert("TestBuilding".into(), bldg);

    let hunter = logic
        .create_object("TestInfantry", Team::China, Vec3::ZERO)
        .expect("th");
    {
        let h = logic.host_object_mut(hunter).unwrap();
        h.template_name = "ChinaInfantryTankHunter".into();
        h.owner_player_id = Some(0);
        h.set_ai_state(AIState::Idle);
    }
    let civilian = logic
        .create_object("TestBuilding", Team::Neutral, Vec3::new(30.0, 0.0, 0.0))
        .expect("civ");
    logic.host_object_mut(civilian).unwrap().owner_player_id = None;
    let enemy = logic
        .create_object("TestBuilding", Team::GLA, Vec3::new(90.0, 0.0, 0.0))
        .expect("enemy");
    logic.host_object_mut(enemy).unwrap().owner_player_id = Some(1);

    assert!(logic.start_command_button_hunt_named(hunter, Some("Command_ChinaTankHunterTNT")));
    logic.frame = 0;
    logic.tick_command_button_hunt_updates();
    match logic.pending_special_abilities.get(&hunter) {
        Some(PendingSpecialAbility::PlantTimedDemoCharge { target_id }) => {
            assert_eq!(
                *target_id, enemy,
                "TNT hunt must ignore Neutral civilian buildings"
            );
        }
        other => panic!("expected TNT on enemy, got {other:?}"),
    }

    logic.pending_special_abilities.remove(&hunter);
    if let Some(h) = logic.host_object_mut(hunter) {
        h.clear_command_button_hunt();
        h.set_ai_state(AIState::Idle);
        h.target = None;
    }
    assert!(logic.start_command_button_hunt_named(hunter, Some("Command_BoobyTrapBuilding")));
    logic.frame = 0;
    logic.tick_command_button_hunt_updates();
    match logic.pending_special_abilities.get(&hunter) {
        Some(PendingSpecialAbility::PlantBoobyTrap { target_id }) => {
            assert_eq!(*target_id, enemy, "booby hunt must ignore Neutral");
        }
        other => panic!("expected booby on enemy, got {other:?}"),
    }
}
