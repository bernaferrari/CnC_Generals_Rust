// W3D G-Buffer Shader
// Advanced deferred rendering G-Buffer generation with PBR support

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

const MAX_BONES: u32 = 256u;

@group(0) @binding(0)
var<uniform> camera: CameraData;

@group(2) @binding(0)
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
    @location(4) prev_clip_position: vec4<f32>,
};

struct GBufferOutput {
    @location(0) albedo_metallic: vec4<f32>,    // RGB: Albedo, A: Metallic
    @location(1) normal_roughness: vec4<f32>,   // RGB: Normal, A: Roughness
    @location(2) position_ao: vec4<f32>,        // RGB: World Position, A: AO
    @location(3) motion_depth: vec4<f32>,       // RG: Motion Vector, B: Depth, A: Material ID
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
    out.prev_clip_position = camera.prev_view_projection_matrix * world_position;
    
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> GBufferOutput {
    var out: GBufferOutput;
    
    // Sample material properties (would come from textures)
    let albedo = in.color.rgb;
    let metallic = 0.0;
    let roughness = 0.5;
    let ao = 1.0;
    let material_id = 0.0;
    
    // Pack G-Buffer data
    out.albedo_metallic = vec4<f32>(albedo, metallic);
    out.normal_roughness = vec4<f32>(normalize(in.world_normal) * 0.5 + 0.5, roughness);
    out.position_ao = vec4<f32>(in.world_position, ao);
    
    // Calculate motion vectors
    let current_ndc = in.clip_position.xy / in.clip_position.w;
    let prev_ndc = in.prev_clip_position.xy / in.prev_clip_position.w;
    let motion = current_ndc - prev_ndc;
    
    let depth = in.clip_position.z / in.clip_position.w;
    out.motion_depth = vec4<f32>(motion, depth, material_id);
    
    return out;
}
