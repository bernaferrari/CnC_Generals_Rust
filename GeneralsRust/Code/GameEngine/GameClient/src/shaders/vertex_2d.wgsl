// 2D Vertex Shader for Command & Conquer Generals Zero Hour
// Modern WGSL shader for 2D rendering operations

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) tex_coords: vec2<f32>,
    @location(1) color: vec4<f32>,
}

struct Uniforms {
    projection: mat4x4<f32>,
    view: mat4x4<f32>,
}

@group(0) @binding(0)
var<uniform> uniforms: Uniforms;

@vertex
fn vs_main(
    @location(0) position: vec2<f32>,
    @location(1) tex_coords: vec2<f32>,
    @location(2) color: vec4<f32>,
) -> VertexOutput {
    var out: VertexOutput;
    
    // Transform 2D position to clip space
    let world_pos = vec4<f32>(position, 0.0, 1.0);
    out.clip_position = uniforms.projection * uniforms.view * world_pos;
    
    out.tex_coords = tex_coords;
    out.color = color;
    
    return out;
}