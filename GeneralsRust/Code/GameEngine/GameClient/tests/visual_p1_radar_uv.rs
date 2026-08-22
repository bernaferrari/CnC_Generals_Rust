use game_client_rust::gui::w3d_gadget_draw::radar_layer_vflip_uv;

#[test]
fn radar_layer_uv_puts_texture_origin_at_hud_bottom() {
    let uv = radar_layer_vflip_uv();
    assert!((uv.y - 1.0).abs() < f32::EPSILON);
    assert!((uv.height + 1.0).abs() < f32::EPSILON);
    assert!((uv.y + uv.height).abs() < f32::EPSILON);
}
