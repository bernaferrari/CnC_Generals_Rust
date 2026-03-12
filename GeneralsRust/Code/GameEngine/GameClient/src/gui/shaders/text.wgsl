// Text Rendering Shader for Command & Conquer Generals Zero Hour
// Handles rendering of UI text with proper anti-aliasing and effects

struct UIUniforms {
    view_projection: mat4x4<f32>,
    screen_size: vec2<f32>,
    time: f32,
    _padding: f32,
};

struct VertexInput {
    @location(0) position: vec3<f32>,
    @location(1) tex_coord: vec2<f32>,
    @location(2) color: vec4<f32>,
};

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) tex_coord: vec2<f32>,
    @location(1) color: vec4<f32>,
    @location(2) world_position: vec2<f32>,
};

@group(0) @binding(0)
var<uniform> uniforms: UIUniforms;

@group(1) @binding(0)
var font_atlas: texture_2d<f32>;
@group(1) @binding(1)
var font_sampler: sampler;

@vertex
fn vs_main(input: VertexInput) -> VertexOutput {
    var out: VertexOutput;
    
    let world_pos = vec4<f32>(input.position, 1.0);
    out.clip_position = uniforms.view_projection * world_pos;
    out.tex_coord = input.tex_coord;
    out.color = input.color;
    out.world_position = input.position.xy;
    
    return out;
}

// Fragment shader for SDF (Signed Distance Field) text rendering
@fragment
fn fs_main(input: VertexOutput) -> @location(0) vec4<f32> {
    // Sample the font atlas texture
    let distance = textureSample(font_atlas, font_sampler, input.tex_coord).r;
    
    // SDF text rendering with smooth edges
    let smoothing = fwidth(distance);
    let alpha = smoothstep(0.5 - smoothing, 0.5 + smoothing, distance);
    
    var color = input.color;
    color.a = color.a * alpha;
    
    return color;
}

// Fragment shader for outlined text
@fragment
fn fs_outline(input: VertexOutput) -> @location(0) vec4<f32> {
    let distance = textureSample(font_atlas, font_sampler, input.tex_coord).r;
    
    let smoothing = fwidth(distance);
    
    // Main text
    let text_alpha = smoothstep(0.5 - smoothing, 0.5 + smoothing, distance);
    
    // Outline
    let outline_width = 0.2;
    let outline_alpha = smoothstep(0.5 - outline_width - smoothing, 0.5 - outline_width + smoothing, distance);
    
    // Combine text and outline
    let outline_color = vec4<f32>(0.0, 0.0, 0.0, 1.0); // Black outline
    var final_color = mix(outline_color, input.color, text_alpha);
    final_color.a = final_color.a * outline_alpha;
    
    return final_color;
}

// Fragment shader for drop shadow text
@fragment
fn fs_shadow(input: VertexOutput) -> @location(0) vec4<f32> {
    let distance = textureSample(font_atlas, font_sampler, input.tex_coord).r;
    
    // Sample shadow offset
    let shadow_offset = vec2<f32>(1.0, 1.0) / uniforms.screen_size;
    let shadow_distance = textureSample(font_atlas, font_sampler, input.tex_coord + shadow_offset).r;
    
    let smoothing = fwidth(distance);
    
    // Main text
    let text_alpha = smoothstep(0.5 - smoothing, 0.5 + smoothing, distance);
    
    // Shadow
    let shadow_alpha = smoothstep(0.5 - smoothing, 0.5 + smoothing, shadow_distance) * 0.5;
    
    // Combine text and shadow
    let shadow_color = vec4<f32>(0.0, 0.0, 0.0, 1.0); // Black shadow
    var final_color = mix(shadow_color, input.color, text_alpha);
    final_color.a = max(final_color.a * text_alpha, shadow_alpha);
    
    return final_color;
}

// Fragment shader for glowing text effect
@fragment
fn fs_glow(input: VertexOutput) -> @location(0) vec4<f32> {
    let distance = textureSample(font_atlas, font_sampler, input.tex_coord).r;
    
    let smoothing = fwidth(distance);
    let text_alpha = smoothstep(0.5 - smoothing, 0.5 + smoothing, distance);
    
    // Add glow effect
    let glow_width = 0.3;
    let glow_alpha = smoothstep(0.5 - glow_width - smoothing, 0.5 - glow_width + smoothing, distance);
    
    var color = input.color;
    
    // Pulsing glow based on time
    let pulse = sin(uniforms.time * 2.0) * 0.3 + 0.7;
    let glow_intensity = glow_alpha * pulse;
    
    color = vec4<f32>(color.rgb + color.rgb * glow_intensity, color.a);
    color.a = color.a * max(text_alpha, glow_alpha * 0.5);
    
    return color;
}
