// Modern WGSL Shader for GameEngineDevice
// This shader provides physically-based rendering with modern graphics features

struct VertexInput {
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) tex_coords: vec2<f32>,
    @location(3) color: vec4<f32>,
}

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) world_position: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) tex_coords: vec2<f32>,
    @location(3) color: vec4<f32>,
    @location(4) tangent: vec3<f32>,
    @location(5) view_direction: vec3<f32>,
}

struct CameraUniform {
    view_matrix: mat4x4<f32>,
    projection_matrix: mat4x4<f32>,
    view_proj_matrix: mat4x4<f32>,
    camera_position: vec4<f32>,
    view_direction: vec4<f32>,
}

struct ModelUniform {
    model_matrix: mat4x4<f32>,
    normal_matrix: mat4x4<f32>,
    mvp_matrix: mat4x4<f32>,
    color: vec4<f32>,
}

struct MaterialUniform {
    ambient: vec4<f32>,
    diffuse: vec4<f32>,
    specular: vec4<f32>,
    emission: vec4<f32>,
    shininess: f32,
    metallic: f32,
    roughness: f32,
    ao: f32,
    normal_scale: f32,
    displacement_scale: f32,
    alpha_cutoff: f32,
    _padding: f32,
}

struct LightUniform {
    position: vec4<f32>,
    direction: vec4<f32>,
    color: vec4<f32>,
    intensity: f32,
    range: f32,
    inner_cone: f32,
    outer_cone: f32,
    constant_attenuation: f32,
    linear_attenuation: f32,
    quadratic_attenuation: f32,
    light_type: u32, // 0=directional, 1=point, 2=spot
}

struct EnvironmentUniform {
    ambient_color: vec4<f32>,
    fog_color: vec4<f32>,
    fog_density: f32,
    fog_start: f32,
    fog_end: f32,
    time: f32,
}

// Bind groups
@group(0) @binding(0) var<uniform> camera: CameraUniform;
@group(1) @binding(0) var<uniform> model: ModelUniform;
@group(2) @binding(0) var<uniform> material: MaterialUniform;
@group(2) @binding(1) var<uniform> light: LightUniform;
@group(2) @binding(2) var<uniform> environment: EnvironmentUniform;

// Textures and samplers
@group(3) @binding(0) var diffuse_texture: texture_2d<f32>;
@group(3) @binding(1) var diffuse_sampler: sampler;
@group(3) @binding(2) var normal_texture: texture_2d<f32>;
@group(3) @binding(3) var normal_sampler: sampler;
@group(3) @binding(4) var metallic_roughness_texture: texture_2d<f32>;
@group(3) @binding(5) var metallic_roughness_sampler: sampler;
@group(3) @binding(6) var ao_texture: texture_2d<f32>;
@group(3) @binding(7) var ao_sampler: sampler;
@group(3) @binding(8) var emission_texture: texture_2d<f32>;
@group(3) @binding(9) var emission_sampler: sampler;
@group(3) @binding(10) var environment_map: texture_cube<f32>;
@group(3) @binding(11) var environment_sampler: sampler;

// Vertex shader
@vertex
fn vs_main(input: VertexInput) -> VertexOutput {
    var out: VertexOutput;
    
    let world_position = model.model_matrix * vec4<f32>(input.position, 1.0);
    out.world_position = world_position.xyz;
    out.clip_position = camera.view_proj_matrix * world_position;
    
    // Transform normal to world space
    let world_normal = (model.normal_matrix * vec4<f32>(input.normal, 0.0)).xyz;
    out.normal = normalize(world_normal);
    
    out.tex_coords = input.tex_coords;
    out.color = input.color * model.color;
    
    // Calculate tangent space (simplified)
    // In a complete implementation, you'd pass tangent as vertex attribute
    let c1 = cross(out.normal, vec3<f32>(0.0, 0.0, 1.0));
    let c2 = cross(out.normal, vec3<f32>(0.0, 1.0, 0.0));
    
    if (length(c1) > length(c2)) {
        out.tangent = normalize(c1);
    } else {
        out.tangent = normalize(c2);
    }
    
    // View direction for specular calculations
    out.view_direction = normalize(camera.camera_position.xyz - out.world_position);
    
    return out;
}

// Fragment shader with PBR
@fragment
fn fs_main(input: VertexOutput) -> @location(0) vec4<f32> {
    let normal = normalize(input.normal);
    let view_dir = normalize(input.view_direction);
    
    // Sample textures
    let base_color = textureSample(diffuse_texture, diffuse_sampler, input.tex_coords);
    let normal_map = textureSample(normal_texture, normal_sampler, input.tex_coords);
    let metallic_roughness = textureSample(metallic_roughness_texture, metallic_roughness_sampler, input.tex_coords);
    let ao = textureSample(ao_texture, ao_sampler, input.tex_coords).r;
    let emission = textureSample(emission_texture, emission_sampler, input.tex_coords);
    
    // Unpack normal from normal map
    let tangent = normalize(input.tangent);
    let bitangent = cross(normal, tangent);
    let tbn = mat3x3<f32>(tangent, bitangent, normal);
    
    let normal_sample = normal_map.xyz * 2.0 - 1.0;
    let world_normal = normalize(tbn * (normal_sample * vec3<f32>(material.normal_scale, material.normal_scale, 1.0)));
    
    // Material properties
    let albedo = base_color.rgb * material.diffuse.rgb * input.color.rgb;
    let metallic = metallic_roughness.b * material.metallic;
    let roughness = metallic_roughness.g * material.roughness;
    let alpha = base_color.a * material.diffuse.a * input.color.a;
    
    // Alpha test
    if (alpha < material.alpha_cutoff) {
        discard;
    }
    
    // PBR calculations
    let f0 = mix(vec3<f32>(0.04), albedo, metallic);
    
    // Light direction
    var light_dir: vec3<f32>;
    var light_distance: f32 = 1.0;
    var attenuation: f32 = 1.0;
    
    if (light.light_type == 0u) {
        // Directional light
        light_dir = normalize(-light.direction.xyz);
    } else {
        // Point or spot light
        let light_vec = light.position.xyz - input.world_position;
        light_distance = length(light_vec);
        light_dir = light_vec / light_distance;
        
        // Attenuation
        attenuation = 1.0 / (light.constant_attenuation + 
                            light.linear_attenuation * light_distance + 
                            light.quadratic_attenuation * light_distance * light_distance);
        
        // Spot light cone
        if (light.light_type == 2u) {
            let spot_factor = dot(-light_dir, normalize(light.direction.xyz));
            let spot_attenuation = smoothstep(light.outer_cone, light.inner_cone, spot_factor);
            attenuation *= spot_attenuation;
        }
    }
    
    let half_dir = normalize(light_dir + view_dir);
    let ndotl = max(dot(world_normal, light_dir), 0.0);
    let ndotv = max(dot(world_normal, view_dir), 0.0);
    let ndoth = max(dot(world_normal, half_dir), 0.0);
    let hdotv = max(dot(half_dir, view_dir), 0.0);
    
    // Fresnel (Schlick approximation)
    let fresnel = f0 + (1.0 - f0) * pow(1.0 - hdotv, 5.0);
    
    // Normal Distribution Function (GGX/Trowbridge-Reitz)
    let alpha_roughness = roughness * roughness;
    let alpha2 = alpha_roughness * alpha_roughness;
    let denom = ndoth * ndoth * (alpha2 - 1.0) + 1.0;
    let ndf = alpha2 / (3.14159265 * denom * denom);
    
    // Geometry Function (Smith)
    let k = (roughness + 1.0) * (roughness + 1.0) / 8.0;
    let g1l = ndotl / (ndotl * (1.0 - k) + k);
    let g1v = ndotv / (ndotv * (1.0 - k) + k);
    let geometry = g1l * g1v;
    
    // BRDF
    let numerator = ndf * geometry * fresnel;
    let denominator = max(4.0 * ndotv * ndotl, 0.001);
    let specular = numerator / denominator;
    
    // Lambertian diffuse
    let ks = fresnel;
    let kd = (1.0 - ks) * (1.0 - metallic);
    let diffuse = kd * albedo / 3.14159265;
    
    // Combine lighting
    let radiance = light.color.rgb * light.intensity * attenuation;
    let color = (diffuse + specular) * radiance * ndotl;
    
    // Ambient lighting (simplified)
    let ambient_strength = 0.03;
    let ambient = environment.ambient_color.rgb * albedo * ambient_strength * ao;
    
    // Environment mapping (simplified)
    let reflect_dir = reflect(-view_dir, world_normal);
    let env_color = textureSample(environment_map, environment_sampler, reflect_dir).rgb;
    let env_contribution = env_color * fresnel * (1.0 - roughness) * 0.1;
    
    // Emission
    let emissive = emission.rgb * material.emission.rgb;
    
    // Final color
    var final_color = ambient + color + env_contribution + emissive;
    
    // Fog
    let fog_distance = length(input.world_position - camera.camera_position.xyz);
    let fog_factor = exp(-environment.fog_density * fog_distance);
    final_color = mix(environment.fog_color.rgb, final_color, fog_factor);
    
    // Tone mapping (ACES)
    final_color = aces_tonemap(final_color);
    
    // Gamma correction
    final_color = pow(final_color, vec3<f32>(1.0 / 2.2));
    
    return vec4<f32>(final_color, alpha);
}

// ACES tone mapping
fn aces_tonemap(color: vec3<f32>) -> vec3<f32> {
    let a = 2.51;
    let b = 0.03;
    let c = 2.43;
    let d = 0.59;
    let e = 0.14;
    
    return clamp((color * (a * color + b)) / (color * (c * color + d) + e), vec3<f32>(0.0), vec3<f32>(1.0));
}

// Compute shader for post-processing effects
@compute @workgroup_size(8, 8)
fn cs_post_process(@builtin(global_invocation_id) global_id: vec3<u32>) {
    // Post-processing compute shader implementation
    // This would handle effects like bloom, screen-space reflections, etc.
    
    let screen_size = textureDimensions(diffuse_texture);
    let uv = vec2<f32>(global_id.xy) / vec2<f32>(screen_size);
    
    if (global_id.x >= screen_size.x || global_id.y >= screen_size.y) {
        return;
    }
    
    // Sample input texture
    let color = textureSample(diffuse_texture, diffuse_sampler, uv);
    
    // Apply post-processing effects here
    // For now, just copy the color
    // textureStore(output_texture, global_id.xy, color);
}