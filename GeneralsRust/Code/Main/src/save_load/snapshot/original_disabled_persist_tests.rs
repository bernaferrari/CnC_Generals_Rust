use super::*;
use crate::game_logic::{Team, ThingTemplate};

fn owner() -> (GameLogic, ObjectId) {
    let mut world = GameLogic::new();
    world
        .templates
        .insert("DisabledOwner".into(), ThingTemplate::new("DisabledOwner"));
    let id = world
        .create_object("DisabledOwner", Team::USA, glam::Vec3::ZERO)
        .unwrap();
    (world, id)
}
#[test]
fn original_remaining_types_belong_to_owner_and_refresh_can_shorten() {
    let (mut a, id) = owner();
    let (b, same_id) = owner();
    assert_eq!(id, same_id);
    let object = a.objects.get_mut(&id).unwrap();
    object.set_awestruck_until(100, 10);
    object.set_brainwashed_until(80, 10);
    object.set_status_disabled_held(true);
    assert!(object.has_disabled_type());
    assert!(!object.can_move());
    object.set_awestruck_until(50, 10);
    object.tick_remaining_disabled(49);
    assert!(object.status.disabled_awestruck);
    object.tick_remaining_disabled(50);
    assert!(!object.status.disabled_awestruck);
    assert!(object.status.disabled_brainwashed);
    assert!(object.clear_brainwashed());
    assert!(!object.clear_brainwashed());
    assert!(object.has_disabled_type(), "HELD remains independent");
    object.set_status_disabled_held(false);
    assert!(!object.has_disabled_type());
    object.set_awestruck_until(50, 50);
    assert!(!object.status.disabled_awestruck);
    assert!(!b.objects[&id].has_disabled_type());
    object.set_brainwashed_until(u32::MAX, 50);
    let clone = object.clone();
    object.clear_brainwashed();
    assert!(clone.status.disabled_brainwashed);
    assert!(!object.status.disabled_brainwashed);
    object.set_brainwashed_until(90, 50);
    assert!(object.clear_brainwashed());
    assert_eq!(object.status.disabled_brainwashed_until_frame, 0);
    object.set_brainwashed_until(90, 50);
    assert!(
        object.status.disabled_brainwashed,
        "clear resets NEVER, so a new set of the former deadline re-enables"
    );
    object.set_brainwashed_until(90, 100);
    assert!(
        object.status.disabled_brainwashed,
        "identical deadline is a source no-op until expiry check"
    );
    object.tick_remaining_disabled(100);
    assert!(!object.status.disabled_brainwashed);
}
#[test]
fn encoded_flags_and_deadlines_restore_without_positional_layout_changes() {
    let (mut source, id) = owner();
    let object = source.objects.get_mut(&id).unwrap();
    let positional_before = bincode_legacy::serialize(&object.status).unwrap();
    object.set_awestruck_until(100, 10);
    object.set_brainwashed_until(80, 10);
    // These operations also change existing disable-edge fields; compare new
    // fields to a clone with only those four companion values cleared.
    let mut clear = object.status.clone();
    clear.disabled_awestruck = false;
    clear.disabled_brainwashed = false;
    clear.disabled_awestruck_until_frame = 0;
    clear.disabled_brainwashed_until_frame = 0;
    assert_eq!(
        bincode_legacy::serialize(&object.status).unwrap(),
        bincode_legacy::serialize(&clear).unwrap()
    );
    assert!(!positional_before.is_empty());
    let builder = super::super::SnapshotBuilder::new();
    let snapshot = builder.create_world_snapshot(&source).unwrap();
    let bytes = bincode_legacy::serialize(&snapshot).unwrap();
    let decoded = super::super::decode_bincode_world_snapshot(&bytes).unwrap();
    for mut receiving in [GameLogic::new(), owner().0] {
        builder
            .restore_from_snapshot(&decoded, &mut receiving)
            .unwrap();
        let restored = receiving.objects.get_mut(&id).unwrap();
        assert!(restored.status.disabled_awestruck);
        assert!(restored.status.disabled_brainwashed);
        assert_eq!(restored.status.disabled_awestruck_until_frame, 100);
        assert_eq!(restored.status.disabled_brainwashed_until_frame, 80);
        restored.tick_remaining_disabled(80);
        assert!(!restored.status.disabled_brainwashed);
        assert!(restored.status.disabled_awestruck);
        restored.tick_remaining_disabled(100);
        assert!(!restored.has_disabled_type());
    }
}
#[test]
fn malformed_frame_and_owner_roster_reject_before_receiver_mutation() {
    let (world, id) = owner();
    let snapshot = super::super::SnapshotBuilder::new()
        .create_world_snapshot(&world)
        .unwrap();
    assert!(validate(Some(&[]), &snapshot.objects).is_err());
    let record = Record {
        object_id: id.0,
        awestruck: true,
        brainwashed: false,
        awestruck_until: 9,
        brainwashed_until: 0,
    };
    assert!(validate(Some(&[record.clone(), record.clone()]), &snapshot.objects).is_err());
    let mut unknown = record;
    unknown.object_id += 1;
    assert!(validate(Some(&[unknown]), &snapshot.objects).is_err());
    let mut bytes = super::super::lifecycle_tail::encode_lifecycle_tail(&Default::default());
    let boundary = bytes.len();
    append(&mut bytes, &world).unwrap();
    for length in boundary + 1..bytes.len() {
        assert!(decode(&bytes[..length]).is_err());
    }
    let mut bad = bytes.clone();
    bad[boundary + 4..boundary + 8].copy_from_slice(&2u32.to_le_bytes());
    assert!(decode(&bad).is_err());
    bytes.extend_from_within(boundary..);
    assert!(decode(&bytes).is_err());
}

#[test]
fn both_original_types_defer_native_timer_and_stealth_except_held_override() {
    let mut parser = crate::assets::IniParser::new();
    parser.parse_ini_content("Object DisabledTimer\nBody = ActiveBody BodyTag\nMaxHealth = 100\nInitialHealth = 100\nEnd\nBehavior = LifetimeUpdate TimerTag\nMinLifetime = 100\nMaxLifetime = 100\nEnd\nEnd", "disabled-timer.ini").unwrap();
    for brainwashed in [false, true] {
        let mut world = GameLogic::new();
        world.templates.insert(
            "DisabledTimer".into(),
            GameLogic::build_template_from_object_definition(
                "DisabledTimer",
                parser.get_definition("DisabledTimer").unwrap(),
                None,
            ),
        );
        let id = world
            .create_object("DisabledTimer", Team::USA, glam::Vec3::ZERO)
            .unwrap();
        // ActiveBody's original InitialHealth default is zero; this positive
        // expiry fixture must author a live body before applying any disable.
        assert!(world.objects[&id].is_alive());
        assert_eq!(world.objects[&id].health.current, 100.0);
        world.frame = 10;
        let object = world.objects.get_mut(&id).unwrap();
        if brainwashed {
            object.set_brainwashed_until(20, 10);
        } else {
            object.set_awestruck_until(20, 10);
        }
        assert!(!object.stealth_or_detector_update_processes());
        object.set_status_disabled_held(true);
        assert!(
            object.stealth_or_detector_update_processes(),
            "source StealthUpdate mask processes HELD even mixed"
        );
        object.set_status_disabled_held(false);
        world.tick_native_lifetime_timers(id);
        assert!(world.objects[&id].is_alive());
        assert!(
            world.objects[&id]
                .native_lifecycle
                .as_ref()
                .unwrap()
                .states
                .iter()
                .any(|state| matches!(
                    state,
                    crate::game_logic::native_lifecycle::NativeLifecycleState::Lifetime {
                        active: true,
                        ..
                    }
                ))
        );
        world
            .objects
            .get_mut(&id)
            .unwrap()
            .tick_remaining_disabled(20);
        assert!(world.objects[&id].stealth_or_detector_update_processes());
        world.tick_native_lifetime_timers(id);
        assert!(!world.objects[&id].is_alive());
    }
}
#[test]
fn malformed_complete_roster_leaves_direct_receiver_unchanged() {
    let (source, _) = owner();
    let builder = super::super::SnapshotBuilder::new();
    let mut snapshot = builder.create_world_snapshot(&source).unwrap();
    snapshot.lifecycle_tail = super::super::lifecycle_tail::encode_lifecycle_tail(
        &super::super::lifecycle_tail::capture_lifecycle_tail(&source).unwrap(),
    );
    super::super::combat_projectile_persist::append(&mut snapshot.lifecycle_tail, &source).unwrap();
    super::super::body_kind_persist::append(&mut snapshot.lifecycle_tail, &source).unwrap();
    super::super::native_lifecycle_persist::append(&mut snapshot.lifecycle_tail, &source).unwrap();
    let payload = bincode_legacy::serialize(&Vec::<Record>::new()).unwrap();
    snapshot.lifecycle_tail.extend_from_slice(TAG);
    snapshot
        .lifecycle_tail
        .extend_from_slice(&VERSION.to_le_bytes());
    snapshot
        .lifecycle_tail
        .extend_from_slice(&(payload.len() as u32).to_le_bytes());
    snapshot.lifecycle_tail.extend_from_slice(&payload);
    let (mut receiver, id) = owner();
    receiver.frame = 99;
    receiver
        .objects
        .get_mut(&id)
        .unwrap()
        .set_awestruck_until(199, 99);
    assert!(
        builder
            .restore_from_snapshot(&snapshot, &mut receiver)
            .is_err()
    );
    assert_eq!(receiver.frame, 99);
    assert!(receiver.objects[&id].status.disabled_awestruck);
    assert_eq!(
        receiver.objects[&id].status.disabled_awestruck_until_frame,
        199
    );
}
