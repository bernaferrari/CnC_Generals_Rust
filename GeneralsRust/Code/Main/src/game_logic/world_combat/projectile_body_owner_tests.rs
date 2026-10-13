//! Authored projectile Body initialization precedes real factory callbacks.
use super::*;
use crate::game_logic::{GameLogic, KindOf, ObjectId, Team, ThingTemplate};
use glam::Vec3;

fn world(frame: u32) -> (GameLogic, ObjectId) {
    let mut world = GameLogic::new();
    world.frame = frame;
    let mut source_template = ThingTemplate::new("OwnedProjectileSource");
    source_template
        .set_health(200.0)
        .add_kind_of(KindOf::Vehicle);
    world
        .templates
        .insert("OwnedProjectileSource".into(), source_template);
    for name in [
        "CleanupStreamProjectile",
        "GenericTankShell",
        "GLAAngryMobRockProjectileObject",
    ] {
        world
            .templates
            .insert(name.into(), authored_projectile(name, false));
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

fn authored_projectile(name: &str, veteran: bool) -> ThingTemplate {
    let create = if veteran {
        "Behavior = VeterancyGainCreate ModuleTag_BornRank\n StartingLevel = VETERAN\n End"
    } else {
        ""
    };
    let ini = format!(
        "Object {name}\n KindOf = PROJECTILE\n IsTrainable = Yes\n Body = ActiveBody ModuleTag_Body\n MaxHealth = 200\n InitialHealth = 60\n End\n {create}\nEnd"
    );
    let mut parser = crate::assets::IniParser::new();
    parser
        .parse_ini_content(&ini, "owned-projectile-body.ini")
        .unwrap();
    let template = GameLogic::build_template_from_object_definition(
        name,
        parser.get_definition(name).unwrap(),
        None,
    );
    assert_eq!(
        template.body_initial_health,
        Some(60.0),
        "actual Body block is bound before creation"
    );
    template
}

#[test]
fn projectile_factory_veterancy_callback_observes_authored_initial_ratio() {
    let (mut world, source) = world(101);
    for name in [
        "CleanupStreamProjectile",
        "GenericTankShell",
        "GLAAngryMobRockProjectileObject",
    ] {
        world
            .templates
            .insert(name.into(), authored_projectile(name, true));
    }
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
    let mut loaded = GameLogic::new();
    loaded.templates = world.templates.clone();
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
    let (mut world, source) = world(102);
    for name in [
        "CleanupStreamProjectile",
        "GenericTankShell",
        "GLAAngryMobRockProjectileObject",
    ] {
        world.templates.get_mut(name).unwrap().body_initial_health = Some(0.0);
    }
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
