// Shadow mapping shader for dynamic lights
// Generates depth maps from light perspectives for realistic shadows

struct LightingUniforms {
    view_matrix: mat4x4<f32>,
    projection_matrix: mat4x4<f32>,
    camera_position: vec4<f32>,
    ambient_color: vec4<f32>,
    time: f32,
    num_active_lights: u32,
    shadow_cascade_count: u32,
    _padding: u32,
}

struct GPULight {
    position: vec4<f32>,
    direction: vec4<f32>,
    color_intensity: vec4<f32>,
    radius_falloff: vec4<f32>,
    flicker_pulse: vec4<f32>,
    flags: vec4<u32>,
}

struct ShadowCascade {
    view_proj_matrix: mat4x4<f32>,
    split_distance: f32,
    _padding: vec3<f32>,
}

struct VertexInput {
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
}

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) world_pos: vec3<f32>,
}

@group(0) @binding(0) var<uniform> uniforms: LightingUniforms;
@group(0) @binding(1) var<storage, read> lights: array<GPULight>;
@group(0) @binding(4) var<uniform> cascades: array<ShadowCascade, 4>;

// Vertex shader for shadow mapping
@vertex
fn vs_main(input: VertexInput) -> VertexOutput {
    let world_pos = input.position;
    
    // For shadow mapping, we need to render from the light's perspective
    // This would typically use a light's view-projection matrix
    // For now, we'll use the main camera matrices as placeholders
    
    let view_pos = uniforms.view_matrix * vec4<f32>(world_pos, 1.0);
    let clip_pos = uniforms.projection_matrix * view_pos;
    
    return VertexOutput(
        clip_pos,
        world_pos
    );
}

// Fragment shader for shadow mapping
// Note: In depth-only shadow passes, the fragment shader is often omitted
// This is here for completeness and potential alpha testing
@fragment
fn fs_main(input: VertexOutput) -> @location(0) vec4<f32> {
    // For depth-only shadow mapping, we don't need to output color
    // The depth buffer will be written automatically
    
    // If we needed alpha testing for vegetation or transparent objects:
    // let alpha = sample_alpha_texture(input.tex_coords);
    // if (alpha < 0.5) {
    //     discard;
    // }
    
    return vec4<f32>(1.0, 1.0, 1.0, 1.0);
}