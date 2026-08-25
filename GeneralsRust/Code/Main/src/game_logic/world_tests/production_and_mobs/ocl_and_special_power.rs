//! Behavior suite extracted from `production_and_mobs`.
use super::*;

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
        assert!(
            chinook.add_occupant(tank_id),
            "Combat Chinook admits vehicles"
        );
    }
    game_logic.refresh_battle_bus_armed_riders_weapon_set(chinook_id);
    let chinook = game_logic.host_object(chinook_id).expect("chinook");
    assert!(
        !chinook.weapon_set_player_upgrade,
        "vehicle-only load must not set WEAPONSET_PLAYER_UPGRADE"
    );
}

#[test]
fn listening_outpost_residual_capacity_detect_and_payload() {
    use crate::game_logic::host_listening_outpost::{
        LISTENING_OUTPOST_DETECTION_RANGE, LISTENING_OUTPOST_INITIAL_PAYLOAD_COUNT,
        LISTENING_OUTPOST_STEALTH_DELAY_FRAMES, LISTENING_OUTPOST_TRANSPORT_SLOTS,
        is_listening_outpost_template,
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
    assert_eq!(
        outpost.stealth_delay_frames,
        LISTENING_OUTPOST_STEALTH_DELAY_FRAMES
    );
    assert_eq!(
        outpost.stealth_allowed_frame,
        LISTENING_OUTPOST_STEALTH_DELAY_FRAMES
    );
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
                && o.template_name
                    .eq_ignore_ascii_case("GLAInfantryTunnelDefender")
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
        assert!(
            !trooper.can_move(),
            "HELD spawn-point units cannot march off"
        );
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
                && o.template_name
                    .eq_ignore_ascii_case("GLAInfantryTunnelDefender")
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
                && o.template_name
                    .eq_ignore_ascii_case("GLAInfantryStingerSoldier")
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
                && o.template_name
                    .eq_ignore_ascii_case("GLAInfantryStingerSoldier")
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

    register_pristine_bone_lookup_hook(Some(std::sync::Arc::new(|model, _scale, _frame, bone| {
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
            "SpawnPoint02" => Some((2, Matrix3D::from_translation(Coord3D::new(0.0, 5.0, 0.0)))),
            "SpawnPoint03" => Some((3, Matrix3D::from_translation(Coord3D::new(-3.0, -2.0, 0.0)))),
            _ => None,
        }
    })));

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
                && o.template_name
                    .eq_ignore_ascii_case("GLAInfantryStingerSoldier")
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
                && o.template_name
                    .eq_ignore_ascii_case("GLAInfantryStingerSoldier")
                && o.is_alive()
        })
        .count();
    assert_eq!(
        again, 3,
        "reserveDoorForExit refuses when every bone is occupied"
    );

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
