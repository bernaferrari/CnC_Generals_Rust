//! CPU preparation at the original post-scene particle boundary.
//! Prepared batches own vertices and immutable submission facts, never systems.

use super::*;

#[derive(Debug, Clone, PartialEq)]
pub(super) struct PreparedParticleBatch {
    pub(super) shader_type: ParticleShaderType,
    pub(super) texture_name: String,
    pub(super) vertices: Vec<ParticleVertex>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ParticleRenderFrame {
    pub(super) uniforms: ParticleUniforms,
    pub(super) batches: Vec<PreparedParticleBatch>,
    pub(super) particles_rendered: usize,
    pub(super) preparation_time: std::time::Duration,
    heat_smudges: Vec<crate::effects::heat_haze::HeatHazeSmudge>,
}

impl ParticleRenderFrame {
    /// Call at the existing DoParticles phase after culling. SMUD reset and
    /// RNG consumption retain their original order relative to each system.
    pub fn prepare(systems: &[&ParticleSystem], uniforms: &ParticleUniforms) -> Self {
        let preparation_started = std::time::Instant::now();
        begin_particle_heat_smudge_frame();
        let mut frame = Self {
            uniforms: *uniforms,
            batches: Vec::new(),
            particles_rendered: 0,
            preparation_time: std::time::Duration::ZERO,
            heat_smudges: Vec::new(),
        };
        for system in systems {
            frame.collect_system_particles(system, uniforms.camera_position);
        }
        frame.heat_smudges = get_smudge_manager()
            .lock()
            .map(|manager| {
                crate::effects::heat_haze::collect_heat_haze_smudges(
                    &manager.collect_used_smudges(),
                )
            })
            .unwrap_or_default();
        frame.preparation_time = preparation_started.elapsed();
        frame
    }

    pub fn heat_smudges(&self) -> &[crate::effects::heat_haze::HeatHazeSmudge] {
        &self.heat_smudges
    }

    fn collect_system_particles(&mut self, system: &ParticleSystem, camera_position: [f32; 3]) {
        let template = system.template();
        let info = template.info();
        if matches!(
            info.particle_type,
            ParticleType::Invalid | ParticleType::Drawable
        ) {
            return;
        }
        // C++ DWORD "SMUD" prefix / Type=SMUDGE: convert visible particles
        // into heat smudges and continue — never submit as sprites
        // (`W3DParticleSys.cpp:142-172`).
        if system_is_heat_smudge(system) {
            let _ = feed_system_heat_smudges(system);
            return;
        }
        if info.shader_type == ParticleShaderType::Invalid {
            return;
        }

        let texture_name = if info.particle_type_name.is_empty() {
            "default".to_string()
        } else {
            info.particle_type_name.clone()
        };

        // `W3DParticleSystemManager::doParticles` renders every live system
        // immediately in `m_allParticleSystemList` order.  Keep a separate
        // submission even when adjacent systems share a texture/shader.
        let mut batch = ParticleBatch::new(info.shader_type, texture_name);

        if info.particle_type == ParticleType::Streak {
            let before = batch.vertices.len();
            batch.add_streak_polyline(system);
            self.particles_rendered += batch.vertices.len().saturating_sub(before);
        } else {
            for particle in system.particles() {
                if !particle.is_draw_alive() {
                    continue;
                }
                match info.particle_type {
                    ParticleType::VolumeParticle => {
                        let before = batch.vertices.len();
                        batch.add_volume_particle(particle, system, camera_position);
                        self.particles_rendered += batch.vertices.len().saturating_sub(before);
                        continue;
                    }
                    ParticleType::Particle => batch.add_particle(particle, system),
                    ParticleType::Streak
                    | ParticleType::Invalid
                    | ParticleType::Drawable
                    | ParticleType::Smudge => continue,
                }
                self.particles_rendered += 1;
            }
        }

        // C++ skips texture lookup and draw submission for an empty system.
        if !batch.vertices.is_empty() {
            self.batches.push(PreparedParticleBatch {
                shader_type: batch.shader_type,
                texture_name: batch.texture_name,
                vertices: batch.vertices,
            });
        }
    }
}
