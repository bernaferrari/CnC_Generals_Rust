use game_client_rust::display::shader_filter::ShaderFilterResources;

#[test]
fn filtered_rtt_capture_state_is_owned_by_each_resource_instance() {
    let first = ShaderFilterResources::default();
    let second = ShaderFilterResources::default();
    let format = wgpu::TextureFormat::Rgba8Unorm;

    assert!(!std::ptr::eq(&first, &second));
    assert!(!first.has_scene_capture_for(8, 8, format));
    assert!(!second.has_scene_capture_for(8, 8, format));
}
