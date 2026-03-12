// W3D Deferred Lighting Shader
// Advanced PBR lighting with multiple light types and shadow mapping

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
var<uniform> lights: array<LightData, 256>;

@group(1) @binding(0)
var t_albedo_metallic: texture_2d<f32>;
@group(1) @binding(1)
var t_normal_roughness: texture_2d<f32>;
@group(1) @binding(2)
var t_position_ao: texture_2d<f32>;
@group(1) @binding(3)
var t_motion_depth: texture_2d<f32>;
@group(1) @binding(4)
var s_gbuffer: sampler;

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

// Fullscreen triangle vertices
@vertex
fn vs_main(@builtin(vertex_index) vertex_index: u32) -> VertexOutput {
    var out: VertexOutput;
    
    // Generate fullscreen triangle
    let x = f32((vertex_index & 1u) << 2u) - 1.0;
    let y = f32((vertex_index & 2u) << 1u) - 1.0;
    
    out.position = vec4<f32>(x, y, 0.0, 1.0);
    out.uv = vec2<f32>((x + 1.0) * 0.5, (1.0 - y) * 0.5);
    
    return out;
}

// PBR BRDF functions
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

// Calculate PBR lighting for a single light
fn calculate_pbr_lighting(
    albedo: vec3<f32>,
    metallic: f32,
    roughness: f32,
    ao: f32,
    world_pos: vec3<f32>,
    world_normal: vec3<f32>,
    view_pos: vec3<f32>,
    light: LightData
) -> vec3<f32> {
    let v = normalize(view_pos - world_pos);
    let n = normalize(world_normal);
    
    // Calculate light direction and attenuation
    var l: vec3<f32>;
    var attenuation = 1.0;
    
    if (light.light_type == 0u) {
        // Directional light
        l = normalize(-light.direction);
    } else if (light.light_type == 1u) {
        // Point light
        l = normalize(light.position - world_pos);
        let distance = length(light.position - world_pos);
        attenuation = 1.0 / (distance * distance);
        attenuation = attenuation * smoothstep(light.range, 0.0, distance);
    } else if (light.light_type == 2u) {
        // Spot light
        l = normalize(light.position - world_pos);
        let distance = length(light.position - world_pos);
        attenuation = 1.0 / (distance * distance);
        
        let spot_dir = normalize(-light.direction);
        let cos_angle = dot(l, spot_dir);
        let inner_cone = cos(light.spot_angles.x);
        let outer_cone = cos(light.spot_angles.y);
        let spot_factor = smoothstep(outer_cone, inner_cone, cos_angle);
        attenuation = attenuation * spot_factor;
    }
    
    let h = normalize(v + l);
    let radiance = light.color_intensity.rgb * light.color_intensity.a * attenuation;
    
    // PBR calculations
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
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    // Sample G-Buffer
    let albedo_metallic = textureSample(t_albedo_metallic, s_gbuffer, in.uv);
    let normal_roughness = textureSample(t_normal_roughness, s_gbuffer, in.uv);
    let position_ao = textureSample(t_position_ao, s_gbuffer, in.uv);
    let motion_depth = textureSample(t_motion_depth, s_gbuffer, in.uv);
    
    // Unpack material properties
    let albedo = albedo_metallic.rgb;
    let metallic = albedo_metallic.a;
    let world_normal = normalize(normal_roughness.rgb * 2.0 - 1.0);
    let roughness = normal_roughness.a;
    let world_pos = position_ao.rgb;
    let ao = position_ao.a;
    let material_id = motion_depth.a;
    
    // Early exit for background pixels
    if (motion_depth.b >= 1.0) {
        discard;
    }
    
    // Camera position would come from uniform in a real implementation
    let view_pos = vec3<f32>(0.0, 0.0, 10.0);
    
    // Accumulate lighting from all active lights
    var final_color = vec3<f32>(0.0);
    
    // For now, assume first 4 lights are active (would be passed via uniform)
    for (var i = 0u; i < 4u; i = i + 1u) {
        if (lights[i].color_intensity.a > 0.0) {
            final_color = final_color + calculate_pbr_lighting(
                albedo,
                metallic,
                roughness,
                ao,
                world_pos,
                world_normal,
                view_pos,
                lights[i]
            );
        }
    }
    
    // Apply ambient occlusion
    final_color = final_color * ao;
    
    // Add basic ambient lighting
    let ambient = vec3<f32>(0.03) * albedo * ao;
    final_color = final_color + ambient;
    
    return vec4<f32>(final_color, 1.0);
}