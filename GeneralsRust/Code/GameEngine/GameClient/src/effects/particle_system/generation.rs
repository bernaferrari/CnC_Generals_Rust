//! One particle-generation algorithm with an explicitly borrowed sampling source.
use super::random::ParticleRandomSource;
use super::*;

impl ParticleSystem {
    /// Generate particle info (matches C++ ParticleSystem::generateParticleInfo)
    /// pub(crate) so the manager can call it for slave merge processing.
    pub(crate) fn generate_particle_info(
        &mut self,
        particle_num: u32,
        particle_count: u32,
    ) -> Option<ParticleInfo> {
        // One pure generation phase borrows no callback services. Commit the
        // locally advanced stream before any emission is admitted by the owner.
        let mut random = self.random_state.clone();
        let result =
            self.generate_particle_info_with_random(particle_num, particle_count, &mut random);
        self.random_state = random;
        result
    }

    pub(in crate::effects) fn generate_particle_info_with_random(
        &mut self,
        particle_num: u32,
        particle_count: u32,
        random: &mut dyn ParticleRandomSource,
    ) -> Option<ParticleInfo> {
        let info = &self.info;

        let mut particle_info = ParticleInfo::default();

        // C++ computeParticlePosition/Velocity stay in local space; transform is
        // applied below only when m_isIdentity is false (ParticleSys.cpp:1724-1771).
        particle_info.position = self.compute_particle_position_with_random(random);
        particle_info.emitter_position = Vec3::from(self.transform_translation);

        particle_info.velocity =
            self.compute_particle_velocity_with_random(&particle_info.position, random);
        let vel_scale = 0.5 + self.particle_scale / 2.0;
        particle_info.velocity *= self.vel_coeff;
        particle_info.velocity *= vel_scale;

        if !self.is_identity {
            if self.is_first_pos {
                self.last_position = self.position;
                self.is_first_pos = false;
            }
            let last = self.last_position;
            let denom = particle_count.max(1) as f32;
            let spread = 1.0 - (particle_num as f32 / denom);
            let adjustment = (self.position - last) * spread;
            let world = self.transform * particle_info.position + self.transform_translation;
            particle_info.position = Vec3::from(world) - adjustment;
            particle_info.velocity = self.transform * particle_info.velocity;
        }
        particle_info.vel_damping = info.vel_damping.sample_with(|lo, hi| random.real(lo, hi));
        particle_info.angular_damping = info
            .angular_damping
            .sample_with(|lo, hi| random.real(lo, hi));
        particle_info.angle_z = info.angle_z.sample_with(|lo, hi| random.real(lo, hi));
        particle_info.angular_rate_z = info
            .angular_rate_z
            .sample_with(|lo, hi| random.real(lo, hi));
        particle_info.lifetime = info.lifetime.sample_with(|lo, hi| random.real(lo, hi)) as u32;
        particle_info.size = info.start_size.sample_with(|lo, hi| random.real(lo, hi))
            * self.size_coeff
            * self.particle_scale;
        particle_info.size_rate = info.size_rate.sample_with(|lo, hi| random.real(lo, hi))
            * self.size_coeff
            * self.particle_scale;
        particle_info.size_rate_damping = info
            .size_rate_damping
            .sample_with(|lo, hi| random.real(lo, hi));
        particle_info.size += self.accumulated_size_bonus;
        self.accumulated_size_bonus += info
            .start_size_rate
            .sample_with(|lo, hi| random.real(lo, hi));
        self.accumulated_size_bonus = self.accumulated_size_bonus.min(50.0);

        // Copy keyframes (sample random alpha keyframes into fixed runtime values).
        for (dst, src) in particle_info
            .alpha_keys
            .iter_mut()
            .zip(info.alpha_keys.iter())
        {
            dst.value = GameClientRandomVariable {
                min: src.min_value,
                max: src.max_value,
                distribution_type: src.distribution_type,
            }
            .sample_with(|lo, hi| random.real(lo, hi));
            dst.frame = src.frame;
        }
        particle_info.color_keys = info.color_keys;

        // Runtime info, including native-loaded info, is already normalized.
        particle_info.color_scale = info.color_scale.sample_with(|lo, hi| random.real(lo, hi));

        // Wind randomness — C++ ParticleSys.cpp:1817 (client stream)
        particle_info.wind_randomness = random.real(0.7, 1.3);

        // Particle up towards emitter
        particle_info.particle_up_towards_emitter = info.is_particle_up_towards_emitter;

        Some(particle_info)
    }

    /// Compute particle position based on emission volume (matches C++ ParticleSystem::computeParticlePosition)
    #[cfg(test)]
    pub(super) fn compute_particle_position(&self) -> Vec3 {
        self.compute_particle_position_with_random(&mut self.random_state.clone())
    }

    fn compute_particle_position_with_random(&self, random: &mut dyn ParticleRandomSource) -> Vec3 {
        let info = &self.info;

        let emission_volume = self.effective_emission_volume();
        let local_pos = match emission_volume {
            EmissionVolume::Point => Vec3::ZERO,

            EmissionVolume::Line { start, end } => {
                // C++ ParticleSys.cpp:1629 — t from the client stream.
                let t = random.real(0.0, 1.0);
                start + (end - start) * t
            }

            EmissionVolume::Box { half_size } => {
                if info.is_emission_volume_hollow {
                    // Match C++ bug exactly: side % 3 == 1 uses halfSize.y for X
                    // (C++ ParticleSys.cpp:1579-1598; the side draw is
                    // GameClientRandomValue(0, 6) — seven outcomes).
                    let side = random.integer(0, 6);
                    if side % 3 == 0 {
                        // Bottom or top face (Z = -/+halfSize.z)
                        Vec3::new(
                            random.real(-half_size.x, half_size.x),
                            random.real(-half_size.y, half_size.y),
                            if side == 0 { -half_size.z } else { half_size.z },
                        )
                    } else if side % 3 == 1 {
                        // Left or right face (X = -/+halfSize.x)
                        // C++ bug: uses halfSize.y instead of halfSize.x for X coordinate
                        Vec3::new(
                            if side == 1 { -half_size.x } else { half_size.y },
                            random.real(-half_size.y, half_size.y),
                            random.real(-half_size.z, half_size.z),
                        )
                    } else {
                        // Front or back face (Y = -/+halfSize.y)
                        Vec3::new(
                            random.real(-half_size.x, half_size.x),
                            if side == 2 { -half_size.y } else { half_size.y },
                            random.real(-half_size.z, half_size.z),
                        )
                    }
                } else {
                    Vec3::new(
                        random.real(-half_size.x, half_size.x),
                        random.real(-half_size.y, half_size.y),
                        random.real(-half_size.z, half_size.z),
                    )
                }
            }

            EmissionVolume::Sphere { radius } => {
                // C++ ParticleSys.cpp:1553-1568 — radius draw first, then the
                // unit-sphere draws.
                let r = if info.is_emission_volume_hollow {
                    radius
                } else {
                    random.real(0.0, radius)
                };
                generals_particles::point_on_unit_sphere(|lo, hi| random.real(lo, hi)) * r
            }

            EmissionVolume::Cylinder { radius, length } => {
                // C++ ParticleSys.cpp:1534-1550 — angle, radius, then z.
                let half_length = length * 0.5;
                let theta = random.real(0.0, 2.0 * std::f32::consts::PI);
                let r = if info.is_emission_volume_hollow {
                    radius
                } else {
                    random.real(0.0, radius)
                };
                let z = random.real(-half_length, half_length);
                Vec3::new(r * theta.cos(), r * theta.sin(), z)
            }
        };

        // C++ parity: ParticleSys.cpp lines 1644-1646
        // C++: newPos *= (0.5f + m_particleScale/2.0f)
        let pos_scale = 0.5 + self.particle_scale / 2.0;
        Vec3::from(local_pos * pos_scale)
    }

    pub(super) fn effective_emission_volume(&self) -> EmissionVolume {
        self.emission_volume_override
            .unwrap_or(self.info().emission_volume)
    }

    /// Compute particle velocity based on emission properties (matches C++ ParticleSystem::computeParticleVelocity)
    #[cfg(test)]
    pub(super) fn compute_particle_velocity(&self, position: &Vec3) -> Vec3 {
        self.compute_particle_velocity_with_random(position, &mut self.random_state.clone())
    }

    fn compute_particle_velocity_with_random(
        &self,
        position: &Vec3,
        random: &mut dyn ParticleRandomSource,
    ) -> Vec3 {
        let info = &self.info;

        match info.emission_velocity {
            EmissionVelocity::Ortho { x, y, z } => Vec3::new(
                x.sample_with(|lo, hi| random.real(lo, hi)),
                y.sample_with(|lo, hi| random.real(lo, hi)),
                z.sample_with(|lo, hi| random.real(lo, hi)),
            ),

            EmissionVelocity::Spherical { speed } => {
                speed.sample_with(|lo, hi| random.real(lo, hi))
                    * generals_particles::point_on_unit_sphere(|lo, hi| random.real(lo, hi))
            }

            EmissionVelocity::Hemispherical { speed } => {
                speed.sample_with(|lo, hi| random.real(lo, hi))
                    * generals_particles::point_on_unit_hemisphere(|lo, hi| random.real(lo, hi))
            }

            EmissionVelocity::Cylindrical { radial, normal } => {
                // C++ ParticleSys.cpp:1386-1392 — radial, angle, then normal.
                let radial_speed = radial.sample_with(|lo, hi| random.real(lo, hi));
                let theta = random.real(0.0, 2.0 * std::f32::consts::PI);
                let normal_speed = normal.sample_with(|lo, hi| random.real(lo, hi));
                Vec3::new(
                    radial_speed * theta.cos(),
                    radial_speed * theta.sin(),
                    normal_speed,
                )
            }

            EmissionVelocity::Outward { speed, other_speed } => {
                let speed_val = speed.sample_with(|lo, hi| random.real(lo, hi));
                let other_speed_val = other_speed.sample_with(|lo, hi| random.real(lo, hi));

                match info.emission_volume_type {
                    EmissionVolumeType::Invalid => Vec3::ZERO,
                    EmissionVolumeType::Cylinder => {
                        let dx = position.x;
                        let dy = position.y;
                        let len = (dx * dx + dy * dy).sqrt();
                        if len > 0.0 {
                            Vec3::new(speed_val * dx / len, speed_val * dy / len, other_speed_val)
                        } else {
                            Vec3::new(speed_val, 0.0, other_speed_val)
                        }
                    }
                    EmissionVolumeType::Box | EmissionVolumeType::Sphere => {
                        let dir = position;
                        let len = dir.length();
                        if len > 0.0 {
                            dir * (speed_val / len)
                        } else {
                            speed_val
                                * generals_particles::point_on_unit_sphere(|lo, hi| {
                                    random.real(lo, hi)
                                })
                        }
                    }
                    EmissionVolumeType::Line => {
                        let vol = self.effective_emission_volume();
                        if let EmissionVolume::Line { start, end } = vol {
                            let along = (end - start).normalize();
                            let up = Vec3::new(0.0, 0.0, 1.0);
                            let perp = up.cross(along).normalize();
                            let new_up = along.cross(perp);
                            speed_val * perp + other_speed_val * new_up
                        } else {
                            Vec3::new(0.0, 0.0, other_speed_val)
                        }
                    }
                    EmissionVolumeType::Point => {
                        speed_val
                            * generals_particles::point_on_unit_sphere(|lo, hi| random.real(lo, hi))
                    }
                }
            }
        }
    }
}
