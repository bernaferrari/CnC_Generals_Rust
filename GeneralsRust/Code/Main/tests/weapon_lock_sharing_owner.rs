use generals_main::game_logic::object::WeaponLockType;
use generals_main::game_logic::{GameLogic, ObjectId, Player, Team};
use generals_main::save_load::SnapshotBuilder;

fn weapon(name: &str) -> gamelogic::weapon::WeaponTemplate {
    let mut weapon = gamelogic::weapon::WeaponTemplate::new(name.to_owned());
    weapon.primary_damage = 5.0;
    weapon.attack_range = 100.0;
    weapon.suspend_fx_delay = 7;
    weapon.projectile_name = "NONE".into();
    weapon
}

fn admitted_world(shares_lock: bool) -> GameLogic {
    let mut world = GameLogic::new();
    let share = if shares_lock { "Yes" } else { "No" };
    world
        .admit_object_ini_sources([(
            "WeaponLock.ini".into(),
            format!(
                "Object WeaponLockOwner\n\
                 KindOf = INFANTRY SELECTABLE\n\
                 Body = ActiveBody BodyTag\n MaxHealth = 100\n InitialHealth = 100\n End\n\
                 WeaponSet\n Conditions = None\n Weapon = PRIMARY LockPolicyPrimary\n Weapon = SECONDARY LockPolicySecondary\n End\n\
                 WeaponSet\n Conditions = PLAYER_UPGRADE\n Weapon = PRIMARY LockPolicyPrimary\n Weapon = SECONDARY LockPolicySecondary\n WeaponLockSharedAcrossSets = {share}\n End\n\
                 End\n"
            ),
        )])
        .unwrap();
    world
        .admit_receiving_weapon_snapshot(vec![
            ("LockPolicyPrimary".into(), weapon("LockPolicyPrimary")),
            ("LockPolicySecondary".into(), weapon("LockPolicySecondary")),
        ])
        .unwrap();
    world
}

fn spawn(world: &mut GameLogic) -> ObjectId {
    world.add_player(Player::new(1, Team::USA, "Owner", true));
    world
        .create_object_for_player("WeaponLockOwner", 1, glam::Vec3::ZERO)
        .unwrap()
}

fn lock_secondary(world: &mut GameLogic, id: ObjectId) {
    let object = world.host_object_mut(id).unwrap();
    assert!(object.weapon_slot(1).is_some());
    assert!(object.set_weapon_lock(1, WeaponLockType::LockedPermanently));
}

fn add_player_upgrade_and_assert_lock(world: &mut GameLogic, id: ObjectId, expected: bool) {
    assert!(world.unit_command_set_weapon_set_flag(id, 0, true));
    let object = world.host_object(id).unwrap();
    assert_eq!(
        object.weapon_lock_type,
        if expected {
            WeaponLockType::LockedPermanently
        } else {
            WeaponLockType::NotLocked
        }
    );
    assert_eq!(object.active_weapon_slot, if expected { 1 } else { 0 });
}

#[test]
fn same_named_receiving_weapon_sets_keep_lock_policy_across_restore() {
    let mut sharing = admitted_world(true);
    let sharing_id = spawn(&mut sharing);
    let mut releasing = admitted_world(false);
    let releasing_id = spawn(&mut releasing);
    assert_eq!(
        sharing_id, releasing_id,
        "worlds intentionally reuse ObjectId"
    );
    lock_secondary(&mut sharing, sharing_id);
    lock_secondary(&mut releasing, releasing_id);

    // Save before the incoming condition is selected. The receiver must use
    // its admitted WeaponSet row when the real command transitions it.
    let builder = SnapshotBuilder::new();
    let saved = builder.create_world_snapshot(&sharing).unwrap();
    let mut restored = admitted_world(true);
    restored.add_player(Player::new(1, Team::USA, "Owner", true));
    builder
        .restore_from_snapshot(&saved, &mut restored)
        .unwrap();
    let restored_id = sharing_id;
    assert_eq!(
        restored.host_object(restored_id).unwrap().weapon_lock_type,
        WeaponLockType::LockedPermanently
    );

    add_player_upgrade_and_assert_lock(&mut sharing, sharing_id, true);
    add_player_upgrade_and_assert_lock(&mut releasing, releasing_id, false);
    add_player_upgrade_and_assert_lock(&mut restored, restored_id, true);

    // Interleave the two sessions after the transition. Their same-name
    // authored rows must continue to produce independent lock outcomes.
    assert_eq!(
        sharing.host_object(sharing_id).unwrap().weapon_lock_type,
        WeaponLockType::LockedPermanently
    );
    assert_eq!(
        releasing
            .host_object(releasing_id)
            .unwrap()
            .weapon_lock_type,
        WeaponLockType::NotLocked
    );
}
