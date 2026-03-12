// Shadow Mapping Shaders
//
// Implements shadow rendering with PCF filtering matching C++ quality.

// Shadow map uniforms
struct ShadowUniforms {
    light_vp_matrix: mat4x4<f32>,
    bias: f32,
    normal_offset: f32,
    pcf_radius: f32,
    shadow_map_size: f32,
}

@group(1) @binding(0)
var<uniform> shadow_uniforms: ShadowUniforms;

@group(1) @binding(1)
var shadow_map: texture_depth_2d;

@group(1) @binding(2)
var shadow_sampler: sampler_comparison;

// PCF kernel offsets (4x4 from C++)
const PCF_OFFSETS: array<vec2<f32>, 16> = array<vec2<f32>, 16>(
    vec2<f32>(-1.5, -1.5), vec2<f32>(-0.5, -1.5), vec2<f32>(0.5, -1.5), vec2<f32>(1.5, -1.5),
    vec2<f32>(-1.5, -0.5), vec2<f32>(-0.5, -0.5), vec2<f32>(0.5, -0.5), vec2<f32>(1.5, -0.5),
    vec2<f32>(-1.5,  0.5), vec2<f32>(-0.5,  0.5), vec2<f32>(0.5,  0.5), vec2<f32>(1.5,  0.5),
    vec2<f32>(-1.5,  1.5), vec2<f32>(-0.5,  1.5), vec2<f32>(0.5,  1.5), vec2<f32>(1.5,  1.5),
);

// Shadow depth vertex shader
struct ShadowVertexInput {
    @location(0) position: vec3<f32>,
}

struct ShadowVertexOutput {
    @builtin(position) clip_position: vec4<f32>,
}

@vertex
fn shadow_vertex_main(in: ShadowVertexInput) -> ShadowVertexOutput {
    var out: ShadowVertexOutput;
    out.clip_position = shadow_uniforms.light_vp_matrix * vec4<f32>(in.position, 1.0);
    return out;
}

// Shadow depth fragment shader (depth only, no color output)
@fragment
fn shadow_fragment_main(in: ShadowVertexOutput) {
    // Depth is written automatically, no output needed
}

// Sample shadow map with PCF filtering
fn sample_shadow_pcf(shadow_coord: vec3<f32>) -> f32 {
    let texel_size = 1.0 / shadow_uniforms.shadow_map_size;
    var shadow_factor = 0.0;

    // Apply bias
    let biased_depth = shadow_coord.z - shadow_uniforms.bias;

    // PCF 4x4 sampling
    for (var i = 0; i < 16; i++) {
        let offset = PCF_OFFSETS[i] * texel_size * shadow_uniforms.pcf_radius;
        let sample_coord = vec2<f32>(
            shadow_coord.x + offset.x,
            shadow_coord.y + offset.y
        );

        // Compare depth
        shadow_factor += textureSampleCompare(
            shadow_map,
            shadow_sampler,
            sample_coord,
            biased_depth
        );
    }

    // Average samples
    return shadow_factor / 16.0;
}

// Simple shadow sampling (no PCF)
fn sample_shadow_simple(shadow_coord: vec3<f32>) -> f32 {
    let biased_depth = shadow_coord.z - shadow_uniforms.bias;
    return textureSampleCompare(
        shadow_map,
        shadow_sampler,
        shadow_coord.xy,
        biased_depth
    );
}

// Calculate shadow factor for a world position
fn calculate_shadow_factor(world_pos: vec3<f32>, world_normal: vec3<f32>) -> f32 {
    // Transform to light space
    let light_space_pos = shadow_uniforms.light_vp_matrix * vec4<f32>(world_pos, 1.0);

    // Perspective divide
    var shadow_coord = light_space_pos.xyz / light_space_pos.w;

    // Transform from [-1, 1] to [0, 1] range
    shadow_coord = shadow_coord * 0.5 + 0.5;

    // Flip Y for texture coordinates
    shadow_coord.y = 1.0 - shadow_coord.y;

    // Check if position is outside shadow map
    if (shadow_coord.x < 0.0 || shadow_coord.x > 1.0 ||
        shadow_coord.y < 0.0 || shadow_coord.y > 1.0 ||
        shadow_coord.z < 0.0 || shadow_coord.z > 1.0) {
        return 1.0; // No shadow
    }

    // Apply normal offset bias to reduce shadow acne
    let light_dir = normalize(vec3<f32>(0.0, -1.0, 0.0)); // Should come from light
    let normal_offset = shadow_uniforms.normal_offset * (1.0 - dot(world_normal, -light_dir));
    shadow_coord.z -= normal_offset;

    // Sample with PCF
    return sample_shadow_pcf(shadow_coord);
}

// Cascaded shadow map sampling (for directional lights)
struct CascadeInfo {
    cascade_index: i32,
    shadow_coord: vec3<f32>,
}

fn select_cascade(view_depth: f32) -> i32 {
    // Simple cascade selection based on view depth
    // In production, this would use proper split distances
    if (view_depth < 10.0) {
        return 0;
    } else if (view_depth < 50.0) {
        return 1;
    } else {
        return 2;
    }
}

// Apply shadow to final lighting
fn apply_shadow(base_color: vec3<f32>, shadow_factor: f32) -> vec3<f32> {
    // shadow_factor: 1.0 = fully lit, 0.0 = fully shadowed
    let shadow_color = base_color * 0.3; // Ambient in shadow (from C++)
    return mix(shadow_color, base_color, shadow_factor);
}
