// Road overlay shader (W3DRoadBuffer.cpp RoadType::applyTexture + GRADIENT_MODULATE).
// Live terrain path records these draws through `record_road_draws`.
// Bind group 0 = camera. Bind group 1 = road albedo + repeat sampler.


struct Camera {
    view_proj: mat4x4<f32>,
    view: mat4x4<f32>,
    proj: mat4x4<f32>,
    position: vec3<f32>,
}

struct RoadVertex {
    @location(0) position: vec3<f32>,
    @location(1) color: vec3<f32>,
    @location(2) tex_coords: vec2<f32>,
    @location(3) road_width: f32,
    @location(4) packed_diffuse: u32,
}

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) color: vec3<f32>,
    @location(1) tex_coords: vec2<f32>,
    @location(2) road_width: f32,
}

@group(0) @binding(0)
var<uniform> camera: Camera;

@group(1) @binding(0)
var road_texture: texture_2d<f32>;
@group(1) @binding(1)
var road_sampler: sampler;

@vertex
fn vs_main(vertex: RoadVertex) -> VertexOutput {
    var out: VertexOutput;
    out.clip_position = camera.view_proj * vec4<f32>(vertex.position, 1.0);
    out.color = vertex.color;
    out.tex_coords = vertex.tex_coords;
    out.road_width = vertex.road_width;
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    // C++ SC_ALPHA_DETAIL GRADIENT_MODULATE: vertex lighting * texture.
    // Overlay VB already stores W3DRoadBuffer do_road_dynamic_light in color;
    // road_width remains the edge/coverage alpha (0..1).
    // Unlit bake (diffuse=0) or a missing/black albedo would otherwise draw
    // pitch-black strips. Floor lighting and substitute gravel if the sample
    // has no chroma/luma (INI miss / 2x2 black).
    let tex = textureSample(road_texture, road_sampler, in.tex_coords);
    let tex_luma = dot(tex.rgb, vec3<f32>(0.2126, 0.7152, 0.0722));
    let gravel = vec3<f32>(0.47, 0.43, 0.37);
    let albedo = select(tex.rgb, gravel, tex_luma < 0.04);
    let lit = max(in.color, vec3<f32>(0.35));
    let alpha = clamp(in.road_width, 0.0, 1.0);
    return vec4<f32>(lit * albedo, alpha);
}
