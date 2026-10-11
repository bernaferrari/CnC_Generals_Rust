//! C++ Radar.cpp reset, event-ring and update contracts through live Main owners.
use super::*;
use crate::game_logic::{GameLogic, KindOf, Player, Team, ThingTemplate};
use crate::save_load::snapshot::persist_v18::{capture_persist_v18, restore_persist_v18};
use game_engine::common::system::radar::{MAX_RADAR_EVENTS, RadarEventType};
use glam::Vec3;

fn world(pos: Vec3) -> (GameLogic, crate::game_logic::ObjectId) {
    let mut world = GameLogic::new();
    world.add_player(Player::new(1, Team::USA, "Local", true));
    let mut template = ThingTemplate::new("RadarOwnerUnit");
    template.add_kind_of(KindOf::Infantry).set_health(100.0);
    world.templates.insert("RadarOwnerUnit".into(), template);
    let id = world
        .create_object_for_player("RadarOwnerUnit", 1, pos)
        .unwrap();
    (world, id)
}

#[test]
fn radar_owner_same_ids_maps_events_and_reset_do_not_cross_sessions() {
    let (mut a, a_id) = world(Vec3::new(10.0, 0.0, 20.0));
    a.frame = 7;
    a.host_create_radar_event(Vec3::new(12.0, 3.0, 24.0), RadarEventType::UnderAttack);
    let before = capture_persist_v18(&a);
    let (mut b, b_id) = world(Vec3::new(90.0, 0.0, 80.0));
    assert_eq!(a_id, b_id);
    b.world_min = Vec3::new(100.0, 0.0, 200.0);
    b.world_max = Vec3::new(612.0, 0.0, 712.0);
    b.host_radar_on_map_loaded();
    b.frame = 40;
    b.host_create_radar_event(Vec3::new(110.0, 4.0, 220.0), RadarEventType::Construction);
    b.host_radar_sync_live_objects();
    assert_eq!(
        a.radar()
            .get_all_objects()
            .find(|o| o.object_id == a_id.0)
            .unwrap()
            .world_pos
            .x,
        10.0
    );
    assert_eq!(
        b.radar()
            .get_all_objects()
            .find(|o| o.object_id == b_id.0)
            .unwrap()
            .world_pos
            .x,
        90.0
    );
    assert_ne!(a.radar().map_extent().lo.x, b.radar().map_extent().lo.x);
    b.reset();
    drop(b);
    let after = capture_persist_v18(&a);
    assert_eq!(
        serde_json::to_value(before.radar_events).unwrap(),
        serde_json::to_value(after.radar_events).unwrap()
    );
    assert_eq!(
        a.last_the_radar_event_host_position(),
        Some(Vec3::new(12.0, 3.0, 24.0))
    );
}

#[test]
fn radar_owner_v18_restores_only_receiver_and_keeps_ring_order() {
    let (mut a, _) = world(Vec3::ZERO);
    a.frame = 30;
    for index in 0..MAX_RADAR_EVENTS + 3 {
        a.host_create_radar_event(
            Vec3::new(index as f32, 2.0, 3.0),
            RadarEventType::Information,
        );
    }
    a.host_create_radar_event_for(Vec3::ZERO, RadarEventType::BeaconPulse, 0.5);
    let saved = capture_persist_v18(&a);
    assert_eq!(saved.radar_next_event, 4);
    assert_eq!(saved.radar_last_event, 2);
    let (mut b, _) = world(Vec3::ZERO);
    b.frame = 90;
    b.host_create_radar_event(Vec3::new(99.0, 0.0, 99.0), RadarEventType::UnderAttack);
    let before = capture_persist_v18(&b);
    a.radar_mut().clear_all_events();
    restore_persist_v18(&saved, &mut a).unwrap();
    let restored = capture_persist_v18(&a);
    let after = capture_persist_v18(&b);
    assert_eq!(
        serde_json::to_value(saved.radar_events).unwrap(),
        serde_json::to_value(restored.radar_events).unwrap()
    );
    assert_eq!(restored.radar_next_event, 4);
    assert_eq!(restored.radar_last_event, 2);
    assert_eq!(
        serde_json::to_value(before.radar_events).unwrap(),
        serde_json::to_value(after.radar_events).unwrap()
    );
    assert_eq!(
        b.last_the_radar_event_host_position(),
        Some(Vec3::new(99.0, 0.0, 99.0))
    );
}

#[test]
fn radar_owner_cpp_expiry_and_refresh_are_strict_and_frame_zero_is_inactive() {
    let mut world = GameLogic::new();
    world.host_create_radar_event(Vec3::ZERO, RadarEventType::Information);
    world.radar_mut().queue_terrain_refresh();
    assert!(!world.radar_mut().update_owned(1000));
    assert_eq!(
        world.radar().get_active_events().len(),
        1,
        "C++ createFrame zero is not expired"
    );
    world.radar_mut().clear_all_events();
    world.frame = 10;
    world.host_create_radar_event(Vec3::ZERO, RadarEventType::Information);
    world.host_radar_queue_terrain_refresh();
    assert!(!world.radar_mut().update_owned(100));
    assert!(world.radar_mut().update_owned(101));
    assert!(
        !world.radar_mut().update_owned(102),
        "refresh consumed once"
    );
    world.radar_mut().update_owned(130);
    assert_eq!(world.radar().get_active_events().len(), 1);
    world.radar_mut().update_owned(131);
    assert!(world.radar().get_active_events().is_empty());
}

#[test]
fn radar_owner_rejected_v18_load_leaves_both_running_rings_unchanged() {
    let (mut a, _) = world(Vec3::ZERO);
    let (mut b, _) = world(Vec3::ZERO);
    a.frame = 10;
    b.frame = 20;
    a.host_create_radar_event(Vec3::new(1.0, 2.0, 3.0), RadarEventType::Information);
    b.host_create_radar_event(Vec3::new(4.0, 5.0, 6.0), RadarEventType::Construction);
    let before_a = capture_persist_v18(&a);
    let before_b = capture_persist_v18(&b);
    let mut malformed = before_b.clone();
    malformed.radar_next_event = MAX_RADAR_EVENTS as i32;
    assert!(restore_persist_v18(&malformed, &mut a).is_err());
    assert_eq!(
        serde_json::to_value(before_a.radar_events).unwrap(),
        serde_json::to_value(capture_persist_v18(&a).radar_events).unwrap()
    );
    assert_eq!(
        serde_json::to_value(before_b.radar_events).unwrap(),
        serde_json::to_value(capture_persist_v18(&b).radar_events).unwrap()
    );
}

#[test]
fn radar_owner_refresh_preserves_each_maps_admitted_tile_colors() {
    let (mut a, _) = world(Vec3::ZERO);
    let (mut b, _) = world(Vec3::ZERO);
    assert!(a.admit_radar_terrain_colors(vec![Some([0.1, 0.2, 0.8]); 128 * 128], [0.2, 0.3, 0.9]));
    let a_texture = a.radar().get_terrain_texture().to_vec();
    assert!(b.admit_radar_terrain_colors(vec![Some([0.8, 0.2, 0.1]); 128 * 128], [0.9, 0.3, 0.2]));
    assert_ne!(a_texture, b.radar().get_terrain_texture());
    b.reset();
    a.host_radar_refresh_terrain();
    assert_eq!(a.radar().get_terrain_texture(), a_texture);
    assert!(!a.admit_radar_terrain_colors(vec![Some([1.0; 3]); 1], [1.0; 3]));
    assert_eq!(
        a.radar().get_terrain_texture(),
        a_texture,
        "invalid admission is inert"
    );
}

#[test]
fn radar_owner_cpp_unsigned_event_deadlines_and_refresh_wrap() {
    let mut world = GameLogic::new();
    world.frame = u32::MAX - 10;
    world.host_create_radar_event(Vec3::ZERO, RadarEventType::Information);
    let event = world.radar().get_active_events()[0];
    assert_eq!(event.create_frame, u32::MAX - 10);
    assert_eq!(event.die_frame, 109);
    assert_eq!(event.fade_frame, 94);
    world.host_radar_queue_terrain_refresh();
    assert!(!world.radar_mut().update_owned(79));
    assert!(world.radar_mut().update_owned(80));
}
