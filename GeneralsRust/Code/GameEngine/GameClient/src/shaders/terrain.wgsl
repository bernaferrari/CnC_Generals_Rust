// Terrain Shader for Command & Conquer Generals
// Handles terrain rendering with multi-texturing support

struct Camera {
    view_proj: mat4x4<f32>,
    view: mat4x4<f32>,
    proj: mat4x4<f32>,
    position: vec4<f32>,
    time: f32,
    _time_padding_0: f32,
    _time_padding_1: f32,
    _time_padding_2: f32,
    fog_color: vec4<f32>,
    fog_range: vec4<f32>,
}
struct TerrainVertex {
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) tex_coords: vec2<f32>,
    @location(3) blend_indices: vec4<u32>,
    @location(4) blend_weights: vec4<f32>,
    @location(5) color: vec4<f32>,
}

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) world_position: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) tex_coords: vec2<f32>,
    @location(3) blend_weights: vec4<f32>,
    @location(4) color: vec4<f32>,
    @location(5) fog_factor: f32,
    // Bound texture-class slot of each blend weight; the four corners of a
    // cell carry the same slots (sorted by slot), so flat is exact.
    @location(6) @interpolate(flat) blend_indices: vec4<u32>,
}

@group(0) @binding(0)
var<uniform> camera: Camera;

@group(1) @binding(0)
var terrain_texture_0: texture_2d<f32>;
@group(1) @binding(1)
var terrain_texture_1: texture_2d<f32>;
@group(1) @binding(2)
var terrain_texture_2: texture_2d<f32>;
@group(1) @binding(3)
var terrain_texture_3: texture_2d<f32>;
@group(1) @binding(4)
var terrain_texture_4: texture_2d<f32>;
@group(1) @binding(5)
var terrain_texture_5: texture_2d<f32>;
@group(1) @binding(6)
var terrain_texture_6: texture_2d<f32>;
@group(1) @binding(7)
var terrain_texture_7: texture_2d<f32>;
@group(1) @binding(8)
var terrain_texture_8: texture_2d<f32>;
@group(1) @binding(9)
var terrain_texture_9: texture_2d<f32>;
@group(1) @binding(10)
var terrain_texture_10: texture_2d<f32>;
@group(1) @binding(11)
var terrain_texture_11: texture_2d<f32>;
@group(1) @binding(12)
var terrain_texture_12: texture_2d<f32>;
@group(1) @binding(13)
var terrain_texture_13: texture_2d<f32>;
@group(1) @binding(14)
var terrain_texture_14: texture_2d<f32>;
@group(1) @binding(15)
var terrain_texture_15: texture_2d<f32>;
@group(1) @binding(16)
var terrain_sampler: sampler;

@vertex
fn vs_main(vertex: TerrainVertex) -> VertexOutput {
    var out: VertexOutput;
    
    let world_position = vertex.position;
    out.world_position = world_position;
    out.clip_position = camera.view_proj * vec4<f32>(world_position, 1.0);
    out.normal = vertex.normal;
    out.tex_coords = vertex.tex_coords;
    out.blend_weights = vertex.blend_weights;
    out.blend_indices = vertex.blend_indices;
    // C++ HeightMap VB `diffuse` already includes doTheDynamicLight.
    out.color = vertex.color;
    // DX8Wrapper sets FOGVERTEXMODE=LINEAR and enables range fog when supported.
    // Compute on vertices so raster interpolation follows that original phase.
    out.fog_factor = 0.0;
    let fog_span = camera.fog_range.y - camera.fog_range.x;
    if (fog_span > 0.0) {
        let fog_distance = distance(camera.position.xyz, world_position);
        out.fog_factor = clamp((fog_distance - camera.fog_range.x) / fog_span, 0.0, 1.0);
    }
    
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let weight_sum = in.blend_weights.x + in.blend_weights.y + in.blend_weights.z + in.blend_weights.w;
    let blend = select(in.blend_weights, vec4<f32>(1.0, 0.0, 0.0, 0.0), weight_sum <= 0.0);
    // Explicit gradients keep sampling valid inside the slot switch
    // (non-uniform control flow).
    let ddx = dpdx(in.tex_coords);
    let ddy = dpdy(in.tex_coords);
    var final_color = vec4<f32>(0.0);
    for (var i = 0u; i < 4u; i = i + 1u) {
        let w = blend[i];
        if (w > 0.0) {
            final_color = final_color + sample_slot(in.blend_indices[i], in.tex_coords, ddx, ddy) * w;
        }
    }

    // C++ HeightMap.cpp:1889/1980-2001 — doCloud = UseCloudMap && !NIGHT,
    // noise/lightmap = UseLightMap. Packed in camera.position.w as bit0/bit1.
    let mode = i32(camera.position.w + 0.5);
    if ((mode & 1) != 0) {
        let cloud_uv = in.world_position.xz * 0.002 + vec2<f32>(camera.time * -0.02, camera.time * -0.03);
        let cloud = 0.70 + 0.30 * hash2(cloud_uv);
        final_color = vec4<f32>(final_color.rgb * cloud, final_color.a);
    }
    if ((mode & 2) != 0) {
        let noise_uv = in.world_position.xz * 0.015;
        let noise = 0.85 + 0.15 * hash2(noise_uv + vec2<f32>(17.0, 9.0));
        final_color = vec4<f32>(final_color.rgb * noise, final_color.a);
    }

    // Modulate by baked vb.diffuse (doTheDynamicLight). Do not fake-N·L again.
    var lit_color = vec4<f32>(final_color.rgb * in.color.rgb, final_color.a * in.color.a);
    // SceneClass defaults FogEnabled=false. The receiving visual admits a
    // nonzero range explicitly; a color alone never enables terrain fog.
    // DX8 linear range fog: original color weight=(end-distance)/(end-start).
    let fog_span = camera.fog_range.y - camera.fog_range.x;
    if (fog_span > 0.0) {
        let fog_factor = in.fog_factor;
        lit_color = vec4<f32>(mix(lit_color.rgb, camera.fog_color.rgb, fog_factor), lit_color.a);
    }
    return lit_color;
}

fn hash2(p: vec2<f32>) -> f32 {
    return fract(sin(dot(p, vec2<f32>(127.1, 311.7))) * 43758.5453);
}

fn sample_slot(slot: u32, uv: vec2<f32>, ddx: vec2<f32>, ddy: vec2<f32>) -> vec4<f32> {
    switch slot {
        case 1u: { return textureSampleGrad(terrain_texture_1, terrain_sampler, uv, ddx, ddy); }
        case 2u: { return textureSampleGrad(terrain_texture_2, terrain_sampler, uv, ddx, ddy); }
        case 3u: { return textureSampleGrad(terrain_texture_3, terrain_sampler, uv, ddx, ddy); }
        case 4u: { return textureSampleGrad(terrain_texture_4, terrain_sampler, uv, ddx, ddy); }
        case 5u: { return textureSampleGrad(terrain_texture_5, terrain_sampler, uv, ddx, ddy); }
        case 6u: { return textureSampleGrad(terrain_texture_6, terrain_sampler, uv, ddx, ddy); }
        case 7u: { return textureSampleGrad(terrain_texture_7, terrain_sampler, uv, ddx, ddy); }
        case 8u: { return textureSampleGrad(terrain_texture_8, terrain_sampler, uv, ddx, ddy); }
        case 9u: { return textureSampleGrad(terrain_texture_9, terrain_sampler, uv, ddx, ddy); }
        case 10u: { return textureSampleGrad(terrain_texture_10, terrain_sampler, uv, ddx, ddy); }
        case 11u: { return textureSampleGrad(terrain_texture_11, terrain_sampler, uv, ddx, ddy); }
        case 12u: { return textureSampleGrad(terrain_texture_12, terrain_sampler, uv, ddx, ddy); }
        case 13u: { return textureSampleGrad(terrain_texture_13, terrain_sampler, uv, ddx, ddy); }
        case 14u: { return textureSampleGrad(terrain_texture_14, terrain_sampler, uv, ddx, ddy); }
        case 15u: { return textureSampleGrad(terrain_texture_15, terrain_sampler, uv, ddx, ddy); }
        default: { return textureSampleGrad(terrain_texture_0, terrain_sampler, uv, ddx, ddy); }
    }
}
