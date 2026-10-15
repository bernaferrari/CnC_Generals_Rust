//! Receiving particle admission and destructor effects, without copied particle data.
use super::*;
use crate::effects::particle_system::Particle;

impl ParticleSystemManager {
    /// Admit one owned particle in actual creation order. This is the only
    /// insertion boundary for a particle belonging to this receiver.
    pub fn admit_particle(
        &mut self,
        system_id: ParticleSystemId,
        particle: Particle,
    ) -> Result<u64, ParticleSystemError> {
        let next = self.next_particle_ordinal.checked_add(1).ok_or_else(|| {
            ParticleSystemError::InitializationFailed("particle admission ordinal exhausted".into())
        })?;
        let system = self
            .active_systems
            .get_mut(&system_id)
            .ok_or(ParticleSystemError::InvalidSystemId(system_id))?;
        let ordinal = self.next_particle_ordinal;
        system.push_admitted_particle(particle, ordinal);
        self.next_particle_ordinal = next;
        self.particle_count += 1;
        Ok(ordinal)
    }

    pub(super) fn detach_and_destroy_controlled_system(&mut self, controlled: ParticleSystemId) {
        if let Some(child) = self.active_systems.get_mut(&controlled) {
            child.clear_control_particle_position();
        }
        self.destroy_particle_system(controlled);
    }

    pub(super) fn remove_admitted_particle(&mut self, id: ParticleSystemId, index: usize) {
        let removed = self
            .active_systems
            .get_mut(&id)
            .expect("eviction system exists")
            .remove_admitted_particle(index);
        self.finish_removed_particle(removed);
    }

    pub(super) fn finish_removed_particle(&mut self, removed: Particle) {
        if let Some(controlled) = removed.controlled_system {
            self.detach_and_destroy_controlled_system(controlled);
        }
        self.particle_count -= 1;
        // The last rendered field observation changes only at presentation/reset.
    }
}

#[cfg(test)]
#[path = "eviction_tests.rs"]
mod tests;
