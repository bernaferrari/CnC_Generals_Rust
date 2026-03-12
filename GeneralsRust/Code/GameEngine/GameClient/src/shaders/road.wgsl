// Startup-safe road shader.
// The active road runtime currently renders through RoadSystem, not this fallback pipeline.
// Keep this shader aligned with the live camera-only bind surface so terrain startup does not
// fail on an unused pipeline that still expects a richer texture/material setup.

struct Camera {
    view_proj: mat4x4<f32>,
    view: mat4x4<f32>,
    proj: mat4x4<f32>,
    position: vec3<f32>,
}

struct RoadVertex {
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) tex_coords: vec2<f32>,
    @location(3) road_width: f32,
}

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) normal: vec3<f32>,
    @location(1) tex_coords: vec2<f32>,
    @location(2) road_width: f32,
}

@group(0) @binding(0)
var<uniform> camera: Camera;

@vertex
fn vs_main(vertex: RoadVertex) -> VertexOutput {
    var out: VertexOutput;
    out.clip_position = camera.view_proj * vec4<f32>(vertex.position, 1.0);
    out.normal = normalize(vertex.normal);
    out.tex_coords = vertex.tex_coords;
    out.road_width = vertex.road_width;
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let light_dir = normalize(vec3<f32>(0.3, 0.8, 0.25));
    let diffuse = max(dot(in.normal, light_dir), 0.35);
    let stripe = 0.04 * sin(in.tex_coords.y * 24.0);
    let base_color = vec3<f32>(0.40, 0.34, 0.27) + vec3<f32>(stripe);
    let alpha = clamp(in.road_width, 0.0, 1.0);
    return vec4<f32>(base_color * diffuse, alpha);
}
