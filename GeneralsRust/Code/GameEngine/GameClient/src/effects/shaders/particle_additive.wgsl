// Particle Additive Blending Fragment Shader
// For bright effects like explosions, muzzle flashes, sparks

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

    // For additive blending, we premultiply by alpha
    // The blend state will add this to the framebuffer
    final_color.rgb *= final_color.a;

    return final_color;
}
