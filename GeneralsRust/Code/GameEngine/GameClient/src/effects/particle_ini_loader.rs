//! # Particle System INI Loader
//!
//! Loads particle system definitions from INI files, matching the C++ parser exactly.
//! Supports all C++ particle system properties and parameters.

use nalgebra::{Point3, Vector3};
use std::collections::HashMap;
use std::sync::Arc;

use super::particle_manager::*;
use game_engine::common::ini::{INIError, INI};

/// Particle system INI field parser
pub struct ParticleSystemINIParser {
    /// Name mappings for enums (matches C++ exactly)
    shader_type_names: HashMap<String, ParticleShaderType>,
    particle_type_names: HashMap<String, ParticleType>,
    emission_velocity_names: HashMap<String, EmissionVelocityType>,
    emission_volume_names: HashMap<String, EmissionVolumeType>,
    priority_names: HashMap<String, ParticlePriorityType>,
    wind_motion_names: HashMap<String, WindMotion>,
}

impl Default for ParticleSystemINIParser {
    fn default() -> Self {
        let mut parser = Self {
            shader_type_names: HashMap::new(),
            particle_type_names: HashMap::new(),
            emission_velocity_names: HashMap::new(),
            emission_volume_names: HashMap::new(),
            priority_names: HashMap::new(),
            wind_motion_names: HashMap::new(),
        };

        // Initialize name mappings (matches C++ arrays exactly)
        parser.init_shader_type_names();
        parser.init_particle_type_names();
        parser.init_emission_velocity_names();
        parser.init_emission_volume_names();
        parser.init_priority_names();
        parser.init_wind_motion_names();

        parser
    }
}

impl ParticleSystemINIParser {
    /// Parse particle system definition from INI (matches C++ INI::parseParticleSystemDefinition)
    pub fn parse_particle_system_definition(
        &self,
        ini: &mut INI,
        manager: &mut ParticleSystemManager,
    ) -> Result<(), INIError> {
        // Get the particle system name
        let name = ini
            .get_current_token()
            .ok_or(INIError::UnexpectedEndOfFile)?;

        // Find existing template or create new one (matches C++ behavior)
        let template = if let Some(existing) = manager.find_template(&name) {
            existing
        } else {
            manager.new_template(name.clone())
        };

        // Parse all fields for this particle system into a mutable info struct
        let mut info = ParticleSystemInfo::default();
        self.parse_info_from_ini(ini, &mut info)?;

        // Apply parsed info to the template via mutable reference
        // Clone-then-replace pattern to avoid simultaneous borrows on the HashMap
        if let Some(arc_template) = manager.templates.get(&name).cloned() {
            let mut new_template = (*arc_template).clone();
            *new_template.info_mut() = info;
            manager.templates.insert(name, Arc::new(new_template));
        }

        Ok(())
    }

    /// Parse INI fields into a ParticleSystemInfo (matches C++ ParticleSystemTemplate::m_fieldParseTable)
    fn parse_info_from_ini(
        &self,
        ini: &mut INI,
        info: &mut ParticleSystemInfo,
    ) -> Result<(), INIError> {
        // Parse each field in the INI section (matches C++ field table order)
        while let Some(field_name) = ini.get_next_field() {
            match field_name.as_str() {
                "Priority" => {
                    let value = ini.get_field_value(&field_name)?;
                    info.priority = self.parse_priority(&value)?;
                }

                "IsOneShot" => {
                    let value = ini.get_field_value(&field_name)?;
                    info.is_one_shot = self.parse_bool(&value)?;
                }

                "Shader" => {
                    let value = ini.get_field_value(&field_name)?;
                    info.shader_type = self.parse_shader_type(&value)?;
                }

                "Type" => {
                    let value = ini.get_field_value(&field_name)?;
                    info.particle_type = self.parse_particle_type(&value)?;
                }

                "ParticleName" => {
                    let value = ini.get_field_value(&field_name)?;
                    info.particle_type_name = value;
                }

                "AngleZ" => {
                    let value = ini.get_field_value(&field_name)?;
                    info.angle_z = self.parse_random_variable(&value)?;
                }

                "AngularRateZ" => {
                    let value = ini.get_field_value(&field_name)?;
                    info.angular_rate_z = self.parse_random_variable(&value)?;
                }

                "AngularDamping" => {
                    let value = ini.get_field_value(&field_name)?;
                    info.angular_damping = self.parse_random_variable(&value)?;
                }

                "VelocityDamping" => {
                    let value = ini.get_field_value(&field_name)?;
                    info.vel_damping = self.parse_random_variable(&value)?;
                }

                "Gravity" => {
                    let value = ini.get_field_value(&field_name)?;
                    info.gravity = self.parse_float(&value)?;
                }

                "SlaveSystem" => {
                    let value = ini.get_field_value(&field_name)?;
                    info.slave_system_name = value;
                }

                "SlavePosOffset" => {
                    let value = ini.get_field_value(&field_name)?;
                    info.slave_pos_offset = self.parse_coord3d(&value)?;
                }

                "PerParticleAttachedSystem" => {
                    let value = ini.get_field_value(&field_name)?;
                    info.attached_system_name = value;
                }

                "Lifetime" => {
                    let value = ini.get_field_value(&field_name)?;
                    info.lifetime = self.parse_random_variable(&value)?;
                }

                "SystemLifetime" => {
                    let value = ini.get_field_value(&field_name)?;
                    info.system_lifetime = self.parse_uint(&value)?;
                }

                "Size" => {
                    let value = ini.get_field_value(&field_name)?;
                    info.start_size = self.parse_random_variable(&value)?;
                }

                "StartSizeRate" => {
                    let value = ini.get_field_value(&field_name)?;
                    info.start_size_rate = self.parse_random_variable(&value)?;
                }

                "SizeRate" => {
                    let value = ini.get_field_value(&field_name)?;
                    info.size_rate = self.parse_random_variable(&value)?;
                }

                "SizeRateDamping" => {
                    let value = ini.get_field_value(&field_name)?;
                    info.size_rate_damping = self.parse_random_variable(&value)?;
                }

                "Alpha1" | "Alpha2" | "Alpha3" | "Alpha4" | "Alpha5" | "Alpha6" | "Alpha7"
                | "Alpha8" => {
                    let index =
                        field_name.chars().last().unwrap().to_digit(10).unwrap() as usize - 1;
                    if index < MAX_KEYFRAMES {
                        let value = ini.get_field_value(&field_name)?;
                        info.alpha_keys[index] = self.parse_random_keyframe(&value)?;
                    }
                }

                "Color1" | "Color2" | "Color3" | "Color4" | "Color5" | "Color6" | "Color7"
                | "Color8" => {
                    let index =
                        field_name.chars().last().unwrap().to_digit(10).unwrap() as usize - 1;
                    if index < MAX_KEYFRAMES {
                        let value = ini.get_field_value(&field_name)?;
                        info.color_keys[index] = self.parse_rgb_color_keyframe(&value)?;
                    }
                }

                "ColorScale" => {
                    let value = ini.get_field_value(&field_name)?;
                    info.color_scale = self.parse_random_variable(&value)?;
                }

                "BurstDelay" => {
                    let value = ini.get_field_value(&field_name)?;
                    info.burst_delay = self.parse_random_variable(&value)?;
                }

                "BurstCount" => {
                    let value = ini.get_field_value(&field_name)?;
                    info.burst_count = self.parse_random_variable(&value)?;
                }

                "InitialDelay" => {
                    let value = ini.get_field_value(&field_name)?;
                    info.initial_delay = self.parse_random_variable(&value)?;
                }

                "DriftVelocity" => {
                    let value = ini.get_field_value(&field_name)?;
                    info.drift_velocity = self.parse_coord3d(&value)?;
                }

                "VelocityType" => {
                    let value = ini.get_field_value(&field_name)?;
                    info.emission_velocity_type = self.parse_emission_velocity_type(&value)?;
                }

                // Ortho velocity components
                "VelOrthoX" => {
                    let value = ini.get_field_value(&field_name)?;
                    let var = self.parse_random_variable(&value)?;
                    info.emission_velocity = match &info.emission_velocity {
                        EmissionVelocity::Ortho { y, z, .. } => EmissionVelocity::Ortho {
                            x: var,
                            y: *y,
                            z: *z,
                        },
                        _ => EmissionVelocity::Ortho {
                            x: var,
                            y: GameClientRandomVariable::default(),
                            z: GameClientRandomVariable::default(),
                        },
                    };
                }
                "VelOrthoY" => {
                    let value = ini.get_field_value(&field_name)?;
                    let var = self.parse_random_variable(&value)?;
                    info.emission_velocity = match &info.emission_velocity {
                        EmissionVelocity::Ortho { x, z, .. } => EmissionVelocity::Ortho {
                            x: *x,
                            y: var,
                            z: *z,
                        },
                        _ => EmissionVelocity::Ortho {
                            x: GameClientRandomVariable::default(),
                            y: var,
                            z: GameClientRandomVariable::default(),
                        },
                    };
                }
                "VelOrthoZ" => {
                    let value = ini.get_field_value(&field_name)?;
                    let var = self.parse_random_variable(&value)?;
                    info.emission_velocity = match &info.emission_velocity {
                        EmissionVelocity::Ortho { x, y, .. } => EmissionVelocity::Ortho {
                            x: *x,
                            y: *y,
                            z: var,
                        },
                        _ => EmissionVelocity::Ortho {
                            x: GameClientRandomVariable::default(),
                            y: GameClientRandomVariable::default(),
                            z: var,
                        },
                    };
                }

                "VelSpherical" => {
                    let value = ini.get_field_value(&field_name)?;
                    info.emission_velocity = EmissionVelocity::Spherical {
                        speed: self.parse_random_variable(&value)?,
                    };
                }

                "VelHemispherical" => {
                    let value = ini.get_field_value(&field_name)?;
                    info.emission_velocity = EmissionVelocity::Hemispherical {
                        speed: self.parse_random_variable(&value)?,
                    };
                }

                "VelCylindricalRadial" => {
                    let value = ini.get_field_value(&field_name)?;
                    let var = self.parse_random_variable(&value)?;
                    info.emission_velocity = match &info.emission_velocity {
                        EmissionVelocity::Cylindrical { normal, .. } => {
                            EmissionVelocity::Cylindrical {
                                radial: var,
                                normal: *normal,
                            }
                        }
                        _ => EmissionVelocity::Cylindrical {
                            radial: var,
                            normal: GameClientRandomVariable::default(),
                        },
                    };
                }
                "VelCylindricalNormal" => {
                    let value = ini.get_field_value(&field_name)?;
                    let var = self.parse_random_variable(&value)?;
                    info.emission_velocity = match &info.emission_velocity {
                        EmissionVelocity::Cylindrical { radial, .. } => {
                            EmissionVelocity::Cylindrical {
                                radial: *radial,
                                normal: var,
                            }
                        }
                        _ => EmissionVelocity::Cylindrical {
                            radial: GameClientRandomVariable::default(),
                            normal: var,
                        },
                    };
                }

                "VelOutward" => {
                    let value = ini.get_field_value(&field_name)?;
                    let var = self.parse_random_variable(&value)?;
                    info.emission_velocity = match &info.emission_velocity {
                        EmissionVelocity::Outward { other_speed, .. } => {
                            EmissionVelocity::Outward {
                                speed: var,
                                other_speed: *other_speed,
                            }
                        }
                        _ => EmissionVelocity::Outward {
                            speed: var,
                            other_speed: GameClientRandomVariable::default(),
                        },
                    };
                }
                "VelOutwardOther" => {
                    let value = ini.get_field_value(&field_name)?;
                    let var = self.parse_random_variable(&value)?;
                    info.emission_velocity = match &info.emission_velocity {
                        EmissionVelocity::Outward { speed, .. } => EmissionVelocity::Outward {
                            speed: *speed,
                            other_speed: var,
                        },
                        _ => EmissionVelocity::Outward {
                            speed: GameClientRandomVariable::default(),
                            other_speed: var,
                        },
                    };
                }

                "VolumeType" => {
                    let value = ini.get_field_value(&field_name)?;
                    info.emission_volume_type = self.parse_emission_volume_type(&value)?;
                    // Initialize volume based on type
                    info.emission_volume = match info.emission_volume_type {
                        EmissionVolumeType::Point => EmissionVolume::Point,
                        EmissionVolumeType::Line => EmissionVolume::Line {
                            start: Point3::origin(),
                            end: Point3::new(1.0, 0.0, 0.0),
                        },
                        EmissionVolumeType::Box => EmissionVolume::Box {
                            half_size: Vector3::new(1.0, 1.0, 1.0),
                        },
                        EmissionVolumeType::Sphere => EmissionVolume::Sphere { radius: 1.0 },
                        EmissionVolumeType::Cylinder => EmissionVolume::Cylinder {
                            radius: 1.0,
                            length: 2.0,
                        },
                    };
                }

                "VolLineStart" => {
                    let value = ini.get_field_value(&field_name)?;
                    let coord = self.parse_coord3d(&value)?;
                    if let EmissionVolume::Line { ref mut start, .. } = info.emission_volume {
                        *start = Point3::new(coord.x, coord.y, coord.z);
                    }
                }
                "VolLineEnd" => {
                    let value = ini.get_field_value(&field_name)?;
                    let coord = self.parse_coord3d(&value)?;
                    if let EmissionVolume::Line { ref mut end, .. } = info.emission_volume {
                        *end = Point3::new(coord.x, coord.y, coord.z);
                    }
                }

                "VolBoxHalfSize" => {
                    let value = ini.get_field_value(&field_name)?;
                    let coord = self.parse_coord3d(&value)?;
                    if let EmissionVolume::Box { ref mut half_size } = info.emission_volume {
                        *half_size = coord;
                    }
                }

                "VolSphereRadius" => {
                    let value = ini.get_field_value(&field_name)?;
                    if let EmissionVolume::Sphere { ref mut radius } = info.emission_volume {
                        *radius = self.parse_float(&value)?;
                    }
                }

                "VolCylinderRadius" => {
                    let value = ini.get_field_value(&field_name)?;
                    if let EmissionVolume::Cylinder { ref mut radius, .. } = info.emission_volume {
                        *radius = self.parse_float(&value)?;
                    }
                }
                "VolCylinderLength" => {
                    let value = ini.get_field_value(&field_name)?;
                    if let EmissionVolume::Cylinder { ref mut length, .. } = info.emission_volume {
                        *length = self.parse_float(&value)?;
                    }
                }

                "IsHollow" => {
                    let value = ini.get_field_value(&field_name)?;
                    info.is_emission_volume_hollow = self.parse_bool(&value)?;
                }

                "IsGroundAligned" => {
                    let value = ini.get_field_value(&field_name)?;
                    info.is_ground_aligned = self.parse_bool(&value)?;
                }

                "IsEmitAboveGroundOnly" => {
                    let value = ini.get_field_value(&field_name)?;
                    info.is_emit_above_ground_only = self.parse_bool(&value)?;
                }

                "IsParticleUpTowardsEmitter" => {
                    let value = ini.get_field_value(&field_name)?;
                    info.is_particle_up_towards_emitter = self.parse_bool(&value)?;
                }

                "WindMotion" => {
                    let value = ini.get_field_value(&field_name)?;
                    info.wind_motion = self.parse_wind_motion(&value)?;
                }

                "WindAngleChangeMin" => {
                    let value = ini.get_field_value(&field_name)?;
                    info.wind_angle_change_min = self.parse_float(&value)?;
                }
                "WindAngleChangeMax" => {
                    let value = ini.get_field_value(&field_name)?;
                    info.wind_angle_change_max = self.parse_float(&value)?;
                }

                "WindPingPongStartAngleMin" => {
                    let value = ini.get_field_value(&field_name)?;
                    info.wind_motion_start_angle_min = self.parse_float(&value)?;
                }
                "WindPingPongStartAngleMax" => {
                    let value = ini.get_field_value(&field_name)?;
                    info.wind_motion_start_angle_max = self.parse_float(&value)?;
                }

                "WindPingPongEndAngleMin" => {
                    let value = ini.get_field_value(&field_name)?;
                    info.wind_motion_end_angle_min = self.parse_float(&value)?;
                }
                "WindPingPongEndAngleMax" => {
                    let value = ini.get_field_value(&field_name)?;
                    info.wind_motion_end_angle_max = self.parse_float(&value)?;
                }

                _ => {
                    // Unknown field - log warning but continue (matches C++ behavior of skipping unknown)
                }
            }
        }

        Ok(())
    }

    // Parsing helper methods (match C++ parsers exactly)

    fn parse_bool(&self, value: &str) -> Result<bool, INIError> {
        match value.to_uppercase().as_str() {
            "TRUE" | "YES" | "1" => Ok(true),
            "FALSE" | "NO" | "0" => Ok(false),
            _ => Err(INIError::InvalidValue),
        }
    }

    fn parse_float(&self, value: &str) -> Result<f32, INIError> {
        value.parse::<f32>().map_err(|_| INIError::InvalidValue)
    }

    fn parse_uint(&self, value: &str) -> Result<u32, INIError> {
        value.parse::<u32>().map_err(|_| INIError::InvalidValue)
    }

    fn parse_coord3d(&self, value: &str) -> Result<Vector3<f32>, INIError> {
        let parts: Vec<&str> = value.split_whitespace().collect();
        if parts.len() != 3 {
            return Err(INIError::InvalidValue);
        }

        let x = parts[0]
            .parse::<f32>()
            .map_err(|_| INIError::InvalidValue)?;
        let y = parts[1]
            .parse::<f32>()
            .map_err(|_| INIError::InvalidValue)?;
        let z = parts[2]
            .parse::<f32>()
            .map_err(|_| INIError::InvalidValue)?;

        Ok(Vector3::new(x, y, z))
    }

    fn parse_random_variable(&self, value: &str) -> Result<GameClientRandomVariable, INIError> {
        let parts: Vec<&str> = value.split_whitespace().collect();
        match parts.len() {
            1 => {
                let val = self.parse_float(parts[0])?;
                Ok(GameClientRandomVariable::new(val, val))
            }
            2 => {
                let min = self.parse_float(parts[0])?;
                let max = self.parse_float(parts[1])?;
                Ok(GameClientRandomVariable::new(min, max))
            }
            _ => Err(INIError::InvalidValue),
        }
    }

    fn parse_random_keyframe(&self, value: &str) -> Result<RandomKeyframe, INIError> {
        let parts: Vec<&str> = value.split_whitespace().collect();
        if parts.len() != 3 {
            return Err(INIError::InvalidValue);
        }

        let min_value = self.parse_float(parts[0])?;
        let max_value = self.parse_float(parts[1])?;
        let frame = self.parse_uint(parts[2])?;

        Ok(RandomKeyframe {
            min_value,
            max_value,
            frame,
        })
    }

    fn parse_rgb_color_keyframe(&self, value: &str) -> Result<RGBColorKeyframe, INIError> {
        let parts: Vec<&str> = value.split_whitespace().collect();
        if parts.len() != 4 {
            return Err(INIError::InvalidValue);
        }

        let r = self.parse_float(parts[0])? / 255.0; // Convert from 0-255 to 0-1
        let g = self.parse_float(parts[1])? / 255.0;
        let b = self.parse_float(parts[2])? / 255.0;
        let frame = self.parse_uint(parts[3])?;

        Ok(RGBColorKeyframe {
            color: [r, g, b],
            frame,
        })
    }

    fn parse_shader_type(&self, value: &str) -> Result<ParticleShaderType, INIError> {
        self.shader_type_names
            .get(value)
            .copied()
            .ok_or_else(|| INIError::InvalidValue)
    }

    fn parse_particle_type(&self, value: &str) -> Result<ParticleType, INIError> {
        self.particle_type_names
            .get(value)
            .copied()
            .ok_or_else(|| INIError::InvalidValue)
    }

    fn parse_emission_velocity_type(&self, value: &str) -> Result<EmissionVelocityType, INIError> {
        self.emission_velocity_names
            .get(value)
            .copied()
            .ok_or_else(|| INIError::InvalidValue)
    }

    fn parse_emission_volume_type(&self, value: &str) -> Result<EmissionVolumeType, INIError> {
        self.emission_volume_names
            .get(value)
            .copied()
            .ok_or_else(|| INIError::InvalidValue)
    }

    fn parse_priority(&self, value: &str) -> Result<ParticlePriorityType, INIError> {
        self.priority_names
            .get(value)
            .copied()
            .ok_or_else(|| INIError::InvalidValue)
    }

    fn parse_wind_motion(&self, value: &str) -> Result<WindMotion, INIError> {
        self.wind_motion_names
            .get(value)
            .copied()
            .ok_or_else(|| INIError::InvalidValue)
    }

    // Initialize name mappings (matches C++ arrays exactly)

    fn init_shader_type_names(&mut self) {
        self.shader_type_names
            .insert("ADDITIVE".to_string(), ParticleShaderType::Additive);
        self.shader_type_names
            .insert("ALPHA".to_string(), ParticleShaderType::Alpha);
        self.shader_type_names
            .insert("ALPHA_TEST".to_string(), ParticleShaderType::AlphaTest);
        self.shader_type_names
            .insert("MULTIPLY".to_string(), ParticleShaderType::Multiply);
    }

    fn init_particle_type_names(&mut self) {
        self.particle_type_names
            .insert("PARTICLE".to_string(), ParticleType::Particle);
        self.particle_type_names
            .insert("DRAWABLE".to_string(), ParticleType::Drawable);
        self.particle_type_names
            .insert("STREAK".to_string(), ParticleType::Streak);
        self.particle_type_names
            .insert("VOLUME_PARTICLE".to_string(), ParticleType::VolumeParticle);
        self.particle_type_names
            .insert("SMUDGE".to_string(), ParticleType::Smudge);
    }

    fn init_emission_velocity_names(&mut self) {
        self.emission_velocity_names
            .insert("ORTHO".to_string(), EmissionVelocityType::Ortho);
        self.emission_velocity_names
            .insert("SPHERICAL".to_string(), EmissionVelocityType::Spherical);
        self.emission_velocity_names.insert(
            "HEMISPHERICAL".to_string(),
            EmissionVelocityType::Hemispherical,
        );
        self.emission_velocity_names
            .insert("CYLINDRICAL".to_string(), EmissionVelocityType::Cylindrical);
        self.emission_velocity_names
            .insert("OUTWARD".to_string(), EmissionVelocityType::Outward);
    }

    fn init_emission_volume_names(&mut self) {
        self.emission_volume_names
            .insert("POINT".to_string(), EmissionVolumeType::Point);
        self.emission_volume_names
            .insert("LINE".to_string(), EmissionVolumeType::Line);
        self.emission_volume_names
            .insert("BOX".to_string(), EmissionVolumeType::Box);
        self.emission_volume_names
            .insert("SPHERE".to_string(), EmissionVolumeType::Sphere);
        self.emission_volume_names
            .insert("CYLINDER".to_string(), EmissionVolumeType::Cylinder);
    }

    fn init_priority_names(&mut self) {
        self.priority_names.insert(
            "WEAPON_EXPLOSION".to_string(),
            ParticlePriorityType::WeaponExplosion,
        );
        self.priority_names
            .insert("SCORCHMARK".to_string(), ParticlePriorityType::ScorchMark);
        self.priority_names
            .insert("DUST_TRAIL".to_string(), ParticlePriorityType::DustTrail);
        self.priority_names
            .insert("BUILDUP".to_string(), ParticlePriorityType::Buildup);
        self.priority_names.insert(
            "DEBRIS_TRAIL".to_string(),
            ParticlePriorityType::DebrisTrail,
        );
        self.priority_names.insert(
            "UNIT_DAMAGE_FX".to_string(),
            ParticlePriorityType::UnitDamageFx,
        );
        self.priority_names.insert(
            "DEATH_EXPLOSION".to_string(),
            ParticlePriorityType::DeathExplosion,
        );
        self.priority_names.insert(
            "SEMI_CONSTANT".to_string(),
            ParticlePriorityType::SemiConstant,
        );
        self.priority_names
            .insert("CONSTANT".to_string(), ParticlePriorityType::Constant);
        self.priority_names.insert(
            "WEAPON_TRAIL".to_string(),
            ParticlePriorityType::WeaponTrail,
        );
        self.priority_names
            .insert("AREA_EFFECT".to_string(), ParticlePriorityType::AreaEffect);
        self.priority_names
            .insert("CRITICAL".to_string(), ParticlePriorityType::Critical);
        self.priority_names.insert(
            "ALWAYS_RENDER".to_string(),
            ParticlePriorityType::AlwaysRender,
        );
    }

    fn init_wind_motion_names(&mut self) {
        self.wind_motion_names
            .insert("Unused".to_string(), WindMotion::NotUsed);
        self.wind_motion_names
            .insert("PingPong".to_string(), WindMotion::PingPong);
        self.wind_motion_names
            .insert("Circular".to_string(), WindMotion::Circular);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parser_creation() {
        let parser = ParticleSystemINIParser::default();

        // Test name mappings are populated
        assert!(!parser.shader_type_names.is_empty());
        assert!(!parser.particle_type_names.is_empty());
        assert!(!parser.priority_names.is_empty());
    }

    #[test]
    fn test_basic_parsing() {
        let parser = ParticleSystemINIParser::default();

        // Test boolean parsing
        assert_eq!(parser.parse_bool("TRUE").unwrap(), true);
        assert_eq!(parser.parse_bool("false").unwrap(), false);

        // Test float parsing
        assert_eq!(parser.parse_float("1.5").unwrap(), 1.5);

        // Test random variable parsing
        let var = parser.parse_random_variable("1.0 5.0").unwrap();
        assert_eq!(var.min, 1.0);
        assert_eq!(var.max, 5.0);
    }

    #[test]
    fn test_enum_parsing() {
        let parser = ParticleSystemINIParser::default();

        assert_eq!(
            parser.parse_shader_type("ADDITIVE").unwrap(),
            ParticleShaderType::Additive
        );
        assert_eq!(
            parser.parse_particle_type("PARTICLE").unwrap(),
            ParticleType::Particle
        );
        assert_eq!(
            parser.parse_priority("CRITICAL").unwrap(),
            ParticlePriorityType::Critical
        );
    }
}
