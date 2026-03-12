// Reflection and Water Rendering Shaders
//
// Implements planar reflections, Fresnel effects, and water distortion
// matching C++ WW3D visual quality.

// ============================================================================
// Reflection Rendering
// ============================================================================

struct ReflectionUniforms {
    view_proj: mat4x4<f32>,
    reflection_view_proj: mat4x4<f32>,
    camera_position: vec3<f32>,
    reflection_strength: f32,
    plane_normal: vec3<f32>,
    plane_distance: f32,
    fresnel_power: f32,
    use_fresnel: f32,  // 1.0 = enabled, 0.0 = disabled
    _padding: vec2<f32>,
}

@group(0) @binding(0)
var<uniform> reflection_uniforms: ReflectionUniforms;

@group(0) @binding(1)
var reflection_texture: texture_2d<f32>;

@group(0) @binding(2)
var reflection_sampler: sampler;

@group(1) @binding(0)
var scene_texture: texture_2d<f32>;

@group(1) @binding(1)
var scene_sampler: sampler;

struct ReflectionVertexInput {
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) uv: vec2<f32>,
}

struct ReflectionVertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) world_position: vec3<f32>,
    @location(1) world_normal: vec3<f32>,
    @location(2) uv: vec2<f32>,
    @location(3) reflection_coords: vec4<f32>,
}

@vertex
fn reflection_vertex_main(in: ReflectionVertexInput) -> ReflectionVertexOutput {
    var out: ReflectionVertexOutput;

    out.world_position = in.position;
    out.world_normal = normalize(in.normal);
    out.uv = in.uv;

    // Regular projection
    out.clip_position = reflection_uniforms.view_proj * vec4<f32>(in.position, 1.0);

    // Calculate reflection space coordinates
    out.reflection_coords = reflection_uniforms.reflection_view_proj * vec4<f32>(in.position, 1.0);

    return out;
}

// Schlick's Fresnel approximation (F0 = 0.02 for water, from C++)
fn calculate_fresnel(view_dir: vec3<f32>, normal: vec3<f32>, power: f32) -> f32 {
    let cos_theta = abs(dot(view_dir, normal));
    let f0 = 0.02;  // From C++ water shader
    return f0 + (1.0 - f0) * pow(1.0 - cos_theta, power);
}

@fragment
fn reflection_fragment_main(in: ReflectionVertexOutput) -> @location(0) vec4<f32> {
    // Calculate view direction
    let view_dir = normalize(reflection_uniforms.camera_position - in.world_position);

    // Sample reflection texture using projected coordinates
    let reflection_uv = in.reflection_coords.xy / in.reflection_coords.w;
    let reflection_uv_normalized = reflection_uv * 0.5 + 0.5;
    let reflection_color = textureSample(reflection_texture, reflection_sampler, reflection_uv_normalized).rgb;

    // Sample scene color
    let scene_color = textureSample(scene_texture, scene_sampler, in.uv).rgb;

    // Calculate Fresnel term if enabled
    var fresnel: f32;
    if (reflection_uniforms.use_fresnel > 0.5) {
        fresnel = calculate_fresnel(view_dir, in.world_normal, reflection_uniforms.fresnel_power);
    } else {
        fresnel = reflection_uniforms.reflection_strength;
    }

    // Blend reflection with scene based on Fresnel
    let final_color = mix(scene_color, reflection_color, fresnel * reflection_uniforms.reflection_strength);

    return vec4<f32>(final_color, 1.0);
}

// ============================================================================
// Water Rendering with Wave Distortion
// ============================================================================

struct WaterUniforms {
    view_proj: mat4x4<f32>,
    camera_position: vec3<f32>,
    time: f32,
    wave_distortion: f32,
    wave_speed: f32,
    wave_scale: f32,
    fresnel_bias: f32,
    water_color: vec3<f32>,
    _padding: f32,
}

@group(0) @binding(3)
var<uniform> water_uniforms: WaterUniforms;

@group(0) @binding(4)
var normal_map: texture_2d<f32>;

@group(0) @binding(5)
var normal_sampler: sampler;

struct WaterVertexInput {
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) uv: vec2<f32>,
}

struct WaterVertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) world_position: vec3<f32>,
    @location(1) world_normal: vec3<f32>,
    @location(2) uv: vec2<f32>,
}

@vertex
fn water_vertex_main(in: WaterVertexInput) -> WaterVertexOutput {
    var out: WaterVertexOutput;

    // Apply wave displacement
    var position = in.position;
    let wave_phase = (position.x * water_uniforms.wave_scale + water_uniforms.time * water_uniforms.wave_speed);
    let wave_offset = sin(wave_phase) * water_uniforms.wave_distortion;
    position.y += wave_offset;

    out.world_position = position;
    out.world_normal = normalize(in.normal);
    out.uv = in.uv;
    out.clip_position = water_uniforms.view_proj * vec4<f32>(position, 1.0);

    return out;
}

@fragment
fn water_fragment_main(in: WaterVertexOutput) -> @location(0) vec4<f32> {
    // Calculate animated UV for normal map
    let time_offset = water_uniforms.time * water_uniforms.wave_speed * 0.1;
    let uv1 = in.uv * water_uniforms.wave_scale + vec2<f32>(time_offset, 0.0);
    let uv2 = in.uv * water_uniforms.wave_scale * 0.7 + vec2<f32>(-time_offset * 0.5, time_offset * 0.3);

    // Sample normal maps with distortion
    let normal1 = textureSample(normal_map, normal_sampler, uv1).rgb * 2.0 - 1.0;
    let normal2 = textureSample(normal_map, normal_sampler, uv2).rgb * 2.0 - 1.0;
    let perturbed_normal = normalize(normal1 + normal2);

    // Calculate view direction
    let view_dir = normalize(water_uniforms.camera_position - in.world_position);

    // Calculate Fresnel with bias
    let fresnel = calculate_fresnel(view_dir, perturbed_normal, 5.0);

    // Sample reflection with distorted coordinates
    let distortion = perturbed_normal.xy * water_uniforms.wave_distortion;
    let reflection_uv = in.uv + distortion;
    let reflection_color = textureSample(reflection_texture, reflection_sampler, reflection_uv).rgb;

    // Blend water color with reflection based on Fresnel
    let water_base = water_uniforms.water_color;
    let final_color = mix(water_base, reflection_color, fresnel);

    return vec4<f32>(final_color, 0.8);  // Semi-transparent water
}

// ============================================================================
// Planar Reflection Matrix Calculation (CPU-side equivalent)
// ============================================================================

// Helper function to check if point is above reflection plane
fn is_point_above_plane(point: vec3<f32>, plane_normal: vec3<f32>, plane_distance: f32) -> bool {
    return dot(plane_normal, point) + plane_distance > 0.0;
}

// Reflect a vector across a plane
fn reflect_vector_across_plane(v: vec3<f32>, plane_normal: vec3<f32>) -> vec3<f32> {
    return v - 2.0 * dot(v, plane_normal) * plane_normal;
}
