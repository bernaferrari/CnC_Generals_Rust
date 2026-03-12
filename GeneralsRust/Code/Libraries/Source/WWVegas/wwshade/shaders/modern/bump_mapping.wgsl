// Modern WGSL Bump Mapping Shader for WWShade
// Supports: Windows (Vulkan/DX12), macOS (Metal), Linux (Vulkan), Web (WebGPU)

// Vertex input
struct VertexInput {
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) uv: vec2<f32>,
    @location(3) tangent: vec3<f32>,
    @location(4) color: vec4<f32>,
}

// Vertex output / Fragment input
struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) world_position: vec3<f32>,
    @location(1) world_normal: vec3<f32>,
    @location(2) world_tangent: vec3<f32>,
    @location(3) uv: vec2<f32>,
    @location(4) vertex_color: vec4<f32>,
    @location(5) view_position: vec3<f32>,
}

// Uniform structures
struct CameraUniform {
    view_projection: mat4x4<f32>,
    view_position: vec3<f32>,
}

struct LightUniform {
    position: vec3<f32>,
    color: vec3<f32>,
    intensity: f32,
    direction: vec3<f32>,
}

struct MaterialUniform {
    ambient: vec3<f32>,
    diffuse: vec3<f32>, 
    specular: vec3<f32>,
    shininess: f32,
}

// Bind groups
@group(0) @binding(0) var<uniform> camera: CameraUniform;
@group(0) @binding(1) var<uniform> lights: array<LightUniform, 8>;
@group(0) @binding(2) var<uniform> material: MaterialUniform;
@group(0) @binding(3) var diffuse_texture: texture_2d<f32>;
@group(0) @binding(4) var normal_texture: texture_2d<f32>;
@group(0) @binding(5) var texture_sampler: sampler;

// Vertex shader - transforms vertices to clip space
@vertex
fn vs_main(input: VertexInput) -> VertexOutput {
    var out: VertexOutput;
    
    // Transform position to world space (assume identity transform for now)
    let world_position = input.position;
    
    // Transform to clip space
    out.clip_position = camera.view_projection * vec4<f32>(world_position, 1.0);
    
    // Pass world-space data to fragment shader
    out.world_position = world_position;
    out.world_normal = normalize(input.normal);
    out.world_tangent = normalize(input.tangent);
    out.uv = input.uv;
    out.vertex_color = input.color;
    out.view_position = camera.view_position;
    
    return out;
}

// Fragment shader - performs bump mapping and lighting
@fragment  
fn fs_main(input: VertexOutput) -> @location(0) vec4<f32> {
    // Sample textures
    let diffuse_color = textureSample(diffuse_texture, texture_sampler, input.uv);
    let normal_map = textureSample(normal_texture, texture_sampler, input.uv);
    
    // Decode normal from normal map (from [0,1] to [-1,1])
    let tangent_normal = normalize(normal_map.xyz * 2.0 - 1.0);
    
    // Build tangent-to-world matrix (TBN)
    let N = normalize(input.world_normal);
    let T = normalize(input.world_tangent);
    let B = cross(N, T); // Compute bitangent
    let TBN = mat3x3<f32>(T, B, N);
    
    // Transform normal from tangent space to world space
    let world_normal = TBN * tangent_normal;
    
    // Lighting calculation
    var final_color = material.ambient * diffuse_color.rgb;
    
    // Calculate lighting for each light
    for (var i: i32 = 0; i < 8; i++) {
        let light = lights[i];
        
        // Skip inactive lights (intensity = 0)
        if (light.intensity <= 0.0) {
            continue;
        }
        
        // Calculate light direction
        let light_dir = normalize(light.position - input.world_position);
        
        // Diffuse lighting (Lambertian)
        let diffuse_strength = max(dot(world_normal, light_dir), 0.0);
        let diffuse = diffuse_strength * material.diffuse * light.color * light.intensity;
        
        // Specular lighting (Blinn-Phong)
        let view_dir = normalize(input.view_position - input.world_position);
        let half_dir = normalize(light_dir + view_dir);
        let specular_strength = pow(max(dot(world_normal, half_dir), 0.0), material.shininess);
        let specular = specular_strength * material.specular * light.color * light.intensity;
        
        // Accumulate lighting
        final_color += (diffuse + specular) * diffuse_color.rgb;
    }
    
    // Apply vertex color modulation (for legacy compatibility)
    final_color *= input.vertex_color.rgb;
    
    return vec4<f32>(final_color, diffuse_color.a * input.vertex_color.a);
}