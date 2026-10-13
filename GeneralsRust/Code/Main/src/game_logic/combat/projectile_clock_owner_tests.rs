//! CPP Weapon.cpp:705 skips structures while their JetAI sneaky interval is active.
use super::*;

fn receiving_world(frame: u32) -> (GameLogic, ObjectId, ObjectId) {
    let mut world = GameLogic::new();
    world.frame = frame;
    world.add_player(Player::new(1, Team::USA, "Shooter", true));
    world.add_player(Player::new(2, Team::GLA, "Blocker", false));
    for (name, kind) in [
        ("ClockShooter", KindOf::Infantry),
        ("AmericaJetAurora", KindOf::Structure),
    ] {
        let mut template = ThingTemplate::new(name);
        template.set_health(100.0).add_kind_of(kind);
        world.templates.insert(name.into(), template);
    }
    let shooter = world
        .create_object_for_player("ClockShooter", 1, Vec3::new(-100.0, 0.0, 0.0))
        .unwrap();
    let blocker = world
        .create_object_for_player("AmericaJetAurora", 2, Vec3::ZERO)
        .unwrap();
    assert!(
        world.host_object(blocker).unwrap().is_alive(),
        "collision blocker must be a live canonical Body"
    );
    assert!(
        world
            .host_object(blocker)
            .unwrap()
            .is_kind_of(KindOf::Structure),
        "factory retains authored structure kind"
    );
    assert!(
        !world
            .host_object(blocker)
            .unwrap()
            .status
            .under_construction,
        "AtCreation blocker is completed"
    );
    world
        .host_object_mut(blocker)
        .unwrap()
        .jet_ai
        .attackers_miss_expire_frame = 110;
    (world, shooter, blocker)
}

fn batch(world: &mut GameLogic, shooter: ObjectId) -> (Vec<DamageEvent>, Vec<ObjectId>) {
    let id = ObjectId(9001);
    let mut projectile = Projectile::new(
        id,
        Vec3::new(0.0, 20.0, 0.0),
        Vec3::new(100.0, 20.0, 0.0),
        10.0,
        DamageType::Bullet,
        shooter,
        None,
    );
    // Projectile::new is inert with instant speed zero. Install an actual
    // traveling shell: a zero-duration collision phase must inspect its current
    // footprint rather than teleport to its distant aim point.
    projectile.speed = 100.0;
    projectile.velocity = Vec3::new(100.0, 0.0, 0.0);
    world.combat_system.projectiles.insert(id, projectile);
    world.combat_system.prepare_projectile_impacts(
        0.0,
        &mut world.objects,
        None,
        world.frame,
        None,
        &mut world.ownership_events,
    )
}

#[test]
fn same_ids_use_receiving_collision_clock_and_exact_expiry() {
    let (mut owner, shooter, blocker) = receiving_world(109);
    let (mut peer, peer_shooter, peer_blocker) = receiving_world(700);
    assert_eq!((shooter, blocker), (peer_shooter, peer_blocker));
    assert!(batch(&mut owner, shooter).0.is_empty());
    let (events, retired) = batch(&mut peer, peer_shooter);
    assert_eq!(events.len(), 1);
    assert_eq!(retired, vec![ObjectId(9001)]);
    assert!(batch(&mut owner, shooter).0.is_empty());
    owner.frame = 110;
    assert_eq!(batch(&mut owner, shooter).0.len(), 1);
}

#[test]
fn normal_save_preserves_sneaky_collision_expiry_without_peer_retiming() {
    let (mut original, shooter, _) = receiving_world(109);
    let builder = crate::save_load::SnapshotBuilder::new();
    let snapshot = builder.create_world_snapshot(&original).unwrap();
    let bytes = bincode_legacy::serialize(&snapshot).unwrap();
    let decoded = crate::save_load::snapshot::decode_bincode_world_snapshot(&bytes).unwrap();
    let (mut restored, _, _) = receiving_world(700);
    builder
        .restore_from_snapshot(&decoded, &mut restored)
        .unwrap();
    for world in [&mut original, &mut restored] {
        world.frame = 109;
        assert!(batch(world, shooter).0.is_empty());
        world.frame = 110;
        assert_eq!(batch(world, shooter).0.len(), 1);
    }
}

#[test]
fn factory_installs_active_three_slot_deadlines_and_restores_saved_frames() {
    fn factory(frame: u32) -> (GameLogic, ObjectId) {
        let mut world = GameLogic::new();
        world.frame = frame;
        let mut template = ThingTemplate::new("OwnedClockFourSlots");
        template.set_health(100.0).add_kind_of(KindOf::Vehicle);
        let name = crate::game_logic::weapon_bootstrap::RANGER_PRIMARY_WEAPON;
        template.set_primary_weapon_name(name);
        template.set_secondary_weapon_name(name);
        template.set_tertiary_weapon_name(name);
        template.set_mine_clearing_primary_weapon_name(name);
        assert!(
            template.resolve_primary_weapon(frame).is_some(),
            "actual PRIMARY catalog admission"
        );
        assert!(
            template.resolve_secondary_weapon(frame).is_some(),
            "actual SECONDARY catalog admission"
        );
        assert!(
            template.resolve_tertiary_weapon(frame).is_some(),
            "actual TERTIARY catalog admission"
        );
        assert!(
            template
                .resolve_mine_clearing_primary_weapon(frame)
                .is_some(),
            "actual MINE_CLEARING catalog admission"
        );
        world.templates.insert(template.name.clone(), template);
        let id = world
            .create_object("OwnedClockFourSlots", Team::USA, Vec3::ZERO)
            .unwrap();
        (world, id)
    }
    fn deadlines(world: &GameLogic, id: ObjectId) -> [u32; 3] {
        let object = world.host_object(id).unwrap();
        [
            ("PRIMARY", &object.weapon),
            ("SECONDARY", &object.secondary_weapon),
            ("TERTIARY", &object.tertiary_weapon),
        ]
        .map(|(slot, weapon)| {
            weapon
                .as_ref()
                .unwrap_or_else(|| {
                    panic!("factory dropped admitted {slot}: {}", object.template_name)
                })
                .suspend_fx_frame
        })
    }
    let (owner, id) = factory(100);
    let (peer, peer_id) = factory(700);
    assert_eq!(id, peer_id);
    assert_eq!(deadlines(&owner, id), [100; 3]);
    assert_eq!(deadlines(&peer, peer_id), [700; 3]);
    let builder = crate::save_load::SnapshotBuilder::new();
    let snapshot = builder.create_world_snapshot(&owner).unwrap();
    let bytes = bincode_legacy::serialize(&snapshot).unwrap();
    let decoded = crate::save_load::snapshot::decode_bincode_world_snapshot(&bytes).unwrap();
    let (mut restored, _) = factory(900);
    builder
        .restore_from_snapshot(&decoded, &mut restored)
        .unwrap();
    assert_eq!(deadlines(&restored, id), [100; 3]);
}
