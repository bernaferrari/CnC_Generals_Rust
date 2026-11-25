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
    /// Capture exactly the domain the native system xfer would persist, but as
    /// typed archive data. Runtime-only native reset fields are deliberately absent.
    pub(in crate::effects) fn capture_archive(
        &self,
    ) -> crate::effects::particle_archive::ParticleSystemArchive {
        crate::effects::particle_archive::ParticleSystemArchive {
            template_name: self.template().name().to_string(),
            info: crate::effects::particle_archive::canonical_info_for_archive(self.info()),
            system_id: self.system_id,
            random_seed: self.random_state.seed_words(),
            attached_drawable_id: self.attached_drawable_id.0,
            attached_object_id: self.attached_object_id,
            is_local_identity: self.is_local_identity,
            local_transform: self.local_transform,
            local_translation: self.local_translation,
            is_identity: self.is_identity,
            transform: self.transform,
            transform_translation: self.transform_translation,
            burst_delay_left: self.burst_delay_left,
            delay_left: self.delay_left,
            start_timestamp: self.start_timestamp,
            system_lifetime_left: self.system_lifetime_left,
            personality_counter: self.personality_counter,
            is_forever: self.is_forever,
            accumulated_size_bonus: self.accumulated_size_bonus,
            is_stopped: self.is_stopped,
            vel_coeff: self.vel_coeff,
            count_coeff: self.count_coeff,
            delay_coeff: self.delay_coeff,
            size_coeff: self.size_coeff,
            position: self.position,
            last_position: self.last_position,
            is_first_pos: self.is_first_pos,
            slave_system: self.slave_system,
            master_system: self.master_system,
            particles: self.particles.iter().map(capture_particle_archive).collect(),
        }
    }

    /// Reconstruct a detached system with the same constructor, generated-prototype,
    /// and force-creation side effects as the native xfer load. The borrowed random
    /// stream is the receiving manager's staged stream; the system's own final random
    /// state comes from the archive capsule only after reconstruction succeeds.
    pub(in crate::effects) fn restore_archive(
        template: Arc<ParticleSystemTemplate>,
        archive: &crate::effects::particle_archive::ParticleSystemArchive,
        random: &mut dyn ParticleRandomSource,
    ) -> Result<Self, String> {
        crate::effects::particle_archive::validate_archive_info(&archive.info)?;
        if archive.system_id == INVALID_PARTICLE_SYSTEM_ID
            || archive.slave_system == Some(INVALID_PARTICLE_SYSTEM_ID)
            || archive.master_system == Some(INVALID_PARTICLE_SYSTEM_ID)
        {
            return Err("invalid particle system id in typed archive".into());
        }
        let mut system = Self::new_with_random(template, 1, false, &mut |lo, hi| {
            random.real(lo, hi)
        });
        let template_volume_particle_depth = system.info.volume_particle_depth;
        let template_emission_velocity = system.info.emission_velocity;
        let mut info = archive.info.clone();
        info.volume_particle_depth = template_volume_particle_depth;
        if info.emission_velocity_type == EmissionVelocityType::Invalid {
            info.emission_velocity = template_emission_velocity;
        }
        if matches!(
            info.emission_volume_type,
            EmissionVolumeType::Invalid | EmissionVolumeType::Point
        ) {
            info.emission_volume = EmissionVolume::Point;
        }
        system.info = info;

        system.system_id = archive.system_id;
        system.attached_drawable_id = DrawableId(archive.attached_drawable_id);
        system.attached_object_id = archive.attached_object_id;
        system.is_local_identity = archive.is_local_identity;
        system.local_transform = archive.local_transform;
        system.local_translation = archive.local_translation;
        system.is_identity = archive.is_identity;
        system.transform = archive.transform;
        system.transform_translation = archive.transform_translation;
        system.burst_delay_left = archive.burst_delay_left;
        system.delay_left = archive.delay_left;
        system.start_timestamp = archive.start_timestamp;
        system.system_lifetime_left = archive.system_lifetime_left;
        system.personality_counter = archive.personality_counter;
        system.is_forever = archive.is_forever;
        system.accumulated_size_bonus = archive.accumulated_size_bonus;
        system.is_stopped = archive.is_stopped;
        system.vel_coeff = archive.vel_coeff;
        system.count_coeff = archive.count_coeff;
        system.delay_coeff = archive.delay_coeff;
        system.size_coeff = archive.size_coeff;
        system.position = archive.position;
        system.last_position = archive.last_position;
        system.is_first_pos = archive.is_first_pos;
        system.slave_system = archive.slave_system;
        system.master_system = archive.master_system;

        system.particles.clear();
        // Preserve C++2488: generate once even for an empty saved list. This
        // deliberately mutates first-position state and StartSizeRate accumulation.
        let generated = system
            .generate_particle_info_with_random(0, 1, random)
            .expect("one-particle generation always produces attributes");
        for particle_archive in &archive.particles {
            let particle = Particle::new(&generated, system.personality_counter, 0);
            system.push_particle_inner(particle);
            let particle = system
                .particles
                .back_mut()
                .expect("force-created particle exists");
            apply_particle_archive(particle, particle_archive)?;
        }
        Snapshotable::load_post_process(&mut system)?;
        system.random_state.set_seed_words(archive.random_seed);
        Ok(system)
    }

}

fn capture_particle_archive(
    particle: &Particle,
) -> crate::effects::particle_archive::ParticleArchive {
    crate::effects::particle_archive::ParticleArchive {
        info: ParticleInfo {
            velocity: particle.velocity,
            position: particle.position,
            emitter_position: particle.emitter_position,
            vel_damping: particle.vel_damping,
            angle_z: particle.angle_z,
            angular_rate_z: particle.angular_rate_z,
            angular_damping: particle.angular_damping,
            lifetime: particle.lifetime,
            size: particle.size,
            size_rate: particle.size_rate,
            size_rate_damping: particle.size_rate_damping,
            alpha_keys: particle.alpha_keys,
            color_keys: particle.color_keys,
            color_scale: particle.color_scale,
            wind_randomness: particle.wind_randomness,
            particle_up_towards_emitter: particle.particle_up_towards_emitter,
        },
        personality: particle.personality,
        acceleration: particle.acceleration,
        last_position: particle.last_position,
        lifetime_left: particle.lifetime_left,
        create_timestamp: particle.create_timestamp,
        alpha: particle.alpha,
        alpha_rate: particle.alpha_rate,
        alpha_target_key: particle.alpha_target_key as u32,
        color: particle.color,
        color_rate: particle.color_rate,
        color_target_key: particle.color_target_key as u32,
        controlled_system: particle.controlled_system,
    }
}

fn apply_particle_archive(
    particle: &mut Particle,
    archive: &crate::effects::particle_archive::ParticleArchive,
) -> Result<(), String> {
    if archive.alpha_target_key as usize > MAX_KEYFRAMES
        || archive.color_target_key as usize > MAX_KEYFRAMES
    {
        return Err("invalid particle target key".into());
    }
    if archive.controlled_system == Some(INVALID_PARTICLE_SYSTEM_ID) {
        return Err("invalid controlled particle system id".into());
    }
    particle.velocity = archive.info.velocity;
    particle.position = archive.info.position;
    particle.emitter_position = archive.info.emitter_position;
    particle.vel_damping = archive.info.vel_damping;
    particle.angle_z = archive.info.angle_z;
    particle.angular_rate_z = archive.info.angular_rate_z;
    // angular_damping is intentionally absent from the native continuation:
    // the generated prototype's value remains, exactly as in the xfer load.
    particle.lifetime = archive.info.lifetime;
    particle.size = archive.info.size;
    particle.size_rate = archive.info.size_rate;
    particle.size_rate_damping = archive.info.size_rate_damping;
    particle.alpha_keys = archive.info.alpha_keys;
    particle.color_keys = archive.info.color_keys;
    particle.color_scale = archive.info.color_scale;
    particle.wind_randomness = archive.info.wind_randomness;
    particle.particle_up_towards_emitter = archive.info.particle_up_towards_emitter;

    particle.personality = archive.personality;
    particle.acceleration = archive.acceleration;
    particle.last_position = archive.last_position;
    particle.lifetime_left = archive.lifetime_left;
    particle.create_timestamp = archive.create_timestamp;
    particle.alpha = archive.alpha;
    particle.alpha_rate = archive.alpha_rate;
    particle.alpha_target_key = archive.alpha_target_key as usize;
    particle.color = archive.color;
    particle.color_rate = archive.color_rate;
    particle.color_target_key = archive.color_target_key as usize;
    particle.controlled_system = archive.controlled_system;
    particle.system_next = None;
    particle.system_prev = None;
    particle.overall_next = None;
    particle.overall_prev = None;
    particle.in_system_list = false;
    particle.in_overall_list = false;
    Ok(())
}
