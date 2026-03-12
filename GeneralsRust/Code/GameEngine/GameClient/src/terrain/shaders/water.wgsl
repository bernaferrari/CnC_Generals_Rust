// Water Shader for Command & Conquer Generals Zero Hour
// Animated water surface with reflection, refraction, and flow effects

struct WaterVertexInput {
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) tex_coords: vec2<f32>,
    @location(3) flow_direction: vec2<f32>,
}

struct WaterVertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) world_position: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) tex_coords: vec2<f32>,
    @location(3) flow_direction: vec2<f32>,
    @location(4) view_direction: vec3<f32>,
    @location(5) screen_position: vec4<f32>,
    @location(6) water_depth: f32,
}

struct WaterUniforms {
    view_matrix: mat4x4<f32>,
    projection_matrix: mat4x4<f32>,
    camera_position: vec3<f32>,
    time: f32,
    water_level: f32,
    wave_amplitude: f32,
    wave_frequency: f32,
    wave_speed: f32,
    water_color: vec4<f32>,
    foam_color: vec4<f32>,
    reflection_strength: f32,
    refraction_strength: f32,
    flow_speed: f32,
    foam_threshold: f32,
}

@group(0) @binding(0)
var<uniform> uniforms: WaterUniforms;

@group(0) @binding(1)
var water_normal_map: texture_2d<f32>;

@group(0) @binding(2)
var water_sampler: sampler;

@group(0) @binding(3)
var foam_texture: texture_2d<f32>;

@group(0) @binding(4)
var reflection_texture: texture_2d<f32>;

@group(0) @binding(5)
var refraction_texture: texture_2d<f32>;

@group(0) @binding(6)
var depth_texture: texture_depth_2d;

@group(0) @binding(7)
var caustics_texture: texture_2d<f32>;

@vertex
fn vs_main(input: WaterVertexInput) -> WaterVertexOutput {
    var out: WaterVertexOutput;
    
    // Start with base position
    var world_pos = input.position;
    
    // Apply wave animation
    let wave_time = uniforms.time * uniforms.wave_speed;
    let wave_x = sin(world_pos.x * uniforms.wave_frequency + wave_time) * uniforms.wave_amplitude;
    let wave_z = sin(world_pos.z * uniforms.wave_frequency + wave_time * 0.7) * uniforms.wave_amplitude;
    let wave_xz = sin((world_pos.x + world_pos.z) * uniforms.wave_frequency * 0.5 + wave_time * 1.3) * uniforms.wave_amplitude * 0.5;

    // The live terrain path is X/Z ground with Y-up.
    world_pos.y = uniforms.water_level + wave_x + wave_z + wave_xz;
    
    out.world_position = world_pos;
    
    // Transform to clip space
    let view_pos = uniforms.view_matrix * vec4<f32>(world_pos, 1.0);
    out.clip_position = uniforms.projection_matrix * view_pos;
    out.screen_position = out.clip_position;
    
    // Calculate animated normal based on wave derivatives
    let normal_x = -cos(world_pos.x * uniforms.wave_frequency + wave_time) * uniforms.wave_frequency * uniforms.wave_amplitude;
    let normal_z = -cos(world_pos.z * uniforms.wave_frequency + wave_time * 0.7) * uniforms.wave_frequency * uniforms.wave_amplitude * 0.7;
    let normal_xz = -cos((world_pos.x + world_pos.z) * uniforms.wave_frequency * 0.5 + wave_time * 1.3) * 
                    uniforms.wave_frequency * 0.5 * uniforms.wave_amplitude * 0.5;

    out.normal = normalize(vec3<f32>(normal_x + normal_xz * 0.5, 1.0, normal_z + normal_xz * 0.5));
    
    // Pass through texture coordinates with flow animation
    let flow_offset = input.flow_direction * uniforms.time * uniforms.flow_speed;
    out.tex_coords = input.tex_coords + flow_offset;
    out.flow_direction = input.flow_direction;
    
    // Calculate view direction
    out.view_direction = normalize(uniforms.camera_position - world_pos);
    
    // Calculate water depth (simplified)
    out.water_depth = max(world_pos.y - input.position.y + 1.0, 0.0);
    
    return out;
}

@fragment
fn fs_main(input: WaterVertexOutput) -> @location(0) vec4<f32> {
    // Sample normal map with flow animation
    let time_offset1 = uniforms.time * 0.1;
    let time_offset2 = uniforms.time * 0.07;
    
    let normal1 = textureSample(water_normal_map, water_sampler, input.tex_coords + vec2<f32>(time_offset1, 0.0));
    let normal2 = textureSample(water_normal_map, water_sampler, input.tex_coords * 0.7 + vec2<f32>(-time_offset2, time_offset2));
    
    // Combine normal maps
    let combined_normal = normalize(normal1.xyz + normal2.xyz - 1.0);
    let world_normal = normalize(input.normal + combined_normal * 0.3);
    
    // Calculate screen coordinates for reflection/refraction
    let screen_coords = (input.screen_position.xy / input.screen_position.w) * 0.5 + 0.5;
    let distorted_coords = screen_coords + world_normal.xy * 0.05;
    
    // Sample reflection and refraction
    let reflection = textureSample(reflection_texture, water_sampler, vec2<f32>(1.0 - distorted_coords.x, distorted_coords.y));
    let refraction = textureSample(refraction_texture, water_sampler, distorted_coords);
    
    // Calculate fresnel effect
    let view_dot_normal = max(dot(input.view_direction, world_normal), 0.0);
    let fresnel = pow(1.0 - view_dot_normal, 3.0);
    
    // Mix reflection and refraction based on fresnel and settings
    var water_color = mix(refraction.rgb, reflection.rgb, 
                         fresnel * uniforms.reflection_strength);
    
    // Apply base water color
    water_color = mix(water_color, uniforms.water_color.rgb, 0.3);
    
    // Calculate foam based on wave peaks and shore proximity
    let foam_factor = max(world_normal.y - uniforms.foam_threshold, 0.0) / (1.0 - uniforms.foam_threshold);
    let foam_sample = textureSample(foam_texture, water_sampler, input.tex_coords * 4.0);
    
    // Apply foam
    water_color = mix(water_color, uniforms.foam_color.rgb, foam_factor * foam_sample.r);
    
    // Add caustics effect for shallow water
    if input.water_depth < 10.0 {
        let caustics_coords = input.world_position.xz * 0.1 + uniforms.time * 0.05;
        let caustics = textureSample(caustics_texture, water_sampler, caustics_coords);
        let caustics_strength = (1.0 - input.water_depth / 10.0) * 0.3;
        water_color += caustics.rgb * caustics_strength;
    }
    
    // Calculate water transparency based on depth
    let water_alpha = min(input.water_depth * 0.1 + 0.7, 1.0);
    
    // Add subtle animation to water surface
    let surface_animation = sin(uniforms.time * 2.0 + input.world_position.x * 0.1) * 0.02 + 
                           cos(uniforms.time * 1.5 + input.world_position.z * 0.1) * 0.02;
    water_color += surface_animation;
    
    return vec4<f32>(water_color, water_alpha);
}

// Utility functions for water effects

fn calculate_wave_height(position: vec2<f32>, time: f32, frequency: f32, amplitude: f32, speed: f32) -> f32 {
    let wave1 = sin(position.x * frequency + time * speed) * amplitude;
    let wave2 = sin(position.y * frequency * 0.7 + time * speed * 0.8) * amplitude * 0.6;
    let wave3 = sin((position.x + position.y) * frequency * 0.3 + time * speed * 1.2) * amplitude * 0.4;
    return wave1 + wave2 + wave3;
}

fn calculate_flow_distortion(tex_coords: vec2<f32>, flow_dir: vec2<f32>, time: f32, strength: f32) -> vec2<f32> {
    // Create flowing texture coordinates
    let flow_speed = 0.5;
    let phase0 = fract(time * flow_speed);
    let phase1 = fract(time * flow_speed + 0.5);
    
    let tex0 = tex_coords - flow_dir * phase0;
    let tex1 = tex_coords - flow_dir * phase1;
    
    // Blend between the two phases
    let blend = abs((phase0 - 0.5) * 2.0);
    return mix(tex0, tex1, blend) * strength;
}

fn apply_depth_fade(color: vec3<f32>, depth: f32, fade_start: f32, fade_end: f32, deep_color: vec3<f32>) -> vec3<f32> {
    let depth_factor = clamp((depth - fade_start) / (fade_end - fade_start), 0.0, 1.0);
    return mix(color, deep_color, depth_factor);
}

fn calculate_underwater_caustics(world_pos: vec3<f32>, time: f32, intensity: f32) -> f32 {
    let caustic_scale = 0.2;
    let caustic_speed = 1.0;
    
    let caustic1 = sin(world_pos.x * caustic_scale + time * caustic_speed);
    let caustic2 = sin(world_pos.z * caustic_scale * 0.7 + time * caustic_speed * 0.8);
    let caustic3 = sin((world_pos.x + world_pos.z) * caustic_scale * 0.5 + time * caustic_speed * 1.3);
    
    return (caustic1 + caustic2 + caustic3) * intensity * 0.3 + intensity;
}
