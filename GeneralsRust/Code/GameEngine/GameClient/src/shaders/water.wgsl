// Live water overlay. Group 0 is the same camera uniform as terrain
// (view_proj first). Group 1 is a single standing-water albedo if the
// pipeline created it; the host binds TWWater01 or a teal 1x1 fallback.
// Do not declare bindings 1-7 on group 0 — those are never created.

struct Camera {
    view_proj: mat4x4<f32>,
    view: mat4x4<f32>,
    proj: mat4x4<f32>,
    position: vec3<f32>,
}

struct WaterVertex {
    @location(0) position: vec3<f32>,
    @location(1) color: vec3<f32>,
    @location(2) tex_coords: vec2<f32>,
    @location(3) alpha: f32,
    @location(4) packed_c: u32,
}

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) world_position: vec3<f32>,
    @location(1) color: vec3<f32>,
    @location(2) tex_coords: vec2<f32>,
    @location(3) alpha: f32,
}

@group(0) @binding(0)
var<uniform> camera: Camera;

@group(1) @binding(0)
var water_texture: texture_2d<f32>;
@group(1) @binding(1)
var water_sampler: sampler;

@vertex
fn vs_main(vertex: WaterVertex) -> VertexOutput {
    var out: VertexOutput;
    out.world_position = vertex.position;
    out.clip_position = camera.view_proj * vec4<f32>(vertex.position, 1.0);
    out.color = vertex.color;
    out.tex_coords = vertex.tex_coords;
    out.alpha = vertex.alpha;
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    // C++ HeightMap sea-patch bake puts POINT/global lighting into
    // SeaPatchVertex.c; the FS must honour that packed colour rather than
    // inventing a shallow/deep palette.
    let _tex = textureSample(water_texture, water_sampler, in.tex_coords);
    return vec4<f32>(in.color, in.alpha);
}
