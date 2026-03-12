// 2D rendering shader for UI and text
// Ported from WW3D's 2D rendering system

struct VertexInput {
    @location(0) position: vec2<f32>,
    @location(1) uv: vec2<f32>,
    @location(2) color: u32,
};

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) color: vec4<f32>,
};

struct FragmentOutput {
    @location(0) color: vec4<f32>,
};

@vertex
fn vs_main(input: VertexInput) -> VertexOutput {
    var output: VertexOutput;

    // Convert position to clip space (-1 to 1)
    output.clip_position = vec4<f32>(input.position, 0.0, 1.0);

    // Pass through UV coordinates
    output.uv = input.uv;

    // Convert packed color to vec4
    let r = f32((input.color >> 16) & 0xFF) / 255.0;
    let g = f32((input.color >> 8) & 0xFF) / 255.0;
    let b = f32(input.color & 0xFF) / 255.0;
    let a = f32((input.color >> 24) & 0xFF) / 255.0;
    output.color = vec4<f32>(r, g, b, a);

    return output;
}

@group(0) @binding(0) var t_texture: texture_2d<f32>;
@group(0) @binding(1) var s_texture: sampler;

@fragment
fn fs_main(input: VertexOutput) -> FragmentOutput {
    var output: FragmentOutput;

    // Sample texture
    let tex_color = textureSample(t_texture, s_texture, input.uv);

    // Multiply with vertex color
    output.color = tex_color * input.color;

    return output;
}

@fragment
fn fs_solid_main(input: VertexOutput) -> FragmentOutput {
    var output: FragmentOutput;

    // Use vertex color only (no texture)
    output.color = input.color;

    return output;
}

@fragment
fn fs_grayscale_main(input: VertexOutput) -> FragmentOutput {
    var output: FragmentOutput;

    // Sample texture and convert to grayscale
    let tex_color = textureSample(t_texture, s_texture, input.uv);

    // Calculate luminance
    let luminance = dot(tex_color.rgb, vec3<f32>(0.299, 0.587, 0.114));
    let gray_color = vec3<f32>(luminance, luminance, luminance);

    // Multiply with vertex color
    output.color = vec4<f32>(gray_color * input.color.rgb, tex_color.a * input.color.a);

    return output;
}