//! Real SnapshotBuilder admission of definitions excluded from the host cache.
use super::*;
use crate::game_logic::{ObjectId, Team};
use crate::save_load::snapshot::{SnapshotBuilder, WorldSnapshot};
use glam::Vec3;

const LAZY: &str = "GLAAngryMobRockProjectileObject";
const KEEP: &str = "ReceivingRestoreWitness";

fn content(body: &str, lifetime: u32, die_tag: &str, include_lazy: bool) -> String {
    let mut text = format!(
        "Object {KEEP}\nKindOf = VEHICLE\nBody = ActiveBody WitnessBody\nMaxHealth = 100\nInitialHealth = 100\nEnd\nEnd\n"
    );
    if include_lazy {
        let health = if body == "InactiveBody" {
            ""
        } else {
            "MaxHealth = 100\nInitialHealth = 60\n"
        };
        text.push_str(&format!("Object {LAZY}\nKindOf = PROJECTILE\nBody = {body} ProjectileBody\n{health}End\nBehavior = LifetimeUpdate Life\nMinLifetime = {lifetime}\nMaxLifetime = {lifetime}\nEnd\nBehavior = KeepObjectDie {die_tag}\nEnd\nEnd\n"));
    }
    text
}
fn world(text: String) -> (GameLogic, ObjectId) {
    let mut world = GameLogic::new();
    world
        .admit_receiving_module_content(
            crate::assets::ReceivingObjectContent::from_sources([(
                "restore-lazy.ini".into(),
                text,
            )])
            .unwrap(),
        )
        .unwrap();
    assert!(
        !world.templates.contains_key(LAZY),
        "fixture must exercise the uncached receiving definition"
    );
    let id = world
        .create_object(KEEP, Team::USA, Vec3::new(7., 8., 9.))
        .unwrap();
    world.frame = 915;
    (world, id)
}
fn saved() -> (WorldSnapshot, ObjectId) {
    let (mut source, _) = world(content("ActiveBody", 5000, "KeepA", true));
    assert!(source.ensure_host_spawn_template(LAZY));
    let id = source
        .create_object(LAZY, Team::USA, Vec3::new(1., 2., 3.))
        .unwrap();
    let snapshot = SnapshotBuilder::new()
        .create_world_snapshot(&source)
        .unwrap();
    let encoded = bincode_legacy::serialize(&snapshot).unwrap();
    (
        crate::save_load::snapshot::decode_bincode_world_snapshot(&encoded).unwrap(),
        id,
    )
}

#[test]
fn detached_receiving_definition_preflight_restores_an_uncached_object() {
    let (snapshot, id) = saved();
    let (mut receiver, _) = world(content("ActiveBody", 5000, "KeepA", true));
    let (peer, peer_id) = world(content("ActiveBody", 9000, "KeepPeer", true));
    let keys: std::collections::HashSet<_> = receiver.templates.keys().cloned().collect();
    let resolved = resolve(&receiver, LAZY).unwrap().unwrap();
    assert!(matches!(resolved, Cow::Owned(_)));
    assert_eq!(
        resolved.body_kind,
        Some(crate::game_logic::thing::BodyKind::Active)
    );
    drop(resolved);
    assert_eq!(keys, receiver.templates.keys().cloned().collect());
    SnapshotBuilder::new()
        .restore_from_snapshot(&snapshot, &mut receiver)
        .unwrap();
    assert_eq!(receiver.host_object(id).unwrap().health.current, 60.);
    assert_eq!(
        receiver.host_object(id).unwrap().get_position(),
        Vec3::new(1., 2., 3.)
    );
    assert!(receiver.host_object(id).unwrap().native_lifecycle.is_some());
    assert!(!peer.templates.contains_key(LAZY));
    assert_eq!(peer.host_objects().len(), 1);
    assert_eq!(
        peer.host_object(peer_id).unwrap().get_position(),
        Vec3::new(7., 8., 9.)
    );
}

fn rejected(text: String, message: &str) {
    let (snapshot, _) = saved();
    let (mut receiver, id) = world(text);
    let (peer, peer_id) = world(content("ActiveBody", 9000, "KeepPeer", true));
    let keys: std::collections::HashSet<_> = receiver.templates.keys().cloned().collect();
    let error = SnapshotBuilder::new()
        .restore_from_snapshot(&snapshot, &mut receiver)
        .unwrap_err();
    assert!(
        error.to_string().contains(message),
        "unexpected rejection: {error}"
    );
    assert_eq!(receiver.frame, 915);
    assert_eq!(receiver.host_objects().len(), 1);
    assert_eq!(
        receiver.host_object(id).unwrap().get_position(),
        Vec3::new(7., 8., 9.)
    );
    assert_eq!(receiver.host_object(id).unwrap().health.current, 100.);
    assert_eq!(keys, receiver.templates.keys().cloned().collect());
    assert!(
        !receiver.templates.contains_key(LAZY),
        "preflight rejection must not prewarm the live cache"
    );
    assert!(!peer.templates.contains_key(LAZY));
    assert_eq!(peer.frame, 915);
    assert_eq!(peer.host_objects().len(), 1);
    assert_eq!(
        peer.host_object(peer_id).unwrap().get_position(),
        Vec3::new(7., 8., 9.)
    );
    assert_eq!(peer.host_object(peer_id).unwrap().health.current, 100.);
}
#[test]
fn uncached_receiving_body_mismatch_rejects_without_mutation() {
    rejected(
        content("InactiveBody", 5000, "KeepA", true),
        "Body class differs",
    );
}
#[test]
fn uncached_receiving_lifecycle_mismatch_rejects_without_mutation() {
    rejected(
        content("ActiveBody", 6000, "KeepA", true),
        "receiving definitions differ",
    );
}
#[test]
fn uncached_receiving_die_mismatch_rejects_without_mutation() {
    rejected(
        content("ActiveBody", 5000, "KeepB", true),
        "receiving Die declarations differ",
    );
}
#[test]
fn absent_receiving_definition_rejects_without_mutation() {
    rejected(
        content("ActiveBody", 5000, "KeepA", false),
        "absent from receiving content",
    );
}

#[test]
fn uncached_malformed_receiving_body_rejects_before_roster_clear() {
    assert!(
        game_engine::common::ini::INI::parse_real("NaN")
            .unwrap()
            .is_nan(),
        "lexically valid authored real reaches Main semantic finite-value validation"
    );
    rejected(
        content("ActiveBody", 5000, "KeepA", true)
            .replace("InitialHealth = 60", "InitialHealth = NaN"),
        "receiving Body definition invalid",
    );
}
#[test]
fn uncached_malformed_receiving_lifecycle_rejects_before_roster_clear() {
    rejected(
        content("ActiveBody", 5000, "KeepA", true)
            .replace("MinLifetime = 5000", "MinLifetime = 6000"),
        "receiving lifecycle definitions invalid",
    );
}
