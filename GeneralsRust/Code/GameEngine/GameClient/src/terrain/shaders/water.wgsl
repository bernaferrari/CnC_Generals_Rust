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
    let tex = textureSample(water_texture, water_sampler, in.tex_coords);
    let tex_luma = dot(tex.rgb, vec3<f32>(0.2126, 0.7152, 0.0722));
    let teal = vec4<f32>(0.13, 0.42, 0.50, 0.70);
    let use_tex = tex_luma > 0.02 || tex.a > 0.02;
    let water = select(teal, tex, use_tex);
    let lit = max(in.color, vec3<f32>(0.35));
    // C++ drawSea reflection: mix sky-facing Fresnel into the standing plane.
    let view = normalize(camera.position - in.world_position);
    let n = vec3<f32>(0.0, 1.0, 0.0);
    let r = reflect(-view, n);
    let sky = mix(vec3<f32>(0.18, 0.38, 0.55), vec3<f32>(0.78, 0.86, 0.95), saturate(r.y * 0.5 + 0.5));
    let fresnel = pow(1.0 - saturate(dot(view, n)), 3.0);
    let bump = 0.04 * sin(in.tex_coords.x * 28.0 + in.tex_coords.y * 17.0);
    let rgb = mix(lit * water.rgb, sky, fresnel * 0.45 + bump);
    let alpha = clamp(max(water.a, 0.55) * max(in.alpha, 0.50), 0.0, 1.0);
    return vec4<f32>(rgb, alpha);
}
