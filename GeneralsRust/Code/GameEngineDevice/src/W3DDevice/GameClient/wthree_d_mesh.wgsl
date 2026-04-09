struct MeshUniforms {
    view_proj: mat4x4<f32>,
    world: mat4x4<f32>,
};

@group(0) @binding(0)
var<uniform> uniforms: MeshUniforms;

struct VertexInput {
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) uv: vec2<f32>,
    @location(3) color: vec4<f32>,
};

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) color: vec4<f32>,
};

@vertex
fn vs_main(input: VertexInput) -> VertexOutput {
    var output: VertexOutput;
    let world_position = uniforms.world * vec4<f32>(input.position, 1.0);
    output.clip_position = uniforms.view_proj * world_position;
    output.color = input.color;
    return output;
}

@fragment
fn fs_main(input: VertexOutput) -> @location(0) vec4<f32> {
    return input.color;
}
