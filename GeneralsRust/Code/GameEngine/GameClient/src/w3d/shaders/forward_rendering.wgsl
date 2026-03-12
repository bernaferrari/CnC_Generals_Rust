// W3D Forward Rendering Shader
// Forward rendering with PBR lighting for transparent objects and particles

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

const MAX_BONES: u32 = 256u;

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

@group(3) @binding(0)
var<storage, read> bone_matrices: array<mat4x4<f32>>;

struct VertexInput {
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) uv: vec2<f32>,
    @location(3) color: vec4<f32>,
    @location(4) bone_indices: vec4<u32>,
    @location(5) bone_weights: vec4<f32>,
};

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) world_position: vec3<f32>,
    @location(1) world_normal: vec3<f32>,
    @location(2) uv: vec2<f32>,
    @location(3) color: vec4<f32>,
};

@vertex
fn vs_main(vertex: VertexInput) -> VertexOutput {
    var out: VertexOutput;
    
    let weight_sum = vertex.bone_weights.x + vertex.bone_weights.y + vertex.bone_weights.z + vertex.bone_weights.w;
    var weights = vertex.bone_weights / max(weight_sum, 0.0001);
    if (weight_sum < 0.0001) {
        weights = vec4<f32>(1.0, 0.0, 0.0, 0.0);
    }

    let i0 = min(vertex.bone_indices.x, MAX_BONES - 1u);
    let i1 = min(vertex.bone_indices.y, MAX_BONES - 1u);
    let i2 = min(vertex.bone_indices.z, MAX_BONES - 1u);
    let i3 = min(vertex.bone_indices.w, MAX_BONES - 1u);

    let skinned_position =
        (bone_matrices[i0] * vec4<f32>(vertex.position, 1.0)) * weights.x +
        (bone_matrices[i1] * vec4<f32>(vertex.position, 1.0)) * weights.y +
        (bone_matrices[i2] * vec4<f32>(vertex.position, 1.0)) * weights.z +
        (bone_matrices[i3] * vec4<f32>(vertex.position, 1.0)) * weights.w;
    let skinned_normal =
        (bone_matrices[i0] * vec4<f32>(vertex.normal, 0.0)) * weights.x +
        (bone_matrices[i1] * vec4<f32>(vertex.normal, 0.0)) * weights.y +
        (bone_matrices[i2] * vec4<f32>(vertex.normal, 0.0)) * weights.z +
        (bone_matrices[i3] * vec4<f32>(vertex.normal, 0.0)) * weights.w;

    let world_position = skinned_position;
    out.world_position = world_position.xyz;
    out.world_normal = normalize(skinned_normal.xyz);
    out.uv = vertex.uv;
    out.color = vertex.color;
    out.clip_position = camera.view_projection_matrix * world_position;
    
    return out;
}

// PBR functions (same as deferred shader)
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
        attenuation = 1.0 / (distance * distance);
        attenuation = attenuation * smoothstep(light.range, 0.0, distance);
    } else if (light.light_type == 2u) {
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

fn compute_tbn(world_pos: vec3<f32>, uv: vec2<f32>, normal: vec3<f32>) -> mat3x3<f32> {
    let dp1 = dpdx(world_pos);
    let dp2 = dpdy(world_pos);
    let duv1 = dpdx(uv);
    let duv2 = dpdy(uv);

    let n = normalize(normal);
    let t = normalize(dp1 * duv2.y - dp2 * duv1.y);
    let tangent = normalize(t - n * dot(n, t));
    let bitangent = normalize(cross(n, tangent));

    return mat3x3<f32>(tangent, bitangent, n);
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    // Sample material textures
    let albedo_sample = textureSample(t_albedo, s_material, in.uv);
    let normal_sample = textureSample(t_normal, s_material, in.uv);
    
    // Material properties
    let albedo = albedo_sample.rgb * in.color.rgb;
    let alpha = albedo_sample.a * in.color.a;
    let metallic = 0.0; // Could sample from a metallic texture
    let roughness = 0.5; // Could sample from a roughness texture
    
    // Normal mapping
    let normal_map = normalize(normal_sample.rgb * 2.0 - 1.0);
    let tbn = compute_tbn(in.world_position, in.uv, in.world_normal);
    let world_normal = normalize(tbn * normal_map);
    
    // Calculate lighting
    var final_color = vec3<f32>(0.0);
    
    // Process first few lights (would be optimized in a real implementation)
    for (var i = 0u; i < 4u; i = i + 1u) {
        if (lights[i].color_intensity.a > 0.0) {
            final_color = final_color + calculate_pbr_lighting(
                albedo,
                metallic,
                roughness,
                in.world_position,
                world_normal,
                camera.camera_position,
                lights[i]
            );
        }
    }
    
    // Add ambient lighting
    let ambient = vec3<f32>(0.03) * albedo;
    final_color = final_color + ambient;
    
    return vec4<f32>(final_color, alpha);
}
