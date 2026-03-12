// W3D Instanced Rendering Shader
// High-performance rendering for large armies and duplicate objects

struct CameraData {
    view_matrix: mat4x4<f32>,
    projection_matrix: mat4x4<f32>,
    view_projection_matrix: mat4x4<f32>,
    prev_view_projection_matrix: mat4x4<f32>,
    inverse_view_matrix: mat4x4<f32>,
    inverse_projection_matrix: mat4x4<f32>,
    camera_position: vec3<f32>,
    camera_direction: vec3<f32>,
    near_plane: f32,
    far_plane: f32,
    fov: f32,
    aspect_ratio: f32,
};

struct LightData {
    position: vec3<f32>,
    light_type: u32,
    color_intensity: vec4<f32>,
    direction: vec3<f32>,
    range: f32,
    spot_angles: vec2<f32>,
    shadow_index: i32,
    _padding: u32,
};

@group(0) @binding(0)
var<uniform> camera: CameraData;

@group(1) @binding(0)
var<uniform> lights: array<LightData, 256>;

@group(2) @binding(0)
var t_albedo: texture_2d<f32>;
@group(2) @binding(1)
var t_normal: texture_2d<f32>;
@group(2) @binding(2)
var s_material: sampler;

// Per-vertex attributes
struct VertexInput {
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) uv: vec2<f32>,
    @location(3) color: vec4<f32>,
    // Instance attributes
    @location(4) instance_transform_0: vec4<f32>, // First row of transform matrix
    @location(5) instance_transform_1: vec4<f32>, // Second row
    @location(6) instance_transform_2: vec4<f32>, // Third row
    @location(7) instance_transform_3: vec4<f32>, // Fourth row
    @location(8) instance_color: vec4<f32>,       // Per-instance color/team color
    @location(9) instance_data: vec4<f32>,        // Health, animation frame, etc.
};

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) world_position: vec3<f32>,
    @location(1) world_normal: vec3<f32>,
    @location(2) uv: vec2<f32>,
    @location(3) color: vec4<f32>,
    @location(4) instance_color: vec4<f32>,
    @location(5) instance_data: vec4<f32>,
};

@vertex
fn vs_main(input: VertexInput) -> VertexOutput {
    var output: VertexOutput;
    
    // Reconstruct instance transform matrix
    let instance_transform = mat4x4<f32>(
        input.instance_transform_0,
        input.instance_transform_1,
        input.instance_transform_2,
        input.instance_transform_3
    );
    
    // Transform vertex position and normal to world space
    let world_position = instance_transform * vec4<f32>(input.position, 1.0);
    let world_normal = normalize((instance_transform * vec4<f32>(input.normal, 0.0)).xyz);
    
    output.world_position = world_position.xyz;
    output.world_normal = world_normal;
    output.uv = input.uv;
    output.color = input.color;
    output.instance_color = input.instance_color;
    output.instance_data = input.instance_data;
    output.clip_position = camera.view_projection_matrix * world_position;
    
    return output;
}

// PBR lighting functions (reused from other shaders)
fn fresnel_schlick(cos_theta: f32, f0: vec3<f32>) -> vec3<f32> {
    return f0 + (vec3<f32>(1.0) - f0) * pow(clamp(1.0 - cos_theta, 0.0, 1.0), 5.0);
}

fn distribution_ggx(n: vec3<f32>, h: vec3<f32>, roughness: f32) -> f32 {
    let a = roughness * roughness;
    let a2 = a * a;
    let ndoth = max(dot(n, h), 0.0);
    let ndoth2 = ndoth * ndoth;
    
    let num = a2;
    let denom = ndoth2 * (a2 - 1.0) + 1.0;
    let denom_final = 3.14159265 * denom * denom;
    
    return num / denom_final;
}

fn geometry_schlick_ggx(ndotv: f32, roughness: f32) -> f32 {
    let r = roughness + 1.0;
    let k = (r * r) / 8.0;
    
    let num = ndotv;
    let denom = ndotv * (1.0 - k) + k;
    
    return num / denom;
}

fn geometry_smith(n: vec3<f32>, v: vec3<f32>, l: vec3<f32>, roughness: f32) -> f32 {
    let ndotv = max(dot(n, v), 0.0);
    let ndotl = max(dot(n, l), 0.0);
    let ggx2 = geometry_schlick_ggx(ndotv, roughness);
    let ggx1 = geometry_schlick_ggx(ndotl, roughness);
    
    return ggx1 * ggx2;
}

fn calculate_pbr_lighting(
    albedo: vec3<f32>,
    metallic: f32,
    roughness: f32,
    world_pos: vec3<f32>,
    world_normal: vec3<f32>,
    view_pos: vec3<f32>,
    light: LightData
) -> vec3<f32> {
    let v = normalize(view_pos - world_pos);
    let n = normalize(world_normal);
    
    var l: vec3<f32>;
    var attenuation = 1.0;
    
    if (light.light_type == 0u) {
        l = normalize(-light.direction);
    } else if (light.light_type == 1u) {
        l = normalize(light.position - world_pos);
        let distance = length(light.position - world_pos);
        attenuation = 1.0 / (distance * distance + 1.0);
        attenuation = attenuation * smoothstep(light.range, 0.0, distance);
    } else if (light.light_type == 2u) {
        l = normalize(light.position - world_pos);
        let distance = length(light.position - world_pos);
        attenuation = 1.0 / (distance * distance + 1.0);
        
        let spot_dir = normalize(-light.direction);
        let cos_angle = dot(l, spot_dir);
        let inner_cone = cos(light.spot_angles.x);
        let outer_cone = cos(light.spot_angles.y);
        let spot_factor = smoothstep(outer_cone, inner_cone, cos_angle);
        attenuation = attenuation * spot_factor;
    }
    
    let h = normalize(v + l);
    let radiance = light.color_intensity.rgb * light.color_intensity.a * attenuation;
    
    let f0 = mix(vec3<f32>(0.04), albedo, metallic);
    let f = fresnel_schlick(max(dot(h, v), 0.0), f0);
    
    let ndf = distribution_ggx(n, h, roughness);
    let g = geometry_smith(n, v, l, roughness);
    
    let numerator = ndf * g * f;
    let denominator = 4.0 * max(dot(n, v), 0.0) * max(dot(n, l), 0.0) + 0.0001;
    let specular = numerator / denominator;
    
    let ks = f;
    let kd = (vec3<f32>(1.0) - ks) * (1.0 - metallic);
    
    let ndotl = max(dot(n, l), 0.0);
    
    return (kd * albedo / 3.14159265 + specular) * radiance * ndotl;
}

@fragment
fn fs_main(input: VertexOutput) -> @location(0) vec4<f32> {
    // Sample material textures
    let albedo_sample = textureSample(t_albedo, s_material, input.uv);
    let normal_sample = textureSample(t_normal, s_material, input.uv);
    
    // Combine base albedo with vertex color and instance color (team colors)
    let base_albedo = albedo_sample.rgb * input.color.rgb;
    let team_albedo = mix(base_albedo, input.instance_color.rgb, input.instance_color.a);
    
    // Material properties
    let metallic = 0.1;
    let roughness = 0.7;
    
    // Health visualization (optional)
    let health = input.instance_data.x; // Health percentage 0-1
    let health_color = mix(vec3<f32>(1.0, 0.2, 0.2), vec3<f32>(0.2, 1.0, 0.2), health);
    let final_albedo = mix(team_albedo, health_color, 0.1 * (1.0 - health));
    
    // Normal mapping (simplified)
    let world_normal = normalize(input.world_normal);
    
    // Calculate lighting
    var final_color = vec3<f32>(0.0);
    
    // Process lights (simplified to first 4)
    for (var i = 0u; i < 4u; i++) {
        if (lights[i].color_intensity.a > 0.0) {
            final_color += calculate_pbr_lighting(
                final_albedo,
                metallic,
                roughness,
                input.world_position,
                world_normal,
                camera.camera_position,
                lights[i]
            );
        }
    }
    
    // Add ambient lighting
    let ambient = vec3<f32>(0.03) * final_albedo;
    final_color += ambient;
    
    // Distance-based LOD fade (optional)
    let distance_to_camera = length(camera.camera_position - input.world_position);
    let lod_fade = smoothstep(800.0, 1000.0, distance_to_camera);
    let alpha = mix(1.0, 0.0, lod_fade);
    
    return vec4<f32>(final_color, alpha);
}