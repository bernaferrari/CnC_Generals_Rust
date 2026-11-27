//! Production and construction ownership fixtures.
use super::*;

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
    assert!({
        let (player, economy_events) = logic.player_and_economy_events_mut(0).unwrap();
        player.queue_upgrade(
            UPGRADE,
            &cost,
            game_engine::common::system::upgrade::UpgradeType::Player,
            economy_events,
        )
    });
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
fn exact_player_template_production_maps_keep_cpp_name_semantics() {
    use crate::game_logic::host_upgrade_module_residuals::apply_production_cost_factor;
    use game_engine::common::game_common::VeterancyLevel as CommonVeterancyLevel;
    use game_engine::common::rts::player_template::PlayerTemplate;
    use generals_sim::production::production_total_logic_frames;

    // C++ Player::init copies these three maps from its selected
    // PlayerTemplate, and Player.cpp subsequently queries the exact interned
    // ThingTemplate name. Exercise the Main adapter without a
    // global-store mutation so parallel tests cannot alter retail templates.
    let mut template = PlayerTemplate::new("TestExactGeneral".to_string());
    template
        .production_cost_changes
        .insert("ExactUnit".into(), -0.20);
    template
        .production_time_changes
        .insert("ExactUnit".into(), 0.25);
    template
        .production_veterancy_levels
        .insert("ExactUnit".into(), CommonVeterancyLevel::Elite);

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
    use super::super::super::helpers::{
        advance_logic_frames, ensure_admitted_dozer_content,
    };
    let mut logic = GameLogic::new();
    // A live session runs past frame 0 (the Dozer machine picks its request next update).
    logic.frame = 10;
    // Authored dozer content (DozerAIUpdate is the only builder) and the
    // authored TestBuilding (BuildTime = 10 seconds).
    super::super::super::helpers::admit_retail_production_energy_rules(&mut logic);
    ensure_admitted_dozer_content(&mut logic).expect("authored dozer content admits");
    // A powered grid, so the Min/MaxLowEnergyProductionSpeed rules do not apply.
    super::super::super::helpers::add_completed_power_plant(&mut logic, 0);
    logic
        .players
        .get_mut(&0)
        .expect("player")
        .map_side
        .handicap_build_time_buildings = 2.0;
    let pad_id = logic
        .create_object_for_player("TestBuilding", 0, Vec3::ZERO)
        .expect("pad");
    let dozer = logic
        .create_object_for_player("TestDozer", 0, Vec3::new(5.0, 0.0, 0.0))
        .expect("dozer");
    {
        let obj = logic.host_object_mut(pad_id).expect("pad");
        obj.set_status_under_construction(true);
        obj.construction_percent = 0.0;
    }
    assert!(logic.resume_construction(&[dozer], pad_id).expect("resume"));
    // The builder walks to its dock; the first BUILD frame is the first progress.
    let mut progress = 0.0;
    for _ in 0..300 {
        advance_logic_frames(&mut logic, 1);
        progress = logic.host_object(pad_id).unwrap().construction_percent;
        if progress > 0.0 {
            break;
        }
    }
    // 10s * 30 = 300 frames * 2.0 handicap = 600 frames per full build.
    assert!(
        (progress - 1.0 / 600.0).abs() < 1e-6,
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
        .and_then(|d| d.dozer_dock_action())
        .expect("dock");
    {
        let obj = logic.host_object_mut(pad_id).expect("pad");
        obj.set_status_under_construction(true);
        obj.builder_id = Some(dozer);
    }
    {
        let ai_entry_frame = logic.frame;
        let (obj, attack_events, _fixture_audio_obj, fixture_partition_obj) = logic
            .host_object_attack_audio_partition_mut(dozer)
            .expect("dozer");
        obj.set_position_with_partition_owner(
            dock,
            0,
            &mut Default::default(),
            fixture_partition_obj,
        );
        obj.set_target(
            Some(pad_id),
            ai_entry_frame,
            attack_events,
            &mut crate::game_logic::ObjectAudio::record(&mut Vec::new()),
        );
        obj.set_ai_state(
            AIState::Moving,
            ai_entry_frame,
            &mut crate::game_logic::ObjectAudio::record(&mut Vec::new()),
        );
        obj.movement.clear_path();
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
        .and_then(|d| d.dozer_dock_action())
        .unwrap_or(Vec3::ZERO);
    {
        let obj = logic.host_object_mut(pad_id).expect("pad");
        obj.set_status_under_construction(true);
        obj.builder_id = Some(dozer);
    }
    {
        let ai_entry_frame = logic.frame;
        let (obj, attack_events, _fixture_audio_obj, fixture_partition_obj) = logic
            .host_object_attack_audio_partition_mut(dozer)
            .expect("dozer");
        obj.set_position_with_partition_owner(
            dock,
            0,
            &mut Default::default(),
            fixture_partition_obj,
        );
        obj.set_target(
            Some(pad_id),
            ai_entry_frame,
            attack_events,
            &mut crate::game_logic::ObjectAudio::record(&mut Vec::new()),
        );
        obj.set_ai_state(
            AIState::Constructing,
            ai_entry_frame,
            &mut crate::game_logic::ObjectAudio::record(&mut Vec::new()),
        );
        obj.movement.clear_path();
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

/// A real short build: the authored Dozer resumes a TestBuilding scaffold, walks
/// to its dock and builds it in a few frames. Returns the world, the pad, the
/// dozer, its action dock and its end dock, plus the dozer's path end and
/// ignored obstacle observed on the frame the build completed.
fn run_authored_short_build() -> (GameLogic, ObjectId, ObjectId, Vec3, Vec3, Option<Vec3>, Option<ObjectId>) {
    use super::super::super::helpers::{advance_logic_frames, ensure_admitted_dozer_content};
    let mut logic = GameLogic::new();
    // A live session runs past frame 0 (the Dozer machine picks its request next update).
    logic.frame = 10;
    super::super::super::helpers::admit_retail_production_energy_rules(&mut logic);
    ensure_admitted_dozer_content(&mut logic).expect("authored dozer content admits");
    super::super::super::helpers::add_completed_power_plant(&mut logic, 0);
    let pad_id = logic
        .create_object_for_player("TestBuilding", 0, Vec3::ZERO)
        .expect("pad");
    let dozer = logic
        .create_object_for_player("TestDozer", 0, Vec3::new(5.0, 0.0, 0.0))
        .expect("dozer");
    {
        let obj = logic.host_object_mut(pad_id).expect("pad");
        obj.template_mut().build_time = 0.1; // 3 frames
        obj.set_status_under_construction(true);
        obj.construction_percent = 0.0;
    }
    assert!(logic.resume_construction(&[dozer], pad_id).expect("resume"));
    let action = logic
        .host_object(dozer)
        .and_then(|d| d.dozer_dock_action())
        .expect("resume stores the action dock");
    let building_pos = logic.host_object(pad_id).expect("pad").get_position();
    let end_dock = crate::game_logic::host_repair::dozer_end_dock_position(action, building_pos);
    let mut path_end = None;
    let mut ignored = None;
    for _ in 0..300 {
        advance_logic_frames(&mut logic, 1);
        let done = logic
            .host_object(pad_id)
            .is_some_and(|pad| !pad.status.under_construction && pad.construction_percent >= 1.0);
        if done {
            let unit = logic.host_object(dozer).expect("dozer");
            path_end = unit.movement.path.last().copied();
            ignored = unit.ignored_obstacle_id;
            break;
        }
    }
    // The Dozer machine's exit order reaches the unit on the frames right
    // after the construction update finishes the building, and the ignore
    // follows the live path request; keep the first obstacle it was told to
    // ignore while the dozer walks off.
    for _ in 0..40 {
        advance_logic_frames(&mut logic, 1);
        let u = logic.host_object(dozer).expect("dozer");
        if ignored.is_none() {
            ignored = u.ignored_obstacle_id;
        }
    }
    (logic, pad_id, dozer, action, end_dock, path_end, ignored)
}

#[test]
fn simulation_step_finishes_a_short_build() {
    let (logic, pad_id, dozer, action, end_dock, path_end, _) = run_authored_short_build();
    let pad = logic.host_object(pad_id).unwrap();
    assert!(
        !pad.status.under_construction && pad.construction_percent >= 1.0,
        "a short build must finish, percent={} under={}",
        pad.construction_percent,
        pad.status.under_construction
    );
    let path_end = path_end.expect("completion-frame path to the end dock");
    assert!(
        path_end.distance(end_dock) < 15.0,
        "completion-frame path must end at the end dock, path_end={path_end:?} end_dock={end_dock:?}"
    );
    let unit = logic.host_object(dozer).expect("dozer");
    assert!(
        unit.get_position().distance(action) > 1.0,
        "pos={:?} ai={:?} vel={:?} ignore={:?} target={:?} path={:?}",
        unit.get_position(),
        unit.ai_state(),
        unit.movement.velocity,
        unit.ignored_obstacle_id,
        unit.movement.target_position,
        unit.movement.path
    );
    assert_eq!(
        unit.dozer_task_build_target(), None,
        "a finished build must clear the dozer task or construct buttons stay grey"
    );
}

#[test]
fn owned_construction_completion_walks_the_dozer_off_the_pad() {
    let (logic, pad_id, dozer, action, end_dock, path_end, ignored) = run_authored_short_build();
    // C++ DozerAIUpdate.cpp:628-641: the completion exit is a plain
    // aiMoveToPosition(end dock); only the approach (lines 203/210) ignores the
    // building, so the finished pad is not an ignored obstacle on the way out.
    assert_ne!(
        ignored,
        Some(pad_id),
        "the completion exit must not keep ignoring the finished building"
    );
    assert!(
        path_end.is_some_and(|p| p.distance(end_dock) < 15.0),
        "completion exit must aim at the end dock, path_end={path_end:?} end={end_dock:?}"
    );
    let unit = logic.host_object(dozer).expect("dozer");
    assert!(
        unit.get_position().distance(action) > 1.0,
        "dozer must leave the pad, pos={:?} ai={:?} vel={:?} ignore={:?} target={:?} path={:?}",
        unit.get_position(),
        unit.ai_state(),
        unit.movement.velocity,
        unit.ignored_obstacle_id,
        unit.movement.target_position,
        unit.movement.path
    );
}
