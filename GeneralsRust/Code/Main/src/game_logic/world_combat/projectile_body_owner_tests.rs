//! Admitted projectile initialization uses ordinary Body math, without damage callbacks.
use super::*;
use crate::game_logic::{GameLogic, KindOf, ObjectId, Team, ThingTemplate};
use glam::Vec3;

fn world(frame: u32) -> (GameLogic, ObjectId) {
    let mut world = GameLogic::new();
    world.frame = frame;
    for name in [
        "OwnedProjectileSource",
        "CleanupStreamProjectile",
        "GenericTankShell",
        "GLAAngryMobRockProjectileObject",
    ] {
        let mut template = ThingTemplate::new(name);
        template.set_health(200.0).add_kind_of(KindOf::Projectile);
        world.templates.insert(name.into(), template);
    }
    let source = world
        .create_object("OwnedProjectileSource", Team::USA, Vec3::ZERO)
        .unwrap();
    (world, source)
}
fn spawn(world: &mut GameLogic, source: ObjectId) -> [ObjectId; 3] {
    let aim = Vec3::new(100.0, 0.0, 0.0);
    [
        world
            .spawn_cleanup_stream_projectile(source, Vec3::ZERO, aim, 0)
            .unwrap(),
        world
            .spawn_angry_mob_projectile(
                source,
                Vec3::ZERO,
                aim,
                None,
                crate::game_logic::host_angry_mob::AngryMobProjectileKind::Rock,
            )
            .unwrap(),
        world
            .spawn_technical_cannon_shell_projectile(source, Vec3::ZERO, aim, None)
            .unwrap(),
    ]
}
fn assert_body(world: &GameLogic, ids: [ObjectId; 3]) {
    for id in ids {
        let object = world.host_object(id).unwrap();
        assert_eq!(
            (
                object.health.current,
                object.health.maximum,
                object.max_health
            ),
            (100.0, 100.0, 100.0)
        );
        assert_eq!(object.previous_health, 200.0);
        assert_eq!(
            object.body_damage_state,
            crate::game_logic::host_enum_table_residual::HostBodyDamageType::Pristine
        );
        assert!(!object.status.effectively_dead);
        assert!(!object.status.on_die_started);
        assert_eq!(object.last_damage_timestamp, None);
        assert_eq!(object.last_healing_timestamp, None);
    }
}
#[test]
fn admitted_stream_mob_cannon_body_state_and_save_are_canonical() {
    let (mut owner, source) = world(100);
    let (mut peer, peer_source) = world(700);
    assert_eq!(source, peer_source);
    let ids = spawn(&mut owner, source);
    let peer_ids = spawn(&mut peer, peer_source);
    assert_eq!(ids, peer_ids);
    assert_body(&owner, ids);
    assert_body(&peer, peer_ids);
    let builder = crate::save_load::SnapshotBuilder::new();
    let snapshot = builder.create_world_snapshot(&owner).unwrap();
    let encoded = bincode_legacy::serialize(&snapshot).unwrap();
    let decoded = crate::save_load::snapshot::decode_bincode_world_snapshot(&encoded).unwrap();
    let (mut restored, _) = world(900);
    builder
        .restore_from_snapshot(&decoded, &mut restored)
        .unwrap();
    assert_body(&restored, ids);
}
