// Water Shader for WGPU
//
// Based on C++ pixel/vertex shaders from:
// - GameEngineDevice/Source/W3DDevice/GameClient/Water/W3DWater.cpp
// - shaders/wave.vso and wave.pso
//
// This shader implements:
// - Wave animation with sine-based displacement
// - Normal mapping for wave details
// - Reflection and refraction effects
// - Fresnel effect for realistic water appearance
// - Specular highlights
// - Caustics (light patterns underwater)

// Uniform bindings
struct CameraUniforms {
    view_proj: mat4x4<f32>,
    view: mat4x4<f32>,
    projection: mat4x4<f32>,
    camera_pos: vec3<f32>,
    _padding: f32,
}

struct WaterUniforms {
    world_transform: mat4x4<f32>,
    water_color: vec4<f32>,
    water_level: f32,
    time: f32,
    wave_scale: f32,
    wave_speed: f32,
    bump_scale: f32,
    reflection_factor: f32,
    fresnel_bias: f32,
    fresnel_power: f32,
    uv_scroll: vec2<f32>,
    grid_scale: vec2<f32>,
}

struct LightUniforms {
    direction: vec3<f32>,
    _padding1: f32,
    ambient: vec3<f32>,
    _padding2: f32,
    diffuse: vec3<f32>,
    _padding3: f32,
    specular: vec3<f32>,
    specular_power: f32,
}

@group(0) @binding(0)
var<uniform> camera: CameraUniforms;

@group(0) @binding(1)
var<uniform> water: WaterUniforms;

@group(0) @binding(2)
var<uniform> light: LightUniforms;

// Textures
@group(1) @binding(0)
var water_texture: texture_2d<f32>;

@group(1) @binding(1)
var water_sampler: sampler;

@group(1) @binding(2)
var normal_map: texture_2d<f32>;

@group(1) @binding(3)
var normal_sampler: sampler;

@group(1) @binding(4)
var reflection_texture: texture_2d<f32>;

@group(1) @binding(5)
var reflection_sampler: sampler;

@group(1) @binding(6)
var caustics_texture: texture_2d<f32>;

@group(1) @binding(7)
var caustics_sampler: sampler;

// Vertex shader input
struct VertexInput {
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) uv: vec2<f32>,
    @location(3) color: vec4<f32>,
}

// Vertex shader output / Fragment shader input
struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) world_position: vec3<f32>,
    @location(1) world_normal: vec3<f32>,
    @location(2) uv: vec2<f32>,
    @location(3) vertex_color: vec4<f32>,
    @location(4) view_direction: vec3<f32>,
    @location(5) light_direction: vec3<f32>,
    @location(6) screen_position: vec2<f32>,
}

// Wave function - generates sine-based wave displacement
// Matches C++ wave vertex shader logic
fn calculate_wave_offset(pos: vec2<f32>, time: f32) -> f32 {
    let wave1_freq = 0.3;
    let wave1_amp = 0.15;
    let wave2_freq = 0.5;
    let wave2_amp = 0.08;

    // Two overlapping sine waves for more natural appearance
    let wave1 = sin(pos.x * wave1_freq + time * water.wave_speed) *
                cos(pos.y * wave1_freq + time * water.wave_speed * 0.7) * wave1_amp;
    let wave2 = sin(pos.x * wave2_freq - time * water.wave_speed * 0.5) *
                sin(pos.y * wave2_freq + time * water.wave_speed * 0.8) * wave2_amp;

    return (wave1 + wave2) * water.wave_scale;
}

// Calculate wave normal from displacement
fn calculate_wave_normal(pos: vec2<f32>, time: f32) -> vec3<f32> {
    let offset = 0.1;
    let h_center = calculate_wave_offset(pos, time);
    let h_right = calculate_wave_offset(pos + vec2<f32>(offset, 0.0), time);
    let h_forward = calculate_wave_offset(pos + vec2<f32>(0.0, offset), time);

    let tangent = vec3<f32>(offset, 0.0, h_right - h_center);
    let bitangent = vec3<f32>(0.0, offset, h_forward - h_center);

    return normalize(cross(tangent, bitangent));
}

@vertex
fn vs_main(vertex: VertexInput) -> VertexOutput {
    var out: VertexOutput;

    // Apply wave displacement to vertex position
    var world_pos = vec4<f32>(vertex.position, 1.0);
    let wave_offset = calculate_wave_offset(vertex.position.xy, water.time);
    world_pos.z = water.water_level + wave_offset;

    // Transform to world space
    world_pos = water.world_transform * world_pos;
    out.world_position = world_pos.xyz;

    // Calculate wave-modified normal
    let wave_normal = calculate_wave_normal(vertex.position.xy, water.time);
    let world_normal = (water.world_transform * vec4<f32>(wave_normal, 0.0)).xyz;
    out.world_normal = normalize(world_normal);

    // Transform to clip space
    out.clip_position = camera.view_proj * world_pos;

    // Calculate screen position for reflection sampling
    let ndc = out.clip_position.xyz / out.clip_position.w;
    out.screen_position = (ndc.xy + 1.0) * 0.5;
    out.screen_position.y = 1.0 - out.screen_position.y; // Flip Y for texture coordinates

    // Scroll UVs for animated water texture
    out.uv = vertex.uv * water.grid_scale + water.uv_scroll;

    // Pass through vertex color
    out.vertex_color = vertex.color;

    // Calculate view direction
    out.view_direction = normalize(camera.camera_pos - out.world_position);

    // Calculate light direction (directional light)
    out.light_direction = normalize(-light.direction);

    return out;
}

// Fresnel effect - water is more reflective at glancing angles
fn fresnel_schlick(cos_theta: f32, bias: f32, power: f32) -> f32 {
    return bias + (1.0 - bias) * pow(1.0 - cos_theta, power);
}

// Sample normal map with perturbation
fn sample_perturbed_normal(uv: vec2<f32>, time: f32) -> vec3<f32> {
    // Sample two layers of normal map with different scrolling
    let uv1 = uv + vec2<f32>(time * 0.03, time * 0.02);
    let uv2 = uv * 1.5 - vec2<f32>(time * 0.02, time * 0.04);

    let normal1 = textureSample(normal_map, normal_sampler, uv1).xyz * 2.0 - 1.0;
    let normal2 = textureSample(normal_map, normal_sampler, uv2).xyz * 2.0 - 1.0;

    // Blend normals
    let blended = normalize(normal1 + normal2);
    return blended * water.bump_scale;
}

// Calculate caustics pattern
fn calculate_caustics(world_pos: vec3<f32>, time: f32) -> f32 {
    let caustics_uv = world_pos.xy * 0.05 + vec2<f32>(time * 0.01, time * 0.015);
    let caustics1 = textureSample(caustics_texture, caustics_sampler, caustics_uv).r;
    let caustics2 = textureSample(caustics_texture, caustics_sampler, caustics_uv * 1.3 + 0.5).r;
    return (caustics1 * caustics2) * 0.5;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    // Sample base water texture
    let base_color = textureSample(water_texture, water_sampler, in.uv);

    // Get perturbed normal from normal map
    let normal_perturbation = sample_perturbed_normal(in.uv, water.time);
    let surface_normal = normalize(in.world_normal + normal_perturbation);

    // Calculate lighting
    let n_dot_l = max(dot(surface_normal, in.light_direction), 0.0);
    let diffuse = light.diffuse * n_dot_l;

    // Specular highlight (Blinn-Phong)
    let half_vector = normalize(in.light_direction + in.view_direction);
    let n_dot_h = max(dot(surface_normal, half_vector), 0.0);
    let specular = light.specular * pow(n_dot_h, light.specular_power);

    // Calculate reflection
    let reflection_uv = in.screen_position + surface_normal.xy * 0.05; // Distort by normal
    var reflection_color = textureSample(reflection_texture, reflection_sampler, reflection_uv).rgb;

    // Calculate Fresnel effect
    let view_dot_normal = max(dot(in.view_direction, surface_normal), 0.0);
    let fresnel = fresnel_schlick(view_dot_normal, water.fresnel_bias, water.fresnel_power);

    // Calculate caustics (underwater light patterns)
    let caustics = calculate_caustics(in.world_position, water.time);

    // Combine all lighting components
    var final_color = light.ambient;
    final_color += diffuse;
    final_color += specular;
    final_color *= base_color.rgb * water.water_color.rgb * in.vertex_color.rgb;

    // Add reflection with Fresnel
    final_color = mix(final_color, reflection_color, fresnel * water.reflection_factor);

    // Add caustics
    final_color += caustics * vec3<f32>(0.2, 0.3, 0.4) * (1.0 - fresnel);

    // Calculate alpha based on water depth (for transparency near shores)
    let alpha = mix(water.water_color.a, 1.0, fresnel) * in.vertex_color.a;

    return vec4<f32>(final_color, alpha);
}

// Simple water shader for low-detail rendering
@fragment
fn fs_simple(in: VertexOutput) -> @location(0) vec4<f32> {
    let base_color = textureSample(water_texture, water_sampler, in.uv);

    let n_dot_l = max(dot(in.world_normal, in.light_direction), 0.0);
    let diffuse = light.diffuse * n_dot_l;

    var color = (light.ambient + diffuse) * base_color.rgb * water.water_color.rgb;
    color *= in.vertex_color.rgb;

    return vec4<f32>(color, water.water_color.a * in.vertex_color.a);
}
