//! # Particle System INI Loader
//!
//! Loads particle system definitions from INI files, matching the C++ parser exactly.
//! Supports all C++ particle system properties and parameters.

use nalgebra::{Point3, Vector3};
use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;

use super::particle_manager::*;
use game_engine::common::ini::{INI, INIError, INILoadType};

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
        // The current line is `ParticleSystem <name>`.  `get_current_token`
        // returns the block keyword, not the authored template identity.
        let name = ini
            .get_next_value_token()
            .ok_or(INIError::UnexpectedEndOfFile)?;

        // Find existing template or create new one (matches C++ behavior)
        if manager.find_template(&name).is_none() {
            manager.new_template(name.clone());
        }

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

    /// Overlay a Common-parsed ParticleSystem block onto the live manager.
    /// C++ INIParticleSys.cpp:24-32 find-or-create + initFromINI.
    pub fn overlay_from_property_map(
        &self,
        name: &str,
        properties: &HashMap<String, String>,
        manager: &mut ParticleSystemManager,
    ) -> Result<(), INIError> {
        let mut src = format!("ParticleSystem {name}\n");
        for (key, value) in properties {
            src.push_str(&format!("  {key} = {value}\n"));
        }
        src.push_str("End\n");
        let mut ini = INI::new();
        ini.with_inline_source(&src, |ini| {
            self.parse_particle_system_source(ini, manager).map(|_| ())
        })
    }

    /// Parse a source that contains only `ParticleSystem` blocks into `manager`.
    pub fn overlay_mixed_source(
        &self,
        contents: &str,
        manager: &mut ParticleSystemManager,
    ) -> Result<usize, INIError> {
        let mut ini = INI::new();
        ini.with_inline_source(contents, |ini| {
            self.parse_particle_system_source(ini, manager)
        })
    }

    /// Load C++ `ParticleSystem.ini` directly into the GameClient manager.
    ///
    /// The common INI loader also has a `ParticleSystem` block parser for its
    /// gameplay-side compatibility registry.  Replacing that process-global
    /// parser would make the two registries interfere with each other, so this
    /// walks the same virtual/BIG-backed source privately instead.  Each block
    /// is still parsed by the full GameClient field parser below.
    pub fn load_particle_system_definitions<P: AsRef<Path>>(
        &self,
        path: P,
        manager: &mut ParticleSystemManager,
    ) -> Result<usize, INIError> {
        let mut ini = INI::new();
        ini.with_file_source(path, INILoadType::Overwrite, |ini| {
            self.parse_particle_system_source(ini, manager)
        })
    }

    fn parse_particle_system_source(
        &self,
        ini: &mut INI,
        manager: &mut ParticleSystemManager,
    ) -> Result<usize, INIError> {
        let mut definitions = 0usize;

        loop {
            ini.read_line()?;
            if ini.is_end_of_file() {
                break;
            }

            let Some(block) = ini.get_first_token() else {
                continue;
            };
            // `INI::read_line` strips semicolon comments, but C++ content and
            // mods may also contain standalone C++-style comment lines.  They
            // are not particle definitions and must not abort the independent
            // manager parser before a later valid retail block.
            if block.starts_with("//") {
                continue;
            }
            if !block.eq_ignore_ascii_case("ParticleSystem") {
                return Err(INIError::UnknownToken);
            }

            self.parse_particle_system_definition(ini, manager)?;
            definitions += 1;
        }

        Ok(definitions)
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
                        EmissionVolumeType::Invalid => EmissionVolume::Point,
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
        let uses_labels = parts.iter().any(|part| part.contains(':'));
        if !uses_labels {
            if parts.len() != 3 {
                return Err(INIError::InvalidValue);
            }
            return Ok(Vector3::new(
                self.parse_float(parts[0])?,
                self.parse_float(parts[1])?,
                self.parse_float(parts[2])?,
            ));
        }

        let mut x = None;
        let mut y = None;
        let mut z = None;
        for part in parts {
            let Some((axis, raw_value)) = part.split_once(':') else {
                return Err(INIError::InvalidValue);
            };
            let component = self.parse_float(raw_value)?;
            match axis.to_ascii_uppercase().as_str() {
                "X" if x.replace(component).is_none() => {}
                "Y" if y.replace(component).is_none() => {}
                "Z" if z.replace(component).is_none() => {}
                _ => return Err(INIError::InvalidValue),
            }
        }

        Ok(Vector3::new(
            x.ok_or(INIError::InvalidValue)?,
            y.ok_or(INIError::InvalidValue)?,
            z.ok_or(INIError::InvalidValue)?,
        ))
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
            distribution_type: 0,
            frame,
        })
    }

    fn parse_rgb_color_keyframe(&self, value: &str) -> Result<RGBColorKeyframe, INIError> {
        let parts: Vec<&str> = value.split_whitespace().collect();
        let uses_labels = parts.iter().any(|part| part.contains(':'));
        let (r, g, b, frame) = if uses_labels {
            let mut r = None;
            let mut g = None;
            let mut b = None;
            let mut frame = None;
            for part in parts {
                if let Some((channel, raw_value)) = part.split_once(':') {
                    let component = self.parse_float(raw_value)?;
                    match channel.to_ascii_uppercase().as_str() {
                        "R" if r.replace(component).is_none() => {}
                        "G" if g.replace(component).is_none() => {}
                        "B" if b.replace(component).is_none() => {}
                        _ => return Err(INIError::InvalidValue),
                    }
                } else if frame.replace(self.parse_uint(part)?).is_some() {
                    return Err(INIError::InvalidValue);
                }
            }
            (
                r.ok_or(INIError::InvalidValue)?,
                g.ok_or(INIError::InvalidValue)?,
                b.ok_or(INIError::InvalidValue)?,
                frame.ok_or(INIError::InvalidValue)?,
            )
        } else {
            if parts.len() != 4 {
                return Err(INIError::InvalidValue);
            }
            (
                self.parse_float(parts[0])?,
                self.parse_float(parts[1])?,
                self.parse_float(parts[2])?,
                self.parse_uint(parts[3])?,
            )
        };

        Ok(RGBColorKeyframe {
            color: [r / 255.0, g / 255.0, b / 255.0],
            frame,
        })
    }

    fn parse_shader_type(&self, value: &str) -> Result<ParticleShaderType, INIError> {
        if value.trim().eq_ignore_ascii_case("NONE") {
            return Ok(ParticleShaderType::Invalid);
        }
        self.shader_type_names
            .get(&value.trim().to_ascii_uppercase())
            .copied()
            .ok_or(INIError::InvalidValue)
    }

    fn parse_particle_type(&self, value: &str) -> Result<ParticleType, INIError> {
        if value.trim().eq_ignore_ascii_case("NONE") {
            return Ok(ParticleType::Invalid);
        }
        self.particle_type_names
            .get(&value.trim().to_ascii_uppercase())
            .copied()
            .ok_or(INIError::InvalidValue)
    }

    fn parse_emission_velocity_type(&self, value: &str) -> Result<EmissionVelocityType, INIError> {
        if value.trim().eq_ignore_ascii_case("NONE") {
            return Ok(EmissionVelocityType::Invalid);
        }
        self.emission_velocity_names
            .get(&value.trim().to_ascii_uppercase())
            .copied()
            .ok_or(INIError::InvalidValue)
    }

    fn parse_emission_volume_type(&self, value: &str) -> Result<EmissionVolumeType, INIError> {
        if value.trim().eq_ignore_ascii_case("NONE") {
            return Ok(EmissionVolumeType::Invalid);
        }
        self.emission_volume_names
            .get(&value.trim().to_ascii_uppercase())
            .copied()
            .ok_or(INIError::InvalidValue)
    }

    pub fn parse_priority(&self, value: &str) -> Result<ParticlePriorityType, INIError> {
        // C++ ParticleSys.h:251-253 scanIndexList — "NONE" is index 0.
        if value.trim().eq_ignore_ascii_case("NONE") {
            return Ok(ParticlePriorityType::None);
        }
        self.priority_names
            .get(&value.trim().to_ascii_uppercase())
            .copied()
            .ok_or(INIError::InvalidValue)
    }

    fn parse_wind_motion(&self, value: &str) -> Result<WindMotion, INIError> {
        if value.trim().eq_ignore_ascii_case("NONE") {
            return Ok(WindMotion::Invalid);
        }
        self.wind_motion_names
            .get(&value.trim().to_ascii_uppercase())
            .copied()
            .ok_or(INIError::InvalidValue)
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
        self.priority_names
            .insert("NONE".to_string(), ParticlePriorityType::None);
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
            .insert("UNUSED".to_string(), WindMotion::NotUsed);
        self.wind_motion_names
            .insert("PINGPONG".to_string(), WindMotion::PingPong);
        self.wind_motion_names
            .insert("CIRCULAR".to_string(), WindMotion::Circular);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

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
        // C++ ParticleSys.h:251-253 — scanIndexList maps NONE to INVALID_PRIORITY (0).
        assert_eq!(
            parser.parse_priority("NONE").unwrap(),
            ParticlePriorityType::None
        );
        assert_eq!(
            parser.parse_priority("none").unwrap(),
            ParticlePriorityType::None
        );
    }

    #[test]
    fn retail_blocks_keep_their_authored_name_and_labeled_values() {
        let source = r#"
ParticleSystem RetailLabeledParticle
  Priority = WEAPON_EXPLOSION
  IsOneShot = No
  Shader = ALPHA
  Type = PARTICLE
  ParticleName = EXSmokNew1.tga
  Lifetime = 60.00 60.00
  Color1 = R:255 G:127 B:0 5
  DriftVelocity = X:1.25 Y:-2.5 Z:3.75
  VelocityType = NONE
  VolumeType = NONE
  WindMotion = Unused
End

ParticleSystem RetailSecondParticle
  Priority = CRITICAL
  Shader = ADDITIVE
  Type = PARTICLE
  ParticleName = EXLnzFlar2.tga
  VelocityType = OUTWARD
  VelOutward = 1.0 2.0
  VolumeType = SPHERE
  VolSphereRadius = 4.0
End
"#;

        let parser = ParticleSystemINIParser::default();
        let mut manager = ParticleSystemManager::new();
        let mut ini = INI::new();
        let count = ini
            .with_inline_source(source, |ini| {
                parser.parse_particle_system_source(ini, &mut manager)
            })
            .expect("retail-shaped particle source parses");

        assert_eq!(count, 2);
        let first = manager
            .find_template("RetailLabeledParticle")
            .expect("first exact template identity");
        let info = first.info();
        assert_eq!(info.particle_type_name, "EXSmokNew1.tga");
        assert_eq!(info.color_keys[0].color, [1.0, 127.0 / 255.0, 0.0]);
        assert_eq!(info.color_keys[0].frame, 5);
        assert_eq!(info.drift_velocity, Vector3::new(1.25, -2.5, 3.75));
        assert_eq!(info.emission_velocity_type, EmissionVelocityType::Invalid);
        assert_eq!(info.emission_volume_type, EmissionVolumeType::Invalid);
        assert_eq!(info.wind_motion, WindMotion::NotUsed);

        let second = manager
            .find_template("RetailSecondParticle")
            .expect("second exact template identity");
        assert_eq!(
            second.info().emission_volume_type,
            EmissionVolumeType::Sphere
        );
    }

    #[test]
    fn installed_retail_particle_system_ini_loads_all_authored_templates() {
        let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let Some(repository_root) = manifest.ancestors().nth(4) else {
            return;
        };
        let source = repository_root
            .join("windows_game")
            .join("extracted_big_files")
            .join("INIZH")
            .join("Data")
            .join("INI")
            .join("ParticleSystem.ini");
        if !source.is_file() {
            // Source-only CI checkouts legitimately do not have the licensed
            // retail extraction.  Runtime still loads through the BIG-backed
            // virtual filesystem when an install is present.
            return;
        }

        let parser = ParticleSystemINIParser::default();
        let mut manager = ParticleSystemManager::new();
        let count = parser
            .load_particle_system_definitions(&source, &mut manager)
            .expect("installed retail ParticleSystem.ini must parse");

        assert_eq!(count, 1_087);
        assert!(manager.find_template("TsingMaTrailSmoke").is_some());
        assert!(manager.find_template("Explosion").is_some());
        let drawable = manager
            .find_template("CarCrushGlassDebris")
            .expect("retail NONE shader/drawable template");
        assert_eq!(drawable.info().shader_type, ParticleShaderType::Invalid);
        assert_eq!(drawable.info().particle_type, ParticleType::Drawable);
    }

    #[test]
    fn priority_none_does_not_abort_remaining_templates() {
        // C++ ParticleSys.h:251-253 — NONE is valid index 0, not a parse error.
        let source = r#"
ParticleSystem FirstWithNone
  Priority = NONE
  Shader = ALPHA
  Type = PARTICLE
End

ParticleSystem SecondStillLoads
  Priority = CRITICAL
  Shader = ADDITIVE
  Type = PARTICLE
End
"#;
        let parser = ParticleSystemINIParser::default();
        let mut manager = ParticleSystemManager::new();
        let mut ini = INI::new();
        let count = ini
            .with_inline_source(source, |ini| {
                parser.parse_particle_system_source(ini, &mut manager)
            })
            .expect("NONE must not abort the ParticleSystem load");
        assert_eq!(count, 2);
        assert_eq!(
            manager
                .find_template("FirstWithNone")
                .expect("first")
                .info()
                .priority,
            ParticlePriorityType::None
        );
        assert!(manager.find_template("SecondStillLoads").is_some());
        assert_eq!(
            ParticlePriorityType::from_index(0),
            Some(ParticlePriorityType::None)
        );
    }
}
