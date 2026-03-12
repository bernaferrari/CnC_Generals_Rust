// Particle Vertex Shader
// GPU-accelerated billboard particle rendering for Command & Conquer Generals Zero Hour

struct ParticleUniforms {
    view_matrix: mat4x4<f32>,
    projection_matrix: mat4x4<f32>,
    camera_position: vec3<f32>,
    time: f32,
    screen_size: vec2<f32>,
    particle_count: u32,
    _padding: u32,
};

struct ParticleInstance {
    @location(0) position: vec3<f32>,
    @location(1) size: vec2<f32>,
    @location(2) color: vec4<f32>,
    @location(3) uv_rect: vec4<f32>,
    @location(4) rotation: f32,
    @location(5) alpha: f32,
};

struct BillboardVertex {
    @location(6) corner: vec2<f32>,
    @location(7) tex_coord: vec2<f32>,
};

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) tex_coord: vec2<f32>,
    @location(1) color: vec4<f32>,
    @location(2) alpha: f32,
    @location(3) world_position: vec3<f32>,
};

@group(0) @binding(0)
var<uniform> uniforms: ParticleUniforms;

@vertex
fn vs_main(
    vertex: BillboardVertex,
    particle: ParticleInstance,
) -> VertexOutput {
    var output: VertexOutput;

    // Calculate billboard vectors (camera-facing)
    let view_inv = transpose(uniforms.view_matrix);
    let right = vec3<f32>(view_inv[0].x, view_inv[0].y, view_inv[0].z);
    let up = vec3<f32>(view_inv[1].x, view_inv[1].y, view_inv[1].z);

    // Apply rotation to billboard corners
    let cos_rot = cos(particle.rotation);
    let sin_rot = sin(particle.rotation);
    let rotated_corner = vec2<f32>(
        vertex.corner.x * cos_rot - vertex.corner.y * sin_rot,
        vertex.corner.x * sin_rot + vertex.corner.y * cos_rot
    );

    // Calculate world position with billboarding
    let offset = right * rotated_corner.x * particle.size.x + up * rotated_corner.y * particle.size.y;
    let world_pos = particle.position + offset;

    // Transform to clip space
    let view_pos = uniforms.view_matrix * vec4<f32>(world_pos, 1.0);
    output.position = uniforms.projection_matrix * view_pos;

    // Calculate texture coordinates with UV rect for atlas support
    output.tex_coord = mix(
        particle.uv_rect.xy,
        particle.uv_rect.zw,
        vertex.tex_coord
    );

    // Pass through color and alpha
    output.color = particle.color;
    output.alpha = particle.alpha;
    output.world_position = world_pos;

    return output;
}
