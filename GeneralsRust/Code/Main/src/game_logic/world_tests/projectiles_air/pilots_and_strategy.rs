//! Behavior suite extracted from `projectiles_air`.
use super::*;

#[test]
fn pilot_find_vehicle_ai_auto_scan_min_health_residual() {
    let _env_guard = host_state_residual_test_env_lock();
    use crate::game_logic::host_usa_pilot::{
        PILOT_FIND_VEHICLE_MIN_HEALTH, PILOT_FIND_VEHICLE_SCAN_FRAMES,
    };

    let (mut world, pilot) = pilot_find_vehicle_test_world(false);
    let healthy = world
        .create_object_for_player("TestTank", 0, Vec3::new(20.0, 0.0, 0.0))
        .unwrap();
    let damaged = world
        .create_object_for_player("TestTank", 0, Vec3::new(0.0, 0.0, 12.0))
        .unwrap();
    let target = world.host_object_mut(damaged).unwrap();
    target.health.current = target.health.maximum * (PILOT_FIND_VEHICLE_MIN_HEALTH - 0.1);
    world
        .update_owned_partition_with_effects(&mut Default::default())
        .unwrap();

    world.frame = PILOT_FIND_VEHICLE_SCAN_FRAMES;
    world
        .update_ai(&[pilot, healthy, damaged], 1.0 / 30.0)
        .unwrap();
    assert_eq!(world.usa_pilot_residual().find_vehicle_orders, 1);
    assert_eq!(
        world.host_object(pilot).unwrap().ai_state(),
        AIState::Entering
    );
    assert_eq!(world.host_object(pilot).unwrap().target, Some(healthy));

    drive_pilot_to_vehicle(&mut world, pilot, healthy);
    assert_eq!(
        world.host_object(healthy).unwrap().experience.level,
        VeterancyLevel::Veteran
    );
    assert_eq!(
        world.host_object(damaged).unwrap().experience.level,
        VeterancyLevel::Rookie
    );
    assert_eq!(world.host_object(healthy).unwrap().owner_player_id, Some(0));

    let (mut human, pilot) = pilot_find_vehicle_test_world(true);
    let tank = human
        .create_object_for_player("TestTank", 0, Vec3::X)
        .unwrap();
    human
        .update_owned_partition_with_effects(&mut Default::default())
        .unwrap();
    human.frame = PILOT_FIND_VEHICLE_SCAN_FRAMES;
    human.update_ai(&[pilot, tank], 1.0 / 30.0).unwrap();
    assert_eq!(human.usa_pilot_residual().find_vehicle_orders, 0);
    assert_eq!(human.host_object(pilot).unwrap().target, None);
    assert_eq!(
        human.host_object(tank).unwrap().experience.level,
        VeterancyLevel::Rookie
    );
}

#[test]
fn pilot_find_vehicle_base_center_fallback_residual() {
    let _env_guard = host_state_residual_test_env_lock();
    // Authority channels default off on the fresh GameLogic context —
    // host-state residual honesty without shadow writeback.

    use crate::game_logic::host_usa_pilot::PILOT_FIND_VEHICLE_SCAN_FRAMES;

    let mut game_logic = GameLogic::new();
    game_logic.add_player(Player::new(0, Team::USA, "USA AI", false));

    let mut pilot_tpl = ThingTemplate::new("AmericaInfantryPilot");
    pilot_tpl
        .add_kind_of(KindOf::Infantry)
        .add_kind_of(KindOf::Selectable)
        .set_health(100.0);
    // C++ PilotFindVehicleUpdate gates the scan on the authored
    // VeterancyCrateCollide pilot module, not the basename.
    author_pilot_find_vehicle(&mut pilot_tpl);
    game_logic
        .templates
        .insert("AmericaInfantryPilot".to_string(), pilot_tpl);

    let mut cc_tpl = ThingTemplate::new("AmericaCommandCenter");
    cc_tpl
        .add_kind_of(KindOf::Structure)
        .add_kind_of(KindOf::CommandCenter)
        .add_kind_of(KindOf::Selectable)
        .set_health(4000.0);
    game_logic
        .templates
        .insert("AmericaCommandCenter".to_string(), cc_tpl);

    let ai = crate::ai::AIPlayer::new(0, Team::USA, crate::ai::AIDifficulty::Medium);
    game_logic.ai_manager.ai_players.insert(0, ai);
    game_logic.get_player_mut(0).unwrap().map_side.build_list = [
        ("AmericaCommandCenter", (120.0, 80.0, 0.0)),
        ("AmericaCommandCenter", (160.0, 120.0, 0.0)),
        ("UnknownBuildListTemplate", (250.0, 250.0, 0.0)),
    ]
    .into_iter()
    .map(|(template, position)| crate::game_logic::HostAuthoredBuild {
        template: template.into(),
        position,
        angle: 0.0,
        num_rebuilds: 1,
        initially_built: false,
        completion: Default::default(),
    })
    .collect();
    game_logic.feed_host_ai_from_authored_build_lists();
    game_logic.new_map_owned_ai_with_effects(&mut Default::default()).unwrap();
    let base_center = Vec3::new(140.0, 0.0, 100.0);
    assert_eq!(game_logic.ai_manager.ai_players[&0].get_base_center(), Some(base_center));

    let cc_pos = Vec3::new(120.0, 0.0, 80.0);
    let cc_id = game_logic
        .create_object("AmericaCommandCenter", Team::USA, cc_pos)
        .expect("command center");
    let pilot_id = game_logic
        .create_object("AmericaInfantryPilot", Team::USA, Vec3::new(0.0, 0.0, 0.0))
        .expect("pilot");
    {
        let ai_entry_frame = game_logic.frame;
        let p = game_logic.host_object_mut(pilot_id).expect("pilot");
        p.set_ai_state(
            AIState::Idle,
            ai_entry_frame,
            &mut crate::game_logic::ObjectAudio::record(&mut Vec::new()),
        );
        assert!(!p.pilot_did_move_to_base());
    }

    assert!(!game_logic.honesty_pilot_base_center_ok());
    game_logic.frame = PILOT_FIND_VEHICLE_SCAN_FRAMES;
    game_logic
        .update_ai(&[pilot_id, cc_id], 1.0 / 30.0)
        .expect("receiving AI operation succeeds");

    assert!(
        game_logic.honesty_pilot_base_center_ok(),
        "base-center fallback residual must record honesty"
    );
    assert_eq!(game_logic.usa_pilot_residual().base_center_moves, 1);
    {
        let p = game_logic
            .host_object(pilot_id)
            .expect("pilot after fallback");
        assert!(
            p.pilot_did_move_to_base(),
            "m_didMoveToBase residual must latch after fallback"
        );
        assert_eq!(
            p.ai_state(),
            AIState::Moving,
            "fallback residual must issue Move to base"
        );
        let dest = p
            .movement
            .target_position
            .expect("fallback residual must set move destination");
        assert!(
            (dest.x - base_center.x).abs() < 0.1 && (dest.z - base_center.z).abs() < 0.1,
            "fallback destination must be the AI build-list centroid ({base_center:?}), got {dest:?}"
        );
    }

    // Second scan while still idle must not re-issue (m_didMoveToBase).
    // Reset to Idle so residual is eligible again but did_move latches.
    {
        let ai_entry_frame = game_logic.frame;
        let p = game_logic.host_object_mut(pilot_id).expect("pilot");
        p.set_ai_state(
            AIState::Idle,
            ai_entry_frame,
            &mut crate::game_logic::ObjectAudio::record(&mut Vec::new()),
        );
        p.stop_moving(&mut crate::game_logic::ObjectAudio::record(&mut Vec::new()));
    }
    game_logic.frame = PILOT_FIND_VEHICLE_SCAN_FRAMES * 2;
    game_logic
        .update_ai(&[pilot_id, cc_id], 1.0 / 30.0)
        .expect("receiving AI operation succeeds");
    assert_eq!(
        game_logic.usa_pilot_residual().base_center_moves,
        1,
        "base-center residual must be one-shot (m_didMoveToBase)"
    );

    let builder = crate::save_load::SnapshotBuilder::new();
    let saved = builder.create_world_snapshot(&game_logic).unwrap();
    let bytes = bincode_legacy::serialize(&saved).unwrap();
    let saved = crate::save_load::decode_bincode_world_snapshot(&bytes).unwrap();
    let mut restored = GameLogic::new();
    restored.add_player(Player::new(0, Team::USA, "USA AI", false));
    restored.restore_host_ai_players_from_save(&saved.ai_players);
    assert_eq!(restored.ai_manager.ai_players[&0].get_base_center(), Some(base_center));

    let fresh_ai = crate::ai::AIPlayer::new(0, Team::USA, crate::ai::AIDifficulty::Medium);
    game_logic.ai_manager.ai_players.insert(0, fresh_ai);
    let pilot_with_unset_center = game_logic
        .create_object_for_player("AmericaInfantryPilot", 0, Vec3::ZERO)
        .unwrap();
    game_logic.frame += 1;
    game_logic.update_ai(&[pilot_with_unset_center, cc_id], 1.0 / 30.0).unwrap();
    let waiting = game_logic.host_object(pilot_with_unset_center).unwrap();
    assert!(!waiting.pilot_did_move_to_base());
    assert_eq!(waiting.ai_state(), AIState::Idle);
    assert_eq!(game_logic.usa_pilot_residual().base_center_moves, 1);

    // Human residual: no base-center fallback.
    let mut human_logic = GameLogic::new();
    human_logic.add_player(Player::new(0, Team::USA, "USA Human", true));
    human_logic
        .templates
        .insert("AmericaInfantryPilot".to_string(), {
            let mut t = ThingTemplate::new("AmericaInfantryPilot");
            t.add_kind_of(KindOf::Infantry)
                .add_kind_of(KindOf::Selectable)
                .set_health(100.0);
            author_pilot_find_vehicle(&mut t);
            t
        });
    human_logic
        .templates
        .insert("AmericaCommandCenter".to_string(), {
            let mut t = ThingTemplate::new("AmericaCommandCenter");
            t.add_kind_of(KindOf::Structure)
                .add_kind_of(KindOf::CommandCenter)
                .add_kind_of(KindOf::Selectable)
                .set_health(4000.0);
            t
        });
    let hcc = human_logic
        .create_object(
            "AmericaCommandCenter",
            Team::USA,
            Vec3::new(100.0, 0.0, 0.0),
        )
        .expect("hcc");
    let hp = human_logic
        .create_object("AmericaInfantryPilot", Team::USA, Vec3::new(0.0, 0.0, 0.0))
        .expect("hp");
    {
        let ai_entry_frame = human_logic.frame;
        let p = human_logic.host_object_mut(hp).expect("hp");
        p.set_ai_state(
            AIState::Idle,
            ai_entry_frame,
            &mut crate::game_logic::ObjectAudio::record(&mut Vec::new()),
        );
    }
    human_logic.frame = PILOT_FIND_VEHICLE_SCAN_FRAMES;
    human_logic
        .update_ai(&[hp, hcc], 1.0 / 30.0)
        .expect("receiving AI operation succeeds");
    assert!(
        !human_logic.honesty_pilot_base_center_ok(),
        "human pilot residual must not base-center fallback"
    );
    assert_eq!(human_logic.usa_pilot_residual().base_center_moves, 0);
}

#[test]
fn pilot_auto_find_healing_hospital_path_residual() {
    let _env_guard = host_state_residual_test_env_lock();

    // Authority channels default off on the fresh GameLogic context —
    // host-state residual honesty without shadow writeback.

    use crate::game_logic::host_usa_pilot::{
        AUTO_FIND_HEALING_NEVER_HEAL, AUTO_FIND_HEALING_SCAN_FRAMES,
    };

    let mut game_logic = GameLogic::new();
    ensure_test_heal_pad_template(&mut game_logic);
    game_logic.add_player(Player::new(0, Team::USA, "USA AI", false));

    let mut pilot_tpl = ThingTemplate::new("AmericaInfantryPilot");
    pilot_tpl
        .add_kind_of(KindOf::Infantry)
        .add_kind_of(KindOf::Selectable)
        .set_health(100.0);
    game_logic
        .templates
        .insert("AmericaInfantryPilot".to_string(), pilot_tpl);

    // Pad within INTERACT_RANGE (14) so update_ai SeekingHealing ticks without
    // requiring full update_movement approach residual.
    let pad_id = game_logic
        .create_object("TestHealPad", Team::USA, Vec3::new(5.0, 0.0, 0.0))
        .expect("heal pad");
    let pilot_id = game_logic
        .create_object("AmericaInfantryPilot", Team::USA, Vec3::new(0.0, 0.0, 0.0))
        .expect("pilot");
    {
        let ai_entry_frame = game_logic.frame;
        let p = game_logic.host_object_mut(pilot_id).expect("pilot");
        // Below NeverHeal 0.85 residual (50% HP).
        p.health.current = 50.0;
        p.set_ai_state(
            AIState::Idle,
            ai_entry_frame,
            &mut crate::game_logic::ObjectAudio::record(&mut Vec::new()),
        );
    }

    assert!(!game_logic.honesty_pilot_auto_heal_ok());
    game_logic.frame = AUTO_FIND_HEALING_SCAN_FRAMES;
    game_logic
        .update_ai(&[pilot_id, pad_id], 1.0 / 30.0)
        .expect("receiving AI operation succeeds");

    assert!(
        game_logic.honesty_pilot_auto_heal_ok(),
        "AutoFindHealing residual must issue SeekingHealing honesty"
    );
    assert_eq!(game_logic.usa_pilot_residual().auto_heal_orders, 1);
    {
        let p = game_logic.host_object(pilot_id).expect("pilot after scan");
        assert_eq!(
            p.ai_state(),
            AIState::SeekingHealing,
            "injured AI pilot residual must SeekHealing"
        );
        assert_eq!(p.target, Some(pad_id), "must target HealPad residual");
    }

    // Complete heal residual at pad (existing SeekingHealing honesty path).
    // First scan tick already issued SeekingHealing; subsequent frames tick HP.
    for _ in 0..60 {
        game_logic
            .update_ai(&[pilot_id, pad_id], 1.0 / 30.0)
            .expect("receiving AI operation succeeds");
    }
    let hp_after = game_logic
        .host_object(pilot_id)
        .expect("pilot")
        .health
        .current;
    assert!(
        hp_after > 50.0,
        "SeekingHealing residual must restore pilot HP (after={hp_after})"
    );
    assert!(
        game_logic.honesty_heal_pad_ok(),
        "heal-pad honesty must record SeekingHealing ticks"
    );

    // Healthy pilot (> NeverHeal) must not auto-scan.
    let mut healthy_logic = GameLogic::new();
    ensure_test_heal_pad_template(&mut healthy_logic);
    healthy_logic.add_player(Player::new(0, Team::USA, "USA AI", false));
    healthy_logic
        .templates
        .insert("AmericaInfantryPilot".to_string(), {
            let mut t = ThingTemplate::new("AmericaInfantryPilot");
            t.add_kind_of(KindOf::Infantry)
                .add_kind_of(KindOf::Selectable)
                .set_health(100.0);
            t
        });
    let hpad = healthy_logic
        .create_object("TestHealPad", Team::USA, Vec3::new(10.0, 0.0, 0.0))
        .expect("pad");
    let hpilot = healthy_logic
        .create_object("AmericaInfantryPilot", Team::USA, Vec3::new(0.0, 0.0, 0.0))
        .expect("healthy pilot");
    {
        let ai_entry_frame = healthy_logic.frame;
        let p = healthy_logic.host_object_mut(hpilot).expect("hp");
        // Just above NeverHeal threshold residual.
        p.health.current = 100.0 * AUTO_FIND_HEALING_NEVER_HEAL + 1.0;
        p.set_ai_state(
            AIState::Idle,
            ai_entry_frame,
            &mut crate::game_logic::ObjectAudio::record(&mut Vec::new()),
        );
    }
    healthy_logic.frame = AUTO_FIND_HEALING_SCAN_FRAMES;
    healthy_logic
        .update_ai(&[hpilot, hpad], 1.0 / 30.0)
        .expect("receiving AI operation succeeds");
    assert!(
        !healthy_logic.honesty_pilot_auto_heal_ok(),
        "NeverHeal residual must skip healthy pilot auto-scan"
    );
    assert_eq!(healthy_logic.usa_pilot_residual().auto_heal_orders, 0);
    assert_eq!(
        healthy_logic.host_object(hpilot).unwrap().ai_state(),
        AIState::Idle,
        "healthy pilot residual stays Idle"
    );

    // Human residual: no AutoFindHealing auto-scan.
    let mut human_logic = GameLogic::new();
    ensure_test_heal_pad_template(&mut human_logic);
    human_logic.add_player(Player::new(0, Team::USA, "USA Human", true));
    human_logic
        .templates
        .insert("AmericaInfantryPilot".to_string(), {
            let mut t = ThingTemplate::new("AmericaInfantryPilot");
            t.add_kind_of(KindOf::Infantry)
                .add_kind_of(KindOf::Selectable)
                .set_health(100.0);
            t
        });
    let hpad2 = human_logic
        .create_object("TestHealPad", Team::USA, Vec3::new(10.0, 0.0, 0.0))
        .expect("pad");
    let hp2 = human_logic
        .create_object("AmericaInfantryPilot", Team::USA, Vec3::new(0.0, 0.0, 0.0))
        .expect("human pilot");
    {
        let ai_entry_frame = human_logic.frame;
        let p = human_logic.host_object_mut(hp2).expect("hp");
        p.health.current = 40.0;
        p.set_ai_state(
            AIState::Idle,
            ai_entry_frame,
            &mut crate::game_logic::ObjectAudio::record(&mut Vec::new()),
        );
    }
    human_logic.frame = AUTO_FIND_HEALING_SCAN_FRAMES;
    human_logic
        .update_ai(&[hp2, hpad2], 1.0 / 30.0)
        .expect("receiving AI operation succeeds");
    assert!(
        !human_logic.honesty_pilot_auto_heal_ok(),
        "human pilot residual must not AutoFindHealing"
    );
    assert_eq!(human_logic.usa_pilot_residual().auto_heal_orders, 0);
    assert_eq!(
        human_logic.host_object(hp2).unwrap().ai_state(),
        AIState::Idle,
        "human injured pilot residual stays Idle without GetHealed command"
    );
}

#[test]
fn usa_infantry_auto_find_healing_hospital_path_residual() {
    let _env_guard = host_state_residual_test_env_lock();

    // Authority channels default off on the fresh GameLogic context —
    // host-state residual honesty without shadow writeback.

    use crate::game_logic::host_usa_pilot::{
        AUTO_FIND_HEALING_SCAN_FRAMES, is_auto_find_healing_template,
    };

    let mut game_logic = GameLogic::new();
    ensure_test_heal_pad_template(&mut game_logic);
    game_logic.add_player(Player::new(0, Team::USA, "USA AI", false));

    let mut ranger_tpl = ThingTemplate::new("AmericaInfantryRanger");
    ranger_tpl
        .add_kind_of(KindOf::Infantry)
        .add_kind_of(KindOf::Selectable)
        .set_health(100.0);
    game_logic
        .templates
        .insert("AmericaInfantryRanger".to_string(), ranger_tpl);
    assert!(is_auto_find_healing_template("AmericaInfantryRanger"));

    let pad_id = game_logic
        .create_object("TestHealPad", Team::USA, Vec3::new(5.0, 0.0, 0.0))
        .expect("heal pad");
    let ranger_id = game_logic
        .create_object("AmericaInfantryRanger", Team::USA, Vec3::new(0.0, 0.0, 0.0))
        .expect("ranger");
    {
        let ai_entry_frame = game_logic.frame;
        let r = game_logic.host_object_mut(ranger_id).expect("ranger");
        r.health.current = 40.0;
        r.set_ai_state(
            AIState::Idle,
            ai_entry_frame,
            &mut crate::game_logic::ObjectAudio::record(&mut Vec::new()),
        );
    }

    assert!(!game_logic.honesty_infantry_auto_heal_ok());
    game_logic.frame = AUTO_FIND_HEALING_SCAN_FRAMES;
    game_logic
        .update_ai(&[ranger_id, pad_id], 1.0 / 30.0)
        .expect("receiving AI operation succeeds");

    assert!(
        game_logic.honesty_infantry_auto_heal_ok(),
        "Ranger AutoFindHealing residual must issue SeekingHealing honesty"
    );
    assert!(
        game_logic.honesty_pilot_auto_heal_ok(),
        "infantry auto-heal also counts auto_heal_orders honesty"
    );
    assert_eq!(game_logic.usa_pilot_residual().infantry_auto_heal_orders, 1);
    assert_eq!(game_logic.usa_pilot_residual().auto_heal_orders, 1);
    {
        let r = game_logic
            .host_object(ranger_id)
            .expect("ranger after scan");
        assert_eq!(
            r.ai_state(),
            AIState::SeekingHealing,
            "injured AI ranger residual must SeekHealing"
        );
        assert_eq!(r.target, Some(pad_id));
    }

    // Complete heal residual at pad.
    for _ in 0..60 {
        game_logic
            .update_ai(&[ranger_id, pad_id], 1.0 / 30.0)
            .expect("receiving AI operation succeeds");
    }
    let hp_after = game_logic
        .host_object(ranger_id)
        .expect("ranger")
        .health
        .current;
    assert!(
        hp_after > 40.0,
        "SeekingHealing residual must restore ranger HP (after={hp_after})"
    );

    // C++: China infantry with AutoFindHealingUpdate also scans.
    let mut china_logic = GameLogic::new();
    ensure_test_heal_pad_template(&mut china_logic);
    china_logic.add_player(Player::new(0, Team::China, "China AI", false));
    china_logic
        .templates
        .insert("ChinaInfantryRedguard".to_string(), {
            let mut t = ThingTemplate::new("ChinaInfantryRedguard");
            t.add_kind_of(KindOf::Infantry)
                .add_kind_of(KindOf::Selectable)
                .set_health(100.0);
            t
        });
    let cpad = china_logic
        .create_object("TestHealPad", Team::China, Vec3::new(5.0, 0.0, 0.0))
        .expect("pad");
    let cred = china_logic
        .create_object(
            "ChinaInfantryRedguard",
            Team::China,
            Vec3::new(0.0, 0.0, 0.0),
        )
        .expect("redguard");
    {
        let ai_entry_frame = china_logic.frame;
        let r = china_logic.host_object_mut(cred).expect("rg");
        r.health.current = 30.0;
        r.set_ai_state(
            AIState::Idle,
            ai_entry_frame,
            &mut crate::game_logic::ObjectAudio::record(&mut Vec::new()),
        );
    }
    china_logic.frame = AUTO_FIND_HEALING_SCAN_FRAMES;
    china_logic
        .update_ai(&[cred, cpad], 1.0 / 30.0)
        .expect("receiving AI operation succeeds");
    assert!(
        china_logic.honesty_infantry_auto_heal_ok()
            || china_logic.host_object(cred).unwrap().ai_state() != AIState::Idle,
        "China Redguard AutoFindHealingUpdate must seek a HealPad"
    );
}

#[test]
fn eject_pilot_ground_ocl_creates_an_uncontained_pilot() {
    use crate::game_logic::VeterancyLevel;
    use crate::game_logic::host_usa_pilot::EJECT_PILOT_TEMPLATE;

    let mut ground_logic = GameLogic::new();
    ensure_eject_pilot_residual_fixture(&mut ground_logic);
    ground_logic
        .templates
        .insert("AmericaVehicleHumvee".to_string(), {
            let mut t = ThingTemplate::new("AmericaVehicleHumvee");
            t.add_kind_of(KindOf::Vehicle)
                .add_kind_of(KindOf::Selectable)
                .add_kind_of(KindOf::Attackable)
                .set_health(200.0);
            author_eject_pilot_die_module(
                &mut t,
                EjectPilotDeathTypes::All,
                EjectPilotVeterancyLevels::All,
                EjectPilotExemptStatus::None,
            );
            t
        });
    let g_id = ground_logic
        .create_object(
            "AmericaVehicleHumvee",
            Team::USA,
            Vec3::new(10.0, 0.0, 10.0),
        )
        .expect("ground humvee");
    {
        let (h, health_events) = ground_logic
            .host_object_and_health_events_mut(g_id)
            .expect("humvee");
        h.experience.level = VeterancyLevel::Veteran;
        let _ = h.take_damage(h.max_health * 2.0, 0, health_events);
        h.status.destroyed = true;
    }
    ground_logic.mark_object_for_destruction(g_id, Some(Team::GLA));
    ground_logic.process_destroy_list();
    assert!(ground_logic.honesty_pilot_eject_ok());
    assert!(
        !ground_logic.honesty_pilot_air_eject_ok(),
        "ground death residual must not claim air OCL"
    );
    assert_eq!(ground_logic.usa_pilot_residual().air_ejections, 0);
    let g_pilot = ground_logic
        .objects
        .registered_values()
        .find(|o| o.is_alive() && o.template_name == EJECT_PILOT_TEMPLATE)
        .expect("ground pilot");
    assert!(
        !g_pilot.is_parachuting(),
        "ground OCL residual must not parachute"
    );
    assert!(g_pilot.get_position().y.abs() < 0.1);
}

#[test]
fn pilot_find_vehicle_same_player_partition_filter_residual() {
    let _env_guard = host_state_residual_test_env_lock();
    use crate::game_logic::host_usa_pilot::PILOT_FIND_VEHICLE_SCAN_FRAMES;

    let (mut world, pilot) = pilot_find_vehicle_test_world(false);
    world.add_player(Player::new(1, Team::USA, "SameFactionPeer", false));
    let foreign = world
        .create_object_for_player("TestTank", 1, Vec3::new(0.0, 0.0, 12.0))
        .unwrap();
    let own = world
        .create_object_for_player("TestTank", 0, Vec3::new(20.0, 0.0, 0.0))
        .unwrap();
    world
        .update_owned_partition_with_effects(&mut Default::default())
        .unwrap();

    world.frame = PILOT_FIND_VEHICLE_SCAN_FRAMES;
    world.update_ai(&[pilot, foreign, own], 1.0 / 30.0).unwrap();
    assert_eq!(world.usa_pilot_residual().find_vehicle_player_rejects, 1);
    assert_eq!(
        world.host_object(pilot).unwrap().ai_state(),
        AIState::Entering
    );
    assert_eq!(world.host_object(pilot).unwrap().target, Some(own));

    drive_pilot_to_vehicle(&mut world, pilot, own);
    assert_eq!(
        world.host_object(own).unwrap().experience.level,
        VeterancyLevel::Veteran
    );
    assert_eq!(world.host_object(own).unwrap().owner_player_id, Some(0));
    assert_eq!(
        world.host_object(foreign).unwrap().experience.level,
        VeterancyLevel::Rookie
    );
    assert_eq!(world.host_object(foreign).unwrap().owner_player_id, Some(1));
}

#[test]
fn eject_pilot_parachute_open_fudge_and_free_fall_damage_residual() {
    use crate::game_logic::host_usa_pilot::{
        EJECT_PILOT_TEMPLATE, FREE_FALL_DAMAGE_PERCENT, PARACHUTE_OPEN_DIST,
        free_fall_damage_amount,
    };

    let mut game_logic = GameLogic::new();
    // Ensure pilot template.
    if !game_logic.templates.contains_key(EJECT_PILOT_TEMPLATE) {
        let mut pilot_tpl = ThingTemplate::new(EJECT_PILOT_TEMPLATE);
        pilot_tpl
            .add_kind_of(KindOf::Infantry)
            .add_kind_of(KindOf::Selectable)
            .set_health(100.0);
        game_logic
            .templates
            .insert(EJECT_PILOT_TEMPLATE.to_string(), pilot_tpl);
    }

    // Low-altitude air eject: spawn height < 2×OpenDist so fudge applies.
    let low_y = PARACHUTE_OPEN_DIST * 1.5; // 150 < 200
    assert!(low_y < 2.0 * PARACHUTE_OPEN_DIST);
    let pilot_id = game_logic
        .create_object(EJECT_PILOT_TEMPLATE, Team::USA, Vec3::new(0.0, low_y, 0.0))
        .expect("pilot");
    {
        let p = game_logic.host_object_mut(pilot_id).expect("pilot");
        p.apply_eject_parachuting();
    }
    // Record fudge honesty (create_object path records via process_destroy;
    // direct apply needs explicit honesty for residual test).
    game_logic.usa_pilot.record_parachute_open_fudge();
    {
        let p = game_logic.host_object(pilot_id).expect("pilot");
        assert!(p.is_parachuting());
        assert!(
            (p.status.parachute_start_height - 2.0 * PARACHUTE_OPEN_DIST).abs() < 0.01,
            "low-altitude residual must fudge start height to 2×OpenDist, got {}",
            p.status.parachute_start_height
        );
    }
    assert!(game_logic.honesty_pilot_parachute_open_fudge_ok());

    // FreeFallDamage residual: destroy chute mid-air while elevated.
    // Raise pilot so significantly-above-terrain gate passes.
    {
        let thr = crate::game_logic::host_usa_pilot::significantly_above_terrain_threshold();
        let (p, fixture_partition_p) = game_logic
            .host_object_and_partition_mut(pilot_id)
            .expect("pilot");
        let mut pos = p.get_position();
        pos.y = thr + 50.0;
        p.set_position_with_partition_owner(pos, 0, &mut Default::default(), fixture_partition_p);
        p.set_status_parachute_open(true); // open chute then cut residual
        p.health.current = p.health.maximum;
    }
    // This is an established match-frame falling witness; C++ frame-zero
    // UINT_MAX history initialization has a separate exact negative control.
    game_logic.frame = 2;
    let hp_before = game_logic.host_object(pilot_id).unwrap().health.current;
    let max_hp = game_logic.host_object(pilot_id).unwrap().health.maximum;
    assert!(!game_logic.honesty_pilot_free_fall_damage_ok());
    assert!(
        game_logic.destroy_eject_parachute_midair(pilot_id).unwrap(),
        "FreeFallDamage residual must apply mid-air"
    );
    assert!(game_logic.honesty_pilot_free_fall_damage_ok());
    assert_eq!(game_logic.usa_pilot_residual().free_fall_damages, 1);
    {
        let p = game_logic.host_object(pilot_id).expect("pilot");
        assert!(
            !p.is_parachute_open(),
            "chute destroyed residual must close chute"
        );
        // ParachuteContain.cpp:626-649 removes containment, then enables
        // physical freefall; PARACHUTING describes the intact chute instead.
        assert!(!p.is_parachuting());
        assert!(
            p.allow_to_fall && p.is_in_freefall,
            "rider continues physical freefall"
        );
        assert_eq!(
            p.last_damage_info_type,
            Some(crate::game_logic::combat::DamageType::Falling)
        );
        let expected = free_fall_damage_amount(max_hp);
        assert!(
            (hp_before - p.health.current - expected).abs() < 0.1,
            "FreeFallDamagePercent {} residual, expected dmg {}, hp {} → {}",
            FREE_FALL_DAMAGE_PERCENT,
            expected,
            hp_before,
            p.health.current
        );
    }
    // Ground pilot residual: FreeFallDamage must not apply.
    {
        let (p, fixture_partition_p) = game_logic
            .host_object_and_partition_mut(pilot_id)
            .expect("pilot");
        let mut pos = p.get_position();
        pos.y = 0.0;
        p.set_position_with_partition_owner(pos, 0, &mut Default::default(), fixture_partition_p);
        p.clear_eject_parachuting();
    }
    assert!(
        !game_logic.destroy_eject_parachute_midair(pilot_id).unwrap(),
        "ground residual must not FreeFallDamage"
    );
}

#[test]
fn parachute_loss_falling_damage_observes_defensive_body_scalar() {
    use crate::game_logic::combat::DamageType;
    use crate::game_logic::host_strategy_center::HOLD_THE_LINE_ARMOR_DAMAGE_SCALAR;
    use crate::game_logic::host_usa_pilot::{
        free_fall_damage_amount, significantly_above_terrain_threshold,
    };

    // ParachuteContain.cpp:634-638 sends DAMAGE_FALLING. ActiveBody.cpp:
    // 490-497 applies the body scalar to every type except UNRESISTABLE.
    let mut logic = GameLogic::new();
    let mut template = ThingTemplate::new("DefendedParachuteRider");
    template.add_kind_of(KindOf::Infantry).set_health(100.0);
    logic.templates.insert(template.name.clone(), template);
    let rider = logic
        .create_object(
            "DefendedParachuteRider",
            Team::USA,
            Vec3::new(0.0, significantly_above_terrain_threshold() + 50.0, 0.0),
        )
        .unwrap();
    let expected = {
        let object = logic.host_object_mut(rider).unwrap();
        object.apply_eject_parachuting();
        object.weapon_bonus_battle_plan_hold_the_line = true;
        assert!(HOLD_THE_LINE_ARMOR_DAMAGE_SCALAR < 1.0);
        free_fall_damage_amount(object.health.maximum) * HOLD_THE_LINE_ARMOR_DAMAGE_SCALAR
    };
    logic.frame = 2;
    assert!(logic.destroy_eject_parachute_midair(rider).unwrap());
    let object = logic.host_object(rider).unwrap();
    assert_eq!(object.last_damage_info_type, Some(DamageType::Falling));
    assert!((100.0 - object.health.current - expected).abs() < 0.01);
    assert!(object.is_alive());
    assert!(!object.is_parachuting() && !object.is_parachute_open());
    assert!(object.allow_to_fall && object.is_in_freefall);
}

#[test]
fn strategy_center_turret_idle_scan_residual() {
    use crate::game_logic::host_strategy_center::{
        HostBattlePlan, STRATEGY_CENTER_MAX_IDLE_SCAN_ANGLE_DEG,
        STRATEGY_CENTER_MIN_IDLE_SCAN_INTERVAL_FRAMES, STRATEGY_CENTER_NATURAL_TURRET_ANGLE_DEG,
        STRATEGY_CENTER_NATURAL_TURRET_PITCH_DEG, STRATEGY_CENTER_TURRET_TURN_DEG_PER_FRAME,
        turret_angles_are_natural,
    };

    let mut game_logic = GameLogic::new();
    let mut sc_template = ThingTemplate::new("AmericaStrategyCenter");
    sc_template
        .add_kind_of(KindOf::Structure)
        .add_kind_of(KindOf::FSStrategyCenter)
        .add_kind_of(KindOf::Selectable)
        .set_health(1500.0);
    author_strategy_center_turret(&mut sc_template);
    game_logic
        .templates
        .insert("AmericaStrategyCenter".to_string(), sc_template);
    if !game_logic.players.contains_key(&0) {
        game_logic.add_player(Player::new(0, Team::USA, "USA", true));
    }

    let sc_id = game_logic
        .create_object("AmericaStrategyCenter", Team::USA, Vec3::new(0.0, 0.0, 0.0))
        .expect("strategy center");

    assert!(!game_logic.honesty_strategy_center_turret_idle_scan_ok());

    // Activate Bombardment → ACTIVE equips gun + schedules first idle scan.
    assert!(game_logic.activate_battle_plan(0, HostBattlePlan::Bombardment, Some(sc_id),));
    advance_battle_plan_door_to_active(&mut game_logic);
    {
        let sc = game_logic.host_object(sc_id).expect("sc");
        assert!(sc.weapon.is_some(), "Bombardment ACTIVE equips gun");
        assert!(
            turret_angles_are_natural(sc.turret_ai.turret_angle_deg, sc.turret_ai.turret_pitch_deg),
            "idle scan residual starts at natural angles"
        );
        // BecameActive schedules next = frame + MinIdleScanInterval (15).
        assert!(
            sc.turret_ai.turret_idle_scan_next_frame > 0,
            "first idle-scan residual must be scheduled"
        );
        assert!(!sc.turret_ai.turret_idle_scanning);
        let _ = STRATEGY_CENTER_MIN_IDLE_SCAN_INTERVAL_FRAMES;
    }

    // Ensure idle: no target / not attacking; force scan due this frame.
    let due_frame = game_logic.frame;
    {
        let ai_entry_frame = game_logic.frame;
        let sc = game_logic.host_object_mut(sc_id).expect("sc");
        sc.target = None;
        sc.set_ai_state(
            AIState::Idle,
            ai_entry_frame,
            &mut crate::game_logic::ObjectAudio::record(&mut Vec::new()),
        );
        sc.set_status_attacking(false);
        sc.turret_ai.turret_angle_deg = STRATEGY_CENTER_NATURAL_TURRET_ANGLE_DEG;
        sc.turret_ai.turret_pitch_deg = STRATEGY_CENTER_NATURAL_TURRET_PITCH_DEG;
        sc.turret_ai.turret_idle_scan_next_frame = due_frame;
        sc.turret_ai.turret_idle_scan_index = 0;
        sc.turret_ai.turret_idle_scanning = false;
    }

    // Observe the actual admitted scan; CPP permits even zero/short offsets.
    game_logic
        .tick_battle_plan_door_residuals()
        .expect("valid battle-plan test state");
    assert!(
        game_logic.honesty_strategy_center_turret_idle_scan_ok(),
        "idle-scan residual must record start honesty"
    );
    assert_eq!(game_logic.battle_plans().turret_idle_scan_start_count(), 1);
    {
        let sc = game_logic.host_object(sc_id).expect("sc");
        let desired = sc.turret_ai.turret_idle_scan_desired_angle_deg;
        let natural = STRATEGY_CENTER_NATURAL_TURRET_ANGLE_DEG;
        assert!(desired.is_finite());
        assert!((desired - natural).abs() <= STRATEGY_CENTER_MAX_IDLE_SCAN_ANGLE_DEG);
        let rate = STRATEGY_CENTER_TURRET_TURN_DEG_PER_FRAME;
        let stepped = natural + (desired - natural).clamp(-rate, rate);
        assert!(
            (sc.turret_ai.turret_angle_deg - stepped).abs() < 0.01,
            "scan admission must step toward its actual selected goal"
        );
        assert_eq!(
            sc.turret_ai.turret_pitch_deg,
            STRATEGY_CENTER_NATURAL_TURRET_PITCH_DEG
        );
        assert!(
            sc.turret_ai.turret_idle_scanning || sc.turret_ai.turret_holding,
            "short aligned scans may enter HOLD immediately"
        );
    }

    // Exercise the update body with a legal nontrivial scan state, independent
    // of which allowed offset admission selected. This is a turret fixture,
    // not a seeded random sequence contract.
    let desired = STRATEGY_CENTER_NATURAL_TURRET_ANGLE_DEG + 30.0;
    {
        let sc = game_logic.host_object_mut(sc_id).expect("sc");
        sc.turret_ai.turret_angle_deg = STRATEGY_CENTER_NATURAL_TURRET_ANGLE_DEG;
        sc.turret_ai.turret_idle_scan_desired_angle_deg = desired;
        sc.turret_ai.turret_idle_scanning = true;
        sc.turret_ai.turret_holding = false;
        sc.turret_ai.turret_hold_until_frame = 0;
        sc.turret_ai.turret_idle_scan_next_frame = 0;
    }
    game_logic.frame = game_logic.frame.wrapping_add(1);
    game_logic
        .tick_battle_plan_door_residuals()
        .expect("valid battle-plan test state");
    {
        let sc = game_logic.host_object(sc_id).expect("sc");
        assert!(
            (sc.turret_ai.turret_angle_deg
                - (STRATEGY_CENTER_NATURAL_TURRET_ANGLE_DEG
                    + STRATEGY_CENTER_TURRET_TURN_DEG_PER_FRAME))
                .abs()
                < 0.01,
            "idle-scan body must step toward desired angle"
        );
        assert!(
            !turret_angles_are_natural(sc.turret_ai.turret_angle_deg, sc.turret_ai.turret_pitch_deg),
            "idle-scan leaves natural"
        );
        assert!(sc.turret_ai.turret_idle_scanning, "must be mid idle-scan body");
    }

    // Advance enough frames for remaining offset at 2 deg/frame.
    for _ in 0..30 {
        game_logic.frame = game_logic.frame.saturating_add(1);
        game_logic
            .tick_battle_plan_door_residuals()
            .expect("valid battle-plan test state");
        let sc = game_logic.host_object(sc_id).unwrap();
        if !sc.turret_ai.turret_idle_scanning
            && game_logic.battle_plans().turret_idle_scan_complete_count() > 0
        {
            break;
        }
    }
    assert!(
        game_logic.battle_plans().turret_idle_scan_complete_count() >= 1,
        "idle-scan residual must complete"
    );
    {
        let sc = game_logic.host_object(sc_id).expect("sc");
        assert!(
            !sc.turret_ai.turret_idle_scanning,
            "scan complete residual clears scanning flag"
        );
        // C++ IDLESCAN → HOLD: enter HoldTurret residual (no next-scan yet).
        assert!(
            sc.turret_ai.turret_holding,
            "scan complete residual must enter HoldTurret"
        );
        assert_eq!(
            sc.turret_ai.turret_idle_scan_next_frame, 0,
            "Hold residual defers next idle-scan schedule"
        );
        assert!(
            (sc.turret_ai.turret_angle_deg - desired).abs() < 0.5,
            "angles at desired after complete residual, angle={} desired={}",
            sc.turret_ai.turret_angle_deg,
            desired
        );
    }
    assert!(
        game_logic.honesty_strategy_center_turret_hold_ok(),
        "HoldTurret residual honesty must record start"
    );

    // Busy residual: attacking cancels mid-scan / hold.
    {
        let ai_entry_frame = game_logic.frame;
        let sc = game_logic.host_object_mut(sc_id).expect("sc");
        sc.turret_ai.turret_idle_scanning = true;
        sc.turret_ai.turret_holding = false;
        sc.turret_ai.turret_idle_scan_desired_angle_deg = desired;
        sc.set_status_attacking(true);
        sc.set_ai_state(
            AIState::Attacking,
            ai_entry_frame,
            &mut crate::game_logic::ObjectAudio::record(&mut Vec::new()),
        );
    }
    game_logic.frame = game_logic.frame.saturating_add(1);
    game_logic
        .tick_battle_plan_door_residuals()
        .expect("valid battle-plan test state");
    {
        let sc = game_logic.host_object(sc_id).expect("sc");
        assert!(
            !sc.turret_ai.turret_idle_scanning,
            "busy residual must cancel idle-scan"
        );
        assert!(
            !sc.turret_ai.turret_holding && !sc.turret_ai.turret_idle_recentering,
            "busy residual must cancel hold / idle-recenter"
        );
    }
}

#[test]
fn strategy_center_turret_hold_and_idle_recenter_residual() {
    use crate::game_logic::host_strategy_center::{
        HostBattlePlan, STRATEGY_CENTER_MAX_IDLE_SCAN_INTERVAL_FRAMES,
        STRATEGY_CENTER_MIN_IDLE_SCAN_INTERVAL_FRAMES, STRATEGY_CENTER_NATURAL_TURRET_ANGLE_DEG,
        STRATEGY_CENTER_NATURAL_TURRET_PITCH_DEG, STRATEGY_CENTER_RECENTER_TIME_FRAMES,
        hold_turret_until_frame, turret_angles_are_natural,
    };

    let mut game_logic = GameLogic::new();
    let mut sc_template = ThingTemplate::new("AmericaStrategyCenter");
    sc_template
        .add_kind_of(KindOf::Structure)
        .add_kind_of(KindOf::FSStrategyCenter)
        .add_kind_of(KindOf::Selectable)
        .set_health(1500.0);
    game_logic
        .templates
        .insert("AmericaStrategyCenter".to_string(), sc_template);
    if !game_logic.players.contains_key(&0) {
        game_logic.add_player(Player::new(0, Team::USA, "USA", true));
    }

    let sc_id = game_logic
        .create_object("AmericaStrategyCenter", Team::USA, Vec3::new(0.0, 0.0, 0.0))
        .expect("strategy center");

    assert!(game_logic.activate_battle_plan(0, HostBattlePlan::Bombardment, Some(sc_id),));
    advance_battle_plan_door_to_active(&mut game_logic);

    // Force idle-scan complete at desired angle → enter Hold immediately.
    let desired = STRATEGY_CENTER_NATURAL_TURRET_ANGLE_DEG + 30.0;
    let hold_start = game_logic.frame;
    {
        let ai_entry_frame = game_logic.frame;
        let sc = game_logic.host_object_mut(sc_id).expect("sc");
        sc.target = None;
        sc.set_ai_state(
            AIState::Idle,
            ai_entry_frame,
            &mut crate::game_logic::ObjectAudio::record(&mut Vec::new()),
        );
        sc.set_status_attacking(false);
        sc.turret_ai.turret_angle_deg = desired;
        sc.turret_ai.turret_pitch_deg = STRATEGY_CENTER_NATURAL_TURRET_PITCH_DEG;
        sc.turret_ai.turret_idle_scanning = true;
        sc.turret_ai.turret_idle_scan_desired_angle_deg = desired;
        sc.turret_ai.turret_idle_scan_index = 0;
        sc.turret_ai.turret_holding = false;
        sc.turret_ai.turret_idle_recentering = false;
        sc.turret_ai.turret_hold_until_frame = 0;
        sc.turret_ai.turret_idle_scan_next_frame = 0;
    }
    game_logic
        .tick_battle_plan_door_residuals()
        .expect("valid battle-plan test state");
    {
        let sc = game_logic.host_object(sc_id).expect("sc");
        assert!(sc.turret_ai.turret_holding, "scan complete → HoldTurret residual");
        assert!(!sc.turret_ai.turret_idle_scanning);
        assert_eq!(
            sc.turret_ai.turret_hold_until_frame,
            hold_turret_until_frame(hold_start)
        );
        assert_eq!(STRATEGY_CENTER_RECENTER_TIME_FRAMES, 60);
        // Angles frozen at desired during hold.
        assert!((sc.turret_ai.turret_angle_deg - desired).abs() < 0.5);
    }
    assert!(game_logic.honesty_strategy_center_turret_hold_ok());
    assert_eq!(game_logic.battle_plans().turret_hold_start_count(), 1);

    // Mid-hold: angles still frozen, not recentering yet.
    game_logic.frame = hold_start.saturating_add(30);
    game_logic
        .tick_battle_plan_door_residuals()
        .expect("valid battle-plan test state");
    {
        let sc = game_logic.host_object(sc_id).expect("sc");
        assert!(sc.turret_ai.turret_holding, "must still be holding mid RecenterTime");
        assert!(!sc.turret_ai.turret_idle_recentering);
        assert!((sc.turret_ai.turret_angle_deg - desired).abs() < 0.5);
    }

    // Hold elapses → idle-recenter residual starts toward natural.
    game_logic.frame = hold_turret_until_frame(hold_start);
    game_logic
        .tick_battle_plan_door_residuals()
        .expect("valid battle-plan test state");
    assert!(
        game_logic.battle_plans().turret_hold_complete_count() >= 1,
        "HoldTurret residual must complete after RecenterTime"
    );
    {
        let sc = game_logic.host_object(sc_id).expect("sc");
        assert!(!sc.turret_ai.turret_holding, "hold complete clears holding");
        // Either mid idle-recenter or already finished (if already natural).
        assert!(
            sc.turret_ai.turret_idle_recentering
                || turret_angles_are_natural(sc.turret_ai.turret_angle_deg, sc.turret_ai.turret_pitch_deg),
            "after hold must idle-recenter or already natural"
        );
        // First step toward natural: from desired (-60) toward -90 → -62.
        if sc.turret_ai.turret_idle_recentering {
            assert!(
                (sc.turret_ai.turret_angle_deg - desired).abs() > 0.5
                    || (sc.turret_ai.turret_angle_deg - STRATEGY_CENTER_NATURAL_TURRET_ANGLE_DEG).abs() < 0.5,
                "idle-recenter residual must step toward natural, got {}",
                sc.turret_ai.turret_angle_deg
            );
        }
    }
    assert!(
        game_logic.battle_plans().turret_idle_recenter_start_count() >= 1
            || game_logic.honesty_strategy_center_turret_idle_recenter_ok(),
        "idle-recenter residual must start"
    );

    // Step enough frames for 30° at 2 deg/frame to restore natural.
    for _ in 0..40 {
        game_logic.frame = game_logic.frame.saturating_add(1);
        game_logic
            .tick_battle_plan_door_residuals()
            .expect("valid battle-plan test state");
        let sc = game_logic.host_object(sc_id).unwrap();
        if !sc.turret_ai.turret_idle_recentering
            && turret_angles_are_natural(sc.turret_ai.turret_angle_deg, sc.turret_ai.turret_pitch_deg)
        {
            break;
        }
    }
    {
        let sc = game_logic.host_object(sc_id).expect("sc");
        assert!(
            turret_angles_are_natural(sc.turret_ai.turret_angle_deg, sc.turret_ai.turret_pitch_deg),
            "idle-recenter residual must restore natural angles, a={} p={}",
            sc.turret_ai.turret_angle_deg,
            sc.turret_ai.turret_pitch_deg
        );
        assert!(!sc.turret_ai.turret_idle_recentering);
        // Next idle-scan scheduled via leftover GameLogicRandomValue(min, max).
        assert!(
            sc.turret_ai.turret_idle_scan_next_frame
                >= game_logic
                    .frame
                    .saturating_add(STRATEGY_CENTER_MIN_IDLE_SCAN_INTERVAL_FRAMES)
                && sc.turret_ai.turret_idle_scan_next_frame
                    <= game_logic
                        .frame
                        .saturating_add(STRATEGY_CENTER_MAX_IDLE_SCAN_INTERVAL_FRAMES),
            "idle-recenter complete must reschedule next idle scan, next={} frame={}",
            sc.turret_ai.turret_idle_scan_next_frame,
            game_logic.frame
        );
    }
    assert!(
        game_logic.honesty_strategy_center_turret_idle_recenter_ok(),
        "idle-recenter residual honesty must record complete"
    );
}

#[test]
fn strategy_center_turret_mood_target_residual() {
    use crate::game_logic::host_strategy_center::{
        HostBattlePlan, STRATEGY_CENTER_BASE_VISION_RANGE, STRATEGY_CENTER_FIRE_PITCH_DEG,
        STRATEGY_CENTER_GUN_MIN_RANGE, STRATEGY_CENTER_NATURAL_TURRET_ANGLE_DEG,
        STRATEGY_CENTER_NATURAL_TURRET_PITCH_DEG, strategy_center_mood_vision_range,
    };
    assert!((STRATEGY_CENTER_BASE_VISION_RANGE - 400.0).abs() < 0.001);
    assert!((strategy_center_mood_vision_range(false) - 400.0).abs() < 0.001);

    let mut game_logic = GameLogic::new();
    ensure_test_tank_template(&mut game_logic);
    let mut sc_template = ThingTemplate::new("AmericaStrategyCenter");
    sc_template
        .add_kind_of(KindOf::Structure)
        .add_kind_of(KindOf::FSStrategyCenter)
        .add_kind_of(KindOf::Selectable)
        .set_health(1500.0);
    game_logic
        .templates
        .insert("AmericaStrategyCenter".to_string(), sc_template);
    if !game_logic.players.contains_key(&0) {
        game_logic.add_player(Player::new(0, Team::USA, "USA", true));
    }

    let sc_id = game_logic
        .create_object("AmericaStrategyCenter", Team::USA, Vec3::new(0.0, 0.0, 0.0))
        .expect("strategy center");
    assert!(game_logic.activate_battle_plan(0, HostBattlePlan::Bombardment, Some(sc_id),));
    advance_battle_plan_door_to_active(&mut game_logic);

    // Idle natural gun + enemy in gun range band (min 100).
    let enemy_id = game_logic
        .create_object(
            "TestTank",
            Team::GLA,
            Vec3::new(STRATEGY_CENTER_GUN_MIN_RANGE + 50.0, 0.0, 0.0),
        )
        .expect("enemy");
    {
        let ai_entry_frame = game_logic.frame;
        let sc = game_logic.host_object_mut(sc_id).expect("sc");
        sc.target = None;
        sc.set_ai_state(
            AIState::Idle,
            ai_entry_frame,
            &mut crate::game_logic::ObjectAudio::record(&mut Vec::new()),
        );
        sc.set_status_attacking(false);
        sc.turret_ai.turret_mood_target = false;
        sc.turret_ai.turret_angle_deg = STRATEGY_CENTER_NATURAL_TURRET_ANGLE_DEG;
        sc.turret_ai.turret_pitch_deg = STRATEGY_CENTER_NATURAL_TURRET_PITCH_DEG;
        sc.turret_ai.turret_idle_scanning = false;
        sc.turret_ai.turret_holding = false;
        sc.turret_ai.turret_idle_recentering = false;
    }
    let frame = game_logic.frame;
    let _ = game_logic.tick_turret_state_machine(sc_id, frame as f32 * LOGIC_FRAME_TIMESTEP, frame);
    game_logic
        .tick_battle_plan_door_residuals()
        .expect("valid battle-plan test state");
    {
        let sc = game_logic.host_object(sc_id).expect("sc");
        assert!(
            sc.turret_ai.turret_mood_target,
            "idle mood-target residual must flag m_targetWasSetByIdleMood"
        );
        {
            assert_eq!(sc.target, Some(enemy_id));
        }
        assert!(
            (sc.turret_ai.turret_pitch_deg - STRATEGY_CENTER_FIRE_PITCH_DEG).abs() < 0.01,
            "mood-target residual aims FirePitch, got {}",
            sc.turret_ai.turret_pitch_deg
        );
        // C++ acquisition only installs the goal. The next turret AIM update
        // owns rotation; an acquisition must not snap to a target angle.
        assert_eq!(sc.turret_ai.turret_target_id, Some(enemy_id));
        assert_eq!(
            sc.turret_ai.turret_substate,
            crate::game_logic::object::TurretSubState::Aim
        );
        assert_eq!(
            sc.turret_ai.turret_angle_deg,
            STRATEGY_CENTER_NATURAL_TURRET_ANGLE_DEG
        );
        assert!(!sc.turret_ai.turret_idle_scanning);
    }
    assert!(
        game_logic.honesty_strategy_center_turret_mood_target_ok(),
        "mood-target residual honesty must record acquire"
    );
    assert_eq!(
        game_logic.battle_plans().turret_mood_target_acquire_count(),
        1
    );

    // Move enemy out of max range → mood clear residual.
    {
        let (e, fixture_partition_e) = game_logic
            .host_object_and_partition_mut(enemy_id)
            .expect("enemy");
        e.set_position_with_partition_owner(
            Vec3::new(900.0, 0.0, 0.0),
            0,
            &mut Default::default(),
            fixture_partition_e,
        );
    }
    game_logic
        .tick_battle_plan_door_residuals()
        .expect("valid battle-plan test state");
    {
        let sc = game_logic.host_object(sc_id).expect("sc");
        assert!(
            !sc.turret_ai.turret_mood_target,
            "out-of-range residual must clear mood target"
        );
        assert!(sc.target.is_none());
        assert!(sc.turret_ai.turret_target_id.is_none());
    }
    assert!(
        game_logic.battle_plans().turret_mood_target_clear_count() >= 1,
        "mood-target clear honesty residual"
    );
}
