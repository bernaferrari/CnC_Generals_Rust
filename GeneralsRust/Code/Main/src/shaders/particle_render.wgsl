// Particle render shader for authentic C&C visual effects
// Renders billboarded particles with proper blending and effects

struct ParticleUniforms {
    view_proj: mat4x4<f32>,
    time: f32,
    delta_time: f32,
    gravity: f32,
    _padding: f32,
}

struct GPUParticle {
    position: vec4<f32>,
    velocity: vec4<f32>,
    color: vec4<f32>,
    size_angle: vec4<f32>,
    lifetime_data: vec4<u32>,
}

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) tex_coords: vec2<f32>,
    @location(1) color: vec4<f32>,
}

@group(0) @binding(0) var<uniform> uniforms: ParticleUniforms;
@group(0) @binding(1) var<storage, read> particles: array<GPUParticle>;

// Vertex shader - generates quad vertices for each particle
@vertex
fn vs_main(@builtin(vertex_index) vertex_index: u32, @builtin(instance_index) instance_index: u32) -> VertexOutput {
    let particle = particles[instance_index];
    
    // Skip dead particles
    if (particle.lifetime_data.x == 0u) {
        // Return degenerate triangle
        return VertexOutput(
            vec4<f32>(0.0, 0.0, -1000.0, 1.0),
            vec2<f32>(0.0, 0.0),
            vec4<f32>(0.0, 0.0, 0.0, 0.0)
        );
    }
    
    // Quad vertices in local space
    var quad_vertices = array<vec2<f32>, 6>(
        vec2<f32>(-1.0, -1.0), // Bottom-left
        vec2<f32>( 1.0, -1.0), // Bottom-right
        vec2<f32>(-1.0,  1.0), // Top-left
        vec2<f32>( 1.0, -1.0), // Bottom-right
        vec2<f32>( 1.0,  1.0), // Top-right
        vec2<f32>(-1.0,  1.0)  // Top-left
    );
    
    var tex_coords_array = array<vec2<f32>, 6>(
        vec2<f32>(0.0, 1.0), // Bottom-left
        vec2<f32>(1.0, 1.0), // Bottom-right
        vec2<f32>(0.0, 0.0), // Top-left
        vec2<f32>(1.0, 1.0), // Bottom-right
        vec2<f32>(1.0, 0.0), // Top-right
        vec2<f32>(0.0, 0.0)  // Top-left
    );
    
    let quad_pos = quad_vertices[vertex_index % 6u];
    let tex_coord = tex_coords_array[vertex_index % 6u];
    
    // Get particle properties
    let world_pos = particle.position.xyz;
    let size = particle.size_angle.x;
    let angle = particle.size_angle.y;
    let particle_color = particle.color;
    
    // Create rotation matrix for particle angle
    let cos_angle = cos(angle);
    let sin_angle = sin(angle);
    let rotation = mat2x2<f32>(
        vec2<f32>(cos_angle, -sin_angle),
        vec2<f32>(sin_angle, cos_angle)
    );
    
    // Apply rotation and scale to quad vertex
    let rotated_pos = rotation * quad_pos;
    let scaled_pos = rotated_pos * size;
    
    // Billboard the particle to face the camera
    // For now, just use screen-space billboarding
    let view_pos = uniforms.view_proj * vec4<f32>(world_pos, 1.0);
    let screen_pos = view_pos.xy / view_pos.w;
    
    // Add quad offset in screen space
    let final_screen_pos = screen_pos + scaled_pos * 0.01; // Scale factor for screen space
    let final_pos = vec4<f32>(final_screen_pos * view_pos.w, view_pos.z, view_pos.w);
    
    return VertexOutput(
        final_pos,
        tex_coord,
        particle_color
    );
}

// Fragment shader - renders the particle with blending
@fragment
fn fs_main(input: VertexOutput) -> @location(0) vec4<f32> {
    let tex_coord = input.tex_coords;
    let color = input.color;
    
    // Calculate distance from center for circular particles
    let center_dist = length(tex_coord - vec2<f32>(0.5, 0.5)) * 2.0;
    
    // Create circular falloff
    let alpha_falloff = 1.0 - smoothstep(0.0, 1.0, center_dist);
    
    // Apply particle color and alpha
    var final_color = color;
    final_color.w *= alpha_falloff;
    
    // Add some variation based on texture coordinates for visual interest
    let flame_pattern = sin(tex_coord.x * 6.28) * sin(tex_coord.y * 6.28) * 0.1 + 0.9;
    final_color.xyz *= flame_pattern;
    
    // Ensure we don't output transparent pixels that would waste bandwidth
    if (final_color.w < 0.01) {
        discard;
    }
    
    return final_color;
}