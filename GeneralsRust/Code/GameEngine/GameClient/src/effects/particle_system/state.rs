//! Original ordered ParticleSystem field stream and native-load side effects.
use super::random::ParticleRandomSource;
use super::*;

impl ParticleSystem {
    pub(in crate::effects) fn xfer_with_particle_random(
        &mut self,
        xfer: &mut dyn Xfer,
        random: &mut dyn ParticleRandomSource,
    ) -> Result<(), String> {
        const CURRENT_VERSION: XferVersion = 1;
        let mut version = CURRENT_VERSION;
        xfer.xfer_version(&mut version, CURRENT_VERSION)
            .map_err(|e| e.to_string())?;

        let mut system_info = self.info().clone();
        system_info.xfer(xfer)?;
        if xfer.get_xfer_mode() == XferMode::Load {
            self.info.clone_from(&system_info);
        }

        xfer.xfer_unsigned_int(&mut self.system_id)
            .map_err(|e| e.to_string())?;

        let mut attached_drawable_id = self.attached_drawable_id.0;
        xfer.xfer_drawable_id(&mut attached_drawable_id)
            .map_err(|e| e.to_string())?;
        self.attached_drawable_id = DrawableId(attached_drawable_id);

        xfer.xfer_object_id(&mut self.attached_object_id)
            .map_err(|e| e.to_string())?;
        xfer.xfer_bool(&mut self.is_local_identity)
            .map_err(|e| e.to_string())?;
        xfer_matrix3(xfer, &mut self.local_transform, &mut self.local_translation)?;
        xfer.xfer_bool(&mut self.is_identity)
            .map_err(|e| e.to_string())?;
        xfer_matrix3(xfer, &mut self.transform, &mut self.transform_translation)?;
        xfer.xfer_unsigned_int(&mut self.burst_delay_left)
            .map_err(|e| e.to_string())?;
        xfer.xfer_unsigned_int(&mut self.delay_left)
            .map_err(|e| e.to_string())?;
        xfer.xfer_unsigned_int(&mut self.start_timestamp)
            .map_err(|e| e.to_string())?;
        xfer.xfer_unsigned_int(&mut self.system_lifetime_left)
            .map_err(|e| e.to_string())?;
        xfer.xfer_unsigned_int(&mut self.personality_counter)
            .map_err(|e| e.to_string())?;
        xfer.xfer_bool(&mut self.is_forever)
            .map_err(|e| e.to_string())?;
        xfer.xfer_real(&mut self.accumulated_size_bonus)
            .map_err(|e| e.to_string())?;
        xfer.xfer_bool(&mut self.is_stopped)
            .map_err(|e| e.to_string())?;
        xfer_vec3(xfer, &mut self.vel_coeff)?;
        xfer.xfer_real(&mut self.count_coeff)
            .map_err(|e| e.to_string())?;
        xfer.xfer_real(&mut self.delay_coeff)
            .map_err(|e| e.to_string())?;
        xfer.xfer_real(&mut self.size_coeff)
            .map_err(|e| e.to_string())?;
        xfer_point3(xfer, &mut self.position)?;
        xfer_point3(xfer, &mut self.last_position)?;
        xfer.xfer_bool(&mut self.is_first_pos)
            .map_err(|e| e.to_string())?;

        let mut slave_system = self.slave_system.unwrap_or(INVALID_PARTICLE_SYSTEM_ID);
        let mut master_system = self.master_system.unwrap_or(INVALID_PARTICLE_SYSTEM_ID);
        xfer.xfer_unsigned_int(&mut slave_system)
            .map_err(|e| e.to_string())?;
        xfer.xfer_unsigned_int(&mut master_system)
            .map_err(|e| e.to_string())?;
        self.slave_system = if slave_system == INVALID_PARTICLE_SYSTEM_ID {
            None
        } else {
            Some(slave_system)
        };
        self.master_system = if master_system == INVALID_PARTICLE_SYSTEM_ID {
            None
        } else {
            Some(master_system)
        };

        let mut particle_count = self.particles.len() as u32;
        xfer.xfer_unsigned_int(&mut particle_count)
            .map_err(|e| e.to_string())?;

        if xfer.get_xfer_mode() == XferMode::Load {
            self.particles.clear();
            // C++2488 generates once, even for an empty saved system. This
            // advances first-position and StartSizeRate before force creation.
            let generated = self
                .generate_particle_info_with_random(0, 1, random)
                .expect("one-particle generation always produces attributes");
            for _ in 0..particle_count {
                let particle = Particle::new(&generated, self.personality_counter, 0);
                // Force creation advances the unsigned personality store; the
                // following Xfer overwrites the particle's saved attributes.
                self.push_particle_inner(particle);
                self.particles
                    .back_mut()
                    .expect("force-created particle exists")
                    .xfer(xfer)?;
            }
        } else {
            for particle in &mut self.particles {
                particle.xfer(xfer)?;
            }
        }

        Ok(())
    }
}
