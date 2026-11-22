@group(0) @binding(0) var<uniform> view_proj: mat4x4<f32>;
@group(0) @binding(1) var ribbon_texture: texture_2d<f32>;
@group(0) @binding(2) var ribbon_sampler: sampler;

struct RibbonOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) color: vec3<f32>,
};

@vertex
fn vertex_main(
    @location(0) point: vec3<f32>,
    @location(1) uv: vec2<f32>,
    @location(2) color: vec3<f32>,
) -> RibbonOutput {
    return RibbonOutput(view_proj * vec4<f32>(point, 1.0), uv, color);
}

@fragment
fn fragment_main(input: RibbonOutput) -> @location(0) vec4<f32> {
    let source = textureSample(ribbon_texture, ribbon_sampler, input.uv);
    return vec4<f32>(input.color * source.rgb, source.a);
}
