// FOW Terrain Overlay Shader
// GPU-accelerated fog-of-war rendering
//
// Renders a translucent black overlay on terrain based on per-cell visibility state.
// Implements smooth gradients at fog boundaries for visual quality.

// Uniform buffer with FOW rendering parameters
struct FowUniforms {
    world_to_texture: mat4x4<f32>,  // Transform from world space to texture UV
    player_id: u32,                  // Current player ID
    fog_intensity: f32,              // Base fog opacity (0-1)
    smoothing: f32,                  // Gradient smoothing factor (0-1)
    observer_mode: u32,              // 1=bypass FOW, 0=normal
}

// Texture and sampler bindings
@group(0) @binding(0) var fow_texture: texture_2d<f32>;
@group(0) @binding(1) var fow_sampler: sampler;
@group(0) @binding(2) var<uniform> uniforms: FowUniforms;

// Vertex shader output / Fragment shader input
struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) world_pos: vec3<f32>,
    @location(1) tex_coords: vec2<f32>,
}

// Vertex shader - generates full-screen quad
// Uses triangle strip topology: vertices 0,1,2,3 form two triangles
@vertex
fn vs_main(@builtin(vertex_index) vertex_index: u32) -> VertexOutput {
    var output: VertexOutput;

    // Generate full-screen quad coordinates
    // Maps vertex indices to clip space positions
    let x = f32((vertex_index & 1u) << 1u) - 1.0;
    let y = 1.0 - f32((vertex_index & 2u));

    output.clip_position = vec4<f32>(x, y, 0.0, 1.0);

    // Texture coordinates (0,0) to (1,1)
    output.tex_coords = vec2<f32>((x + 1.0) * 0.5, (1.0 - y) * 0.5);

    // World position (will be interpolated for fragment shader)
    output.world_pos = vec3<f32>(x, 0.0, y);

    return output;
}

// Fragment shader - applies FOW effect
@fragment
fn fs_main(input: VertexOutput) -> @location(0) vec4<f32> {
    // Observer mode bypass: fully transparent (no FOW)
    if (uniforms.observer_mode != 0u) {
        return vec4<f32>(0.0, 0.0, 0.0, 0.0);
    }

    // Transform world position to texture UV coordinates
    let world_pos_4 = vec4<f32>(input.world_pos.x, input.world_pos.z, 0.0, 1.0);
    let tex_pos = uniforms.world_to_texture * world_pos_4;
    let uv = vec2<f32>(tex_pos.x, tex_pos.y);

    // Sample FOW texture (R8 format: 0=shrouded, 0.5=fogged, 1=visible)
    let fow_value = textureSample(fow_texture, fow_sampler, uv).r;

    // Convert FOW value to alpha
    // 0.0 (shrouded) -> 1.0 alpha (fully dark)
    // 0.5 (fogged) -> 0.6 alpha (darkened)
    // 1.0 (visible) -> 0.0 alpha (transparent)
    var alpha: f32;
    if (fow_value < 0.25) {
        // Shrouded: fully opaque black
        alpha = 1.0;
    } else if (fow_value < 0.75) {
        // Fogged: partially transparent (60% dark)
        alpha = 0.6;
    } else {
        // Visible: fully transparent
        alpha = 0.0;
    }

    // Apply smoothing for gradient transitions
    // Reduces hard edges at fog boundaries
    if (uniforms.smoothing > 0.0) {
        // Sample neighboring cells for gradient
        let offset = 1.0 / 256.0; // Assumes 256x256 texture
        let n = textureSample(fow_texture, fow_sampler, uv + vec2<f32>(0.0, -offset)).r;
        let s = textureSample(fow_texture, fow_sampler, uv + vec2<f32>(0.0, offset)).r;
        let e = textureSample(fow_texture, fow_sampler, uv + vec2<f32>(offset, 0.0)).r;
        let w = textureSample(fow_texture, fow_sampler, uv + vec2<f32>(-offset, 0.0)).r;

        // Average with neighbors
        let avg = (fow_value + n + s + e + w) / 5.0;

        // Blend based on smoothing factor
        let smoothed_value = mix(fow_value, avg, uniforms.smoothing);

        // Recalculate alpha with smoothed value
        alpha = mix(alpha, 1.0 - smoothed_value, uniforms.smoothing);
    }

    // Apply fog intensity multiplier
    alpha *= uniforms.fog_intensity;

    // Return black with computed alpha
    return vec4<f32>(0.0, 0.0, 0.0, alpha);
}
