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
