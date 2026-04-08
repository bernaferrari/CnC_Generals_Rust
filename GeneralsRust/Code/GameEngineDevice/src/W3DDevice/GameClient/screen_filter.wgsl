// Screen Filter Shader
// Corresponds to C++ W3DShaderManager.cpp filter pixel shaders
//
// Implements:
// - Black & White (luminance) filter
// - Motion Blur (additive offset sampling)
// - Crossfade (lerp between two textures)
// - Full-screen viewport quad

struct ScreenFilterUniforms {
    // BW filter: luminance weights (0.3, 0.59, 0.11, 1.0)
    // Crossfade: fade_level in x
    params: vec4<f32>,
    // BW filter: tint color (RGB)
    tint_color: vec4<f32>,
    // Motion blur: scroll delta (XY) and sample count/intensity (ZW)
    blur_params: vec4<f32>,
}

@group(0) @binding(0)
var<uniform> filter_uniforms: ScreenFilterUniforms;

@group(1) @binding(0)
var scene_texture: texture_2d<f32>;

@group(1) @binding(1)
var scene_sampler: sampler;

@group(2) @binding(0)
var scene_texture_2: texture_2d<f32>;

@group(2) @binding(1)
var scene_sampler_2: sampler;

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) uv: vec2<f32>,
}

// Full-screen quad vertex shader (triangle strip, 4 vertices)
// C++ drawViewport uses 4 vertices forming a triangle strip
@vertex
fn vs_main(@builtin(vertex_index) vertex_index: u32) -> VertexOutput {
    var out: VertexOutput;
    let x = f32((vertex_index & 1u) << 1u) - 1.0;
    let y = 1.0 - f32((vertex_index & 2u));
    out.clip_position = vec4<f32>(x, y, 0.0, 1.0);
    out.uv = vec2<f32>((x + 1.0) * 0.5, (1.0 - y) * 0.5);
    return out;
}

// Black & White filter
// C++ ScreenBWFilter::set() uses pixel shader constants:
//   c0 = (0.3, 0.59, 0.11, 1.0) - luminance weights
//   c1 = tint_color (e.g., 1,1,1 for B&W, 1,0,0 for red & white)
//   c2 = fade_value
@fragment
fn fs_bw(input: VertexOutput) -> @location(0) vec4<f32> {
    let color = textureSample(scene_texture, scene_sampler, input.uv);
    let luminance = dot(color.rgb, filter_uniforms.params.xyz);
    let bw_color = vec3<f32>(luminance) * filter_uniforms.tint_color.xyz;
    let fade = filter_uniforms.params.w;
    let result = mix(color.rgb, bw_color, fade);
    return vec4<f32>(result, color.a);
}

// Motion Blur filter
// C++ ScreenMotionBlurFilter samples the scene texture multiple times
// at slight UV offsets and additive-blends them together
@fragment
fn fs_motion_blur(input: VertexOutput) -> @location(0) vec4<f32> {
    let delta = filter_uniforms.blur_params.xy;
    let intensity = filter_uniforms.blur_params.z;
    let sample_count = 5u;
    var accum = vec4<f32>(0.0);
    for (var i = 0u; i < sample_count; i++) {
        let offset = delta * (f32(i) / f32(sample_count - 1u) - 0.5);
        let sample_color = textureSample(scene_texture, scene_sampler, input.uv + offset);
        accum = accum + sample_color;
    }
    accum = accum / f32(sample_count);
    return vec4<f32>(accum.rgb * intensity, accum.a);
}

// Crossfade filter
// C++ ScreenCrossFadeFilter lerps between two textures based on fade level
@fragment
fn fs_crossfade(input: VertexOutput) -> @location(0) vec4<f32> {
    let color_a = textureSample(scene_texture, scene_sampler, input.uv);
    let color_b = textureSample(scene_texture_2, scene_sampler_2, input.uv);
    let fade = filter_uniforms.params.x;
    return mix(color_a, color_b, fade);
}

// Simple viewport blit (identity pass-through for drawViewport)
@fragment
fn fs_viewport(input: VertexOutput) -> @location(0) vec4<f32> {
    return textureSample(scene_texture, scene_sampler, input.uv);
}
