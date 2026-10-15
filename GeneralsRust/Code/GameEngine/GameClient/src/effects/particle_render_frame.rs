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
        let info = system.info();
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

#[cfg(test)]
mod owned_info_tests {
    use super::*;
    use game_engine::common::system::xfer_load::XferLoad;
    use game_engine::common::system::xfer_save::XferSave;
    use game_engine::common::system::{Snapshotable, Xfer};
    use std::io::Cursor;

    #[test]
    fn owned_particle_info_load_drives_emission_and_prepared_vertices() {
        let mut authored = ParticleSystemTemplate::new("OwnedInfo".into());
        authored.info_mut().particle_type = ParticleType::Particle;
        authored.info_mut().shader_type = ParticleShaderType::Alpha;
        authored.info_mut().particle_type_name = "SavedTexture".into();
        authored.info_mut().alpha_keys[0].min_value = 1.0;
        authored.info_mut().alpha_keys[0].max_value = 1.0;
        authored.info_mut().color_keys[0].color = [1.0, 0.5, 0.25];
        authored.info_mut().color_keys[1].color = [1.0, 0.5, 0.25];
        authored.info_mut().color_keys[1].frame = 10;
        let authored = Arc::new(authored);
        let mut saved =
            ParticleSystem::new_with_random(authored.clone(), 7, false, &mut |lo, _| lo);
        saved.set_lifetime_range(99.0, 99.0);
        saved.tint_all_colors([0.25, 0.5, 1.0]);
        let mut bytes = Vec::new();
        let mut save = XferSave::new(Cursor::new(&mut bytes), 1);
        save.open("owned_info").unwrap();
        saved.xfer(&mut save).unwrap();
        save.close().unwrap();
        drop(save);

        let receiving = Arc::new(ParticleSystemTemplate::new("OwnedInfo".into()));
        let sibling = ParticleSystem::new_with_random(receiving.clone(), 8, false, &mut |lo, _| lo);
        let mut loaded =
            ParticleSystem::new_with_random(receiving.clone(), 0, false, &mut |lo, _| lo);
        let mut load = XferLoad::new(Cursor::new(bytes), 1);
        load.open("owned_info").unwrap();
        loaded.xfer(&mut load).unwrap();
        load.close().unwrap();
        assert!(Arc::ptr_eq(loaded.template(), &receiving));
        assert!(Arc::ptr_eq(sibling.template(), &receiving));
        assert_eq!(receiving.info().particle_type_name, "");
        assert_eq!(sibling.info().particle_type_name, "");
        let emitted = loaded.generate_particle_info(0, 1).unwrap();
        assert_eq!(emitted.lifetime, 99);
        assert_eq!(emitted.color_keys[1].color, [0.25, 0.25, 0.25]);
        assert_eq!(emitted.alpha_keys[0].value, 1.0);
        loaded.push_particle(Particle::new(&emitted, 1, 0));
        let mut frame = ParticleRenderFrame {
            uniforms: ParticleUniforms::default(),
            batches: Vec::new(),
            particles_rendered: 0,
            preparation_time: std::time::Duration::ZERO,
            heat_smudges: Vec::new(),
        };
        // Exercise the actual CPU collector without unrelated global SMUD
        // reset/collection: this is an ordinary sprite system.
        frame.collect_system_particles(&loaded, [0.0; 3]);
        assert_eq!(frame.particles_rendered, 1);
        assert_eq!(frame.batches.len(), 1);
        assert_eq!(frame.batches[0].shader_type, ParticleShaderType::Alpha);
        assert_eq!(frame.batches[0].texture_name, "SavedTexture");
        assert_eq!(frame.batches[0].vertices.len(), 1);
        assert_eq!(frame.batches[0].vertices[0].color, [1.0, 0.5, 0.25, 1.0]);
        assert!(Arc::ptr_eq(saved.template(), &authored));
    }
}
