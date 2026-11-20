// wgpu analog of C++ setupJbaWaterShader / m_riverWaterPixelShader
// and m_trapezoidWaterPixelShader (W3DWater.cpp).
//
// t0 river/standing albedo, t1 waterSparkles, t2 waterNoise (camera-space
// + riverVOrigin scroll), t3 riverAlphaEdge. Do not implement MD type-2 bump-sea.

struct Camera {
    view_proj: mat4x4<f32>,
    view: mat4x4<f32>,
    proj: mat4x4<f32>,
    position: vec3<f32>,
}

struct RiverParams {
    river_v_origin: f32,
    noise_repeat: f32,
    reflection: f32,
    is_trapezoid: f32,
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
var river_texture: texture_2d<f32>;
@group(1) @binding(1)
var river_sampler: sampler;
@group(1) @binding(2)
var sparkle_texture: texture_2d<f32>;
@group(1) @binding(3)
var noise_texture: texture_2d<f32>;
@group(1) @binding(4)
var alpha_edge_texture: texture_2d<f32>;
@group(1) @binding(5)
var<uniform> river: RiverParams;

@group(2) @binding(0)
var shroud_texture: texture_2d<f32>;
@group(2) @binding(1)
var shroud_sampler: sampler;
@group(2) @binding(2)
var<uniform> shroud: ShroudParams;

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

// C++ m_riverWaterPixelShader / m_trapezoidWaterPixelShader distilled:
// mul r0, v0, t0            -> base = vertex diffuse * river albedo
// mad r0.rgb, t1, t2, r0    -> sparkle * noise added to RGB
// river only: t3 (alpha edge) tints RGB and scales alpha.
// The V scroll lives in the baked vertex coordinate
// (-m_riverVOrigin + vScale*i + wobble, W3DWater.cpp:2834-2841), NOT here —
// the vertex phase is recomputed every frame from river_v_origin.
fn shade_water(in: VertexOutput) -> vec4<f32> {
    let t0 = textureSample(river_texture, river_sampler, in.tex_coords);
    let t1 = textureSample(sparkle_texture, river_sampler, in.tex_coords);
    // C++ stage 2: camera-space world XZ * NOISE_REPEAT_FACTOR + m_riverVOrigin
    // (setupJbaWaterShader texture transform, W3DWater.cpp:229-233).
    let noise_uv = in.world_position.xz * river.noise_repeat
        + vec2<f32>(river.river_v_origin, river.river_v_origin);
    let t2 = textureSample(noise_texture, river_sampler, noise_uv);

    var rgb = in.color * t0.rgb;
    var alpha = clamp(t0.a * in.alpha, 0.0, 1.0);

    if (river.is_trapezoid > 0.5) {
        // trapezoid: mad r0.rgb, t1, t2, r0
        rgb = rgb + t1.rgb * t2.rgb;
    } else {
        // river: add r0.rgb, r0, t3 ; mul r0.a, r0, t3 ; add r0.rgb, r0, t1*t2
        // W3DWater.cpp:201,2843,2861: stage 3 uses u2 across the full width.
        // The albedo uses u1 across half the width and v1 is identical to v2.
        let edge_uv = vec2<f32>(in.tex_coords.x * 2.0, in.tex_coords.y);
        let t3 = textureSample(alpha_edge_texture, river_sampler, edge_uv);
        rgb = rgb + t3.rgb;
        alpha = alpha * t3.a;
        rgb = rgb + t1.rgb * t2.rgb;
    }

    return vec4<f32>(rgb, alpha);
}

@fragment
fn fs_river(in: VertexOutput) -> @location(0) vec4<f32> {
    return shade_water(in);
}

@fragment
fn fs_trapezoid(in: VertexOutput) -> @location(0) vec4<f32> {
    let water = shade_water(in);
    // C++ setupFlatWaterShader applies stage 3 to the source RGB before blending.
    let mask = sample_projected_shroud(shroud_texture, shroud_sampler, shroud, in.world_position.xz);
    return vec4<f32>(water.rgb * mask, water.a);
}
