// W3D Screen Space Ambient Occlusion (SSAO) Shader
// High-quality ambient occlusion using depth buffer and normals

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

@group(0) @binding(0)
var<uniform> camera: CameraData;

@group(1) @binding(0)
var t_depth: texture_2d<f32>;
@group(1) @binding(1)
var t_normal: texture_2d<f32>;
@group(1) @binding(2)
var t_noise: texture_2d<f32>;
@group(1) @binding(3)
var s_sampler: sampler;

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

@vertex
fn vs_main(@builtin(vertex_index) vertex_index: u32) -> VertexOutput {
    var out: VertexOutput;
    
    let x = f32((vertex_index & 1u) << 2u) - 1.0;
    let y = f32((vertex_index & 2u) << 1u) - 1.0;
    
    out.position = vec4<f32>(x, y, 0.0, 1.0);
    out.uv = vec2<f32>((x + 1.0) * 0.5, (1.0 - y) * 0.5);
    
    return out;
}

// Sample kernel for SSAO
const SAMPLE_KERNEL_SIZE: u32 = 16u;
var<private> sample_kernel: array<vec3<f32>, SAMPLE_KERNEL_SIZE> = array<vec3<f32>, SAMPLE_KERNEL_SIZE>(
    vec3<f32>(0.5381, 0.1856, -0.4317), vec3<f32>(-0.1379, 0.2486, 0.4430),
    vec3<f32>(0.3371, 0.5679, -0.0057), vec3<f32>(-0.6999, -0.0451, -0.0019),
    vec3<f32>(0.0689, -0.1598, -0.8547), vec3<f32>(0.0560, 0.0069, -0.1843),
    vec3<f32>(-0.0146, 0.1402, 0.0762), vec3<f32>(0.0100, -0.1924, -0.0344),
    vec3<f32>(-0.3577, -0.5301, -0.4358), vec3<f32>(-0.3169, 0.1063, 0.0158),
    vec3<f32>(0.0103, -0.5869, 0.0046), vec3<f32>(-0.0897, -0.4940, 0.3287),
    vec3<f32>(0.7119, -0.0154, -0.0918), vec3<f32>(-0.0533, 0.0596, -0.5411),
    vec3<f32>(0.0352, -0.0631, 0.5460), vec3<f32>(-0.4776, 0.2847, -0.0271)
);

fn reconstruct_position(uv: vec2<f32>, depth: f32) -> vec3<f32> {
    let ndc = vec4<f32>(uv * 2.0 - 1.0, depth, 1.0);
    let world_pos = camera.inverse_view_projection_matrix * ndc;
    return world_pos.xyz / world_pos.w;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let depth = textureSample(t_depth, s_sampler, in.uv).r;
    if (depth >= 1.0) {
        return vec4<f32>(1.0); // No occlusion for background
    }
    
    let normal = normalize(textureSample(t_normal, s_sampler, in.uv).rgb * 2.0 - 1.0);
    let position = reconstruct_position(in.uv, depth);
    
    // Sample noise texture for rotation
    let noise_scale = vec2<f32>(1920.0 / 4.0, 1080.0 / 4.0); // 4x4 noise texture
    let random_vec = textureSample(t_noise, s_sampler, in.uv * noise_scale).rgb;
    
    // Create TBN matrix for sample space
    let tangent = normalize(random_vec - normal * dot(random_vec, normal));
    let bitangent = cross(normal, tangent);
    let tbn = mat3x3<f32>(tangent, bitangent, normal);
    
    var occlusion = 0.0;
    let radius = 0.5;
    let bias = 0.025;
    
    for (var i = 0u; i < SAMPLE_KERNEL_SIZE; i++) {
        // Get sample position
        let sample_pos = tbn * sample_kernel[i];
        let sample_point = position + sample_pos * radius;
        
        // Project sample position to screen space
        let offset = camera.view_projection_matrix * vec4<f32>(sample_point, 1.0);
        let offset_ndc = offset.xyz / offset.w;
        let sample_uv = offset_ndc.xy * 0.5 + 0.5;
        
        // Sample depth at offset position
        let sample_depth = textureSample(t_depth, s_sampler, sample_uv).r;
        let sample_world_pos = reconstruct_position(sample_uv, sample_depth);
        
        // Range check & accumulate
        let range_check = smoothstep(0.0, 1.0, radius / abs(position.z - sample_world_pos.z));
        occlusion += select(0.0, 1.0, sample_world_pos.z >= sample_point.z + bias) * range_check;
    }
    
    occlusion = 1.0 - (occlusion / f32(SAMPLE_KERNEL_SIZE));
    occlusion = pow(occlusion, 2.0); // Increase contrast
    
    return vec4<f32>(occlusion, occlusion, occlusion, 1.0);
}