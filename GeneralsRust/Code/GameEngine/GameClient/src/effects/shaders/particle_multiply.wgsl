// Particle Multiply Blending Fragment Shader
// For darkening effects like shadows and scorch marks

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) tex_coord: vec2<f32>,
    @location(1) color: vec4<f32>,
    @location(2) alpha: f32,
    @location(3) world_position: vec3<f32>,
};

@group(1) @binding(0)
var particle_texture: texture_2d<f32>;

@group(1) @binding(1)
var particle_sampler: sampler;

@fragment
fn fs_main(input: VertexOutput) -> @location(0) vec4<f32> {
    // Sample texture
    let tex_color = textureSample(particle_texture, particle_sampler, input.tex_coord);

    // Combine with particle color
    var final_color = tex_color * input.color;

    // Apply alpha
    final_color.a *= input.alpha;

    // For multiply blending, we darken the output
    // The blend state will multiply this with the framebuffer
    // Lerp between white (no darkening) and the color based on alpha
    final_color = vec4<f32>(
        mix(vec3<f32>(1.0, 1.0, 1.0), final_color.rgb, final_color.a),
        final_color.a,
    );
    final_color.a = 1.0; // Multiply blending doesn't use alpha channel

    return final_color;
}
