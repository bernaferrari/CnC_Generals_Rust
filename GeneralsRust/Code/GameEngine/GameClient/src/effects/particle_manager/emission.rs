//! Immediate slave admission at the successful master-particle boundary.
use super::*;

impl ParticleSystemManager {
    /// Particle destruction updates the manager budget before later emitters.
    pub(super) fn integrate_particle_system(&mut self, id: ParticleSystemId, frame: u32) {
        let system = self
            .active_systems
            .get_mut(&id)
            .expect("integrating particle system remains admitted");
        let before = system.particle_count();
        system.finish_frame_integrate(frame);
        let removed = before - system.particle_count();
        let dead = system.take_dead_controlled_systems();
        self.particle_count = self.particle_count.saturating_sub(removed);
        for id in dead {
            if let Some(child) = self.active_systems.get_mut(&id) {
                child.clear_control_particle_position();
            }
            self.destroy_particle_system(id);
        }
    }

    /// C++ destructor detaches links; explicit destroy's cascade is separate.
    pub(super) fn retire_particle_system(&mut self, id: ParticleSystemId) {
        let retired = self
            .remove_active_system(id)
            .expect("retiring particle system is registered");
        if let Some(slave_id) = retired.slave_system_id() {
            if let Some(slave) = self.active_systems.get_mut(&slave_id) {
                debug_assert_eq!(slave.master_system_id(), Some(id));
                if slave.master_system_id() == Some(id) {
                    slave.set_master(None);
                }
            }
        }
        if let Some(master_id) = retired.master_system_id() {
            if let Some(master) = self.active_systems.get_mut(&master_id) {
                debug_assert_eq!(master.slave_system_id(), Some(id));
                if master.slave_system_id() == Some(id) {
                    master.set_slave(None);
                }
            }
        }
        for system in self.active_systems.values_mut() {
            for particle in system.particles_mut() {
                if particle.controlled_system == Some(id) {
                    particle.controlled_system = None;
                }
            }
        }
        self.system_count = self.active_systems.len();
    }

    /// Release the parent's particle borrow before updating controlled emitters.
    pub(super) fn refresh_controlled_particle_origins(&mut self, parent_id: ParticleSystemId) {
        let pairs: Vec<_> = self
            .active_systems
            .get(&parent_id)
            .into_iter()
            .flat_map(|system| system.particles().iter())
            .filter_map(|particle| particle.controlled_system.map(|id| (id, particle.position)))
            .collect();
        for (id, position) in pairs {
            if let Some(system) = self.active_systems.get_mut(&id) {
                system.set_control_particle_position(position);
            }
        }
    }

    pub(super) fn admit_slave_for_master(
        &mut self,
        master_id: ParticleSystemId,
        priority: ParticlePriorityType,
        current_frame: u32,
    ) {
        let Some(slave_id) = self
            .active_systems
            .get(&master_id)
            .and_then(|s| s.slave_system_id())
        else {
            return;
        };
        if !self.active_systems.contains_key(&slave_id) {
            return;
        }
        // C++ generates a fresh master observation, then the slave observation.
        // Each borrow ends before another object or the admission policy is accessed.
        let Some(master_info) = self
            .active_systems
            .get_mut(&master_id)
            .and_then(|s| s.generate_particle_info(1, 1))
        else {
            return;
        };
        let Some((slave_info, offset, ground_aligned, count)) =
            self.active_systems.get_mut(&slave_id).and_then(|s| {
                s.generate_particle_info(1, 1).map(|info| {
                    (
                        info,
                        s.slave_position_offset(),
                        s.info().is_ground_aligned,
                        s.particle_count(),
                    )
                })
            })
        else {
            return;
        };
        let info = crate::effects::particle_system::merge_related_particle_infos(
            master_info,
            slave_info,
            offset,
        );
        // C++ passes the MASTER priority to slave createParticle (2009).
        if !self.can_create_particle_for_system(priority, ground_aligned, count, false) {
            return;
        }
        if let Some(slave) = self.active_systems.get_mut(&slave_id) {
            let particle = crate::effects::particle_system::Particle::new(
                &info,
                slave.personality_counter(),
                current_frame,
            );
            slave.push_particle(particle);
            self.particle_count += 1;
        }
    }
}

#[cfg(test)]
#[path = "emission_tests.rs"]
mod tests;
