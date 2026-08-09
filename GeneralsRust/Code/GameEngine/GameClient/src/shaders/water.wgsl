// Water Shader for Command & Conquer Generals
// Handles water rendering with reflection and transparency

struct Camera {
    view_proj: mat4x4<f32>,
    view: mat4x4<f32>,
    proj: mat4x4<f32>,
    position: vec3<f32>,
}

struct WaterVertex {
    @location(0) position: vec3<f32>,
    @location(1) color: vec3<f32>,
    @location(2) tex_coords: vec2<f32>,
    @location(3) alpha: f32,
    @location(4) packed_c: u32,
}

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) world_position: vec3<f32>,
    @location(1) color: vec3<f32>,
    @location(2) tex_coords: vec2<f32>,
    @location(3) alpha: f32,
}

@group(0) @binding(0)
var<uniform> camera: Camera;

@vertex
fn vs_main(vertex: WaterVertex) -> VertexOutput {
    var out: VertexOutput;
    
    let world_position = vertex.position;
    out.world_position = world_position;
    out.clip_position = camera.view_proj * vec4<f32>(world_position, 1.0);
    // C++ SEA_PATCH_VERTEX.c already includes standing-water + HeightMap POINT lights.
    out.color = vertex.color;
    out.tex_coords = vertex.tex_coords;
    out.alpha = vertex.alpha;
    
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    return vec4<f32>(in.color, in.alpha);
}
