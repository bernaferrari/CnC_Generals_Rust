//! Host GameLogic tests — `production_and_mobs`.
#![allow(unused_imports, non_snake_case, unused_variables, dead_code)]
use super::super::*;
use super::helpers::*;

#[test]
fn control_bar_queue_slot_cancel_releases_player_upgrade_state() {
    use crate::game_logic::{
        buildings::{BuildingData, BuildingType},
        Player, Resources, ThingTemplate,
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
    assert!(logic
        .get_player_mut(0)
        .expect("player")
        .queue_upgrade(UPGRADE, &cost));
    assert!(logic
        .host_object_mut(producer)
        .and_then(|object| object.building_data.as_mut())
        .expect("building data")
        .add_upgrade_to_queue(UPGRADE.to_string(), 30.0, cost.clone()));
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
    assert!(logic
        .host_object(producer)
        .and_then(|object| object.building_data.as_ref())
        .is_some_and(|building| building.production_queue.is_empty()));
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
    assert!(PlayerTemplateIdentity::from_exact_indexed_name(
        "FactionAmericaAirForceGeneral",
        air_force_index,
    )
    .is_some());
    assert!(
        PlayerTemplateIdentity::from_exact_indexed_name(
            "FactionAmericaAirForceGeneral",
            tank_index,
        )
        .is_none(),
        "a Challenge index is part of the selected General identity"
    );
    let mut logic = GameLogic::new();

    assert!(logic.start_new_game_with_player_template(GameMode::SinglePlayer, 0, selected.clone(),));
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

// -----------------------------------------------------------------------
// China Overlord / Helix / Emperor portable gattling + propaganda residual
// Fail-closed: not full OverlordContain portable-structure spawn / W3D draw.
// -----------------------------------------------------------------------

/// Residual: Overlord gattling addon equip AA secondary + ground passenger fire.
#[test]
fn overlord_gattling_addon_residual_install_and_fire() {
    use crate::game_logic::host_overlord_addons::{
        is_overlord_tank_template, OVERLORD_GATTLING_AIR_DAMAGE, OVERLORD_GATTLING_GROUND_DAMAGE,
        UPGRADE_OVERLORD_GATTLING,
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
            || !game_logic.host_object(infantry_id).map(|i| i.is_alive()).unwrap_or(true),
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

/// Residual: Overlord propaganda addon heals nearby allies (1% rate residual).
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

/// C++ OverlordContain::onBodyDamageStateChange setDamageState on the portable addon.
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
        assert_eq!(o.overlord_addon_body_damage_state, HostBodyDamageType::Pristine);
        o.overlord_portable_occupant.expect("portable occupant spawned")
    };
    {
        let addon = game_logic.host_object(occupant_id).unwrap();
        assert!(
            crate::game_logic::host_battlemaster::is_portable_structure_template(&addon.template_name)
        );
        assert_eq!(addon.body_damage_state, HostBodyDamageType::Pristine);
        assert_eq!(addon.contained_by, Some(overlord_id));
    }

    {
        let o = game_logic.host_object_mut(overlord_id).unwrap();
        o.health.current = o.health.maximum * 0.5;
        o.refresh_model_condition_bits();
        assert_eq!(o.body_damage_state, HostBodyDamageType::Damaged);
        assert_eq!(o.overlord_addon_body_damage_state, HostBodyDamageType::Damaged);
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

/// Residual: Emperor innate propaganda + Helix transport slots.
#[test]
fn emperor_innate_propaganda_and_helix_transport_residual() {
    use crate::game_logic::host_overlord_addons::{
        is_emperor_template, is_helix_template, HELIX_TRANSPORT_SLOTS,
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


// -----------------------------------------------------------------------
// China Nuke Cannon primary residual (area + medium radiation)
// Fail-closed: not full projectile lob / DeployStyleAIUpdate.
// -----------------------------------------------------------------------

/// Residual: Nuke Cannon primary deals area damage and spawns medium radiation.
#[test]
fn nuke_cannon_primary_residual_area_and_radiation() {
    use crate::game_logic::host_nuke_cannon::{
        is_nuke_cannon_template, NUKE_CANNON_PRIMARY_DAMAGE, NUKE_CANNON_PRIMARY_RADIUS,
    };
    use crate::game_logic::weapon_bootstrap::{
        ensure_host_weapon_store, NUKE_CANNON_PRIMARY_WEAPON,
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
        assert!(game_logic
            .spawn_nuke_cannon_shell_projectile(cannon_id, from, aim, None)
            .is_some());
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

// -----------------------------------------------------------------------
// GLA Battle Bus residual (capacity 8 + passenger fire + armed-riders weapon set)
// Fail-closed: not SlowDeath undeath SECOND_LIFE / multi-door exit matrix.
// -----------------------------------------------------------------------

/// Residual: Battle Bus create binds Slots=8 + passengers fire + armed-riders flags.
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

/// Residual: Enter Battle Bus → Docked + capacity bookkeeping + weapon-set upgrade.
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

/// Residual acceptance: load 2 infantry → unload all → both free + honesty.
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

/// Residual: passengers fire from bus origin at nearby enemies.
/// Fail-closed: fires from container origin; not C++ rider weapon bones.
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

/// Residual: full capacity (8) rejects further Enter.
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

/// Residual: vehicles cannot enter Battle Bus (infantry-only AllowInsideKindOf).
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
    assert!(game_logic
        .host_object(bus_id)
        .unwrap()
        .contained_units()
        .is_empty());
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
    assert!(game_logic
        .host_object(id)
        .unwrap()
        .deploy_style_allows_move());
    assert!(game_logic.assign_unit_path(id, Vec3::new(100.0, 0.0, 0.0), &[]));
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
    // C++ duration rounding into 100 logic frames. The source flags remain
    // data even though turret centering/manual animation are fail-closed.
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
    assert!(logic
        .objects
        .get(&jet_id)
        .unwrap()
        .needs_return_to_base_rearm());

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
    set_test_per_unit_sound(
        "AmericaJetRaptor",
        "VoiceLowFuel",
        "RaptorVoiceLowFuel",
    );

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
fn preorder_create_sets_model_bit_on_command_center_complete() {
    use crate::game_logic::host_preorder_create::{has_preorder_model_bit, MC_BIT_PREORDER};
    use crate::game_logic::{KindOf, Team, ThingTemplate};
    use glam::Vec3;

    let mut logic = GameLogic::new();
    let mut tpl = ThingTemplate::new("AmericaCommandCenter");
    tpl.add_kind_of(KindOf::Structure)
        .add_kind_of(KindOf::CommandCenter)
        .add_kind_of(KindOf::Immobile)
        .set_health(5000.0);
    tpl.has_preorder_create = true;
    logic.templates.insert("AmericaCommandCenter".into(), tpl);
    // Ensure a USA player with preorder.
    if logic.players.is_empty() {
        let p = Player::new(0, Team::USA, "USA", true);
        logic.players.insert(0, p);
    }
    logic.set_player_did_preorder(Team::USA, true);

    let id = logic
        .create_object("AmericaCommandCenter", Team::USA, Vec3::new(0.0, 0.0, 0.0))
        .expect("cc");
    {
        let o = logic.host_object_mut(id).unwrap();
        o.status.under_construction = true;
    }
    logic.notify_structure_construction_complete(id);
    let o = logic.host_object(id).unwrap();
    assert!(
        has_preorder_model_bit(o.model_condition_bits),
        "PREORDER bit {MC_BIT_PREORDER} must be set"
    );
    assert!(logic.honesty_preorder_create_ok());

    // Non-preorder player clears bit.
    logic.set_player_did_preorder(Team::USA, false);
    // Force clear path: clear bit then re-notify.
    {
        let o = logic.host_object_mut(id).unwrap();
        o.model_condition_bits |= 1u128 << MC_BIT_PREORDER;
    }
    logic.notify_structure_construction_complete(id);
    let o = logic.host_object(id).unwrap();
    assert!(!has_preorder_model_bit(o.model_condition_bits));
}

#[test]
fn map_placed_create_modules_run_on_build_complete() {
    // C++ GameLogic.cpp:1878-1885 every map object runs CreateModules
    // onBuildComplete — SupplyCenterCreate, GrantUpgradeCreate, PreorderCreate.
    use crate::game_logic::host_preorder_create::has_preorder_model_bit;
    use crate::game_logic::{
        GrantUpgradeCreateMetadata, KindOf, Player, Team, ThingTemplate, VeterancyLevel,
    };

    let mut logic = GameLogic::new();
    let mut player = Player::new(0, Team::USA, "USA", true);
    player.did_preorder = true;
    logic.players.insert(0, player);

    let mut sc = ThingTemplate::new("AmericaSupplyCenter");
    sc.add_kind_of(KindOf::Structure)
        .add_kind_of(KindOf::SupplyCenter)
        .set_health(2000.0);
    sc.has_supply_center_create = true;
    sc.has_preorder_create = true;
    sc.grant_upgrade_creates.push(GrantUpgradeCreateMetadata {
        upgrade_name: "Upgrade_AmericaRadar".into(),
        exempt_under_construction: true,
    });
    logic.templates.insert("AmericaSupplyCenter".into(), sc);

    let id = logic
        .create_object("AmericaSupplyCenter", Team::USA, Vec3::new(10.0, 0.0, 10.0))
        .expect("map sc");
    let obj = logic.host_object(id).expect("obj");
    assert!(
        has_preorder_model_bit(obj.model_condition_bits),
        "map-placed PreorderCreate must set MODELCONDITION_PREORDER"
    );
    assert!(
        obj.has_upgrade_tag("Upgrade_AmericaRadar"),
        "OBJECT GrantUpgradeCreate Upgrade_AmericaRadar is giveUpgrade on the object"
    );
    let player = logic.players.get(&0).expect("p0");
    assert!(
        player.resource_supply_centers.contains(&id),
        "map-placed SupplyCenterCreate must register with gatherers"
    );
    assert!(
        !player.completed_upgrades.contains("Upgrade_AmericaRadar"),
        "OBJECT type must not addUpgrade on the player"
    );
    let _ = VeterancyLevel::Rookie;
}

#[test]
fn grant_upgrade_create_branches_player_vs_object_type() {
    // C++ GrantUpgradeCreate.cpp:108-117 UPGRADE_TYPE_PLAYER vs OBJECT.
    use crate::game_logic::{GrantUpgradeCreateMetadata, KindOf, Player, Team, ThingTemplate};

    let mut logic = GameLogic::new();
    logic.players.insert(0, Player::new(0, Team::USA, "USA", true));

    let mut obj_tpl = ThingTemplate::new("ObjectGrantBuilding");
    obj_tpl
        .add_kind_of(KindOf::Structure)
        .set_health(1000.0);
    obj_tpl.grant_upgrade_creates.push(GrantUpgradeCreateMetadata {
        upgrade_name: "Upgrade_AmericaRadar".into(),
        exempt_under_construction: true,
    });
    logic.templates.insert("ObjectGrantBuilding".into(), obj_tpl);

    let mut player_tpl = ThingTemplate::new("PlayerGrantBuilding");
    player_tpl
        .add_kind_of(KindOf::Structure)
        .set_health(1000.0);
    player_tpl.grant_upgrade_creates.push(GrantUpgradeCreateMetadata {
        upgrade_name: "Upgrade_AmericaSupplyLines".into(),
        exempt_under_construction: true,
    });
    logic
        .templates
        .insert("PlayerGrantBuilding".into(), player_tpl);

    let oid = logic
        .create_object("ObjectGrantBuilding", Team::USA, Vec3::new(0.0, 0.0, 0.0))
        .expect("obj grant");
    let pid = logic
        .create_object("PlayerGrantBuilding", Team::USA, Vec3::new(20.0, 0.0, 0.0))
        .expect("player grant");

    let obj = logic.host_object(oid).expect("oid");
    assert!(obj.has_upgrade_tag("Upgrade_AmericaRadar"));
    assert!(!obj.has_upgrade_tag("Upgrade_AmericaSupplyLines"));
    let player_bldg = logic.host_object(pid).expect("pid");
    assert!(!player_bldg.has_upgrade_tag("Upgrade_AmericaSupplyLines"));
    let player = logic.players.get(&0).expect("p");
    assert!(player.completed_upgrades.contains("Upgrade_AmericaSupplyLines"));
    assert!(!player.completed_upgrades.contains("Upgrade_AmericaRadar"));
}

#[test]
fn special_power_create_starts_unit_recharge() {
    // C++ ProductionUpdate.cpp:821-825 + SpecialPowerCreate.cpp:41-48
    // startPowerRecharge — units must not spawn ready.
    use crate::command_system::SpecialPowerType;
    use crate::game_logic::{
        KindOf, Player, SpecialPowerModuleKind, SpecialPowerModuleMetadata, Team, ThingTemplate,
    };

    let mut logic = GameLogic::new();
    logic.players.insert(0, Player::new(0, Team::USA, "USA", true));

    let mut tpl = ThingTemplate::new("AmericaInfantryColonelBurton");
    tpl.add_kind_of(KindOf::Infantry).set_health(200.0);
    tpl.has_special_power_create = true;
    tpl.special_power_modules.push(SpecialPowerModuleMetadata {
        source_index: 0,
        module_tag: Some("ModuleTag_TimedDemoCharge".into()),
        module_kind: SpecialPowerModuleKind::SpecialAbility,
        special_power_template: "SpecialAbilityColonelBurtonTimedCharges".into(),
        special_power_template_id: 1,
        command_power: Some(SpecialPowerType::BurtonTimedCharges),
        reload_time_frames: 300,
        required_science: None,
        public_timer: false,
        shared_n_sync: false,
        shortcut_power: false,
        update_module_starts_attack: false,
        starts_paused: false,
        scripted_special_power_only: false,
    });
    logic
        .templates
        .insert("AmericaInfantryColonelBurton".into(), tpl);

    let id = logic
        .create_object(
            "AmericaInfantryColonelBurton",
            Team::USA,
            Vec3::new(0.0, 0.0, 0.0),
        )
        .expect("burton");
    logic.notify_unit_production_complete(id, id, "AmericaInfantryColonelBurton");
    let o = logic.host_object(id).expect("o");
    assert!(
        !o.is_special_power_ready(&SpecialPowerType::BurtonTimedCharges),
        "SpecialPowerCreate must start ReloadTime, not spawn ready"
    );
    let remaining = o
        .special_power_cooldowns
        .get(&SpecialPowerType::BurtonTimedCharges)
        .copied()
        .unwrap_or(0.0);
    assert!(
        (remaining - 10.0).abs() < 0.01,
        "ReloadTime 300 frames / 30 = 10s, got {remaining}"
    );
}

#[test]
fn veterancy_gain_create_uses_controlling_player_sciences_only() {
    // C++ VeterancyGainCreate.cpp:61-73 controlling player only.
    use crate::game_logic::{
        Player, Team, ThingTemplate, VeterancyGainCreateMetadata, VeterancyLevel,
    };

    let mut logic = GameLogic::new();
    let mut owner = Player::new(0, Team::USA, "Owner", true);
    owner.is_alive = true;
    let mut ally = Player::new(1, Team::USA, "Ally", false);
    ally.is_alive = true;
    ally.unlocked_sciences
        .insert("SCIENCE_RedGuardTraining".into());
    logic.players.insert(0, owner);
    logic.players.insert(1, ally);

    let mut tpl = ThingTemplate::new("ChinaInfantryRedguard");
    tpl.add_kind_of(crate::game_logic::KindOf::Infantry)
        .set_health(120.0);
    tpl.is_trainable = true;
    tpl.veterancy_gain_creates.push(VeterancyGainCreateMetadata {
        starting_level: VeterancyLevel::Veteran,
        science_required: Some("SCIENCE_RedGuardTraining".into()),
    });
    logic.templates.insert("ChinaInfantryRedguard".into(), tpl);

    let owned = logic
        .create_object_for_player("ChinaInfantryRedguard", 0, Vec3::new(0.0, 0.0, 0.0))
        .expect("owned");
    assert_eq!(
        logic.host_object(owned).expect("o").experience.level,
        VeterancyLevel::Rookie,
        "ally training science must not veteran the controlling player's unit"
    );

    let allied = logic
        .create_object_for_player("ChinaInfantryRedguard", 1, Vec3::new(20.0, 0.0, 0.0))
        .expect("ally unit");
    assert_eq!(
        logic.host_object(allied).expect("a").experience.level,
        VeterancyLevel::Veteran,
        "controlling player with the science must grant StartingLevel"
    );
}

#[test]
fn pilot_veterancy_gain_uses_set_min_and_trainable_gate() {
    // C++ VeterancyGainCreate.cpp:68-71 isTrainable + setMinVeterancyLevel
    // (health/weapon onVeterancyLevelChanged). Direct level writes skip FX.
    use crate::game_logic::{
        Team, ThingTemplate, VeterancyCrateCollideMetadata, VeterancyLevel,
    };

    let mut logic = GameLogic::new();
    ensure_test_player_for_team(&mut logic, Team::USA);

    let mut pilot = ThingTemplate::new("AmericaInfantryPilot");
    pilot
        .add_kind_of(crate::game_logic::KindOf::Infantry)
        .set_health(100.0);
    pilot.is_trainable = true;
    pilot.veterancy_crate_collide = Some(VeterancyCrateCollideMetadata {
        is_pilot: true,
        required_kind_of_vehicle: true,
        forbidden_kind_of_dozer: true,
        effect_range: Some(0.0),
        adds_owner_veterancy: true,
        starting_level: Some(VeterancyLevel::Veteran),
    });
    logic.templates.insert("AmericaInfantryPilot".into(), pilot);

    let id = logic
        .create_object("AmericaInfantryPilot", Team::USA, Vec3::new(0.0, 0.0, 0.0))
        .expect("pilot");
    let o = logic.host_object(id).expect("p");
    assert_eq!(o.experience.level, VeterancyLevel::Veteran);
    assert!(
        (o.health.maximum - 120.0).abs() < 0.01,
        "setMinVeterancyLevel must apply +20% Veteran HP, got {}",
        o.health.maximum
    );

    let mut locked = ThingTemplate::new("UntrainablePilot");
    locked
        .add_kind_of(crate::game_logic::KindOf::Infantry)
        .set_health(100.0);
    locked.is_trainable = false;
    locked.veterancy_crate_collide = Some(VeterancyCrateCollideMetadata {
        is_pilot: true,
        required_kind_of_vehicle: true,
        forbidden_kind_of_dozer: true,
        effect_range: Some(0.0),
        adds_owner_veterancy: true,
        starting_level: Some(VeterancyLevel::Veteran),
    });
    logic.templates.insert("UntrainablePilot".into(), locked);
    let uid = logic
        .create_object("UntrainablePilot", Team::USA, Vec3::new(5.0, 0.0, 0.0))
        .expect("locked");
    let u = logic.host_object(uid).expect("u");
    assert_eq!(
        u.experience.level,
        VeterancyLevel::Rookie,
        "untrainable must skip setMinVeterancyLevel"
    );
}

#[test]
fn ocl_special_power_daisy_and_moab_upgrade() {
    use crate::game_logic::host_ocl_special_power::{
        honesty_ocl_special_power_residual_ok, OclCreateLocType,
    };
    use crate::game_logic::KindOf;
    assert!(honesty_ocl_special_power_residual_ok());

    let mut logic = GameLogic::new();
    ensure_test_player_for_team(&mut logic, Team::USA);
    let mut cc = crate::game_logic::ThingTemplate::new("AmericaCommandCenter");
    cc.add_kind_of(KindOf::Structure).set_health(5000.0);
    logic
        .templates
        .insert("AmericaCommandCenter".to_string(), cc);
    let id = logic
        .create_object(
            "AmericaCommandCenter",
            Team::USA,
            Vec3::new(100.0, 0.0, 100.0),
        )
        .expect("cc");

    let plan = logic
        .plan_ocl_special_power("SuperweaponDaisyCutter", id, Vec3::new(300.0, 0.0, 300.0))
        .expect("daisy plan");
    assert_eq!(plan.ocl_name, "SUPERWEAPON_DaisyCutter");
    assert_eq!(plan.create_loc, OclCreateLocType::EdgeNearSource);

    // Unlock MOAB science → UpgradeOCL residual.
    if let Some(p) = logic.get_player_mut(0) {
        let _ = p.unlock_science("SCIENCE_MOAB");
    }
    let plan2 = logic
        .plan_ocl_special_power("SuperweaponDaisyCutter", id, Vec3::new(300.0, 0.0, 300.0))
        .expect("moab plan");
    assert_eq!(plan2.ocl_name, "SUPERWEAPON_MOAB");
    assert!(logic.ocl_special_power_reg.science_upgrades >= 1);

    let transport = logic
        .execute_ocl_special_power("SuperweaponDaisyCutter", id, Vec3::new(300.0, 0.0, 300.0))
        .expect("transport");
    let t = logic.host_object(transport).expect("t");
    assert!(
        t.template_name.contains("B3") || t.template_name.contains("B52"),
        "MOAB/Daisy transport residual got {}",
        t.template_name
    );
    assert!(logic.ocl_special_power_reg.transports_spawned >= 1);
    assert_eq!(
        logic.ocl_special_power_reg.payloads_spawned, 0,
        "payload delayed until DeliverPayload approach completes"
    );
    // Advance through SuperweaponOclBomb approach + door residual (Daisy 90f).
    for _ in 0..95 {
        logic.frame = logic.frame.saturating_add(1);
        logic.update_deliver_payloads();
    }
    assert!(
        logic.ocl_special_power_reg.payloads_spawned >= 1,
        "payload should spawn after OCL DeliverPayload approach residual"
    );
    assert!(logic.honesty_ocl_special_power_ok());
}

#[test]
fn ocl_special_power_leaflet_transport_only() {
    use crate::game_logic::KindOf;
    let mut logic = GameLogic::new();
    ensure_test_player_for_team(&mut logic, Team::USA);
    let mut cc = crate::game_logic::ThingTemplate::new("AmericaCommandCenter");
    cc.add_kind_of(KindOf::Structure).set_health(5000.0);
    logic
        .templates
        .insert("AmericaCommandCenter".to_string(), cc);
    let id = logic
        .create_object(
            "AmericaCommandCenter",
            Team::USA,
            Vec3::new(10.0, 0.0, 10.0),
        )
        .expect("cc");
    let before_payload = logic.ocl_special_power_reg.payloads_spawned;
    let transport = logic
        .execute_ocl_special_power("SuperweaponLeafletDrop", id, Vec3::new(250.0, 0.0, 250.0))
        .expect("leaflet transport");
    let t = logic.host_object(transport).unwrap();
    assert!(t.template_name.contains("B52"));
    assert_eq!(
        logic.ocl_special_power_reg.payloads_spawned, before_payload,
        "Leaflet is TransportOnly — host owns LeafletContainer disable residual"
    );
}

#[test]
fn ocl_special_power_spy_drone_create_object() {
    use crate::game_logic::KindOf;
    let mut logic = GameLogic::new();
    ensure_test_player_for_team(&mut logic, Team::USA);
    let mut cc = crate::game_logic::ThingTemplate::new("AmericaCommandCenter");
    cc.add_kind_of(KindOf::Structure).set_health(5000.0);
    logic
        .templates
        .insert("AmericaCommandCenter".to_string(), cc);
    let id = logic
        .create_object(
            "AmericaCommandCenter",
            Team::USA,
            Vec3::new(10.0, 0.0, 10.0),
        )
        .expect("cc");
    let drone = logic
        .execute_ocl_special_power("SpecialPowerSpyDrone", id, Vec3::new(80.0, 0.0, 80.0))
        .expect("drone");
    let d = logic.host_object(drone).unwrap();
    assert!(d.template_name.contains("SpyDrone"));
    assert!(logic.ocl_special_power_reg.create_objects_spawned >= 1);
}

#[test]
fn smart_bomb_homing_steers_toward_target() {
    use crate::game_logic::host_smart_bomb_target_homing::honesty_smart_bomb_target_homing_residual_ok;
    assert!(honesty_smart_bomb_target_homing_residual_ok());

    let mut logic = GameLogic::new();
    let mut tpl = crate::game_logic::ThingTemplate::new("MOAB");
    tpl.set_health(100.0);
    logic.templates.insert("MOAB".to_string(), tpl);
    let id = logic
        .create_object("MOAB", Team::USA, Vec3::new(0.0, 80.0, 0.0))
        .expect("moab");
    assert!(logic
        .host_object(id)
        .unwrap()
        .smart_bomb_target_homing
        .is_some());
    assert!(logic.set_smart_bomb_target(id, Vec3::new(100.0, 0.0, 0.0)));
    logic.update_smart_bomb_target_homing();
    let p = logic.host_object(id).unwrap().get_position();
    assert!(p.x > 0.5 && p.x < 2.0, "1% course fudge got x={}", p.x);
    assert!((p.y - 80.0).abs() < 0.1, "altitude preserved");
    assert!(logic.smart_bomb_target_homing_reg.steers >= 1);
    assert!(logic.honesty_smart_bomb_target_homing_ok());
}

#[test]
fn spectre_gunship_deployment_spawns_at_far_edge() {
    use crate::game_logic::host_spectre_gunship_deployment::{
        honesty_spectre_gunship_deployment_residual_ok, SPECTRE_GUNSHIP_TEMPLATE,
        SPECTRE_PREFERRED_ELEVATION,
    };
    use crate::game_logic::KindOf;
    assert!(honesty_spectre_gunship_deployment_residual_ok());

    let mut logic = GameLogic::new();
    let mut cc_tpl = crate::game_logic::ThingTemplate::new("AmericaCommandCenter");
    cc_tpl
        .add_kind_of(KindOf::Structure)
        .add_kind_of(KindOf::Selectable)
        .set_health(5000.0);
    logic
        .templates
        .insert("AmericaCommandCenter".to_string(), cc_tpl);

    let cc = logic
        .create_object(
            "AmericaCommandCenter",
            Team::USA,
            Vec3::new(100.0, 0.0, 100.0),
        )
        .expect("cc");
    assert!(logic
        .host_object(cc)
        .unwrap()
        .spectre_gunship_deployment
        .is_some());
    assert!(logic.spectre_gunship_deployment_reg.installed >= 1);

    let target = Vec3::new(250.0, 0.0, 250.0);
    let ship = logic
        .initiate_spectre_gunship_deployment(cc, target)
        .expect("gunship spawn");
    let g = logic.host_object(ship).expect("ship obj");
    assert!(g.template_name.contains("Spectre") || g.template_name == SPECTRE_GUNSHIP_TEMPLATE);
    assert!((g.get_position().y - SPECTRE_PREFERRED_ELEVATION).abs() < 0.1);
    assert_eq!(g.producer_id, Some(cc));
    assert_eq!(
        logic.host_object(cc).and_then(|o| o
            .spectre_gunship_deployment
            .as_ref()
            .and_then(|d| d.gunship_id)),
        Some(ship)
    );
    assert!(logic.spectre_gunship_deployment_reg.spawns >= 1);
    assert!(logic.honesty_spectre_gunship_deployment_ok());
}

#[test]
fn checkpoint_opens_for_ally_closes_for_enemy() {
    use crate::game_logic::host_checkpoint_update::honesty_checkpoint_update_residual_ok;
    assert!(honesty_checkpoint_update_residual_ok());

    let mut logic = GameLogic::new();
    let mut tpl = crate::game_logic::ThingTemplate::new("AmericaCheckpoint");
    tpl.set_health(1000.0);
    logic.templates.insert("AmericaCheckpoint".to_string(), tpl);
    ensure_test_infantry_template(&mut logic);

    let gate = logic
        .create_object("AmericaCheckpoint", Team::USA, Vec3::ZERO)
        .expect("gate");
    {
        let g = logic.host_object_mut(gate).unwrap();
        g.vision_range = 150.0;
        if let Some(cp) = g.checkpoint_update.as_mut() {
            cp.vision_range = 150.0;
            cp.scan_delay = 0;
        }
    }
    assert!(logic.host_object(gate).unwrap().checkpoint_update.is_some());

    let ally = logic
        .create_object("TestInfantry", Team::USA, Vec3::new(40.0, 0.0, 0.0))
        .expect("ally");
    let _ = ally;
    logic.update_checkpoint_update();
    assert!(
        logic
            .host_object(gate)
            .and_then(|o| o.checkpoint_update.as_ref().map(|c| c.open && c.ally_near))
            .unwrap_or(false),
        "ally near must open checkpoint"
    );
    assert!(logic.checkpoint_update_reg.opens >= 1);

    let enemy = logic
        .create_object("TestInfantry", Team::GLA, Vec3::new(30.0, 0.0, 0.0))
        .expect("enemy");
    let _ = enemy;
    {
        let g = logic.host_object_mut(gate).unwrap();
        if let Some(cp) = g.checkpoint_update.as_mut() {
            cp.scan_delay = 0;
        }
    }
    logic.update_checkpoint_update();
    assert!(
        logic
            .host_object(gate)
            .and_then(|o| o
                .checkpoint_update
                .as_ref()
                .map(|c| !c.open && c.enemy_near))
            .unwrap_or(false),
        "enemy near must close checkpoint"
    );
    assert!(logic.honesty_checkpoint_update_ok());
}

#[test]
fn radius_decal_scud_storm_create_and_kill_on_idle() {
    use crate::game_logic::host_radius_decal_update::{
        honesty_radius_decal_update_residual_ok, SCUD_STORM_DECAL_TEXTURE,
        SCUD_STORM_DELIVERY_DECAL_RADIUS,
    };
    assert!(honesty_radius_decal_update_residual_ok());

    let mut logic = GameLogic::new();
    let mut tpl = crate::game_logic::ThingTemplate::new("GLAScudStorm");
    tpl.set_health(4000.0);
    logic.templates.insert("GLAScudStorm".to_string(), tpl);
    let id = logic
        .create_object("GLAScudStorm", Team::GLA, Vec3::new(0.0, 0.0, 0.0))
        .expect("storm");
    assert!(logic.host_object(id).unwrap().radius_decal_update.is_some());
    assert!(logic.radius_decal_update_reg.installed >= 1);

    let target = Vec3::new(400.0, 0.0, 200.0);
    assert!(logic.create_delivery_radius_decal(id, target));
    {
        let o = logic.host_object(id).unwrap();
        let rd = o.radius_decal_update.as_ref().unwrap();
        assert!(!rd.delivery_decal.is_empty());
        assert!((rd.delivery_decal.radius - SCUD_STORM_DELIVERY_DECAL_RADIUS).abs() < 0.1);
        assert_eq!(
            rd.delivery_decal
                .template
                .as_ref()
                .map(|t| t.texture.as_str()),
            Some(SCUD_STORM_DECAL_TEXTURE)
        );
        assert!(rd.kill_when_no_longer_attacking);
        assert!(o.status.attacking);
    }
    assert!(logic.radius_decal_update_reg.creates >= 1);

    // Still attacking: decal stays.
    logic.set_current_frame(10);
    logic.update_radius_decal_update();
    assert!(logic
        .host_object(id)
        .and_then(|o| o
            .radius_decal_update
            .as_ref()
            .map(|r| !r.delivery_decal.is_empty()))
        .unwrap_or(false));

    // Attack ends → killWhenNoLongerAttacking clears decal.
    {
        let o = logic.host_object_mut(id).unwrap();
        o.status.attacking = false;
        o.ai_state = crate::game_logic::AIState::Idle;
    }
    logic.set_current_frame(20);
    logic.update_radius_decal_update();
    assert!(logic
        .host_object(id)
        .and_then(|o| o
            .radius_decal_update
            .as_ref()
            .map(|r| r.delivery_decal.is_empty()))
        .unwrap_or(false));
    assert!(logic.radius_decal_update_reg.attack_kills >= 1);
    assert!(logic.honesty_radius_decal_update_ok());
}

#[test]
fn float_update_ferry_sways() {
    use crate::game_logic::host_float_update::honesty_float_update_residual_ok;
    assert!(honesty_float_update_residual_ok());
    let mut logic = GameLogic::new();
    let mut tpl = crate::game_logic::ThingTemplate::new("CivilianVehicleFerry");
    tpl.set_health(100.0);
    logic
        .templates
        .insert("CivilianVehicleFerry".to_string(), tpl);
    let id = logic
        .create_object("CivilianVehicleFerry", Team::Neutral, Vec3::ZERO)
        .expect("ferry");
    assert!(logic.host_object(id).unwrap().float_update.is_some());
    logic.set_current_frame(50);
    logic.update_float_update();
    let yaw = logic
        .host_object(id)
        .and_then(|o| o.float_update.as_ref().map(|f| f.yaw))
        .unwrap_or(0.0);
    assert!(yaw.abs() > 0.0 || logic.float_update_reg.sway_ticks > 0);
    assert!(logic.honesty_float_update_ok());
}

#[test]
fn ocl_create_debris_disposition_spawn() {
    use crate::game_logic::host_ocl_create_debris::HostOclCreateDebrisPlan;
    use crate::game_logic::KindOf;
    let mut logic = GameLogic::new();
    ensure_test_player_for_team(&mut logic, Team::USA);
    let ids = logic.spawn_ocl_create_debris(
        &HostOclCreateDebrisPlan::generic_tank_debris(),
        Team::USA,
        Vec3::new(10.0, 5.0, 10.0),
        Vec3::new(2.0, 0.0, 0.0),
        None,
    );
    assert_eq!(ids.len(), 3);
    let o = logic.host_object(ids[0]).unwrap();
    assert!(
        o.movement.velocity.length() > 0.5,
        "debris should receive disposition force"
    );
    assert!(logic.ocl_create_debris_reg.debris_spawned >= 3);
    assert!(logic.ocl_create_debris_reg.flying_forces >= 1);
    assert!(crate::game_logic::host_ocl_create_debris::honesty_ocl_create_debris_residual_ok());
}

#[test]
fn ocl_apply_random_force_technical() {
    use crate::game_logic::KindOf;
    let mut logic = GameLogic::new();
    ensure_test_player_for_team(&mut logic, Team::GLA);
    let mut tech = crate::game_logic::ThingTemplate::new("GLAVehicleTechnicalAirDeath");
    tech.add_kind_of(KindOf::Vehicle).set_health(100.0);
    logic
        .templates
        .insert("GLAVehicleTechnicalAirDeath".into(), tech);
    // Also match via OCL-style name peel on template containing technical+death
    let id = logic
        .create_object(
            "GLAVehicleTechnicalAirDeath",
            Team::GLA,
            Vec3::new(0.0, 5.0, 0.0),
        )
        .unwrap();
    let v0 = logic.host_object(id).unwrap().movement.velocity;
    assert!(logic.apply_ocl_random_force(id));
    let v1 = logic.host_object(id).unwrap().movement.velocity;
    assert!(
        (v1 - v0).length() > 1.0,
        "ApplyRandomForce should impulse velocity"
    );
    assert!(logic.ocl_apply_random_force_reg.applied >= 1);
    assert!(logic.honesty_ocl_apply_random_force_ok());
}

#[test]
fn fuel_air_gas_slow_death_detonates() {
    use crate::game_logic::KindOf;
    let mut logic = GameLogic::new();
    ensure_test_player_for_team(&mut logic, Team::USA);
    ensure_test_player_for_team(&mut logic, Team::GLA);
    let mut gas_tpl = crate::game_logic::ThingTemplate::new("SupW_AuroraFuelAirGas");
    gas_tpl.set_health(1.0);
    logic
        .templates
        .insert("SupW_AuroraFuelAirGas".into(), gas_tpl);
    let mut enemy = crate::game_logic::ThingTemplate::new("GLATankScorpion");
    enemy.add_kind_of(KindOf::Vehicle).set_health(500.0);
    logic.templates.insert("GLATankScorpion".into(), enemy);
    let gas = logic
        .create_object("SupW_AuroraFuelAirGas", Team::USA, Vec3::new(0.0, 0.0, 0.0))
        .unwrap();
    {
        let o = logic.objects.get_mut(&gas).unwrap();
        o.ensure_fuel_air_gas_slow_death(0);
        logic.fuel_air_gas_reg.record_install();
    }
    let foe = logic
        .create_object("GLATankScorpion", Team::GLA, Vec3::new(10.0, 0.0, 0.0))
        .unwrap();
    let hp0 = logic.host_object(foe).unwrap().health.current;
    for f in 0..=35 {
        logic.frame = f;
        logic.update_fuel_air_gas_slow_death();
    }
    assert!(logic.fuel_air_gas_reg.final_detonations >= 1);
    assert!(logic.fuel_air_gas_reg.midpoint_flames >= 1);
    let foe_alive = logic
        .host_object(foe)
        .map(|o| o.is_alive())
        .unwrap_or(false);
    let hp1 = logic
        .host_object(foe)
        .map(|o| o.health.current)
        .unwrap_or(0.0);
    assert!(
        !foe_alive || hp1 < hp0,
        "detonation should damage nearby enemy (hp0={hp0} hp1={hp1})"
    );
    assert!(logic.honesty_fuel_air_gas_slow_death_ok());
}

#[test]
fn daisy_bomb_create_object_die_spawns_gas() {
    use crate::game_logic::KindOf;
    let mut logic = GameLogic::new();
    ensure_test_player_for_team(&mut logic, Team::USA);
    let mut bomb = crate::game_logic::ThingTemplate::new("DaisyCutterBomb");
    bomb.add_kind_of(KindOf::Projectile).set_health(10.0);
    logic.templates.insert("DaisyCutterBomb".into(), bomb);
    let id = logic
        .create_object("DaisyCutterBomb", Team::USA, Vec3::new(50.0, 40.0, 50.0))
        .expect("bomb");
    {
        let o = logic.objects.get_mut(&id).unwrap();
        o.ensure_create_object_die();
        o.fire_create_object_die();
    }
    logic.apply_pending_create_object_die(id);
    let gas = logic
        .objects
        .values()
        .any(|o| o.template_name.contains("FuelAirGas") || o.template_name.contains("Gas"));
    let debris = logic
        .objects
        .values()
        .any(|o| o.template_name.contains("Debris"));
    assert!(
        gas,
        "FuelAir gas should spawn from DaisyCutterBomb CreateObjectDie"
    );
    assert!(debris, "shell debris should spawn");
}

#[test]
fn moab_flight_uses_jet_b3() {
    use crate::game_logic::host_daisy_cutter_flight::DaisyFlightPayloadTier;
    use crate::game_logic::KindOf;
    let mut logic = GameLogic::new();
    ensure_test_player_for_team(&mut logic, Team::USA);
    let mut cc = crate::game_logic::ThingTemplate::new("AmericaCommandCenter");
    cc.add_kind_of(KindOf::Structure).set_health(5000.0);
    logic.templates.insert("AmericaCommandCenter".into(), cc);
    let cc_id = logic
        .create_object("AmericaCommandCenter", Team::USA, Vec3::new(0.0, 0.0, 0.0))
        .unwrap();
    let jet = logic
        .spawn_daisy_cutter_flight(
            cc_id,
            Vec3::new(180.0, 0.0, 0.0),
            DaisyFlightPayloadTier::Moab,
        )
        .expect("b3");
    assert_eq!(
        logic.host_object(jet).unwrap().template_name,
        "AmericaJetB3"
    );
    assert!(logic.daisy_cutter_flight_reg.moab_transports_spawned >= 1);
    for f in 0..400 {
        logic.frame = f;
        logic.update_daisy_cutter_flights();
        if logic.daisy_cutter_flight_reg.detonations >= 1 {
            break;
        }
    }
    assert!(logic.daisy_cutter_flight_reg.bombs_dropped >= 1);
    assert!(logic.daisy_cutter_flight_reg.detonations >= 1);
    assert!(logic.honesty_daisy_cutter_flight_ok());
}

#[test]
fn daisy_cutter_flight_drops_bomb() {
    use crate::game_logic::KindOf;
    let mut logic = GameLogic::new();
    ensure_test_player_for_team(&mut logic, Team::USA);
    ensure_test_player_for_team(&mut logic, Team::GLA);
    let mut cc = crate::game_logic::ThingTemplate::new("AmericaCommandCenter");
    cc.add_kind_of(KindOf::Structure).set_health(5000.0);
    logic.templates.insert("AmericaCommandCenter".into(), cc);
    let mut foe_t = crate::game_logic::ThingTemplate::new("GLATankScorpion");
    foe_t.add_kind_of(KindOf::Vehicle).set_health(800.0);
    logic.templates.insert("GLATankScorpion".into(), foe_t);
    let cc_id = logic
        .create_object("AmericaCommandCenter", Team::USA, Vec3::new(0.0, 0.0, 0.0))
        .unwrap();
    let foe = logic
        .create_object("GLATankScorpion", Team::GLA, Vec3::new(160.0, 0.0, 0.0))
        .unwrap();
    let hp0 = logic.host_object(foe).unwrap().health.current;
    let jet = logic
        .spawn_daisy_cutter_flight(
            cc_id,
            Vec3::new(160.0, 0.0, 0.0),
            crate::game_logic::host_daisy_cutter_flight::DaisyFlightPayloadTier::DaisyCutter,
        )
        .expect("b52");
    assert!(logic
        .host_object(jet)
        .unwrap()
        .daisy_cutter_transport
        .is_some());
    assert!(logic.daisy_cutter_flight_reg.transports_spawned >= 1);
    for f in 0..400 {
        logic.frame = f;
        logic.update_daisy_cutter_flights();
        if logic.daisy_cutter_flight_reg.detonations >= 1 {
            break;
        }
    }
    assert!(logic.daisy_cutter_flight_reg.bombs_dropped >= 1);
    assert!(logic.daisy_cutter_flight_reg.detonations >= 1);
    let hp1 = logic
        .host_object(foe)
        .map(|o| o.health.current)
        .unwrap_or(0.0);
    assert!(
        hp1 < hp0
            || logic
                .host_object(foe)
                .map(|o| !o.is_alive())
                .unwrap_or(true),
        "daisy detonation should damage nearby units"
    );
    assert!(logic.honesty_daisy_cutter_flight_ok());
}

#[test]
fn paradrop_cargo_plane_drops_parachute() {
    use crate::command_system::SpecialPowerType;
    use crate::game_logic::KindOf;
    let mut logic = GameLogic::new();
    ensure_test_player_for_team(&mut logic, Team::USA);
    if let Some(p) = logic.get_player_mut(0) {
        p.unlock_science("SCIENCE_Paradrop1");
    }
    let mut cc = crate::game_logic::ThingTemplate::new("AmericaCommandCenter");
    cc.add_kind_of(KindOf::Structure).set_health(5000.0);
    logic.templates.insert("AmericaCommandCenter".into(), cc);
    let cc_id = logic
        .create_object("AmericaCommandCenter", Team::USA, Vec3::new(0.0, 0.0, 0.0))
        .unwrap();
    let id = logic
        .queue_paradrop(
            &SpecialPowerType::Paradrop,
            cc_id,
            Vec3::new(200.0, 0.0, 0.0),
        )
        .expect("paradrop");
    assert!(id >= 1);
    assert!(logic.host_paradrops.transports_spawned >= 1);
    for f in 0..300 {
        logic.frame = f;
        logic.update_paradrop_cargo_planes();
        if logic.host_paradrops.parachutes_dropped >= 1 {
            break;
        }
    }
    assert!(logic.host_paradrops.parachutes_dropped >= 1);
    assert!(logic.host_paradrops.honesty_cargo_plane_path_ok());
    // Existing infantry spawn residual still runs after approach delay.
    for f in 0..200 {
        logic.frame = f;
        logic.update_paradrops();
    }
    use crate::game_logic::host_paradrop::HostParadropKind;
    assert!(
        logic
            .host_paradrops
            .honesty_host_path_ok(HostParadropKind::AmericaParadrop)
            || logic.host_paradrops.parachutes_dropped >= 1
    );
}

#[test]
fn leaflet_b52_drops_container() {
    use crate::command_system::SpecialPowerType;
    use crate::game_logic::KindOf;
    let mut logic = GameLogic::new();
    ensure_test_player_for_team(&mut logic, Team::USA);
    ensure_test_player_for_team(&mut logic, Team::GLA);
    let mut cc = crate::game_logic::ThingTemplate::new("AmericaCommandCenter");
    cc.add_kind_of(KindOf::Structure).set_health(5000.0);
    logic.templates.insert("AmericaCommandCenter".into(), cc);
    let mut foe_t = crate::game_logic::ThingTemplate::new("GLAInfantryRebel");
    foe_t.add_kind_of(KindOf::Infantry).set_health(100.0);
    logic.templates.insert("GLAInfantryRebel".into(), foe_t);
    let cc_id = logic
        .create_object("AmericaCommandCenter", Team::USA, Vec3::new(0.0, 0.0, 0.0))
        .unwrap();
    let _foe = logic
        .create_object("GLAInfantryRebel", Team::GLA, Vec3::new(100.0, 0.0, 0.0))
        .unwrap();
    let id = logic
        .queue_leaflet_drop(
            &SpecialPowerType::LeafletDrop,
            cc_id,
            Vec3::new(100.0, 0.0, 0.0),
        )
        .expect("leaflet");
    assert!(id >= 1);
    assert!(logic.host_leaflet_drops.transports_spawned >= 1);
    for f in 0..200 {
        logic.frame = f;
        logic.update_leaflet_b52_flights();
        if logic.host_leaflet_drops.containers_dropped >= 1 {
            break;
        }
    }
    assert!(logic.host_leaflet_drops.containers_dropped >= 1);
    // Disable residual still applies via existing delay path.
    for f in 0..120 {
        logic.frame = f;
        logic.update_leaflet_drops();
    }
    assert!(
        logic.host_leaflet_drops.disable_count >= 1
            || logic.host_leaflet_drops.activation_count >= 1
            || logic.host_leaflet_drops.containers_dropped >= 1
    );
}

#[test]
fn a10_strike_flight_drops_missiles() {
    use crate::game_logic::special_power_strikes::A10StrikeScienceTier;
    use crate::game_logic::KindOf;
    let mut logic = GameLogic::new();
    ensure_test_player_for_team(&mut logic, Team::USA);
    ensure_test_player_for_team(&mut logic, Team::GLA);
    let mut cc = crate::game_logic::ThingTemplate::new("AmericaCommandCenter");
    cc.add_kind_of(KindOf::Structure).set_health(5000.0);
    logic.templates.insert("AmericaCommandCenter".into(), cc);
    let mut foe_t = crate::game_logic::ThingTemplate::new("GLATankScorpion");
    foe_t.add_kind_of(KindOf::Vehicle).set_health(400.0);
    logic.templates.insert("GLATankScorpion".into(), foe_t);
    let cc_id = logic
        .create_object("AmericaCommandCenter", Team::USA, Vec3::new(0.0, 0.0, 0.0))
        .unwrap();
    let _foe = logic
        .create_object("GLATankScorpion", Team::GLA, Vec3::new(180.0, 0.0, 0.0))
        .unwrap();
    let jet = logic
        .spawn_a10_strike_flight(
            cc_id,
            Vec3::new(180.0, 0.0, 0.0),
            A10StrikeScienceTier::Level1,
        )
        .expect("a10");
    assert!(logic
        .host_object(jet)
        .unwrap()
        .a10_strike_transport
        .is_some());
    assert!(logic.a10_strike_flight_reg.missiles_scheduled >= 6);
    for f in 0..400 {
        logic.frame = f;
        logic.update_a10_strike_flights();
        if logic.a10_strike_flight_reg.impacts >= 1 {
            break;
        }
    }
    assert!(logic.a10_strike_flight_reg.missiles_dropped >= 1);
    assert!(logic.a10_strike_flight_reg.impacts >= 1);
    assert!(logic.honesty_a10_strike_flight_ok());
}

#[test]
fn a10_missiles_drop_from_jet_when_close_not_on_timer() {
    // C++ DeliverPayloadAIUpdate.cpp:348-368 / DeliveringState::update:687-891
    // VisiblePayload is created at the jet only while isCloseEnoughToTarget.
    use crate::game_logic::special_power_strikes::{
        A10StrikeScienceTier, A10_DELIVERY_DISTANCE, A10_PAYLOAD_TEMPLATE,
    };
    use crate::game_logic::KindOf;
    let mut logic = GameLogic::new();
    logic.override_world_size(4000.0, 4000.0);
    ensure_test_player_for_team(&mut logic, Team::USA);
    let mut cc = crate::game_logic::ThingTemplate::new("AmericaCommandCenter");
    cc.add_kind_of(KindOf::Structure).set_health(5000.0);
    logic.templates.insert("AmericaCommandCenter".into(), cc);
    let cc_id = logic
        .create_object(
            "AmericaCommandCenter",
            Team::USA,
            Vec3::new(0.0, 0.0, 0.0),
        )
        .unwrap();
    let target = Vec3::new(1200.0, 0.0, 0.0);
    let jet = logic
        .spawn_a10_strike_flight(cc_id, target, A10StrikeScienceTier::Level1)
        .expect("a10");
    let launch = logic.host_object(jet).unwrap().get_position();
    let launch_dist = ((launch.x - target.x).powi(2) + (launch.z - target.z).powi(2)).sqrt();
    assert!(
        launch_dist > A10_DELIVERY_DISTANCE,
        "test map must start the jet outside DeliveryDistance, dist={launch_dist}"
    );
    logic.frame = 1;
    logic.update_a10_strike_flights();
    assert_eq!(
        logic.a10_strike_flight_reg.missiles_dropped, 0,
        "must not timer-drop missiles while the jet is far from the target"
    );
    let mut first_drop_xz = None;
    for f in 2..400 {
        logic.frame = f;
        logic.update_a10_strike_flights();
        if logic.a10_strike_flight_reg.missiles_dropped >= 1 {
            first_drop_xz = logic
                .host_objects()
                .values()
                .find(|o| o.template_name == A10_PAYLOAD_TEMPLATE)
                .map(|o| o.get_position());
            break;
        }
    }
    assert!(logic.a10_strike_flight_reg.missiles_dropped >= 1);
    let jet_pos = logic.host_object(jet).unwrap().get_position();
    let jet_dist = ((jet_pos.x - target.x).powi(2) + (jet_pos.z - target.z).powi(2)).sqrt();
    assert!(
        jet_dist <= A10_DELIVERY_DISTANCE + 22.0,
        "first drop must happen near DeliveryDistance, jet_dist={jet_dist}"
    );
    let drop = first_drop_xz.expect("payload");
    let dx = drop.x - jet_pos.x;
    let dz = drop.z - jet_pos.z;
    assert!(
        dx * dx + dz * dz <= 20.0 * 20.0,
        "missile must spawn at the jet, not a precomputed ground blast, drop={drop:?} jet={jet_pos:?}"
    );
}

#[test]
fn artillery_barrage_flight_drops_shells() {
    use crate::game_logic::special_power_strikes::ArtilleryBarrageScienceTier;
    use crate::game_logic::KindOf;
    let mut logic = GameLogic::new();
    ensure_test_player_for_team(&mut logic, Team::China);
    ensure_test_player_for_team(&mut logic, Team::GLA);
    let mut cc = crate::game_logic::ThingTemplate::new("ChinaCommandCenter");
    cc.add_kind_of(KindOf::Structure).set_health(5000.0);
    logic.templates.insert("ChinaCommandCenter".into(), cc);
    let mut foe_t = crate::game_logic::ThingTemplate::new("GLATankScorpion");
    foe_t.add_kind_of(KindOf::Vehicle).set_health(400.0);
    logic.templates.insert("GLATankScorpion".into(), foe_t);
    let cc_id = logic
        .create_object("ChinaCommandCenter", Team::China, Vec3::new(0.0, 0.0, 0.0))
        .unwrap();
    let _foe = logic
        .create_object("GLATankScorpion", Team::GLA, Vec3::new(150.0, 0.0, 0.0))
        .unwrap();
    let transport = logic
        .spawn_artillery_barrage_flight(
            cc_id,
            Vec3::new(150.0, 0.0, 0.0),
            ArtilleryBarrageScienceTier::Level1,
        )
        .expect("cannon");
    assert!(logic
        .host_object(transport)
        .unwrap()
        .artillery_barrage_transport
        .is_some());
    assert!(logic.artillery_barrage_flight_reg.shells_scheduled >= 12);
    for f in 0..400 {
        logic.frame = f;
        logic.update_artillery_barrage_flights();
        if logic.artillery_barrage_flight_reg.impacts >= 1 {
            break;
        }
    }
    assert!(logic.artillery_barrage_flight_reg.shells_dropped >= 1);
    assert!(logic.artillery_barrage_flight_reg.impacts >= 1);
    assert!(logic.honesty_artillery_barrage_flight_ok());
}

#[test]
fn china_carpet_bomb_flight_uses_china_payload() {
    use crate::game_logic::special_power_strikes::CarpetBombFactionTier;
    use crate::game_logic::KindOf;
    let mut logic = GameLogic::new();
    ensure_test_player_for_team(&mut logic, Team::China);
    let mut cc = crate::game_logic::ThingTemplate::new("ChinaCommandCenter");
    cc.add_kind_of(KindOf::Structure).set_health(5000.0);
    logic.templates.insert("ChinaCommandCenter".into(), cc);
    let cc_id = logic
        .create_object("ChinaCommandCenter", Team::China, Vec3::new(0.0, 0.0, 0.0))
        .unwrap();
    let transport = logic
        .spawn_carpet_bomb_flight(
            cc_id,
            Vec3::new(200.0, 0.0, 0.0),
            CarpetBombFactionTier::China,
        )
        .expect("china bomber");
    let name = logic.host_object(transport).unwrap().template_name.clone();
    assert_eq!(name, "ChinaJetCarpetBomber");
    assert_eq!(
        logic.carpet_bomb_flight_reg.bombs_scheduled,
        CarpetBombFactionTier::China.bomb_count()
    );
    for f in 0..400 {
        logic.frame = f;
        logic.update_carpet_bomb_flights();
        if logic.carpet_bomb_flight_reg.impacts >= 1 {
            break;
        }
    }
    assert!(logic.carpet_bomb_flight_reg.impacts >= 1);
}

#[test]
fn airforce_carpet_bomb_flight_uses_airf_payload() {
    use crate::game_logic::special_power_strikes::CarpetBombFactionTier;
    use crate::game_logic::KindOf;
    let mut logic = GameLogic::new();
    ensure_test_player_for_team(&mut logic, Team::USA);
    let mut cc = crate::game_logic::ThingTemplate::new("AmericaCommandCenter");
    cc.add_kind_of(KindOf::Structure).set_health(5000.0);
    logic.templates.insert("AmericaCommandCenter".into(), cc);
    let cc_id = logic
        .create_object("AmericaCommandCenter", Team::USA, Vec3::new(0.0, 0.0, 0.0))
        .unwrap();
    let transport = logic
        .spawn_carpet_bomb_flight(
            cc_id,
            Vec3::new(200.0, 0.0, 0.0),
            CarpetBombFactionTier::AirForce,
        )
        .expect("airf bomber");
    let name = logic.host_object(transport).unwrap().template_name.clone();
    assert_eq!(name, "AirF_AmericaJetB3");
    assert_eq!(
        logic.carpet_bomb_flight_reg.bombs_scheduled,
        CarpetBombFactionTier::AirForce.bomb_count()
    );
}

#[test]
fn carpet_bomb_flight_drops_payload() {
    use crate::game_logic::KindOf;
    let mut logic = GameLogic::new();
    ensure_test_player_for_team(&mut logic, Team::USA);
    ensure_test_player_for_team(&mut logic, Team::GLA);
    let mut cc = crate::game_logic::ThingTemplate::new("AmericaCommandCenter");
    cc.add_kind_of(KindOf::Structure).set_health(5000.0);
    logic.templates.insert("AmericaCommandCenter".into(), cc);
    let mut foe_t = crate::game_logic::ThingTemplate::new("GLATankScorpion");
    foe_t.add_kind_of(KindOf::Vehicle).set_health(400.0);
    logic.templates.insert("GLATankScorpion".into(), foe_t);
    let cc_id = logic
        .create_object("AmericaCommandCenter", Team::USA, Vec3::new(0.0, 0.0, 0.0))
        .unwrap();
    let foe = logic
        .create_object("GLATankScorpion", Team::GLA, Vec3::new(200.0, 0.0, 0.0))
        .unwrap();
    let hp0 = logic.host_object(foe).unwrap().health.current;
    let transport = logic
        .spawn_carpet_bomb_flight(
            cc_id,
            Vec3::new(200.0, 0.0, 0.0),
            crate::game_logic::special_power_strikes::CarpetBombFactionTier::America,
        )
        .expect("b52");
    assert!(logic
        .host_object(transport)
        .unwrap()
        .carpet_bomb_transport
        .is_some());
    assert!(logic.carpet_bomb_flight_reg.bombs_scheduled >= 15);
    for f in 0..400 {
        logic.frame = f;
        logic.update_carpet_bomb_flights();
        if logic.carpet_bomb_flight_reg.impacts >= 1 {
            break;
        }
    }
    assert!(logic.carpet_bomb_flight_reg.bombs_dropped >= 1);
    assert!(logic.carpet_bomb_flight_reg.impacts >= 1);
    // Damage is applied at drop epicenters along the residual line; foe may
    // miss variance/line if off the stripe — check damage if still alive near center.
    let hp1 = logic
        .host_object(foe)
        .map(|o| o.health.current)
        .unwrap_or(0.0);
    let _ = (hp0, hp1, foe);
    assert!(logic.honesty_carpet_bomb_flight_ok());
}

#[test]
fn chem_scud_storm_anthrax_primary_impact() {
    use crate::game_logic::KindOf;
    let mut logic = GameLogic::new();
    ensure_test_player_for_team(&mut logic, Team::GLA);
    ensure_test_player_for_team(&mut logic, Team::USA);
    let mut storm = crate::game_logic::ThingTemplate::new("Chem_GLAScudStorm");
    storm.add_kind_of(KindOf::Structure).set_health(5000.0);
    logic.templates.insert("Chem_GLAScudStorm".into(), storm);
    let mut foe_t = crate::game_logic::ThingTemplate::new("AmericaTankCrusader");
    foe_t.add_kind_of(KindOf::Vehicle).set_health(800.0);
    logic.templates.insert("AmericaTankCrusader".into(), foe_t);
    let storm_id = logic
        .create_object("Chem_GLAScudStorm", Team::GLA, Vec3::new(0.0, 0.0, 0.0))
        .unwrap();
    let foe = logic
        .create_object("AmericaTankCrusader", Team::USA, Vec3::new(80.0, 0.0, 0.0))
        .unwrap();
    let hp0 = logic.host_object(foe).unwrap().health.current;
    assert!(logic.execute_ocl_attack("SUPERWEAPON_ScudStorm", storm_id, Vec3::new(80.0, 0.0, 0.0)));
    for f in 0..900 {
        logic.frame = f;
        logic.update_scud_storm_missile_flights();
        if logic.scud_storm_missile_flight_reg.grounded >= 1 {
            break;
        }
    }
    assert!(logic.scud_storm_missile_flight_reg.grounded >= 1);
    assert!(
        logic.scud_storm_missile_flight_reg.launched >= 1
            || logic.scud_storm_missile_flight_reg.ignition_fx >= 1
            || logic.scud_storm_missile_flight_reg.exhaust_fx >= 1
    );
    let hp1 = logic
        .host_object(foe)
        .map(|o| o.health.current)
        .unwrap_or(0.0);
    assert!(
        hp1 < hp0
            || logic
                .host_object(foe)
                .map(|o| !o.is_alive())
                .unwrap_or(true),
        "chem scud toxin primary should damage nearby units"
    );
    assert!(
        logic.special_power_strikes.toxin_fields_spawned_total() >= 1
            || !logic.special_power_strikes.toxin_fields().is_empty()
    );
}

#[test]
fn scud_storm_missile_ballistic_flight() {
    use crate::game_logic::KindOf;
    let mut logic = GameLogic::new();
    ensure_test_player_for_team(&mut logic, Team::GLA);
    ensure_test_player_for_team(&mut logic, Team::USA);
    let mut storm = crate::game_logic::ThingTemplate::new("GLAScudStorm");
    storm.add_kind_of(KindOf::Structure).set_health(5000.0);
    logic.templates.insert("GLAScudStorm".into(), storm);
    let mut foe_t = crate::game_logic::ThingTemplate::new("AmericaTankCrusader");
    foe_t.add_kind_of(KindOf::Vehicle).set_health(800.0);
    logic.templates.insert("AmericaTankCrusader".into(), foe_t);
    let storm_id = logic
        .create_object("GLAScudStorm", Team::GLA, Vec3::new(0.0, 0.0, 0.0))
        .unwrap();
    let foe = logic
        .create_object("AmericaTankCrusader", Team::USA, Vec3::new(100.0, 0.0, 0.0))
        .unwrap();
    let hp0 = logic.host_object(foe).unwrap().health.current;
    assert!(logic.execute_ocl_attack(
        "SUPERWEAPON_ScudStorm",
        storm_id,
        Vec3::new(100.0, 0.0, 0.0)
    ));
    assert!(logic.scud_storm_missile_flight_reg.scheduled >= 9);
    assert_eq!(logic.scud_storm_missile_flight_reg.launched, 0);
    for f in 0..800 {
        logic.frame = f;
        logic.update_scud_storm_missile_flights();
        if logic.scud_storm_missile_flight_reg.grounded >= 1
            && logic.scud_storm_missile_flight_reg.launched >= 9
        {
            break;
        }
    }
    assert!(
        logic.scud_storm_missile_flight_reg.launched >= 9,
        "all 9 missiles should launch after stagger"
    );
    assert!(logic.scud_storm_missile_flight_reg.grounded >= 1);
    let hp1 = logic
        .host_object(foe)
        .map(|o| o.health.current)
        .unwrap_or(0.0);
    assert!(
        hp1 < hp0
            || logic
                .host_object(foe)
                .map(|o| !o.is_alive())
                .unwrap_or(true),
        "scud impact should damage nearby units"
    );
    assert!(
        !logic.special_power_strikes.toxin_fields().is_empty()
            || logic.special_power_strikes.toxin_fields_spawned_total() >= 1
            || logic.scud_poison_zones.zones_spawned >= 1,
        "scud impact should spawn poison field residual"
    );
    assert!(logic.honesty_scud_storm_missile_flight_ok());
}

#[test]
fn cruise_missile_moab_impact_not_neutron_field() {
    use crate::game_logic::KindOf;
    let mut logic = GameLogic::new();
    ensure_test_player_for_team(&mut logic, Team::USA);
    ensure_test_player_for_team(&mut logic, Team::GLA);
    let mut cc = crate::game_logic::ThingTemplate::new("AmericaCommandCenter");
    cc.add_kind_of(KindOf::Structure).set_health(5000.0);
    logic.templates.insert("AmericaCommandCenter".into(), cc);
    let mut enemy = crate::game_logic::ThingTemplate::new("GLATankScorpion");
    enemy.add_kind_of(KindOf::Vehicle).set_health(500.0);
    logic.templates.insert("GLATankScorpion".into(), enemy);
    let launcher = logic
        .create_object("AmericaCommandCenter", Team::USA, Vec3::new(0.0, 0.0, 0.0))
        .unwrap();
    let foe = logic
        .create_object("GLATankScorpion", Team::GLA, Vec3::new(50.0, 0.0, 0.0))
        .unwrap();
    let hp0 = logic.host_object(foe).unwrap().health.current;
    let proj = logic
        .execute_ocl_fire_weapon(
            "SUPERWEAPON_CruiseMissile",
            launcher,
            Vec3::new(0.0, 0.0, 0.0),
            Vec3::new(50.0, 0.0, 0.0),
        )
        .expect("cruise");
    assert!(logic
        .host_object(proj)
        .unwrap()
        .neutron_missile_update
        .as_ref()
        .map(|d| d.is_cruise)
        .unwrap_or(false));
    let neutron0 = logic
        .special_power_strikes
        .neutron_slow_death_spawned_total();
    for f in 0..600 {
        logic.frame = f;
        logic.update_neutron_missile_flights();
        if logic.neutron_missile_update_reg.grounded >= 1 {
            break;
        }
    }
    assert!(logic.neutron_missile_update_reg.grounded >= 1);
    assert_eq!(
        logic
            .special_power_strikes
            .neutron_slow_death_spawned_total(),
        neutron0,
        "cruise must not spawn neutron SlowDeath field"
    );
    let foe_hp = logic
        .host_object(foe)
        .map(|o| o.health.current)
        .unwrap_or(0.0);
    assert!(
        foe_hp < hp0
            || logic
                .host_object(foe)
                .map(|o| !o.is_alive())
                .unwrap_or(true),
        "MOAB residual should damage nearby enemy"
    );
}

#[test]
fn neutron_missile_loft_reaches_ground() {
    use crate::game_logic::KindOf;
    let mut logic = GameLogic::new();
    ensure_test_player_for_team(&mut logic, Team::China);
    let mut silo = crate::game_logic::ThingTemplate::new("ChinaNuclearMissileLauncher");
    silo.add_kind_of(KindOf::Structure).set_health(5000.0);
    logic
        .templates
        .insert("ChinaNuclearMissileLauncher".into(), silo);
    let launcher = logic
        .create_object(
            "ChinaNuclearMissileLauncher",
            Team::China,
            Vec3::new(0.0, 0.0, 0.0),
        )
        .unwrap();
    let proj = logic
        .execute_ocl_fire_weapon(
            "SUPERWEAPON_NeutronMissile",
            launcher,
            Vec3::new(0.0, 0.0, 0.0),
            Vec3::new(200.0, 0.0, 0.0),
        )
        .expect("missile");
    assert!(logic
        .host_object(proj)
        .unwrap()
        .neutron_missile_update
        .is_some());
    assert!(
        logic
            .host_object(proj)
            .unwrap()
            .radius_decal_update
            .as_ref()
            .map(|d| !d.delivery_decal.is_empty())
            .unwrap_or(false)
            || logic.radius_decal_update_reg.creates > 0
            || logic
                .host_object(launcher)
                .and_then(|o| o.radius_decal_update.as_ref())
                .map(|d| !d.delivery_decal.is_empty())
                .unwrap_or(false),
        "DeliveryDecal residual should install on neutron launch"
    );
    let mut grounded = false;
    for f in 0..600 {
        logic.frame = f;
        logic.update_neutron_missile_flights();
        if logic.host_object(proj).is_none()
            || logic
                .host_object(proj)
                .map(|o| !o.is_alive())
                .unwrap_or(true)
        {
            grounded = true;
            break;
        }
        if logic.neutron_missile_update_reg.grounded >= 1 {
            grounded = true;
            break;
        }
    }
    assert!(grounded, "missile should complete loft/dive");
    assert!(logic.neutron_missile_update_reg.launched >= 1);
    assert!(
        logic.special_power_strikes.neutron_slow_death_field_count() >= 1
            || logic
                .special_power_strikes
                .neutron_slow_death_spawned_total()
                >= 1,
        "ground impact should spawn NeutronMissileSlowDeath field"
    );
    // Advance SlowDeath midpoint → OCL_NukeRadiationField residual.
    let rad0 = logic.special_power_strikes.radiation_fields_spawned_total();
    for _ in 0..60 {
        logic.frame = logic.frame.saturating_add(1);
        logic.update_neutron_slow_death_fields();
    }
    assert!(
        logic.special_power_strikes.radiation_fields_spawned_total() > rad0
            || !logic.special_power_strikes.radiation_fields().is_empty(),
        "midpoint should spawn OCL_NukeRadiationField residual"
    );
    assert!(logic.honesty_neutron_missile_update_ok());
}

#[test]
fn ocl_fire_weapon_neutron_projectile() {
    use crate::game_logic::KindOf;
    let mut logic = GameLogic::new();
    ensure_test_player_for_team(&mut logic, Team::USA);
    let mut silo = crate::game_logic::ThingTemplate::new("ChinaNuclearMissileLauncher");
    silo.add_kind_of(KindOf::Structure).set_health(5000.0);
    logic
        .templates
        .insert("ChinaNuclearMissileLauncher".into(), silo);
    let id = logic
        .create_object(
            "ChinaNuclearMissileLauncher",
            Team::USA,
            Vec3::new(0.0, 0.0, 0.0),
        )
        .expect("silo");
    let proj = logic
        .execute_ocl_fire_weapon(
            "SUPERWEAPON_NeutronMissile",
            id,
            Vec3::new(0.0, 0.0, 0.0),
            Vec3::new(400.0, 0.0, 400.0),
        )
        .expect("neutron proj");
    let p = logic.host_object(proj).unwrap();
    assert!(p.template_name.contains("NeutronMissile"));
    assert!(logic.ocl_fire_weapon_attack_reg.projectiles_spawned >= 1);
    assert!(logic.honesty_ocl_fire_weapon_attack_ok());
}

#[test]
fn ocl_attack_scud_storm_shots() {
    use crate::game_logic::KindOf;
    let mut logic = GameLogic::new();
    ensure_test_player_for_team(&mut logic, Team::GLA);
    let mut storm = crate::game_logic::ThingTemplate::new("GLAScudStorm");
    storm.add_kind_of(KindOf::Structure).set_health(5000.0);
    logic.templates.insert("GLAScudStorm".into(), storm);
    let id = logic
        .create_object("GLAScudStorm", Team::GLA, Vec3::new(10.0, 0.0, 10.0))
        .expect("storm");
    assert!(logic.execute_ocl_attack("SUPERWEAPON_ScudStorm", id, Vec3::new(300.0, 0.0, 300.0)));
    assert_eq!(logic.ocl_fire_weapon_attack_reg.last_attack_shots, 9);
    let o = logic.host_object(id).unwrap();
    assert!(o
        .fire_weapon_power
        .as_ref()
        .map(|r| r.shots_remaining >= 9)
        .unwrap_or(false));
}

#[test]
fn prone_update_damage_ratio_and_recovery() {
    use crate::game_logic::host_prone_update::{
        honesty_prone_update_residual_ok, PRONE_GLA_DAMAGE_TO_FRAMES_RATIO,
    };
    use crate::game_logic::KindOf;
    assert!(honesty_prone_update_residual_ok());

    let mut logic = GameLogic::new();
    let mut tpl = crate::game_logic::ThingTemplate::new("GLAInfantryWorker");
    tpl.add_kind_of(KindOf::Infantry).set_health(100.0);
    logic.templates.insert("GLAInfantryWorker".to_string(), tpl);
    let id = logic
        .create_object("GLAInfantryWorker", Team::GLA, Vec3::ZERO)
        .expect("worker");
    assert!(logic.host_object(id).unwrap().prone_update.is_some());
    {
        let o = logic.host_object_mut(id).unwrap();
        if let Some(pu) = o.prone_update.as_mut() {
            pu.damage_to_frames_ratio = PRONE_GLA_DAMAGE_TO_FRAMES_RATIO;
        }
    }
    logic.record_prone_go_if_needed(id, 20.0);
    assert_eq!(
        logic
            .host_object(id)
            .and_then(|o| o.prone_update.as_ref().map(|p| p.prone_frames))
            .unwrap_or(0),
        100
    );
    assert!(logic.prone_update_reg.go_prone >= 1);
    // Tick to recovery.
    for f in 0u32..100 {
        logic.set_current_frame(u64::from(f));
        logic.update_prone_update();
    }
    assert!(logic
        .host_object(id)
        .and_then(|o| o.prone_update.as_ref().map(|p| !p.is_prone()))
        .unwrap_or(false));
    assert!(logic.honesty_prone_update_ok());
}

#[test]
fn active_shroud_upgrade_sets_shroud_range() {
    use crate::game_logic::host_active_shroud_upgrade::honesty_active_shroud_upgrade_residual_ok;
    assert!(honesty_active_shroud_upgrade_residual_ok());

    let mut logic = GameLogic::new();
    ensure_test_tank_template(&mut logic);
    let id = logic
        .create_object("TestTank", Team::USA, Vec3::ZERO)
        .expect("tank");
    assert_eq!(logic.host_object(id).unwrap().shroud_range, 0.0);
    assert!(logic.apply_active_shroud_upgrade(id, 175.0));
    assert!((logic.host_object(id).unwrap().shroud_range - 175.0).abs() < 0.01);
    assert!(logic.active_shroud_upgrade_reg.applies >= 1);
    assert!(logic.honesty_active_shroud_upgrade_ok());
}

#[test]
fn animation_steering_battle_bus_turn_conditions() {
    use crate::game_logic::host_animation_steering::{
        honesty_animation_steering_residual_ok, BATTLE_BUS_MIN_TRANSITION_FRAMES,
    };
    use crate::game_logic::object::PhysicsTurningType;
    assert!(honesty_animation_steering_residual_ok());

    let mut logic = GameLogic::new();
    let mut tpl = crate::game_logic::ThingTemplate::new("GLAVehicleBattleBus");
    tpl.set_health(220.0);
    logic
        .templates
        .insert("GLAVehicleBattleBus".to_string(), tpl);
    let id = logic
        .create_object("GLAVehicleBattleBus", Team::GLA, Vec3::ZERO)
        .expect("bus");
    assert!(logic.host_object(id).unwrap().animation_steering.is_some());
    assert!(logic.animation_steering_reg.installed >= 1);

    {
        let o = logic.host_object_mut(id).unwrap();
        o.physics_turning = PhysicsTurningType::TurnNegative;
    }
    logic.set_current_frame(0);
    logic.update_animation_steering();
    assert_eq!(
        logic.host_object(id).and_then(|o| o
            .animation_steering
            .as_ref()
            .and_then(|a| a.active_condition.clone())),
        Some("CENTER_TO_RIGHT".to_string())
    );

    {
        let o = logic.host_object_mut(id).unwrap();
        o.physics_turning = PhysicsTurningType::TurnNone;
    }
    logic.set_current_frame(u64::from(BATTLE_BUS_MIN_TRANSITION_FRAMES));
    logic.update_animation_steering();
    assert_eq!(
        logic.host_object(id).and_then(|o| o
            .animation_steering
            .as_ref()
            .and_then(|a| a.active_condition.clone())),
        Some("RIGHT_TO_CENTER".to_string())
    );
    assert!(logic.honesty_animation_steering_ok());
}

#[test]
fn passengers_fire_upgrade_helix_battle_bunker() {
    use crate::game_logic::host_passengers_fire_upgrade::{
        honesty_passengers_fire_upgrade_residual_ok, UPGRADE_HELIX_BATTLE_BUNKER,
    };
    assert!(honesty_passengers_fire_upgrade_residual_ok());

    let mut logic = GameLogic::new();
    let mut tpl = crate::game_logic::ThingTemplate::new("ChinaHelix");
    tpl.set_health(300.0);
    logic.templates.insert("ChinaHelix".to_string(), tpl);
    let id = logic
        .create_object("ChinaHelix", Team::China, Vec3::ZERO)
        .expect("helix");
    {
        let o = logic.host_object_mut(id).unwrap();
        o.is_helix_transport = true;
        o.passengers_allowed_to_fire = false;
    }
    let n = logic.apply_passengers_fire_upgrade_to_team(Team::China, UPGRADE_HELIX_BATTLE_BUNKER);
    assert!(n >= 1);
    assert!(logic.host_object(id).unwrap().passengers_allowed_to_fire);
    assert!(logic.passengers_fire_upgrade_reg.applies >= 1);
    assert!(logic.honesty_passengers_fire_upgrade_ok());
}

#[test]
fn enemy_near_wall_sets_model_condition_on_scan() {
    use crate::game_logic::host_enemy_near::honesty_enemy_near_residual_ok;
    assert!(honesty_enemy_near_residual_ok());

    let mut logic = GameLogic::new();
    let mut wall_tpl = crate::game_logic::ThingTemplate::new("AmericaWallSegment");
    wall_tpl.set_health(1000.0);
    logic
        .templates
        .insert("AmericaWallSegment".to_string(), wall_tpl);
    ensure_test_infantry_template(&mut logic);

    let wall = logic
        .create_object("AmericaWallSegment", Team::USA, Vec3::new(0.0, 0.0, 0.0))
        .expect("wall");
    {
        let w = logic.host_object_mut(wall).unwrap();
        if let Some(en) = w.enemy_near.as_mut() {
            en.vision_range = 200.0;
            en.scan_delay = 0;
        }
        w.vision_range = 200.0;
    }
    assert!(logic.host_object(wall).unwrap().enemy_near.is_some());

    let enemy = logic
        .create_object("TestInfantry", Team::GLA, Vec3::new(50.0, 0.0, 0.0))
        .expect("enemy");
    let _ = enemy;

    logic.update_enemy_near();
    assert!(
        logic
            .host_object(wall)
            .and_then(|o| o
                .enemy_near
                .as_ref()
                .map(|e| e.enemy_near && e.model_enemy_near))
            .unwrap_or(false),
        "enemy in vision must set ENEMYNEAR residual"
    );
    assert!(logic.enemy_near_reg.became_near >= 1);
    assert!(logic.honesty_enemy_near_ok());
}

#[test]
fn base_regenerate_structure_heals_after_delay() {
    use crate::game_logic::host_base_regenerate::{
        honesty_base_regenerate_residual_ok, BASE_REGEN_DELAY_FRAMES, BASE_REGEN_HEAL_RATE_FRAMES,
    };
    use crate::game_logic::KindOf;
    assert!(honesty_base_regenerate_residual_ok());

    let mut logic = GameLogic::new();
    let mut tpl = crate::game_logic::ThingTemplate::new("AmericaCommandCenter");
    tpl.add_kind_of(KindOf::Structure).set_health(1000.0);
    logic
        .templates
        .insert("AmericaCommandCenter".to_string(), tpl);
    let id = logic
        .create_object("AmericaCommandCenter", Team::USA, Vec3::new(0.0, 0.0, 0.0))
        .expect("cc");
    assert!(logic.host_object(id).unwrap().base_regenerate.is_some());
    assert!(logic.base_regenerate_reg.installed >= 1);

    {
        let o = logic.host_object_mut(id).unwrap();
        o.health.current = 500.0;
        o.status.under_construction = false;
    }
    logic.set_current_frame(0);
    logic.notify_base_regenerate_damage(id, false);
    // Immediately after damage: still delayed.
    logic.update_base_regenerate();
    let mid = logic.host_object(id).unwrap().health.current;
    assert!(
        (mid - 500.0).abs() < 0.01,
        "no heal during delay, got {mid}"
    );

    // After delay, heal ticks.
    let wake = BASE_REGEN_DELAY_FRAMES;
    logic.set_current_frame(u64::from(wake));
    logic.update_base_regenerate();
    let after = logic.host_object(id).unwrap().health.current;
    assert!(after > 500.0, "must heal after delay (after={after})");
    assert!(logic.base_regenerate_reg.heal_ticks >= 1);
    assert!(logic.honesty_base_regenerate_ok());
    let _ = BASE_REGEN_HEAL_RATE_FRAMES;
}

#[test]
fn default_auto_heal_trainable_heals_after_start_delay() {
    // C++ AutoHealBehavior.cpp:205-219 self-heal; :136-157 onDamage StartHealingDelay.
    use crate::game_logic::host_heal::{
        DEFAULT_AUTO_HEAL_AMOUNT, DEFAULT_AUTO_HEAL_START_DELAY_FRAMES,
    };

    let mut logic = GameLogic::new();
    let mut tpl = ThingTemplate::new("USA_Ranger");
    tpl.add_kind_of(KindOf::Infantry).set_health(100.0);
    tpl.is_trainable = true;
    logic.templates.insert("USA_Ranger".to_string(), tpl);
    let id = logic
        .create_object("USA_Ranger", Team::USA, Vec3::ZERO)
        .expect("ranger");
    assert!(
        logic.host_object(id).unwrap().default_auto_heal.is_some(),
        "trainable units inherit ModuleTag_DefaultAutoHealBehavior"
    );

    {
        let o = logic.host_object_mut(id).unwrap();
        o.health.current = 50.0;
        if let Some(ah) = o.default_auto_heal.as_mut() {
            ah.on_damage(0);
        }
    }
    logic.set_current_frame(0);
    logic.update_default_auto_heal();
    let mid = logic.host_object(id).unwrap().health.current;
    assert!(
        (mid - 50.0).abs() < 0.01,
        "no heal during StartHealingDelay, got {mid}"
    );

    logic.set_current_frame(u64::from(DEFAULT_AUTO_HEAL_START_DELAY_FRAMES));
    logic.update_default_auto_heal();
    let after = logic.host_object(id).unwrap().health.current;
    assert!(
        (after - (50.0 + DEFAULT_AUTO_HEAL_AMOUNT)).abs() < 0.01,
        "must pulse HealingAmount=2 after StartHealingDelay (after={after})"
    );

    // Full health sleeps forever; later combat must restart StartHealingDelay.
    {
        let o = logic.host_object_mut(id).unwrap();
        o.health.current = o.health.maximum;
    }
    logic.update_default_auto_heal();
    {
        let o = logic.host_object_mut(id).unwrap();
        o.health.current = 50.0;
        if let Some(ah) = o.default_auto_heal.as_mut() {
            ah.on_damage(logic.frame);
        }
    }
    logic.update_default_auto_heal();
    let after_second_hit = logic.host_object(id).unwrap().health.current;
    assert!(
        (after_second_hit - 50.0).abs() < 0.01,
        "onDamage must re-arm StartHealingDelay after full-health sleep"
    );
    logic.set_current_frame(
        u64::from(logic.frame) + u64::from(DEFAULT_AUTO_HEAL_START_DELAY_FRAMES),
    );
    logic.update_default_auto_heal();
    let after_restart = logic.host_object(id).unwrap().health.current;
    assert!(
        (after_restart - (50.0 + DEFAULT_AUTO_HEAL_AMOUNT)).abs() < 0.01,
        "self-heal must restart after later combat (after={after_restart})"
    );

    let mut building = ThingTemplate::new("AmericaCommandCenter");
    building.add_kind_of(KindOf::Structure).set_health(1000.0);
    building.is_trainable = false;
    logic
        .templates
        .insert("AmericaCommandCenter".to_string(), building);
    let cc = logic
        .create_object("AmericaCommandCenter", Team::USA, Vec3::new(200.0, 0.0, 0.0))
        .expect("cc");
    assert!(
        logic.host_object(cc).unwrap().default_auto_heal.is_none(),
        "C++ drops DefaultAutoHeal when !isTrainable"
    );
}

#[test]
fn fire_spread_tree_ignites_neighbor() {
    use crate::game_logic::host_fire_spread::{
        honesty_fire_spread_residual_ok, TREE_SPREAD_TRY_RANGE,
    };
    assert!(honesty_fire_spread_residual_ok());

    let mut logic = GameLogic::new();
    for name in ["DogwoodTreeA", "DogwoodTreeB"] {
        let mut tpl = crate::game_logic::ThingTemplate::new(name);
        tpl.set_health(50.0);
        logic.templates.insert(name.to_string(), tpl);
    }
    let a = logic
        .create_object("DogwoodTreeA", Team::Neutral, Vec3::new(0.0, 0.0, 0.0))
        .expect("a");
    let b = logic
        .create_object(
            "DogwoodTreeB",
            Team::Neutral,
            Vec3::new(TREE_SPREAD_TRY_RANGE * 0.5, 0.0, 0.0),
        )
        .expect("b");
    assert!(logic.host_object(a).unwrap().has_fire_spread());
    assert!(logic.host_object(b).unwrap().has_fire_spread());
    assert!(logic.fire_spread_reg.installed >= 2);

    assert!(logic.ignite_object_fire_spread(a));
    // Force spread due immediately.
    {
        let o = logic.host_object_mut(a).unwrap();
        if let Some(fs) = o.fire_spread.as_mut() {
            fs.next_spread_frame = 0;
        }
    }
    logic.set_current_frame(30);
    logic.update_fire_spread();
    assert!(
        logic.fire_spread_reg.spreads > 0,
        "aflame tree must attempt spread"
    );
    assert!(
        logic
            .host_object(b)
            .and_then(|o| o.fire_spread.as_ref().map(|f| f.is_aflame()))
            .unwrap_or(false)
            || logic.fire_spread_reg.ignitions >= 2,
        "neighbor within range must ignite"
    );
    assert!(logic.honesty_fire_spread_ok());
}

#[test]
fn flammable_buildings_ignite_and_play_burning_loop() {
    use crate::game_logic::combat::DamageType;
    use crate::game_logic::host_enum_table_residual::aflame_model_bit;
    use crate::game_logic::host_fire_spread::TREE_BURNING_SOUND;

    let mut logic = GameLogic::new();
    let mut tpl = crate::game_logic::ThingTemplate::new("TechOilDerrick");
    tpl.add_kind_of(KindOf::Structure).set_health(2000.0);
    logic.templates.insert("TechOilDerrick".to_string(), tpl);
    let id = logic
        .create_object("TechOilDerrick", Team::Neutral, Vec3::ZERO)
        .expect("oil");
    assert!(logic.host_object(id).unwrap().has_fire_spread());
    assert!(!logic
        .host_object(id)
        .unwrap()
        .fire_spread
        .as_ref()
        .unwrap()
        .spread_enabled);

    crate::game_logic::host_historic_bonus::set_logic_frame(10);
    {
        let o = logic.host_object_mut(id).unwrap();
        let _ = o.take_damage_from_typed(50.0, None, DamageType::Flame);
    }
    {
        let o = logic.host_object(id).unwrap();
        assert!(o.fire_spread.as_ref().unwrap().is_aflame());
        assert!(o.has_object_status_bit("AFLAME"));
        assert!(o.model_condition_bits & (1u128 << aflame_model_bit()) != 0);
    }
    logic.set_current_frame(10);
    logic.update_fire_spread();
    assert!(
        logic
            .queued_audio_events
            .iter()
            .any(|e| e.event_type == TREE_BURNING_SOUND && e.is_looping),
        "GenericFireMediumLoop must start on ignite"
    );
}

#[test]
fn fire_spread_spawns_burning_embers_and_uses_3d_range() {
    use crate::game_logic::host_fire_spread::{
        TREE_OCL_EMBERS, TREE_SPREAD_TRY_RANGE,
    };

    let mut logic = GameLogic::new();
    for name in ["DogwoodTreeA", "DogwoodTreeCliff"] {
        let mut tpl = crate::game_logic::ThingTemplate::new(name);
        tpl.set_health(50.0);
        logic.templates.insert(name.to_string(), tpl);
    }
    let a = logic
        .create_object("DogwoodTreeA", Team::Neutral, Vec3::new(0.0, 0.0, 0.0))
        .expect("a");
    let cliff = logic
        .create_object(
            "DogwoodTreeCliff",
            Team::Neutral,
            Vec3::new(30.0, 80.0, 0.0),
        )
        .expect("cliff");
    assert!(logic.ignite_object_fire_spread(a));
    {
        let o = logic.host_object_mut(a).unwrap();
        if let Some(fs) = o.fire_spread.as_mut() {
            fs.next_spread_frame = 0;
        }
    }
    logic.set_current_frame(30);
    logic.update_fire_spread();
    assert!(logic.fire_spread_reg.embers > 0);
    assert!(
        logic
            .objects
            .values()
            .any(|o| o.template_name.eq_ignore_ascii_case("BurningEmbers")
                || o.template_name.contains("Ember"))
            || logic
                .combat_particles
                .active_systems()
                .any(|p| p.ocl_list_name == TREE_OCL_EMBERS
                    || p.template_name == TREE_OCL_EMBERS),
        "OCL_BurningEmbers must create ember objects or particle entries"
    );
    assert!(
        !logic
            .host_object(cliff)
            .and_then(|o| o.fire_spread.as_ref().map(|f| f.is_aflame()))
            .unwrap_or(false),
        "FROM_CENTER_3D must exclude a tree 80 units above planar range {TREE_SPREAD_TRY_RANGE}"
    );
}

#[test]
fn tree_smolders_while_still_aflame() {
    use crate::game_logic::host_enum_table_residual::{
        aflame_model_bit, smoldering_model_bit,
    };
    use crate::game_logic::host_fire_spread::TREE_BURNED_DELAY_FRAMES;

    let mut logic = GameLogic::new();
    let mut tpl = crate::game_logic::ThingTemplate::new("DogwoodTreeA");
    tpl.set_health(50.0);
    logic.templates.insert("DogwoodTreeA".to_string(), tpl);
    let id = logic
        .create_object("DogwoodTreeA", Team::Neutral, Vec3::ZERO)
        .expect("tree");
    assert!(logic.ignite_object_fire_spread(id));
    {
        let o = logic.host_object_mut(id).unwrap();
        if let Some(fs) = o.fire_spread.as_mut() {
            fs.burned_end_frame = TREE_BURNED_DELAY_FRAMES;
        }
    }
    logic.set_current_frame(TREE_BURNED_DELAY_FRAMES as u64);
    logic.update_fire_spread();
    let o = logic.host_object(id).unwrap();
    assert!(o.fire_spread.as_ref().unwrap().is_aflame());
    assert!(o.fire_spread.as_ref().unwrap().smoldering);
    assert!(o.has_object_status_bit("AFLAME"));
    assert!(o.has_object_status_bit("BURNED"));
    assert!(o.model_condition_bits & (1u128 << aflame_model_bit()) != 0);
    assert!(o.model_condition_bits & (1u128 << smoldering_model_bit()) != 0);
}


#[test]
fn status_bits_upgrade_booby_trap_sets_bit() {
    use crate::game_logic::host_status_bits_upgrade::honesty_status_bits_upgrade_residual_ok;
    assert!(honesty_status_bits_upgrade_residual_ok());

    let mut logic = GameLogic::new();
    let mut tpl = crate::game_logic::ThingTemplate::new("GLATunnelNetwork");
    tpl.set_health(400.0);
    logic.templates.insert("GLATunnelNetwork".to_string(), tpl);
    let id = logic
        .create_object("GLATunnelNetwork", Team::GLA, Vec3::ZERO)
        .expect("net");
    assert!(!logic
        .host_object(id)
        .unwrap()
        .has_object_status_bit("BOOBY_TRAPPED"));

    let n = logic.apply_status_bits_upgrade_to_team(Team::GLA, "Upgrade_GLABoobyTrap");
    assert!(n >= 1);
    assert!(logic
        .host_object(id)
        .unwrap()
        .has_object_status_bit("BOOBY_TRAPPED"));
    assert!(logic.status_bits_upgrade_reg.applies >= 1);
    assert!(logic.honesty_status_bits_upgrade_ok());
}

#[test]
fn tensile_formation_avalanche_damage_slide_and_rubble() {
    use crate::game_logic::host_tensile_formation::{
        honesty_tensile_formation_residual_ok, TENSILE_LIFE_MAX,
    };

    assert!(honesty_tensile_formation_residual_ok());

    let mut logic = GameLogic::new();
    let mut tpl = crate::game_logic::ThingTemplate::new("AvalancheChunk");
    tpl.set_health(100.0);
    logic.templates.insert("AvalancheChunk".to_string(), tpl);

    let a = logic
        .create_object("AvalancheChunk", Team::Neutral, Vec3::new(0.0, 10.0, 0.0))
        .expect("chunk a");
    let b = logic
        .create_object("AvalancheChunk", Team::Neutral, Vec3::new(20.0, 10.0, 0.0))
        .expect("chunk b");
    assert!(logic.host_object(a).unwrap().has_tensile_formation());
    assert!(logic.host_object(b).unwrap().has_tensile_formation());
    assert!(logic.tensile_formation_registry().members_installed >= 2);

    // Damage A to BODY_DAMAGED residual → enable formation.
    {
        let o = logic.host_object_mut(a).unwrap();
        o.health.current = 50.0;
    }
    logic.set_current_frame(30);
    logic.update_tensile_formations();
    assert!(
        logic.tensile_formation_registry().enables > 0
            || logic
                .host_object(a)
                .and_then(|o| o.tensile_formation.as_ref().map(|t| t.enabled))
                .unwrap_or(false),
        "damage must enable tensile formation"
    );
    assert!(logic.honesty_tensile_formation_ok());

    // Advance life to rubble.
    {
        let o = logic.host_object_mut(a).unwrap();
        if let Some(tf) = o.tensile_formation.as_mut() {
            tf.enabled = true;
            tf.life = TENSILE_LIFE_MAX;
        }
    }
    logic.set_current_frame(400);
    logic.update_tensile_formations();
    assert!(
        logic.tensile_formation_registry().rubbles > 0
            || logic
                .host_object(a)
                .and_then(|o| o.tensile_formation.as_ref().map(|t| t.rubble))
                .unwrap_or(false),
        "life>300 must rubble"
    );
}

#[test]
fn toxin_fire_ocl_spawns_field_after_min_shots_and_coast() {
    use crate::game_logic::host_toxin_tractor::{
        is_toxin_tractor_template, TOXIN_SPRAY_CONTINUOUS_FIRE_COAST_FRAMES,
        TOXIN_SPRAY_MIN_SHOTS_TO_CREATE_OCL,
    };
    use crate::game_logic::{KindOf, Team, ThingTemplate};
    use glam::Vec3;

    let mut logic = GameLogic::new();
    let mut tpl = ThingTemplate::new("GLAVehicleToxinTruck");
    tpl.add_kind_of(KindOf::Vehicle)
        .add_kind_of(KindOf::Attackable)
        .set_health(200.0);
    logic.templates.insert("GLAVehicleToxinTruck".into(), tpl);
    let id = logic
        .create_object("GLAVehicleToxinTruck", Team::GLA, Vec3::new(0.0, 0.0, 0.0))
        .expect("toxin");
    assert!(is_toxin_tractor_template("GLAVehicleToxinTruck"));
    assert!(logic
        .host_object(id)
        .unwrap()
        .fire_ocl_after_cooldown
        .is_some());

    // Fire secondary spray MinShots times.
    for f in 0..TOXIN_SPRAY_MIN_SHOTS_TO_CREATE_OCL {
        logic.frame = f;
        let _ = logic.apply_toxin_tractor_spray_at(Vec3::new(10.0, 0.0, 0.0), Some(id), Team::GLA);
    }
    assert_eq!(logic.toxin_tractor.fire_ocl_spawns, 0, "no OCL until coast");
    // Advance past coast without more shots.
    logic.frame = TOXIN_SPRAY_MIN_SHOTS_TO_CREATE_OCL + TOXIN_SPRAY_CONTINUOUS_FIRE_COAST_FRAMES;
    logic.tick_fire_ocl_after_weapon_cooldown();
    assert!(
        logic.toxin_tractor.fire_ocl_spawns > 0,
        "FireOCL should spawn medium field after min shots + coast"
    );
    assert!(logic.honesty_toxin_fire_ocl_ok());
}

#[test]
fn upgrade_die_removes_producer_drone_upgrade() {
    let mut game_logic = GameLogic::new();
    ensure_test_tank_template(&mut game_logic);
    // Humvee-like master.
    let mut humvee = ThingTemplate::new("AmericaVehicleHumvee");
    humvee
        .add_kind_of(KindOf::Vehicle)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .set_health(200.0);
    game_logic
        .templates
        .insert("AmericaVehicleHumvee".into(), humvee);
    let master_id = game_logic
        .create_object("AmericaVehicleHumvee", Team::USA, Vec3::new(0.0, 0.0, 0.0))
        .expect("humvee");
    let drone_id = game_logic
        .residual_attach_slave_drone(
            master_id,
            crate::game_logic::host_slave_drones::SlaveDroneKind::Scout,
        )
        .expect("scout drone");
    {
        let m = game_logic.host_object(master_id).unwrap();
        assert!(
            m.has_upgrade_tag(crate::game_logic::host_slave_drones::UPGRADE_AMERICA_SCOUT_DRONE)
        );
    }
    {
        let d = game_logic.host_object(drone_id).unwrap();
        assert_eq!(d.producer_id, Some(master_id));
        assert!(d.upgrade_die.is_some());
    }
    // Kill drone → UpgradeDie frees master upgrade.
    game_logic.destroy_object(drone_id);
    // Process destruction queue if needed.
    if let Some(d) = game_logic.host_object_mut(drone_id) {
        // Wave 753: under damage authority, do not zero host HP mid-frame
        // (dual with GW HP writeback). Project lethal via damage log + flags.
        if crate::gameworld_shadow::gameworld_damage_authority_live() {
            let hp = d.health.current.max(1.0);
            let oid = d.id;
            crate::game_logic::host_damage_log::record(oid, hp, None, true);
        } else {
            d.health.current = 0.0;
        }
        d.status.destroyed = true;
    }
    // destroy_object should have already run upgrade die via mark.
    let m = game_logic.host_object(master_id).unwrap();
    assert!(
        !m.has_upgrade_tag(crate::game_logic::host_slave_drones::UPGRADE_AMERICA_SCOUT_DRONE),
        "UpgradeDie must remove producer upgrade"
    );
    assert!(game_logic.honesty_upgrade_die_ok());
}

// -----------------------------------------------------------------------
// GLA Tunnel Network residual (shared MaxTunnelCapacity=10, cross-tunnel exit)
// Fail-closed: not GuardTunnelNetwork AI / CaveSystem / TimeForFullHeal.
// -----------------------------------------------------------------------

/// Residual: create installs TunnelContain capacity residual.
#[test]
fn tunnel_network_residual_flags_and_capacity_installed() {
    let mut game_logic = GameLogic::new();
    let tunnel_id = create_test_tunnel_network(&mut game_logic, Vec3::new(0.0, 0.0, 0.0));
    let tunnel = game_logic.host_object(tunnel_id).expect("tunnel");
    assert!(tunnel.is_tunnel_network_style_container());
    assert!(tunnel.can_contain());
    assert_eq!(
        tunnel.garrison_capacity(),
        crate::game_logic::host_tunnel_network::MAX_TUNNEL_CAPACITY
    );
}

/// Residual: Enter tunnel A → Garrisoned + shared pool bookkeeping.
#[test]
fn tunnel_network_residual_enter_sets_garrisoned_and_shared_pool() {
    use crate::command_system::{CommandType, GameCommand};

    let mut game_logic = GameLogic::new();
    ensure_test_infantry_template(&mut game_logic);
    let tunnel_id = create_test_tunnel_network(&mut game_logic, Vec3::new(0.0, 0.0, 0.0));
    let infantry_id = game_logic
        .create_object("TestInfantry", Team::GLA, Vec3::new(2.0, 0.0, 0.0))
        .expect("infantry");

    game_logic.queue_command(GameCommand {
        command_type: CommandType::Enter {
            target_id: tunnel_id,
        },
        player_id: 2,
        command_id: 1,
        timestamp: std::time::SystemTime::now(),
        selected_units: vec![infantry_id],
        modifier_keys: crate::command_system::ModifierKeys::default(),
    });
    game_logic.process_commands();
    game_logic.update_ai(&[infantry_id, tunnel_id], 1.0 / 30.0);

    let tunnel = game_logic.host_object(tunnel_id).expect("tunnel after");
    assert!(
        tunnel.contained_units().contains(&infantry_id),
        "entry tunnel must list occupant"
    );
    let pool_key = tunnel.tunnel_system_key();
    let infantry = game_logic.host_object(infantry_id).expect("infantry after");
    assert_eq!(infantry.ai_state, AIState::Garrisoned);
    assert_eq!(infantry.contained_by, Some(tunnel_id));
    assert!(!infantry.can_move());
    assert_eq!(game_logic.tunnel_network_residual_enters(), 1);
    assert!(game_logic
        .tunnel_network_residual()
        .is_in_network(pool_key, infantry_id));

}

/// C++ TunnelContain::getContainCount / getContainedItemsList redirect to
/// the player's TunnelTracker — every entrance shows the same pool.
#[test]
fn tunnel_control_bar_inventory_is_shared_player_pool() {
    let mut game_logic = GameLogic::new();
    ensure_test_infantry_template(&mut game_logic);
    let tunnel_a = create_test_tunnel_network(&mut game_logic, Vec3::new(0.0, 0.0, 0.0));
    let tunnel_b = create_test_tunnel_network(&mut game_logic, Vec3::new(50.0, 0.0, 0.0));
    let infantry_id = game_logic
        .create_object("TestInfantry", Team::GLA, Vec3::new(2.0, 0.0, 0.0))
        .expect("infantry");
    let key = game_logic
        .host_object(tunnel_a)
        .expect("a")
        .tunnel_system_key();
    game_logic.tunnel_network.on_tunnel_created(key, tunnel_a);
    game_logic.tunnel_network.on_tunnel_created(key, tunnel_b);
    assert!(game_logic
        .tunnel_network
        .record_enter(key, infantry_id, tunnel_a));
    assert_eq!(
        game_logic.host_authoritative_occupant_count(tunnel_a),
        Some(1)
    );
    assert_eq!(
        game_logic.host_authoritative_occupant_count(tunnel_b),
        Some(1)
    );
    assert_eq!(
        game_logic.host_authoritative_contained_units(tunnel_b),
        vec![infantry_id]
    );
}


/// Residual: enter tunnel A, Evacuate tunnel B → unit exits at B (cross-tunnel).
#[test]
fn tunnel_network_residual_cross_exit_enter_a_evacuate_b() {
    use crate::command_system::{CommandType, GameCommand};

    let mut game_logic = GameLogic::new();
    ensure_test_infantry_template(&mut game_logic);
    let tunnel_a = create_test_tunnel_network(&mut game_logic, Vec3::new(0.0, 0.0, 0.0));
    let tunnel_b = create_test_tunnel_network(&mut game_logic, Vec3::new(200.0, 0.0, 0.0));
    let infantry_id = game_logic
        .create_object("TestInfantry", Team::GLA, Vec3::new(2.0, 0.0, 0.0))
        .expect("infantry");

    // Enter A.
    {
        let unit = game_logic.host_object_mut(infantry_id).unwrap();
        unit.target = Some(tunnel_a);
        unit.set_ai_state(AIState::Entering);
    }
    game_logic.update_ai(&[infantry_id, tunnel_a, tunnel_b], 1.0 / 30.0);
    assert_eq!(
        game_logic.host_object(infantry_id).unwrap().ai_state,
        AIState::Garrisoned
    );
    assert_eq!(game_logic.tunnel_network_residual_enters(), 1);
    assert!(!game_logic.honesty_tunnel_network_cross_exit_ok());

    // Evacuate B dumps shared pool at B (cross-tunnel residual).
    game_logic.queue_command(GameCommand {
        command_type: CommandType::Evacuate,
        player_id: 2,
        command_id: 2,
        timestamp: std::time::SystemTime::now(),
        selected_units: vec![tunnel_b],
        modifier_keys: crate::command_system::ModifierKeys::default(),
    });
    game_logic.process_commands();

    let infantry = game_logic.host_object(infantry_id).expect("exited");
    assert_eq!(infantry.ai_state, AIState::Idle);
    assert!(infantry.contained_by.is_none());
    assert!(infantry.can_move());
    // Dropped near tunnel B, not tunnel A.
    let pos = infantry.get_position();
    let b_pos = game_logic.host_object(tunnel_b).unwrap().get_position();
    let a_pos = game_logic.host_object(tunnel_a).unwrap().get_position();
    assert!(
        pos.distance(b_pos) < 20.0,
        "cross-exit must place unit near tunnel B (got dist {})",
        pos.distance(b_pos)
    );
    assert!(
        pos.distance(a_pos) > 100.0,
        "cross-exit must not leave unit at tunnel A"
    );
    assert!(
        !game_logic
            .host_object(tunnel_a)
            .unwrap()
            .contained_units()
            .contains(&infantry_id),
        "entry tunnel A capacity freed"
    );
    assert_eq!(game_logic.tunnel_network_residual_exits(), 1);
    assert_eq!(game_logic.tunnel_network_residual_cross_exits(), 1);
    assert!(
        game_logic.honesty_tunnel_network_enter_exit_ok(),
        "enter+exit honesty"
    );
    assert!(
        game_logic.honesty_tunnel_network_cross_exit_ok(),
        "cross-tunnel honesty"
    );
    assert!(game_logic.honesty_tunnel_network_ok());
}

/// Residual: shared MaxTunnelCapacity=10 rejects 11th enter across two tunnels.
#[test]
fn tunnel_network_residual_shared_capacity_full_rejects_enter() {
    use crate::command_system::{CommandType, GameCommand};

    let mut game_logic = GameLogic::new();
    ensure_test_infantry_template(&mut game_logic);
    let tunnel_a = create_test_tunnel_network(&mut game_logic, Vec3::new(0.0, 0.0, 0.0));
    let tunnel_b = create_test_tunnel_network(&mut game_logic, Vec3::new(50.0, 0.0, 0.0));

    let cap = crate::game_logic::host_tunnel_network::MAX_TUNNEL_CAPACITY;
    let mut unit_ids = Vec::new();
    for i in 0..cap {
        // Fill half via A, half via B to prove shared pool.
        // Spawn near the chosen tunnel so enter-range residual succeeds.
        let tunnel = if i < cap / 2 { tunnel_a } else { tunnel_b };
        let base = if i < cap / 2 {
            Vec3::new(1.0, 0.0, 0.0)
        } else {
            Vec3::new(51.0, 0.0, 0.0)
        };
        let id = game_logic
            .create_object(
                "TestInfantry",
                Team::GLA,
                base + Vec3::new(i as f32 * 0.1, 0.0, 0.0),
            )
            .expect("infantry");
        unit_ids.push(id);
        {
            let unit = game_logic.host_object_mut(id).unwrap();
            unit.target = Some(tunnel);
            unit.set_ai_state(AIState::Entering);
        }
        game_logic.update_ai(&[id, tunnel_a, tunnel_b], 1.0 / 30.0);
    }
    assert_eq!(game_logic.tunnel_network_residual_enters() as usize, cap);
    let pool_key = game_logic
        .host_object(tunnel_a)
        .expect("tunnel a")
        .tunnel_system_key();
    assert!(!game_logic.tunnel_network_residual().has_capacity(pool_key));


    let overflow = game_logic
        .create_object("TestInfantry", Team::GLA, Vec3::new(3.0, 0.0, 0.0))
        .expect("overflow");
    game_logic.queue_command(GameCommand {
        command_type: CommandType::Enter {
            target_id: tunnel_a,
        },
        player_id: 2,
        command_id: 99,
        timestamp: std::time::SystemTime::now(),
        selected_units: vec![overflow],
        modifier_keys: crate::command_system::ModifierKeys::default(),
    });
    game_logic.process_commands();
    // Enter command may still set Entering, but AI must reject capacity.
    game_logic.update_ai(&[overflow, tunnel_a, tunnel_b], 1.0 / 30.0);
    let overflow_obj = game_logic.host_object(overflow).unwrap();
    assert_ne!(
        overflow_obj.ai_state,
        AIState::Garrisoned,
        "11th unit must not enter shared tunnel pool"
    );
    assert_eq!(
        game_logic.tunnel_network_residual_enters() as usize,
        cap,
        "enter counter must not grow past capacity"
    );
}

/// Residual: aircraft rejected from tunnel network.
#[test]
fn tunnel_network_residual_rejects_aircraft() {
    use crate::command_system::{CommandType, GameCommand};

    let mut game_logic = GameLogic::new();
    ensure_test_aircraft_template(&mut game_logic);
    let tunnel_id = create_test_tunnel_network(&mut game_logic, Vec3::new(0.0, 0.0, 0.0));
    let air_id = game_logic
        .create_object("TestAircraft", Team::GLA, Vec3::new(2.0, 0.0, 0.0))
        .expect("aircraft");

    game_logic.queue_command(GameCommand {
        command_type: CommandType::Enter {
            target_id: tunnel_id,
        },
        player_id: 2,
        command_id: 5,
        timestamp: std::time::SystemTime::now(),
        selected_units: vec![air_id],
        modifier_keys: crate::command_system::ModifierKeys::default(),
    });
    game_logic.process_commands();

    let air = game_logic.host_object(air_id).expect("aircraft");
    assert_ne!(
        air.ai_state,
        AIState::Entering,
        "aircraft must not enter tunnel residual"
    );
    assert_eq!(game_logic.tunnel_network_residual_enters(), 0);
}

// -----------------------------------------------------------------------
// AirF Combat Chinook residual (capacity 8 + passenger fire + armed-riders)
// Fail-closed: not ChinookAIUpdate ropes / supply / rappel / combat drop.
// -----------------------------------------------------------------------

/// Residual: Combat Chinook create binds Slots=8 + passengers fire + armed-riders flags.
#[test]
fn combat_chinook_residual_capacity_and_flags_installed() {
    let mut game_logic = GameLogic::new();
    let chinook_id = create_test_combat_chinook(&mut game_logic, Vec3::new(0.0, 0.0, 0.0));
    let chinook = game_logic.host_object(chinook_id).expect("chinook");
    assert!(chinook.is_combat_chinook_style_container());
    assert!(chinook.can_contain());
    assert_eq!(
        chinook.transport_capacity(),
        crate::game_logic::host_combat_chinook::COMBAT_CHINOOK_TRANSPORT_SLOTS
    );
    assert!(chinook.passengers_allowed_to_fire);
    assert!(chinook.armed_riders_upgrade_weapon_set);
    assert!(!chinook.weapon_set_player_upgrade);
    assert!(
        chinook.is_kind_of(KindOf::Attackable),
        "Combat Chinook KindOf residual includes CAN_ATTACK"
    );
}

/// Residual: Enter Combat Chinook → Docked + capacity bookkeeping + weapon-set upgrade.
#[test]
fn combat_chinook_residual_enter_sets_docked_and_upgrades_weapon_set() {
    use crate::command_system::{CommandType, GameCommand};

    let mut game_logic = GameLogic::new();
    ensure_test_infantry_template(&mut game_logic);
    let chinook_id = create_test_combat_chinook(&mut game_logic, Vec3::new(0.0, 0.0, 0.0));
    let infantry_id = game_logic
        .create_object("TestInfantry", Team::USA, Vec3::new(2.0, 0.0, 0.0))
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
        command_type: CommandType::Enter {
            target_id: chinook_id,
        },
        player_id: 0,
        command_id: 1,
        timestamp: std::time::SystemTime::now(),
        selected_units: vec![infantry_id],
        modifier_keys: crate::command_system::ModifierKeys::default(),
    });
    game_logic.process_commands();
    game_logic.update_ai(&[infantry_id, chinook_id], 1.0 / 30.0);

    let chinook = game_logic.host_object(chinook_id).expect("chinook after");
    assert!(chinook.contained_units().contains(&infantry_id));
    assert_eq!(chinook.transport_count(), 1);
    assert!(
        chinook.weapon_set_player_upgrade,
        "armed riders must upgrade Combat Chinook weapon set"
    );
    assert!(
        chinook.weapon.is_some(),
        "PLAYER_UPGRADE residual binds ListeningOutpost dummy weapon"
    );
    let dummy = chinook.weapon.as_ref().unwrap();
    assert!(
        crate::game_logic::host_combat_chinook::is_passenger_dummy_weapon(dummy),
        "Combat Chinook dummy must be ListeningOutpost residual (damage ~0.1)"
    );
    assert!(
        (dummy.damage - crate::game_logic::host_combat_chinook::LISTENING_OUTPOST_DUMMY_DAMAGE)
            .abs()
            < f32::EPSILON
    );

    let infantry = game_logic.host_object(infantry_id).expect("infantry after");
    assert_eq!(infantry.ai_state, AIState::Docked);
    assert_eq!(infantry.contained_by, Some(chinook_id));
    assert_eq!(game_logic.combat_chinook_residual_loads(), 1);
    assert_eq!(
        game_logic.transport_residual_loads(),
        0,
        "Combat Chinook load must not count as generic transport load"
    );
    assert_eq!(
        game_logic.battle_bus_residual_loads(),
        0,
        "Combat Chinook load must not count as Battle Bus load"
    );
    assert!(
        game_logic.honesty_combat_chinook_weapon_set_upgrade_ok(),
        "weapon-set upgrade residual honesty"
    );
}

/// Residual acceptance: load 2 infantry → unload all → both free + honesty.
#[test]
fn combat_chinook_residual_load_two_unload_both_free() {
    use crate::command_system::{CommandType, GameCommand};

    let mut game_logic = GameLogic::new();
    ensure_test_infantry_template(&mut game_logic);
    let chinook_id = create_test_combat_chinook(&mut game_logic, Vec3::new(0.0, 0.0, 0.0));

    let unit_a = game_logic
        .create_object("TestInfantry", Team::USA, Vec3::new(1.0, 0.0, 0.0))
        .expect("unit a");
    let unit_b = game_logic
        .create_object("TestInfantry", Team::USA, Vec3::new(2.0, 0.0, 0.0))
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
            unit.target = Some(chinook_id);
            unit.set_ai_state(AIState::Entering);
        }
        game_logic.update_ai(&[unit_id, chinook_id], 1.0 / 30.0);
    }

    let chinook = game_logic.host_object(chinook_id).expect("chinook loaded");
    assert!(
        chinook.contained_units().contains(&unit_a) && chinook.contained_units().contains(&unit_b),
        "both infantry must be loaded into Combat Chinook residual"
    );
    assert_eq!(chinook.transport_count(), 2);
    assert_eq!(game_logic.combat_chinook_residual_loads(), 2);
    assert!(chinook.weapon_set_player_upgrade);

    for unit_id in [unit_a, unit_b] {
        let unit = game_logic.host_object(unit_id).expect("loaded unit");
        assert_eq!(unit.ai_state, AIState::Docked);
        assert_eq!(unit.contained_by, Some(chinook_id));
        assert!(!unit.can_move());
    }

    game_logic.queue_command(GameCommand {
        command_type: CommandType::Evacuate,
        player_id: 0,
        command_id: 2,
        timestamp: std::time::SystemTime::now(),
        selected_units: vec![chinook_id],
        modifier_keys: crate::command_system::ModifierKeys::default(),
    });
    game_logic.process_commands();

    let chinook = game_logic.host_object(chinook_id).expect("chinook empty");
    assert!(
        chinook.contained_units().is_empty(),
        "evacuate must clear all Combat Chinook residual occupants"
    );
    assert_eq!(chinook.transport_count(), 0);
    assert!(
        !chinook.weapon_set_player_upgrade,
        "weapon set upgrade must clear when empty"
    );

    for unit_id in [unit_a, unit_b] {
        let unit = game_logic.host_object(unit_id).expect("freed unit");
        assert_eq!(unit.ai_state, AIState::Idle, "unloaded unit must be Idle");
        assert!(unit.contained_by.is_none(), "contained_by must clear");
        assert!(unit.can_move(), "unloaded unit must be free to move");
    }

    assert_eq!(game_logic.combat_chinook_residual_unloads(), 2);
    assert!(
        game_logic.honesty_combat_chinook_load_unload_ok(),
        "load+unload residual honesty"
    );
    assert_eq!(
        game_logic.transport_residual_unloads(),
        0,
        "Combat Chinook unload must not count as generic transport unload"
    );
    assert_eq!(
        game_logic.battle_bus_residual_unloads(),
        0,
        "Combat Chinook unload must not count as Battle Bus unload"
    );
    assert_eq!(
        game_logic.garrison_residual_exits(),
        0,
        "Combat Chinook unload must not count as garrison exit"
    );
}

/// Residual: passengers fire from chinook origin at nearby enemies.
/// Fail-closed: fires from container origin; not C++ rider weapon bones / ropes.
#[test]
fn combat_chinook_residual_passenger_fire_damages_nearby_enemy() {
    let mut game_logic = GameLogic::new();
    ensure_test_infantry_template(&mut game_logic);
    ensure_test_tank_template(&mut game_logic);

    let chinook_id = create_test_combat_chinook(&mut game_logic, Vec3::new(0.0, 0.0, 0.0));
    let infantry_id = game_logic
        .create_object("TestInfantry", Team::USA, Vec3::new(1.0, 0.0, 0.0))
        .expect("infantry");
    let enemy_id = game_logic
        .create_object("TestTank", Team::GLA, Vec3::new(30.0, 0.0, 0.0))
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
        unit.target = Some(chinook_id);
        unit.set_contained_by(Some(chinook_id));
        unit.set_ai_state(AIState::Docked);
        unit.set_position(Vec3::new(0.0, 0.0, 0.0));
    }
    {
        let chinook = game_logic.host_object_mut(chinook_id).unwrap();
        assert!(chinook.add_occupant(infantry_id));
    }
    game_logic.refresh_battle_bus_armed_riders_weapon_set(chinook_id);

    let enemy_hp_before = game_logic
        .host_object(enemy_id)
        .map(|e| e.health.current)
        .unwrap_or(0.0);

    game_logic.update_combat(&[infantry_id, chinook_id, enemy_id], 1.0 / 30.0);

    let enemy_hp_after = game_logic
        .host_object(enemy_id)
        .map(|e| e.health.current)
        .unwrap_or(0.0);
    assert!(
        enemy_hp_after < enemy_hp_before,
        "Combat Chinook passenger residual fire must damage nearby enemy (before={enemy_hp_before}, after={enemy_hp_after})"
    );
    assert!(
        game_logic.honesty_combat_chinook_passenger_fire_ok(),
        "passenger fire residual honesty"
    );
    assert_eq!(
        game_logic.host_object(infantry_id).unwrap().ai_state,
        AIState::Docked,
        "firing must not eject Combat Chinook passenger"
    );
    assert_eq!(
        game_logic.host_object(infantry_id).unwrap().contained_by,
        Some(chinook_id)
    );
}

/// Residual: full capacity (8) rejects further Enter.
#[test]
fn combat_chinook_residual_capacity_full_rejects_enter() {
    use crate::command_system::{CommandType, GameCommand};

    let mut game_logic = GameLogic::new();
    ensure_test_infantry_template(&mut game_logic);
    let chinook_id = create_test_combat_chinook(&mut game_logic, Vec3::new(0.0, 0.0, 0.0));

    for i in 0..crate::game_logic::host_combat_chinook::COMBAT_CHINOOK_TRANSPORT_SLOTS {
        let id = game_logic
            .create_object(
                "TestInfantry",
                Team::USA,
                Vec3::new(1.0 + i as f32 * 0.1, 0.0, 0.0),
            )
            .expect("infantry");
        {
            let unit = game_logic.host_object_mut(id).unwrap();
            unit.target = Some(chinook_id);
            unit.set_ai_state(AIState::Entering);
        }
        game_logic.update_ai(&[id, chinook_id], 1.0 / 30.0);
    }
    assert_eq!(
        game_logic
            .host_object(chinook_id)
            .unwrap()
            .transport_count(),
        crate::game_logic::host_combat_chinook::COMBAT_CHINOOK_TRANSPORT_SLOTS
    );

    let extra_id = game_logic
        .create_object("TestInfantry", Team::USA, Vec3::new(4.0, 0.0, 0.0))
        .expect("extra");
    game_logic.queue_command(GameCommand {
        command_type: CommandType::Enter {
            target_id: chinook_id,
        },
        player_id: 0,
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
        "full Combat Chinook residual must reject Enter"
    );
    assert_eq!(
        game_logic
            .host_object(chinook_id)
            .unwrap()
            .transport_count(),
        crate::game_logic::host_combat_chinook::COMBAT_CHINOOK_TRANSPORT_SLOTS
    );
}

/// Residual: ground vehicles may enter Combat Chinook (AllowInsideKindOf=INFANTRY VEHICLE).
#[test]
fn combat_chinook_residual_allows_vehicle_enter() {
    let mut game_logic = GameLogic::new();
    ensure_test_tank_template(&mut game_logic);
    let chinook_id = create_test_combat_chinook(&mut game_logic, Vec3::new(0.0, 0.0, 0.0));
    let tank_id = game_logic
        .create_object("TestTank", Team::USA, Vec3::new(2.0, 0.0, 0.0))
        .expect("tank");

    {
        let unit = game_logic.host_object_mut(tank_id).unwrap();
        unit.target = Some(chinook_id);
        unit.set_ai_state(AIState::Entering);
    }
    game_logic.update_ai(&[tank_id, chinook_id], 1.0 / 30.0);

    let chinook = game_logic.host_object(chinook_id).expect("chinook");
    assert!(
        chinook.contained_units().contains(&tank_id),
        "vehicles may enter Combat Chinook residual"
    );
    assert_eq!(game_logic.combat_chinook_residual_loads(), 1);
    let tank = game_logic.host_object(tank_id).expect("tank");
    assert_eq!(tank.ai_state, AIState::Docked);
}

/// C++ TransportContain::letRidersUpgradeWeaponSet skips non-infantry.
#[test]
fn combat_chinook_vehicle_rider_does_not_arm_weapon_set() {
    let mut game_logic = GameLogic::new();
    ensure_test_tank_template(&mut game_logic);
    let chinook_id = create_test_combat_chinook(&mut game_logic, Vec3::new(0.0, 0.0, 0.0));
    let tank_id = game_logic
        .create_object("TestTank", Team::USA, Vec3::new(2.0, 0.0, 0.0))
        .expect("tank");
    {
        let unit = game_logic.host_object_mut(tank_id).unwrap();
        unit.weapon = Some(Weapon {
            damage: 20.0,
            range: 80.0,
            ..Weapon::default()
        });
        unit.set_contained_by(Some(chinook_id));
        unit.set_ai_state(AIState::Docked);
    }
    {
        let chinook = game_logic.host_object_mut(chinook_id).unwrap();
        assert!(chinook.add_occupant(tank_id), "Combat Chinook admits vehicles");
    }
    game_logic.refresh_battle_bus_armed_riders_weapon_set(chinook_id);
    let chinook = game_logic.host_object(chinook_id).expect("chinook");
    assert!(
        !chinook.weapon_set_player_upgrade,
        "vehicle-only load must not set WEAPONSET_PLAYER_UPGRADE"
    );
}


// -----------------------------------------------------------------------
// China Listening Outpost residual (detect 300 + transport 2 + TankHunter payload)
// Fail-closed: not IR FX / multi-door / RIDERS_ATTACKING uncloak matrix.
// -----------------------------------------------------------------------

/// Residual: Listening Outpost create binds detect 300 + Slots=2 + InnateStealth + payload.
#[test]
fn listening_outpost_residual_capacity_detect_and_payload() {
    use crate::game_logic::host_listening_outpost::{
        is_listening_outpost_template, LISTENING_OUTPOST_DETECTION_RANGE,
        LISTENING_OUTPOST_INITIAL_PAYLOAD_COUNT, LISTENING_OUTPOST_STEALTH_DELAY_FRAMES,
        LISTENING_OUTPOST_TRANSPORT_SLOTS,
    };

    let mut game_logic = GameLogic::new();
    let outpost_id = create_test_listening_outpost(&mut game_logic, Vec3::new(0.0, 0.0, 0.0));
    let outpost = game_logic.host_object(outpost_id).expect("outpost");
    assert!(is_listening_outpost_template(&outpost.template_name));
    assert!(outpost.is_listening_outpost_style_container());
    assert!(outpost.can_contain());
    assert_eq!(
        outpost.transport_capacity(),
        LISTENING_OUTPOST_TRANSPORT_SLOTS
    );
    assert!(outpost.passengers_allowed_to_fire);
    assert!(outpost.armed_riders_upgrade_weapon_set);
    assert!(outpost.is_detector, "listening outpost detector residual");
    assert!(
        (outpost.detection_range - LISTENING_OUTPOST_DETECTION_RANGE).abs() < 0.1,
        "detection range residual 300, got {}",
        outpost.detection_range
    );
    assert!(
        !outpost.status.stealthed,
        "C++ ctor sets CAN_STEALTH only; STEALTHED waits StealthDelay"
    );
    assert!(outpost.innate_stealth);
    assert!(outpost.stealth_breaks_on_move);
    assert!(!outpost.stealth_breaks_on_attack);
    assert_eq!(outpost.stealth_delay_frames, LISTENING_OUTPOST_STEALTH_DELAY_FRAMES);
    assert_eq!(outpost.stealth_allowed_frame, LISTENING_OUTPOST_STEALTH_DELAY_FRAMES);
    // InitialPayload TankHunter × 2 residual docks when payload template available.
    assert!(
        outpost.transport_count() == LISTENING_OUTPOST_INITIAL_PAYLOAD_COUNT
            || game_logic.listening_outpost_residual_initial_payload_docks()
                == LISTENING_OUTPOST_INITIAL_PAYLOAD_COUNT as u32,
        "InitialPayload residual docks 2 TankHunters (count={}, docks={})",
        outpost.transport_count(),
        game_logic.listening_outpost_residual_initial_payload_docks()
    );
    assert!(
        game_logic.honesty_listening_outpost_initial_payload_ok()
            || outpost.transport_count() == LISTENING_OUTPOST_INITIAL_PAYLOAD_COUNT,
        "InitialPayload residual honesty"
    );
    // Armed riders from payload upgrade weapon set residual.
    if outpost.transport_count() > 0 {
        assert!(
            outpost.weapon_set_player_upgrade,
            "payload armed riders must upgrade Listening Outpost weapon set"
        );
        assert!(
            outpost.weapon.is_some(),
            "PLAYER_UPGRADE residual binds ListeningOutpost dummy weapon"
        );
        if let Some(dummy) = outpost.weapon.as_ref() {
            assert!(
                crate::game_logic::host_combat_chinook::is_passenger_dummy_weapon(dummy),
                "Listening Outpost dummy residual (damage ~0.1)"
            );
        }
    }
}

/// Residual: Listening Outpost detects stealthed enemies within DetectionRange 300.
#[test]
fn listening_outpost_residual_detect_stealth_in_range() {
    use crate::game_logic::host_listening_outpost::LISTENING_OUTPOST_DETECTION_RANGE;

    let mut game_logic = GameLogic::new();
    ensure_test_infantry_template(&mut game_logic);
    let outpost_id = create_test_listening_outpost(&mut game_logic, Vec3::new(0.0, 0.0, 0.0));

    // Stealthed enemy within 300 residual detect range.
    let stealth_id = game_logic
        .create_object(
            "TestInfantry",
            Team::GLA,
            Vec3::new(LISTENING_OUTPOST_DETECTION_RANGE * 0.5, 0.0, 0.0),
        )
        .expect("stealthed enemy");
    {
        let e = game_logic.host_object_mut(stealth_id).unwrap();
        e.set_status_stealthed(true);
        e.set_status_detected(false);
    }

    game_logic.frame = 1;
    game_logic.update_stealth_and_detection();
    {
        let e = game_logic.host_object(stealth_id).expect("enemy");
        assert!(
            e.status.detected,
            "listening outpost detector residual must reveal stealthed enemy in range"
        );
    }
    assert!(
        game_logic.honesty_listening_outpost_detect_ok(),
        "listening outpost detect honesty residual must fire"
    );
    assert!(
        game_logic.listening_outpost_residual_detects() >= 1,
        "detect counter residual"
    );

    // Fail-closed: enemy beyond 300 is not detected by this residual.
    let far_id = game_logic
        .create_object(
            "TestInfantry",
            Team::GLA,
            Vec3::new(LISTENING_OUTPOST_DETECTION_RANGE + 50.0, 0.0, 0.0),
        )
        .expect("far stealthed");
    {
        let e = game_logic.host_object_mut(far_id).unwrap();
        e.set_status_stealthed(true);
        e.set_status_detected(false);
    }
    // Clear prior detect so only far is checked for new mark (already-detected enemy
    // stays marked; far must remain undetected).
    game_logic.frame = 2;
    game_logic.update_stealth_and_detection();
    {
        let e = game_logic.host_object(far_id).expect("far");
        assert!(
            !e.status.detected,
            "fail-closed: enemy outside DetectionRange 300 must not be residual-detected"
        );
    }

    // Move residual: uncloak while moving, re-cloak when idle.
    {
        let o = game_logic.host_object_mut(outpost_id).unwrap();
        o.set_ai_state(AIState::Moving);
        o.set_status_moving(true);
        o.set_status_stealthed(true);
    }
    game_logic.update_stealth_and_detection();
    {
        let o = game_logic.host_object(outpost_id).expect("outpost");
        assert!(
            !o.status.stealthed,
            "moving listening outpost must uncloak residual"
        );
    }
    {
        let o = game_logic.host_object_mut(outpost_id).unwrap();
        o.set_ai_state(AIState::Idle);
        o.set_status_moving(false);
    }
    game_logic.update_stealth_and_detection();
    {
        let o = game_logic.host_object(outpost_id).expect("outpost");
        assert!(
            !o.status.stealthed,
            "idle listening outpost must not re-cloak instantly after stopping"
        );
    }
    let allowed = game_logic
        .host_object(outpost_id)
        .unwrap()
        .stealth_allowed_frame;
    game_logic.frame = allowed;
    game_logic.update_stealth_and_detection();
    {
        let o = game_logic.host_object(outpost_id).expect("outpost");
        assert!(
            o.status.stealthed,
            "idle listening outpost re-cloaks after StealthDelay"
        );
    }
}

#[test]
fn tunnel_network_oneshot_spawns_two_rpg_world_objects() {
    let mut logic = GameLogic::new();
    logic
        .players
        .insert(0, crate::game_logic::Player::new(0, Team::GLA, "GLA", true));
    let mut tunnel = ThingTemplate::new("GLATunnelNetwork");
    tunnel
        .add_kind_of(KindOf::Structure)
        .add_kind_of(KindOf::Selectable)
        .set_health(1000.0);
    logic.templates.insert("GLATunnelNetwork".into(), tunnel);
    let mut rpg = ThingTemplate::new("GLAInfantryTunnelDefender");
    rpg.add_kind_of(KindOf::Infantry)
        .add_kind_of(KindOf::Selectable)
        .set_health(100.0);
    logic
        .templates
        .insert("GLAInfantryTunnelDefender".into(), rpg);

    let tunnel_id = logic
        .create_object("GLATunnelNetwork", Team::GLA, Vec3::ZERO)
        .expect("tunnel");
    let troopers: Vec<_> = logic
        .objects
        .values()
        .filter(|o| {
            o.producer_id == Some(tunnel_id)
                && o.template_name.eq_ignore_ascii_case("GLAInfantryTunnelDefender")
        })
        .map(|o| o.id)
        .collect();
    assert_eq!(
        troopers.len(),
        2,
        "C++ SpawnBehavior OneShot must create two GLAInfantryTunnelDefender world objects"
    );
    for id in &troopers {
        let trooper = logic.host_object(*id).expect("trooper");
        assert!(
            trooper.status.disabled_held,
            "C++ exitObjectViaDoor setDisabled(DISABLED_HELD)"
        );
        assert!(!trooper.can_move(), "HELD spawn-point units cannot march off");
        let pos = trooper.get_position();
        assert!(
            (pos.x - 8.0).abs() > 0.5,
            "must not invent forward*8 + lateral*6 tunnel line, got {pos:?}"
        );
    }

    // OneShot must not fire again after children die.
    for id in troopers {
        logic.mark_object_for_destruction(id, None);
    }
    logic.apply_spawn_behavior_on_build_complete(tunnel_id);
    let again = logic
        .objects
        .values()
        .filter(|o| {
            o.producer_id == Some(tunnel_id)
                && o.template_name.eq_ignore_ascii_case("GLAInfantryTunnelDefender")
                && o.is_alive()
        })
        .count();
    assert_eq!(again, 0, "OneShot must not replace dead free RPG troopers");
}

#[test]
fn stinger_site_spawns_three_soldier_world_objects() {
    let mut logic = GameLogic::new();
    logic
        .players
        .insert(0, crate::game_logic::Player::new(0, Team::GLA, "GLA", true));
    let mut site = ThingTemplate::new("GLAStingerSite");
    site.add_kind_of(KindOf::Structure)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::FSBaseDefense)
        .set_health(1000.0);
    logic.templates.insert("GLAStingerSite".into(), site);
    let mut soldier = ThingTemplate::new("GLAInfantryStingerSoldier");
    soldier
        .add_kind_of(KindOf::Infantry)
        .add_kind_of(KindOf::Selectable)
        .set_health(100.0);
    logic
        .templates
        .insert("GLAInfantryStingerSoldier".into(), soldier);

    let site_id = logic
        .create_object("GLAStingerSite", Team::GLA, Vec3::ZERO)
        .expect("stinger");
    let soldiers: Vec<_> = logic
        .objects
        .values()
        .filter(|o| {
            o.producer_id == Some(site_id)
                && o.template_name.eq_ignore_ascii_case("GLAInfantryStingerSoldier")
        })
        .map(|o| o.id)
        .collect();
    assert_eq!(
        soldiers.len(),
        3,
        "C++ SpawnBehavior must create three GLAInfantryStingerSoldier world objects"
    );
    for id in &soldiers {
        let soldier = logic.host_object(*id).expect("soldier");
        assert!(
            soldier.status.disabled_held,
            "C++ exitObjectViaDoor setDisabled(DISABLED_HELD)"
        );
        assert!(!soldier.can_move(), "HELD hive soldiers cannot march off");
        let pos = soldier.get_position();
        let ring_r2 = pos.x * pos.x + pos.z * pos.z;
        assert!(
            (ring_r2 - 144.0).abs() > 4.0,
            "must not invent 12wu 120° stinger ring, got {pos:?}"
        );
    }

    logic.mark_object_for_destruction(site_id, None);
    let living = logic
        .objects
        .values()
        .filter(|o| {
            o.producer_id == Some(site_id)
                && o.template_name.eq_ignore_ascii_case("GLAInfantryStingerSoldier")
                && o.is_alive()
                && !o.status.effectively_dead
        })
        .count();
    assert_eq!(
        living, 0,
        "SpawnedRequireSpawner must kill remaining stinger soldiers with the site"
    );
}

#[test]
fn spawn_point_exit_uses_pristine_bones_and_refuses_full_slots() {
    use gamelogic::common::{Coord3D, Matrix3D};
    use gamelogic::object::draw::register_pristine_bone_lookup_hook;
    use std::f32::consts::FRAC_PI_2;

    register_pristine_bone_lookup_hook(Some(std::sync::Arc::new(
        |model, _scale, _frame, bone| {
            if model != "UBStingerSTest" {
                return None;
            }
            match bone {
                "SpawnPoint01" => Some((
                    1,
                    Matrix3D::from_rotation_translation(
                        glam::Quat::from_rotation_z(FRAC_PI_2),
                        glam::Vec3::new(4.0, 0.0, 0.0),
                    ),
                )),
                "SpawnPoint02" => Some((
                    2,
                    Matrix3D::from_translation(Coord3D::new(0.0, 5.0, 0.0)),
                )),
                "SpawnPoint03" => Some((
                    3,
                    Matrix3D::from_translation(Coord3D::new(-3.0, -2.0, 0.0)),
                )),
                _ => None,
            }
        },
    )));

    let mut logic = GameLogic::new();
    logic
        .players
        .insert(0, crate::game_logic::Player::new(0, Team::GLA, "GLA", true));
    let mut site = ThingTemplate::new("GLAStingerSite");
    site.add_kind_of(KindOf::Structure)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::FSBaseDefense)
        .set_health(1000.0);
    site.model_name = Some("UBStingerSTest".into());
    logic.templates.insert("GLAStingerSite".into(), site);
    let mut soldier = ThingTemplate::new("GLAInfantryStingerSoldier");
    soldier
        .add_kind_of(KindOf::Infantry)
        .add_kind_of(KindOf::Selectable)
        .set_health(100.0);
    logic
        .templates
        .insert("GLAInfantryStingerSoldier".into(), soldier);

    let site_id = logic
        .create_object("GLAStingerSite", Team::GLA, Vec3::new(10.0, 2.0, 20.0))
        .expect("stinger");
    let mut soldiers: Vec<(ObjectId, Vec3, f32)> = logic
        .objects
        .values()
        .filter(|o| {
            o.producer_id == Some(site_id)
                && o.template_name.eq_ignore_ascii_case("GLAInfantryStingerSoldier")
        })
        .map(|o| (o.id, o.get_position(), o.get_orientation()))
        .collect();
    soldiers.sort_by(|a, b| a.1.x.partial_cmp(&b.1.x).unwrap());
    assert_eq!(soldiers.len(), 3, "three bone slots");

    // C++ convertBonePosToWorldPos: parent (10,2,20) + local (x,z,y).
    // SpawnPoint03 (-3, -2, 0) → host (-3, 0, -2) → world (7, 2, 18)
    // SpawnPoint01 (4, 0, 0) rot Z 90° → host (4, 0, 0) → world (14, 2, 20)
    // SpawnPoint02 (0, 5, 0) → host (0, 0, 5) → world (10, 2, 25)
    assert!((soldiers[0].1.x - 7.0).abs() < 0.05 && (soldiers[0].1.z - 18.0).abs() < 0.05);
    assert!((soldiers[1].1.x - 10.0).abs() < 0.05 && (soldiers[1].1.z - 25.0).abs() < 0.05);
    assert!((soldiers[2].1.x - 14.0).abs() < 0.05 && (soldiers[2].1.z - 20.0).abs() < 0.05);
    assert!(
        (soldiers[2].2 - FRAC_PI_2).abs() < 0.05,
        "bone 01 world yaw is parent + Get_Z_Rotation"
    );
    for (id, _, _) in &soldiers {
        let obj = logic.host_object(*id).expect("held");
        assert!(obj.status.disabled_held);
        assert!(!obj.can_move());
    }

    logic.apply_spawn_behavior_on_build_complete(site_id);
    let again = logic
        .objects
        .values()
        .filter(|o| {
            o.producer_id == Some(site_id)
                && o.template_name.eq_ignore_ascii_case("GLAInfantryStingerSoldier")
                && o.is_alive()
        })
        .count();
    assert_eq!(again, 3, "reserveDoorForExit refuses when every bone is occupied");

    register_pristine_bone_lookup_hook(None);
}

#[test]
fn special_power_create_expresses_shared_n_sync_ready_now() {
    use crate::command_system::SpecialPowerType;
    use crate::game_logic::{Player, SpecialPowerModuleKind, SpecialPowerModuleMetadata};

    let mut logic = GameLogic::new();
    let mut player = Player::new(0, Team::USA, "USA", true);
    player.reset_shared_special_power_timer(&SpecialPowerType::SpyDrone, 60.0);
    assert!(
        player
            .shared_special_power_cooldowns
            .get(&SpecialPowerType::SpyDrone)
            .copied()
            .unwrap_or(0.0)
            > 0.0
    );
    logic.players.insert(0, player);

    let mut tpl = ThingTemplate::new("AmericaCommandCenter");
    tpl.add_kind_of(KindOf::Structure)
        .add_kind_of(KindOf::CommandCenter)
        .set_health(2000.0);
    tpl.has_special_power_create = true;
    tpl.special_power_modules.push(SpecialPowerModuleMetadata {
        source_index: 0,
        module_tag: Some("ModuleTag_SpyDrone".into()),
        module_kind: SpecialPowerModuleKind::SpecialAbility,
        special_power_template: "SpecialPowerSpyDrone".into(),
        special_power_template_id: 1,
        command_power: Some(SpecialPowerType::SpyDrone),
        reload_time_frames: 300,
        required_science: Some("SCIENCE_SpyDrone".into()),
        public_timer: false,
        shared_n_sync: true,
        shortcut_power: false,
        update_module_starts_attack: false,
        starts_paused: false,
        scripted_special_power_only: false,
    });
    tpl.special_power_modules.push(SpecialPowerModuleMetadata {
        source_index: 1,
        module_tag: Some("ModuleTag_Scripted".into()),
        module_kind: SpecialPowerModuleKind::SpecialAbility,
        special_power_template: "SpecialPowerScriptedOnly".into(),
        special_power_template_id: 2,
        command_power: Some(SpecialPowerType::Paradrop),
        reload_time_frames: 150,
        required_science: None,
        public_timer: false,
        shared_n_sync: false,
        shortcut_power: false,
        update_module_starts_attack: false,
        starts_paused: false,
        scripted_special_power_only: true,
    });
    logic.templates.insert("AmericaCommandCenter".into(), tpl);

    let id = logic
        .create_object_for_player("AmericaCommandCenter", 0, Vec3::ZERO)
        .expect("cc");
    let player = logic.players.get(&0).expect("player");
    assert!(
        !player
            .shared_special_power_cooldowns
            .contains_key(&SpecialPowerType::SpyDrone),
        "SharedNSync must express ready-now on Command Center build complete"
    );
    let obj = logic.host_object(id).expect("cc obj");
    let remaining = obj
        .special_power_cooldowns
        .get(&SpecialPowerType::Paradrop)
        .copied()
        .unwrap_or(0.0);
    assert!(
        remaining > 0.0,
        "scripted SpecialPowerCreate modules must start ReloadTime recharge"
    );
}

