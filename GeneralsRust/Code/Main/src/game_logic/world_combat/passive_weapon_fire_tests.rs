//! Real receiving-world passive discharge and delayed-damage controls.
//! Fixtures admit Object/Weapon definitions before factory creation, including
//! AIUpdateInterface state ownership and the selected slot's immutable weapon row.
use crate::game_logic::{AIState, GameLogic, ObjectId, Player, Team};
use glam::Vec3;
const SOURCE: ObjectId = ObjectId(98401);
const TARGET: ObjectId = ObjectId(98402);
const MUZZLE: Vec3 = Vec3::new(13.0, 47.0, 29.0);
fn admitted_game(delay: f32, slot: u8) -> GameLogic {
    let mut game = GameLogic::new();
    game.add_player(Player::new(0, Team::USA, "local", true));
    game.add_player(Player::new(1, Team::China, "enemy", false));
    let mut primary = gamelogic::weapon::WeaponTemplate::new("PassivePrimary".into());
    primary.primary_damage = 5.0;
    primary.attack_range = 500.0;
    primary.projectile_name = "NONE".into();
    let mut secondary = gamelogic::weapon::WeaponTemplate::new("PassiveDelayed".into());
    secondary.primary_damage = 37.0;
    secondary.attack_range = 500.0;
    secondary.clip_size = 2;
    secondary.weapon_speed = 9.0; // C++ WeaponSpeed is distance per logic frame.
    secondary.pre_attack_delay = (delay * 30.0) as i32;
    secondary.projectile_name = "NONE".into();
    game.admit_receiving_weapon_snapshot(vec![
        (primary.name.clone(), primary),
        (secondary.name.clone(), secondary),
    ]).unwrap();
    let weapons = if slot == 0 {
        "Weapon = PRIMARY PassiveDelayed"
    } else {
        "Weapon = PRIMARY PassivePrimary\n Weapon = SECONDARY PassiveDelayed"
    };
    game.admit_object_ini_sources([("PassiveDischarge.ini".into(), format!(r#"
Object OwnedPassivePassenger
 KindOf = INFANTRY SELECTABLE CAN_ATTACK
 WeaponSet
  Conditions = None
  {weapons}
 End
 Body = ActiveBody Body
  MaxHealth = 100
  InitialHealth = 100
 End
 Behavior = AIUpdateInterface AI
 End
End
Object PriorPassiveShot
 KindOf = INFANTRY SELECTABLE CAN_ATTACK
 WeaponSet
  Conditions = None
  Weapon = PRIMARY PassivePrimary
 End
 Body = ActiveBody Body
  MaxHealth = 100
  InitialHealth = 100
 End
 Behavior = AIUpdateInterface AI
 End
End
Object OwnedPassiveVictim
 KindOf = VEHICLE SELECTABLE
 Body = ActiveBody Body
  MaxHealth = 100
  InitialHealth = 100
 End
End
Object PassiveTestTransport
 KindOf = VEHICLE SELECTABLE
 Body = ActiveBody Body
  MaxHealth = 300
  InitialHealth = 300
 End
 Behavior = TransportContain Contain
  Slots = 2
  PassengersAllowedToFire = Yes
  AllowInsideKindOf = INFANTRY
 End
End
Object PassiveTestGarrison
 KindOf = STRUCTURE SELECTABLE
 Body = ActiveBody Body
  MaxHealth = 300
  InitialHealth = 300
 End
 Behavior = GarrisonContain Contain
  ContainMax = 2
 End
End
"#))]).unwrap();
    game
}
fn world(state: AIState, delay: f32, slot: u8) -> GameLogic {
    let mut game = admitted_game(delay, slot);
    game.set_next_object_id_for_restore(SOURCE);
    assert_eq!(
        game.create_object_for_player("OwnedPassivePassenger", 0, Vec3::ZERO),
        Some(SOURCE)
    );
    assert_eq!(
        game.create_object_for_player(
            "OwnedPassiveVictim", 1, MUZZLE + Vec3::new(90.0, 0.0, 0.0),
        ),
        Some(TARGET)
    );
    let source = game.host_object_mut(SOURCE).unwrap();
    source.restore_ai_state(state);
    source.contained_by = Some(ObjectId(98403));
    source.weapon.as_mut().unwrap().last_fire_time = -10.0;
    assert!(source.set_weapon_barrel_count_for_slot(slot, 3));
    game.frame = 30;
    game
}
fn health(game: &GameLogic) -> f32 {
    game.host_object(TARGET).unwrap().health.current
}
fn delayed(game: &GameLogic) -> usize {
    crate::game_logic::combat::live_projectileless_delayed_count_for_test(&game.combat_system)
}
fn admit_passive_container(game: &mut GameLogic, state: AIState) {
    let name = if state == AIState::Garrisoned {
        "PassiveTestGarrison"
    } else {
        "PassiveTestTransport"
    };
    game.set_next_object_id_for_restore(ObjectId(98403));
    let container = game.create_object_for_player(name, 0, MUZZLE).unwrap();
    assert_eq!(container, ObjectId(98403));
    game.host_object_mut(container).unwrap().occupants.push(SOURCE);
}

#[test]
fn authored_held_passengers_obey_each_containers_fire_permission() {
    fn session(container_name: &str, subdued: bool) -> (GameLogic, ObjectId, ObjectId) {
        let mut game = GameLogic::new();
        let mut weapon = gamelogic::weapon::WeaponTemplate::new("PassengerTestGun".into());
        weapon.primary_damage = 25.0;
        weapon.attack_range = 200.0;
        weapon.projectile_name = "NONE".into();
        game.admit_receiving_weapon_snapshot(vec![("PassengerTestGun".into(), weapon)]).unwrap();
        game.admit_object_ini_sources([("PassengerPermission.ini".into(),
            "Object ArmedCarrier\n KindOf = VEHICLE SELECTABLE\n Body = ActiveBody Body\n MaxHealth = 300\n InitialHealth = 300\n End\n Behavior = TransportContain Contain\n Slots = 2\n PassengersAllowedToFire = Yes\n AllowInsideKindOf = INFANTRY\n End\nEnd\nObject SilentCarrier\n KindOf = VEHICLE SELECTABLE\n Body = ActiveBody Body\n MaxHealth = 300\n InitialHealth = 300\n End\n Behavior = TransportContain Contain\n Slots = 2\n PassengersAllowedToFire = No\n AllowInsideKindOf = INFANTRY\n End\nEnd\nObject WindowBunker\n KindOf = STRUCTURE SELECTABLE\n Body = ActiveBody Body\n MaxHealth = 300\n InitialHealth = 300\n End\n Behavior = GarrisonContain Contain\n ContainMax = 2\n End\nEnd\nObject Gunner\n KindOf = INFANTRY SELECTABLE CAN_ATTACK\n WeaponSet\n Conditions = None\n Weapon = PRIMARY PassengerTestGun\n End\n Body = ActiveBody Body\n MaxHealth = 100\n InitialHealth = 100\n End\n Behavior = AIUpdateInterface AI\n End\nEnd\nObject Enemy\n KindOf = VEHICLE SELECTABLE\n Body = ActiveBody Body\n MaxHealth = 500\n InitialHealth = 500\n End\nEnd\n".into()
        )]).unwrap();
        let container = game.create_object(container_name, Team::USA, Vec3::ZERO).unwrap();
        let rider = game.create_object("Gunner", Team::USA, Vec3::ZERO).unwrap();
        let victim = game.create_object("Enemy", Team::GLA, Vec3::new(30.0, 0.0, 0.0)).unwrap();
        let garrison = container_name == "WindowBunker";
        let host = game.host_object_mut(container).unwrap();
        host.occupants.push(rider);
        host.status.disabled_subdued = subdued;
        let child = game.host_object_mut(rider).unwrap();
        child.contained_by = Some(container);
        child.set_status_disabled_held(true);
        child.restore_ai_state(if garrison { AIState::Garrisoned } else { AIState::Docked });
        game.frame = 30;
        (game, rider, victim)
    }

    for (container, subdued, allowed) in [
        ("ArmedCarrier", false, true),
        ("SilentCarrier", false, false),
        ("WindowBunker", false, true),
        ("WindowBunker", true, false),
    ] {
        let (mut owner, rider, victim) = session(container, subdued);
        let (peer, peer_rider, peer_victim) = session(container, subdued);
        assert_eq!((rider, victim), (peer_rider, peer_victim));
        assert!(owner.host_object(rider).unwrap().can_attack());
        assert_eq!(owner.passenger_allowed_to_fire(rider), allowed);
        let hp = owner.host_object(victim).unwrap().health.current;
        if container == "WindowBunker" {
            owner.try_garrison_residual_fire(rider);
        } else {
            owner.try_transport_passenger_residual_fire(rider);
        }
        assert_eq!(owner.host_object(victim).unwrap().health.current < hp, allowed, "{container} subdued={subdued}");
        assert_eq!(peer.host_object(peer_victim).unwrap().health.current, hp);
    }
    let (mut owner, rider, victim) = session("ArmedCarrier", false);
    owner.host_object_mut(rider).unwrap().status.disabled_emp = true;
    assert!(owner.passenger_allowed_to_fire(rider));
    let hp = owner.host_object(victim).unwrap().health.current;
    owner.try_transport_passenger_residual_fire(rider);
    assert_eq!(owner.host_object(victim).unwrap().health.current, hp);
}

#[test]
fn real_passive_secondary_preserves_container_state_and_delays_damage_from_actual_muzzle() {
    for state in [AIState::Garrisoned, AIState::Docked] {
        let mut game = world(state.clone(), 0.0, 1);
        admit_passive_container(&mut game, state.clone());
        let peer = world(state.clone(), 0.0, 1);
        let hp = health(&game);
        let barrel = game
            .host_object(SOURCE)
            .unwrap()
            .weapon_barrel_cursor_for_snapshot(1);
        assert!(game.fire_owned_passive_weapon(SOURCE, TARGET, 1, MUZZLE));
        let source = game.host_object(SOURCE).unwrap();
        assert_eq!(source.ai_state(), state);
        assert_eq!(source.contained_by, Some(ObjectId(98403)));
        assert_eq!(source.secondary_weapon.as_ref().unwrap().ammo, Some(1));
        assert_eq!(source.weapon.as_ref().unwrap().last_fire_time, -10.0);
        assert_ne!(source.weapon_barrel_cursor_for_snapshot(1), barrel);
        assert_eq!(health(&game), hp);
        assert_eq!(delayed(&game), 1);
        game.frame = 39;
        game.apply_owned_projectileless_delayed_damage();
        assert_eq!(
            health(&game),
            hp,
            "90 / 9 is ten logic frames from supplied muzzle"
        );
        game.frame = 40;
        game.apply_owned_projectileless_delayed_damage();
        assert_eq!(health(&game), hp - 37.0);
        assert_eq!(delayed(&game), 0);
        game.apply_owned_projectileless_delayed_damage();
        assert_eq!(health(&game), hp - 37.0, "accepted shot is applied once");
        assert_eq!(health(&peer), hp);
        assert_eq!(
            peer.host_object(SOURCE)
                .unwrap()
                .secondary_weapon
                .as_ref()
                .unwrap()
                .ammo,
            Some(2)
        );
    }
}
#[test]
fn actual_passive_windup_emits_nothing_until_deadline_and_keeps_prior_pending_shot() {
    let mut game = world(AIState::Garrisoned, 0.5, 1);
    admit_passive_container(&mut game, AIState::Garrisoned);
    let prior = game.create_object_for_player("PriorPassiveShot", 0, Vec3::ZERO).unwrap();
    assert!(game.objects.get_mut(&prior).unwrap().fire_at(
        TARGET,
        1.0,
        30,
        &mut game.combat_system,
        &mut game.attack_events,
        &mut crate::game_logic::ObjectAudio::record(&mut Vec::new()),
    ));
    let pending =
        crate::game_logic::combat::pending_projectile_queue_len_for_test(&game.combat_system);
    assert_eq!(pending, 1);
    assert!(!game.fire_owned_passive_weapon(SOURCE, TARGET, 1, MUZZLE));
    assert_eq!(delayed(&game), 0);
    assert_eq!(
        game.host_object(SOURCE)
            .unwrap()
            .secondary_weapon
            .as_ref()
            .unwrap()
            .ammo,
        Some(2)
    );
    game.frame = 44;
    assert!(!game.fire_owned_passive_weapon(SOURCE, TARGET, 1, MUZZLE));
    game.frame = 45;
    assert!(game.fire_owned_passive_weapon(SOURCE, TARGET, 1, MUZZLE));
    assert_eq!(delayed(&game), 1);
    assert_eq!(
        game.host_object(SOURCE)
            .unwrap()
            .secondary_weapon
            .as_ref()
            .unwrap()
            .ammo,
        Some(1)
    );
    assert_eq!(
        crate::game_logic::combat::pending_projectile_queue_len_for_test(&game.combat_system),
        pending
    );
}

#[test]
fn shipping_transport_passenger_driver_uses_owned_delayed_discharge() {
    let mut game = world(AIState::Docked, 0.0, 0);
    admit_passive_container(&mut game, AIState::Docked);
    let hp = health(&game);
    game.try_transport_passenger_residual_fire(SOURCE);
    assert_eq!(
        game.host_object(SOURCE).unwrap().ai_state(),
        AIState::Docked
    );
    assert_eq!(
        game.host_object(SOURCE)
            .unwrap()
            .weapon
            .as_ref()
            .unwrap()
            .ammo,
        Some(1)
    );
    assert_eq!(health(&game), hp);
    assert_eq!(delayed(&game), 1);
    game.frame = 40;
    game.apply_owned_projectileless_delayed_damage();
    assert_eq!(health(&game), hp - 37.0);
}

#[test]
fn normal_encoded_save_restores_selected_passive_slot_and_delayed_damage_once() {
    let mut game = world(AIState::Docked, 0.0, 1);
    admit_passive_container(&mut game, AIState::Docked);
    game.objects.get_mut(&SOURCE).unwrap().active_weapon_slot = 1;
    let hp = health(&game);
    assert!(game.fire_owned_passive_weapon(SOURCE, TARGET, 1, MUZZLE));
    assert_eq!(delayed(&game), 1);
    let directory = tempfile::TempDir::new().unwrap();
    let mut manager = crate::save_load::SaveFileManager::with_save_directory(directory.path());
    manager.init().unwrap();
    let info = crate::save_load::SaveGameInfo {
        filename: "passive_owner".into(),
        display_name: "Passive continuation".into(),
        description: "Actual selected slot and pending damage".into(),
        map_name: "PassiveOwnerMap".into(),
        pristine_map_name: None,
        campaign_side: None,
        mission_number: None,
        save_date: std::time::SystemTime::now(),
        game_version: env!("CARGO_PKG_VERSION").into(),
        play_time: std::time::Duration::ZERO,
        difficulty: crate::save_load::GameDifficulty::Medium,
        save_type: crate::save_load::SaveFileType::Normal,
    };
    manager.save_game("passive_owner", &game, &info).unwrap();
    let crate::save_load::DecodedGameSave {
        snapshot: decoded, ..
    } = manager.load_game_snapshot("passive_owner").unwrap();
    let mut restored = admitted_game(0.0, 1);
    manager
        .restore_game_snapshot(&decoded, &mut restored)
        .unwrap();
    assert_eq!(restored.host_object(SOURCE).unwrap().active_weapon_slot, 1);
    assert_eq!(
        restored.host_object(SOURCE).unwrap().ai_state(),
        AIState::Docked
    );
    assert_eq!(
        restored.host_object(SOURCE).unwrap().contained_by,
        Some(ObjectId(98403))
    );
    assert_eq!(
        restored
            .host_object(SOURCE)
            .unwrap()
            .secondary_weapon
            .as_ref()
            .unwrap()
            .ammo,
        Some(1)
    );
    assert_eq!(delayed(&restored), 1);
    restored.frame = 39;
    restored.apply_owned_projectileless_delayed_damage();
    assert_eq!(health(&restored), hp);
    restored.frame = 40;
    restored.apply_owned_projectileless_delayed_damage();
    assert_eq!(health(&restored), hp - 37.0);
    restored.apply_owned_projectileless_delayed_damage();
    assert_eq!(health(&restored), hp - 37.0);
    assert_eq!(delayed(&restored), 0);
    assert_eq!(
        delayed(&game),
        1,
        "same-ID saved source remains independently pending"
    );
    assert_eq!(health(&game), hp);
}


#[test]
fn deferred_ammo_commit_matches_cpp_private_fire_order() {
    // C++ Weapon.cpp2611–2669: fireWeaponTemplate runs before ammo/timer commit.
    // GameLogic solo-set prepare must defer; commit_deferred applies afterward.
    use crate::game_logic::host_attack_log::AttackEvents;
    use crate::game_logic::host_fire_sound_loop_log::FireSoundLoopEvents;
    use game_engine::common::random_value::SessionRandom;
    use gamelogic::weapon::WeaponBonusSet;

    let mut game = world(AIState::Idle, 0.0, 1);
    let source = game.host_object_mut(SOURCE).unwrap();
    source.contained_by = None;
    let before = source.secondary_weapon.as_ref().unwrap().ammo;
    let mut sound = FireSoundLoopEvents::default();
    let mut attacks = AttackEvents::default();
    let solo = WeaponBonusSet::new();
    let random = SessionRandom::default();
    let rates = [1.0f32; 3];
    let (slot, _shot) = source
        .prepare_passive_weapon_shot_with_operation_and_solo_set(
            1,
            TARGET,
            0.0,
            false,
            false,
            7,
            MUZZLE,
            &mut sound,
            &mut attacks,
            None,
            &mut crate::game_logic::ObjectAudio::record(&mut Vec::new()),
            7,
            &mut Default::default(),
            &solo,
            1.0,
            1.0,
            rates,
            &random,
            None,
            false,
        )
        .expect("prepared deferred shot");
    assert_eq!(slot, 1);
    assert!(
        source.pending_weapon_fire_commit.is_some(),
        "solo-set prepare must defer C++ post-fire ammo commit"
    );
    assert_eq!(
        source.secondary_weapon.as_ref().unwrap().ammo,
        before,
        "ammo stays pre-fire until commit_deferred"
    );
    assert_eq!(source.last_fire_frame, 0);
    assert!(source.commit_deferred_weapon_fire_after_effects(
        &mut sound,
        Some(&solo),
        Some(&random),
        None,
    ).is_some());
    assert_eq!(
        source.secondary_weapon.as_ref().unwrap().ammo,
        Some(1),
        "commit decrements ammo once"
    );
    assert_eq!(source.last_fire_frame, 7);
    assert!(source.pending_weapon_fire_commit.is_none());
}
