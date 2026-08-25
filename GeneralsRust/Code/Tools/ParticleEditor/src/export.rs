//! Export Module
//!
//! Handles exporting particle systems to various formats.

use crate::particles::*;
use anyhow::Result;
use std::fs;
use std::path::Path;

/// Particle exporter
#[derive(Debug, Clone)]
pub struct ParticleExporter {
    pub export_path: Option<String>,
    pub export_format: ExportFormat,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ExportFormat {
    Json,
    Binary,
    Ini,
}

impl ParticleExporter {
    pub fn new() -> Self {
        Self {
            export_path: None,
            export_format: ExportFormat::Ini, // Default to INI for C&C
        }
    }

    pub fn initialize(&mut self) -> Result<()> {
        log::info!("Initializing particle exporter");
        Ok(())
    }

    pub fn export_particle_system(&self, system: &ParticleSystem, path: &Path) -> Result<()> {
        match self.export_format {
            ExportFormat::Json => {
                let json = serde_json::to_string_pretty(system)?;
                fs::write(path, json)?;
                log::info!("Exported particle system to JSON: {:?}", path);
            }
            ExportFormat::Binary => {
                // C++ ParticleEditor has no separate binary dump — `ShouldWriteINI`
                // writes `_writeSingleParticleSystem` INI that the engine loads.
                let ini_content = self.generate_ini_content(system);
                fs::write(path, ini_content)?;
                log::info!(
                    "Exported particle system as C++ _writeSingleParticleSystem INI: {:?}",
                    path
                );
            }
            ExportFormat::Ini => {
                let ini_content = self.generate_ini_content(system);
                fs::write(path, ini_content)?;
                log::info!("Exported particle system to INI: {:?}", path);
            }
        }
        Ok(())
    }

    pub fn export(&self, data: &str, path: &Path) -> Result<()> {
        match self.export_format {
            ExportFormat::Json => {
                fs::write(path, data)?;
                log::info!("Exported particle system to JSON: {:?}", path);
            }
            ExportFormat::Binary => {
                // C++ ParticleEditor has no separate binary dump — raw path still
                // writes `_writeSingleParticleSystem` INI text the engine loads.
                if !data.lines().any(|line| {
                    line.trim_start()
                        .to_ascii_lowercase()
                        .starts_with("particlesystem ")
                }) {
                    return Err(anyhow::anyhow!(
                        "Binary export requires ParticleSystem <Name> text (C++ _writeSingleParticleSystem)"
                    ));
                }
                fs::write(path, data)?;
                log::info!(
                    "Exported raw C++ particle INI via Binary format: {:?}",
                    path
                );
            }
            ExportFormat::Ini => {
                // Raw path: accept C++ ParticleSystem INI text (HEADER + fields + End).
                if !data.lines().any(|line| {
                    line.trim_start()
                        .to_ascii_lowercase()
                        .starts_with("particlesystem ")
                }) {
                    return Err(anyhow::anyhow!(
                        "INI export requires ParticleSystem <Name> text (C++ _writeSingleParticleSystem)"
                    ));
                }
                fs::write(path, data)?;
                log::info!("Exported raw C++ particle INI: {:?}", path);
            }
        }
        Ok(())
    }

    pub fn export_for_game_engine(&self, system: &ParticleSystem, path: &Path) -> Result<()> {
        // Export in a format optimized for the game engine
        // This is typically the INI format that the C&C engine expects
        let ini_content = self.generate_ini_content(system);
        fs::write(path, ini_content)?;
        log::info!("Exported particle system for game engine: {:?}", path);
        Ok(())
    }

    pub fn import_particle_system(&self, path: &Path) -> Result<ParticleSystem> {
        let content = fs::read_to_string(path)?;
        self.parse_ini_content(&content)
    }

    pub fn parse_ini_content(&self, content: &str) -> Result<ParticleSystem> {
        let mut system = ParticleSystem::new("ImportedParticleSystem".to_string())?;

        for line in content.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with(';') || line.eq_ignore_ascii_case("End") {
                continue;
            }

            // C++ `_writeSingleParticleSystem`: `ParticleSystem <Name>`
            if let Some(rest) = line.strip_prefix("ParticleSystem") {
                let name = rest.trim();
                if !name.is_empty() && name != "{" {
                    system.info.name = name.to_string();
                }
                continue;
            }

            if let Some((key, value)) = self.parse_ini_line(line) {
                self.apply_ini_setting(&mut system, &key, &value)?;
            }
        }

        Ok(system)
    }

    fn parse_ini_line(&self, line: &str) -> Option<(String, String)> {
        if let Some(equals_pos) = line.find('=') {
            let key = line[..equals_pos].trim().to_string();
            let value = line[equals_pos + 1..].trim().to_string();
            Some((key, value))
        } else {
            None
        }
    }

    fn apply_ini_setting(&self, system: &mut ParticleSystem, key: &str, value: &str) -> Result<()> {
        match key {
            "Name" => system.info.name = value.to_string(),
            "Priority" => system.info.priority = self.parse_priority(value)?,
            "IsOneShot" => system.info.is_one_shot = Self::parse_bool_token(value)?,
            "Shader" => system.info.shader_type = self.parse_shader(value)?,
            "Type" | "ParticleType" => {
                system.info.particle_type = self.parse_particle_type(value)?
            }
            "ParticleName" | "ParticleTypeName" => {
                system.info.particle_type_name = value.to_string()
            }

            // Emission settings
            "VolumeType" | "EmissionVolumeType" => {
                system.info.emission_volume_type = self.parse_emission_volume_type(value)?;
                system.info.emission_volume = match system.info.emission_volume_type {
                    EmissionVolumeType::Line => EmissionVolumeData::Line {
                        start: Coord3D::default(),
                        end: Coord3D::default(),
                    },
                    EmissionVolumeType::Box => EmissionVolumeData::Box {
                        half_size: Coord3D::default(),
                    },
                    EmissionVolumeType::Sphere => EmissionVolumeData::Sphere { radius: 1.0 },
                    EmissionVolumeType::Cylinder => EmissionVolumeData::Cylinder {
                        radius: 1.0,
                        length: 2.0,
                    },
                    _ => EmissionVolumeData::Point,
                };
            }
            "VelocityType" | "EmissionVelocityType" => {
                system.info.emission_velocity_type = self.parse_emission_velocity_type(value)?;
                system.info.emission_velocity = match system.info.emission_velocity_type {
                    EmissionVelocityType::Spherical => EmissionVelocityData::Spherical {
                        speed: GameClientRandomVariable::constant(0.0),
                    },
                    EmissionVelocityType::Hemispherical => EmissionVelocityData::Hemispherical {
                        speed: GameClientRandomVariable::constant(0.0),
                    },
                    EmissionVelocityType::Cylindrical => EmissionVelocityData::Cylindrical {
                        radial: GameClientRandomVariable::constant(0.0),
                        normal: GameClientRandomVariable::constant(0.0),
                    },
                    EmissionVelocityType::Outward => EmissionVelocityData::Outward {
                        speed: GameClientRandomVariable::constant(0.0),
                        other_speed: GameClientRandomVariable::constant(0.0),
                    },
                    _ => EmissionVelocityData::Ortho {
                        x: GameClientRandomVariable::constant(0.0),
                        y: GameClientRandomVariable::constant(0.0),
                        z: GameClientRandomVariable::constant(0.0),
                    },
                };
            }
            "IsHollow" | "IsEmissionVolumeHollow" => {
                system.info.is_emission_volume_hollow = Self::parse_bool_token(value)?
            }
            "IsGroundAligned" => system.info.is_ground_aligned = Self::parse_bool_token(value)?,
            "IsEmitAboveGroundOnly" => {
                system.info.is_emit_above_ground_only = Self::parse_bool_token(value)?
            }
            "IsParticleUpTowardsEmitter" => {
                system.info.is_particle_up_towards_emitter = Self::parse_bool_token(value)?
            }

            // System lifetime
            "SystemLifetime" => system.info.system_lifetime = value.parse()?,

            // Physics
            "DriftVelocity" => self.parse_coord3d(value, &mut system.info.drift_velocity)?,
            "Gravity" => system.info.gravity = value.parse()?,

            // Wind
            "WindMotion" => system.info.wind_motion = self.parse_wind_motion(value)?,
            "WindAngle" => system.info.wind_angle = value.parse()?,
            "WindAngleChange" => system.info.wind_angle_change = value.parse()?,
            "WindAngleChangeMin" => system.info.wind_angle_change_min = value.parse()?,
            "WindAngleChangeMax" => system.info.wind_angle_change_max = value.parse()?,
            "WindPingPongStartAngleMin" => {
                system.info.wind_motion_start_angle_min = value.parse()?
            }
            "WindPingPongStartAngleMax" => {
                system.info.wind_motion_start_angle_max = value.parse()?
            }
            "WindPingPongEndAngleMin" => system.info.wind_motion_end_angle_min = value.parse()?,
            "WindPingPongEndAngleMax" => system.info.wind_motion_end_angle_max = value.parse()?,

            // Slave systems
            "SlaveSystem" => system.info.slave_system_name = value.to_string(),
            "SlavePosOffset" => self.parse_coord3d(value, &mut system.info.slave_pos_offset)?,
            "AttachedSystem" | "PerParticleAttachedSystem" => {
                system.info.attached_system_name = value.to_string()
            }

            // Emission volume parameters (C++ Vol* names + legacy Emission* aliases)
            "VolLineStart" | "EmissionLineStart" => {
                self.ensure_volume_line(&mut system.info);
                self.parse_emission_line_start(value, &mut system.info.emission_volume)?
            }
            "VolLineEnd" | "EmissionLineEnd" => {
                self.ensure_volume_line(&mut system.info);
                self.parse_emission_line_end(value, &mut system.info.emission_volume)?
            }
            "VolBoxHalfSize" | "EmissionBoxHalfSize" => {
                self.ensure_volume_box(&mut system.info);
                self.parse_emission_box_half_size(value, &mut system.info.emission_volume)?
            }
            "VolSphereRadius" | "EmissionSphereRadius" => {
                self.ensure_volume_sphere(&mut system.info);
                self.parse_emission_sphere_radius(value, &mut system.info.emission_volume)?
            }
            "VolCylinderRadius" | "EmissionCylinderRadius" => {
                self.ensure_volume_cylinder(&mut system.info);
                self.parse_emission_cylinder_radius(value, &mut system.info.emission_volume)?
            }
            "VolCylinderLength" | "EmissionCylinderLength" => {
                self.ensure_volume_cylinder(&mut system.info);
                self.parse_emission_cylinder_length(value, &mut system.info.emission_volume)?
            }

            // Emission velocity parameters (C++ Vel* names + legacy aliases)
            "VelOrthoX" | "EmissionVelocityOrthoX" => {
                self.ensure_velocity_ortho(&mut system.info);
                self.parse_emission_velocity_ortho_x(value, &mut system.info.emission_velocity)?
            }
            "VelOrthoY" | "EmissionVelocityOrthoY" => {
                self.ensure_velocity_ortho(&mut system.info);
                self.parse_emission_velocity_ortho_y(value, &mut system.info.emission_velocity)?
            }
            "VelOrthoZ" | "EmissionVelocityOrthoZ" => {
                self.ensure_velocity_ortho(&mut system.info);
                self.parse_emission_velocity_ortho_z(value, &mut system.info.emission_velocity)?
            }
            "VelSpherical" | "EmissionVelocitySphericalSpeed" => {
                self.parse_emission_velocity_spherical_speed(
                    value,
                    &mut system.info.emission_velocity,
                )?;
            }
            "VelHemispherical" | "EmissionVelocityHemisphericalSpeed" => {
                self.parse_emission_velocity_hemispherical_speed(
                    value,
                    &mut system.info.emission_velocity,
                )?;
            }
            "VelCylindricalRadial" | "EmissionVelocityCylindricalRadial" => {
                self.parse_emission_velocity_cylindrical_radial(
                    value,
                    &mut system.info.emission_velocity,
                )?;
            }
            "VelCylindricalNormal" | "EmissionVelocityCylindricalNormal" => {
                self.parse_emission_velocity_cylindrical_normal(
                    value,
                    &mut system.info.emission_velocity,
                )?;
            }
            "VelOutward" | "EmissionVelocityOutwardSpeed" => {
                self.parse_emission_velocity_outward_speed(
                    value,
                    &mut system.info.emission_velocity,
                )?;
            }
            "VelOutwardOther" | "EmissionVelocityOutwardOtherSpeed" => {
                self.parse_emission_velocity_outward_other_speed(
                    value,
                    &mut system.info.emission_velocity,
                )?;
            }

            // Particle parameters (C++ writes `Size` for start size)
            "Lifetime" => system.info.lifetime = self.parse_random_variable(value)?,
            "Size" | "StartSize" => system.info.start_size = self.parse_random_variable(value)?,
            "StartSizeRate" => system.info.start_size_rate = self.parse_random_variable(value)?,
            "SizeRate" => system.info.size_rate = self.parse_random_variable(value)?,
            "SizeRateDamping" => {
                system.info.size_rate_damping = self.parse_random_variable(value)?
            }
            "AngleZ" => system.info.angle_z = self.parse_random_variable(value)?,
            "AngularRateZ" => system.info.angular_rate_z = self.parse_random_variable(value)?,
            "AngularDamping" => system.info.angular_damping = self.parse_random_variable(value)?,
            "VelocityDamping" => system.info.vel_damping = self.parse_random_variable(value)?,
            "ColorScale" => system.info.color_scale = self.parse_random_variable(value)?,
            "BurstDelay" => system.info.burst_delay = self.parse_random_variable(value)?,
            "BurstCount" => system.info.burst_count = self.parse_random_variable(value)?,
            "InitialDelay" => system.info.initial_delay = self.parse_random_variable(value)?,
            "Alpha1" | "Alpha2" | "Alpha3" | "Alpha4" | "Alpha5" | "Alpha6" | "Alpha7"
            | "Alpha8" => {
                if let Some(idx) = key.chars().last().and_then(|c| c.to_digit(10)) {
                    let i = idx as usize - 1;
                    if i < MAX_KEYFRAMES {
                        system.info.alpha_key[i] = self.parse_alpha_keyframe(value)?;
                    }
                }
            }
            "Color1" | "Color2" | "Color3" | "Color4" | "Color5" | "Color6" | "Color7"
            | "Color8" => {
                if let Some(idx) = key.chars().last().and_then(|c| c.to_digit(10)) {
                    let i = idx as usize - 1;
                    if i < MAX_KEYFRAMES {
                        system.info.color_key[i] = self.parse_color_keyframe(value)?;
                    }
                }
            }

            _ => {
                // Unknown parameter, skip
                log::debug!("Unknown INI parameter: {}", key);
            }
        }
        Ok(())
    }

    pub fn generate_ini_content(&self, system: &ParticleSystem) -> String {
        let info = &system.info;
        let mut content = String::new();

        // C++ ScriptEngine::_writeSingleParticleSystem field order.
        content.push_str(&format!("ParticleSystem {}\n", info.name));
        content.push_str(&format!(
            "  Priority = {}\n",
            Self::priority_to_string(info.priority)
        ));
        content.push_str(&format!(
            "  IsOneShot = {}\n",
            Self::yes_no(info.is_one_shot)
        ));
        content.push_str(&format!(
            "  Shader = {}\n",
            Self::shader_to_string(info.shader_type)
        ));
        content.push_str(&format!(
            "  Type = {}\n",
            Self::particle_type_to_string(info.particle_type)
        ));
        content.push_str(&format!("  ParticleName = {}\n", info.particle_type_name));

        content.push_str("  AngleZ = ");
        content.push_str(&self.random_var_to_string(&info.angle_z));
        content.push_str("\n  AngularRateZ = ");
        content.push_str(&self.random_var_to_string(&info.angular_rate_z));
        content.push_str("\n  AngularDamping = ");
        content.push_str(&self.random_var_to_string(&info.angular_damping));
        content.push_str("\n  VelocityDamping = ");
        content.push_str(&self.random_var_to_string(&info.vel_damping));
        content.push_str(&format!("\n  Gravity = {}\n", info.gravity));

        if !info.slave_system_name.is_empty() {
            content.push_str(&format!("  SlaveSystem = {}\n", info.slave_system_name));
            content.push_str(&format!(
                "  SlavePosOffset = X:{} Y:{} Z:{}\n",
                info.slave_pos_offset.x, info.slave_pos_offset.y, info.slave_pos_offset.z
            ));
        }
        if !info.attached_system_name.is_empty() {
            content.push_str(&format!(
                "  PerParticleAttachedSystem = {}\n",
                info.attached_system_name
            ));
        }

        content.push_str("  Lifetime = ");
        content.push_str(&self.random_var_to_string(&info.lifetime));
        content.push_str(&format!("\n  SystemLifetime = {}\n", info.system_lifetime));
        content.push_str("  Size = ");
        content.push_str(&self.random_var_to_string(&info.start_size));
        content.push_str("\n  StartSizeRate = ");
        content.push_str(&self.random_var_to_string(&info.start_size_rate));
        content.push_str("\n  SizeRate = ");
        content.push_str(&self.random_var_to_string(&info.size_rate));
        content.push_str("\n  SizeRateDamping = ");
        content.push_str(&self.random_var_to_string(&info.size_rate_damping));
        content.push_str("\n");

        for (i, key) in info.alpha_key.iter().enumerate() {
            content.push_str(&format!(
                "  Alpha{} = {} {} {}\n",
                i + 1,
                key.var.low,
                key.var.high,
                key.frame
            ));
        }
        for (i, key) in info.color_key.iter().enumerate() {
            let r = (key.color.red * 255.0 + 0.5) as i32;
            let g = (key.color.green * 255.0 + 0.5) as i32;
            let b = (key.color.blue * 255.0 + 0.5) as i32;
            content.push_str(&format!(
                "  Color{} = R:{} G:{} B:{} {}\n",
                i + 1,
                r,
                g,
                b,
                key.frame
            ));
        }

        content.push_str("  ColorScale = ");
        content.push_str(&self.random_var_to_string(&info.color_scale));
        content.push_str("\n  BurstDelay = ");
        content.push_str(&self.random_var_to_string(&info.burst_delay));
        content.push_str("\n  BurstCount = ");
        content.push_str(&self.random_var_to_string(&info.burst_count));
        content.push_str("\n  InitialDelay = ");
        content.push_str(&self.random_var_to_string(&info.initial_delay));
        content.push_str(&format!(
            "\n  DriftVelocity = X:{} Y:{} Z:{}\n",
            info.drift_velocity.x, info.drift_velocity.y, info.drift_velocity.z
        ));

        content.push_str(&format!(
            "  VelocityType = {}\n",
            Self::emission_velocity_to_string(info.emission_velocity_type)
        ));
        content.push_str(&self.generate_emission_velocity_ini(&info.emission_velocity));
        content.push_str(&format!(
            "  VolumeType = {}\n",
            Self::emission_volume_to_string(info.emission_volume_type)
        ));
        content.push_str(&self.generate_emission_volume_ini(&info.emission_volume));

        content.push_str(&format!(
            "  IsHollow = {}\n",
            Self::yes_no(info.is_emission_volume_hollow)
        ));
        content.push_str(&format!(
            "  IsGroundAligned = {}\n",
            Self::yes_no(info.is_ground_aligned)
        ));
        content.push_str(&format!(
            "  IsEmitAboveGroundOnly = {}\n",
            Self::yes_no(info.is_emit_above_ground_only)
        ));
        content.push_str(&format!(
            "  IsParticleUpTowardsEmitter = {}\n",
            Self::yes_no(info.is_particle_up_towards_emitter)
        ));

        content.push_str(&format!(
            "  WindMotion = {}\n",
            Self::wind_motion_to_string(info.wind_motion)
        ));
        content.push_str(&format!(
            "  WindAngleChangeMin = {}\n",
            info.wind_angle_change_min
        ));
        content.push_str(&format!(
            "  WindAngleChangeMax = {}\n",
            info.wind_angle_change_max
        ));
        // C++ `_writeSingleParticleSystem` then WindPingPong* after WindAngleChange*.
        content.push_str(&format!(
            "  WindPingPongStartAngleMin = {}\n",
            info.wind_motion_start_angle_min
        ));
        content.push_str(&format!(
            "  WindPingPongStartAngleMax = {}\n",
            info.wind_motion_start_angle_max
        ));
        content.push_str(&format!(
            "  WindPingPongEndAngleMin = {}\n",
            info.wind_motion_end_angle_min
        ));
        content.push_str(&format!(
            "  WindPingPongEndAngleMax = {}\n",
            info.wind_motion_end_angle_max
        ));

        content.push_str("End\n\n");
        content
    }

    fn yes_no(value: bool) -> &'static str {
        if value { "Yes" } else { "No" }
    }

    fn parse_bool_token(value: &str) -> Result<bool> {
        match value.trim().to_ascii_lowercase().as_str() {
            "1" | "true" | "yes" | "y" => Ok(true),
            "0" | "false" | "no" | "n" => Ok(false),
            _ => Err(anyhow::anyhow!("Invalid bool token: {}", value)),
        }
    }

    fn ensure_volume_line(&self, info: &mut ParticleSystemInfo) {
        if !matches!(info.emission_volume, EmissionVolumeData::Line { .. }) {
            info.emission_volume_type = EmissionVolumeType::Line;
            info.emission_volume = EmissionVolumeData::Line {
                start: Coord3D::default(),
                end: Coord3D::default(),
            };
        }
    }

    fn ensure_volume_box(&self, info: &mut ParticleSystemInfo) {
        if !matches!(info.emission_volume, EmissionVolumeData::Box { .. }) {
            info.emission_volume_type = EmissionVolumeType::Box;
            info.emission_volume = EmissionVolumeData::Box {
                half_size: Coord3D::default(),
            };
        }
    }

    fn ensure_volume_sphere(&self, info: &mut ParticleSystemInfo) {
        if !matches!(info.emission_volume, EmissionVolumeData::Sphere { .. }) {
            info.emission_volume_type = EmissionVolumeType::Sphere;
            info.emission_volume = EmissionVolumeData::Sphere { radius: 1.0 };
        }
    }

    fn ensure_volume_cylinder(&self, info: &mut ParticleSystemInfo) {
        if !matches!(info.emission_volume, EmissionVolumeData::Cylinder { .. }) {
            info.emission_volume_type = EmissionVolumeType::Cylinder;
            info.emission_volume = EmissionVolumeData::Cylinder {
                radius: 1.0,
                length: 2.0,
            };
        }
    }

    fn ensure_velocity_ortho(&self, info: &mut ParticleSystemInfo) {
        if !matches!(info.emission_velocity, EmissionVelocityData::Ortho { .. }) {
            info.emission_velocity_type = EmissionVelocityType::Ortho;
            info.emission_velocity = EmissionVelocityData::Ortho {
                x: GameClientRandomVariable::constant(0.0),
                y: GameClientRandomVariable::constant(0.0),
                z: GameClientRandomVariable::constant(0.0),
            };
        }
    }

    fn generate_emission_volume_ini(&self, volume: &EmissionVolumeData) -> String {
        let mut content = String::new();

        match volume {
            EmissionVolumeData::Point => {
                // No additional parameters for point
            }
            EmissionVolumeData::Line { start, end } => {
                content.push_str("  VolLineStart = X:");
                content.push_str(&format!("{} Y:{} Z:{}\n", start.x, start.y, start.z));
                content.push_str("  VolLineEnd = X:");
                content.push_str(&format!("{} Y:{} Z:{}\n", end.x, end.y, end.z));
            }
            EmissionVolumeData::Box { half_size } => {
                content.push_str("  VolBoxHalfSize = X:");
                content.push_str(&format!(
                    "{} Y:{} Z:{}\n",
                    half_size.x, half_size.y, half_size.z
                ));
            }
            EmissionVolumeData::Sphere { radius } => {
                content.push_str(&format!("  VolSphereRadius = {}\n", radius));
            }
            EmissionVolumeData::Cylinder { radius, length } => {
                content.push_str(&format!("  VolCylinderRadius = {}\n", radius));
                content.push_str(&format!("  VolCylinderLength = {}\n", length));
            }
        }

        content
    }

    fn generate_emission_velocity_ini(&self, velocity: &EmissionVelocityData) -> String {
        let mut content = String::new();

        match velocity {
            EmissionVelocityData::Ortho { x, y, z } => {
                content.push_str("  VelOrthoX = ");
                content.push_str(&self.random_var_to_string(x));
                content.push_str("\n");
                content.push_str("  VelOrthoY = ");
                content.push_str(&self.random_var_to_string(y));
                content.push_str("\n");
                content.push_str("  VelOrthoZ = ");
                content.push_str(&self.random_var_to_string(z));
                content.push_str("\n");
            }
            EmissionVelocityData::Spherical { speed } => {
                content.push_str("  VelSpherical = ");
                content.push_str(&self.random_var_to_string(speed));
                content.push_str("\n");
            }
            EmissionVelocityData::Hemispherical { speed } => {
                content.push_str("  VelHemispherical = ");
                content.push_str(&self.random_var_to_string(speed));
                content.push_str("\n");
            }
            EmissionVelocityData::Cylindrical { radial, normal } => {
                content.push_str("  VelCylindricalRadial = ");
                content.push_str(&self.random_var_to_string(radial));
                content.push_str("\n");
                content.push_str("  VelCylindricalNormal = ");
                content.push_str(&self.random_var_to_string(normal));
                content.push_str("\n");
            }
            EmissionVelocityData::Outward { speed, other_speed } => {
                content.push_str("  VelOutward = ");
                content.push_str(&self.random_var_to_string(speed));
                content.push_str("\n");
                content.push_str("  VelOutwardOther = ");
                content.push_str(&self.random_var_to_string(other_speed));
                content.push_str("\n");
            }
        }

        content
    }

    fn parse_alpha_keyframe(&self, value: &str) -> Result<RandomKeyframe> {
        let parts: Vec<f32> = value
            .split_whitespace()
            .filter_map(|s| s.parse().ok())
            .collect();
        if parts.len() < 2 {
            return Err(anyhow::anyhow!("Invalid Alpha keyframe: {}", value));
        }
        let frame = if parts.len() >= 3 { parts[2] as u32 } else { 0 };
        Ok(RandomKeyframe {
            var: GameClientRandomVariable::new(parts[0], parts[1]),
            frame,
        })
    }

    fn parse_color_keyframe(&self, value: &str) -> Result<RGBColorKeyframe> {
        let mut red = 255.0;
        let mut green = 255.0;
        let mut blue = 255.0;
        let mut frame = 0u32;
        for part in value.split_whitespace() {
            if let Some(v) = part.strip_prefix("R:") {
                red = v.parse().unwrap_or(red);
            } else if let Some(v) = part.strip_prefix("G:") {
                green = v.parse().unwrap_or(green);
            } else if let Some(v) = part.strip_prefix("B:") {
                blue = v.parse().unwrap_or(blue);
            } else if let Ok(f) = part.parse::<u32>() {
                frame = f;
            }
        }
        Ok(RGBColorKeyframe {
            color: RGBColor {
                red: red / 255.0,
                green: green / 255.0,
                blue: blue / 255.0,
            },
            frame,
        })
    }

    fn random_var_to_string(&self, var: &GameClientRandomVariable) -> String {
        if var.distribution == DistributionType::Constant {
            format!("{}", var.low)
        } else {
            format!("{} {}", var.low, var.high)
        }
    }

    fn priority_to_string(priority: ParticlePriorityType) -> &'static str {
        match priority {
            ParticlePriorityType::Invalid => "INVALID",
            ParticlePriorityType::WeaponExplosion => "WEAPON_EXPLOSION",
            ParticlePriorityType::ScorchMark => "SCORCHMARK",
            ParticlePriorityType::DustTrail => "DUST_TRAIL",
            ParticlePriorityType::Buildup => "BUILDUP",
            ParticlePriorityType::DebrisTrail => "DEBRIS_TRAIL",
            ParticlePriorityType::UnitDamageFx => "UNIT_DAMAGE_FX",
            ParticlePriorityType::DeathExplosion => "DEATH_EXPLOSION",
            ParticlePriorityType::SemiConstant => "SEMI_CONSTANT",
            ParticlePriorityType::Constant => "CONSTANT",
            ParticlePriorityType::WeaponTrail => "WEAPON_TRAIL",
            ParticlePriorityType::AreaEffect => "AREA_EFFECT",
            ParticlePriorityType::Critical => "CRITICAL",
            ParticlePriorityType::AlwaysRender => "ALWAYS_RENDER",
        }
    }

    fn shader_to_string(shader: ParticleShaderType) -> &'static str {
        match shader {
            ParticleShaderType::Invalid => "INVALID",
            ParticleShaderType::Additive => "ADDITIVE",
            ParticleShaderType::Alpha => "ALPHA",
            ParticleShaderType::AlphaTest => "ALPHA_TEST",
            ParticleShaderType::Multiply => "MULTIPLY",
        }
    }

    fn particle_type_to_string(pt: ParticleType) -> &'static str {
        match pt {
            ParticleType::Invalid => "INVALID",
            ParticleType::Particle => "PARTICLE",
            ParticleType::Drawable => "DRAWABLE",
            ParticleType::Streak => "STREAK",
            ParticleType::VolumeParticle => "VOLUME_PARTICLE",
            ParticleType::Smudge => "SMUDGE",
        }
    }

    fn emission_volume_to_string(evt: EmissionVolumeType) -> &'static str {
        match evt {
            EmissionVolumeType::Invalid => "INVALID",
            EmissionVolumeType::Point => "POINT",
            EmissionVolumeType::Line => "LINE",
            EmissionVolumeType::Box => "BOX",
            EmissionVolumeType::Sphere => "SPHERE",
            EmissionVolumeType::Cylinder => "CYLINDER",
        }
    }

    fn emission_velocity_to_string(evt: EmissionVelocityType) -> &'static str {
        match evt {
            EmissionVelocityType::Invalid => "INVALID",
            EmissionVelocityType::Ortho => "ORTHO",
            EmissionVelocityType::Spherical => "SPHERICAL",
            EmissionVelocityType::Hemispherical => "HEMISPHERICAL",
            EmissionVelocityType::Cylindrical => "CYLINDRICAL",
            EmissionVelocityType::Outward => "OUTWARD",
        }
    }

    fn wind_motion_to_string(wm: WindMotion) -> &'static str {
        match wm {
            WindMotion::Invalid => "INVALID",
            WindMotion::NotUsed => "NOT_USED",
            WindMotion::PingPong => "PING_PONG",
            WindMotion::Circular => "CIRCULAR",
        }
    }

    // Import parsing helper methods
    fn parse_priority(&self, value: &str) -> Result<ParticlePriorityType> {
        match value {
            "INVALID" => Ok(ParticlePriorityType::Invalid),
            "WEAPON_EXPLOSION" => Ok(ParticlePriorityType::WeaponExplosion),
            "SCORCHMARK" => Ok(ParticlePriorityType::ScorchMark),
            "DUST_TRAIL" => Ok(ParticlePriorityType::DustTrail),
            "BUILDUP" => Ok(ParticlePriorityType::Buildup),
            "DEBRIS_TRAIL" => Ok(ParticlePriorityType::DebrisTrail),
            "UNIT_DAMAGE_FX" => Ok(ParticlePriorityType::UnitDamageFx),
            "DEATH_EXPLOSION" => Ok(ParticlePriorityType::DeathExplosion),
            "SEMI_CONSTANT" => Ok(ParticlePriorityType::SemiConstant),
            "CONSTANT" => Ok(ParticlePriorityType::Constant),
            "WEAPON_TRAIL" => Ok(ParticlePriorityType::WeaponTrail),
            "AREA_EFFECT" => Ok(ParticlePriorityType::AreaEffect),
            "CRITICAL" => Ok(ParticlePriorityType::Critical),
            "ALWAYS_RENDER" => Ok(ParticlePriorityType::AlwaysRender),
            _ => Err(anyhow::anyhow!("Unknown priority: {}", value)),
        }
    }

    fn parse_shader(&self, value: &str) -> Result<ParticleShaderType> {
        match value {
            "INVALID" => Ok(ParticleShaderType::Invalid),
            "ADDITIVE" => Ok(ParticleShaderType::Additive),
            "ALPHA" => Ok(ParticleShaderType::Alpha),
            "ALPHA_TEST" => Ok(ParticleShaderType::AlphaTest),
            "MULTIPLY" => Ok(ParticleShaderType::Multiply),
            _ => Err(anyhow::anyhow!("Unknown shader: {}", value)),
        }
    }

    fn parse_particle_type(&self, value: &str) -> Result<ParticleType> {
        match value {
            "INVALID" => Ok(ParticleType::Invalid),
            "PARTICLE" => Ok(ParticleType::Particle),
            "DRAWABLE" => Ok(ParticleType::Drawable),
            "STREAK" => Ok(ParticleType::Streak),
            "VOLUME_PARTICLE" => Ok(ParticleType::VolumeParticle),
            "SMUDGE" => Ok(ParticleType::Smudge),
            _ => Err(anyhow::anyhow!("Unknown particle type: {}", value)),
        }
    }

    fn parse_emission_volume_type(&self, value: &str) -> Result<EmissionVolumeType> {
        match value {
            "INVALID" => Ok(EmissionVolumeType::Invalid),
            "POINT" => Ok(EmissionVolumeType::Point),
            "LINE" => Ok(EmissionVolumeType::Line),
            "BOX" => Ok(EmissionVolumeType::Box),
            "SPHERE" => Ok(EmissionVolumeType::Sphere),
            "CYLINDER" => Ok(EmissionVolumeType::Cylinder),
            _ => Err(anyhow::anyhow!("Unknown emission volume type: {}", value)),
        }
    }

    fn parse_emission_velocity_type(&self, value: &str) -> Result<EmissionVelocityType> {
        match value {
            "INVALID" => Ok(EmissionVelocityType::Invalid),
            "ORTHO" => Ok(EmissionVelocityType::Ortho),
            "SPHERICAL" => Ok(EmissionVelocityType::Spherical),
            "HEMISPHERICAL" => Ok(EmissionVelocityType::Hemispherical),
            "CYLINDRICAL" => Ok(EmissionVelocityType::Cylindrical),
            "OUTWARD" => Ok(EmissionVelocityType::Outward),
            _ => Err(anyhow::anyhow!("Unknown emission velocity type: {}", value)),
        }
    }

    fn parse_wind_motion(&self, value: &str) -> Result<WindMotion> {
        match value {
            "INVALID" => Ok(WindMotion::Invalid),
            "NOT_USED" => Ok(WindMotion::NotUsed),
            "PING_PONG" => Ok(WindMotion::PingPong),
            "CIRCULAR" => Ok(WindMotion::Circular),
            _ => Err(anyhow::anyhow!("Unknown wind motion: {}", value)),
        }
    }

    fn parse_coord3d(&self, value: &str, coord: &mut Coord3D) -> Result<()> {
        // Parse format like "X:1.0 Y:2.0 Z:3.0"
        let parts: Vec<&str> = value.split_whitespace().collect();
        for part in parts {
            if let Some((axis, val_str)) = part.split_once(':') {
                let val: f32 = val_str.parse()?;
                match axis {
                    "X" => coord.x = val,
                    "Y" => coord.y = val,
                    "Z" => coord.z = val,
                    _ => {}
                }
            }
        }
        Ok(())
    }

    fn parse_random_variable(&self, value: &str) -> Result<GameClientRandomVariable> {
        let parts: Vec<f32> = value
            .split_whitespace()
            .filter_map(|s| s.parse().ok())
            .collect();

        match parts.len() {
            1 => Ok(GameClientRandomVariable::constant(parts[0])),
            2 => Ok(GameClientRandomVariable::new(parts[0], parts[1])),
            _ => Err(anyhow::anyhow!("Invalid random variable format: {}", value)),
        }
    }

    // Emission volume parsing helpers
    fn parse_emission_line_start(
        &self,
        value: &str,
        volume: &mut EmissionVolumeData,
    ) -> Result<()> {
        if let EmissionVolumeData::Line { start, .. } = volume {
            self.parse_coord3d(value, start)?;
        }
        Ok(())
    }

    fn parse_emission_line_end(&self, value: &str, volume: &mut EmissionVolumeData) -> Result<()> {
        if let EmissionVolumeData::Line { end, .. } = volume {
            self.parse_coord3d(value, end)?;
        }
        Ok(())
    }

    fn parse_emission_box_half_size(
        &self,
        value: &str,
        volume: &mut EmissionVolumeData,
    ) -> Result<()> {
        if let EmissionVolumeData::Box { half_size } = volume {
            self.parse_coord3d(value, half_size)?;
        }
        Ok(())
    }

    fn parse_emission_sphere_radius(
        &self,
        value: &str,
        volume: &mut EmissionVolumeData,
    ) -> Result<()> {
        if let EmissionVolumeData::Sphere { radius } = volume {
            *radius = value.parse()?;
        }
        Ok(())
    }

    fn parse_emission_cylinder_radius(
        &self,
        value: &str,
        volume: &mut EmissionVolumeData,
    ) -> Result<()> {
        if let EmissionVolumeData::Cylinder { radius, .. } = volume {
            *radius = value.parse()?;
        }
        Ok(())
    }

    fn parse_emission_cylinder_length(
        &self,
        value: &str,
        volume: &mut EmissionVolumeData,
    ) -> Result<()> {
        if let EmissionVolumeData::Cylinder { length, .. } = volume {
            *length = value.parse()?;
        }
        Ok(())
    }

    // Emission velocity parsing helpers
    fn parse_emission_velocity_ortho_x(
        &self,
        value: &str,
        velocity: &mut EmissionVelocityData,
    ) -> Result<()> {
        if let EmissionVelocityData::Ortho { x, .. } = velocity {
            *x = self.parse_random_variable(value)?;
        }
        Ok(())
    }

    fn parse_emission_velocity_ortho_y(
        &self,
        value: &str,
        velocity: &mut EmissionVelocityData,
    ) -> Result<()> {
        if let EmissionVelocityData::Ortho { y, .. } = velocity {
            *y = self.parse_random_variable(value)?;
        }
        Ok(())
    }

    fn parse_emission_velocity_ortho_z(
        &self,
        value: &str,
        velocity: &mut EmissionVelocityData,
    ) -> Result<()> {
        if let EmissionVelocityData::Ortho { z, .. } = velocity {
            *z = self.parse_random_variable(value)?;
        }
        Ok(())
    }

    fn parse_emission_velocity_spherical_speed(
        &self,
        value: &str,
        velocity: &mut EmissionVelocityData,
    ) -> Result<()> {
        let speed = self.parse_random_variable(value)?;
        *velocity = EmissionVelocityData::Spherical { speed };
        Ok(())
    }

    fn parse_emission_velocity_hemispherical_speed(
        &self,
        value: &str,
        velocity: &mut EmissionVelocityData,
    ) -> Result<()> {
        let speed = self.parse_random_variable(value)?;
        *velocity = EmissionVelocityData::Hemispherical { speed };
        Ok(())
    }

    fn parse_emission_velocity_cylindrical_radial(
        &self,
        value: &str,
        velocity: &mut EmissionVelocityData,
    ) -> Result<()> {
        let radial = self.parse_random_variable(value)?;
        *velocity = match velocity {
            EmissionVelocityData::Cylindrical { normal, .. } => EmissionVelocityData::Cylindrical {
                radial,
                normal: normal.clone(),
            },
            _ => EmissionVelocityData::Cylindrical {
                radial,
                normal: GameClientRandomVariable::constant(0.0),
            },
        };
        Ok(())
    }

    fn parse_emission_velocity_cylindrical_normal(
        &self,
        value: &str,
        velocity: &mut EmissionVelocityData,
    ) -> Result<()> {
        let normal = self.parse_random_variable(value)?;
        *velocity = match velocity {
            EmissionVelocityData::Cylindrical { radial, .. } => EmissionVelocityData::Cylindrical {
                radial: radial.clone(),
                normal,
            },
            _ => EmissionVelocityData::Cylindrical {
                radial: GameClientRandomVariable::constant(0.0),
                normal,
            },
        };
        Ok(())
    }

    fn parse_emission_velocity_outward_speed(
        &self,
        value: &str,
        velocity: &mut EmissionVelocityData,
    ) -> Result<()> {
        let speed = self.parse_random_variable(value)?;
        *velocity = match velocity {
            EmissionVelocityData::Outward { other_speed, .. } => EmissionVelocityData::Outward {
                speed,
                other_speed: other_speed.clone(),
            },
            _ => EmissionVelocityData::Outward {
                speed,
                other_speed: GameClientRandomVariable::constant(0.0),
            },
        };
        Ok(())
    }

    fn parse_emission_velocity_outward_other_speed(
        &self,
        value: &str,
        velocity: &mut EmissionVelocityData,
    ) -> Result<()> {
        let other_speed = self.parse_random_variable(value)?;
        *velocity = match velocity {
            EmissionVelocityData::Outward { speed, .. } => EmissionVelocityData::Outward {
                speed: speed.clone(),
                other_speed,
            },
            _ => EmissionVelocityData::Outward {
                speed: GameClientRandomVariable::constant(0.0),
                other_speed,
            },
        };
        Ok(())
    }
}

impl Default for ParticleExporter {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn particle_ini_roundtrip_preserves_name_and_lifetime() {
        let mut system = ParticleSystem::new("TestBurst".to_string()).expect("system");
        system.info.priority = ParticlePriorityType::WeaponExplosion;
        system.info.shader_type = ParticleShaderType::Additive;
        system.info.particle_type = ParticleType::Particle;
        system.info.particle_type_name = "EXPburst".to_string();
        system.info.is_one_shot = true;
        system.info.system_lifetime = 30;
        system.info.gravity = 0.25;
        system.info.emission_volume_type = EmissionVolumeType::Point;
        system.info.emission_velocity_type = EmissionVelocityType::Ortho;
        system.info.alpha_key[0] = RandomKeyframe {
            var: GameClientRandomVariable::new(0.2, 0.8),
            frame: 10,
        };
        system.info.color_key[0] = RGBColorKeyframe {
            color: RGBColor {
                red: 1.0,
                green: 0.0,
                blue: 0.0,
            },
            frame: 4,
        };
        let exporter = ParticleExporter::new();
        let ini = exporter.generate_ini_content(&system);
        assert!(ini.contains("ParticleSystem TestBurst"));
        assert!(!ini.contains("Name = TestBurst"));
        assert!(ini.contains("Type = PARTICLE"));
        assert!(ini.contains("ParticleName = EXPburst"));
        assert!(ini.contains("IsOneShot = Yes"));
        assert!(ini.contains("SystemLifetime = 30"));
        assert!(ini.contains("Gravity = 0.25") || ini.contains("Gravity = 0.250"));
        assert!(ini.contains("Alpha1 = 0.2 0.8 10"));
        assert!(ini.contains("Color1 = R:255 G:0 B:0 4"));
        assert!(ini.contains("Alpha8 ="));
        assert!(ini.contains("Color8 ="));
        let loaded = exporter.parse_ini_content(&ini).expect("parse");
        assert_eq!(loaded.info.name, "TestBurst");
        assert_eq!(loaded.info.particle_type_name, "EXPburst");
        assert_eq!(loaded.info.system_lifetime, 30);
        assert_eq!(loaded.info.priority, ParticlePriorityType::WeaponExplosion);
        assert_eq!(loaded.info.shader_type, ParticleShaderType::Additive);
        assert!(loaded.info.is_one_shot);
        assert!((loaded.info.alpha_key[0].var.low - 0.2).abs() < 1e-5);
        assert_eq!(loaded.info.alpha_key[0].frame, 10);
        assert!((loaded.info.color_key[0].color.red - 1.0).abs() < 0.01);
        assert_eq!(loaded.info.color_key[0].frame, 4);
    }

    #[test]
    fn particle_ini_field_order_matches_cpp_write_single_particle_system() {
        let mut system = ParticleSystem::new("OrderCheck".to_string()).expect("system");
        system.info.wind_motion = WindMotion::PingPong;
        system.info.wind_angle_change_min = 0.15;
        system.info.wind_angle_change_max = 0.45;
        system.info.wind_motion_start_angle_min = 0.1;
        system.info.wind_motion_start_angle_max = 0.2;
        system.info.wind_motion_end_angle_min = 3.0;
        system.info.wind_motion_end_angle_max = 3.1;
        let ini = ParticleExporter::new().generate_ini_content(&system);
        let keys = [
            "ParticleName",
            "AngleZ",
            "Gravity",
            "Lifetime",
            "SystemLifetime",
            "Size",
            "Alpha1",
            "Color1",
            "ColorScale",
            "DriftVelocity",
            "VelocityType",
            "VolumeType",
            "IsHollow",
            "WindMotion",
            "WindAngleChangeMin",
            "WindAngleChangeMax",
            "WindPingPongStartAngleMin",
            "WindPingPongStartAngleMax",
            "WindPingPongEndAngleMin",
            "WindPingPongEndAngleMax",
            "End",
        ];
        let mut last = 0usize;
        for key in keys {
            let at = ini[last..]
                .find(key)
                .unwrap_or_else(|| panic!("{key} missing after offset {last}\n{ini}"));
            last += at;
        }
        let loaded = ParticleExporter::new()
            .parse_ini_content(&ini)
            .expect("parse wind ping-pong");
        assert_eq!(loaded.info.wind_motion, WindMotion::PingPong);
        assert!((loaded.info.wind_angle_change_min - 0.15).abs() < 1e-5);
        assert!((loaded.info.wind_motion_start_angle_min - 0.1).abs() < 1e-5);
        assert!((loaded.info.wind_motion_start_angle_max - 0.2).abs() < 1e-5);
        assert!((loaded.info.wind_motion_end_angle_min - 3.0).abs() < 1e-5);
        assert!((loaded.info.wind_motion_end_angle_max - 3.1).abs() < 1e-5);
    }

    #[test]
    fn binary_export_writes_cpp_write_single_particle_system_ini() {
        let mut system = ParticleSystem::new("BinDump".to_string()).expect("system");
        system.info.particle_type_name = "EXPtracer".to_string();
        system.info.gravity = 0.5;
        let mut exporter = ParticleExporter::new();
        exporter.export_format = ExportFormat::Binary;
        let dir = std::env::temp_dir().join(format!(
            "particle_editor_bin_{}_{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        ));
        std::fs::create_dir_all(&dir).expect("tmp dir");
        let path = dir.join("BinDump.ini");
        exporter
            .export_particle_system(&system, &path)
            .expect("binary export is C++ INI");
        let written = std::fs::read_to_string(&path).expect("read");
        let expected = exporter.generate_ini_content(&system);
        assert_eq!(written, expected);
        assert!(written.starts_with("ParticleSystem BinDump\n"));
        assert!(written.contains("ParticleName = EXPtracer"));
        assert!(written.contains("End\n"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn binary_raw_export_writes_cpp_particle_system_ini_text() {
        let mut exporter = ParticleExporter::new();
        exporter.export_format = ExportFormat::Binary;
        let dir =
            std::env::temp_dir().join(format!("particle_editor_bin_raw_{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("tmp dir");
        let path = dir.join("RawDump.ini");
        let text = "ParticleSystem RawDump\nParticleName = EXPtracer\nEnd\n";
        exporter.export(text, &path).expect("binary raw INI");
        let written = std::fs::read_to_string(&path).expect("read");
        assert_eq!(written, text);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
