// W3D Depth Pre-pass Shader
// Optimized depth-only rendering for early-z rejection

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

struct VertexInput {
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) uv: vec2<f32>,
};

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) world_position: vec3<f32>,
};

@vertex
fn vs_main(vertex: VertexInput) -> VertexOutput {
    var out: VertexOutput;
    
    let world_position = vec4<f32>(vertex.position, 1.0);
    out.world_position = world_position.xyz;
    out.clip_position = camera.view_projection_matrix * world_position;
    
    return out;
}

// Depth-only, no fragment shader needed for basic depth pre-pass