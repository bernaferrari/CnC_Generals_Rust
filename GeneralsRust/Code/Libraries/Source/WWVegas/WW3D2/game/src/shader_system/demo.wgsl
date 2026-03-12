//! Demo WGSL shader for WW3D Engine
//!
//! This is a simple shader that renders a colored triangle
//! to demonstrate that the WGPU pipeline is working correctly.

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) color: vec3<f32>,
}

@vertex
fn vs_main(@builtin(vertex_index) in_vertex_index: u32) -> VertexOutput {
    var out: VertexOutput;

    // Use vertex index to determine position and color
    var pos: vec2<f32>;
    var color: vec3<f32>;

    if (in_vertex_index == 0u) {
        pos = vec2<f32>(0.0, 0.5);
        color = vec3<f32>(1.0, 0.0, 0.0); // Red
    } else if (in_vertex_index == 1u) {
        pos = vec2<f32>(-0.5, -0.5);
        color = vec3<f32>(0.0, 1.0, 0.0); // Green
    } else {
        pos = vec2<f32>(0.5, -0.5);
        color = vec3<f32>(0.0, 0.0, 1.0); // Blue
    }

    out.clip_position = vec4<f32>(pos, 0.0, 1.0);
    out.color = color;

    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    return vec4<f32>(in.color, 1.0);
}