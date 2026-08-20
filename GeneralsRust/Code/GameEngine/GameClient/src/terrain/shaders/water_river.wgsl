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
    let river_uv = in.tex_coords + vec2<f32>(0.0, river.river_v_origin);
    let t0 = textureSample(river_texture, river_sampler, river_uv);
    let t1 = textureSample(sparkle_texture, river_sampler, in.tex_coords);
    // C++ stage 2: camera-space world XZ * NOISE_REPEAT_FACTOR + m_riverVOrigin.
    let noise_uv = in.world_position.xz * river.noise_repeat
        + vec2<f32>(river.river_v_origin, river.river_v_origin);
    let t2 = textureSample(noise_texture, river_sampler, noise_uv);
    let t3 = textureSample(alpha_edge_texture, river_sampler, in.tex_coords);

    let lit = max(in.color, vec3<f32>(0.28));
    // mul r0, v0, t0
    var rgb = lit * t0.rgb;
    var alpha = clamp(max(t0.a, 0.45) * max(in.alpha, 0.45), 0.0, 1.0);

    if (river.is_trapezoid > 0.5) {
        // trapezoid: mad r0.rgb, t1, t2, r0
        rgb = rgb + t1.rgb * t2.rgb;
    } else {
        // river: add r0.rgb, r0, t3 ; mul r0.a, r0, t3 ; add r0.rgb, r0, t1*t2
        rgb = rgb + t3.rgb;
        alpha = alpha * saturate(max(t3.a, t3.r));
        rgb = rgb + t1.rgb * t2.rgb;
    }

    let view = normalize(camera.position - in.world_position);
    let n = vec3<f32>(0.0, 1.0, 0.0);
    let fresnel = pow(1.0 - saturate(dot(view, n)), 3.0);
    rgb = rgb + vec3<f32>(river.reflection) * fresnel;
    return vec4<f32>(rgb, alpha);
}
