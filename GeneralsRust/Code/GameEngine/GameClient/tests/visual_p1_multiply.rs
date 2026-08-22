use game_client_rust::effects::particle_renderer::particle_multiply_color_blend;

#[test]
fn multiply_blend_is_dest_times_src() {
    let blend = particle_multiply_color_blend();
    assert_eq!(blend.src_factor, wgpu::BlendFactor::Zero);
    assert_eq!(blend.dst_factor, wgpu::BlendFactor::Src);
    assert_eq!(blend.operation, wgpu::BlendOperation::Add);
}
