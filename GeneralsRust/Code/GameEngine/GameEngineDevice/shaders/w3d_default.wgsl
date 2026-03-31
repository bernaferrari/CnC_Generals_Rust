// W3D Default PBR Shader - Modern WGSL Implementation
// This shader provides complete W3D compatibility with modern PBR rendering

// Uniform buffer structures
struct W3DUniforms {
    mvp_matrix: mat4x4<f32>,
    model_matrix: mat4x4<f32>,
    view_matrix: mat4x4<f32>,
    projection_matrix: mat4x4<f32>,
    normal_matrix: mat4x4<f32>,
    camera_position: vec4<f32>,
    time: f32,
    delta_time: f32,
    _padding: vec2<f32>,
}

struct W3DMaterial {
    base_color: vec4<f32>,
    material_params: vec4<f32>, // metallic, roughness, ao, fixed-function unlit flag
    emissive: vec4<f32>,
    texture_params: vec4<f32>, // normal_scale, height_scale, detail_blend_mode, unused
}

struct W3DLight {
    position_or_direction: vec4<f32>, // w=1 for position, w=0 for direction
    color_intensity: vec4<f32>,
    attenuation: vec4<f32>, // constant, linear, quadratic, range
    spot_params: vec4<f32>, // inner_cos, outer_cos, unused, unused
    light_type: u32, // 0=directional, 1=point, 2=spot, 3=area
    cast_shadows: u32,
    _padding: vec2<u32>,
}

// Bind groups
@group(0) @binding(0) var<uniform> uniforms: W3DUniforms;
@group(0) @binding(1) var<uniform> material: W3DMaterial;
@group(0) @binding(2) var<uniform> lights: array<W3DLight, 256>;

@group(1) @binding(0) var diffuse_texture: texture_2d<f32>;
@group(1) @binding(1) var diffuse_sampler: sampler;
@group(1) @binding(2) var normal_texture: texture_2d<f32>;
@group(1) @binding(3) var normal_sampler: sampler;
@group(1) @binding(4) var detail_texture: texture_2d<f32>;
@group(1) @binding(5) var detail_sampler: sampler;

// Vertex input structure (matches W3DVertex)
struct VertexInput {
    @location(0) position: vec4<f32>,
    @location(1) normal: vec4<f32>,
    @location(2) tex_coords: vec4<f32>,
    @location(3) color: vec4<f32>,
    @location(4) bone_indices: vec4<u32>,
    @location(5) bone_weights: vec4<f32>,
}

// Vertex output / Fragment input
struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) world_position: vec3<f32>,
    @location(1) world_normal: vec3<f32>,
    @location(2) tex_coords: vec2<f32>,
    @location(3) vertex_color: vec4<f32>,
    @location(4) tangent: vec3<f32>,
    @location(5) bitangent: vec3<f32>,
    @location(6) tex_coords2: vec2<f32>,
}

// G-Buffer output for deferred rendering
struct GBufferOutput {
    @location(0) albedo: vec4<f32>,
    @location(1) normal: vec4<f32>,
    @location(2) material_props: vec4<f32>,
}

// Utility functions for PBR
fn calculate_tangent_space(world_normal: vec3<f32>, tex_coords: vec2<f32>) -> mat3x3<f32> {
    // Calculate tangent and bitangent from world normal and texture coordinates
    let dp1 = dpdx(tex_coords);
    let dp2 = dpdy(tex_coords);
    let du1 = dp1.x;
    let dv1 = dp1.y;
    let du2 = dp2.x;
    let dv2 = dp2.y;
    
    let dp2perp = cross(world_normal, vec3<f32>(dp2.x, dp2.y, 0.0));
    let dp1perp = cross(vec3<f32>(dp1.x, dp1.y, 0.0), world_normal);
    
    let tangent = normalize(dp2perp * du1 + dp1perp * du2);
    let bitangent = normalize(cross(world_normal, tangent));
    
    return mat3x3<f32>(tangent, bitangent, world_normal);
}

fn sample_normal_map(normal_tex: texture_2d<f32>, normal_sam: sampler, uv: vec2<f32>, tbn: mat3x3<f32>) -> vec3<f32> {
    let normal_sample = textureSample(normal_tex, normal_sam, uv).xyz;
    let normal_map = normalize(normal_sample * 2.0 - 1.0);
    return normalize(tbn * normal_map);
}

// Fresnel-Schlick approximation
fn fresnel_schlick(cos_theta: f32, f0: vec3<f32>) -> vec3<f32> {
    return f0 + (1.0 - f0) * pow(clamp(1.0 - cos_theta, 0.0, 1.0), 5.0);
}

// Normal distribution function (GGX/Trowbridge-Reitz)
fn distribution_ggx(normal: vec3<f32>, halfway: vec3<f32>, roughness: f32) -> f32 {
    let a = roughness * roughness;
    let a2 = a * a;
    let n_dot_h = max(dot(normal, halfway), 0.0);
    let n_dot_h2 = n_dot_h * n_dot_h;
    
    let num = a2;
    let denom = (n_dot_h2 * (a2 - 1.0) + 1.0);
    let denom2 = 3.14159265 * denom * denom;
    
    return num / denom2;
}

// Geometry function (Smith's method)
fn geometry_schlick_ggx(n_dot_v: f32, roughness: f32) -> f32 {
    let r = (roughness + 1.0);
    let k = (r * r) / 8.0;
    
    let num = n_dot_v;
    let denom = n_dot_v * (1.0 - k) + k;
    
    return num / denom;
}

fn geometry_smith(normal: vec3<f32>, view_dir: vec3<f32>, light_dir: vec3<f32>, roughness: f32) -> f32 {
    let n_dot_v = max(dot(normal, view_dir), 0.0);
    let n_dot_l = max(dot(normal, light_dir), 0.0);
    let ggx2 = geometry_schlick_ggx(n_dot_v, roughness);
    let ggx1 = geometry_schlick_ggx(n_dot_l, roughness);
    
    return ggx1 * ggx2;
}

// Cook-Torrance BRDF
fn cook_torrance_brdf(
    normal: vec3<f32>,
    view_dir: vec3<f32>,
    light_dir: vec3<f32>,
    albedo: vec3<f32>,
    metallic: f32,
    roughness: f32
) -> vec3<f32> {
    let halfway = normalize(view_dir + light_dir);
    
    // Calculate F0 (surface reflection at zero incidence)
    var f0 = vec3<f32>(0.04);
    f0 = mix(f0, albedo, metallic);
    
    // Calculate the BRDF components
    let ndf = distribution_ggx(normal, halfway, roughness);
    let g = geometry_smith(normal, view_dir, light_dir, roughness);
    let f = fresnel_schlick(max(dot(halfway, view_dir), 0.0), f0);
    
    let n_dot_l = max(dot(normal, light_dir), 0.0);
    let n_dot_v = max(dot(normal, view_dir), 0.0);
    
    let numerator = ndf * g * f;
    let denominator = 4.0 * n_dot_v * n_dot_l + 0.0001;
    let specular = numerator / denominator;
    
    let ks = f;
    let kd = vec3<f32>(1.0) - ks;
    let kd_final = kd * (1.0 - metallic);
    
    return (kd_final * albedo / 3.14159265 + specular) * n_dot_l;
}

// Vertex Shader
@vertex
fn vs_main(input: VertexInput) -> VertexOutput {
    var output: VertexOutput;
    
    // Transform position to clip space
    output.clip_position = uniforms.mvp_matrix * input.position;
    
    // World space position
    output.world_position = (uniforms.model_matrix * input.position).xyz;
    
    // Transform normal to world space
    output.world_normal = normalize((uniforms.normal_matrix * input.normal).xyz);
    
    // Pass through texture coordinates
    output.tex_coords = input.tex_coords.xy;

    // Pass through second UV set for multi-texture blending
    output.tex_coords2 = input.tex_coords.zw;

    // Pass through vertex color
    output.vertex_color = input.color;
    
    // Calculate tangent space (simplified - could be improved with proper tangent attributes)
    let tbn = calculate_tangent_space(output.world_normal, output.tex_coords);
    output.tangent = tbn[0];
    output.bitangent = tbn[1];
    
    return output;
}

// Fragment Shader - Forward Rendering Path
@fragment
fn fs_main(input: VertexOutput) -> @location(0) vec4<f32> {
    // Sample base textures
    let albedo_sample = textureSample(diffuse_texture, diffuse_sampler, input.tex_coords);
    var base_albedo = material.base_color.rgb * albedo_sample.rgb * input.vertex_color.rgb;
    let alpha = albedo_sample.a * material.base_color.a;

    // Multi-texture detail blending (DX8 fixed-function Stage 1)
    let blend_mode = material.texture_params.z;
    if (blend_mode > 0.5 && blend_mode < 1.5) {
        // Mode 1: MODULATE — result = base * detail
        let detail_sample = textureSample(detail_texture, detail_sampler, input.tex_coords2);
        base_albedo = base_albedo * detail_sample.rgb;
    } else if (blend_mode > 1.5 && blend_mode < 2.5) {
        // Mode 2: ADDSIGNED — result = base + detail - 0.5
        let detail_sample = textureSample(detail_texture, detail_sampler, input.tex_coords2);
        base_albedo = base_albedo + detail_sample.rgb - vec3<f32>(0.5);
    } else if (blend_mode > 2.5 && blend_mode < 3.5) {
        // Mode 3: BLENDCURRENTALPHA — result = lerp(base, detail, detail.a)
        let detail_sample = textureSample(detail_texture, detail_sampler, input.tex_coords2);
        base_albedo = mix(base_albedo, detail_sample.rgb, detail_sample.a);
    }

    if (material.material_params.w > 0.5) {
        let fixed_function_color = clamp(
            base_albedo + material.emissive.rgb * material.emissive.w,
            vec3<f32>(0.0),
            vec3<f32>(1.0),
        );
        return vec4<f32>(fixed_function_color, alpha);
    }
    
    // Sample normal map
    let tbn = mat3x3<f32>(input.tangent, input.bitangent, input.world_normal);
    let world_normal = sample_normal_map(normal_texture, normal_sampler, input.tex_coords, tbn);
    
    // Material properties
    let metallic = material.material_params.x;
    let roughness = max(material.material_params.y, 0.04);
    let ao = material.material_params.z;
    
    // View direction
    let view_dir = normalize(uniforms.camera_position.xyz - input.world_position);
    
    // Lighting calculation
    var final_color = vec3<f32>(0.0);
    
    // Ambient lighting
    let ambient = vec3<f32>(0.03) * base_albedo * ao;
    final_color += ambient;
    
    // Dynamic lighting
    for (var i = 0u; i < 256u; i++) {
        let light = lights[i];
        
        // Skip inactive lights (light_type = 255 indicates inactive)
        if (light.light_type > 3u) {
            break;
        }
        
        var light_dir: vec3<f32>;
        var attenuation = 1.0;
        
        if (light.light_type == 0u) { // Directional light
            light_dir = normalize(-light.position_or_direction.xyz);
        } else { // Point/Spot light
            let light_vec = light.position_or_direction.xyz - input.world_position;
            let distance = length(light_vec);
            light_dir = light_vec / distance;
            
            // Calculate attenuation
            let constant = light.attenuation.x;
            let linear = light.attenuation.y;
            let quadratic = light.attenuation.z;
            attenuation = 1.0 / (constant + linear * distance + quadratic * (distance * distance));
            
            // Spot light cone
            if (light.light_type == 2u) {
                let spot_dir = normalize(-light.position_or_direction.xyz);
                let theta = dot(light_dir, spot_dir);
                let epsilon = light.spot_params.x - light.spot_params.y;
                let intensity = clamp((theta - light.spot_params.y) / epsilon, 0.0, 1.0);
                attenuation *= intensity;
            }
        }
        
        // Calculate PBR lighting
        let radiance = light.color_intensity.rgb * light.color_intensity.w * attenuation;
        let brdf_contrib = cook_torrance_brdf(world_normal, view_dir, light_dir, base_albedo, metallic, roughness);
        final_color += brdf_contrib * radiance;
    }
    
    // Add emissive
    final_color += material.emissive.rgb * material.emissive.w;
    
    // Simple tone mapping (Reinhard)
    final_color = final_color / (final_color + vec3<f32>(1.0));
    
    // Gamma correction
    final_color = pow(final_color, vec3<f32>(1.0/2.2));
    
    return vec4<f32>(final_color, alpha);
}

// G-Buffer Fragment Shader - Deferred Rendering Path
@fragment
fn fs_gbuffer(input: VertexOutput) -> GBufferOutput {
    var output: GBufferOutput;
    
    // Sample textures
    let albedo_sample = textureSample(diffuse_texture, diffuse_sampler, input.tex_coords);
    var base_albedo = material.base_color.rgb * albedo_sample.rgb * input.vertex_color.rgb;

    // Multi-texture detail blending (same logic as forward path)
    let blend_mode = material.texture_params.z;
    if (blend_mode > 0.5 && blend_mode < 1.5) {
        let detail_sample = textureSample(detail_texture, detail_sampler, input.tex_coords2);
        base_albedo = base_albedo * detail_sample.rgb;
    } else if (blend_mode > 1.5 && blend_mode < 2.5) {
        let detail_sample = textureSample(detail_texture, detail_sampler, input.tex_coords2);
        base_albedo = base_albedo + detail_sample.rgb - vec3<f32>(0.5);
    } else if (blend_mode > 2.5 && blend_mode < 3.5) {
        let detail_sample = textureSample(detail_texture, detail_sampler, input.tex_coords2);
        base_albedo = mix(base_albedo, detail_sample.rgb, detail_sample.a);
    }
    
    // Sample and encode normal
    let tbn = mat3x3<f32>(input.tangent, input.bitangent, input.world_normal);
    let world_normal = sample_normal_map(normal_texture, normal_sampler, input.tex_coords, tbn);
    let encoded_normal = world_normal * 0.5 + 0.5;
    
    // Pack G-Buffer data
    output.albedo = vec4<f32>(base_albedo, material.material_params.x); // RGB + metallic
    output.normal = vec4<f32>(encoded_normal, material.material_params.y); // Normal + roughness  
    output.material_props = vec4<f32>(material.material_params.z, material.emissive.w, material.material_params.w, 1.0); // AO + emissive + fixed-function unlit flag + unused
    
    return output;
}
