use game_engine::common::system::radar::{Coord3D, RadarEventType, get_radar_system};
use generals_main::game_logic::host_radar::{
    host_world_to_radar_coord, last_the_radar_event_host_position,
};
use glam::Vec3;

#[test]
fn spacebar_last_event_uses_the_radar_not_hud_queue() {
    {
        let radar = get_radar_system();
        let mut radar = radar.write().expect("radar write");
        radar.reset();
        radar.new_map(
            Coord3D::new(0.0, 0.0, 0.0),
            Coord3D::new(1024.0, 1024.0, 100.0),
            &[],
        );
        radar.create_event(
            &host_world_to_radar_coord(Vec3::new(150.0, 4.0, 250.0)),
            RadarEventType::UnderAttack,
            4.0,
        );
        radar.create_event(
            &host_world_to_radar_coord(Vec3::new(1.0, 0.0, 1.0)),
            RadarEventType::BeaconPulse,
            0.5,
        );
    }
    let pos = last_the_radar_event_host_position().expect("TheRadar last event");
    assert!((pos.x - 150.0).abs() < f32::EPSILON);
    assert!((pos.y - 4.0).abs() < f32::EPSILON);
    assert!((pos.z - 250.0).abs() < f32::EPSILON);
}
