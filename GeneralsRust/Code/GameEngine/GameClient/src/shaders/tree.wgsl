// W3DTreeBuffer detailAlphaShader: tree atlas then projected terrain shroud.
struct Camera {
    view_proj: mat4x4<f32>,
    view: mat4x4<f32>,
    proj: mat4x4<f32>,
    position: vec3<f32>,
}

struct TreeVertex {
    @location(0) position: vec3<f32>,
    @location(1) color: vec3<f32>,
    @location(2) tex_coords: vec2<f32>,
    @location(4) sway_index: f32,
    @location(5) darkening: f32,
    @location(6) base_z: f32,
}

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) color: vec3<f32>,
    @location(1) tex_coords: vec2<f32>,
    @location(2) world_xz: vec2<f32>,
}

@group(0) @binding(0)
var<uniform> camera: Camera;

@group(1) @binding(0)
var tree_atlas: texture_2d<f32>;
@group(1) @binding(1)
var tree_sampler: sampler;
@group(2) @binding(0)
var shroud_texture: texture_2d<f32>;
@group(2) @binding(1)
var shroud_sampler: sampler;
@group(2) @binding(2)
var<uniform> shroud: ShroudParams;

struct TreeSway {
    factors: array<vec4<f32>, 10>,
}

@group(3) @binding(0)
var<uniform> tree_sway: TreeSway;

@vertex
fn vs_main(vertex: TreeVertex) -> VertexOutput {
    var out: VertexOutput;
    let sway_index = u32(vertex.sway_index);
    var wave = vec3<f32>(0.0);
    if (sway_index > 0u && sway_index <= 10u) {
        let native_wave = tree_sway.factors[sway_index - 1u].xyz;
        wave = vec3<f32>(native_wave.x, native_wave.z, native_wave.y);
    }
    let height_above_base = vertex.position.y - vertex.base_z;
    let position = vertex.position + height_above_base * wave;
    out.clip_position = camera.view_proj * vec4<f32>(position, 1.0);
    out.color = vertex.color * vertex.darkening;
    out.tex_coords = vertex.tex_coords;
    // Original Trees.vso uses the unswayed v0 for its stage-1 projection.
    out.world_xz = vertex.position.xz;
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let tex = textureSample(tree_atlas, tree_sampler, in.tex_coords);
    let fog_light = sample_projected_shroud(shroud_texture, shroud_sampler, shroud, in.world_xz);
    // WW3D ShaderClass uses D3DCMP_GREATEREQUAL with reference 0x60.
    if (tex.a < 96.0 / 255.0) {
        discard;
    }
    return vec4<f32>(tex.rgb * in.color * fog_light, tex.a);
}
