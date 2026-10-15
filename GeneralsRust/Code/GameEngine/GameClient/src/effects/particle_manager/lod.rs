//! Particle admission observes one LOD owner and advances that owner's skip counter.
use super::*;
use game_engine::common::ini::{GameLODManager, ParticlePriorityType as IniPriority};

pub(super) trait ParticleLod {
    fn should_skip(&mut self, priority: ParticlePriorityType) -> bool;
}

fn priority_from_ini(priority: IniPriority) -> ParticlePriorityType {
    ParticlePriorityType::from_index(priority as usize)
        .unwrap_or(ParticlePriorityType::WeaponExplosion)
}

impl ParticleLod for GameLODManager {
    fn should_skip(&mut self, priority: ParticlePriorityType) -> bool {
        // ParticleSys.cpp:1680-1695: ALWAYS_RENDER and below-minimum particles
        // never advance the skip counter; only the conditional skip branch does.
        if priority == ParticlePriorityType::AlwaysRender {
            return false;
        }
        if priority < priority_from_ini(self.get_min_dynamic_particle_priority()) {
            return true;
        }
        priority < priority_from_ini(self.get_min_dynamic_particle_skip_priority())
            && self.is_particle_skipped()
    }
}

pub(super) struct AmbientParticleLod;
impl ParticleLod for AmbientParticleLod {
    fn should_skip(&mut self, priority: ParticlePriorityType) -> bool {
        if priority == ParticlePriorityType::AlwaysRender {
            return false;
        }
        let (minimum, skip_minimum) = {
            let lod = game_engine::common::ini::get_game_lod_manager();
            (
                priority_from_ini(lod.get_min_dynamic_particle_priority()),
                priority_from_ini(lod.get_min_dynamic_particle_skip_priority()),
            )
        };
        if priority < minimum {
            return true;
        }
        priority < skip_minimum && game_engine::common::ini::is_particle_skipped()
    }
}

impl ParticleSystemManager {
    /// C++ `ParticleSystem::createParticle` gates including field-particle cap.
    pub(super) fn can_create_particle_for_system_with_lod(
        &mut self,
        priority: ParticlePriorityType,
        is_ground_aligned: bool,
        system_particle_count: usize,
        force_create: bool,
        lod: &mut dyn ParticleLod,
    ) -> bool {
        if self.next_particle_ordinal.checked_add(1).is_none() {
            return false;
        }
        if force_create {
            return true;
        }
        let use_fx = game_engine::common::ini::ini_game_data::get_global_data()
            .map(|data| data.read().use_fx)
            .unwrap_or(true);
        if !use_fx {
            return false;
        }
        if lod.should_skip(priority) {
            return false;
        }
        // C++1685 rejects the last-rendered field cap before any eviction.
        if system_particle_count > 0
            && priority == ParticlePriorityType::AreaEffect
            && is_ground_aligned
            && self.field_particle_count > self.live_max_field_particle_count()
        {
            return false;
        }
        self.can_create_particle_after_lod(priority)
    }

    /// Check if we can create a particle with given priority (matches C++ createParticle logic)
    pub(super) fn can_create_particle_with_lod(
        &mut self,
        priority: ParticlePriorityType,
        lod: &mut dyn ParticleLod,
    ) -> bool {
        // Check LOD skip (C++ lines 1680-1683)
        if lod.should_skip(priority) {
            return false;
        }

        self.can_create_particle_after_lod(priority)
    }

    /// C++1694-1709: called only after the original LOD and field gates.
    fn can_create_particle_after_lod(&mut self, priority: ParticlePriorityType) -> bool {
        // ALWAYS_RENDER bypasses all limits (C++ lines 1694-1696)
        if priority == ParticlePriorityType::AlwaysRender {
            return true;
        }
        self.enforce_particle_limit(priority, self.live_max_particle_count())
    }

    /// Remaining non-ALWAYS_RENDER budget block, with its observed limit.
    fn enforce_particle_limit(
        &mut self,
        priority: ParticlePriorityType,
        max_particle_count: usize,
    ) -> bool {
        debug_assert_ne!(priority, ParticlePriorityType::AlwaysRender);
        // Check particle count limit (C++ ParticleSys.cpp:1699-1708).
        // C++ culls only when the count strictly EXCEEDS the cap, evicts the
        // excess, and then still rejects the NEW particle:
        // removeOldestParticles (ParticleSys.cpp:3177-3201) does
        // `while (count-- && ...)` on an UnsignedInt, so its post-decrement
        // wraps to 0xFFFFFFFF on loop exit and the function returns
        // requested + 1 — never equal to the requested count. The
        // `!= numInExcess` check therefore always fails, and the steady-state
        // population hovers between max and max+1 as each new particle that
        // pushes the count over the cap is itself rejected after the eviction.
        if self.particle_count > max_particle_count {
            let num_in_excess = self.particle_count - max_particle_count;
            self.remove_oldest_particles(num_in_excess, priority);
            // C++ removeOldestParticles returns requested+1 (see above), so
            // `!= numInExcess` holds and the new particle is never created.
            return false;
        }

        // C++1709 checks the zero limit only after excess eviction.
        max_particle_count != 0
    }
}

#[cfg(test)]
#[path = "lod_tests.rs"]
mod tests;
