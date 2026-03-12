struct Uniforms {
    view_proj : mat4x4<f32>,
};

@group(0) @binding(0)
var<uniform> uniforms : Uniforms;

@group(1) @binding(0)
var line_texture : texture_2d<f32>;

@group(1) @binding(1)
var line_sampler : sampler;

struct VSInput {
    @location(0) position : vec3<f32>,
    @location(1) color : vec4<f32>,
    @location(2) uv : vec2<f32>,
};

struct VSOutput {
    @builtin(position) position : vec4<f32>,
    @location(0) color : vec4<f32>,
    @location(1) uv : vec2<f32>,
};

@vertex
fn vs_main(input : VSInput) -> VSOutput {
    var output : VSOutput;
    output.position = uniforms.view_proj * vec4<f32>(input.position, 1.0);
    output.color = input.color;
    output.uv = input.uv;
    return output;
}

@fragment
fn fs_main(input : VSOutput) -> @location(0) vec4<f32> {
    let tex = textureSample(line_texture, line_sampler, input.uv);
    return tex * input.color;
}
