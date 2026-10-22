//! Real receiving-world passive discharge and delayed-damage controls.
use crate::game_logic::{
    AIState, GameLogic, KindOf, Object, ObjectId, Player, Team, ThingTemplate, Weapon,
};
use glam::Vec3;
const SOURCE: ObjectId = ObjectId(98401);
const TARGET: ObjectId = ObjectId(98402);
const MUZZLE: Vec3 = Vec3::new(13.0, 47.0, 29.0);
fn world(state: AIState, delay: f32) -> GameLogic {
    let mut game = GameLogic::new();
    game.add_player(Player::new(0, Team::USA, "local", true));
    game.add_player(Player::new(1, Team::China, "enemy", false));
    let mut template = ThingTemplate::new("OwnedPassivePassenger");
    template.add_kind_of(KindOf::Infantry);
    template.add_kind_of(KindOf::Attackable);
    game.templates
        .insert(template.name.clone(), template.clone());
    let mut source = Object::new(template, SOURCE, Team::USA);
    source.owner_player_id = Some(0);
    source.ai_state = state;
    source.contained_by = Some(ObjectId(98403));
    source.weapon = Some(Weapon {
        damage: 5.0,
        last_fire_time: -10.0,
        ..Weapon::default()
    });
    source.secondary_weapon = Some(Weapon {
        damage: 37.0,
        range: 500.0,
        last_fire_time: -10.0,
        ammo: Some(2),
        clip_size: 2,
        projectile_speed: 9.0,
        pre_attack_delay: delay,
        ..Weapon::default()
    });
    assert!(source.set_weapon_barrel_count_for_slot(1, 3));
    let mut template = ThingTemplate::new("OwnedPassiveVictim");
    template.add_kind_of(KindOf::Attackable);
    game.templates
        .insert(template.name.clone(), template.clone());
    let mut target = Object::new(template, TARGET, Team::China);
    target.owner_player_id = Some(1);
    target.set_position(MUZZLE + Vec3::new(90.0, 0.0, 0.0));
    game.add_object(source);
    game.add_object(target);
    game.frame = 30;
    game
}
fn health(game: &GameLogic) -> f32 {
    game.host_object(TARGET).unwrap().health.current
}
fn delayed(game: &GameLogic) -> usize {
    crate::game_logic::combat::live_projectileless_delayed_count_for_test(&game.combat_system)
}
#[test]
fn real_passive_secondary_preserves_container_state_and_delays_damage_from_actual_muzzle() {
    for state in [AIState::Garrisoned, AIState::Docked] {
        let mut game = world(state.clone(), 0.0);
        let peer = world(state.clone(), 0.0);
        let hp = health(&game);
        let barrel = game
            .host_object(SOURCE)
            .unwrap()
            .weapon_barrel_cursor_for_snapshot(1);
        assert!(game.fire_owned_passive_weapon(SOURCE, TARGET, 1, MUZZLE));
        let source = game.host_object(SOURCE).unwrap();
        assert_eq!(source.ai_state, state);
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
    let mut game = world(AIState::Garrisoned, 0.5);
    let mut prior = Object::new(
        ThingTemplate::new("PriorPassiveShot"),
        ObjectId(98404),
        Team::USA,
    );
    prior.weapon = Some(Weapon {
        last_fire_time: -10.0,
        ..Weapon::default()
    });
    assert!(prior.fire_at(
        TARGET,
        1.0,
        30,
        &mut game.combat_system,
        &mut game.attack_events,
        &mut crate::game_logic::MovementAudio::queue(&mut Vec::new()),
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
    let mut game = world(AIState::Docked, 0.0);
    let source = game.objects.get_mut(&SOURCE).unwrap();
    source.weapon = source.secondary_weapon.take();
    let mut template = ThingTemplate::new("OwnedPassiveCarrier");
    template.add_kind_of(KindOf::Vehicle);
    let mut carrier = Object::new(template, ObjectId(98403), Team::USA);
    carrier.owner_player_id = Some(0);
    carrier.passengers_allowed_to_fire = true;
    carrier.occupants.push(SOURCE);
    carrier.set_position(MUZZLE);
    game.add_object(carrier);
    let hp = health(&game);
    game.try_transport_passenger_residual_fire(SOURCE);
    assert_eq!(game.host_object(SOURCE).unwrap().ai_state, AIState::Docked);
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
    let mut game = world(AIState::Docked, 0.0);
    let mut template = ThingTemplate::new("OwnedSavedPassiveCarrier");
    template.add_kind_of(KindOf::Vehicle);
    template.contain_module.kind = crate::game_logic::ContainModuleKind::Transport;
    template.contain_module.slots = Some(2);
    template.contain_module.admission = crate::game_logic::ContainAdmission::InfantryOnly;
    game.templates
        .insert(template.name.clone(), template.clone());
    let mut carrier = Object::new(template, ObjectId(98403), Team::USA);
    carrier.owner_player_id = Some(0);
    carrier.occupants.push(SOURCE);
    carrier.passengers_allowed_to_fire = true;
    carrier.set_position(MUZZLE);
    game.add_object(carrier);
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
    let mut restored = GameLogic::new();
    restored.templates = game.templates.clone();
    restored.add_player(Player::new(0, Team::USA, "local", true));
    restored.add_player(Player::new(1, Team::China, "enemy", false));
    manager
        .restore_game_snapshot(&decoded, &mut restored)
        .unwrap();
    assert_eq!(restored.host_object(SOURCE).unwrap().active_weapon_slot, 1);
    assert_eq!(
        restored.host_object(SOURCE).unwrap().ai_state,
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
