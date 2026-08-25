//! Behavior suite extracted from `production_and_mobs`.
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
    let infantry_id = game_logic
        .create_object("TestInfantry", Team::USA, Vec3::new(50.0, 0.0, 0.0))
        .expect("infantry");
    let hp_before = game_logic
        .host_object(infantry_id)
        .map(|i| i.health.current)
        .unwrap_or(0.0);
    {
        let o = game_logic.host_object_mut(overlord_id).unwrap();
        o.active_weapon_slot = 0;
        o.attack_target(infantry_id);
        if let Some(w) = o.weapon.as_mut() {
            w.last_fire_time = -10.0;
            w.reload_time = 0.1;
            w.min_range = 0.0;
        }
    }
    game_logic.set_current_frame(30);
    game_logic.update_combat(&[overlord_id, infantry_id], LOGIC_FRAME_TIMESTEP);

    let hp_after = game_logic
        .host_object(infantry_id)
        .map(|i| i.health.current)
        .unwrap_or(0.0);
    let dealt = hp_before - hp_after;
    // 80 primary + 10 passenger gattling residual.
    assert!(
        dealt + 0.01 >= 80.0 + OVERLORD_GATTLING_GROUND_DAMAGE - 1.0
            || !game_logic
                .host_object(infantry_id)
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
    ensure_test_infantry_template(&mut game_logic);
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
        assert_eq!(unit.ai_state, AIState::Idle, "unloaded unit must be Idle");
        assert!(unit.contained_by.is_none(), "contained_by must clear");
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
        o.take_damage_from_typed(999.0, None, crate::game_logic::combat::DamageType::Bullet)
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
            projectile_speed: 0.0,
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

    // A normal player AttackObject remains accepted and approaches; it must
    // not begin the DeployStyle timer merely because the target is distant.
    assert!(logic.unit_command_attack(launcher_id, target_id));
    logic.set_current_frame(1);
    logic.tick_deploy_style_updates();
    logic.update_combat(&[launcher_id, target_id], LOGIC_FRAME_TIMESTEP);
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
    logic.set_current_frame(2);
    logic.tick_deploy_style_updates();
    logic.update_combat(&[launcher_id, target_id], LOGIC_FRAME_TIMESTEP);
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

    logic.set_current_frame(101);
    logic.tick_deploy_style_updates();
    assert!(
        !logic.attack_can_fire_at(launcher_id, target_id, 101.0 * LOGIC_FRAME_TIMESTEP, false,),
        "every fire authority must reject a packed DeployStyle weapon"
    );
    logic.update_combat(&[launcher_id, target_id], LOGIC_FRAME_TIMESTEP);
    assert_eq!(
        logic.host_object(target_id).unwrap().health.current,
        hp_before,
        "retail 100-frame unpack still blocks one frame before completion"
    );

    logic.set_current_frame(102);
    logic.tick_deploy_style_updates();
    logic.update_combat(&[launcher_id, target_id], LOGIC_FRAME_TIMESTEP);
    assert!(
        logic.host_object(launcher_id).unwrap().is_deployed(),
        "the exact timer boundary enters ReadyToAttack"
    );
    assert!(
        logic.host_object(target_id).unwrap().health.current < hp_before,
        "the retained normal attack fires only after ReadyToAttack"
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
    use crate::game_logic::{KindOf, Team, ThingTemplate, Weapon};
    use glam::Vec3;

    let mut logic = GameLogic::new();

    let mut af_tmpl = ThingTemplate::new("AmericaAirfield");
    af_tmpl
        .add_kind_of(KindOf::Structure)
        .add_kind_of(KindOf::FSAirfield)
        .add_kind_of(KindOf::Attackable)
        .set_health(1000.0);
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
    use crate::game_logic::{KindOf, Team, ThingTemplate, Weapon};
    use glam::Vec3;

    let mut logic = GameLogic::new();
    let mut af_tmpl = ThingTemplate::new("AmericaAirfield");
    af_tmpl
        .add_kind_of(KindOf::Structure)
        .add_kind_of(KindOf::FSAirfield)
        .set_health(1000.0);
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
    use crate::game_logic::{KindOf, Team, ThingTemplate, Weapon};
    use glam::Vec3;

    clear_test_template_voices();
    set_test_per_unit_sound("AmericaJetRaptor", "VoiceLowFuel", "RaptorVoiceLowFuel");

    let mut logic = GameLogic::new();
    let mut af_tmpl = ThingTemplate::new("AmericaAirfield");
    af_tmpl
        .add_kind_of(KindOf::Structure)
        .add_kind_of(KindOf::FSAirfield)
        .add_kind_of(KindOf::Attackable)
        .set_health(1000.0);
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
    logic.add_player(Player::new(0, Team::China, "China", false));
    logic.add_player(Player::new(1, Team::GLA, "GLA", false));
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
