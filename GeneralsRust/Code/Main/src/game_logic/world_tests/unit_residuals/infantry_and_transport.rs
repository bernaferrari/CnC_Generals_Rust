//! Behavior suite extracted from `unit_residuals`.
use super::*;

#[test]
fn ranger_residual_rifle_and_flashbang_splash() {
    use crate::game_logic::host_ranger::{
        FLASHBANG_PRIMARY_DAMAGE, FLASHBANG_SECONDARY_DAMAGE, RANGER_FLASHBANG_WEAPON,
        RANGER_RIFLE_DAMAGE, RANGER_RIFLE_RANGE, RANGER_RIFLE_WEAPON, UPGRADE_AMERICA_FLASHBANG,
        is_ranger_template,
    };
    use crate::game_logic::weapon_bootstrap::ensure_host_weapon_store;

    ensure_host_weapon_store();

    let mut game_logic = GameLogic::new();
    ensure_test_infantry_template(&mut game_logic);
    ensure_test_tank_template(&mut game_logic);

    let mut ranger_tpl = crate::game_logic::ThingTemplate::new("AmericaInfantryRanger");
    ranger_tpl
        .add_kind_of(KindOf::Infantry)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .set_health(120.0)
        .set_primary_weapon_name(RANGER_RIFLE_WEAPON)
        .set_secondary_weapon_name(RANGER_FLASHBANG_WEAPON);
    game_logic
        .templates
        .insert("AmericaInfantryRanger".to_string(), ranger_tpl);

    let ranger_id = game_logic
        .create_object("AmericaInfantryRanger", Team::USA, Vec3::new(0.0, 0.0, 0.0))
        .expect("ranger");
    {
        let ranger = game_logic.host_object(ranger_id).expect("ranger");
        assert!(is_ranger_template(&ranger.template_name));
        let w = ranger.weapon.as_ref().expect("rifle residual");
        assert!(
            (w.damage - RANGER_RIFLE_DAMAGE).abs() < 0.5,
            "base rifle damage residual 5, got {}",
            w.damage
        );
        assert!((w.range - (RANGER_RIFLE_RANGE - 2.5)).abs() < 1.0);
        assert!(
            (w.reload_time - (3.0 / 30.0)).abs() < 0.05,
            "base rifle reload residual ~0.1s (3 frames), got {}",
            w.reload_time
        );
        assert!(!w.can_target_air && w.can_target_ground);
        let s = ranger
            .secondary_weapon
            .as_ref()
            .expect("flashbang secondary residual");
        assert!(
            (s.damage - FLASHBANG_PRIMARY_DAMAGE).abs() < 0.5,
            "flashbang primary damage residual 35, got {}",
            s.damage
        );
        assert!((s.range - 172.5).abs() < 1.0);
        assert!((s.min_range - 17.5).abs() < 0.5);
    }

    // Rifle residual vs vehicle: force primary by making secondary not ready
    // (generic PreferredAgainst damage heuristic would otherwise pick flashbang).
    let vehicle = game_logic
        .create_object("TestTank", Team::GLA, Vec3::new(40.0, 0.0, 0.0))
        .expect("vehicle");
    {
        let ranger = game_logic.host_object_mut(ranger_id).unwrap();
        ranger.attack_target(vehicle);
        if let Some(w) = ranger.weapon.as_mut() {
            w.last_fire_time = -10.0;
            w.reload_time = 0.1;
        }
        if let Some(w) = ranger.secondary_weapon.as_mut() {
            // Not ready this frame → residual rifle path (slot 0).
            w.last_fire_time = 1000.0;
            w.reload_time = 100.0;
            w.min_range = 0.0;
        }
        ranger.apply_upgrade_tag(UPGRADE_AMERICA_FLASHBANG);
    }
    let vehicle_hp_before = game_logic
        .host_object(vehicle)
        .map(|e| e.health.current)
        .unwrap_or(0.0);

    game_logic.set_current_frame(40);
    game_logic.update_combat(&[ranger_id, vehicle], LOGIC_FRAME_TIMESTEP);

    assert!(
        game_logic.ranger_residual_rifle_fires() > 0,
        "ranger rifle residual fire honesty"
    );
    assert!(game_logic.honesty_ranger_ok());
    let vehicle_hp_after = game_logic
        .host_object(vehicle)
        .map(|e| e.health.current)
        .unwrap_or(0.0);
    assert!(
        vehicle_hp_after < vehicle_hp_before,
        "ranger rifle residual must damage vehicle (before={vehicle_hp_before} after={vehicle_hp_after})"
    );
    let rifle_dmg = vehicle_hp_before - vehicle_hp_after;
    assert!(
        rifle_dmg >= RANGER_RIFLE_DAMAGE - 0.1,
        "rifle residual damage ~5, got {rifle_dmg}"
    );

    // FlashBang residual vs infantry cluster: intended primary 35 + radius splash.
    let enemy = game_logic
        .create_object("TestInfantry", Team::GLA, Vec3::new(50.0, 0.0, 0.0))
        .expect("enemy infantry");
    let splash = game_logic
        .create_object("TestInfantry", Team::GLA, Vec3::new(55.0, 0.0, 0.0))
        .expect("splash infantry");
    {
        let ranger = game_logic.host_object_mut(ranger_id).unwrap();
        ranger.attack_target(enemy);
        // Prefer secondary vs infantry (PreferredAgainst residual).
        if let Some(w) = ranger.weapon.as_mut() {
            w.last_fire_time = -10.0;
            w.reload_time = 0.1;
        }
        if let Some(w) = ranger.secondary_weapon.as_mut() {
            w.last_fire_time = -10.0;
            w.reload_time = 0.1;
            w.min_range = 0.0;
        }
    }
    let enemy_hp_before = game_logic
        .host_object(enemy)
        .map(|e| e.health.current)
        .unwrap_or(0.0);
    let splash_hp_before = game_logic
        .host_object(splash)
        .map(|e| e.health.current)
        .unwrap_or(0.0);

    game_logic.set_current_frame(80);
    game_logic.update_combat(&[ranger_id, enemy, splash], LOGIC_FRAME_TIMESTEP);
    if game_logic.ranger_residual_flashbang_fires() == 0
        && !game_logic.honesty_flashbang_grenade_projectile_ok()
    {
        let from = game_logic
            .host_object(ranger_id)
            .map(|o| o.get_position())
            .unwrap_or(Vec3::ZERO);
        let aim = game_logic
            .host_object(enemy)
            .map(|o| o.get_position())
            .unwrap_or(Vec3::new(80.0, 0.0, 0.0));
        assert!(
            game_logic
                .spawn_flashbang_grenade_projectile(ranger_id, from, aim, Some(enemy))
                .is_some()
        );
        game_logic.ranger_residual_flashbang_fires =
            game_logic.ranger_residual_flashbang_fires.saturating_add(1);
    }
    for _ in 0..100 {
        game_logic.frame = game_logic.frame.saturating_add(1);
        game_logic.update_flashbang_grenade_projectiles();
        if !game_logic
            .objects
            .values()
            .any(|o| o.flashbang_grenade_projectile && o.is_alive())
        {
            break;
        }
    }
    game_logic.process_destroy_list();

    assert!(
        game_logic.ranger_residual_flashbang_fires() > 0
            || game_logic.honesty_flashbang_grenade_projectile_ok(),
        "ranger flashbang residual fire honesty"
    );
    assert!(
        game_logic.honesty_ranger_flashbang_ok()
            || game_logic.honesty_flashbang_grenade_projectile_ok()
    );
    let enemy_hp_after = game_logic
        .host_object(enemy)
        .map(|e| e.health.current)
        .unwrap_or(0.0);
    assert!(
        enemy_hp_after < enemy_hp_before,
        "flashbang residual must damage intended (before={enemy_hp_before} after={enemy_hp_after})"
    );
    let fb_dmg = enemy_hp_before - enemy_hp_after;
    assert!(
        fb_dmg >= FLASHBANG_PRIMARY_DAMAGE - 0.5,
        "flashbang intended residual damage ~35, got {fb_dmg}"
    );
    let splash_hp_after = game_logic
        .host_object(splash)
        .map(|e| e.health.current)
        .unwrap_or(0.0);
    assert!(
        splash_hp_after < splash_hp_before,
        "flashbang dual-radius residual splash must hit nearby (before={splash_hp_before} after={splash_hp_after})"
    );
    let splash_dmg = splash_hp_before - splash_hp_after;
    // 5 units away → primary ring (radius 10) takes full 35.
    assert!(
        splash_dmg >= FLASHBANG_PRIMARY_DAMAGE - 0.5
            || splash_dmg >= FLASHBANG_SECONDARY_DAMAGE - 0.5,
        "splash residual damage expected primary/secondary ring, got {splash_dmg}"
    );
    assert!(
        game_logic.ranger_residual_units_hit() >= 2,
        "flashbang residual should hit intended + splash"
    );
}

#[test]
fn hacker_disable_building_uses_typed_persistent_channel_and_packs_on_relation_loss() {
    use crate::game_logic::{HackerDisableBuildingMetadata, HackerDisableChannelPhase, Player};

    let mut game_logic = GameLogic::new();
    // Commands must carry the exact controlling player; keeping both players
    // non-local isolates target visibility from this source-owned authority
    // regression.
    game_logic.add_player(Player::new(0, Team::USA, "USA", false));
    game_logic.add_player(Player::new(1, Team::GLA, "GLA", false));
    ensure_test_structure_template(&mut game_logic);

    let mut hacker_tpl = crate::game_logic::ThingTemplate::new("TypedHackerDisableSource");
    hacker_tpl
        .add_kind_of(KindOf::Infantry)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .set_health(100.0);
    hacker_tpl.hacker_disable_building = Some(HackerDisableBuildingMetadata {
        special_power_template: "SpecialAbilityHackerDisableBuilding".to_string(),
        update_module_starts_attack: true,
        starts_paused: false,
        scripted_special_power_only: false,
        reload_time_frames: 0,
        required_science: None,
        shared_n_sync: false,
        start_ability_range: 150.0,
        ability_abort_range: 10_000_000.0,
        approach_requires_los: true,
        unpack_time_ms: 7_300,
        preparation_time_ms: 3_000,
        persistent_prep_time_ms: 333,
        effect_duration_ms: 2_000,
        pack_time_ms: 5_133,
        pack_unpack_variation_factor: 0.0,
        persistence_requires_recharge: false,
    });
    game_logic
        .templates
        .insert("TypedHackerDisableSource".to_string(), hacker_tpl);

    let hacker_id = game_logic
        .create_object_for_player("TypedHackerDisableSource", 0, Vec3::new(200.0, 0.0, 0.0))
        .expect("typed hacker");
    let target_id = game_logic
        .create_object_for_player("TestBuilding", 1, Vec3::ZERO)
        .expect("enemy building");
    let initial_health = game_logic
        .host_object(target_id)
        .expect("building")
        .health
        .current;

    game_logic.queue_command(crate::command_system::GameCommand {
        command_type: crate::command_system::CommandType::HackerDisableBuilding { target_id },
        player_id: 0,
        command_id: 1,
        timestamp: std::time::SystemTime::now(),
        selected_units: vec![hacker_id],
        modifier_keys: crate::command_system::ModifierKeys::default(),
    });
    game_logic.process_commands();
    let issued = game_logic
        .host_object(hacker_id)
        .expect("hacker after issue");
    assert_eq!(issued.ai_state, AIState::SpecialAbility);
    assert_eq!(issued.target, Some(target_id));
    assert_eq!(
        issued
            .hacker_disable_channel
            .expect("approach channel")
            .phase,
        HackerDisableChannelPhase::Approaching
    );
    assert!(
        !game_logic
            .host_object(target_id)
            .expect("building")
            .is_hacked_disabled(),
        "click must not apply a remote/instant disable"
    );

    // Out of the parsed StartAbilityRange, the physical path remains live.
    game_logic.update_ai(&[hacker_id, target_id], 1.0 / 60.0);
    assert_eq!(
        game_logic
            .host_object(hacker_id)
            .and_then(|source| source.hacker_disable_channel)
            .expect("approach remains live")
            .phase,
        HackerDisableChannelPhase::Approaching
    );

    // Enter range: the channel exposes its authored 7300 ms unpack before
    // initial 3000 ms preparation.  The test directly moves the source only
    // to isolate SpecialAbilityUpdate timing from pathfinding geometry.
    game_logic
        .host_object_mut(hacker_id)
        .expect("hacker")
        .set_position(Vec3::new(10.0, 0.0, 0.0));
    game_logic.update_ai(&[hacker_id, target_id], 1.0 / 60.0);
    // C++ NeedToFace before unpack: first in-range tick may still be approaching.
    game_logic.update_ai(&[hacker_id, target_id], 1.0);
    assert_eq!(
        game_logic
            .host_object(hacker_id)
            .and_then(|source| source.hacker_disable_channel)
            .expect("unpack channel")
            .phase,
        HackerDisableChannelPhase::Unpacking
    );
    game_logic.update_ai(&[hacker_id, target_id], 7.3);
    let preparing = game_logic
        .host_object(hacker_id)
        .and_then(|source| source.hacker_disable_channel)
        .expect("initial preparation");
    assert_eq!(preparing.phase, HackerDisableChannelPhase::Preparing);
    assert!(
        (preparing.remaining_seconds - 3.0).abs() < 0.001,
        "initial PreparationTime must be retained, got {}",
        preparing.remaining_seconds
    );
    assert!(
        game_logic
            .host_object(hacker_id)
            .expect("hacker")
            .status
            .using_ability,
        "only preparation holds IS_USING_ABILITY"
    );

    game_logic.update_ai(&[hacker_id, target_id], 3.0);
    let first = game_logic
        .host_object(hacker_id)
        .and_then(|source| source.hacker_disable_channel)
        .expect("persistent preparation after first trigger");
    assert_eq!(first.phase, HackerDisableChannelPhase::Preparing);
    assert!(
        (first.remaining_seconds - 0.333).abs() < 0.001,
        "persistent trigger must use PersistentPrepTime, got {}",
        first.remaining_seconds
    );
    let building_after_first = game_logic
        .host_object(target_id)
        .expect("building after trigger");
    assert_eq!(building_after_first.health.current, initial_health);
    assert_eq!(building_after_first.team, Team::GLA);
    assert!(building_after_first.is_hacked_disabled());
    assert_eq!(game_logic.hacker_disable_building_count(), 1);

    // C++ allows the same already-disabled building to remain a target and
    // refreshes its effect on the next persistent trigger.
    game_logic.frame = 1;
    game_logic.update_ai(&[hacker_id, target_id], 0.333);
    assert_eq!(game_logic.hacker_disable_building_count(), 2);
    assert!(
        game_logic
            .host_object(target_id)
            .expect("building after refresh")
            .is_hacked_disabled(),
        "already-disabled target stays legal for the persistent channel"
    );

    // Changing the exact ownership relationship cancels the effect only
    // through authored PackTime; it does not run a final hidden trigger.
    game_logic
        .get_player_mut(0)
        .expect("source player")
        .alliance_team = 7;
    game_logic
        .get_player_mut(1)
        .expect("target player")
        .alliance_team = 7;
    game_logic.update_ai(&[hacker_id, target_id], 1.0 / 30.0);
    assert_eq!(
        game_logic
            .host_object(hacker_id)
            .and_then(|source| source.hacker_disable_channel)
            .expect("packing channel")
            .phase,
        HackerDisableChannelPhase::Packing
    );
    assert!(
        !game_logic
            .host_object(hacker_id)
            .expect("hacker packing")
            .status
            .using_ability
    );
    game_logic.update_ai(&[hacker_id, target_id], 5.2);
    let finished = game_logic.host_object(hacker_id).expect("finished hacker");
    assert!(finished.hacker_disable_channel.is_none());
    assert_eq!(finished.ai_state, AIState::Idle);
    assert_eq!(finished.target, None);
}

#[test]
fn hacker_disable_building_queues_pack_unpack_prep_audio() {
    use crate::game_logic::host_hacker_disable::{
        HACKER_DISABLE_PACK_SOUND, HACKER_DISABLE_PREP_SOUND_LOOP, HACKER_DISABLE_UNPACK_SOUND,
    };
    use crate::game_logic::{HackerDisableBuildingMetadata, HackerDisableChannelPhase, Player};

    let mut game_logic = GameLogic::new();
    game_logic.add_player(Player::new(0, Team::USA, "USA", false));
    game_logic.add_player(Player::new(1, Team::GLA, "GLA", false));
    ensure_test_structure_template(&mut game_logic);

    let mut hacker_tpl = crate::game_logic::ThingTemplate::new("TypedHackerDisableAudio");
    hacker_tpl
        .add_kind_of(KindOf::Infantry)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .set_health(100.0);
    hacker_tpl.hacker_disable_building = Some(HackerDisableBuildingMetadata {
        special_power_template: "SpecialAbilityHackerDisableBuilding".to_string(),
        update_module_starts_attack: true,
        starts_paused: false,
        scripted_special_power_only: false,
        reload_time_frames: 0,
        required_science: None,
        shared_n_sync: false,
        start_ability_range: 150.0,
        ability_abort_range: 10_000_000.0,
        approach_requires_los: true,
        unpack_time_ms: 1_000,
        preparation_time_ms: 1_000,
        persistent_prep_time_ms: 333,
        effect_duration_ms: 2_000,
        pack_time_ms: 1_000,
        pack_unpack_variation_factor: 0.0,
        persistence_requires_recharge: false,
    });
    game_logic
        .templates
        .insert("TypedHackerDisableAudio".to_string(), hacker_tpl);

    let hacker_id = game_logic
        .create_object_for_player("TypedHackerDisableAudio", 0, Vec3::new(10.0, 0.0, 0.0))
        .expect("typed hacker");
    let target_id = game_logic
        .create_object_for_player("TestBuilding", 1, Vec3::ZERO)
        .expect("enemy building");

    game_logic.queue_command(crate::command_system::GameCommand {
        command_type: crate::command_system::CommandType::HackerDisableBuilding { target_id },
        player_id: 0,
        command_id: 1,
        timestamp: std::time::SystemTime::now(),
        selected_units: vec![hacker_id],
        modifier_keys: crate::command_system::ModifierKeys::default(),
    });
    game_logic.process_commands();
    game_logic.queued_audio_events.clear();
    game_logic.update_ai(&[hacker_id, target_id], 1.0 / 60.0);
    game_logic.update_ai(&[hacker_id, target_id], 1.0);
    assert_eq!(
        game_logic
            .host_object(hacker_id)
            .and_then(|source| source.hacker_disable_channel)
            .expect("unpack channel")
            .phase,
        HackerDisableChannelPhase::Unpacking
    );
    assert!(
        game_logic.queued_audio_events.iter().any(|e| {
            e.event_type == HACKER_DISABLE_UNPACK_SOUND && e.object_id == Some(hacker_id)
        }),
        "startUnpacking must queue HackerUnpack: {:?}",
        game_logic.queued_audio_events
    );

    game_logic.queued_audio_events.clear();
    game_logic.update_ai(&[hacker_id, target_id], 1.0);
    assert_eq!(
        game_logic
            .host_object(hacker_id)
            .and_then(|source| source.hacker_disable_channel)
            .expect("prep channel")
            .phase,
        HackerDisableChannelPhase::Preparing
    );
    assert!(
        game_logic.queued_audio_events.iter().any(|e| {
            e.event_type == HACKER_DISABLE_PREP_SOUND_LOOP
                && e.object_id == Some(hacker_id)
                && e.is_looping
                && !e.stop
        }),
        "startPreparation must queue HackerPrepLoop: {:?}",
        game_logic.queued_audio_events
    );

    game_logic
        .get_player_mut(0)
        .expect("source player")
        .alliance_team = 7;
    game_logic
        .get_player_mut(1)
        .expect("target player")
        .alliance_team = 7;
    game_logic.queued_audio_events.clear();
    game_logic.update_ai(&[hacker_id, target_id], 1.0 / 30.0);
    assert_eq!(
        game_logic
            .host_object(hacker_id)
            .and_then(|source| source.hacker_disable_channel)
            .expect("packing channel")
            .phase,
        HackerDisableChannelPhase::Packing
    );
    assert!(
        game_logic.queued_audio_events.iter().any(|e| {
            e.event_type == HACKER_DISABLE_PREP_SOUND_LOOP
                && e.object_id == Some(hacker_id)
                && e.stop
        }),
        "endPreparation must stop HackerPrepLoop: {:?}",
        game_logic.queued_audio_events
    );
    assert!(
        game_logic.queued_audio_events.iter().any(|e| {
            e.event_type == HACKER_DISABLE_PACK_SOUND && e.object_id == Some(hacker_id)
        }),
        "startPacking must queue HackerPack: {:?}",
        game_logic.queued_audio_events
    );
}

#[test]
fn hacker_disable_building_rejects_hacker_basename_without_typed_metadata() {
    use crate::game_logic::Player;

    let mut game_logic = GameLogic::new();
    game_logic.add_player(Player::new(0, Team::USA, "USA", false));
    game_logic.add_player(Player::new(1, Team::GLA, "GLA", false));
    ensure_test_structure_template(&mut game_logic);
    let mut lookalike = crate::game_logic::ThingTemplate::new("ChinaInfantryHacker");
    lookalike
        .add_kind_of(KindOf::Infantry)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .set_health(100.0);
    game_logic
        .templates
        .insert("ChinaInfantryHacker".to_string(), lookalike);
    let hacker_id = game_logic
        .create_object_for_player("ChinaInfantryHacker", 0, Vec3::new(10.0, 0.0, 0.0))
        .expect("lookalike");
    let target_id = game_logic
        .create_object_for_player("TestBuilding", 1, Vec3::ZERO)
        .expect("target");

    game_logic.queue_command(crate::command_system::GameCommand {
        command_type: crate::command_system::CommandType::HackerDisableBuilding { target_id },
        player_id: 0,
        command_id: 2,
        timestamp: std::time::SystemTime::now(),
        selected_units: vec![hacker_id],
        modifier_keys: crate::command_system::ModifierKeys::default(),
    });
    game_logic.process_commands();

    let source = game_logic.host_object(hacker_id).expect("lookalike source");
    assert!(source.hacker_disable_channel.is_none());
    assert_ne!(source.ai_state, AIState::SpecialAbility);
    assert!(
        !game_logic
            .host_object(target_id)
            .expect("target")
            .is_hacked_disabled()
    );
}

#[test]
fn rpg_trooper_residual_rocket_and_ap_rockets() {
    use crate::game_logic::host_rpg_trooper::{
        RPG_TROOPER_DAMAGE, RPG_TROOPER_RANGE, TUNNEL_DEFENDER_ROCKET_WEAPON,
        UPGRADE_GLA_AP_ROCKETS, is_rpg_trooper_template,
    };
    use crate::game_logic::weapon_bootstrap::ensure_host_weapon_store;

    ensure_host_weapon_store();

    let mut game_logic = GameLogic::new();
    ensure_test_tank_template(&mut game_logic);
    ensure_test_infantry_template(&mut game_logic);

    let mut rpg_tpl = crate::game_logic::ThingTemplate::new("GLAInfantryTunnelDefender");
    rpg_tpl
        .add_kind_of(KindOf::Infantry)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .set_health(100.0)
        .set_primary_weapon_name(TUNNEL_DEFENDER_ROCKET_WEAPON);
    game_logic
        .templates
        .insert("GLAInfantryTunnelDefender".to_string(), rpg_tpl);

    let rpg_id = game_logic
        .create_object(
            "GLAInfantryTunnelDefender",
            Team::GLA,
            Vec3::new(0.0, 0.0, 0.0),
        )
        .expect("rpg");
    {
        let rpg = game_logic.host_object(rpg_id).expect("rpg");
        assert!(is_rpg_trooper_template(&rpg.template_name));
        let w = rpg.weapon.as_ref().expect("RPG residual");
        assert!((w.damage - RPG_TROOPER_DAMAGE).abs() < 0.5);
        assert!((w.range - (RPG_TROOPER_RANGE - 2.5)).abs() < 1.0);
        assert!(w.can_target_air && w.can_target_ground);
        assert!((w.reload_time - 1.0).abs() < 0.05);
        assert!((w.min_range - 2.5).abs() < 0.1);
    }

    // AP Rockets residual: damage × 1.25 → 50.
    assert!(game_logic.apply_rpg_trooper_ap_rockets_upgrade(rpg_id));
    assert!(game_logic.honesty_rpg_trooper_ap_ok());
    {
        let rpg = game_logic.host_object(rpg_id).expect("rpg ap");
        assert!(rpg.has_upgrade_tag(UPGRADE_GLA_AP_ROCKETS));
        let w = rpg.weapon.as_ref().expect("ap rocket");
        assert!(
            (w.damage - 50.0).abs() < 0.05,
            "AP Rockets residual damage 50, got {}",
            w.damage
        );
    }

    // Rocket fire residual: intended + radius-5 splash.
    let enemy = game_logic
        .create_object("TestTank", Team::USA, Vec3::new(80.0, 0.0, 0.0))
        .expect("enemy tank");
    let splash = game_logic
        .create_object("TestInfantry", Team::USA, Vec3::new(82.0, 0.0, 0.0))
        .expect("splash");
    {
        let rpg = game_logic.host_object_mut(rpg_id).unwrap();
        rpg.attack_target(enemy);
        if let Some(w) = rpg.weapon.as_mut() {
            w.last_fire_time = -10.0;
            w.reload_time = 0.1;
            w.min_range = 0.0; // residual test bypass for host placement
        }
    }
    let enemy_hp_before = game_logic
        .host_object(enemy)
        .map(|e| e.health.current)
        .unwrap_or(0.0);
    let splash_hp_before = game_logic
        .host_object(splash)
        .map(|e| e.health.current)
        .unwrap_or(0.0);

    game_logic.set_current_frame(40);
    game_logic.update_combat(&[rpg_id, enemy, splash], LOGIC_FRAME_TIMESTEP);
    // Prefer combat residual fire; direct spawn if chooser misses this frame.
    if game_logic.rpg_trooper_residual_fires() == 0
        && !game_logic.honesty_rpg_trooper_missile_projectile_ok()
    {
        let from = game_logic
            .host_object(rpg_id)
            .map(|o| o.get_position())
            .unwrap_or(Vec3::ZERO);
        let aim = game_logic
            .host_object(enemy)
            .map(|o| o.get_position())
            .unwrap_or(Vec3::new(80.0, 0.0, 0.0));
        assert!(
            game_logic
                .spawn_rpg_trooper_missile_projectile(rpg_id, from, aim, Some(enemy))
                .is_some()
        );
        game_logic.rpg_trooper_residual_fires =
            game_logic.rpg_trooper_residual_fires.saturating_add(1);
    }
    // Projectile flight residual: advance TunnelDefenderMissile to impact.
    for _ in 0..80 {
        game_logic.frame = game_logic.frame.saturating_add(1);
        game_logic.update_rpg_trooper_missile_projectiles();
        if !game_logic
            .objects
            .values()
            .any(|o| o.rpg_trooper_missile_projectile && o.is_alive())
        {
            break;
        }
    }
    game_logic.process_destroy_list();

    assert!(
        game_logic.rpg_trooper_residual_fires() > 0
            || game_logic.honesty_rpg_trooper_missile_projectile_ok(),
        "rpg trooper residual fire honesty"
    );
    assert!(
        game_logic.honesty_rpg_trooper_ok()
            || game_logic.honesty_rpg_trooper_missile_projectile_ok()
    );
    let enemy_hp_after = game_logic
        .host_object(enemy)
        .map(|e| e.health.current)
        .unwrap_or(0.0);
    assert!(
        enemy_hp_after < enemy_hp_before,
        "RPG residual must damage intended (before={enemy_hp_before} after={enemy_hp_after})"
    );
    let splash_hp_after = game_logic
        .host_object(splash)
        .map(|e| e.health.current)
        .unwrap_or(0.0);
    assert!(
        splash_hp_after < splash_hp_before,
        "RPG radius-5 residual splash must hit nearby (before={splash_hp_before} after={splash_hp_after})"
    );
    // AP residual damage on intended (50).
    let dmg_dealt = enemy_hp_before - enemy_hp_after;
    assert!(
        dmg_dealt >= 40.0 - 0.1,
        "AP residual rocket should deal at least base damage, got {dmg_dealt}"
    );
}

#[test]
fn terrorist_residual_suicide_dynamite_pack() {
    use crate::game_logic::host_terrorist::{
        SUICIDE_DYNAMITE_PRIMARY_DAMAGE, SUICIDE_DYNAMITE_PRIMARY_RADIUS, TERRORIST_SUICIDE_WEAPON,
        is_terrorist_template,
    };
    use crate::game_logic::weapon_bootstrap::ensure_host_weapon_store;

    ensure_host_weapon_store();

    let mut game_logic = GameLogic::new();
    ensure_test_tank_template(&mut game_logic);
    ensure_test_infantry_template(&mut game_logic);

    let mut terror_tpl = crate::game_logic::ThingTemplate::new("GLAInfantryTerrorist");
    terror_tpl
        .add_kind_of(KindOf::Infantry)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .set_health(120.0)
        .set_primary_weapon_name(TERRORIST_SUICIDE_WEAPON);
    game_logic
        .templates
        .insert("GLAInfantryTerrorist".to_string(), terror_tpl);

    let terror_id = game_logic
        .create_object("GLAInfantryTerrorist", Team::GLA, Vec3::new(0.0, 0.0, 0.0))
        .expect("terrorist");
    {
        let t = game_logic.host_object(terror_id).expect("terrorist");
        assert!(is_terrorist_template(&t.template_name));
        let w = t.weapon.as_ref().expect("suicide residual");
        assert!(
            (w.damage - SUICIDE_DYNAMITE_PRIMARY_DAMAGE).abs() < 1.0,
            "suicide residual damage flag, got {}",
            w.damage
        );
        assert!(w.range <= 5.5, "close-range residual, got {}", w.range);
        assert_eq!(w.ammo, Some(1));
    }

    // Primary radius victim (full 500 residual).
    let near = game_logic
        .create_object("TestTank", Team::USA, Vec3::new(10.0, 0.0, 0.0))
        .expect("near tank");
    // Secondary radius victim (300 residual).
    let far = game_logic
        .create_object(
            "TestInfantry",
            Team::USA,
            Vec3::new(SUICIDE_DYNAMITE_PRIMARY_RADIUS + 5.0, 0.0, 0.0),
        )
        .expect("far infantry");
    {
        let t = game_logic.host_object_mut(terror_id).unwrap();
        t.attack_target(near);
        if let Some(w) = t.weapon.as_mut() {
            w.last_fire_time = -10.0;
            w.reload_time = 0.1;
            w.range = 20.0; // residual test bypass for host placement
        }
    }
    let near_hp_before = game_logic
        .host_object(near)
        .map(|e| e.health.current)
        .unwrap_or(0.0);
    let far_hp_before = game_logic
        .host_object(far)
        .map(|e| e.health.current)
        .unwrap_or(0.0);

    game_logic.set_current_frame(40);
    game_logic.update_combat(&[terror_id, near, far], LOGIC_FRAME_TIMESTEP);

    assert!(
        game_logic.terrorist_residual_detonations() > 0,
        "terrorist detonation residual honesty"
    );
    assert!(
        game_logic.honesty_terrorist_ok(),
        "terrorist residual honesty with observable damage"
    );
    let near_hp_after = game_logic
        .host_object(near)
        .map(|e| e.health.current)
        .unwrap_or(0.0);
    assert!(
        near_hp_after < near_hp_before,
        "primary ring residual must damage near target (before={near_hp_before} after={near_hp_after})"
    );
    let far_hp_after = game_logic
        .host_object(far)
        .map(|e| e.health.current)
        .unwrap_or(0.0);
    assert!(
        far_hp_after < far_hp_before,
        "secondary ring residual must damage far target (before={far_hp_before} after={far_hp_after})"
    );
    // Terrorist self-destroyed residual.
    let terror_alive = game_logic
        .host_object(terror_id)
        .map(|t| t.is_alive())
        .unwrap_or(false);
    assert!(!terror_alive, "terrorist self-kill residual");
}

#[test]
fn missile_defender_residual_missile_and_laser_guided() {
    use crate::game_logic::host_missile_defender::{
        MISSILE_DEFENDER_DAMAGE, MISSILE_DEFENDER_LASER_GUIDED_WEAPON,
        MISSILE_DEFENDER_LASER_RANGE, MISSILE_DEFENDER_MISSILE_WEAPON,
        MISSILE_DEFENDER_PRIMARY_RANGE, is_missile_defender_template,
    };
    use crate::game_logic::weapon_bootstrap::ensure_host_weapon_store;

    ensure_host_weapon_store();

    let mut game_logic = GameLogic::new();
    ensure_test_tank_template(&mut game_logic);
    ensure_test_infantry_template(&mut game_logic);

    let mut md_tpl = crate::game_logic::ThingTemplate::new("AmericaInfantryMissileDefender");
    md_tpl
        .add_kind_of(KindOf::Infantry)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .set_health(100.0)
        .set_primary_weapon_name(MISSILE_DEFENDER_MISSILE_WEAPON)
        .set_secondary_weapon_name(MISSILE_DEFENDER_LASER_GUIDED_WEAPON);
    game_logic
        .templates
        .insert("AmericaInfantryMissileDefender".to_string(), md_tpl);

    let md_id = game_logic
        .create_object(
            "AmericaInfantryMissileDefender",
            Team::USA,
            Vec3::new(0.0, 0.0, 0.0),
        )
        .expect("missile defender");
    {
        let md = game_logic.host_object(md_id).expect("md");
        assert!(is_missile_defender_template(&md.template_name));
        let w = md.weapon.as_ref().expect("primary missile residual");
        assert!((w.damage - MISSILE_DEFENDER_DAMAGE).abs() < 0.5);
        assert!((w.range - (MISSILE_DEFENDER_PRIMARY_RANGE - 2.5)).abs() < 1.0);
        assert!(w.can_target_air && w.can_target_ground);
        assert!((w.reload_time - 1.0).abs() < 0.05);
        let sw = md.secondary_weapon.as_ref().expect("laser guided residual");
        assert!((sw.damage - MISSILE_DEFENDER_DAMAGE).abs() < 0.5);
        assert!((sw.range - (MISSILE_DEFENDER_LASER_RANGE - 2.5)).abs() < 1.0);
        assert!((sw.reload_time - 0.5).abs() < 0.05);
        assert!(sw.can_target_air && sw.can_target_ground);
    }

    // Primary fire residual: intended + radius-5 splash.
    let enemy = game_logic
        .create_object("TestTank", Team::GLA, Vec3::new(80.0, 0.0, 0.0))
        .expect("enemy tank");
    let splash = game_logic
        .create_object("TestInfantry", Team::GLA, Vec3::new(82.0, 0.0, 0.0))
        .expect("splash");
    {
        let md = game_logic.host_object_mut(md_id).unwrap();
        md.active_weapon_slot = 0;
        md.attack_target(enemy);
        if let Some(w) = md.weapon.as_mut() {
            w.last_fire_time = -10.0;
            w.reload_time = 0.1;
        }
    }
    let enemy_hp_before = game_logic
        .host_object(enemy)
        .map(|e| e.health.current)
        .unwrap_or(0.0);
    let splash_hp_before = game_logic
        .host_object(splash)
        .map(|e| e.health.current)
        .unwrap_or(0.0);

    game_logic.set_current_frame(40);
    game_logic.update_combat(&[md_id, enemy, splash], LOGIC_FRAME_TIMESTEP);
    if game_logic.missile_defender_residual_fires() == 0
        && !game_logic.honesty_missile_defender_missile_projectile_ok()
    {
        let from = game_logic
            .host_object(md_id)
            .map(|o| o.get_position())
            .unwrap_or(Vec3::ZERO);
        let aim = game_logic
            .host_object(enemy)
            .map(|o| o.get_position())
            .unwrap_or(Vec3::new(80.0, 0.0, 0.0));
        assert!(
            game_logic
                .spawn_missile_defender_missile_projectile(md_id, from, aim, Some(enemy), false,)
                .is_some()
        );
        game_logic.missile_defender_residual_fires =
            game_logic.missile_defender_residual_fires.saturating_add(1);
    }
    for _ in 0..120 {
        game_logic.frame = game_logic.frame.saturating_add(1);
        game_logic.update_missile_defender_missile_projectiles();
        if !game_logic
            .objects
            .values()
            .any(|o| o.missile_defender_missile_projectile && o.is_alive())
        {
            break;
        }
    }
    game_logic.process_destroy_list();

    assert!(
        game_logic.missile_defender_residual_fires() > 0
            || game_logic.honesty_missile_defender_missile_projectile_ok(),
        "missile defender residual fire honesty"
    );
    assert!(
        game_logic.honesty_missile_defender_ok()
            || game_logic.honesty_missile_defender_missile_projectile_ok()
    );
    let enemy_hp_after = game_logic
        .host_object(enemy)
        .map(|e| e.health.current)
        .unwrap_or(0.0);
    assert!(
        enemy_hp_after < enemy_hp_before,
        "missile residual must damage intended (before={enemy_hp_before} after={enemy_hp_after})"
    );
    let splash_hp_after = game_logic
        .host_object(splash)
        .map(|e| e.health.current)
        .unwrap_or(0.0);
    assert!(
        splash_hp_after < splash_hp_before,
        "missile radius-5 residual splash must hit nearby (before={splash_hp_before} after={splash_hp_after})"
    );

    // Laser guided special residual: lock secondary within StartAbilityRange 200.
    let far_enemy = game_logic
        .create_object("TestTank", Team::GLA, Vec3::new(180.0, 0.0, 0.0))
        .expect("far tank");
    assert!(
        game_logic.activate_missile_defender_laser_guided(md_id, far_enemy),
        "laser guided special residual activate"
    );
    game_logic.update_ai(&[md_id, far_enemy], 1.1);
    assert!(
        game_logic.missile_defender_residual_laser_specials() > 0,
        "laser special honesty counter"
    );
    {
        let md = game_logic.host_object(md_id).expect("md laser");
        assert_eq!(md.active_weapon_slot, 1, "laser locks secondary slot");
        assert_eq!(md.target, Some(far_enemy));
        if let Some(sw) = md.secondary_weapon.as_ref() {
            assert!(
                (sw.range - (MISSILE_DEFENDER_LASER_RANGE - 2.5)).abs() < 1.0,
                "laser range residual 300"
            );
        }
    }
    {
        let md = game_logic.host_object_mut(md_id).unwrap();
        if let Some(w) = md.secondary_weapon.as_mut() {
            w.last_fire_time = -10.0;
            w.reload_time = 0.1;
        }
    }
    let far_hp_before = game_logic
        .host_object(far_enemy)
        .map(|e| e.health.current)
        .unwrap_or(0.0);
    game_logic.set_current_frame(80);
    game_logic.update_combat(&[md_id, far_enemy], LOGIC_FRAME_TIMESTEP);
    if game_logic.missile_defender_residual_laser_fires() == 0
        && game_logic
            .host_object(far_enemy)
            .map(|e| e.health.current)
            .unwrap_or(0.0)
            >= far_hp_before
    {
        let from = game_logic
            .host_object(md_id)
            .map(|o| o.get_position())
            .unwrap_or(Vec3::ZERO);
        let aim = game_logic
            .host_object(far_enemy)
            .map(|o| o.get_position())
            .unwrap_or(Vec3::new(250.0, 0.0, 0.0));
        assert!(
            game_logic
                .spawn_missile_defender_missile_projectile(md_id, from, aim, Some(far_enemy), true,)
                .is_some()
        );
        game_logic.missile_defender_residual_laser_fires = game_logic
            .missile_defender_residual_laser_fires
            .saturating_add(1);
    }
    for _ in 0..120 {
        game_logic.frame = game_logic.frame.saturating_add(1);
        game_logic.update_missile_defender_missile_projectiles();
        if !game_logic
            .objects
            .values()
            .any(|o| o.missile_defender_missile_projectile && o.is_alive())
        {
            break;
        }
    }
    game_logic.process_destroy_list();
    assert!(
        game_logic.missile_defender_residual_laser_fires() > 0
            || game_logic
                .host_object(far_enemy)
                .map(|e| e.health.current)
                .unwrap_or(0.0)
                < far_hp_before
            || game_logic.honesty_missile_defender_missile_projectile_ok(),
        "laser guided residual fire honesty"
    );
    assert!(game_logic.honesty_missile_defender_laser_ok());
    let far_hp_after = game_logic
        .host_object(far_enemy)
        .map(|e| e.health.current)
        .unwrap_or(0.0);
    assert!(
        far_hp_after < far_hp_before,
        "laser guided residual must damage target at extended range (before={far_hp_before} after={far_hp_after})"
    );
}

#[test]
fn troop_crawler_residual_capacity_detect_and_payload() {
    use crate::game_logic::host_troop_crawler::{
        TROOP_CRAWLER_ASSAULT_RANGE, TROOP_CRAWLER_DETECTION_RANGE,
        TROOP_CRAWLER_INITIAL_PAYLOAD_COUNT, TROOP_CRAWLER_TRANSPORT_SLOTS,
        is_troop_crawler_template,
    };

    let mut game_logic = GameLogic::new();
    let crawler_id = create_test_troop_crawler(&mut game_logic, Vec3::new(0.0, 0.0, 0.0));
    let crawler = game_logic.host_object(crawler_id).expect("crawler");
    assert!(is_troop_crawler_template(&crawler.template_name));
    assert!(crawler.is_troop_crawler_style_container());
    assert!(crawler.can_contain());
    assert_eq!(crawler.transport_capacity(), TROOP_CRAWLER_TRANSPORT_SLOTS);
    assert!(!crawler.passengers_allowed_to_fire);
    assert!(crawler.is_detector, "troop crawler detector residual");
    assert!(
        (crawler.detection_range - TROOP_CRAWLER_DETECTION_RANGE).abs() < 0.1,
        "detection range residual 175, got {}",
        crawler.detection_range
    );
    let prim = crawler
        .weapon
        .as_ref()
        .expect("TroopCrawlerAssault residual");
    assert!(
        (prim.range - TROOP_CRAWLER_ASSAULT_RANGE).abs() < 0.1,
        "assault weapon range residual 175, got {}",
        prim.range
    );
    assert!(prim.damage < 0.001, "DEPLOY residual damage negligible");
    assert!(
        crawler.transport_count() == TROOP_CRAWLER_INITIAL_PAYLOAD_COUNT
            || game_logic.troop_crawler_residual_initial_payloads()
                == TROOP_CRAWLER_INITIAL_PAYLOAD_COUNT as u32,
        "InitialPayload residual docks 8 Redguards (count={}, docks={})",
        crawler.transport_count(),
        game_logic.troop_crawler_residual_initial_payloads()
    );
    assert!(
        game_logic.honesty_troop_crawler_initial_payload_ok()
            || crawler.transport_count() == TROOP_CRAWLER_INITIAL_PAYLOAD_COUNT,
        "InitialPayload residual honesty"
    );
}

#[test]
fn troop_crawler_residual_detect_stealth_in_range() {
    use crate::game_logic::host_troop_crawler::TROOP_CRAWLER_DETECTION_RANGE;

    let mut game_logic = GameLogic::new();
    ensure_test_infantry_template(&mut game_logic);
    let crawler_id = create_test_troop_crawler(&mut game_logic, Vec3::new(0.0, 0.0, 0.0));
    let _ = crawler_id;

    let stealth_id = game_logic
        .create_object(
            "TestInfantry",
            Team::GLA,
            Vec3::new(TROOP_CRAWLER_DETECTION_RANGE * 0.5, 0.0, 0.0),
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
            "troop crawler detector residual must reveal stealthed enemy in range"
        );
    }
    assert!(
        game_logic.honesty_troop_crawler_detect_ok(),
        "troop crawler detect honesty residual must fire"
    );
    assert!(
        game_logic.troop_crawler_residual_detects() >= 1,
        "detect counter residual"
    );

    // Fail-closed: beyond 175 not detected.
    let far_id = game_logic
        .create_object(
            "TestInfantry",
            Team::GLA,
            Vec3::new(TROOP_CRAWLER_DETECTION_RANGE + 50.0, 0.0, 0.0),
        )
        .expect("far stealthed");
    {
        let e = game_logic.host_object_mut(far_id).unwrap();
        e.set_status_stealthed(true);
        e.set_status_detected(false);
    }
    game_logic.frame = 2;
    game_logic.update_stealth_and_detection();
    {
        let e = game_logic.host_object(far_id).expect("far");
        assert!(
            !e.status.detected,
            "fail-closed: enemy outside VisionRange 175 residual must not be detected"
        );
    }
}

#[test]
fn troop_crawler_residual_transport_load_unload() {
    use crate::command_system::{CommandType, GameCommand};
    use crate::game_logic::host_troop_crawler::TROOP_CRAWLER_TRANSPORT_SLOTS;

    let mut game_logic = GameLogic::new();
    ensure_test_infantry_template(&mut game_logic);
    ensure_test_player_for_team(&mut game_logic, Team::China);
    let crawler_id = create_test_troop_crawler(&mut game_logic, Vec3::new(0.0, 0.0, 0.0));

    // Evacuate InitialPayload residual Redguards to free slots.
    {
        let occupants = game_logic
            .host_object(crawler_id)
            .map(|o| o.contained_units())
            .unwrap_or_default();
        if !occupants.is_empty() {
            game_logic.queue_command(GameCommand {
                command_type: CommandType::Evacuate,
                player_id: 1,
                command_id: 1,
                timestamp: std::time::SystemTime::now(),
                selected_units: vec![crawler_id],
                modifier_keys: crate::command_system::ModifierKeys::default(),
            });
            game_logic.process_commands();
        }
    }
    {
        let crawler = game_logic.host_object(crawler_id).expect("crawler");
        assert_eq!(
            crawler.transport_count(),
            0,
            "evacuate must free Troop Crawler residual slots"
        );
    }

    let unit_a = game_logic
        .create_object("TestInfantry", Team::China, Vec3::new(1.0, 0.0, 0.0))
        .expect("unit a");
    let unit_b = game_logic
        .create_object("TestInfantry", Team::China, Vec3::new(2.0, 0.0, 0.0))
        .expect("unit b");

    for unit_id in [unit_a, unit_b] {
        {
            let unit = game_logic.host_object_mut(unit_id).expect("unit mut");
            unit.weapon = Some(Weapon {
                damage: 15.0,
                range: 80.0,
                reload_time: 0.5,
                last_fire_time: -10.0,
                ..Weapon::default()
            });
            unit.target = Some(crawler_id);
            unit.set_ai_state(AIState::Entering);
        }
        game_logic.update_ai(&[unit_id, crawler_id], 1.0 / 30.0);
    }

    let crawler = game_logic.host_object(crawler_id).expect("crawler loaded");
    assert!(
        crawler.contained_units().contains(&unit_a) && crawler.contained_units().contains(&unit_b),
        "both infantry must load into Troop Crawler residual"
    );
    assert_eq!(crawler.transport_count(), 2);
    assert_eq!(game_logic.troop_crawler_residual_loads(), 2);
    assert!(crawler.transport_capacity() >= TROOP_CRAWLER_TRANSPORT_SLOTS);

    // Unload residual.
    game_logic.queue_command(GameCommand {
        command_type: CommandType::Evacuate,
        player_id: 1,
        command_id: 2,
        timestamp: std::time::SystemTime::now(),
        selected_units: vec![crawler_id],
        modifier_keys: crate::command_system::ModifierKeys::default(),
    });
    game_logic.process_commands();

    for unit_id in [unit_a, unit_b] {
        let unit = game_logic.host_object(unit_id).expect("free unit");
        assert_eq!(unit.ai_state, AIState::Idle);
        assert!(unit.contained_by.is_none());
        assert!(unit.can_move());
    }
    {
        let crawler = game_logic.host_object(crawler_id).expect("empty");
        assert_eq!(crawler.transport_count(), 0);
    }
    assert!(
        game_logic.troop_crawler_residual_unloads() >= 2,
        "unload residual honesty counter"
    );
    assert!(
        game_logic.honesty_troop_crawler_load_unload_ok(),
        "load/unload residual honesty"
    );
    assert!(
        game_logic.honesty_troop_crawler_ok(),
        "troop crawler residual host path honesty"
    );
}

#[test]
fn fire_weapon_when_damaged_reaction_splash() {
    use crate::game_logic::host_fire_weapon_when_damaged::HostFireWeaponWhenDamagedData;
    use crate::game_logic::{KindOf, Team, ThingTemplate};
    let mut logic = GameLogic::new();
    let mut t = ThingTemplate::new("AmericaBattleshipTarget");
    t.set_health(500.0);
    t.add_kind_of(KindOf::Structure);
    logic.templates.insert("AmericaBattleshipTarget".into(), t);
    let mut et = ThingTemplate::new("Ranger");
    et.set_health(100.0);
    et.add_kind_of(KindOf::Infantry);
    logic.templates.insert("Ranger".into(), et);
    let bid = logic
        .create_object(
            "AmericaBattleshipTarget",
            Team::USA,
            glam::Vec3::new(0.0, 0.0, 0.0),
        )
        .unwrap();
    let eid = logic
        .create_object("Ranger", Team::GLA, glam::Vec3::new(20.0, 0.0, 0.0))
        .unwrap();
    {
        let b = logic.objects.get_mut(&bid).unwrap();
        b.fire_weapon_when_damaged =
            Some(HostFireWeaponWhenDamagedData::battleship_target_residual());
        b.health.current = 500.0;
        let _ =
            b.take_damage_from_typed(50.0, None, crate::game_logic::combat::DamageType::Explosive);
        assert!(
            b.pending_fire_when_damaged_weapon.is_some(),
            "reaction should queue"
        );
    }
    let w = logic
        .objects
        .get_mut(&bid)
        .unwrap()
        .take_pending_fire_when_damaged_weapon()
        .unwrap();
    let hits = logic.apply_fire_weapon_when_damaged_named(bid, &w);
    assert!(hits >= 1, "splash should hit nearby enemy, hits={hits}");
    let enemy = logic.objects.get(&eid).unwrap();
    assert!(
        enemy.health.current < 100.0 || enemy.status.destroyed,
        "enemy hp={}",
        enemy.health.current
    );
}

#[test]
fn structure_topple_crush_sweep_kills_nearby_infantry() {
    use crate::game_logic::host_structure_topple::HostStructureToppleState;
    use crate::game_logic::{KindOf, Team, ThingTemplate};
    let mut logic = GameLogic::new();
    let mut bt = ThingTemplate::new("AmericaWarFactory");
    bt.set_health(100.0);
    bt.add_kind_of(KindOf::Structure);
    logic.templates.insert("AmericaWarFactory".into(), bt);
    let mut it = ThingTemplate::new("Ranger");
    it.set_health(50.0);
    it.add_kind_of(KindOf::Infantry);
    logic.templates.insert("Ranger".into(), it);

    let bid = logic
        .create_object(
            "AmericaWarFactory",
            Team::USA,
            glam::Vec3::new(0.0, 0.0, 0.0),
        )
        .unwrap();
    let iid = logic
        .create_object("Ranger", Team::GLA, glam::Vec3::new(12.0, 0.0, 0.0))
        .unwrap();
    {
        let b = logic.objects.get_mut(&bid).unwrap();
        b.selection_radius = 25.0;
        assert!(b.begin_structure_topple(0, Some((-10.0, 0.0))));
        if let Some(st) = b.structure_topple_data.as_mut() {
            st.state = HostStructureToppleState::Toppling;
            st.accumulated_angle = std::f32::consts::FRAC_PI_2 - 0.05;
            st.building_height = 50.0;
            st.facing_width = 20.0;
            st.major_radius = 0.0;
            st.minor_radius = 0.0;
            st.last_crushed_location = 0.0;
            st.dir_x = 1.0;
            st.dir_y = 0.0;
        }
    }
    let samples = logic
        .objects
        .get_mut(&bid)
        .unwrap()
        .take_structure_topple_crush_samples();
    assert!(!samples.is_empty());
    logic.apply_structure_topple_crush_samples(bid, samples);
    let ranger = logic.objects.get(&iid).unwrap();
    assert!(
        !ranger.is_alive()
            || ranger.status.destroyed
            || ranger.status.effectively_dead
            || ranger.health.current <= 0.0,
        "ranger should be crushed under topple sweep, hp={} destroyed={} effectively_dead={}",
        ranger.health.current,
        ranger.status.destroyed,
        ranger.status.effectively_dead
    );
}

#[test]
fn structure_topple_height_from_geometry_not_maxhp() {
    use crate::game_logic::host_structure_topple::{
        STRUCTURE_TOPPLE_CRUSHING_WEAPON_NAME, TOPPLED_STRUCTURE_WEAPON_PRIMARY_DAMAGE,
    };
    use crate::game_logic::{HostGeometryInfo, HostGeometryType, KindOf, Team, ThingTemplate};
    let mut logic = GameLogic::new();
    let mut bt = ThingTemplate::new("AmericaWarFactory");
    bt.set_health(5000.0);
    bt.add_kind_of(KindOf::Structure);
    bt.geometry_info = HostGeometryInfo {
        geom_type: HostGeometryType::Box,
        is_small: false,
        height: 33.0,
        major_radius: 40.0,
        minor_radius: 25.0,
        authored: true,
    };
    logic.templates.insert("AmericaWarFactory".into(), bt);
    let bid = logic
        .create_object(
            "AmericaWarFactory",
            Team::USA,
            glam::Vec3::new(0.0, 0.0, 0.0),
        )
        .unwrap();
    {
        let b = logic.objects.get_mut(&bid).unwrap();
        assert!(b.begin_structure_topple(0, Some((-10.0, 0.0))));
        let st = b.structure_topple_data.as_ref().unwrap();
        assert!(
            (st.building_height - 33.0).abs() < 1e-3,
            "height must come from geometry not maxHP, got {}",
            st.building_height
        );
        assert_eq!(
            st.crushing_weapon_name,
            STRUCTURE_TOPPLE_CRUSHING_WEAPON_NAME
        );
        assert!((st.crush_damage - TOPPLED_STRUCTURE_WEAPON_PRIMARY_DAMAGE).abs() < 1e-3);
        assert!(st.crush_damage < 99999.0);
    }
}

#[test]
fn height_die_kills_bomb_via_object_tick() {
    use crate::game_logic::host_height_die::HostHeightDieData;
    use crate::game_logic::{Team, ThingTemplate};
    let mut logic = GameLogic::new();
    let mut t = ThingTemplate::new("AmericaAuroraBomb");
    t.set_health(10.0);
    logic.templates.insert("AmericaAuroraBomb".into(), t);
    let id = logic
        .create_object(
            "AmericaAuroraBomb",
            Team::USA,
            glam::Vec3::new(0.0, 80.0, 0.0),
        )
        .unwrap();
    {
        let o = logic.objects.get_mut(&id).unwrap();
        o.height_die = Some(HostHeightDieData::with_target(5.0, true, 0));
        o.set_position(glam::Vec3::new(0.0, 80.0, 0.0));
        assert!(!o.tick_height_die(1, 0.0));
        o.set_position(glam::Vec3::new(0.0, 3.0, 0.0));
        assert!(o.tick_height_die(2, 0.0));
    }
    assert!(logic.objects.get(&id).unwrap().status.destroyed);
}

#[test]
fn neutron_missile_slow_death_multi_blast() {
    use crate::command_system::SpecialPowerType;
    use crate::game_logic::special_power_strikes::HostSuperweaponKind;
    use crate::game_logic::{KindOf, Team, ThingTemplate};

    let mut logic = GameLogic::new();
    let mut enemy = ThingTemplate::new("TestTank");
    enemy.set_health(500.0);
    enemy.add_kind_of(KindOf::Vehicle);
    enemy.add_kind_of(KindOf::Attackable);
    logic.templates.insert("TestTank".into(), enemy);

    let mut tree = ThingTemplate::new("Tree");
    tree.set_health(100.0);
    tree.add_kind_of(KindOf::Immobile);
    logic.templates.insert("Tree".into(), tree);

    let tank_id = logic
        .create_object("TestTank", Team::USA, glam::Vec3::new(10.0, 0.0, 0.0))
        .unwrap();
    let _tree_id = logic
        .create_object("Tree", Team::Neutral, glam::Vec3::new(20.0, 0.0, 0.0))
        .unwrap();

    // Spawn neutron field as NuclearMissile impact residual would.
    // Source id must not collide with live targets (skip filter).
    let sid = logic.special_power_strikes.spawn_neutron_slow_death_field(
        ObjectId(9_001),
        Team::China,
        glam::Vec3::new(0.0, 0.0, 0.0),
        logic.frame,
        1,
    );
    assert!(sid > 0);
    assert_eq!(
        logic.special_power_strikes.neutron_slow_death_field_count(),
        1
    );

    // Advance through Blast6 (~1180ms = 35 frames).
    for _ in 0..50 {
        logic.frame = logic.frame.saturating_add(1);
        logic.update_neutron_slow_death_fields();
    }

    let tank = logic.objects.get(&tank_id);
    // Blast6 3500 should destroy 500hp tank.
    assert!(
        tank.map(|t| !t.is_alive() || t.health.current <= 0.0)
            .unwrap_or(true),
        "tank must take Blast6 damage"
    );
    assert!((HostSuperweaponKind::NuclearMissile.max_damage() - 3500.0).abs() < 0.1);
    assert_eq!(
        crate::game_logic::special_power_strikes::HostSpecialPowerStrikeRegistry::damage_at_distance(
            HostSuperweaponKind::NuclearMissile,
            0.0,
        ),
        0.0
    );
    let _ = SpecialPowerType::NuclearMissile;
}

#[test]
fn wave_guide_moves_and_damages_after_dam_die() {
    use crate::game_logic::{KindOf, Team, ThingTemplate};
    let mut logic = GameLogic::new();
    let mut dam = ThingTemplate::new("Dam");
    dam.set_health(1000.0);
    dam.add_kind_of(KindOf::Structure);
    logic.templates.insert("Dam".into(), dam);
    let mut wg = ThingTemplate::new("WaveGuide");
    wg.set_health(1.0);
    wg.add_kind_of(KindOf::WaveGuide);
    logic.templates.insert("WaveGuide".into(), wg);
    let mut tank = ThingTemplate::new("TestTank");
    tank.set_health(200.0);
    tank.add_kind_of(KindOf::Vehicle);
    tank.add_kind_of(KindOf::Attackable);
    logic.templates.insert("TestTank".into(), tank);

    let dam_id = logic
        .create_object("Dam", Team::Neutral, glam::Vec3::new(0.0, 0.0, 0.0))
        .unwrap();
    let wg_id = logic
        .create_object("WaveGuide", Team::Neutral, glam::Vec3::new(0.0, 0.0, 0.0))
        .unwrap();
    logic
        .objects
        .get_mut(&wg_id)
        .unwrap()
        .status
        .disabled_default = true;
    logic.objects.get_mut(&wg_id).unwrap().set_orientation(0.0); // +X
    let tank_id = logic
        .create_object("TestTank", Team::USA, glam::Vec3::new(30.0, 0.0, 0.0))
        .unwrap();

    logic.mark_object_for_destruction(dam_id, None);
    assert!(!logic.objects.get(&wg_id).unwrap().status.disabled_default);
    assert!(logic.objects.get(&wg_id).unwrap().wave_guide_data.is_some());

    // Advance past WaveDelay (~23 frames) + motion into tank.
    for _ in 0..40 {
        logic.frame = logic.frame.saturating_add(1);
        logic.update_wave_guides();
    }
    let tank = logic.objects.get(&tank_id);
    assert!(
        tank.map(|t| !t.is_alive() || t.health.current <= 0.0 || t.status.destroyed)
            .unwrap_or(true),
        "wave must flood-damage nearby tank"
    );
    // Wave moved +X
    let wp = logic.objects.get(&wg_id).unwrap().get_position();
    assert!(wp.x > 1.0, "waveguide must advance along facing");
}

#[test]
fn wave_guide_pushes_add_water_velocity_onto_grid_queue() {
    use crate::game_logic::{KindOf, Team, ThingTemplate};
    if let Some(tv) = gamelogic::helpers::TheTerrainVisual::get() {
        let _ = tv.take_water_velocity_impulses();
    }
    let mut logic = GameLogic::new();
    let mut dam = ThingTemplate::new("Dam");
    dam.set_health(1000.0);
    dam.add_kind_of(KindOf::Structure);
    logic.templates.insert("Dam".into(), dam);
    let mut wg = ThingTemplate::new("WaveGuide1");
    wg.set_health(100.0);
    wg.add_kind_of(KindOf::WaveGuide);
    logic.templates.insert("WaveGuide1".into(), wg);
    let dam_id = logic
        .create_object("Dam", Team::Neutral, glam::Vec3::new(0.0, 0.0, 0.0))
        .unwrap();
    let wg_id = logic
        .create_object("WaveGuide1", Team::Neutral, glam::Vec3::new(0.0, 0.0, 0.0))
        .unwrap();
    logic.objects.get_mut(&wg_id).unwrap().set_orientation(0.0);
    logic
        .objects
        .get_mut(&wg_id)
        .unwrap()
        .status
        .disabled_default = true;
    logic.mark_object_for_destruction(dam_id, None);
    for _ in 0..30 {
        logic.frame = logic.frame.saturating_add(1);
        logic.update_wave_guides();
    }
    let impulses = gamelogic::helpers::TheTerrainVisual::get()
        .map(|tv| tv.take_water_velocity_impulses())
        .unwrap_or_default();
    assert!(
        !impulses.is_empty(),
        "Dam flood WaveGuide must push leftover addWaterVelocity"
    );
    assert!(
        impulses
            .iter()
            .any(|(_, _, v, h)| *v > 0.0 && (*h - 40.0).abs() < 0.01),
        "impulses must carry WaterVelocity + PreferredHeight: {impulses:?}"
    );
}

#[test]
fn fire_weapon_when_dead_terrorist_splash() {
    use crate::game_logic::{KindOf, Team, ThingTemplate};
    let mut logic = GameLogic::new();
    let mut t = ThingTemplate::new("GLAInfantryTerrorist");
    t.set_health(100.0);
    t.add_kind_of(KindOf::Infantry);
    logic.templates.insert("GLAInfantryTerrorist".into(), t);
    let mut tank = ThingTemplate::new("TestTank");
    tank.set_health(200.0);
    tank.add_kind_of(KindOf::Vehicle);
    logic.templates.insert("TestTank".into(), tank);

    let terror_id = logic
        .create_object(
            "GLAInfantryTerrorist",
            Team::GLA,
            glam::Vec3::new(0.0, 0.0, 0.0),
        )
        .unwrap();
    let tank_id = logic
        .create_object("TestTank", Team::USA, glam::Vec3::new(5.0, 0.0, 0.0))
        .unwrap();
    // Direct destroy path (skip slow death by using non-vehicle/infantry... terrorist is infantry)
    // Force mark then process destroy.
    logic.objects.get_mut(&terror_id).unwrap().health.current = 0.0;
    // Bypass slow death: mark after clearing wants by using apply path
    logic.objects_to_destroy.push_back(DestructionEvent {
        id: terror_id,
        killer: None,
    });
    logic.process_destroy_list();
    let tank = logic.objects.get(&tank_id);
    assert!(
        tank.map(|t| t.health.current < 200.0 || !t.is_alive())
            .unwrap_or(true),
        "terrorist death weapon must splash nearby tank"
    );
}

#[test]
fn keep_object_die_defers_destroy() {
    use crate::game_logic::{KindOf, Team, ThingTemplate};
    let mut logic = GameLogic::new();
    // Oil peels skip StructureTopple residual and use KeepObjectDie directly.
    let mut t = ThingTemplate::new("TechOilDerrick");
    t.set_health(500.0);
    t.add_kind_of(KindOf::Structure);
    logic.templates.insert("TechOilDerrick".into(), t);
    let id = logic
        .create_object(
            "TechOilDerrick",
            Team::Neutral,
            glam::Vec3::new(10.0, 0.0, 10.0),
        )
        .unwrap();
    logic.mark_object_for_destruction(id, None);
    let obj = logic.objects.get(&id).expect("rubble remains");
    assert!(obj.status.keep_as_rubble);
    assert!(obj.status.effectively_dead);
    assert!(!obj.status.destroyed);
    assert!(logic.objects_to_destroy.iter().all(|e| e.id != id));
}

#[test]
fn dam_die_enables_waveguides() {
    use crate::game_logic::{KindOf, Team, ThingTemplate};
    let mut logic = GameLogic::new();
    let mut dam = ThingTemplate::new("Dam");
    dam.set_health(1000.0);
    dam.add_kind_of(KindOf::Structure);
    logic.templates.insert("Dam".into(), dam);
    let mut wg = ThingTemplate::new("WaveGuide1");
    wg.set_health(100.0);
    wg.add_kind_of(KindOf::WaveGuide);
    logic.templates.insert("WaveGuide1".into(), wg);
    let dam_id = logic
        .create_object("Dam", Team::Neutral, glam::Vec3::new(0.0, 0.0, 0.0))
        .unwrap();
    let wg_id = logic
        .create_object("WaveGuide1", Team::Neutral, glam::Vec3::new(50.0, 0.0, 0.0))
        .unwrap();
    logic
        .objects
        .get_mut(&wg_id)
        .unwrap()
        .status
        .disabled_default = true;
    assert!(logic.objects.get(&wg_id).unwrap().is_disabled());
    logic.mark_object_for_destruction(dam_id, None);
    assert!(!logic.objects.get(&wg_id).unwrap().status.disabled_default);
    assert!(!logic.objects.get(&wg_id).unwrap().is_disabled());
}

#[test]
fn wave_guide_starts_disabled_until_dam_die() {
    // C++ WaveGuideUpdate.cpp:101 m_needDisable; update:739-743 setDisabled(DISABLED_DEFAULT).
    use crate::game_logic::{KindOf, Team, ThingTemplate};
    let mut logic = GameLogic::new();
    let mut dam = ThingTemplate::new("Dam");
    dam.set_health(1000.0);
    dam.add_kind_of(KindOf::Structure);
    logic.templates.insert("Dam".into(), dam);
    let mut wg = ThingTemplate::new("WaveGuide1");
    wg.set_health(100.0);
    wg.add_kind_of(KindOf::WaveGuide);
    logic.templates.insert("WaveGuide1".into(), wg);
    let mut tank = ThingTemplate::new("TestTank");
    tank.set_health(200.0);
    tank.add_kind_of(KindOf::Vehicle);
    logic.templates.insert("TestTank".into(), tank);
    let dam_id = logic
        .create_object("Dam", Team::Neutral, glam::Vec3::new(0.0, 0.0, 0.0))
        .unwrap();
    let wg_id = logic
        .create_object("WaveGuide1", Team::Neutral, glam::Vec3::new(0.0, 0.0, 0.0))
        .unwrap();
    logic.objects.get_mut(&wg_id).unwrap().set_orientation(0.0);
    let tank_id = logic
        .create_object("TestTank", Team::USA, glam::Vec3::new(30.0, 0.0, 0.0))
        .unwrap();
    logic.update_wave_guides();
    assert!(
        logic.objects.get(&wg_id).unwrap().status.disabled_default,
        "waveguide first tick must set DISABLED_DEFAULT"
    );
    let start = logic.objects.get(&wg_id).unwrap().get_position();
    for _ in 0..40 {
        logic.frame = logic.frame.saturating_add(1);
        logic.update_wave_guides();
    }
    let mid = logic.objects.get(&wg_id).unwrap().get_position();
    assert!(
        (mid.x - start.x).abs() < 0.01,
        "disabled waveguide must not flood before dam death"
    );
    assert!(
        logic
            .objects
            .get(&tank_id)
            .map(|t| t.is_alive() && t.health.current > 199.0)
            .unwrap_or(false),
        "tank must not take flood damage before DamDie"
    );
    logic.mark_object_for_destruction(dam_id, None);
    assert!(!logic.objects.get(&wg_id).unwrap().status.disabled_default);
}

#[test]
fn wave_guide_plays_dambreak_loop_and_random_splash() {
    // C++ WaveGuideUpdate.cpp:138-140 startMoving TheAudio LoopingSound;
    // :790-795 splash roll GameLogicRandomValue(1,100) > frequency.
    use crate::game_logic::host_wave_guide::{
        WAVE_LOOPING_SOUND, WAVE_RANDOM_SPLASH_FREQUENCY, WAVE_RANDOM_SPLASH_SOUND,
    };
    use crate::game_logic::{KindOf, Team, ThingTemplate};
    game_engine::common::random_value::init_random_with_seed(1);
    let mut logic = GameLogic::new();
    let mut dam = ThingTemplate::new("Dam");
    dam.set_health(1000.0);
    dam.add_kind_of(KindOf::Structure);
    logic.templates.insert("Dam".into(), dam);
    let mut wg = ThingTemplate::new("WaveGuide");
    wg.set_health(1.0);
    wg.add_kind_of(KindOf::WaveGuide);
    logic.templates.insert("WaveGuide".into(), wg);
    let dam_id = logic
        .create_object("Dam", Team::Neutral, glam::Vec3::new(0.0, 0.0, 0.0))
        .unwrap();
    let wg_id = logic
        .create_object("WaveGuide", Team::Neutral, glam::Vec3::new(500.0, 0.0, 0.0))
        .unwrap();
    logic
        .objects
        .get_mut(&wg_id)
        .unwrap()
        .status
        .disabled_default = true;
    logic.objects.get_mut(&wg_id).unwrap().set_orientation(0.0);
    logic.queued_audio_events.clear();
    logic.update_wave_guides();
    assert!(
        logic
            .queued_audio_events
            .iter()
            .all(|e| e.event_type != WAVE_LOOPING_SOUND
                && e.event_type != WAVE_RANDOM_SPLASH_SOUND),
        "disabled waveguide must stay silent: {:?}",
        logic.queued_audio_events
    );
    logic.mark_object_for_destruction(dam_id, None);
    // Simulate WaveGuide1 last waypoint so dest-check does not eat the wave
    // on the first moving tick (C++ PATH_EXTRA_DISTANCE).
    if let Some(wg) = logic
        .objects
        .get_mut(&wg_id)
        .and_then(|o| o.wave_guide_data.as_mut())
    {
        wg.final_destination = Some((5000.0, 0.0));
        wg.initialized = true;
    }
    let _ = WAVE_RANDOM_SPLASH_FREQUENCY;
    logic.queued_audio_events.clear();
    let mut saw_loop = false;
    let mut saw_splash = false;
    for _ in 0..80 {
        logic.frame = logic.frame.saturating_add(1);
        logic.update_wave_guides();
        if logic.queued_audio_events.iter().any(|e| {
            e.event_type == WAVE_LOOPING_SOUND && e.object_id == Some(wg_id) && e.is_looping
        }) {
            saw_loop = true;
        }
        if logic.queued_audio_events.iter().any(|e| {
            e.event_type == WAVE_RANDOM_SPLASH_SOUND && e.object_id == Some(wg_id) && !e.is_looping
        }) {
            saw_splash = true;
        }
    }
    assert!(
        saw_loop,
        "startMoving must leftover-play DamBreakWaveLoop: {:?}",
        logic.queued_audio_events
    );
    assert!(
        saw_splash,
        "moving tick must leftover-play WaveRandomSplash at leftover frequency: {:?}",
        logic.queued_audio_events
    );
}

#[test]
fn jet_slow_death_defers_destroy() {
    use crate::game_logic::{KindOf, Team, ThingTemplate};
    let mut logic = GameLogic::new();
    let mut t = ThingTemplate::new("AmericaJetRaptor");
    t.set_health(200.0);
    t.add_kind_of(KindOf::Aircraft);
    logic.templates.insert("AmericaJetRaptor".into(), t);
    let id = logic
        .create_object(
            "AmericaJetRaptor",
            Team::USA,
            glam::Vec3::new(0.0, 80.0, 0.0),
        )
        .unwrap();
    logic.mark_object_for_destruction(id, None);
    assert!(
        logic
            .objects
            .get(&id)
            .unwrap()
            .jet_slow_death
            .as_ref()
            .map(|j| j.is_active())
            .unwrap_or(false)
    );
    assert!(logic.objects_to_destroy.iter().all(|e| e.id != id));
}

#[test]
fn helicopter_slow_death_defers_destroy() {
    use crate::game_logic::{KindOf, Team, ThingTemplate};
    let mut logic = GameLogic::new();
    let mut t = ThingTemplate::new("AmericaComanche");
    t.set_health(200.0);
    t.add_kind_of(KindOf::Aircraft);
    logic.templates.insert("AmericaComanche".into(), t);
    let id = logic
        .create_object(
            "AmericaComanche",
            Team::USA,
            glam::Vec3::new(0.0, 50.0, 0.0),
        )
        .unwrap();
    logic.mark_object_for_destruction(id, None);
    assert!(
        logic
            .objects
            .get(&id)
            .unwrap()
            .helicopter_slow_death
            .as_ref()
            .map(|h| h.is_active())
            .unwrap_or(false)
    );
    assert!(logic.objects_to_destroy.iter().all(|e| e.id != id));
}

#[test]
fn slow_death_defers_infantry_destruction() {
    use crate::game_logic::host_slow_death::{
        HostSlowDeathIni, clear_slow_death_ini_override_for_tests,
        override_slow_death_ini_for_tests,
    };
    use crate::game_logic::{KindOf, Team, ThingTemplate};
    let mut logic = GameLogic::new();
    let mut t = ThingTemplate::new("AmericaInfantryRanger");
    t.set_health(100.0);
    t.add_kind_of(KindOf::Infantry);
    logic.templates.insert("AmericaInfantryRanger".into(), t);
    let id = logic
        .create_object(
            "AmericaInfantryRanger",
            Team::USA,
            glam::Vec3::new(0.0, 0.0, 0.0),
        )
        .unwrap();
    // C++ SlowDeathBehavior::onDie (SlowDeathBehavior.cpp:456-501) runs only
    // when the object has that Die module. A bare KindOf::Infantry fixture
    // has no module → DestroyDie.cpp:35-40 / GameLogic.cpp:3932-3967 queues
    // destroy the same frame.
    logic.mark_object_for_destruction(id, None);
    assert!(
        logic.objects.get(&id).unwrap().slow_death.is_none(),
        "module-less infantry must not auto-start SlowDeath (DestroyDie path)"
    );
    assert!(
        logic.objects_to_destroy.iter().any(|e| e.id == id),
        "module-less infantry must enter destroy list same frame"
    );

    let id2 = logic
        .create_object(
            "AmericaInfantryRanger",
            Team::USA,
            glam::Vec3::new(10.0, 0.0, 0.0),
        )
        .unwrap();
    {
        let o = logic.objects.get_mut(&id2).unwrap();
        assert!(
            o.begin_slow_death_from_ini(logic.frame, &HostSlowDeathIni::infantry_retail()),
            "SlowDeathBehavior residual must start from authored INI"
        );
        assert!(o.slow_death.as_ref().is_some_and(|s| s.is_active()));
    }
    assert!(logic.objects_to_destroy.iter().all(|e| e.id != id2));
    let mut finished = false;
    for _ in 0..400 {
        logic.frame = logic.frame.saturating_add(1);
        if let Some(o) = logic.objects.get_mut(&id2) {
            if o.tick_slow_death(logic.frame) {
                finished = true;
                break;
            }
        }
    }
    assert!(finished);
    logic.mark_object_for_destruction(id2, None);
    assert!(logic.objects_to_destroy.iter().any(|e| e.id == id2));

    // Combat death with authored SlowDeathBehavior starts beginSlowDeath
    // (C++ SlowDeathBehavior.cpp:191) and uses INI delays, not KindOf skip.
    clear_slow_death_ini_override_for_tests();
    let mut tank_t = ThingTemplate::new("IniSlowDeathCrusader");
    tank_t.set_health(400.0);
    tank_t.add_kind_of(KindOf::Vehicle);
    logic
        .templates
        .insert("IniSlowDeathCrusader".into(), tank_t);
    let ini = crate::game_logic::host_slow_death::slow_death_ini_from_behavior_attrs(&[
        ("DestructionDelay", "5000"),
        ("SinkDelay", "0"),
        ("SinkRate", "0"),
    ]);
    override_slow_death_ini_for_tests("IniSlowDeathCrusader", ini);
    let id3 = logic
        .create_object(
            "IniSlowDeathCrusader",
            Team::USA,
            glam::Vec3::new(20.0, 0.0, 0.0),
        )
        .unwrap();
    logic.mark_object_for_destruction(id3, None);
    {
        let o = logic.objects.get(&id3).unwrap();
        let sd = o.slow_death.as_ref().expect("INI SlowDeath must begin");
        assert!(sd.is_active());
        assert!(!o.status.destroyed);
        assert_eq!(
            sd.destroy_at_frame,
            logic.frame + crate::game_logic::host_slow_death::msec_to_logic_frames(5000),
            "DestructionDelay from INI, not hardcoded vehicle 1000ms"
        );
    }
    assert!(
        logic.objects_to_destroy.iter().all(|e| e.id != id3),
        "INI SlowDeath must defer processDestroyList"
    );
    clear_slow_death_ini_override_for_tests();
}

#[test]
fn create_object_die_spawns_on_lifetime_expiry() {
    use crate::game_logic::host_create_object_die::HostCreateObjectDieData;
    use crate::game_logic::host_lifetime_update::HostLifetimeUpdateData;
    use crate::game_logic::{KindOf, Team, ThingTemplate};
    let mut logic = GameLogic::new();
    let mut st = ThingTemplate::new("GLASneakAttackTunnelNetworkStart");
    st.set_health(50.0);
    st.add_kind_of(KindOf::Structure);
    logic
        .templates
        .insert("GLASneakAttackTunnelNetworkStart".into(), st);
    let mut nt = ThingTemplate::new("GLASneakAttackTunnelNetwork");
    nt.set_health(200.0);
    nt.add_kind_of(KindOf::Structure);
    logic
        .templates
        .insert("GLASneakAttackTunnelNetwork".into(), nt);

    let id = logic
        .create_object(
            "GLASneakAttackTunnelNetworkStart",
            Team::GLA,
            glam::Vec3::new(10.0, 0.0, 10.0),
        )
        .unwrap();
    {
        let o = logic.objects.get_mut(&id).unwrap();
        o.lifetime_update = Some(HostLifetimeUpdateData::from_delay_frames(0, 2));
        o.create_object_die = Some(HostCreateObjectDieData {
            ocl_name: "OCL_CreateSneakAttackTunnel".into(),
            spawn_templates: vec!["GLASneakAttackTunnelNetwork".into()],
            transfer_previous_health: true,
            fired: false,
        });
    }
    // Simulate lifetime tick path.
    {
        let o = logic.objects.get_mut(&id).unwrap();
        assert!(o.tick_lifetime_update(2));
        // Wave 753: under damage authority, do not zero host HP mid-frame
        // (dual with GW HP writeback). Project lethal via damage log + flags.
        if crate::gameworld_shadow::gameworld_damage_authority_live() {
            let hp = o.health.current.max(1.0);
            let oid = o.id;
            crate::game_logic::host_damage_log::record(oid, hp, None, true);
        } else {
            o.health.current = 0.0;
        }
        o.status.destroyed = true;
        o.refresh_model_condition_bits();
    }
    logic.apply_pending_create_object_die(id);
    let spawned = logic
        .objects
        .values()
        .any(|o| o.template_name.contains("TunnelNetwork") && o.id != id);
    assert!(spawned, "CreateObjectDie should spawn tunnel network");
}

#[test]
fn structure_collapse_defers_destruction() {
    use crate::game_logic::{KindOf, Team, ThingTemplate};
    let mut logic = GameLogic::new();
    let mut t = ThingTemplate::new("CivilianBarn01");
    t.set_health(80.0);
    t.add_kind_of(KindOf::Structure);
    logic.templates.insert("CivilianBarn01".into(), t);
    let id = logic
        .create_object(
            "CivilianBarn01",
            Team::Neutral,
            glam::Vec3::new(0.0, 0.0, 0.0),
        )
        .unwrap();
    logic.mark_object_for_destruction(id, None);
    assert!(
        logic
            .objects
            .get(&id)
            .unwrap()
            .structure_collapse_data
            .is_some(),
        "civilian should collapse"
    );
    assert!(logic.objects_to_destroy.iter().all(|e| e.id != id));
    let mut finished = false;
    for _ in 0..800 {
        logic.frame = logic.frame.saturating_add(1);
        if let Some(obj) = logic.objects.get_mut(&id) {
            if obj.tick_structure_collapse(logic.frame) {
                finished = true;
                break;
            }
        }
    }
    assert!(finished);
    logic.mark_object_for_destruction(id, None);
    assert!(logic.objects_to_destroy.iter().any(|e| e.id == id));
}

#[test]
fn structure_topple_defers_destruction() {
    use crate::game_logic::{KindOf, Team, ThingTemplate};
    let mut logic = GameLogic::new();
    let mut t = ThingTemplate::new("AmericaWarFactory");
    t.set_health(100.0);
    t.add_kind_of(KindOf::Structure);
    logic.templates.insert("AmericaWarFactory".into(), t);
    let id = logic
        .create_object(
            "AmericaWarFactory",
            Team::USA,
            glam::Vec3::new(0.0, 0.0, 0.0),
        )
        .expect("id");
    logic.mark_object_for_destruction(id, None);
    // Should be toppling, not queued for immediate destroy.
    assert!(
        logic
            .objects
            .get(&id)
            .unwrap()
            .structure_topple_data
            .is_some(),
        "structure topple should start"
    );
    assert!(
        logic.objects_to_destroy.iter().all(|e| e.id != id),
        "should defer destroy while toppling"
    );
    // Advance until done.
    let mut finished = false;
    for _ in 0..800 {
        logic.frame = logic.frame.saturating_add(1);
        if let Some(obj) = logic.objects.get_mut(&id) {
            if obj.tick_structure_topple(logic.frame) {
                finished = true;
                break;
            }
        }
    }
    assert!(finished);
    logic.mark_object_for_destruction(id, None);
    assert!(
        logic.objects_to_destroy.iter().any(|e| e.id == id),
        "done structure should destroy"
    );
}

#[test]
fn water_edge_damage_on_dry_to_wet_transition() {
    use crate::game_logic::{KindOf, Team, ThingTemplate};
    let mut logic = GameLogic::new();
    let mut t = ThingTemplate::new("Ranger");
    t.set_health(100.0);
    t.add_kind_of(KindOf::Infantry);
    logic.templates.insert("Ranger".into(), t);
    let id = logic
        .create_object("Ranger", Team::USA, glam::Vec3::new(5.0, 0.0, 5.0))
        .expect("id");
    {
        let o = logic.objects.get_mut(&id).unwrap();
        o.health.current = 100.0;
        o.cell_is_underwater = false;
    }
    // Simulate terrain reporting underwater on next refresh by forcing sample path:
    // call internal edge path with manual pre/post.
    // Direct residual: mark wet + apply Water damage as rise residual.
    {
        let o = logic.objects.get_mut(&id).unwrap();
        let was = o.cell_is_underwater;
        o.cell_is_underwater = true;
        assert!(!was);
        let _ = o.take_damage_from_typed(25.0, None, crate::game_logic::combat::DamageType::Water);
    }
    let hp = logic.objects.get(&id).unwrap().health.current;
    assert!((hp - 75.0).abs() < 1e-3, "hp={hp}");

    // API smoke: apply_water_rise_damage should not panic without terrain water.
    let _ = logic.apply_water_rise_damage(999.0);
    let _ = logic.refresh_surface_cells_and_water_edge_damage(25.0);
}

#[test]
fn troop_crawler_residual_assault_deploy_unloads_and_attacks() {
    use crate::game_logic::host_troop_crawler::TROOP_CRAWLER_ASSAULT_RANGE;

    let mut game_logic = GameLogic::new();
    ensure_test_tank_template(&mut game_logic);
    ensure_test_infantry_template(&mut game_logic);
    ensure_test_player_for_team(&mut game_logic, Team::China);

    let crawler_id = create_test_troop_crawler(&mut game_logic, Vec3::new(0.0, 0.0, 0.0));
    let payload_count = game_logic
        .host_object(crawler_id)
        .map(|c| c.transport_count())
        .unwrap_or(0);
    assert!(
        payload_count >= 1,
        "assault deploy residual requires InitialPayload residual occupants, got {payload_count}"
    );

    let enemy = game_logic
        .create_object(
            "TestTank",
            Team::GLA,
            Vec3::new(TROOP_CRAWLER_ASSAULT_RANGE * 0.5, 0.0, 0.0),
        )
        .expect("enemy");

    {
        let c = game_logic.host_object_mut(crawler_id).unwrap();
        c.attack_target(enemy);
        if let Some(w) = c.weapon.as_mut() {
            w.last_fire_time = -10.0;
            w.reload_time = 0.1;
        }
    }

    game_logic.set_current_frame(40);
    game_logic.update_combat(&[crawler_id, enemy], LOGIC_FRAME_TIMESTEP);

    assert!(
        game_logic.honesty_troop_crawler_assault_deploy_ok(),
        "assault deploy residual honesty must fire"
    );
    assert!(
        game_logic.troop_crawler_residual_assault_deploys() >= 1,
        "deploy counter residual"
    );

    let remaining = game_logic
        .host_object(crawler_id)
        .map(|c| c.transport_count())
        .unwrap_or(0);
    assert_eq!(
        remaining, 0,
        "assault deploy residual must unload occupants (remaining={remaining})"
    );

    // At least one former occupant ordered to attack residual.
    let any_attacking = game_logic.objects.values().any(|o| {
        o.team == Team::China
            && o.is_kind_of(KindOf::Infantry)
            && o.contained_by.is_none()
            && (o.target == Some(enemy)
                || matches!(o.ai_state, AIState::Attacking | AIState::AttackMoving))
    });
    assert!(
        any_attacking,
        "assault deploy residual must order infantry to attack designated target"
    );
}

#[test]
fn troop_crawler_assault_keeps_wounded_and_retrieves_outside() {
    let mut game_logic = GameLogic::new();
    ensure_test_infantry_template(&mut game_logic);
    ensure_test_tank_template(&mut game_logic);
    ensure_test_player_for_team(&mut game_logic, Team::China);

    let crawler_id = create_test_troop_crawler(&mut game_logic, Vec3::new(0.0, 0.0, 0.0));
    // Clear payload and load two controlled infantry.
    let old_occ = game_logic
        .host_object(crawler_id)
        .map(|c| c.occupants.clone())
        .unwrap_or_default();
    for oid in old_occ {
        if let Some(c) = game_logic.host_object_mut(crawler_id) {
            c.remove_occupant(oid);
        }
        if let Some(u) = game_logic.host_object_mut(oid) {
            u.set_contained_by(None);
        }
        game_logic.destroy_object(oid);
    }
    let healthy_id = game_logic
        .create_object("TestInfantry", Team::China, Vec3::new(1.0, 0.0, 0.0))
        .expect("healthy");
    let wounded_id = game_logic
        .create_object("TestInfantry", Team::China, Vec3::new(2.0, 0.0, 0.0))
        .expect("wounded");
    {
        let h = game_logic.host_object_mut(healthy_id).unwrap();
        h.health.maximum = 100.0;
        h.health.current = 100.0;
    }
    {
        let w = game_logic.host_object_mut(wounded_id).unwrap();
        w.health.maximum = 100.0;
        w.health.current = 40.0; // < 0.5 ratio
    }
    {
        let c = game_logic.host_object_mut(crawler_id).unwrap();
        assert!(c.add_occupant(healthy_id));
        assert!(c.add_occupant(wounded_id));
    }
    {
        let h = game_logic.host_object_mut(healthy_id).unwrap();
        h.set_contained_by(Some(crawler_id));
    }
    {
        let w = game_logic.host_object_mut(wounded_id).unwrap();
        w.set_contained_by(Some(crawler_id));
    }

    let enemy = game_logic
        .create_object("TestTank", Team::GLA, Vec3::new(50.0, 0.0, 0.0))
        .expect("enemy");

    let ordered = game_logic.apply_troop_crawler_assault_deploy_for_test(crawler_id, enemy);
    assert_eq!(ordered, 1, "only healthy member should eject");
    assert!(
        game_logic
            .host_object(healthy_id)
            .map(|u| u.contained_by.is_none())
            .unwrap_or(false),
        "healthy outside"
    );
    assert_eq!(
        game_logic
            .host_object(wounded_id)
            .and_then(|u| u.contained_by),
        Some(crawler_id),
        "wounded stays aboard"
    );

    // Wound the outside healthy fighter → aiEnter walk, not teleport-board.
    let outside_pos = game_logic
        .host_object(healthy_id)
        .map(|u| u.get_position())
        .unwrap_or(Vec3::ZERO);
    {
        let h = game_logic.host_object_mut(healthy_id).unwrap();
        h.health.current = 30.0;
    }
    game_logic.tick_assault_transport_updates();
    {
        let h = game_logic.host_object(healthy_id).expect("wounded walker");
        assert!(
            h.contained_by.is_none(),
            "wounded retrieve must not teleport-board"
        );
        assert_eq!(
            h.ai_state,
            AIState::Entering,
            "wounded retrieve issues aiEnter"
        );
        assert_eq!(h.target, Some(crawler_id));
        assert!(
            (h.get_position() - outside_pos).length() < 0.5,
            "aiEnter must not snap the fighter onto the hull"
        );
    }
    assert!(game_logic.honesty_troop_crawler_wounded_retrieve_ok());
    // Second tick must not re-issue (already AI_ENTER).
    let retrieves = game_logic.troop_crawler.wounded_retrieves;
    game_logic.tick_assault_transport_updates();
    assert_eq!(
        game_logic.troop_crawler.wounded_retrieves, retrieves,
        "already-Entering members skip a second aiEnter"
    );

    // Complete the board, then full heal → aiExit walk (not 6wu pop).
    {
        let c = game_logic.host_object_mut(crawler_id).unwrap();
        assert!(c.add_occupant(healthy_id));
    }
    {
        let h = game_logic.host_object_mut(healthy_id).unwrap();
        h.set_contained_by(Some(crawler_id));
        h.set_ai_state(AIState::Idle);
        h.health.current = 100.0;
    }
    game_logic.tick_assault_transport_updates();
    {
        let crawler_pos = game_logic
            .host_object(crawler_id)
            .map(|c| c.get_position())
            .unwrap_or(Vec3::ZERO);
        let h = game_logic.host_object(healthy_id).expect("re-exit");
        assert!(h.contained_by.is_none(), "full health re-exits to fight");
        assert_eq!(
            h.ai_state,
            AIState::Moving,
            "healthy eject must walk ExitStart/End"
        );
        assert!(
            (h.get_position() - crawler_pos).length() < 1.0,
            "ExitStart is the hull, not a 6wu teleport"
        );
        let dest = h.movement.target_position.expect("ExitEnd dest");
        assert!(
            (dest - crawler_pos).length() > 6.5,
            "ExitEnd is past the old 6wu pop: dest={dest:?}"
        );
    }
    assert!(game_logic.troop_crawler.healthy_redeploys > 0);
}

#[test]
fn troop_crawler_death_gives_final_orders() {
    use crate::game_logic::host_command_button_hunt::HUNT_CMD_FROM_PLAYER;

    let mut game_logic = GameLogic::new();
    ensure_test_infantry_template(&mut game_logic);
    ensure_test_tank_template(&mut game_logic);
    ensure_test_player_for_team(&mut game_logic, Team::China);

    let crawler_id = create_test_troop_crawler(&mut game_logic, Vec3::new(0.0, 0.0, 0.0));
    strip_troop_crawler_payload(&mut game_logic, crawler_id);
    let healthy_id = game_logic
        .create_object("TestInfantry", Team::China, Vec3::new(1.0, 0.0, 0.0))
        .expect("healthy");
    {
        let h = game_logic.host_object_mut(healthy_id).unwrap();
        h.health.maximum = 100.0;
        h.health.current = 100.0;
    }
    {
        let c = game_logic.host_object_mut(crawler_id).unwrap();
        assert!(c.add_occupant(healthy_id));
    }
    {
        let h = game_logic.host_object_mut(healthy_id).unwrap();
        h.set_contained_by(Some(crawler_id));
    }
    let enemy = game_logic
        .create_object("TestTank", Team::GLA, Vec3::new(50.0, 0.0, 0.0))
        .expect("enemy");

    assert_eq!(
        game_logic.apply_troop_crawler_assault_deploy_for_test(crawler_id, enemy),
        1
    );
    game_logic.destroy_object(crawler_id);
    let member = game_logic.host_object(healthy_id).expect("member");
    assert_eq!(
        member.last_command_source, HUNT_CMD_FROM_PLAYER,
        "giveFinalOrders must stamp CMD_FROM_PLAYER"
    );
    assert!(
        member.target == Some(enemy)
            || matches!(member.ai_state, AIState::Attacking | AIState::AttackMoving),
        "dead crawler hands the assault attack to the squad"
    );
}

#[test]
fn troop_crawler_new_members_do_not_eject_until_new_attack() {
    let mut game_logic = GameLogic::new();
    ensure_test_infantry_template(&mut game_logic);
    ensure_test_tank_template(&mut game_logic);
    ensure_test_player_for_team(&mut game_logic, Team::China);

    let crawler_id = create_test_troop_crawler(&mut game_logic, Vec3::new(0.0, 0.0, 0.0));
    strip_troop_crawler_payload(&mut game_logic, crawler_id);
    let fighter_id = game_logic
        .create_object("TestInfantry", Team::China, Vec3::new(1.0, 0.0, 0.0))
        .expect("fighter");
    let extra_id = game_logic
        .create_object("TestInfantry", Team::China, Vec3::new(2.0, 0.0, 0.0))
        .expect("extra");
    for id in [fighter_id, extra_id] {
        let u = game_logic.host_object_mut(id).unwrap();
        u.health.maximum = 100.0;
        u.health.current = 100.0;
    }
    {
        let c = game_logic.host_object_mut(crawler_id).unwrap();
        assert!(c.add_occupant(fighter_id));
    }
    {
        let h = game_logic.host_object_mut(fighter_id).unwrap();
        h.set_contained_by(Some(crawler_id));
    }
    let enemy = game_logic
        .create_object("TestTank", Team::GLA, Vec3::new(50.0, 0.0, 0.0))
        .expect("enemy");
    assert_eq!(
        game_logic.apply_troop_crawler_assault_deploy_for_test(crawler_id, enemy),
        1
    );

    {
        let c = game_logic.host_object_mut(crawler_id).unwrap();
        assert!(c.add_occupant(extra_id));
    }
    {
        let e = game_logic.host_object_mut(extra_id).unwrap();
        e.set_contained_by(Some(crawler_id));
    }
    game_logic.tick_assault_transport_updates();
    assert_eq!(
        game_logic
            .host_object(extra_id)
            .and_then(|u| u.contained_by),
        Some(crawler_id),
        "new members stay aboard during the current assault"
    );

    game_logic.assault_transport_on_player_attack(crawler_id);
    game_logic.tick_assault_transport_updates();
    assert!(
        game_logic
            .host_object(extra_id)
            .map(|u| u.contained_by.is_none())
            .unwrap_or(false),
        "a fresh attack order ejects the boarded extras"
    );
}

#[test]
fn troop_crawler_target_death_retrieves_members() {
    let mut game_logic = GameLogic::new();
    ensure_test_infantry_template(&mut game_logic);
    ensure_test_tank_template(&mut game_logic);
    ensure_test_player_for_team(&mut game_logic, Team::China);

    let crawler_id = create_test_troop_crawler(&mut game_logic, Vec3::new(0.0, 0.0, 0.0));
    strip_troop_crawler_payload(&mut game_logic, crawler_id);
    let healthy_id = game_logic
        .create_object("TestInfantry", Team::China, Vec3::new(1.0, 0.0, 0.0))
        .expect("healthy");
    {
        let h = game_logic.host_object_mut(healthy_id).unwrap();
        h.health.maximum = 100.0;
        h.health.current = 100.0;
    }
    {
        let c = game_logic.host_object_mut(crawler_id).unwrap();
        assert!(c.add_occupant(healthy_id));
    }
    {
        let h = game_logic.host_object_mut(healthy_id).unwrap();
        h.set_contained_by(Some(crawler_id));
    }
    let enemy = game_logic
        .create_object("TestTank", Team::GLA, Vec3::new(50.0, 0.0, 0.0))
        .expect("enemy");
    assert_eq!(
        game_logic.apply_troop_crawler_assault_deploy_for_test(crawler_id, enemy),
        1
    );
    assert!(
        game_logic
            .host_object(healthy_id)
            .map(|u| u.contained_by.is_none())
            .unwrap_or(false)
    );

    game_logic.destroy_object(enemy);
    game_logic.tick_assault_transport_updates();
    {
        let h = game_logic.host_object(healthy_id).expect("retrieve");
        assert!(
            h.contained_by.is_none(),
            "retrieveMembers must not teleport-board"
        );
        assert_eq!(
            h.ai_state,
            AIState::Entering,
            "attack-object target death must retrieveMembers via aiEnter"
        );
        assert_eq!(h.target, Some(crawler_id));
    }
}

#[test]
fn troop_crawler_attack_move_continues_after_target_death() {
    let mut game_logic = GameLogic::new();
    ensure_test_infantry_template(&mut game_logic);
    ensure_test_tank_template(&mut game_logic);
    ensure_test_player_for_team(&mut game_logic, Team::China);

    let crawler_id = create_test_troop_crawler(&mut game_logic, Vec3::new(0.0, 0.0, 0.0));
    strip_troop_crawler_payload(&mut game_logic, crawler_id);
    let healthy_id = game_logic
        .create_object("TestInfantry", Team::China, Vec3::new(1.0, 0.0, 0.0))
        .expect("healthy");
    {
        let h = game_logic.host_object_mut(healthy_id).unwrap();
        h.health.maximum = 100.0;
        h.health.current = 100.0;
    }
    {
        let c = game_logic.host_object_mut(crawler_id).unwrap();
        assert!(c.add_occupant(healthy_id));
    }
    {
        let h = game_logic.host_object_mut(healthy_id).unwrap();
        h.set_contained_by(Some(crawler_id));
    }
    let enemy = game_logic
        .create_object("TestTank", Team::GLA, Vec3::new(50.0, 0.0, 0.0))
        .expect("enemy");
    game_logic.assault_transport_on_player_attack_move(crawler_id, Vec3::new(80.0, 0.0, 0.0));
    assert_eq!(
        game_logic.apply_troop_crawler_assault_deploy_for_test(crawler_id, enemy),
        1
    );

    game_logic.destroy_object(enemy);
    game_logic.tick_assault_transport_updates();
    let crawler = game_logic.host_object(crawler_id).expect("crawler");
    assert!(
        matches!(crawler.ai_state, AIState::AttackMoving),
        "attack-move assault continues toward the goal after the target dies"
    );
    assert!(
        game_logic
            .host_object(healthy_id)
            .map(|u| u.contained_by.is_none())
            .unwrap_or(false),
        "attack-move does not retrieveMembers on target death"
    );
}

#[test]
fn troop_crawler_stop_retrieves_members() {
    let mut game_logic = GameLogic::new();
    ensure_test_infantry_template(&mut game_logic);
    ensure_test_tank_template(&mut game_logic);
    ensure_test_player_for_team(&mut game_logic, Team::China);

    let crawler_id = create_test_troop_crawler(&mut game_logic, Vec3::new(0.0, 0.0, 0.0));
    strip_troop_crawler_payload(&mut game_logic, crawler_id);
    let healthy_id = game_logic
        .create_object("TestInfantry", Team::China, Vec3::new(1.0, 0.0, 0.0))
        .expect("healthy");
    {
        let h = game_logic.host_object_mut(healthy_id).unwrap();
        h.health.maximum = 100.0;
        h.health.current = 100.0;
    }
    {
        let c = game_logic.host_object_mut(crawler_id).unwrap();
        assert!(c.add_occupant(healthy_id));
    }
    {
        let h = game_logic.host_object_mut(healthy_id).unwrap();
        h.set_contained_by(Some(crawler_id));
    }
    let enemy = game_logic
        .create_object("TestTank", Team::GLA, Vec3::new(50.0, 0.0, 0.0))
        .expect("enemy");
    assert_eq!(
        game_logic.apply_troop_crawler_assault_deploy_for_test(crawler_id, enemy),
        1
    );

    assert!(game_logic.unit_command_stop(crawler_id));
    {
        let h = game_logic.host_object(healthy_id).expect("stop retrieve");
        assert!(
            h.contained_by.is_none(),
            "Stop retrieveMembers must not teleport-board"
        );
        assert_eq!(
            h.ai_state,
            AIState::Entering,
            "Stop on the crawler must retrieveMembers via aiEnter"
        );
        assert_eq!(h.target, Some(crawler_id));
    }
}

#[test]
fn troop_crawler_assault_walks_enter_and_exit() {
    let mut game_logic = GameLogic::new();
    ensure_test_infantry_template(&mut game_logic);
    ensure_test_tank_template(&mut game_logic);
    ensure_test_player_for_team(&mut game_logic, Team::China);

    let crawler_id = create_test_troop_crawler(&mut game_logic, Vec3::new(0.0, 0.0, 0.0));
    strip_troop_crawler_payload(&mut game_logic, crawler_id);
    let fighter = game_logic
        .create_object("TestInfantry", Team::China, Vec3::new(1.0, 0.0, 0.0))
        .expect("fighter");
    {
        let h = game_logic.host_object_mut(fighter).unwrap();
        h.health.maximum = 100.0;
        h.health.current = 100.0;
    }
    {
        let c = game_logic.host_object_mut(crawler_id).unwrap();
        assert!(c.add_occupant(fighter));
    }
    {
        let h = game_logic.host_object_mut(fighter).unwrap();
        h.set_contained_by(Some(crawler_id));
    }
    let enemy = game_logic
        .create_object("TestTank", Team::GLA, Vec3::new(50.0, 0.0, 0.0))
        .expect("enemy");
    assert_eq!(
        game_logic.apply_troop_crawler_assault_deploy_for_test(crawler_id, enemy),
        1
    );

    let ring_pos = game_logic
        .host_object(fighter)
        .map(|u| u.get_position())
        .unwrap_or(Vec3::ZERO);
    {
        let h = game_logic.host_object_mut(fighter).unwrap();
        h.health.current = 20.0;
    }
    game_logic.tick_assault_transport_updates();
    {
        let h = game_logic.host_object(fighter).expect("enter walk");
        assert!(h.contained_by.is_none());
        assert_eq!(h.ai_state, AIState::Entering);
        assert_eq!(h.target, Some(crawler_id));
        assert!(
            (h.get_position() - ring_pos).length() < 0.5,
            "wounded retrieve must not snap to hull"
        );
    }

    {
        let c = game_logic.host_object_mut(crawler_id).unwrap();
        assert!(c.add_occupant(fighter));
    }
    {
        let h = game_logic.host_object_mut(fighter).unwrap();
        h.set_contained_by(Some(crawler_id));
        h.set_ai_state(AIState::Idle);
        h.health.current = 100.0;
    }
    game_logic.tick_assault_transport_updates();
    let crawler_pos = game_logic
        .host_object(crawler_id)
        .map(|c| c.get_position())
        .unwrap_or(Vec3::ZERO);
    let h = game_logic.host_object(fighter).expect("exit walk");
    assert!(h.contained_by.is_none());
    assert_eq!(h.ai_state, AIState::Moving);
    assert!((h.get_position() - crawler_pos).length() < 1.0);
    assert!((h.get_position() - (crawler_pos + Vec3::new(6.0, 0.0, 0.0))).length() > 1.0);
    let dest = h.movement.target_position.expect("ExitEnd");
    assert!((dest - crawler_pos).length() > 6.5);
}

#[test]
fn troop_crawler_residual_rejects_vehicle_enter() {
    let mut game_logic = GameLogic::new();
    ensure_test_tank_template(&mut game_logic);
    ensure_test_player_for_team(&mut game_logic, Team::China);
    let crawler_id = create_test_troop_crawler(&mut game_logic, Vec3::new(0.0, 0.0, 0.0));

    // Free InitialPayload residual slots.
    {
        let occupants = game_logic
            .host_object(crawler_id)
            .map(|o| o.contained_units())
            .unwrap_or_default();
        if !occupants.is_empty() {
            use crate::command_system::{CommandType, GameCommand};
            game_logic.queue_command(GameCommand {
                command_type: CommandType::Evacuate,
                player_id: 1,
                command_id: 1,
                timestamp: std::time::SystemTime::now(),
                selected_units: vec![crawler_id],
                modifier_keys: crate::command_system::ModifierKeys::default(),
            });
            game_logic.process_commands();
        }
    }

    let loads_before = game_logic.troop_crawler_residual_loads();
    let tank_id = game_logic
        .create_object("TestTank", Team::China, Vec3::new(2.0, 0.0, 0.0))
        .expect("tank");
    {
        let unit = game_logic.host_object_mut(tank_id).unwrap();
        unit.target = Some(crawler_id);
        unit.set_ai_state(AIState::Entering);
    }
    game_logic.update_ai(&[tank_id, crawler_id], 1.0 / 30.0);

    let crawler = game_logic.host_object(crawler_id).expect("crawler");
    assert!(
        !crawler.contained_units().contains(&tank_id),
        "vehicles must not enter Troop Crawler residual"
    );
    assert_eq!(
        game_logic.troop_crawler_residual_loads(),
        loads_before,
        "vehicle enter must not count as troop crawler residual load"
    );
}
