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
    let mut game = GameLogic::new();
    game.admit_combat_bonus_ini_sources(["GameData\n WeaponBonus = SOLO_HUMAN_EASY RADIUS 150%\n WeaponBonus = SOLO_AI_EASY RADIUS 200%\n WeaponBonus = FRENZY_ONE RADIUS 125%\n WeaponBonus = FRENZY_THREE RADIUS 175%\n WeaponBonus = VETERAN RADIUS 125%\n WeaponBonus = HERO RADIUS 150%\nEnd\n"]).unwrap();
    let pass = if authored_pass { "Yes" } else { "No" };
    game.admit_object_ini_sources([("Container.ini".into(), format!(
        "Object Passenger\n KindOf = INFANTRY SELECTABLE\n TransportSlotCount = 1\n Body = ActiveBody Body\n MaxHealth = 100\n InitialHealth = 100\n End\n Behavior = AIUpdateInterface Brain\n End\n WeaponSet\n Conditions = NONE\n Weapon = PRIMARY ContainedWeapon\n End\nEnd\nObject Carrier\n KindOf = VEHICLE SELECTABLE\n Body = ActiveBody Body\n MaxHealth = 100\n InitialHealth = 100\n End\n Behavior = TransportContain Transport\n ContainMax = 2\n Slots = 2\n AllowInsideKindOf = INFANTRY\n WeaponBonusPassedToPassengers = {pass}\n End\n Behavior = AIUpdateInterface Brain\n End\nEnd\n"
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
