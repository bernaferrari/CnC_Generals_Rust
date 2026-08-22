#[test]
fn left_hud_click_does_not_reject_unexplored_shroud() {
    let src = include_str!("../src/graphics/render_pipeline/pipeline_minimap.rs");
    let start = src
        .find("pub fn handle_minimap_click")
        .expect("handle_minimap_click");
    let body = &src[start..start + 400];
    assert!(
        !body.contains("is_position_visible"),
        "C++ LeftHUDInput has no FOW test after radarToWorld"
    );
    assert!(body.contains("screen_to_world"));
}
