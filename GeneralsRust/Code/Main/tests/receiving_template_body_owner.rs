//! Receiving definitions, not foreign cached templates, own body admission.
use generals_main::game_logic::{GameLogic, Team};
use generals_main::save_load::SnapshotBuilder;
use glam::Vec3;

const OBJECT: &str = "ReceivingBodyProbe";
const WITNESS: &str = "ReceivingBodyWitness";

fn world(active: bool) -> GameLogic {
    let body = if active {
        "Body = ActiveBody Body\n MaxHealth = 100\n InitialHealth = 60\n End"
    } else {
        "Body = InactiveBody Body\n End"
    };
    let source = format!(
        "Object {OBJECT}\n KindOf = VEHICLE\n {body}\nEnd\n\
         Object {WITNESS}\n KindOf = VEHICLE\n Body = ActiveBody Body\n MaxHealth = 100\n InitialHealth = 100\n End\nEnd\n"
    );
    let mut game = GameLogic::new();
    game.admit_object_ini_sources([("BodyOwner.ini".into(), source)])
        .unwrap();
    game
}

#[test]
fn foreign_cached_template_does_not_override_receiving_body_on_creation() {
    let source = world(true);
    let mut receiver = world(false);
    receiver
        .templates
        .insert(OBJECT.into(), source.templates.get(OBJECT).unwrap().clone());
    let id = receiver
        .create_object(OBJECT, Team::USA, Vec3::ZERO)
        .unwrap();
    assert!(receiver.host_object(id).unwrap().is_inactive_body());
}

#[test]
fn same_owner_template_edits_remain_on_creation() {
    let mut owner = world(true);
    owner.templates.get_mut(OBJECT).unwrap().set_health(180.0);
    let id = owner.create_object(OBJECT, Team::USA, Vec3::ZERO).unwrap();
    assert_eq!(owner.host_object(id).unwrap().health.maximum, 180.0);
}

#[test]
fn save_preflight_rejects_foreign_body_without_mutating_either_session() {
    let mut source = world(true);
    let source_id = source.create_object(OBJECT, Team::USA, Vec3::ZERO).unwrap();
    let snapshot = SnapshotBuilder::new()
        .create_world_snapshot(&source)
        .unwrap();
    let mut receiver = world(false);
    let witness = receiver
        .create_object(WITNESS, Team::USA, Vec3::new(12.0, 0.0, 16.0))
        .unwrap();
    assert_eq!(source_id, witness);
    receiver
        .templates
        .insert(OBJECT.into(), source.templates.get(OBJECT).unwrap().clone());
    let error = SnapshotBuilder::new()
        .restore_from_snapshot(&snapshot, &mut receiver)
        .unwrap_err();
    assert!(error.to_string().contains("Body class differs"), "{error}");
    assert_eq!(receiver.host_objects().registered_len(), 1);
    assert_eq!(
        receiver.host_object(witness).unwrap().get_position(),
        Vec3::new(12.0, 0.0, 16.0)
    );
    assert_eq!(source.host_object(source_id).unwrap().health.current, 60.0);
}

#[test]
fn compatible_save_ignores_foreign_cached_body() {
    let mut source = world(true);
    let id = source.create_object(OBJECT, Team::USA, Vec3::ZERO).unwrap();
    let snapshot = SnapshotBuilder::new()
        .create_world_snapshot(&source)
        .unwrap();
    let foreign = world(false);
    let mut receiver = world(true);
    receiver.templates.insert(
        OBJECT.into(),
        foreign.templates.get(OBJECT).unwrap().clone(),
    );
    SnapshotBuilder::new()
        .restore_from_snapshot(&snapshot, &mut receiver)
        .unwrap();
    let restored = receiver.host_object(id).unwrap();
    assert!(!restored.is_inactive_body());
    assert_eq!(restored.health.current, 60.0);
}
