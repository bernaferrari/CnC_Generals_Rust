// UI Shader for Command & Conquer Generals Zero Hour
// Handles rendering of UI rectangles, buttons, images, and other GUI elements

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
var ui_texture: texture_2d<f32>;
@group(1) @binding(1)
var ui_sampler: sampler;

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

// Fragment shader for solid color rendering
@fragment
fn fs_solid(input: VertexOutput) -> @location(0) vec4<f32> {
    return input.color;
}

// Fragment shader for textured rendering
@fragment
fn fs_textured(input: VertexOutput) -> @location(0) vec4<f32> {
    let tex_color = textureSample(ui_texture, ui_sampler, input.tex_coord);
    return tex_color * input.color;
}

// Fragment shader for UI elements with special effects
@fragment
fn fs_effect(input: VertexOutput) -> @location(0) vec4<f32> {
    let tex_color = textureSample(ui_texture, ui_sampler, input.tex_coord);
    var color = tex_color * input.color;
    
    // Add pulsing effect for buttons
    let pulse = sin(uniforms.time * 3.0) * 0.1 + 0.9;
    color = color * pulse;
    
    return color;
}

// Fragment shader for disabled UI elements
@fragment  
fn fs_disabled(input: VertexOutput) -> @location(0) vec4<f32> {
    let tex_color = textureSample(ui_texture, ui_sampler, input.tex_coord);
    var color = tex_color * input.color;
    
    // Convert to grayscale and reduce alpha
    let gray = dot(color.rgb, vec3<f32>(0.299, 0.587, 0.114));
    color = vec4<f32>(gray, gray, gray, color.a * 0.5);
    
    return color;
}

// Fragment shader for highlighted UI elements
@fragment
fn fs_highlight(input: VertexOutput) -> @location(0) vec4<f32> {
    let tex_color = textureSample(ui_texture, ui_sampler, input.tex_coord);
    var color = tex_color * input.color;
    
    // Add highlight glow
    color = vec4<f32>(color.rgb + vec3<f32>(0.2, 0.2, 0.2), color.a);
    
    return color;
}

// Fragment shader for pressed button state
@fragment
fn fs_pressed(input: VertexOutput) -> @location(0) vec4<f32> {
    let tex_color = textureSample(ui_texture, ui_sampler, input.tex_coord);
    var color = tex_color * input.color;
    
    // Darken the color for pressed state
    color = vec4<f32>(color.rgb * 0.8, color.a);
    
    return color;
}
