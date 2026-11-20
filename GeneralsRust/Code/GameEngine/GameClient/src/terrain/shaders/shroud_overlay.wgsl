// C++ ST_SHROUD_TEXTURE / setShroudTex analog.
// Extra multiplicative pass: dest * projected dest-texture (world XZ).
// Water uses ZFUNC LESSEQUAL because the water pass does not write Z.

struct Camera {
    view_proj: mat4x4<f32>,
    view: mat4x4<f32>,
    proj: mat4x4<f32>,
    position: vec3<f32>,
}

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) world_position: vec3<f32>,
}

@group(0) @binding(0)
var<uniform> camera: Camera;

@group(1) @binding(0)
var shroud_texture: texture_2d<f32>;
@group(1) @binding(1)
var shroud_sampler: sampler;
@group(1) @binding(2)
var<uniform> shroud: ShroudParams;

@vertex
fn vs_main(@location(0) position: vec3<f32>) -> VertexOutput {
    var out: VertexOutput;
    out.world_position = position;
    out.clip_position = camera.view_proj * vec4<f32>(position, 1.0);
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    return vec4<f32>(sample_projected_shroud(shroud_texture, shroud_sampler, shroud, in.world_position.xz), 1.0);
}
