// UI Shader for Command & Conquer Generals Zero Hour Rust

struct VertexInput {
    @location(0) position: vec3<f32>,
    @location(1) tex_coords: vec2<f32>,
    @location(2) color: vec4<f32>,
};

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) tex_coords: vec2<f32>,
    @location(1) color: vec4<f32>,
};

struct Uniforms {
    transform: mat4x4<f32>,
    view_proj: mat4x4<f32>,
};

@group(0) @binding(0)
var<uniform> uniforms: Uniforms;

@vertex
fn vs_main(vertex: VertexInput) -> VertexOutput {
    var out: VertexOutput;
    
    let world_position = uniforms.transform * vec4<f32>(vertex.position, 1.0);
    out.clip_position = uniforms.view_proj * world_position;
    out.tex_coords = vertex.tex_coords;
    out.color = vertex.color;
    
    return out;
}

@group(1) @binding(0)
var ui_texture: texture_2d<f32>;
@group(1) @binding(1)
var ui_sampler: sampler;

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let tex_color = textureSample(ui_texture, ui_sampler, in.tex_coords);
    return tex_color * in.color;
}