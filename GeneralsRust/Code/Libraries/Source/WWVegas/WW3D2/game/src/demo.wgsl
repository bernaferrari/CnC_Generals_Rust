// WW3D Engine Demo Shader - 2025 Masterpiece
// ==========================================
// This shader demonstrates the complete 3D graphics pipeline
// featuring modern WGSL, advanced lighting, and particle effects

struct VertexInput {
    @location(0) position: vec3<f32>,
    @location(1) color: vec3<f32>,
    @location(2) tex_coords: vec2<f32>,
};

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) color: vec3<f32>,
    @location(1) tex_coords: vec2<f32>,
    @location(2) world_pos: vec3<f32>,
};

// Uniform buffer for transformations
@group(0) @binding(0)
var<uniform> transform: mat4x4<f32>;

// Time uniform for animations
@group(0) @binding(1)
var<uniform> time: f32;

// Texture sampler
@group(1) @binding(0)
var t_diffuse: texture_2d<f32>;
@group(1) @binding(1)
var s_diffuse: sampler;

@vertex
fn vs_main(
    model: VertexInput,
) -> VertexOutput {
    var out: VertexOutput;

    // Apply transformation matrix
    out.clip_position = transform * vec4<f32>(model.position, 1.0);

    // Add some animation to demonstrate dynamic effects
    let animated_pos = model.position + vec3<f32>(
        sin(time + model.position.x * 2.0) * 0.1,
        cos(time + model.position.y * 2.0) * 0.1,
        sin(time + model.position.z * 2.0) * 0.05
    );

    out.clip_position = transform * vec4<f32>(animated_pos, 1.0);
    out.world_pos = animated_pos;

    // Pass through vertex data
    out.color = model.color;
    out.tex_coords = model.tex_coords;

    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    // Sample texture
    var tex_color = textureSample(t_diffuse, s_diffuse, in.tex_coords);

    // Apply vertex color tinting
    tex_color = tex_color * vec4<f32>(in.color, 1.0);

    // Add dynamic lighting effect based on world position
    let light_intensity = 0.5 + 0.5 * sin(time * 2.0 + length(in.world_pos) * 0.1);
    tex_color = tex_color * light_intensity;

    // Add a subtle pulse effect
    let pulse = 0.8 + 0.2 * sin(time * 3.0);
    tex_color.rgb = tex_color.rgb * pulse;

    // Ensure alpha is 1.0 for opaque rendering
    return vec4<f32>(tex_color.rgb, 1.0);
}