// Mesh Vertex Shader
// Forward rendering vertex shader for drawable geometry (units, buildings, etc.)

struct CameraUniforms {
    view_proj: mat4x4<f32>,
};

struct ObjectUniforms {
    world: mat4x4<f32>,
    color_tint: vec4<f32>,
    opacity: f32,
    _pad0: u32,
    _pad1: u32,
    _pad2: u32,
};

struct VertexInput {
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) uv: vec2<f32>,
};

struct VertexOutput {
    @builtin(position) clip_pos: vec4<f32>,
    @location(0) world_pos: vec3<f32>,
    @location(1) world_normal: vec3<f32>,
    @location(2) uv: vec2<f32>,
    @location(3) color_tint: vec4<f32>,
    @location(4) opacity: f32,
};

@group(0) @binding(0)
var<uniform> camera: CameraUniforms;

@group(1) @binding(0)
var<uniform> object: ObjectUniforms;

@vertex
fn vs_main(input: VertexInput) -> VertexOutput {
    var output: VertexOutput;

    let world_pos = object.world * vec4<f32>(input.position, 1.0);
    output.clip_pos = camera.view_proj * world_pos;
    output.world_pos = world_pos.xyz;

    // Transform normal to world space (upper 3x3 of world matrix)
    let world_normal = normalize((object.world * vec4<f32>(input.normal, 0.0)).xyz);
    output.world_normal = world_normal;
    output.uv = input.uv;
    output.color_tint = object.color_tint;
    output.opacity = object.opacity;

    return output;
}
