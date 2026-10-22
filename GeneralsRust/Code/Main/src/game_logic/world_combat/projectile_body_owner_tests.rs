//! Authored projectile Body initialization precedes real factory callbacks.
use super::*;
use crate::game_logic::{GameLogic, ObjectId, Team, ThingTemplate};
use glam::Vec3;

const SOURCE_OBJECT: &str = "Object OwnedProjectileSource\nKindOf = VEHICLE\nBody = ActiveBody SourceBody\nMaxHealth = 200\nInitialHealth = 200\nEnd\nEnd\n";

fn world(frame: u32) -> (GameLogic, ObjectId) {
    world_with_body(frame, 60.0, false)
}
fn world_with_body(frame: u32, initial: f32, veteran: bool) -> (GameLogic, ObjectId) {
    let mut source = SOURCE_OBJECT.to_owned();
    for name in [
        "CleanupStreamProjectile",
        "GenericTankShell",
        "GLAAngryMobRockProjectileObject",
    ] {
        source.push_str(&projectile_source(name, initial, veteran));
    }
    let content = crate::assets::ReceivingObjectContent::from_sources([(
        "owned-projectile-body.ini".into(),
        source,
    )])
    .unwrap();
    let mut world = GameLogic::new();
    world.admit_receiving_module_content(content).unwrap();
    world.frame = frame;
    for name in [
        "CleanupStreamProjectile",
        "GenericTankShell",
        "GLAAngryMobRockProjectileObject",
    ] {
        let definition = world
            .receiving_object_definitions
            .as_ref()
            .unwrap()
            .get(name)
            .unwrap();
        let body = definition
            .behavior_modules
            .iter()
            .find(|module| module.class_name == "ActiveBody")
            .unwrap();
        assert_eq!(
            game_engine::common::ini::INI::parse_real(body.attribute("InitialHealth").unwrap())
                .unwrap(),
            initial,
            "actual receiving Body block exists before lazy template/creation"
        );
        assert!(
            world
                .receiving_module_content
                .as_ref()
                .unwrap()
                .admitted_template(name)
                .is_some()
        );
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
            (60.0, 200.0, 200.0)
        );
        assert_eq!(object.previous_health, 60.0);
        assert_eq!(object.initial_health, 60.0);
        assert_eq!(
            object.body_damage_state,
            crate::game_logic::host_enum_table_residual::HostBodyDamageType::ReallyDamaged
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

fn projectile_source(name: &str, initial: f32, veteran: bool) -> String {
    let create = if veteran {
        "Behavior = VeterancyGainCreate ModuleTag_BornRank\n StartingLevel = VETERAN\n End"
    } else {
        ""
    };
    format!(
        "Object {name}\n KindOf = PROJECTILE\n IsTrainable = Yes\n Body = ActiveBody ModuleTag_Body\n MaxHealth = 200\n InitialHealth = {initial}\n End\n {create}\nEnd\n"
    )
}

#[test]
fn projectile_factory_veterancy_callback_observes_authored_initial_ratio() {
    let (mut world, source) = world_with_body(101, 60.0, true);
    let ids = spawn(&mut world, source);
    for id in ids {
        let object = world.host_object(id).unwrap();
        assert_eq!(
            object.experience.level,
            crate::game_logic::VeterancyLevel::Veteran,
            "real authored onCreate ran"
        );
        // C++ onVeterancyLevelChanged -> ActiveBody PRESERVE_RATIO reads
        // the initial60/200, before any projectile-specific flight setup.
        assert!((object.health.maximum - 240.0).abs() < 0.001);
        assert!((object.health.current - 72.0).abs() < 0.001);
        assert_eq!(object.previous_health, 60.0);
        assert!((object.initial_health - 240.0).abs() < 0.001);
    }
    let builder = crate::save_load::SnapshotBuilder::new();
    let snapshot = builder.create_world_snapshot(&world).unwrap();
    let bytes = bincode_legacy::serialize(&snapshot).unwrap();
    let decoded = crate::save_load::snapshot::decode_bincode_world_snapshot(&bytes).unwrap();
    let (mut loaded, _) = world_with_body(900, 60.0, true);
    builder
        .restore_from_snapshot(&decoded, &mut loaded)
        .unwrap();
    for id in ids {
        let object = loaded.host_object(id).unwrap();
        assert!(
            (object.health.maximum - 240.0).abs() < 0.001,
            "restore does not replay rank scaling"
        );
        assert!((object.health.current - 72.0).abs() < 0.001);
        assert!((object.initial_health - 240.0).abs() < 0.001);
        assert_eq!(object.previous_health, 60.0);
    }
}

#[test]
fn missing_content_projectiles_keep_retail_definition_fallback() {
    let mut world = GameLogic::new();
    world.templates.insert(
        "OwnedProjectileSource".into(),
        ThingTemplate::new("OwnedProjectileSource"),
    );
    let source = world
        .create_object("OwnedProjectileSource", Team::USA, Vec3::ZERO)
        .unwrap();
    for id in spawn(&mut world, source) {
        let object = world.host_object(id).unwrap();
        assert_eq!(object.health.maximum, 100.0);
        assert_eq!(object.health.current, 100.0);
        assert_eq!(object.previous_health, 100.0);
    }
}

#[test]
fn explicit_zero_initial_body_is_not_replaced_by_projectile_retail_health() {
    let (mut world, source) = world_with_body(102, 0.0, false);
    for id in spawn(&mut world, source) {
        let object = world.host_object(id).unwrap();
        assert_eq!(object.health.maximum, 200.0);
        assert_eq!(object.health.current, 0.0);
        assert_eq!(object.initial_health, 0.0);
        assert_eq!(object.previous_health, 0.0);
        assert_eq!(
            object.body_damage_state,
            crate::game_logic::host_enum_table_residual::HostBodyDamageType::Rubble
        );
        // ActiveBody construction sets the damage state, not kill/death flags.
        assert!(!object.status.effectively_dead);
        assert!(!object.status.on_die_started);
        assert_eq!(object.last_damage_timestamp, None);
        assert_eq!(object.last_healing_timestamp, None);
    }
}

#[test]
fn receiving_projectile_lazy_admission_is_isolated_and_missing_never_bootstraps() {
    let (mut owner, source) = world_with_body(31, 73.0, false);
    let (mut peer, peer_source) = world_with_body(91, 139.0, false);
    assert_eq!(source, peer_source);
    // Exercise the real lazy-cache miss in all three specialized helpers,
    // including rows which an ordinary map seed may already have projected.
    // Their complete immutable receiving definitions remain admitted.
    for name in [
        "CleanupStreamProjectile",
        "GenericTankShell",
        "GLAAngryMobRockProjectileObject",
    ] {
        owner.templates.remove(name);
        peer.templates.remove(name);
    }
    let peer_ids = spawn(&mut peer, peer_source);
    let ids = spawn(&mut owner, source);
    assert_eq!(ids, peer_ids);
    for (id, peer_id) in ids.into_iter().zip(peer_ids) {
        assert_eq!(owner.host_object(id).unwrap().health.current, 73.0);
        assert_eq!(owner.host_object(id).unwrap().initial_health, 73.0);
        assert_eq!(peer.host_object(peer_id).unwrap().health.current, 139.0);
        assert_eq!(peer.host_object(peer_id).unwrap().initial_health, 139.0);
    }

    // The same names now exist in another actual world's lazy template cache.
    // An authoritative receiving miss must not borrow them or the standalone
    // bootstrap; it must reject before adding any projectile/stream state.
    let mut missing = GameLogic::new();
    missing
        .admit_receiving_module_content(
            crate::assets::ReceivingObjectContent::from_sources([(
                "missing-owned-projectile.ini".to_owned(),
                SOURCE_OBJECT.to_owned(),
            )])
            .unwrap(),
        )
        .unwrap();
    let missing_source = missing
        .create_object("OwnedProjectileSource", Team::USA, Vec3::ZERO)
        .unwrap();
    assert_eq!(missing_source, source);
    let aim = Vec3::new(100.0, 0.0, 0.0);
    assert_eq!(
        missing.spawn_cleanup_stream_projectile(missing_source, Vec3::ZERO, aim, 0),
        None
    );
    assert_eq!(
        missing.spawn_angry_mob_projectile(
            missing_source,
            Vec3::ZERO,
            aim,
            None,
            crate::game_logic::host_angry_mob::AngryMobProjectileKind::Rock,
        ),
        None
    );
    assert_eq!(
        missing.spawn_technical_cannon_shell_projectile(missing_source, Vec3::ZERO, aim, None),
        None
    );
    assert_eq!(missing.objects.len(), 1);
    assert_eq!(missing.cleanup_stream_missiles_spawned, 0);
    assert_eq!(missing.angry_mob_projectiles_spawned, 0);
    assert_eq!(missing.technical_cannon_shells_spawned, 0);
    for name in [
        "CleanupStreamProjectile",
        "GenericTankShell",
        "GLAAngryMobRockProjectileObject",
    ] {
        assert!(!missing.templates.contains_key(name));
    }
    for id in peer_ids {
        assert_eq!(peer.host_object(id).unwrap().health.current, 139.0);
    }
}
