struct Uniforms {
    view_matrix: mat4x4<f32>,
    projection_matrix: mat4x4<f32>,
    camera_position: vec3<f32>,
    time: f32,
    screen_size: vec2<f32>,
    particle_count: u32,
    _padding: u32,
};

@group(0) @binding(0)
var<uniform> uniforms: Uniforms;

@group(1) @binding(0)
var scene_texture: texture_2d<f32>;

@group(1) @binding(1)
var scene_sampler: sampler;

struct VsIn {
    @location(0) view_pos: vec3<f32>,
    @location(1) uv: vec2<f32>,
    @location(2) opacity: f32,
};

struct VsOut {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) opacity: f32,
};

@vertex
fn vs_main(input: VsIn) -> VsOut {
    var out: VsOut;
    out.position = uniforms.projection_matrix * vec4<f32>(input.view_pos, 1.0);
    out.uv = input.uv;
    out.opacity = input.opacity;
    return out;
}

@fragment
fn fs_main(input: VsOut) -> @location(0) vec4<f32> {
    let color = textureSample(scene_texture, scene_sampler, input.uv);
    return vec4<f32>(color.rgb, input.opacity);
}
