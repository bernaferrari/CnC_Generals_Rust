use generals_main::pick_ray::{
    MIN_PICK_SPHERE_RADIUS, OS_DOUBLE_CLICK_SLOP_PX, is_os_style_double_click,
    opaque_window_chain_blocks_pick, presentation_mesh_pick_radius, ray_sphere_hit_t,
    world_lmb_selection_allowed,
};
use glam::Vec3;

#[test]
fn ground_click_beside_a_unit_does_not_hit_its_mesh_sphere() {
    let camera = Vec3::new(0.0, 120.0, 120.0);
    let ground_beside = Vec3::new(20.0, 0.0, 0.0);
    let dir = ground_beside - camera;
    let hit = ray_sphere_hit_t(camera, dir, Vec3::ZERO, 8.0);
    assert!(
        hit.is_none() || hit.is_some_and(|t| t > 1.0),
        "terrain-adjacent click must not select via origin proximity, hit={hit:?}"
    );
}

#[test]
fn long_vehicle_mesh_far_from_origin_is_hittable() {
    let radius = presentation_mesh_pick_radius(8.0, 80.0);
    assert!((radius - 40.0).abs() < f32::EPSILON);
    let camera = Vec3::new(30.0, 80.0, 80.0);
    let mesh_point = Vec3::new(30.0, 4.0, 0.0);
    let dir = mesh_point - camera;
    let hit = ray_sphere_hit_t(camera, dir, Vec3::ZERO, radius);
    assert!(
        hit.is_some_and(|t| (0.0..=1.0).contains(&t)),
        "long-mesh click must hit, hit={hit:?}"
    );
}

#[test]
fn pick_radius_never_inflates_to_twenty_world_units() {
    assert_eq!(presentation_mesh_pick_radius(5.0, 0.0), 5.0);
    assert_eq!(
        presentation_mesh_pick_radius(0.0, 0.0),
        MIN_PICK_SPHERE_RADIUS
    );
    assert!(presentation_mesh_pick_radius(5.0, 0.0) < 20.0);
}

#[test]
fn opaque_hud_ancestor_refuses_the_pick() {
    assert!(!opaque_window_chain_blocks_pick(&[true, true]));
    assert!(opaque_window_chain_blocks_pick(&[true, false]));
    assert!(opaque_window_chain_blocks_pick(&[false]));
    assert!(!opaque_window_chain_blocks_pick(&[]));
}

#[test]
fn double_click_uses_screen_pixels_not_world_units() {
    assert!(is_os_style_double_click(
        200,
        3.0,
        0.0,
        500,
        OS_DOUBLE_CLICK_SLOP_PX
    ));
    assert!(!is_os_style_double_click(
        200,
        6.0,
        0.0,
        500,
        OS_DOUBLE_CLICK_SLOP_PX
    ));
    assert!(!is_os_style_double_click(
        600,
        0.0,
        0.0,
        500,
        OS_DOUBLE_CLICK_SLOP_PX
    ));
}

#[test]
fn quit_menu_destroys_world_left_click() {
    assert!(!world_lmb_selection_allowed(true));
    assert!(world_lmb_selection_allowed(false));
}

#[test]
fn live_paths_wire_the_four_selection_hud_gates() {
    let mouse = generals_main::cnc_game_engine::ENGINE_SRC;
    let input = include_str!("../src/cnc_game_engine/input.rs");
    let hotkeys = include_str!("../src/cnc_game_engine/hotkeys.rs");
    assert!(mouse.contains("fn find_object_at_cursor"));
    assert!(mouse.contains("is_os_style_double_click"));
    assert!(mouse.contains("fn apply_meta_options_interrupt"));
    assert!(input.contains("world_lmb_selection_allowed"));
    assert!(hotkeys.contains("self.apply_meta_options_interrupt()"));
}
