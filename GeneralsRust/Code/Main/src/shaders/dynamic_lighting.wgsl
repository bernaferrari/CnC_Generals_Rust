// Dynamic lighting shader for authentic C&C effects
// Provides realistic lighting from explosions, muzzle flashes, and environment

struct LightingUniforms {
    view_matrix: mat4x4<f32>,
    projection_matrix: mat4x4<f32>,
    camera_position: vec4<f32>,
    ambient_color: vec4<f32>,  // RGB + intensity
    time: f32,
    num_active_lights: u32,
    shadow_cascade_count: u32,
    _padding: u32,
}

struct GPULight {
    position: vec4<f32>,          // Vec3 + light type (0=point, 1=spot, 2=directional, 3=area)
    direction: vec4<f32>,         // Vec3 + cone angle
    color_intensity: vec4<f32>,   // RGB + intensity
    radius_falloff: vec4<f32>,    // radius, falloff power, cone_softness, shadow_bias
    flicker_pulse: vec4<f32>,     // flicker_speed, flicker_intensity, pulse_speed, pulse_intensity
    flags: vec4<u32>,             // active, cast_shadows, shadow_quality, padding
}

struct ShadowCascade {
    view_proj_matrix: mat4x4<f32>,
    split_distance: f32,
    _padding: vec3<f32>,
}

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) screen_pos: vec2<f32>,
}

@group(0) @binding(0) var<uniform> uniforms: LightingUniforms;
@group(0) @binding(1) var<storage, read> lights: array<GPULight>;
@group(0) @binding(2) var shadow_map: texture_depth_2d_array;
@group(0) @binding(3) var shadow_sampler: sampler_comparison;
@group(0) @binding(4) var<uniform> cascades: array<ShadowCascade, 4>;

// Vertex shader - fullscreen triangle
@vertex
fn vs_main(@builtin(vertex_index) vertex_index: u32) -> VertexOutput {
    // Generate fullscreen triangle coordinates
    let x = f32((vertex_index << 1u) & 2u) - 1.0;
    let y = 1.0 - f32(vertex_index & 2u);
    
    return VertexOutput(
        vec4<f32>(x, y, 0.0, 1.0),
        vec2<f32>(x * 0.5 + 0.5, y * 0.5 + 0.5)
    );
}

// Helper function to calculate light attenuation
fn calculate_attenuation(distance: f32, radius: f32, falloff_power: f32) -> f32 {
    if (distance >= radius) {
        return 0.0;
    }
    
    // Smooth falloff based on falloff type
    if (falloff_power == 1.0) {
        // Linear falloff
        return 1.0 - distance / radius;
    } else if (falloff_power == 2.0) {
        // Quadratic falloff (physically accurate)
        let normalized_dist = distance / radius;
        return 1.0 / (1.0 + normalized_dist * normalized_dist);
    } else if (falloff_power == 3.0) {
        // Exponential falloff
        let normalized_dist = distance / radius;
        return exp(-normalized_dist * 4.0);
    } else {
        // Custom power falloff
        let normalized_dist = distance / radius;
        return pow(1.0 - normalized_dist, falloff_power);
    }
}

// Helper function for spot light cone calculation
fn calculate_spot_cone(light_dir: vec3<f32>, to_light: vec3<f32>, cone_angle: f32, cone_softness: f32) -> f32 {
    let dot_product = dot(normalize(light_dir), normalize(-to_light));
    let cone_cos = cos(cone_angle * 0.5);
    
    if (dot_product < cone_cos) {
        return 0.0;
    }
    
    // Smooth edge based on cone softness
    let inner_cone = cone_cos + (1.0 - cone_cos) * cone_softness;
    if (dot_product > inner_cone) {
        return 1.0;
    }
    
    // Smooth transition
    let edge_factor = (dot_product - cone_cos) / (inner_cone - cone_cos);
    return smoothstep(0.0, 1.0, edge_factor);
}

// Sample shadow map with PCF (Percentage Closer Filtering)
fn sample_shadow_map(world_pos: vec3<f32>, light_index: u32) -> f32 {
    let light = lights[light_index];
    
    // Only sample shadows for shadow-casting lights
    if (light.flags.y == 0u) {
        return 1.0; // No shadows
    }
    
    let view_distance = length(world_pos - uniforms.camera_position.xyz);

    var cascade_index = i32(uniforms.shadow_cascade_count) - 1;
    let cascade_count = min(uniforms.shadow_cascade_count, 4u);
    for (var i = 0u; i < cascade_count; i++) {
        if (view_distance <= cascades[i].split_distance) {
            cascade_index = i32(i);
            break;
        }
    }

    let cascade_matrix = cascades[u32(max(cascade_index, 0))].view_proj_matrix;
    let light_space = cascade_matrix * vec4<f32>(world_pos, 1.0);
    if (light_space.w <= 0.0) {
        return 1.0;
    }

    var shadow_coord = light_space.xyz / light_space.w;
    shadow_coord.xy = shadow_coord.xy * 0.5 + vec2<f32>(0.5, 0.5);
    shadow_coord.y = 1.0 - shadow_coord.y;

    if (shadow_coord.x < 0.0 || shadow_coord.x > 1.0 ||
        shadow_coord.y < 0.0 || shadow_coord.y > 1.0 ||
        shadow_coord.z < 0.0 || shadow_coord.z > 1.0) {
        return 1.0;
    }

    let depth_ref = shadow_coord.z - light.radius_falloff.w;
    let texel = vec2<f32>(1.0 / 1024.0, 1.0 / 1024.0);

    var visibility = 0.0;
    for (var y = -1; y <= 1; y++) {
        for (var x = -1; x <= 1; x++) {
            let offset = vec2<f32>(f32(x), f32(y)) * texel;
            visibility = visibility + textureSampleCompare(
                shadow_map,
                shadow_sampler,
                shadow_coord.xy + offset,
                cascade_index,
                depth_ref
            );
        }
    }

    return visibility / 9.0;
}

// Apply flickering and pulsing effects
fn apply_light_animation(base_intensity: f32, light: GPULight, time: f32) -> f32 {
    var intensity = base_intensity;
    
    // Flicker effect
    if (light.flicker_pulse.x > 0.0) {
        let flicker_phase = time * light.flicker_pulse.x * 6.28318; // 2*PI
        let flicker_noise = sin(flicker_phase) * sin(flicker_phase * 3.7) * 0.3;
        let flicker_value = sin(flicker_phase + flicker_noise);
        intensity *= 1.0 + flicker_value * light.flicker_pulse.y;
    }
    
    // Pulse effect
    if (light.flicker_pulse.z > 0.0) {
        let pulse_phase = time * light.flicker_pulse.z * 6.28318;
        let pulse_value = sin(pulse_phase);
        intensity *= 1.0 + pulse_value * light.flicker_pulse.w;
    }
    
    return max(intensity, 0.0);
}

// Main lighting calculation function
fn calculate_lighting(world_pos: vec3<f32>, world_normal: vec3<f32>, view_dir: vec3<f32>, albedo: vec3<f32>) -> vec3<f32> {
    var final_color = vec3<f32>(0.0);
    
    // Add ambient lighting
    final_color += uniforms.ambient_color.xyz * uniforms.ambient_color.w * albedo;
    
    // Process each dynamic light
    for (var i = 0u; i < uniforms.num_active_lights; i++) {
        let light = lights[i];
        
        // Skip inactive lights
        if (light.flags.x == 0u) {
            continue;
        }
        
        let light_type = u32(light.position.w);
        var light_contribution = vec3<f32>(0.0);
        
        if (light_type == 0u) {
            // Point light
            let to_light = light.position.xyz - world_pos;
            let distance = length(to_light);
            let light_dir = normalize(to_light);
            
            // Calculate attenuation
            let attenuation = calculate_attenuation(distance, light.radius_falloff.x, light.radius_falloff.y);
            if (attenuation <= 0.0) {
                continue;
            }
            
            // Apply lighting animation
            let animated_intensity = apply_light_animation(light.color_intensity.w, light, uniforms.time);
            
            // Basic Lambertian lighting
            let n_dot_l = max(dot(world_normal, light_dir), 0.0);
            
            // Shadow factor
            let shadow_factor = sample_shadow_map(world_pos, i);
            
            light_contribution = light.color_intensity.xyz * animated_intensity * attenuation * n_dot_l * shadow_factor * albedo;
            
        } else if (light_type == 1u) {
            // Spot light
            let to_light = light.position.xyz - world_pos;
            let distance = length(to_light);
            let light_dir = normalize(to_light);
            
            // Calculate attenuation
            let attenuation = calculate_attenuation(distance, light.radius_falloff.x, light.radius_falloff.y);
            if (attenuation <= 0.0) {
                continue;
            }
            
            // Calculate spot cone
            let cone_factor = calculate_spot_cone(light.direction.xyz, to_light, light.direction.w, light.radius_falloff.z);
            if (cone_factor <= 0.0) {
                continue;
            }
            
            // Apply lighting animation
            let animated_intensity = apply_light_animation(light.color_intensity.w, light, uniforms.time);
            
            // Basic Lambertian lighting
            let n_dot_l = max(dot(world_normal, light_dir), 0.0);
            
            // Shadow factor
            let shadow_factor = sample_shadow_map(world_pos, i);
            
            light_contribution = light.color_intensity.xyz * animated_intensity * attenuation * cone_factor * n_dot_l * shadow_factor * albedo;
            
        } else if (light_type == 2u) {
            // Directional light (sun/moon)
            let light_dir = normalize(-light.direction.xyz);
            
            // Apply lighting animation
            let animated_intensity = apply_light_animation(light.color_intensity.w, light, uniforms.time);
            
            // Basic Lambertian lighting
            let n_dot_l = max(dot(world_normal, light_dir), 0.0);
            
            // Directional lights use cascaded shadow map sampling.
            let shadow_factor = sample_shadow_map(world_pos, i);
            
            light_contribution = light.color_intensity.xyz * animated_intensity * n_dot_l * shadow_factor * albedo;
        }
        
        final_color += light_contribution;
    }
    
    return final_color;
}

// Fragment shader - simplified for deferred lighting
@fragment
fn fs_main(input: VertexOutput) -> @location(0) vec4<f32> {
    let screen_pos = input.screen_pos;
    
    // For this simplified version, we'll just output a lighting overlay
    // In a full deferred renderer, we'd sample G-buffer textures here
    
    // Simulate some basic scene lighting for demonstration
    let world_pos = vec3<f32>(
        (screen_pos.x - 0.5) * 100.0,
        (screen_pos.y - 0.5) * 100.0,
        0.0
    );
    
    let world_normal = vec3<f32>(0.0, 0.0, 1.0); // Upward normal
    let view_dir = normalize(uniforms.camera_position.xyz - world_pos);
    let albedo = vec3<f32>(0.8, 0.8, 0.8); // Gray surface
    
    let lit_color = calculate_lighting(world_pos, world_normal, view_dir, albedo);
    
    // Return additive lighting contribution
    return vec4<f32>(lit_color * 0.1, 1.0); // Reduced intensity for overlay
}
