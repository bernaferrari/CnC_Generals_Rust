#[test]
fn main_present_path_calls_visible_box_cull() {
    let src = include_str!("../src/graphics/render_pipeline/forward_render.rs");
    assert!(src.contains("cull_particles_to_visible_box"));
    assert!(src.contains("maximum_visible_box"));
}
