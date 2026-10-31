//! Actual clone mutation cannot borrow the source actor's partition/trigger rights.
use crate::game_logic::{GameLogic, ObjectId, Player, Team, ThingTemplate};
use glam::Vec3;

fn configure_grid(world: &mut GameLogic) {
    let services = world.world_services.clone();
    let mut shroud = services.shroud().lock().unwrap();
    shroud
        .init_shroud_grid_at_origin_with_cell_size([0.0, 0.0], 2000.0, 2000.0, 40.0)
        .unwrap();
    world.partition_manager.configure_grid_from_shroud(&shroud);
}
fn world(position: Vec3) -> (GameLogic, ObjectId) {
    let mut world = GameLogic::new();
    world.add_player(Player::new(0, Team::USA, "CloneAttachmentOwner", true));
    configure_grid(&mut world);
    let mut template = ThingTemplate::new("RuntimeAttachmentCloneProbe");
    template.sight_range = 40.0;
    template.shroud_clearing_range = 40.0;
    template.build_cost.supplies = 50;
    world.templates.insert(template.name.clone(), template);
    let id = world
        .create_object_for_player("RuntimeAttachmentCloneProbe", 0, position)
        .unwrap();
    world.update_owned_partition_with_effects(&mut Default::default()).unwrap();
    (world, id)
}
fn partition_bytes(world: &GameLogic) -> Vec<u8> {
    bincode_legacy::serialize(&world.capture_partition_continuation().unwrap()).unwrap()
}
fn cash_value(world: &GameLogic, position: Vec3) -> u32 {
    world
        .world_services
        .shroud()
        .lock()
        .unwrap()
        .host_scalar_value_at(
            &gamelogic::common::Coord3D::new(position.x, position.z, 0.0),
            0,
            gamelogic::object::collide::partition_manager::ValueOrThreat::CashValue,
        )
        .unwrap()
}
fn trigger_bytes(world: &GameLogic) -> Vec<u8> {
    let capture = world.host_trigger_world.lock().unwrap().capture();
    let values: Vec<_> = capture
        .iter()
        .map(|object| {
            (
                object.object_id,
                object.i_x,
                object.i_y,
                object.entered_or_exited_frame,
                object
                    .slots
                    .iter()
                    .map(|slot| {
                        (
                            slot.trigger_id,
                            slot.trigger_name.as_str(),
                            slot.is_inside,
                            slot.entered,
                            slot.exited,
                        )
                    })
                    .collect::<Vec<_>>(),
            )
        })
        .collect();
    bincode_legacy::serialize(&values).unwrap()
}
#[test]
fn detached_object_clone_pose_preserves_same_id_source_partition_and_trigger_owners() {
    let (mut source, id) = world(Vec3::new(120.0, 0.0, 140.0));
    let (peer, peer_id) = world(Vec3::new(320.0, 0.0, 340.0));
    assert_eq!(id, peer_id);
    let source_partition = partition_bytes(&source);
    let peer_partition = partition_bytes(&peer);
    let source_trigger = trigger_bytes(&source);
    let peer_trigger = trigger_bytes(&peer);
    let source_position = source.host_object(id).unwrap().get_position();
    let mut detached = source.host_object(id).unwrap().clone();
    assert!(detached.partition_namespace.is_detached_clone());
    assert!(detached.host_trigger_world.upgrade().is_none());
    detached.set_position(Vec3::new(820.0, 3.0, 840.0));
    detached.set_orientation(0.75);
    detached.set_transform_matrix(glam::Mat4::from_translation(Vec3::new(920.0, 4.0, 940.0)));
    assert_eq!(
        source.host_object(id).unwrap().get_position(),
        source_position
    );
    assert_eq!(partition_bytes(&source), source_partition);
    assert_eq!(partition_bytes(&peer), peer_partition);
    assert_eq!(trigger_bytes(&source), source_trigger);
    assert_eq!(trigger_bytes(&peer), peer_trigger);

    // An explicit source PM does not turn an observation into an admitted actor.
    let local_position = detached.get_position();
    let rejected = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        detached.set_position_with_partition_owner(
            Vec3::ZERO,
            0,
            &mut Default::default(),
            &mut source.partition_manager,
        );
    }));
    assert!(rejected.is_err());
    assert_eq!(detached.get_position(), local_position);
    assert_eq!(partition_bytes(&source), source_partition);
    // The original registered actor still requires its exact driving owner.
    let rejected = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        source.host_object_mut(id).unwrap().set_position(Vec3::ZERO);
    }));
    assert!(rejected.is_err());
    assert_eq!(partition_bytes(&source), source_partition);
    assert_eq!(
        source.host_object(id).unwrap().get_position(),
        source_position
    );
    drop(peer);
    drop(detached);
    assert_eq!(partition_bytes(&source), source_partition);
    assert_eq!(trigger_bytes(&source), source_trigger);
}
#[test]
fn actual_clone_admission_rebinds_runtime_attachments_with_new_birth_and_clean_scalar_receipt() {
    let (source, id) = world(Vec3::new(150.0, 0.0, 170.0));
    let source_partition = partition_bytes(&source);
    let source_trigger = trigger_bytes(&source);
    assert!(
        source
            .host_object(id)
            .unwrap()
            .partition_last_affect_applied
    );
    let copied = source.host_object(id).unwrap().clone();
    let old_birth = copied.visual_draw_identity_token.clone();
    let mut receiver = GameLogic::new();
    receiver.add_player(Player::new(0, Team::USA, "CloneReceiver", true));
    configure_grid(&mut receiver);
    receiver.add_object(copied);
    let admitted = receiver.host_object(id).unwrap();
    assert!(!admitted.partition_namespace.is_detached_clone());
    assert!(!std::sync::Arc::ptr_eq(
        &old_birth,
        &admitted.visual_draw_identity_token
    ));
    assert!(!admitted.partition_last_affect_applied);
    assert!(admitted.host_trigger_world.upgrade().is_some());
    receiver.set_fixture_object_position(id, Vec3::new(550.0, 0.0, 570.0));
    receiver.update_owned_partition_with_effects(&mut Default::default()).unwrap();
    assert_eq!(
        cash_value(&receiver, Vec3::new(150.0, 0.0, 170.0)),
        0,
        "cloned applied receipt may not subtract from a new grid"
    );
    assert_eq!(cash_value(&receiver, Vec3::new(550.0, 0.0, 570.0)), 50);
    assert_eq!(
        source.host_object(id).unwrap().get_position(),
        Vec3::new(150.0, 0.0, 170.0)
    );
    assert_eq!(partition_bytes(&source), source_partition);
    assert_eq!(trigger_bytes(&source), source_trigger);
}

#[test]
fn physical_retirement_detaches_runtime_leases_before_returning_object() {
    let (mut source, id) = world(Vec3::new(180.0, 0.0, 190.0));
    let mut retired = source
        .take_object_for_physical_retirement(id, &mut Default::default())
        .unwrap()
        .unwrap();
    assert!(source.host_object(id).is_none());
    assert!(retired.partition_namespace.upgrade().is_none());
    assert!(retired.host_trigger_world.upgrade().is_none());
    let partition = partition_bytes(&source);
    let trigger = trigger_bytes(&source);
    retired.set_position(Vec3::new(1180.0, 5.0, 1190.0));
    assert_eq!(partition_bytes(&source), partition);
    assert_eq!(trigger_bytes(&source), trigger);
}

#[test]
fn encoded_lifecycle_continuation_moves_same_birth_attachment_rights_without_touching_peer() {
    use crate::save_load::snapshot::{apply_lifecycle_tail_to_host, capture_lifecycle_tail};
    use crate::save_load::snapshot::{decode_lifecycle_tail, encode_lifecycle_tail};
    let (mut owner, id) = world(Vec3::new(120.0, 0.0, 140.0));
    let (peer, peer_id) = world(Vec3::new(320.0, 0.0, 340.0));
    assert_eq!(id, peer_id);
    let peer_partition = partition_bytes(&peer);
    let peer_trigger = trigger_bytes(&peer);
    let partition_before = partition_bytes(&owner);
    let generation = owner.host_object(id).unwrap().visual_object_generation;
    let identity = owner
        .host_object(id)
        .unwrap()
        .visual_draw_identity_token
        .clone();
    let tail = capture_lifecycle_tail(&owner).unwrap();
    let decoded = decode_lifecycle_tail(&encode_lifecycle_tail(&tail)).unwrap();
    // Duplicate envelopes retain the existing ordered last-value-wins policy.
    let mut duplicate = decoded.clone();
    duplicate.envelopes.extend(decoded.envelopes);
    apply_lifecycle_tail_to_host(&duplicate, &mut owner).unwrap();
    let installed = owner.host_object(id).unwrap();
    assert_eq!(installed.visual_object_generation, generation);
    assert!(std::sync::Arc::ptr_eq(
        &installed.visual_draw_identity_token,
        &identity
    ));
    assert!(!installed.partition_namespace.is_detached_clone());
    assert!(installed.host_trigger_world.upgrade().is_some());
    assert_eq!(partition_bytes(&owner), partition_before);
    owner.set_fixture_object_position(id, Vec3::new(520.0, 0.0, 540.0));
    owner.update_owned_partition_with_effects(&mut Default::default()).unwrap();
    assert_eq!(cash_value(&owner, Vec3::new(120.0, 0.0, 140.0)), 0);
    assert_eq!(cash_value(&owner, Vec3::new(520.0, 0.0, 540.0)), 50);
    assert_eq!(partition_bytes(&peer), peer_partition);
    assert_eq!(trigger_bytes(&peer), peer_trigger);
    owner.reset();
    assert_eq!(partition_bytes(&peer), peer_partition);
}
