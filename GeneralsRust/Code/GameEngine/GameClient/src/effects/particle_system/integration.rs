//! One-particle integration and owned removal continuation.
use super::{Particle, ParticleShaderType, ParticleSystem, Vec3, WindMotion};

/// Operation-local particle integration; cached values match the original loop.
pub(crate) struct ParticleIntegrationCursor {
    index: usize,
    current_frame: u32,
    gravity: f32,
    drift_velocity: Vec3,
    wind_angle: f32,
    wind_motion: WindMotion,
    shader_type: ParticleShaderType,
    system_pos: Vec3,
}

pub(crate) enum ParticleIntegrationStep {
    Alive,
    Removed(Particle),
    Complete,
}

impl ParticleSystem {
    /// C++ particle loop + lifetime tick after createParticle.
    pub(crate) fn finish_frame_integrate(&mut self, current_frame: u32) {
        let mut cursor = self.begin_particle_integration(current_frame);
        while !matches!(
            self.step_particle_integration(&mut cursor),
            ParticleIntegrationStep::Complete
        ) {}
        self.finish_system_lifetime();
    }

    pub(in crate::effects) fn finish_system_lifetime(&mut self) {
        // C++2057 returns after the final destroyed particle dies, before
        // the finite system-lifetime phase at2064.
        if self.is_destroyed && self.particles.is_empty() {
            return;
        }
        if !self.is_forever && self.system_lifetime_left > 0 {
            self.system_lifetime_left -= 1;
        }
    }

    pub(in crate::effects) fn begin_particle_integration(
        &self,
        current_frame: u32,
    ) -> ParticleIntegrationCursor {
        ParticleIntegrationCursor {
            index: 0,
            current_frame,
            gravity: self.info().gravity,
            drift_velocity: self.info().drift_velocity,
            wind_angle: self.wind_angle(),
            wind_motion: self.info().wind_motion,
            shader_type: self.info().shader_type,
            system_pos: Vec3::from(self.position + self.local_translation),
        }
    }

    pub(in crate::effects) fn step_particle_integration(
        &mut self,
        cursor: &mut ParticleIntegrationCursor,
    ) -> ParticleIntegrationStep {
        let Some(particle) = self.particles.get_mut(cursor.index) else {
            return ParticleIntegrationStep::Complete;
        };
        if cursor.gravity != 0.0 {
            particle.apply_force(Vec3::new(0.0, 0.0, cursor.gravity));
        }
        if !particle.update(
            cursor.drift_velocity,
            cursor.current_frame,
            cursor.shader_type,
        ) {
            return ParticleIntegrationStep::Removed(
                self.particles
                    .remove(cursor.index)
                    .expect("updated particle exists"),
            );
        }
        if cursor.wind_motion != WindMotion::NotUsed {
            particle.do_wind_motion(cursor.wind_angle, cursor.system_pos);
        }
        cursor.index += 1;
        ParticleIntegrationStep::Alive
    }
}

#[cfg(test)]
#[path = "integration_tests.rs"]
mod tests;
