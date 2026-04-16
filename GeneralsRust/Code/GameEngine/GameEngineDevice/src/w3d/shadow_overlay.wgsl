// Shadow Overlay Shader
// C++ parity: W3DVolumetricShadow.cpp shadow mask fullscreen quad
// Darkens areas where stencil buffer != 0 (Carmack's reverse result)

struct ShadowUniforms {
    light_direction: vec4<f32>,
    shadow_color: vec4<f32>,
};

@group(0) @binding(0) var<uniform> uniforms: ShadowUniforms;

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
};

@vertex
fn vs_main(@builtin(vertex_index) vertex_index: u32) -> VertexOutput {
    // Fullscreen triangle strip (4 vertices)
    var pos = array<vec2<f32>, 4>(
        vec2(-1.0, -1.0),
        vec2(1.0, -1.0),
        vec2(-1.0, 1.0),
        vec2(1.0, 1.0),
    );
    var out: VertexOutput;
    out.position = vec4<f32>(pos[vertex_index], 0.999, 1.0);
    return out;
}

@fragment
fn fs_shadow_mask() -> @location(0) vec4<f32> {
    // Stencil test (NotEqual 0) is handled by the pipeline.
    // This fragment only runs where stencil != 0.
    // C++ shadow color: 0x7fa0a0a0
    return uniforms.shadow_color;
}
