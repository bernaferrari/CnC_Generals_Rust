// W3D Tone Mapping Shader
// HDR to LDR tone mapping with multiple tone mapping operators

@group(0) @binding(0)
var t_hdr: texture_2d<f32>;
@group(0) @binding(1)
var s_hdr: sampler;

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

// Fullscreen triangle vertices
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

// Reinhard tone mapping
fn reinhard_tonemap(color: vec3<f32>) -> vec3<f32> {
    return color / (color + vec3<f32>(1.0));
}

// ACES tone mapping (approximation)
fn aces_tonemap(color: vec3<f32>) -> vec3<f32> {
    let a = 2.51;
    let b = 0.03;
    let c = 2.43;
    let d = 0.59;
    let e = 0.14;
    
    return clamp((color * (a * color + b)) / (color * (c * color + d) + e), vec3<f32>(0.0), vec3<f32>(1.0));
}

// Exposure tone mapping
fn exposure_tonemap(color: vec3<f32>, exposure: f32) -> vec3<f32> {
    return vec3<f32>(1.0) - exp(-color * exposure);
}

// Linear to sRGB conversion
fn linear_to_srgb(color: vec3<f32>) -> vec3<f32> {
    return select(
        pow(color, vec3<f32>(1.0 / 2.2)),
        color * 12.92,
        color <= vec3<f32>(0.0031308)
    );
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let hdr_color = textureSample(t_hdr, s_hdr, in.uv).rgb;
    
    // Apply tone mapping (using ACES for high quality)
    let tone_mapped = aces_tonemap(hdr_color);
    
    // Convert to sRGB
    let final_color = linear_to_srgb(tone_mapped);
    
    return vec4<f32>(final_color, 1.0);
}