//! Shots are accepted, stepped, and retired by the receiving CombatSystem.
use crate::fixture_content::{FixtureObject, FixtureWeapon, admit_fixture_content};
use crate::game_logic::combat::pending_projectile_queue_len_for_test;
use crate::game_logic::{GameLogic, ObjectAudio, ObjectId, Team, Weapon};
use crate::presentation_frame::{PresentationFrame, PresentationProjectile};
use glam::Vec3;

const STEP: f32 = 1.0 / 30.0;

fn seed_homing_projectile(world: &mut GameLogic, origin: Vec3, damage: f32) -> ObjectId {
    let target = origin + Vec3::X * 90.0;
    let id = world.combat_system.fire_projectile(
        origin,
        target,
        &Weapon {
            damage,
            range: 500.0,
            ..Default::default()
        },
        ObjectId(7),
        Some(ObjectId(8)),
        200.0,
    );
    let shot = world.combat_system.projectile_mut(id).unwrap();
    shot.velocity = Vec3::X * 5.0;
    shot.lifetime = 0.5;
    shot.max_lifetime = 3.0;
    shot.is_homing = true;
    id
}

fn projectile_row(frame: &PresentationFrame, id: ObjectId) -> &PresentationProjectile {
    frame.projectiles.iter().find(|row| row.id == id).unwrap()
}

#[test]
fn projectile_frames_freeze_each_worlds_flight_and_retirement() {
    let mut owner = GameLogic::new();
    let mut other = GameLogic::new();
    let origin = Vec3::new(10.0, 15.0, 20.0);
    let peer_origin = origin + Vec3::Z * 70.0;
    let id = seed_homing_projectile(&mut owner, origin, 25.0);
    let other_id = seed_homing_projectile(&mut other, peer_origin, 31.0);
    assert_eq!(id, other_id);

    let before = PresentationFrame::try_build_from_logic(&mut owner, 0).unwrap();
    let original = projectile_row(&before, id);
    assert_eq!(original.position, origin);
    assert_eq!(original.velocity, Vec3::X * 5.0);
    assert_eq!(original.target_position, origin + Vec3::X * 90.0);
    assert_eq!(original.shooter_id, ObjectId(7));
    assert_eq!(original.target_id, Some(ObjectId(8)));
    assert_eq!(original.damage, 25.0);
    assert_eq!(original.lifetime, 0.5);
    assert_eq!(original.max_lifetime, 3.0);
    assert!(original.is_homing);
    assert_eq!(owner.combat_system.get_projectiles()[&id].speed, 200.0);

    for world in [&mut owner, &mut other] {
        world.set_current_frame(16);
        assert!(
            world
                .update_owned_projectile_impacts(STEP)
                .unwrap()
                .is_empty()
        );
    }
    let after = PresentationFrame::try_build_from_logic(&mut owner, 0).unwrap();
    let advanced = projectile_row(&after, id);
    assert!(advanced.position.x > origin.x);
    assert_eq!(
        advanced.position,
        owner.combat_system.get_projectiles()[&id].position
    );
    assert!((advanced.lifetime - (0.5 + STEP)).abs() < 1e-6);
    assert_eq!(original.position, origin);
    assert_eq!(original.lifetime, 0.5);

    assert!(owner.combat_system.remove_projectile(id));
    let retired = PresentationFrame::try_build_from_logic(&mut owner, 0).unwrap();
    let peer = PresentationFrame::try_build_from_logic(&mut other, 0).unwrap();
    assert!(retired.projectiles.is_empty());
    assert_eq!(projectile_row(&peer, id).damage, 31.0);
    assert_eq!(projectile_row(&peer, id).position.z, peer_origin.z);
    assert!(advanced.position.x > origin.x);
}

fn armed_world(damage: f32) -> (GameLogic, ObjectId, ObjectId) {
    const SHOOTER: &str = "PublicationShooter";
    const VICTIM: &str = "PublicationVictim";
    const SHELL: &str = "PublicationShell";
    const GUN: &str = "PublicationGun";
    let mut world = GameLogic::new();
    // DumbProjectileBehavior builds its arc above the highest terrain sample.
    let mut map = game_engine::map_data::MapData::new();
    map.width = 32;
    map.height = 32;
    map.heightmap = vec![0; 32 * 32];
    map.boundaries = vec![glam::IVec2::new(32, 32)];
    world
        .world_services
        .terrain()
        .write()
        .unwrap()
        .try_load_map_geometry(map)
        .unwrap();
    world.initialize_receiving_partition_grid().unwrap();
    admit_fixture_content(
        &mut world,
        &[
            FixtureObject::new(SHOOTER)
                .kind_of("INFANTRY CAN_ATTACK SELECTABLE")
                .health(100.0)
                .weapon("SECONDARY", GUN),
            FixtureObject::new(VICTIM)
                .kind_of("INFANTRY SELECTABLE")
                .health(100.0),
            FixtureObject::new(SHELL)
                .kind_of("PROJECTILE")
                .health(1.0)
                .model("PublicationShellModel")
                .module(
                    "Behavior = DumbProjectileBehavior Flight\n FirstHeight = 5\n SecondHeight = 5\n FirstPercentIndent = 50%\n SecondPercentIndent = 90%\nEnd",
                ),
        ],
        &[FixtureWeapon::new(GUN)
            .damage(damage)
            .range(150.0)
            .delay_ms(0)
            .damage_type("UNRESISTABLE")
            .field("AntiGround = Yes")
            .field("WeaponSpeed = 100")
            .field("ProjectileObject = PublicationShell")],
    );
    let shooter = world
        .create_object(SHOOTER, Team::USA, Vec3::new(10.0, 15.0, 20.0))
        .unwrap();
    let victim = world
        .create_object(VICTIM, Team::China, Vec3::new(70.0, 15.0, 20.0))
        .unwrap();
    world
        .host_object_mut(shooter)
        .unwrap()
        .set_active_weapon_slot(1);
    (world, shooter, victim)
}

#[test]
fn accepted_shot_materializes_in_its_receiving_world() {
    let (mut world, source, target) = armed_world(12.0);
    let (mut peer, peer_source, peer_target) = armed_world(19.0);
    assert_eq!((source, target), (peer_source, peer_target));
    world.set_current_frame(77);
    let shooter = world.objects.get_mut(&source).unwrap();
    assert!(shooter.fire_at(
        target,
        9.5,
        world.frame,
        &mut world.combat_system,
        &mut world.attack_events,
        &mut ObjectAudio::record(&mut Vec::new()),
    ));
    assert_eq!(shooter.last_fire_victim_host, target.0);
    assert_eq!(shooter.last_fire_slot, 1);
    assert_eq!(shooter.last_fire_damage, 12.0);
    assert_eq!(shooter.last_fire_range, 150.0);
    assert_eq!(shooter.last_fire_sim_time, 9.5);
    assert_eq!(shooter.last_fire_frame, 77);
    assert_eq!(shooter.fire_intent_count, 1);
    assert_eq!(shooter.weapon_slot(1).unwrap().last_fire_time, 9.5);
    assert_eq!(
        pending_projectile_queue_len_for_test(&world.combat_system),
        1
    );
    let before = PresentationFrame::try_build_from_logic(&mut world, 0).unwrap();
    assert!(before.projectiles.is_empty());

    peer.drain_pending_projectiles_into_combat();
    assert!(peer.combat_system.get_projectiles().is_empty());
    assert_eq!(peer.host_object(peer_source).unwrap().fire_intent_count, 0);
    assert_eq!(
        pending_projectile_queue_len_for_test(&world.combat_system),
        1
    );
    world.drain_pending_projectiles_into_combat();
    assert_eq!(
        pending_projectile_queue_len_for_test(&world.combat_system),
        0
    );
    let view = PresentationFrame::try_build_from_logic(&mut world, 0).unwrap();
    let [shot] = view.projectiles.as_slice() else {
        panic!(
            "expected one published flight, got {}",
            view.projectiles.len()
        );
    };
    assert_eq!(shot.shooter_id, source);
    assert_eq!(shot.target_id, Some(target));
    assert_eq!(shot.damage, 12.0);
    assert_eq!(shot.projectile_object_name, "PublicationShell");
    assert!(shot.model_key.eq_ignore_ascii_case("PublicationShellModel"));
    assert!(before.projectiles.is_empty());

    world.drain_pending_projectiles_into_combat();
    let repeated = PresentationFrame::try_build_from_logic(&mut world, 0).unwrap();
    assert_eq!(repeated.projectiles, view.projectiles);
    assert_eq!(world.host_object(source).unwrap().fire_intent_count, 1);
}
