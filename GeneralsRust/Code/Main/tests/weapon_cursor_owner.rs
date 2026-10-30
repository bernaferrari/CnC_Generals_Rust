use gamelogic::weapon::WeaponTemplate;
use generals_main::game_logic::{GameLogic, ObjectId, Player, Team};

fn world(secondary_radius: f32, bonus: u32, extra_radius: Option<f32>) -> (GameLogic, ObjectId) {
    world_with_upgrade_bonus(secondary_radius, bonus, extra_radius, None)
}

fn world_with_upgrade_bonus(
    secondary_radius: f32,
    bonus: u32,
    extra_radius: Option<f32>,
    upgrade_radius: Option<u32>,
) -> (GameLogic, ObjectId) {
    let mut game = GameLogic::new();
    let upgrade_row = upgrade_radius
        .map(|percent| format!(" WeaponBonus = PLAYER_UPGRADE RADIUS {percent}%\n"))
        .unwrap_or_default();
    game.admit_combat_bonus_ini_sources([format!(
        "GameData\n WeaponBonus = SOLO_HUMAN_EASY RADIUS {bonus}%\n{upgrade_row}End\n"
    )])
    .unwrap();
    game.admit_object_ini_sources([("Cursor.ini".into(),
        "Object CursorUnit\n KindOf = VEHICLE SELECTABLE\n Body = ActiveBody Body\n MaxHealth = 100\n InitialHealth = 100\n End\n WeaponSet\n Conditions = NONE\n Weapon = PRIMARY CursorPrimary\n Weapon = SECONDARY CursorSecondary\n End\nEnd\n".into())]).unwrap();
    let mut primary = WeaponTemplate::new("CursorPrimary".into());
    primary.primary_damage_radius = 5.0;
    primary.scatter_radius = 1.0;
    primary.scatter_target_scalar = 2.0;
    primary.continue_attack_range = 3.0;
    let mut secondary = WeaponTemplate::new("CursorSecondary".into());
    secondary.primary_damage_radius = secondary_radius;
    secondary.scatter_radius = 7.0;
    secondary.scatter_target_scalar = 4.0;
    secondary.continue_attack_range = 90.0;
    if let Some(radius) = extra_radius {
        use gamelogic::weapon::{
            WeaponBonus, WeaponBonusConditionType, WeaponBonusField, WeaponBonusSet,
        };
        let mut bonus = WeaponBonus::new();
        bonus.set_field(WeaponBonusField::Radius, radius);
        let mut set = WeaponBonusSet::new();
        set.set_bonus(WeaponBonusConditionType::SoloHumanEasy, bonus);
        secondary.extra_bonus = Some(set);
    }
    game.admit_receiving_weapon_snapshot(vec![
        ("CursorPrimary".into(), primary),
        ("CursorSecondary".into(), secondary),
    ])
    .unwrap();
    game.add_player(Player::new(1, Team::USA, "Owner", true));
    let id = game
        .create_object_for_player("CursorUnit", 1, glam::Vec3::ZERO)
        .unwrap();
    game.host_object_mut(id).unwrap().weapon_bonus_solo = 16;
    game.host_object_mut(id)
        .unwrap()
        .weapon_bonus_player_upgrade = upgrade_radius.is_some();
    (game, id)
}

#[test]
fn cursor_uses_each_sessions_authored_upgrade_bonus() {
    let (owner, id) = world_with_upgrade_bonus(20.0, 100, None, Some(50));
    let (peer, peer_id) = world_with_upgrade_bonus(20.0, 100, None, Some(200));
    assert_eq!(id, peer_id);
    assert_eq!(owner.weapon_cursor_radii(id, 1).unwrap().damage, 10.0);
    assert_eq!(peer.weapon_cursor_radii(peer_id, 1).unwrap().damage, 40.0);
    assert_eq!(owner.weapon_cursor_radii(id, 1).unwrap().damage, 10.0);
    let snapshot = generals_main::save_load::SnapshotBuilder::new()
        .create_world_snapshot(&owner)
        .unwrap();
    let (mut restored, _) = world_with_upgrade_bonus(20.0, 100, None, Some(50));
    generals_main::save_load::SnapshotBuilder::new()
        .restore_from_snapshot(&snapshot, &mut restored)
        .unwrap();
    assert_eq!(restored.weapon_cursor_radii(id, 1).unwrap().damage, 10.0);
    assert_eq!(peer.weapon_cursor_radii(peer_id, 1).unwrap().damage, 40.0);
}

#[test]
fn bonus_conditions_are_appended_in_cpp_order() {
    use gamelogic::weapon::{
        WeaponBonus, WeaponBonusConditionFlags, WeaponBonusConditionType as Condition,
        WeaponBonusField, WeaponBonusSet,
    };
    let mut set = WeaponBonusSet::new();
    let mut flags = WeaponBonusConditionFlags::new();
    for (condition, value) in [
        (Condition::PlayerUpgrade, 3.0),
        (Condition::Nationalism, -1.0e20),
        (Condition::Horde, 1.0e20),
    ] {
        let mut bonus = WeaponBonus::new();
        bonus.set_field(WeaponBonusField::Damage, value);
        set.set_bonus(condition, bonus);
        flags.set(condition);
    }
    let mut result = WeaponBonus::new();
    set.append_bonuses(flags, &mut result);
    assert_eq!(result.get_field(WeaponBonusField::Damage), 2.0);
}

fn contained_weapon_world(
    authored_pass: bool,
    force_metadata: bool,
) -> (GameLogic, ObjectId, ObjectId) {
    contained_weapon_world_with_fire(authored_pass, force_metadata, None)
}

fn contained_weapon_world_with_fire(
    authored_pass: bool,
    force_metadata: bool,
    allow_fire: Option<bool>,
) -> (GameLogic, ObjectId, ObjectId) {
    contained_weapon_world_with_pre_attack(authored_pass, force_metadata, allow_fire, 0)
}

fn contained_weapon_world_with_pre_attack(
    authored_pass: bool,
    force_metadata: bool,
    allow_fire: Option<bool>,
    pre_attack_frames: i32,
) -> (GameLogic, ObjectId, ObjectId) {
    let mut game = GameLogic::new();
    game.admit_combat_bonus_ini_sources(["GameData\n WeaponBonus = SOLO_HUMAN_EASY RADIUS 150%\n WeaponBonus = SOLO_AI_EASY RADIUS 200%\n WeaponBonus = SOLO_AI_EASY DAMAGE 150%\n WeaponBonus = SOLO_AI_EASY RANGE 200%\n WeaponBonus = SOLO_AI_EASY PRE_ATTACK 200%\n WeaponBonus = GARRISONED RADIUS 125%\n WeaponBonus = FRENZY_ONE RADIUS 125%\n WeaponBonus = FRENZY_THREE RADIUS 175%\n WeaponBonus = VETERAN RADIUS 125%\n WeaponBonus = HERO RADIUS 150%\nEnd\n"]).unwrap();
    let pass = if authored_pass { "Yes" } else { "No" };
    let fire = allow_fire.map_or(String::new(), |allow| {
        format!(
            " PassengersAllowedToFire = {}\n",
            if allow { "Yes" } else { "No" }
        )
    });
    game.admit_object_ini_sources([("Container.ini".into(), format!(
        "Object Passenger\n KindOf = INFANTRY SELECTABLE\n TransportSlotCount = 1\n Body = ActiveBody Body\n MaxHealth = 100\n InitialHealth = 100\n End\n Behavior = AIUpdateInterface Brain\n End\n WeaponSet\n Conditions = NONE\n Weapon = PRIMARY ContainedWeapon\n End\nEnd\nObject Carrier\n KindOf = VEHICLE SELECTABLE\n Body = ActiveBody Body\n MaxHealth = 100\n InitialHealth = 100\n End\n Behavior = TransportContain Transport\n ContainMax = 2\n Slots = 2\n AllowInsideKindOf = INFANTRY\n WeaponBonusPassedToPassengers = {pass}\n{fire} End\n Behavior = AIUpdateInterface Brain\n End\nEnd\nObject Enemy\n KindOf = INFANTRY SELECTABLE\n Body = ActiveBody Body\n MaxHealth = 100\n InitialHealth = 100\n End\nEnd\n"
    ))]).unwrap();
    if force_metadata {
        game.templates
            .get_mut("Carrier")
            .unwrap()
            .contain_module
            .weapon_bonus_passed_to_passengers = true;
    }
    let mut weapon = WeaponTemplate::new("ContainedWeapon".into());
    weapon.primary_damage_radius = 20.0;
    weapon.primary_damage = 10.0;
    weapon.attack_range = 200.0;
    weapon.pre_attack_delay = pre_attack_frames;
    weapon.projectile_name = "NONE".into();
    game.admit_receiving_weapon_snapshot(vec![("ContainedWeapon".into(), weapon)])
        .unwrap();
    game.add_player(Player::new(1, Team::USA, "Owner", true));
    let passenger = game
        .create_object_for_player("Passenger", 1, glam::Vec3::ZERO)
        .unwrap();
    let carrier = game
        .create_object_for_player("Carrier", 1, glam::Vec3::ZERO)
        .unwrap();
    game.host_object_mut(passenger).unwrap().weapon_bonus_solo = 16;
    game.host_object_mut(passenger).unwrap().contained_by = Some(carrier);
    game.host_object_mut(carrier).unwrap().weapon_bonus_solo = 19;
    (game, passenger, carrier)
}

#[test]
fn authored_passenger_fire_permission_controls_actual_firing() {
    let (mut enabled, passenger, carrier) =
        contained_weapon_world_with_fire(true, false, Some(true));
    let (mut disabled, peer_passenger, peer_carrier) =
        contained_weapon_world_with_fire(true, false, Some(false));
    let (default, _, default_carrier) = contained_weapon_world(true, false);
    assert_eq!((passenger, carrier), (peer_passenger, peer_carrier));
    assert!(
        !default
            .host_object(default_carrier)
            .unwrap()
            .passengers_allowed_to_fire
    );
    assert!(
        enabled
            .host_object(carrier)
            .unwrap()
            .passengers_allowed_to_fire
    );
    assert!(
        !disabled
            .host_object(peer_carrier)
            .unwrap()
            .passengers_allowed_to_fire
    );
    let mut enemy_ids = Vec::new();
    for world in [&mut enabled, &mut disabled] {
        world.host_object_mut(passenger).unwrap().contained_by = None;
        world
            .templates
            .get_mut("Enemy")
            .unwrap()
            .add_kind_of(generals_main::game_logic::KindOf::Attackable);
        world.add_player(Player::new(2, Team::GLA, "Enemy", false));
        enemy_ids.push(
            world
                .create_object_for_player("Enemy", 2, glam::Vec3::new(20.0, 0.0, 0.0))
                .unwrap(),
        );
        assert!(world.unit_command_order_enter(passenger, carrier));
        world.try_physics_collide(passenger, carrier, 1.0);
        assert_eq!(
            world.host_object(passenger).unwrap().contained_by,
            Some(carrier)
        );
    }
    assert_eq!(enemy_ids[0], enemy_ids[1]);
    let snapshot = generals_main::save_load::SnapshotBuilder::new()
        .create_world_snapshot(&enabled)
        .unwrap();
    let (mut restored, _, _) = contained_weapon_world_with_fire(true, false, Some(true));
    restored.add_player(Player::new(2, Team::GLA, "Enemy", false));
    generals_main::save_load::SnapshotBuilder::new()
        .restore_from_snapshot(&snapshot, &mut restored)
        .unwrap();
    assert!(
        restored
            .host_object(carrier)
            .unwrap()
            .passengers_allowed_to_fire
    );
    assert_eq!(
        restored.host_object(passenger).unwrap().contained_by,
        Some(carrier)
    );
    enabled.host_object_mut(carrier).unwrap().subdual_damage = 100.0;
    enabled
        .host_object_mut(carrier)
        .unwrap()
        .status
        .disabled_subdued = true;
    for _ in 0..3 {
        enabled.tick_logic_frame(1.0 / 30.0, None, Some(1));
    }
    assert_eq!(
        enabled.host_object(enemy_ids[0]).unwrap().health.current,
        100.0
    );
    enabled.host_object_mut(carrier).unwrap().subdual_damage = 0.0;
    enabled
        .host_object_mut(carrier)
        .unwrap()
        .status
        .disabled_subdued = false;
    for _ in 0..30 {
        enabled.tick_logic_frame(1.0 / 30.0, None, Some(1));
        disabled.tick_logic_frame(1.0 / 30.0, None, Some(1));
    }
    assert!(enabled.host_object(enemy_ids[0]).unwrap().health.current < 100.0);
    assert_eq!(
        disabled.host_object(enemy_ids[1]).unwrap().health.current,
        100.0
    );
}

#[test]
fn helix_passenger_receives_authored_garrisoned_bonus() {
    let (mut world, passenger, carrier) = contained_weapon_world(true, false);
    assert_eq!(
        world.weapon_cursor_radii(passenger, 0).unwrap().damage,
        50.0
    );
    world.host_object_mut(carrier).unwrap().is_helix_transport = true;
    assert_eq!(
        world.weapon_cursor_radii(passenger, 0).unwrap().damage,
        55.0
    );
    world.host_object_mut(passenger).unwrap().contained_by = None;
    assert_eq!(
        world.weapon_cursor_radii(passenger, 0).unwrap().damage,
        30.0
    );
}

#[test]
fn passenger_pre_attack_uses_receiving_carrier_bonus() {
    let (mut delayed, passenger, carrier) =
        contained_weapon_world_with_pre_attack(true, false, Some(true), 30);
    let (mut normal, peer_passenger, peer_carrier) =
        contained_weapon_world_with_pre_attack(false, false, Some(true), 30);
    assert_eq!((passenger, carrier), (peer_passenger, peer_carrier));
    let mut enemies = Vec::new();
    for world in [&mut delayed, &mut normal] {
        world.host_object_mut(passenger).unwrap().contained_by = None;
        world
            .templates
            .get_mut("Enemy")
            .unwrap()
            .add_kind_of(generals_main::game_logic::KindOf::Attackable);
        world.add_player(Player::new(2, Team::GLA, "Enemy", false));
        enemies.push(
            world
                .create_object_for_player("Enemy", 2, glam::Vec3::new(20.0, 0.0, 0.0))
                .unwrap(),
        );
        assert!(world.unit_command_order_enter(passenger, carrier));
        world.try_physics_collide(passenger, carrier, 1.0);
        assert_eq!(
            world.host_object(passenger).unwrap().contained_by,
            Some(carrier)
        );
    }
    assert_eq!(enemies[0], enemies[1]);
    for _ in 0..45 {
        delayed.tick_logic_frame(1.0 / 30.0, None, Some(1));
        normal.tick_logic_frame(1.0 / 30.0, None, Some(1));
        if normal.host_object(enemies[1]).unwrap().health.current < 100.0 {
            break;
        }
    }
    assert!(normal.host_object(enemies[1]).unwrap().health.current < 100.0);
    assert_eq!(
        delayed.host_object(enemies[0]).unwrap().health.current,
        100.0
    );
    let snapshot = generals_main::save_load::SnapshotBuilder::new()
        .create_world_snapshot(&delayed)
        .unwrap();
    let (mut restored, _, _) = contained_weapon_world_with_pre_attack(true, false, Some(true), 30);
    restored
        .templates
        .get_mut("Enemy")
        .unwrap()
        .add_kind_of(generals_main::game_logic::KindOf::Attackable);
    restored.add_player(Player::new(2, Team::GLA, "Enemy", false));
    generals_main::save_load::SnapshotBuilder::new()
        .restore_from_snapshot(&snapshot, &mut restored)
        .unwrap();
    for _ in 0..40 {
        delayed.tick_logic_frame(1.0 / 30.0, None, Some(1));
        restored.tick_logic_frame(1.0 / 30.0, None, Some(1));
        assert_eq!(
            delayed.host_object(enemies[0]).unwrap().health.current,
            restored.host_object(enemies[0]).unwrap().health.current
        );
        if delayed.host_object(enemies[0]).unwrap().health.current < 100.0 {
            break;
        }
    }
    assert!(delayed.host_object(enemies[0]).unwrap().health.current < 100.0);
}

#[test]
fn passenger_acquisition_uses_carriers_receiving_range_bonus() {
    let (mut boosted, passenger, carrier) =
        contained_weapon_world_with_fire(true, false, Some(true));
    let (mut plain, peer_passenger, peer_carrier) =
        contained_weapon_world_with_fire(false, false, Some(true));
    assert_eq!((passenger, carrier), (peer_passenger, peer_carrier));
    let mut targets = Vec::new();
    for world in [&mut boosted, &mut plain] {
        world.host_object_mut(passenger).unwrap().contained_by = None;
        world
            .templates
            .get_mut("Enemy")
            .unwrap()
            .add_kind_of(generals_main::game_logic::KindOf::Attackable);
        world.add_player(Player::new(2, Team::GLA, "Enemy", false));
        targets.push(
            world
                .create_object_for_player("Enemy", 2, glam::Vec3::new(300.0, 0.0, 0.0))
                .unwrap(),
        );
        assert!(world.unit_command_order_enter(passenger, carrier));
        world.try_physics_collide(passenger, carrier, 1.0);
        assert_eq!(
            world.host_object(passenger).unwrap().contained_by,
            Some(carrier)
        );
    }
    assert_eq!(targets[0], targets[1]);
    for _ in 0..100 {
        boosted.tick_logic_frame(1.0 / 30.0, None, Some(1));
        plain.tick_logic_frame(1.0 / 30.0, None, Some(1));
        if boosted.host_object(targets[0]).unwrap().health.current < 100.0 {
            break;
        }
    }
    assert!(boosted.host_object(targets[0]).unwrap().health.current < 100.0);
    assert_eq!(plain.host_object(targets[1]).unwrap().health.current, 100.0);
}

#[test]
fn passenger_firing_inherits_the_receiving_carriers_damage_bonus() {
    let (mut owner, passenger, carrier) = contained_weapon_world_with_fire(true, false, Some(true));
    let (mut peer, peer_passenger, peer_carrier) =
        contained_weapon_world_with_fire(false, false, Some(true));
    assert_eq!((passenger, carrier), (peer_passenger, peer_carrier));
    let mut enemy_ids = Vec::new();
    for world in [&mut owner, &mut peer] {
        world.host_object_mut(passenger).unwrap().contained_by = None;
        assert!(
            world
                .host_object(carrier)
                .unwrap()
                .passengers_allowed_to_fire
        );
        world
            .templates
            .get_mut("Enemy")
            .unwrap()
            .add_kind_of(generals_main::game_logic::KindOf::Attackable);
        world.add_player(Player::new(2, Team::GLA, "Enemy", false));
        enemy_ids.push(
            world
                .create_object_for_player("Enemy", 2, glam::Vec3::new(20.0, 0.0, 0.0))
                .unwrap(),
        );
        assert!(world.can_unit_enter_normal_target(passenger, carrier));
        assert!(world.unit_command_order_enter(passenger, carrier));
        world.try_physics_collide(passenger, carrier, 1.0);
        assert_eq!(
            world.host_object(passenger).unwrap().contained_by,
            Some(carrier)
        );
        assert_eq!(
            world.host_object(passenger).unwrap().ai_state(),
            generals_main::game_logic::AIState::Docked
        );
    }
    assert_eq!(enemy_ids[0], enemy_ids[1]);
    let enemy = enemy_ids[0];
    for _ in 0..100 {
        owner.tick_logic_frame(1.0 / 30.0, None, Some(1));
        peer.tick_logic_frame(1.0 / 30.0, None, Some(1));
        if owner.host_object(enemy).unwrap().health.current < 100.0
            && peer.host_object(enemy).unwrap().health.current < 100.0
        {
            break;
        }
    }
    assert_eq!(
        owner.host_object(enemy).unwrap().health.current,
        85.0,
        "source={:?} weapon={:?} carrier={:?} target={:?}",
        owner.host_object(passenger).unwrap().ai_state(),
        owner.host_object(passenger).unwrap().weapon,
        owner
            .host_object(carrier)
            .unwrap()
            .passengers_allowed_to_fire,
        owner.host_object(enemy).unwrap().health.current
    );
    assert_eq!(peer.host_object(enemy).unwrap().health.current, 90.0);
}

#[test]
fn authored_container_flag_controls_passenger_weapon_bonus() {
    let (yes, passenger, carrier) = contained_weapon_world(true, false);
    let (no, peer_passenger, peer_carrier) = contained_weapon_world(false, false);
    assert_eq!((passenger, carrier), (peer_passenger, peer_carrier));
    assert!(
        yes.host_object(carrier)
            .unwrap()
            .passes_weapon_bonus_to_passengers()
    );
    assert!(
        !no.host_object(peer_carrier)
            .unwrap()
            .passes_weapon_bonus_to_passengers()
    );
    assert_eq!(
        no.weapon_cursor_radii(peer_passenger, 0).unwrap().damage,
        30.0
    );
    assert_eq!(yes.weapon_cursor_radii(passenger, 0).unwrap().damage, 50.0);
}

#[test]
fn contained_weapon_combines_distinct_source_and_carrier_conditions() {
    let (mut game, passenger, carrier) = contained_weapon_world(false, true);
    assert!(
        game.host_object(carrier)
            .unwrap()
            .passes_weapon_bonus_to_passengers()
    );
    assert_eq!(game.weapon_cursor_radii(passenger, 0).unwrap().damage, 50.0);
    game.host_object_mut(passenger).unwrap().weapon_bonus_frenzy = true;
    game.host_object_mut(passenger)
        .unwrap()
        .weapon_bonus_frenzy_level = 1;
    game.host_object_mut(carrier).unwrap().weapon_bonus_frenzy = true;
    game.host_object_mut(carrier)
        .unwrap()
        .weapon_bonus_frenzy_level = 3;
    assert_eq!(game.weapon_cursor_radii(passenger, 0).unwrap().damage, 70.0);
    game.host_object_mut(carrier).unwrap().weapon_bonus_solo = 16;
    assert_eq!(game.weapon_cursor_radii(passenger, 0).unwrap().damage, 50.0);
    game.host_object_mut(passenger)
        .unwrap()
        .weapon_bonus_veteran = true;
    game.host_object_mut(carrier).unwrap().weapon_bonus_hero = true;
    assert_eq!(game.weapon_cursor_radii(passenger, 0).unwrap().damage, 65.0);
}

#[test]
fn boarding_save_and_evacuation_preserve_session_bonus_ownership() {
    let (mut owner, passenger, carrier) = contained_weapon_world(true, false);
    let (mut peer, peer_passenger, peer_carrier) = contained_weapon_world(false, false);
    assert_eq!((passenger, carrier), (peer_passenger, peer_carrier));
    for world in [&mut owner, &mut peer] {
        world.host_object_mut(passenger).unwrap().contained_by = None;
        assert!(world.can_unit_enter_normal_target(passenger, carrier));
        assert!(world.unit_command_order_enter(passenger, carrier));
        world.try_physics_collide(passenger, carrier, 1.0);
        assert_eq!(
            world.host_object(passenger).unwrap().contained_by,
            Some(carrier)
        );
    }
    assert_eq!(
        owner.weapon_cursor_radii(passenger, 0).unwrap().damage,
        50.0
    );
    assert_eq!(
        peer.weapon_cursor_radii(peer_passenger, 0).unwrap().damage,
        30.0
    );
    let snapshot = generals_main::save_load::SnapshotBuilder::new()
        .create_world_snapshot(&owner)
        .unwrap();
    let (mut restored, _, _) = contained_weapon_world(true, false);
    generals_main::save_load::SnapshotBuilder::new()
        .restore_from_snapshot(&snapshot, &mut restored)
        .unwrap();
    assert_eq!(
        restored.host_object(passenger).unwrap().contained_by,
        Some(carrier)
    );
    assert_eq!(
        restored.weapon_cursor_radii(passenger, 0).unwrap().damage,
        50.0
    );
    assert!(owner.evacuate_container_now(carrier, false));
    assert_eq!(owner.host_object(passenger).unwrap().contained_by, None);
    assert_eq!(
        owner.weapon_cursor_radii(passenger, 0).unwrap().damage,
        30.0
    );
    assert_eq!(
        peer.host_object(peer_passenger).unwrap().contained_by,
        Some(peer_carrier)
    );
    assert_eq!(
        peer.weapon_cursor_radii(peer_passenger, 0).unwrap().damage,
        30.0
    );
    assert_eq!(
        restored.weapon_cursor_radii(passenger, 0).unwrap().damage,
        50.0
    );
}

#[test]
fn passenger_fire_runtime_override_survives_save_formats_and_peer_interleaving() {
    use generals_main::save_load::{Snapshot, SnapshotBuilder, WorldSnapshot, XferLoad, XferSave};
    for (authored, changed) in [(false, true), (true, false)] {
        let (mut source, passenger, carrier) =
            contained_weapon_world_with_fire(true, false, Some(authored));
        let (peer, peer_passenger, peer_carrier) =
            contained_weapon_world_with_fire(true, false, Some(!authored));
        assert_eq!((passenger, carrier), (peer_passenger, peer_carrier));
        source.host_object_mut(passenger).unwrap().contained_by = None;
        assert!(source.unit_command_order_enter(passenger, carrier));
        source.try_physics_collide(passenger, carrier, 1.0);
        assert_eq!(
            source.host_object(passenger).unwrap().contained_by,
            Some(carrier)
        );
        source
            .host_object_mut(carrier)
            .unwrap()
            .passengers_allowed_to_fire = changed;
        source
            .templates
            .get_mut("Enemy")
            .unwrap()
            .add_kind_of(generals_main::game_logic::KindOf::Attackable);
        source.add_player(Player::new(2, Team::GLA, "Enemy", false));
        let enemy = source
            .create_object_for_player("Enemy", 2, glam::Vec3::new(20.0, 0.0, 0.0))
            .unwrap();
        let builder = SnapshotBuilder::new();
        let mut snapshot = builder.create_world_snapshot(&source).unwrap();
        let bytes = bincode_legacy::serialize(&snapshot).unwrap();
        let bincode_snapshot: WorldSnapshot = bincode_legacy::deserialize(&bytes).unwrap();
        let mut stream = std::io::Cursor::new(Vec::new());
        snapshot.xfer(&mut XferSave::new(&mut stream)).unwrap();
        let mut xfer_snapshot = WorldSnapshot::default();
        xfer_snapshot
            .xfer(&mut XferLoad::new(std::io::Cursor::new(
                stream.into_inner(),
            )))
            .unwrap();
        for decoded in [&bincode_snapshot, &xfer_snapshot] {
            let (mut restored, _, _) =
                contained_weapon_world_with_fire(true, false, Some(authored));
            restored
                .templates
                .get_mut("Enemy")
                .unwrap()
                .add_kind_of(generals_main::game_logic::KindOf::Attackable);
            restored.add_player(Player::new(2, Team::GLA, "Enemy", false));
            builder
                .restore_from_snapshot(decoded, &mut restored)
                .unwrap();
            assert_eq!(
                restored
                    .host_object(carrier)
                    .unwrap()
                    .passengers_allowed_to_fire,
                changed
            );
            assert_eq!(
                restored.host_object(passenger).unwrap().contained_by,
                Some(carrier)
            );
            assert_eq!(
                peer.host_object(peer_carrier)
                    .unwrap()
                    .passengers_allowed_to_fire,
                !authored
            );
            for _ in 0..30 {
                restored.tick_logic_frame(1.0 / 30.0, None, Some(1));
            }
            let health = restored.host_object(enemy).unwrap().health.current;
            if changed {
                assert!(health < 100.0);
            } else {
                assert_eq!(health, 100.0);
            }
        }
    }
}

#[test]
fn rejected_passenger_fire_continuation_preserves_running_session() {
    let (mut source, passenger, carrier) =
        contained_weapon_world_with_fire(true, false, Some(false));
    source
        .host_object_mut(carrier)
        .unwrap()
        .passengers_allowed_to_fire = true;
    let builder = generals_main::save_load::SnapshotBuilder::new();
    let mut snapshot = builder.create_world_snapshot(&source).unwrap();
    let offset = snapshot
        .lifecycle_tail
        .windows(4)
        .rposition(|tag| tag == b"OXOB")
        .unwrap();
    snapshot.lifecycle_tail[offset + 4..offset + 8].copy_from_slice(&18u32.to_le_bytes());
    let (mut running, _, _) = contained_weapon_world_with_fire(true, false, Some(false));
    let before = running.weapon_cursor_radii(passenger, 0).unwrap();
    assert!(
        builder
            .restore_from_snapshot(&snapshot, &mut running)
            .is_err()
    );
    assert!(
        !running
            .host_object(carrier)
            .unwrap()
            .passengers_allowed_to_fire
    );
    assert_eq!(running.weapon_cursor_radii(passenger, 0).unwrap(), before);
}

#[test]
fn prior_object_continuation_uses_authored_permission() {
    let (mut source, _, carrier) = contained_weapon_world_with_fire(true, false, Some(false));
    source
        .host_object_mut(carrier)
        .unwrap()
        .passengers_allowed_to_fire = true;
    let builder = generals_main::save_load::SnapshotBuilder::new();
    let mut snapshot = builder.create_world_snapshot(&source).unwrap();
    let offset = snapshot
        .lifecycle_tail
        .windows(4)
        .rposition(|tag| tag == b"OXOB")
        .unwrap();
    let length = u32::from_le_bytes(
        snapshot.lifecycle_tail[offset + 8..offset + 12]
            .try_into()
            .unwrap(),
    ) as usize;
    let end = offset + 12 + length;
    let tail_length = 8 + snapshot.objects.len() * 5;
    let start = end - tail_length;
    assert_eq!(
        u64::from_le_bytes(
            snapshot.lifecycle_tail[start..start + 8]
                .try_into()
                .unwrap()
        ) as usize,
        snapshot.objects.len()
    );
    snapshot.lifecycle_tail.drain(start..end);
    snapshot.lifecycle_tail[offset + 4..offset + 8].copy_from_slice(&18u32.to_le_bytes());
    snapshot.lifecycle_tail[offset + 8..offset + 12]
        .copy_from_slice(&((length - tail_length) as u32).to_le_bytes());
    snapshot.version = 38;
    let (mut restored, _, _) = contained_weapon_world_with_fire(true, false, Some(false));
    builder
        .restore_from_snapshot(&snapshot, &mut restored)
        .unwrap();
    assert!(
        !restored
            .host_object(carrier)
            .unwrap()
            .passengers_allowed_to_fire
    );
}

#[test]
fn cursor_radii_use_exact_slot_receiving_definitions_and_bonus() {
    let (owner, id) = world(20.0, 150, None);
    let (peer, peer_id) = world(40.0, 100, None);
    assert_eq!(id, peer_id);
    let primary = owner.weapon_cursor_radii(id, 0).unwrap();
    let secondary = owner.weapon_cursor_radii(id, 1).unwrap();
    let peer_secondary = peer.weapon_cursor_radii(id, 1).unwrap();
    assert_eq!(
        (primary.damage, primary.scatter, primary.continue_attack),
        (7.5, 3.0, 3.0)
    );
    assert_eq!(
        (
            secondary.damage,
            secondary.scatter,
            secondary.continue_attack
        ),
        (30.0, 11.0, 90.0)
    );
    assert_eq!(
        (
            peer_secondary.damage,
            peer_secondary.scatter,
            peer_secondary.continue_attack
        ),
        (40.0, 11.0, 90.0)
    );
    assert!(owner.weapon_cursor_radii(id, 2).is_none());
    assert!(owner.weapon_cursor_radii(id, 3).is_none());
    let snapshot = generals_main::save_load::SnapshotBuilder::new()
        .create_world_snapshot(&owner)
        .unwrap();
    let (mut restored, _) = world(20.0, 150, None);
    generals_main::save_load::SnapshotBuilder::new()
        .restore_from_snapshot(&snapshot, &mut restored)
        .unwrap();
    assert_eq!(restored.weapon_cursor_radii(id, 1), Some(secondary));
}

#[test]
fn cursor_radius_uses_each_sessions_weapon_extra_bonus() {
    let (owner, id) = world(20.0, 150, Some(2.0));
    let (peer, peer_id) = world(20.0, 150, Some(3.0));
    assert_eq!(id, peer_id);
    assert_eq!(owner.weapon_cursor_radii(id, 1).unwrap().damage, 50.0);
    assert_eq!(peer.weapon_cursor_radii(peer_id, 1).unwrap().damage, 70.0);
    assert_eq!(owner.weapon_cursor_radii(id, 1).unwrap().damage, 50.0);
    let snapshot = generals_main::save_load::SnapshotBuilder::new()
        .create_world_snapshot(&owner)
        .unwrap();
    let (mut restored, _) = world(20.0, 150, Some(2.0));
    generals_main::save_load::SnapshotBuilder::new()
        .restore_from_snapshot(&snapshot, &mut restored)
        .unwrap();
    assert_eq!(restored.weapon_cursor_radii(id, 1).unwrap().damage, 50.0);
    assert_eq!(peer.weapon_cursor_radii(peer_id, 1).unwrap().damage, 70.0);
}
