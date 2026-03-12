// W3D Temporal Anti-Aliasing (TAA) Shader
// High-quality temporal anti-aliasing with motion vectors and history reprojection

struct CameraData {
    view_matrix: mat4x4<f32>,
    projection_matrix: mat4x4<f32>,
    view_projection_matrix: mat4x4<f32>,
    prev_view_projection_matrix: mat4x4<f32>,
    inverse_view_matrix: mat4x4<f32>,
    inverse_projection_matrix: mat4x4<f32>,
    camera_position: vec3<f32>,
    camera_direction: vec3<f32>,
    near_plane: f32,
    far_plane: f32,
    fov: f32,
    aspect_ratio: f32,
};

struct TAAUniforms {
    jitter_offset: vec2<f32>,      // Current frame jitter
    prev_jitter_offset: vec2<f32>, // Previous frame jitter
    feedback_min: f32,              // Minimum history blend (0.05 typical)
    feedback_max: f32,              // Maximum history blend (0.95 typical)
    sharpness: f32,                 // Sharpening amount (0.0 - 1.0)
    frame_index: u32,               // For jitter sequence
};

@group(0) @binding(0)
var<uniform> camera: CameraData;

@group(1) @binding(0)
var t_current: texture_2d<f32>;   // Current frame HDR
@group(1) @binding(1)
var t_history: texture_2d<f32>;   // Previous frame accumulation
@group(1) @binding(2)
var t_motion: texture_2d<f32>;    // Motion vectors (RG channels)
@group(1) @binding(3)
var t_depth: texture_2d<f32>;     // Depth buffer
@group(1) @binding(4)
var s_linear: sampler;            // Linear sampler

@group(2) @binding(0)
var<uniform> taa_params: TAAUniforms;

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

// Fullscreen triangle vertex shader
@vertex
fn vs_main(@builtin(vertex_index) vertex_index: u32) -> VertexOutput {
    var out: VertexOutput;

    // Generate fullscreen triangle
    let x = f32((vertex_index & 1u) << 2u) - 1.0;
    let y = f32((vertex_index & 2u) << 1u) - 1.0;

    out.position = vec4<f32>(x, y, 0.0, 1.0);
    out.uv = vec2<f32>((x + 1.0) * 0.5, (1.0 - y) * 0.5);

    return out;
}

// Halton sequence for jitter pattern (2, 3 bases)
fn halton(index: u32, base: u32) -> f32 {
    var f = 1.0;
    var r = 0.0;
    var i = index;

    while (i > 0u) {
        f = f / f32(base);
        r = r + f * f32(i % base);
        i = i / base;
    }

    return r;
}

// Generate jitter offset using Halton sequence
fn get_jitter_offset(frame_index: u32) -> vec2<f32> {
    let index = (frame_index % 16u) + 1u; // 16-sample pattern
    return vec2<f32>(
        halton(index, 2u) - 0.5,
        halton(index, 3u) - 0.5
    );
}

// RGB to YCoCg color space conversion
fn rgb_to_ycocg(color: vec3<f32>) -> vec3<f32> {
    let y = dot(color, vec3<f32>(0.25, 0.5, 0.25));
    let co = dot(color, vec3<f32>(0.5, 0.0, -0.5));
    let cg = dot(color, vec3<f32>(-0.25, 0.5, -0.25));
    return vec3<f32>(y, co, cg);
}

// YCoCg to RGB color space conversion
fn ycocg_to_rgb(color: vec3<f32>) -> vec3<f32> {
    let y = color.x;
    let co = color.y;
    let cg = color.z;

    let r = y + co - cg;
    let g = y + cg;
    let b = y - co - cg;

    return vec3<f32>(r, g, b);
}

// Catmull-Rom filtering for history sampling
fn sample_catmull_rom(tex: texture_2d<f32>, samp: sampler, uv: vec2<f32>) -> vec4<f32> {
    let tex_size = vec2<f32>(textureDimensions(tex, 0));
    let sample_pos = uv * tex_size;
    let tex_pos1 = floor(sample_pos - 0.5) + 0.5;
    let f = sample_pos - tex_pos1;

    // Catmull-Rom weights
    let w0 = f * (-0.5 + f * (1.0 - 0.5 * f));
    let w1 = 1.0 + f * f * (-2.5 + 1.5 * f);
    let w2 = f * (0.5 + f * (2.0 - 1.5 * f));
    let w3 = f * f * (-0.5 + 0.5 * f);

    let w12 = w1 + w2;
    let offset12 = w2 / (w1 + w2);

    let tex_pos0 = tex_pos1 - 1.0;
    let tex_pos3 = tex_pos1 + 2.0;
    let tex_pos12 = tex_pos1 + offset12;

    var result = vec4<f32>(0.0);
    result += textureSample(tex, samp, (vec2<f32>(tex_pos0.x, tex_pos0.y)) / tex_size) * w0.x * w0.y;
    result += textureSample(tex, samp, (vec2<f32>(tex_pos12.x, tex_pos0.y)) / tex_size) * w12.x * w0.y;
    result += textureSample(tex, samp, (vec2<f32>(tex_pos3.x, tex_pos0.y)) / tex_size) * w3.x * w0.y;

    result += textureSample(tex, samp, (vec2<f32>(tex_pos0.x, tex_pos12.y)) / tex_size) * w0.x * w12.y;
    result += textureSample(tex, samp, (vec2<f32>(tex_pos12.x, tex_pos12.y)) / tex_size) * w12.x * w12.y;
    result += textureSample(tex, samp, (vec2<f32>(tex_pos3.x, tex_pos12.y)) / tex_size) * w3.x * w12.y;

    result += textureSample(tex, samp, (vec2<f32>(tex_pos0.x, tex_pos3.y)) / tex_size) * w0.x * w3.y;
    result += textureSample(tex, samp, (vec2<f32>(tex_pos12.x, tex_pos3.y)) / tex_size) * w12.x * w3.y;
    result += textureSample(tex, samp, (vec2<f32>(tex_pos3.x, tex_pos3.y)) / tex_size) * w3.x * w3.y;

    return result;
}

// 3x3 neighborhood clipping for variance-based clamping
fn clip_aabb(history: vec3<f32>, current: vec3<f32>) -> vec3<f32> {
    let tex_size = vec2<f32>(textureDimensions(t_current, 0));
    let texel_size = 1.0 / tex_size;

    // Sample 3x3 neighborhood
    var m1 = vec3<f32>(0.0);
    var m2 = vec3<f32>(0.0);

    for (var x = -1; x <= 1; x++) {
        for (var y = -1; y <= 1; y++) {
            let offset = vec2<f32>(f32(x), f32(y)) * texel_size;
            let sample_uv = current + offset;
            let neighbor = textureSample(t_current, s_linear, sample_uv).rgb;
            let neighbor_ycocg = rgb_to_ycocg(neighbor);

            m1 += neighbor_ycocg;
            m2 += neighbor_ycocg * neighbor_ycocg;
        }
    }

    // Variance-based AABB
    let sample_count = 9.0;
    let mean = m1 / sample_count;
    let variance = (m2 / sample_count) - (mean * mean);
    let std_dev = sqrt(max(variance, vec3<f32>(0.0)));

    let min_color = mean - std_dev * 1.5;
    let max_color = mean + std_dev * 1.5;

    let history_ycocg = rgb_to_ycocg(history);
    let clipped = clamp(history_ycocg, min_color, max_color);

    return ycocg_to_rgb(clipped);
}

// Sharpen using a simple kernel
fn sharpen(color: vec3<f32>, uv: vec2<f32>, amount: f32) -> vec3<f32> {
    let tex_size = vec2<f32>(textureDimensions(t_current, 0));
    let texel_size = 1.0 / tex_size;

    let n  = textureSample(t_current, s_linear, uv + vec2<f32>(0.0, -texel_size.y)).rgb;
    let s  = textureSample(t_current, s_linear, uv + vec2<f32>(0.0, texel_size.y)).rgb;
    let e  = textureSample(t_current, s_linear, uv + vec2<f32>(texel_size.x, 0.0)).rgb;
    let w  = textureSample(t_current, s_linear, uv + vec2<f32>(-texel_size.x, 0.0)).rgb;

    let edge = (n + s + e + w) * 0.25;
    let sharpened = color + (color - edge) * amount;

    return max(sharpened, vec3<f32>(0.0));
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    // Sample current frame
    let current_color = textureSample(t_current, s_linear, in.uv);

    // Sample motion vector
    let motion = textureSample(t_motion, s_linear, in.uv).rg;

    // Calculate history UV with motion reprojection
    let history_uv = in.uv - motion;

    // Check if history UV is valid (within screen bounds)
    if (history_uv.x < 0.0 || history_uv.x > 1.0 ||
        history_uv.y < 0.0 || history_uv.y > 1.0) {
        // Outside screen, no valid history
        return current_color;
    }

    // Sample history with high-quality Catmull-Rom filter
    let history_color = sample_catmull_rom(t_history, s_linear, history_uv);

    // Clip history to neighborhood AABB to reject invalid samples
    let clipped_history = clip_aabb(history_color.rgb, current_color.rgb);

    // Calculate adaptive blend factor based on motion magnitude
    let motion_length = length(motion);
    let motion_factor = saturate(motion_length * 100.0); // Scale factor

    // Blend between min and max feedback based on motion
    let feedback = mix(taa_params.feedback_max, taa_params.feedback_min, motion_factor);

    // Temporal blend
    var result = mix(current_color.rgb, clipped_history, feedback);

    // Apply sharpening to combat blur
    if (taa_params.sharpness > 0.0) {
        result = sharpen(result, in.uv, taa_params.sharpness);
    }

    return vec4<f32>(result, current_color.a);
}

// Copy shader for history buffer update
@fragment
fn fs_copy(in: VertexOutput) -> @location(0) vec4<f32> {
    return textureSample(t_current, s_linear, in.uv);
}
