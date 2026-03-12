// W3D GPU Particle System Shader
// High-performance particle rendering with compute shader simulation

struct CameraData {
    view_matrix: mat4x4<f32>,
    projection_matrix: mat4x4<f32>,
    view_projection_matrix: mat4x4<f32>,
    prev_view_projection_matrix: mat4x4<f32>,
    inverse_view_matrix: mat4x4<f32>,
    inverse_projection_matrix: mat4x4<f32>,
    camera_position: vec3<f32>,
    camera_direction: vec3<f32>,
    near_plane: f32,
    far_plane: f32,
    fov: f32,
    aspect_ratio: f32,
};

struct ParticleUniforms {
    delta_time: f32,
    total_time: f32,
    gravity: vec3<f32>,
    wind: vec3<f32>,
    max_particles: u32,
};

struct Particle {
    position: vec3<f32>,
    velocity: vec3<f32>,
    color: vec4<f32>,
    size_rotation: vec2<f32>, // size, rotation
    age_lifetime: vec2<f32>,  // age, lifetime
};

@group(0) @binding(0)
var<uniform> camera: CameraData;

@group(1) @binding(0)
var<uniform> particle_uniforms: ParticleUniforms;

@group(2) @binding(0)
var<storage, read_write> particles: array<Particle>;

@group(3) @binding(0)
var t_particle: texture_2d<f32>;
@group(3) @binding(1)
var s_particle: sampler;

// ============= COMPUTE SHADER FOR PARTICLE SIMULATION =============

@compute @workgroup_size(64, 1, 1)
fn cs_simulate(@builtin(global_invocation_id) global_id: vec3<u32>) {
    let index = global_id.x;
    if (index >= particle_uniforms.max_particles) {
        return;
    }
    
    var particle = particles[index];
    
    // Skip dead particles
    if (particle.age_lifetime.x >= particle.age_lifetime.y) {
        return;
    }
    
    // Update age
    particle.age_lifetime.x += particle_uniforms.delta_time;
    
    // Apply forces
    let gravity_force = particle_uniforms.gravity * particle_uniforms.delta_time;
    let wind_force = particle_uniforms.wind * particle_uniforms.delta_time * 0.5;
    
    // Add some noise for turbulence
    let noise = vec3<f32>(
        sin(particle_uniforms.total_time + f32(index) * 0.1) * 0.1,
        cos(particle_uniforms.total_time + f32(index) * 0.17) * 0.1,
        sin(particle_uniforms.total_time * 1.3 + f32(index) * 0.13) * 0.1
    );
    
    // Apply drag
    particle.velocity *= 0.98;
    
    // Update velocity
    particle.velocity += gravity_force + wind_force + noise;
    
    // Update position
    particle.position += particle.velocity * particle_uniforms.delta_time;
    
    // Update rotation
    particle.size_rotation.y += particle_uniforms.delta_time * 2.0;
    
    // Fade out over lifetime
    let life_factor = 1.0 - (particle.age_lifetime.x / particle.age_lifetime.y);
    particle.color.a = life_factor;
    
    // Update size over lifetime (grow then shrink)
    let size_curve = 4.0 * life_factor * (1.0 - life_factor); // Parabolic curve
    particle.size_rotation.x = particle.size_rotation.x * size_curve;
    
    // Write back to buffer
    particles[index] = particle;
}

// ============= VERTEX/FRAGMENT SHADERS FOR RENDERING =============

struct VertexInput {
    @builtin(instance_index) instance_id: u32,
    @location(0) position: vec2<f32>, // Quad vertex position (-1 to 1)
    @location(1) uv: vec2<f32>,
};

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) color: vec4<f32>,
    @location(2) world_position: vec3<f32>,
};

@vertex
fn vs_main(input: VertexInput) -> VertexOutput {
    var output: VertexOutput;
    
    let particle = particles[input.instance_id];
    
    // Skip dead particles
    if (particle.age_lifetime.x >= particle.age_lifetime.y) {
        output.clip_position = vec4<f32>(0.0, 0.0, 0.0, 0.0);
        return output;
    }
    
    // Billboard the particle to face the camera
    let view_right = vec3<f32>(camera.view_matrix[0][0], camera.view_matrix[1][0], camera.view_matrix[2][0]);
    let view_up = vec3<f32>(camera.view_matrix[0][1], camera.view_matrix[1][1], camera.view_matrix[2][1]);
    
    // Apply rotation
    let cos_rot = cos(particle.size_rotation.y);
    let sin_rot = sin(particle.size_rotation.y);
    let rotated_pos = vec2<f32>(
        input.position.x * cos_rot - input.position.y * sin_rot,
        input.position.x * sin_rot + input.position.y * cos_rot
    );
    
    // Calculate world position
    let world_pos = particle.position + 
                   (view_right * rotated_pos.x * particle.size_rotation.x) +
                   (view_up * rotated_pos.y * particle.size_rotation.x);
    
    output.world_position = world_pos;
    output.clip_position = camera.view_projection_matrix * vec4<f32>(world_pos, 1.0);
    output.uv = input.uv;
    output.color = particle.color;
    
    return output;
}

@fragment
fn fs_main(input: VertexOutput) -> @location(0) vec4<f32> {
    let texture_color = textureSample(t_particle, s_particle, input.uv);
    let final_color = texture_color * input.color;
    
    // Soft particle effect (fade when close to geometry)
    // This would require depth buffer sampling in a real implementation
    
    // Alpha testing to discard fully transparent pixels
    if (final_color.a < 0.01) {
        discard;
    }
    
    return final_color;
}

// ============= PARTICLE EMISSION COMPUTE SHADER =============

struct EmitterData {
    position: vec3<f32>,
    direction: vec3<f32>,
    spread_angle: f32,
    emission_rate: f32,
    speed_range: vec2<f32>,
    size_range: vec2<f32>,
    lifetime_range: vec2<f32>,
    color: vec4<f32>,
};

@group(4) @binding(0)
var<uniform> emitter: EmitterData;

@group(4) @binding(1)
var<storage, read_write> emission_counter: array<u32>;

// Simple random function
fn random(seed: u32) -> f32 {
    let s = seed * 747796405u + 2891336453u;
    let result = ((s >> ((s >> 28u) + 4u)) ^ s) * 277803737u;
    return f32(result >> 4u) / f32(0xFFFFFFFu);
}

@compute @workgroup_size(1, 1, 1)
fn cs_emit(@builtin(global_invocation_id) global_id: vec3<u32>) {
    let particles_to_emit = u32(emitter.emission_rate * particle_uniforms.delta_time);
    
    for (var i = 0u; i < particles_to_emit; i++) {
        let counter = atomicAdd(&emission_counter[0], 1u);
        let particle_index = counter % particle_uniforms.max_particles;
        
        // Generate random properties
        let seed_base = counter + u32(particle_uniforms.total_time * 1000.0);
        
        let speed = mix(emitter.speed_range.x, emitter.speed_range.y, random(seed_base));
        let size = mix(emitter.size_range.x, emitter.size_range.y, random(seed_base + 1u));
        let lifetime = mix(emitter.lifetime_range.x, emitter.lifetime_range.y, random(seed_base + 2u));
        
        // Random direction within spread cone
        let theta = random(seed_base + 3u) * 2.0 * 3.14159265;
        let phi = random(seed_base + 4u) * emitter.spread_angle;
        
        let dir = vec3<f32>(
            sin(phi) * cos(theta),
            cos(phi),
            sin(phi) * sin(theta)
        );
        
        // Create new particle
        var new_particle: Particle;
        new_particle.position = emitter.position;
        new_particle.velocity = normalize(emitter.direction + dir) * speed;
        new_particle.color = emitter.color;
        new_particle.size_rotation = vec2<f32>(size, 0.0);
        new_particle.age_lifetime = vec2<f32>(0.0, lifetime);
        
        particles[particle_index] = new_particle;
    }
}