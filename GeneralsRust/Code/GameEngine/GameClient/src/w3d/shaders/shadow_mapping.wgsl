// W3D Shadow Mapping Shader
// Depth-only rendering for shadow map generation

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
var<uniform> light_camera: CameraData;

@group(1) @binding(0)
var<storage, read> bone_matrices: array<mat4x4<f32>>;

struct VertexInput {
    @location(0) position: vec3<f32>,
    @location(1) bone_indices: vec4<u32>,
    @location(2) bone_weights: vec4<f32>,
};

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
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

    let world_position =
        (bone_matrices[i0] * vec4<f32>(vertex.position, 1.0)) * weights.x +
        (bone_matrices[i1] * vec4<f32>(vertex.position, 1.0)) * weights.y +
        (bone_matrices[i2] * vec4<f32>(vertex.position, 1.0)) * weights.z +
        (bone_matrices[i3] * vec4<f32>(vertex.position, 1.0)) * weights.w;
    out.clip_position = light_camera.view_projection_matrix * world_position;
    
    return out;
}

// No fragment shader needed - using depth-only rendering
