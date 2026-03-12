// W3D Bloom Shader
// Gaussian blur for HDR bloom effects

@group(0) @binding(0)
var t_input: texture_2d<f32>;
@group(0) @binding(1)
var s_sampler: sampler;

struct BloomUniforms {
    direction: vec2<f32>, // (1,0) for horizontal, (0,1) for vertical
    threshold: f32,       // Brightness threshold for bloom
    intensity: f32,       // Bloom intensity multiplier
};

@group(1) @binding(0)
var<uniform> bloom: BloomUniforms;

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

@vertex
fn vs_main(@builtin(vertex_index) vertex_index: u32) -> VertexOutput {
    var out: VertexOutput;
    
    let x = f32((vertex_index & 1u) << 2u) - 1.0;
    let y = f32((vertex_index & 2u) << 1u) - 1.0;
    
    out.position = vec4<f32>(x, y, 0.0, 1.0);
    out.uv = vec2<f32>((x + 1.0) * 0.5, (1.0 - y) * 0.5);
    
    return out;
}

// Gaussian blur weights for 9-tap kernel
const BLUR_WEIGHTS = array<f32, 5>(
    0.227027, 0.1945946, 0.1216216, 0.054054, 0.016216
);

@fragment
fn fs_extract(in: VertexOutput) -> @location(0) vec4<f32> {
    let color = textureSample(t_input, s_sampler, in.uv);
    let brightness = dot(color.rgb, vec3<f32>(0.2126, 0.7152, 0.0722));
    
    if (brightness > bloom.threshold) {
        return vec4<f32>(color.rgb * bloom.intensity, color.a);
    } else {
        return vec4<f32>(0.0, 0.0, 0.0, 0.0);
    }
}

@fragment
fn fs_blur(in: VertexOutput) -> @location(0) vec4<f32> {
    let tex_offset = 1.0 / textureDimensions(t_input, 0);
    var result = textureSample(t_input, s_sampler, in.uv).rgb * BLUR_WEIGHTS[0];
    
    for (var i = 1; i < 5; i++) {
        let offset = bloom.direction * tex_offset * f32(i);
        result += textureSample(t_input, s_sampler, in.uv + offset).rgb * BLUR_WEIGHTS[i];
        result += textureSample(t_input, s_sampler, in.uv - offset).rgb * BLUR_WEIGHTS[i];
    }
    
    return vec4<f32>(result, 1.0);
}

@fragment
fn fs_combine(in: VertexOutput) -> @location(0) vec4<f32> {
    let hdr_color = textureSample(t_input, s_sampler, in.uv);
    // Bloom texture would be bound as a second texture in a real implementation
    return vec4<f32>(hdr_color.rgb, hdr_color.a);
}