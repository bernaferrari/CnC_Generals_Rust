use super::*;
use crate::drawable::{BasicDrawable, DrawableExt};

fn basic_address(client: &GameClient, id: DrawableId) -> usize {
    client
        .find_drawable_by_id(id)
        .unwrap()
        .downcast_ref::<BasicDrawable>()
        .unwrap() as *const BasicDrawable as usize
}

#[test]
fn moving_owned_direct_drawables_preserves_actual_allocation_and_binding() {
    let mut source = GameClient::new().unwrap();
    let mut receiver = GameClient::new().unwrap();
    let mut entry = presentation_drawable_sync_for_test(
        42,
        19,
        "MovedVisual",
        true,
        false,
        [3.0, 5.0, 7.0],
        0.25,
    );
    entry.draw_module_names = Arc::new(vec!["W3DTankDraw".into()]);
    entry.model_condition_bits = 1 << game_engine::common::bit_flags::ModelConditionFlags::MOVING;
    assert_eq!(source.sync_presentation_drawables([entry]), (1, 0, 0));
    let binding = source
        .presentation_direct_drawable_state(19, 42)
        .unwrap()
        .binding_key;
    let id = binding.drawable_id;
    let allocation = basic_address(&source, id);
    let model_flags = source
        .find_drawable_by_id(id)
        .unwrap()
        .downcast_ref::<BasicDrawable>()
        .unwrap()
        .get_model_condition_flags()
        .clone();
    let module_allocations: Vec<_> = source
        .find_drawable_by_id(id)
        .unwrap()
        .downcast_ref::<BasicDrawable>()
        .unwrap()
        .get_draw_modules()
        .iter()
        .map(|module| {
            (
                module.snapshot_module_identifier().map(str::to_owned),
                module.as_ref() as *const _ as *const () as usize,
            )
        })
        .collect();
    assert!(
        module_allocations
            .iter()
            .any(|(name, _)| name.as_deref() == Some("W3DTankDraw"))
    );
    assert_eq!(
        source.apply_frozen_direct_shroud_statuses(
            80,
            [FrozenDirectShroudStatus {
                binding_key: binding,
                raw_status: ObjectShroudStatus::Fogged,
                effectively_dead: false,
            }]
        ),
        1
    );
    let before = source.presentation_direct_drawable_state(19, 42).unwrap();
    assert!(before.fully_obscured);
    source.add_text_bearing_drawable(id);

    std::mem::swap(&mut source.drawables, &mut receiver.drawables);

    assert!(source.find_drawable_by_id(id).is_none());
    assert!(source.get_drawable_for_object(42).is_none());
    assert!(source.presentation_direct_drawable_state(19, 42).is_none());
    assert_eq!(receiver.get_drawable_for_object(42), Some(id));
    assert_eq!(
        basic_address(&receiver, id),
        allocation,
        "moving the owner must not reconstruct the heap Drawable"
    );
    assert_eq!(
        receiver.presentation_direct_drawable_state(19, 42),
        Some(before)
    );
    assert_eq!(
        receiver.find_drawable_by_id(id).unwrap().get_object_id(),
        Some(42)
    );
    let drawable = receiver.find_drawable_by_id(id).unwrap();
    assert_eq!(drawable.get_position(), Vector3::new(3.0, 5.0, 7.0));
    let basic = drawable.downcast_ref::<BasicDrawable>().unwrap();
    assert_eq!(basic.get_model_condition_flags().size(), model_flags.size());
    for index in 0..model_flags.size() {
        assert_eq!(
            basic.get_model_condition_flags().test(index),
            model_flags.test(index)
        );
    }
    assert!(model_flags.test(game_engine::common::bit_flags::ModelConditionFlags::MOVING));
    let moved_modules: Vec<_> = basic
        .get_draw_modules()
        .iter()
        .map(|module| {
            (
                module.snapshot_module_identifier().map(str::to_owned),
                module.as_ref() as *const _ as *const () as usize,
            )
        })
        .collect();
    assert_eq!(
        moved_modules, module_allocations,
        "all native modules move without recreation"
    );
    assert_eq!(receiver.drawables.text_bearing_drawables, vec![id]);
    assert!(source.drawables.text_bearing_drawables.is_empty());
}

#[test]
fn drawable_and_binding_allocators_move_with_their_owner() {
    let mut first = GameClient::new().unwrap();
    let mut peer = GameClient::new().unwrap();
    first.set_drawable_id_counter(71);
    peer.set_drawable_id_counter(9);
    first.drawables.next_presentation_direct_binding_generation = 400;
    peer.drawables.next_presentation_direct_binding_generation = 20;
    std::mem::swap(&mut first.drawables, &mut peer.drawables);
    assert_eq!(first.alloc_drawable_id(), DrawableId(9));
    assert_eq!(peer.alloc_drawable_id(), DrawableId(71));
    assert_eq!(first.alloc_presentation_direct_binding_generation(), 20);
    assert_eq!(peer.alloc_presentation_direct_binding_generation(), 400);
    for _ in 0..7 {
        peer.alloc_drawable_id();
        peer.alloc_presentation_direct_binding_generation();
    }
    assert_eq!(first.alloc_drawable_id(), DrawableId(10));
    assert_eq!(first.alloc_presentation_direct_binding_generation(), 21);
    assert_eq!(peer.alloc_drawable_id(), DrawableId(79));
    assert_eq!(peer.alloc_presentation_direct_binding_generation(), 408);
}

#[test]
fn receiving_client_destroys_moved_drawable_and_its_owned_bookkeeping() {
    let mut source = GameClient::new().unwrap();
    let mut receiver = GameClient::new().unwrap();
    let entry =
        presentation_drawable_sync_for_test(81, 33, "RetiredVisual", true, false, [0.0; 3], 0.0);
    assert_eq!(source.sync_presentation_drawables([entry]), (1, 0, 0));
    let binding = source
        .presentation_direct_drawable_state(33, 81)
        .unwrap()
        .binding_key;
    let id = binding.drawable_id;
    source.add_text_bearing_drawable(id);
    // RayEffect bookkeeping references an actual allocated Drawable; it does
    // not manufacture a LaserUpdate module or bypass its particle retirement.
    assert!(source.drawables.ray_effects.add(
        id,
        gamelogic::common::Coord3D::new(1.0, 2.0, 3.0),
        gamelogic::common::Coord3D::new(4.0, 5.0, 6.0)
    ));
    assert!(source.drawables.owned_laser_order.is_empty());
    std::mem::swap(&mut source.drawables, &mut receiver.drawables);
    assert!(receiver.drawables.ray_effects.get(id).is_some());
    assert!(source.drawables.ray_effects.get(id).is_none());
    receiver.destroy_drawable(id).unwrap();
    assert!(receiver.find_drawable_by_id(id).is_none());
    assert!(receiver.get_drawable_for_object(81).is_none());
    assert!(
        receiver
            .presentation_direct_drawable_state(33, 81)
            .is_none()
    );
    assert!(
        !receiver
            .drawables
            .presentation_direct_drawable_bindings
            .contains_key(&id)
    );
    assert!(receiver.drawables.text_bearing_drawables.is_empty());
    assert!(receiver.drawables.ray_effects.get(id).is_none());
    assert!(receiver.drawables.owned_laser_order.is_empty());
    assert_eq!(receiver.alloc_drawable_id(), DrawableId(id.0 + 1));
    assert_eq!(
        receiver.alloc_presentation_direct_binding_generation(),
        binding.binding_generation + 1
    );
    assert_eq!(source.alloc_drawable_id(), DrawableId(1));
}

#[test]
fn actual_drawable_owner_can_move_across_threads_without_shared_mutation() {
    let mut client = GameClient::new().unwrap();
    let id = client
        .register_drawable(Box::new(BasicDrawable::new(DrawableId::INVALID)))
        .unwrap();
    let allocation = basic_address(&client, id);
    let owner = std::mem::replace(
        &mut client.drawables,
        super::super::drawable_world::DrawableWorld::new(),
    );
    assert!(client.find_drawable_by_id(id).is_none());
    let owner = std::thread::spawn(move || {
        let mut owner = owner;
        owner
            .drawable_map
            .get_mut(&id)
            .unwrap()
            .set_position(Vector3::new(8.0, 9.0, 10.0));
        owner
    })
    .join()
    .unwrap();
    client.drawables = owner;
    assert_eq!(basic_address(&client, id), allocation);
    assert_eq!(
        client.find_drawable_by_id(id).unwrap().get_position(),
        Vector3::new(8.0, 9.0, 10.0)
    );
}

#[test]
fn receiving_owner_binding_displacement_does_not_touch_same_id_peer() {
    let mut first = GameClient::new().unwrap();
    let mut peer = GameClient::new().unwrap();
    for client in [&mut first, &mut peer] {
        assert_eq!(
            client.sync_presentation_drawables([
                presentation_drawable_sync_for_test(41, 7, "First", true, false, [0.0; 3], 0.0),
                presentation_drawable_sync_for_test(42, 7, "Second", true, false, [1.0; 3], 0.0),
            ]),
            (2, 0, 0)
        );
    }
    let first_key = first
        .drawables
        .presentation_direct_drawable_state(7, 41)
        .unwrap()
        .binding_key;
    let second_key = first
        .drawables
        .presentation_direct_drawable_state(7, 42)
        .unwrap()
        .binding_key;
    let peer_key = peer
        .drawables
        .presentation_direct_drawable_state(7, 41)
        .unwrap()
        .binding_key;
    // Rebinding the identical live association preserves its runtime lifetime.
    first
        .drawables
        .bind_drawable_to_object(first_key.drawable_id, 41)
        .unwrap();
    assert_eq!(
        first
            .drawables
            .presentation_direct_drawable_state(7, 41)
            .unwrap()
            .binding_key,
        first_key
    );
    // Moving onto another resident Object detaches the displaced Drawable and
    // invalidates both runtime keys before the next explicit host sync.
    first
        .drawables
        .bind_drawable_to_object(first_key.drawable_id, 42)
        .unwrap();
    assert!(
        first
            .drawables
            .presentation_direct_drawable_state(7, 41)
            .is_none()
    );
    assert!(
        first
            .drawables
            .presentation_direct_drawable_state(7, 42)
            .is_none()
    );
    assert_eq!(first.drawables.get_drawable_for_object(41), None);
    assert_eq!(
        first.drawables.get_drawable_for_object(42),
        Some(first_key.drawable_id)
    );
    assert_eq!(
        first
            .drawables
            .find_drawable_by_id(second_key.drawable_id)
            .unwrap()
            .get_object_id(),
        None
    );
    assert_eq!(
        peer.drawables
            .presentation_direct_drawable_state(7, 41)
            .unwrap()
            .binding_key,
        peer_key
    );
    assert!(
        peer.drawables
            .presentation_direct_drawable_state(0, 41)
            .is_none()
    );
    assert!(
        peer.drawables
            .presentation_direct_drawable_state(8, 41)
            .is_none()
    );
    assert_eq!(
        peer.drawables.get_drawable_for_object(42),
        Some(second_key.drawable_id)
    );
}

#[test]
fn rejected_missing_drawable_bind_preserves_receiving_lifetime_and_peer() {
    let mut client = GameClient::new().unwrap();
    let mut peer = GameClient::new().unwrap();
    for world in [&mut client, &mut peer] {
        world.sync_presentation_drawables([presentation_drawable_sync_for_test(
            51,
            12,
            "LiveBinding",
            true,
            false,
            [3.0, 4.0, 5.0],
            0.5,
        )]);
        let key = world
            .presentation_direct_drawable_state(12, 51)
            .unwrap()
            .binding_key;
        assert_eq!(
            world.apply_frozen_direct_shroud_statuses(
                90,
                [FrozenDirectShroudStatus {
                    binding_key: key,
                    raw_status: ObjectShroudStatus::Fogged,
                    effectively_dead: false,
                }]
            ),
            1
        );
    }
    let before = client.presentation_direct_drawable_state(12, 51).unwrap();
    let peer_before = peer.presentation_direct_drawable_state(12, 51).unwrap();
    assert!(before.fully_obscured);
    let live_id = before.binding_key.drawable_id;
    let allocation = basic_address(&client, live_id);
    let retired_id = client
        .register_drawable(Box::new(BasicDrawable::new(DrawableId::INVALID)))
        .unwrap();
    client.destroy_drawable(retired_id).unwrap();
    let next_id = client.drawables.next_drawable_id;
    let next_generation = client.drawables.next_presentation_direct_binding_generation;
    for missing_id in [DrawableId(u32::MAX), retired_id] {
        for target in [51, 999] {
            assert!(matches!(
                client.bind_drawable_to_object(missing_id, target),
                Err(GameClientError::DrawableNotFound(id)) if id == missing_id
            ));
            assert_eq!(
                client.presentation_direct_drawable_state(12, 51),
                Some(before)
            );
            assert_eq!(client.get_drawable_for_object(51), Some(live_id));
            assert_eq!(client.get_drawable_for_object(999), None);
            assert_eq!(
                client.find_drawable_by_id(live_id).unwrap().get_object_id(),
                Some(51)
            );
            assert_eq!(basic_address(&client, live_id), allocation);
            assert_eq!(client.drawables.next_drawable_id, next_id);
            assert_eq!(
                client.drawables.next_presentation_direct_binding_generation,
                next_generation
            );
            assert_eq!(
                peer.presentation_direct_drawable_state(12, 51),
                Some(peer_before)
            );
            assert_eq!(
                peer.get_drawable_for_object(51),
                Some(peer_before.binding_key.drawable_id)
            );
        }
    }
}
