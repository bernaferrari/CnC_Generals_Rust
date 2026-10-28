use generals_main::game_logic::{GameLogic, ObjectId, Player, Team};
use generals_main::save_load::SnapshotBuilder;

fn weapon(name: &str, damage: f32) -> gamelogic::weapon::WeaponTemplate {
    let mut weapon = gamelogic::weapon::WeaponTemplate::new(name.to_owned());
    weapon.primary_damage = damage;
    weapon.attack_range = 100.0;
    weapon.suspend_fx_delay = 7;
    weapon.projectile_name = "NONE".into();
    weapon
}

fn admitted_world(veteran_damage: f32) -> GameLogic {
    let mut game = GameLogic::new();
    game.admit_object_ini_sources([(
        "Object.ini".into(),
        "Object ReceivingWeaponOwner\n\
         KindOf = INFANTRY SELECTABLE\n\
         IsTrainable = Yes\n ExperienceRequired = 0 10 100 1000\n\
         Body = ActiveBody BodyTag\n MaxHealth = 100\n InitialHealth = 100\n End\n\
         WeaponSet\n Conditions = None\n Weapon = PRIMARY ReceivingBaseGun\n End\n\
         WeaponSet\n Conditions = VETERAN\n Weapon = PRIMARY ReceivingVeteranGun\n End\n\
         End\n"
            .into(),
    )])
    .unwrap();
    game.admit_receiving_weapon_snapshot(vec![
        ("ReceivingBaseGun".into(), weapon("ReceivingBaseGun", 4.0)),
        (
            "ReceivingVeteranGun".into(),
            weapon("ReceivingVeteranGun", veteran_damage),
        ),
    ])
    .unwrap();
    game
}

fn world(veteran_damage: f32) -> (GameLogic, ObjectId) {
    let mut game = admitted_world(veteran_damage);
    for _ in 0..37 {
        game.tick_logic_frame(1.0 / 30.0, None, Some(1));
    }
    game.add_player(Player::new(1, Team::USA, "Owner", true));
    let id = game
        .create_object_for_player("ReceivingWeaponOwner", 1, glam::Vec3::ZERO)
        .unwrap();
    (game, id)
}

#[test]
fn conditional_weapon_binding_is_world_owned_and_locked_after_objects_are_live() {
    let (mut a, id_a) = world(37.0);
    let (mut b, id_b) = world(83.0);
    assert_eq!(id_a, id_b);

    // Replacing the catalog after a live Object has captured receiving rows is rejected.
    assert!(
        a.admit_receiving_weapon_snapshot(vec![(
            "ReceivingVeteranGun".into(),
            weapon("ReceivingVeteranGun", 999.0),
        )])
        .is_err()
    );

    // Award enough XP to cross the Rookie → Veteran boundary in each world.
    a.award_experience(id_a, 60.0);
    b.award_experience(id_b, 60.0);
    let active_damage = |game: &GameLogic, id| {
        game.host_object(id)
            .unwrap()
            .weapon
            .as_ref()
            .expect("active primary")
            .damage
    };
    assert_eq!(active_damage(&a, id_a), 37.0);
    assert_eq!(active_damage(&b, id_b), 83.0);
    for game in [&a, &b] {
        let object = game.host_object(id_a).unwrap();
        assert!(object.secondary_weapon.is_none());
        assert!(object.tertiary_weapon.is_none());
    }
    assert_eq!(
        a.host_object(id_a)
            .unwrap()
            .weapon
            .as_ref()
            .unwrap()
            .suspend_fx_frame,
        44,
        "live promotion must use the receiving logic frame"
    );

    // Interleaving B must not change A's captured row or live weapon.
    b.award_experience(id_b, 100.0);
    assert_eq!(active_damage(&a, id_a), 37.0);

    // Peer reset retains its own receiving weapon baseline and admits fresh
    // objects with the same ObjectId without inheriting A's active row.
    b.reset();
    b.add_player(Player::new(1, Team::USA, "Owner", true));
    let reset_id = b
        .create_object_for_player("ReceivingWeaponOwner", 1, glam::Vec3::ZERO)
        .unwrap();
    assert_eq!(reset_id, id_b);
    b.award_experience(reset_id, 60.0);
    assert_eq!(active_damage(&b, reset_id), 83.0);

    // A snapshot restored into A's pre-admitted receiving catalog keeps the
    // active row and continues deterministically with the source world.
    let builder = SnapshotBuilder::new();
    let saved = builder.create_world_snapshot(&a).unwrap();
    let mut restored = admitted_world(37.0);
    restored.add_player(Player::new(1, Team::USA, "Owner", true));
    builder
        .restore_from_snapshot(&saved, &mut restored)
        .unwrap();
    assert_eq!(active_damage(&restored, id_a), 37.0);
    a.award_experience(id_a, 40.0);
    restored.award_experience(id_a, 40.0);
    assert_eq!(active_damage(&a, id_a), active_damage(&restored, id_a));
    assert_eq!(
        a.host_object(id_a).unwrap().experience.current,
        restored.host_object(id_a).unwrap().experience.current
    );
    assert_eq!(
        a.host_object(id_a).unwrap().experience.level,
        restored.host_object(id_a).unwrap().experience.level
    );
}
