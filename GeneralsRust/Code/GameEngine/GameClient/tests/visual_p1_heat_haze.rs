use game_client_rust::effects::heat_haze::{
    HeatHazeSmudge, build_heat_haze_quad, heat_haze_triangle_indices,
};

#[test]
fn heat_haze_center_uv_uses_offset_x_like_cpp() {
    let identity = [
        [1.0, 0.0, 0.0, 0.0],
        [0.0, 1.0, 0.0, 0.0],
        [0.0, 0.0, 1.0, 0.0],
        [0.0, 0.0, 0.0, 1.0],
    ];
    let smudge = HeatHazeSmudge {
        world_pos: [0.0, 0.0, -10.0],
        size: 2.0,
        offset: [0.2, 0.4],
        opacity: 0.7,
    };
    let verts =
        build_heat_haze_quad(smudge, &identity, &identity, [0.5, 0.5], [1.0, 1.0]).expect("quad");
    let span_x = verts[3].uv[0] - verts[0].uv[0];
    let span_y = verts[1].uv[1] - verts[0].uv[1];
    assert!((verts[4].uv[0] - (verts[0].uv[0] + span_x * 0.7)).abs() < 1.0e-5);
    assert!((verts[4].uv[1] - (verts[0].uv[1] + span_y * 0.7)).abs() < 1.0e-5);
    assert_eq!(heat_haze_triangle_indices(0).len(), 12);
}
