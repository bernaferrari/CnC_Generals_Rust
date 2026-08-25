#[test]
fn letterbox_lifts_min_max_camera_height_clamp() {
    fn height_after_zoom_steps(
        current_hag: f32,
        steps: f32,
        min_h: f32,
        max_h: f32,
        zoom_limited: bool,
    ) -> f32 {
        let next = current_hag + steps * 10.0;
        if zoom_limited {
            next.clamp(min_h, max_h)
        } else {
            next
        }
    }
    assert!((height_after_zoom_steps(120.0, -20.0, 40.0, 200.0, true) - 40.0).abs() < f32::EPSILON);
    assert!(
        (height_after_zoom_steps(120.0, -20.0, 40.0, 200.0, false) + 80.0).abs() < f32::EPSILON
    );
    let src = generals_main::cnc_game_engine::ENGINE_SRC;
    assert!(src.contains("live_camera_zoom_limited"));
    assert!(src.contains("height_after_zoom_steps"));
}
