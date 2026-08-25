use generals_main::graphics::minimap_renderer::MinimapCoordinates;
use glam::{Vec2, Vec3};

#[test]
fn letterboxed_click_rejects_bars_and_inverts_y() {
    let tall = MinimapCoordinates {
        minimap_width: 200.0,
        minimap_height: 100.0,
        world_min: Vec3::new(0.0, 0.0, 0.0),
        world_max: Vec3::new(100.0, 0.0, 400.0),
        screen_pos: Vec2::new(0.0, 0.0),
    };
    assert!(
        tall.letterboxed_click_to_world(Vec2::new(10.0, 50.0))
            .is_none()
    );
    assert!(
        tall.letterboxed_click_to_world(Vec2::new(190.0, 50.0))
            .is_none()
    );

    let square = MinimapCoordinates {
        minimap_width: 128.0,
        minimap_height: 128.0,
        world_min: Vec3::new(0.0, 0.0, 0.0),
        world_max: Vec3::new(1280.0, 0.0, 1280.0),
        screen_pos: Vec2::new(0.0, 0.0),
    };
    let north = square
        .letterboxed_click_to_world(Vec2::new(64.0, 8.0))
        .expect("north");
    let south = square
        .letterboxed_click_to_world(Vec2::new(64.0, 120.0))
        .expect("south");
    assert!(north.z > south.z);
}
