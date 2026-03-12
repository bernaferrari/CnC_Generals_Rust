// Particle compute shader for authentic C&C effects
// Handles GPU-accelerated particle simulation matching original performance

struct ParticleUniforms {
    view_proj: mat4x4<f32>,
    time: f32,
    delta_time: f32,
    gravity: f32,
    _padding: f32,
}

struct GPUParticle {
    position: vec4<f32>,     // xyz + padding
    velocity: vec4<f32>,     // xyz + padding  
    color: vec4<f32>,        // rgb + alpha
    size_angle: vec4<f32>,   // size, angle, size_rate, angular_rate
    lifetime_data: vec4<u32>, // lifetime_left, create_timestamp, personality, flags
}

@group(0) @binding(0) var<uniform> uniforms: ParticleUniforms;
@group(0) @binding(1) var<storage, read_write> particles: array<GPUParticle>;

@compute @workgroup_size(64)
fn main(@builtin(global_invocation_id) global_id: vec3<u32>) {
    let index = global_id.x;
    
    if (index >= arrayLength(&particles)) {
        return;
    }
    
    var particle = particles[index];
    
    // Skip dead particles
    if (particle.lifetime_data.x == 0u) {
        return;
    }
    
    // Update lifetime
    if (particle.lifetime_data.x > 0u) {
        particle.lifetime_data.x = particle.lifetime_data.x - 1u;
    }
    
    // Physics update
    let dt = uniforms.delta_time;
    let gravity_force = vec3<f32>(0.0, 0.0, -uniforms.gravity * dt);
    
    // Update velocity with gravity
    particle.velocity = vec4<f32>(particle.velocity.xyz + gravity_force, 0.0);
    
    // Apply velocity damping (simplified)
    particle.velocity = particle.velocity * 0.99;
    
    // Update position
    particle.position = vec4<f32>(particle.position.xyz + particle.velocity.xyz * dt, 0.0);
    
    // Update size
    let size_rate = particle.size_angle.z;
    particle.size_angle.x = particle.size_angle.x + size_rate * dt;
    particle.size_angle.x = max(particle.size_angle.x, 0.0);
    
    // Update angle
    let angular_rate = particle.size_angle.w;
    particle.size_angle.y = particle.size_angle.y + angular_rate * dt;
    
    // Update alpha based on lifetime (simplified)
    let lifetime_progress = f32(particle.lifetime_data.x) / 60.0; // Assuming 60 frame lifetime
    particle.color.w = max(lifetime_progress, 0.0);
    
    // Wind motion (simplified)
    let wind_strength = 0.5;
    let wind_angle = uniforms.time * 0.01;
    let wind_force = vec3<f32>(
        cos(wind_angle) * wind_strength,
        sin(wind_angle) * wind_strength,
        0.0
    ) * dt;
    
    particle.velocity = vec4<f32>(particle.velocity.xyz + wind_force, 0.0);
    
    // Write back to buffer
    particles[index] = particle;
}