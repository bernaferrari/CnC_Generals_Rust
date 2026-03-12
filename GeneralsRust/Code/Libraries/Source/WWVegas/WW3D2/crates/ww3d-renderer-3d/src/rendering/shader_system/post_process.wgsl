// Post-Processing Shaders
//
// Bloom, color grading, and FXAA matching C++ quality.

// Full-screen quad vertex shader
struct PostProcessVertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) uv: vec2<f32>,
}

@vertex
fn fullscreen_vertex_main(@builtin(vertex_index) vertex_index: u32) -> PostProcessVertexOutput {
    var out: PostProcessVertexOutput;

    // Generate full-screen triangle
    let x = f32((vertex_index << 1u) & 2u);
    let y = f32(vertex_index & 2u);

    out.clip_position = vec4<f32>(x * 2.0 - 1.0, 1.0 - y * 2.0, 0.0, 1.0);
    out.uv = vec2<f32>(x, y);

    return out;
}

// ============================================================================
// Bloom - Bright Pass
// ============================================================================

struct BloomSettings {
    threshold: f32,
    intensity: f32,
    _padding: vec2<f32>,
}

@group(0) @binding(0)
var input_texture: texture_2d<f32>;

@group(0) @binding(1)
var input_sampler: sampler;

@group(0) @binding(2)
var<uniform> bloom_settings: BloomSettings;

@fragment
fn bright_pass_main(in: PostProcessVertexOutput) -> @location(0) vec4<f32> {
    let color = textureSample(input_texture, input_sampler, in.uv).rgb;

    // Calculate luminance
    let luminance = dot(color, vec3<f32>(0.299, 0.587, 0.114));

    // Extract bright pixels above threshold
    if (luminance > bloom_settings.threshold) {
        return vec4<f32>(color, 1.0);
    } else {
        return vec4<f32>(0.0, 0.0, 0.0, 1.0);
    }
}

// ============================================================================
// Gaussian Blur (Separable)
// ============================================================================

struct BlurSettings {
    direction: vec2<f32>,  // (1,0) for horizontal, (0,1) for vertical
    texel_size: vec2<f32>,
}

@group(0) @binding(3)
var<uniform> blur_settings: BlurSettings;

// Gaussian weights for 5-tap blur (from C++)
const GAUSSIAN_WEIGHTS: array<f32, 5> = array<f32, 5>(
    0.0545, 0.2442, 0.4026, 0.2442, 0.0545
);

@fragment
fn gaussian_blur_main(in: PostProcessVertexOutput) -> @location(0) vec4<f32> {
    var color = vec4<f32>(0.0);

    // 5-tap Gaussian blur
    for (var i = -2; i <= 2; i++) {
        let offset = blur_settings.direction * f32(i) * blur_settings.texel_size;
        let sample_uv = in.uv + offset;
        color += textureSample(input_texture, input_sampler, sample_uv) * GAUSSIAN_WEIGHTS[i + 2];
    }

    return color;
}

// ============================================================================
// Bloom Combine
// ============================================================================

@group(1) @binding(0)
var bloom_texture: texture_2d<f32>;

@fragment
fn bloom_combine_main(in: PostProcessVertexOutput) -> @location(0) vec4<f32> {
    let original = textureSample(input_texture, input_sampler, in.uv).rgb;
    let bloom = textureSample(bloom_texture, input_sampler, in.uv).rgb;

    // Combine with intensity
    let final_color = original + bloom * bloom_settings.intensity;

    return vec4<f32>(final_color, 1.0);
}

// ============================================================================
// Color Grading
// ============================================================================

struct ColorGradingSettings {
    exposure: f32,
    gamma: f32,
    saturation: f32,
    brightness: f32,
    contrast: f32,
    _padding: vec3<f32>,
}

@group(0) @binding(4)
var<uniform> color_grading: ColorGradingSettings;

@fragment
fn color_grading_main(in: PostProcessVertexOutput) -> @location(0) vec4<f32> {
    var color = textureSample(input_texture, input_sampler, in.uv).rgb;

    // Exposure
    color = color * pow(2.0, color_grading.exposure);

    // Gamma correction
    color = pow(color, vec3<f32>(1.0 / color_grading.gamma));

    // Saturation
    let luminance = dot(color, vec3<f32>(0.299, 0.587, 0.114));
    color = mix(vec3<f32>(luminance), color, color_grading.saturation);

    // Contrast and brightness
    color = (color - 0.5) * color_grading.contrast + 0.5 + color_grading.brightness;

    // Clamp to valid range
    color = clamp(color, vec3<f32>(0.0), vec3<f32>(1.0));

    return vec4<f32>(color, 1.0);
}

// ============================================================================
// FXAA (Fast Approximate Anti-Aliasing)
// ============================================================================

struct FxaaSettings {
    edge_threshold: f32,
    edge_threshold_min: f32,
    subpixel_quality: f32,
    texel_size_x: f32,
    texel_size_y: f32,
    _padding: vec3<f32>,
}

@group(0) @binding(5)
var<uniform> fxaa_settings: FxaaSettings;

fn rgb_to_luma(color: vec3<f32>) -> f32 {
    return dot(color, vec3<f32>(0.299, 0.587, 0.114));
}

@fragment
fn fxaa_main(in: PostProcessVertexOutput) -> @location(0) vec4<f32> {
    let texel_size = vec2<f32>(fxaa_settings.texel_size_x, fxaa_settings.texel_size_y);

    // Sample center and neighbors
    let color_center = textureSample(input_texture, input_sampler, in.uv).rgb;
    let luma_center = rgb_to_luma(color_center);

    // Sample 4 neighbors
    let luma_n = rgb_to_luma(textureSample(input_texture, input_sampler, in.uv + vec2<f32>(0.0, -texel_size.y)).rgb);
    let luma_s = rgb_to_luma(textureSample(input_texture, input_sampler, in.uv + vec2<f32>(0.0, texel_size.y)).rgb);
    let luma_e = rgb_to_luma(textureSample(input_texture, input_sampler, in.uv + vec2<f32>(texel_size.x, 0.0)).rgb);
    let luma_w = rgb_to_luma(textureSample(input_texture, input_sampler, in.uv + vec2<f32>(-texel_size.x, 0.0)).rgb);

    // Find min and max luminance
    let luma_min = min(luma_center, min(min(luma_n, luma_s), min(luma_e, luma_w)));
    let luma_max = max(luma_center, max(max(luma_n, luma_s), max(luma_e, luma_w)));

    // Calculate edge contrast
    let luma_range = luma_max - luma_min;

    // Early exit if no edge detected
    if (luma_range < max(fxaa_settings.edge_threshold_min, luma_max * fxaa_settings.edge_threshold)) {
        return vec4<f32>(color_center, 1.0);
    }

    // Determine edge direction
    let edge_horz = abs((luma_n + luma_s) - 2.0 * luma_center);
    let edge_vert = abs((luma_e + luma_w) - 2.0 * luma_center);
    let is_horizontal = edge_horz >= edge_vert;

    // Sample perpendicular to edge
    var blend_factor = 0.5;
    if (is_horizontal) {
        // Horizontal edge, sample vertically
        let luma_nn = rgb_to_luma(textureSample(input_texture, input_sampler, in.uv + vec2<f32>(0.0, -2.0 * texel_size.y)).rgb);
        let luma_ss = rgb_to_luma(textureSample(input_texture, input_sampler, in.uv + vec2<f32>(0.0, 2.0 * texel_size.y)).rgb);
        blend_factor = abs(luma_n - luma_center) / luma_range * 0.5 +
                       abs(luma_s - luma_center) / luma_range * 0.5;
    } else {
        // Vertical edge, sample horizontally
        let luma_ee = rgb_to_luma(textureSample(input_texture, input_sampler, in.uv + vec2<f32>(2.0 * texel_size.x, 0.0)).rgb);
        let luma_ww = rgb_to_luma(textureSample(input_texture, input_sampler, in.uv + vec2<f32>(-2.0 * texel_size.x, 0.0)).rgb);
        blend_factor = abs(luma_e - luma_center) / luma_range * 0.5 +
                       abs(luma_w - luma_center) / luma_range * 0.5;
    }

    // Apply subpixel anti-aliasing
    blend_factor = clamp(blend_factor * fxaa_settings.subpixel_quality, 0.0, 1.0);

    // Sample blended color
    var offset = vec2<f32>(0.0);
    if (is_horizontal) {
        offset.y = (blend_factor * 2.0 - 1.0) * texel_size.y;
    } else {
        offset.x = (blend_factor * 2.0 - 1.0) * texel_size.x;
    }

    let color_blended = textureSample(input_texture, input_sampler, in.uv + offset).rgb;

    return vec4<f32>(color_blended, 1.0);
}

// ============================================================================
// Tone Mapping
// ============================================================================

@fragment
fn tone_map_reinhard_main(in: PostProcessVertexOutput) -> @location(0) vec4<f32> {
    let hdr_color = textureSample(input_texture, input_sampler, in.uv).rgb;

    // Reinhard tone mapping
    let ldr_color = hdr_color / (hdr_color + vec3<f32>(1.0));

    return vec4<f32>(ldr_color, 1.0);
}

@fragment
fn tone_map_exposure_main(in: PostProcessVertexOutput) -> @location(0) vec4<f32> {
    let hdr_color = textureSample(input_texture, input_sampler, in.uv).rgb;
    let exposure = color_grading.exposure;

    // Exposure tone mapping
    let exposed = hdr_color * pow(2.0, exposure);
    let ldr_color = vec3<f32>(1.0) - exp(-exposed);

    return vec4<f32>(ldr_color, 1.0);
}
