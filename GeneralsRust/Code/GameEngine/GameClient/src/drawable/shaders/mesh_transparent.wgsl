// Mesh Transparent Fragment Shader
// Forward rendering for transparent drawable geometry (alpha blend)

struct VertexOutput {
    @builtin(position) clip_pos: vec4<f32>,
    @location(0) world_pos: vec3<f32>,
    @location(1) world_normal: vec3<f32>,
    @location(2) uv: vec2<f32>,
    @location(3) color_tint: vec4<f32>,
    @location(4) opacity: f32,
};

@fragment
fn fs_main(input: VertexOutput) -> @location(0) vec4<f32> {
    // Simple directional light from above
    let light_dir = normalize(vec3<f32>(0.3, 1.0, 0.5));
    let ambient = 0.3;
    let diffuse = max(dot(normalize(input.world_normal), light_dir), 0.0);
    let lighting = ambient + diffuse * 0.7;

    // Base color with tint
    var color = input.color_tint;

    // Apply lighting
    color = vec4<f32>(color.rgb * lighting, color.a);

    // Apply per-object opacity
    color.a *= input.opacity;

    // Discard nearly invisible fragments
    if (color.a < 0.01) {
        discard;
    }

    return color;
}
