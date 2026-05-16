// Mesh Opaque Fragment Shader
// Simple forward rendering for opaque drawable geometry

struct VertexOutput {
    @builtin(position) clip_pos: vec4<f32>,
    @location(0) world_pos: vec3<f32>,
    @location(1) world_normal: vec3<f32>,
    @location(2) uv: vec2<f32>,
    @location(3) vertex_color: vec4<f32>,
    @location(4) color_tint: vec4<f32>,
    @location(5) opacity: f32,
};

@group(2) @binding(0)
var diffuse_texture: texture_2d<f32>;

@group(2) @binding(1)
var diffuse_sampler: sampler;

@fragment
fn fs_main(input: VertexOutput) -> @location(0) vec4<f32> {
    // Simple directional light from above
    let light_dir = normalize(vec3<f32>(0.3, 1.0, 0.5));
    let ambient = 0.3;
    let diffuse = max(dot(normalize(input.world_normal), light_dir), 0.0);
    let lighting = ambient + diffuse * 0.7;

    // Base color with tint
    let diffuse_color = textureSample(diffuse_texture, diffuse_sampler, input.uv);
    var color = diffuse_color * input.vertex_color * input.color_tint;

    // Apply lighting
    color = vec4<f32>(color.rgb * lighting, color.a);

    return color;
}
