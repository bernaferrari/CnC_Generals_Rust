// wgpu analog of W3DShaderManager filterPostRender:
// ScreenBWFilter, ScreenMotionBlurFilter (MAX_COUNT=60), ScreenCrossFadeFilter
// with fade-pattern / ST_MASK_TEXTURE.

struct FilterParams {
    fade: f32,
    kind: f32,          // 0 BW, 1 motion blur, 2 crossfade
    mode: f32,          // BW tint / saturate flag
    mb_count: f32,
    scroll: vec2<f32>,
    mask_radius: f32,
    additive: f32,
}

@group(0) @binding(0)
var scene_texture: texture_2d<f32>;
@group(0) @binding(1)
var scene_sampler: sampler;
@group(0) @binding(2)
var prev_texture: texture_2d<f32>;
@group(0) @binding(3)
var mask_texture: texture_2d<f32>;
@group(0) @binding(4)
var<uniform> params: FilterParams;

struct VsOut {
    @builtin(position) pos: vec4<f32>,
    @location(0) uv: vec2<f32>,
}

@vertex
fn vs_main(@builtin(vertex_index) idx: u32) -> VsOut {
    var out: VsOut;
    let x = f32(i32(idx & 1u) * 4 - 1);
    let y = f32(i32(idx >> 1u) * 4 - 1);
    out.pos = vec4<f32>(x, y, 0.0, 1.0);
    out.uv = vec2<f32>((x + 1.0) * 0.5, (1.0 - y) * 0.5);
    return out;
}

fn sample_scene(uv: vec2<f32>) -> vec4<f32> {
    return textureSample(scene_texture, scene_sampler, clamp(uv, vec2<f32>(0.0), vec2<f32>(1.0)));
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let fade = saturate(params.fade);
    let scene = sample_scene(in.uv);

    if (params.kind < 0.5) {
        // ScreenBWFilter — desaturate (optional red/green death-cam tint).
        let luma = dot(scene.rgb, vec3<f32>(0.30, 0.59, 0.11));
        var bw = vec3<f32>(luma);
        if (params.mode > 1.5) {
            bw = vec3<f32>(luma * 0.35, luma, luma * 0.35);
        } else if (params.mode > 0.5) {
            bw = vec3<f32>(luma, luma * 0.25, luma * 0.25);
        }
        return vec4<f32>(mix(scene.rgb, bw, fade), 1.0);
    }

    if (params.kind < 1.5) {
        // ScreenMotionBlurFilter: current RTT copied MAX_COUNT times, shrinking UVs.
        let center = vec2<f32>(0.5, 0.5) + params.scroll * vec2<f32>(0.5, -0.5);
        let max_count = 60.0;
        var count = clamp(params.mb_count, 1.0, max_count);
        var limit = min(count, 30.0);
        var uv = in.uv;
        if (params.mode < 0.5) {
            var factor = 1.0 - (count / max_count) * 0.90;
            factor = sqrt(max(factor, 0.01));
            uv = (uv - center) * factor + center;
        }
        var acc = sample_scene(uv);
        var i = 0;
        loop {
            if (i >= i32(limit)) {
                break;
            }
            var shrink = 0.99;
            var alpha = 0.082;
            if (params.additive > 0.5) {
                shrink = 0.98;
                alpha = 0.035;
            }
            uv = (uv - center) * shrink + center;
            let s = sample_scene(uv);
            acc = vec4<f32>(mix(acc.rgb, s.rgb, alpha), 1.0);
            i = i + 1;
        }
        return acc;
    }

    // ScreenCrossFadeFilter: previous scene + current, masked by fade-pattern.
    let prev = textureSample(prev_texture, scene_sampler, in.uv);
    var radius = (1.0 - fade) * 2.0;
    if (radius <= 0.0) {
        radius = 0.01;
    }
    radius = 0.5 / radius;
    let mask_uv = (in.uv - vec2<f32>(0.5)) * (radius * 2.0) + vec2<f32>(0.5);
    let mask = textureSample(mask_texture, scene_sampler, clamp(mask_uv, vec2<f32>(0.0), vec2<f32>(1.0)));
    let reveal = saturate(mask.r * mask.a);
    let mixed = mix(prev.rgb, scene.rgb, reveal);
    return vec4<f32>(mix(scene.rgb, mixed, fade), 1.0);
}
