// Enhanced 2D bitmap shader with advanced features
// Supports: texture sampling, UV wrapping, colorization, blend modes, grayscale

struct VertexInput {
    @location(0) position: vec2<f32>,
    @location(1) tex_coords: vec2<f32>,
    @location(2) color: vec4<f32>,
}

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) tex_coords: vec2<f32>,
    @location(1) color: vec4<f32>,
}

struct ShaderUniforms {
    // UV transformation
    uv_offset: vec2<f32>,
    uv_scale: vec2<f32>,

    // Rendering flags
    enable_wrapping: u32,     // 0 = clamp, 1 = repeat
    enable_grayscale: u32,    // Convert to grayscale
    blend_mode: u32,          // 0 = normal, 1 = additive, 2 = multiply
    ignore_alpha: u32,        // Force alpha to 1.0

    // Color modulation
    color_tint: vec4<f32>,

    // Padding for alignment
    _padding: vec2<f32>,
}

@group(0) @binding(0)
var t_diffuse: texture_2d<f32>;

@group(0) @binding(1)
var s_diffuse: sampler;

@group(1) @binding(0)
var<uniform> uniforms: ShaderUniforms;

@vertex
fn vs_main(model: VertexInput) -> VertexOutput {
    var out: VertexOutput;

    // Apply UV transformation
    out.tex_coords = model.tex_coords * uniforms.uv_scale + uniforms.uv_offset;

    // Apply vertex color and tint
    out.color = model.color * uniforms.color_tint;

    // Transform position to clip space
    out.clip_position = vec4<f32>(model.position, 0.0, 1.0);

    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    var uv = in.tex_coords;

    // Handle UV wrapping
    if (uniforms.enable_wrapping != 0u) {
        uv = fract(uv);
    } else {
        uv = clamp(uv, vec2<f32>(0.0), vec2<f32>(1.0));
    }

    // Sample texture
    var color = textureSample(t_diffuse, s_diffuse, uv);

    // Apply vertex color modulation
    color = color * in.color;

    // Grayscale conversion
    if (uniforms.enable_grayscale != 0u) {
        let luminance = dot(color.rgb, vec3<f32>(0.299, 0.587, 0.114));
        color = vec4<f32>(vec3<f32>(luminance), color.a);
    }

    // Ignore alpha if requested
    if (uniforms.ignore_alpha != 0u) {
        color.a = 1.0;
    }

    // Blend mode (handled via color output, actual blending set in pipeline)
    // Mode 0: Normal (default)
    // Mode 1: Additive (requires additive blend state)
    // Mode 2: Multiply (requires multiply blend state)

    return color;
}