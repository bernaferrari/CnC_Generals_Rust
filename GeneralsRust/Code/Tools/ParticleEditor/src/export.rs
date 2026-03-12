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
                // Implement binary export
                log::info!("Binary export not yet implemented");
                return Err(anyhow::anyhow!("Binary export not implemented"));
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
                log::info!("Binary export not yet implemented");
                return Err(anyhow::anyhow!("Binary export not implemented"));
            }
            ExportFormat::Ini => {
                log::info!("INI export not yet implemented for raw data");
                return Err(anyhow::anyhow!(
                    "INI export requires particle system object"
                ));
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

    fn parse_ini_content(&self, content: &str) -> Result<ParticleSystem> {
        let mut system = ParticleSystem::new("ImportedParticleSystem".to_string())?;

        for line in content.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with(';') {
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
            "IsOneShot" => system.info.is_one_shot = value.parse::<i32>()? != 0,
            "Shader" => system.info.shader_type = self.parse_shader(value)?,
            "ParticleType" => system.info.particle_type = self.parse_particle_type(value)?,
            "ParticleTypeName" => system.info.particle_type_name = value.to_string(),

            // Emission settings
            "EmissionVolumeType" => {
                system.info.emission_volume_type = self.parse_emission_volume_type(value)?
            }
            "EmissionVelocityType" => {
                system.info.emission_velocity_type = self.parse_emission_velocity_type(value)?
            }
            "IsEmissionVolumeHollow" => {
                system.info.is_emission_volume_hollow = value.parse::<i32>()? != 0
            }
            "IsGroundAligned" => system.info.is_ground_aligned = value.parse::<i32>()? != 0,
            "IsEmitAboveGroundOnly" => {
                system.info.is_emit_above_ground_only = value.parse::<i32>()? != 0
            }
            "IsParticleUpTowardsEmitter" => {
                system.info.is_particle_up_towards_emitter = value.parse::<i32>()? != 0
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

            // Slave systems
            "SlaveSystem" => system.info.slave_system_name = value.to_string(),
            "SlavePosOffset" => self.parse_coord3d(value, &mut system.info.slave_pos_offset)?,
            "AttachedSystem" => system.info.attached_system_name = value.to_string(),

            // Emission volume parameters
            "EmissionLineStart" => {
                self.parse_emission_line_start(value, &mut system.info.emission_volume)?
            }
            "EmissionLineEnd" => {
                self.parse_emission_line_end(value, &mut system.info.emission_volume)?
            }
            "EmissionBoxHalfSize" => {
                self.parse_emission_box_half_size(value, &mut system.info.emission_volume)?
            }
            "EmissionSphereRadius" => {
                self.parse_emission_sphere_radius(value, &mut system.info.emission_volume)?
            }
            "EmissionCylinderRadius" => {
                self.parse_emission_cylinder_radius(value, &mut system.info.emission_volume)?
            }
            "EmissionCylinderLength" => {
                self.parse_emission_cylinder_length(value, &mut system.info.emission_volume)?
            }

            // Emission velocity parameters
            "EmissionVelocityOrthoX" => {
                self.parse_emission_velocity_ortho_x(value, &mut system.info.emission_velocity)?
            }
            "EmissionVelocityOrthoY" => {
                self.parse_emission_velocity_ortho_y(value, &mut system.info.emission_velocity)?
            }
            "EmissionVelocityOrthoZ" => {
                self.parse_emission_velocity_ortho_z(value, &mut system.info.emission_velocity)?
            }
            "EmissionVelocitySphericalSpeed" => self.parse_emission_velocity_spherical_speed(
                value,
                &mut system.info.emission_velocity,
            )?,
            "EmissionVelocityHemisphericalSpeed" => self
                .parse_emission_velocity_hemispherical_speed(
                    value,
                    &mut system.info.emission_velocity,
                )?,
            "EmissionVelocityCylindricalRadial" => self
                .parse_emission_velocity_cylindrical_radial(
                    value,
                    &mut system.info.emission_velocity,
                )?,
            "EmissionVelocityCylindricalNormal" => self
                .parse_emission_velocity_cylindrical_normal(
                    value,
                    &mut system.info.emission_velocity,
                )?,
            "EmissionVelocityOutwardSpeed" => self
                .parse_emission_velocity_outward_speed(value, &mut system.info.emission_velocity)?,
            "EmissionVelocityOutwardOtherSpeed" => self
                .parse_emission_velocity_outward_other_speed(
                    value,
                    &mut system.info.emission_velocity,
                )?,

            // Particle parameters
            "Lifetime" => system.info.lifetime = self.parse_random_variable(value)?,
            "StartSize" => system.info.start_size = self.parse_random_variable(value)?,
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

            _ => {
                // Unknown parameter, skip
                log::debug!("Unknown INI parameter: {}", key);
            }
        }
        Ok(())
    }

    fn generate_ini_content(&self, system: &ParticleSystem) -> String {
        let mut content = String::new();

        // Main particle system section
        content.push_str(&format!("ParticleSystem\n"));
        content.push_str(&format!("  Name = {}\n", system.info.name));
        content.push_str(&format!(
            "  Priority = {}\n",
            Self::priority_to_string(system.info.priority)
        ));
        content.push_str(&format!(
            "  IsOneShot = {}\n",
            system.info.is_one_shot as i32
        ));
        content.push_str(&format!(
            "  Shader = {}\n",
            Self::shader_to_string(system.info.shader_type)
        ));
        content.push_str(&format!(
            "  ParticleType = {}\n",
            Self::particle_type_to_string(system.info.particle_type)
        ));
        content.push_str(&format!(
            "  ParticleTypeName = {}\n",
            system.info.particle_type_name
        ));

        // Emission settings
        content.push_str(&format!(
            "  EmissionVolumeType = {}\n",
            Self::emission_volume_to_string(system.info.emission_volume_type)
        ));
        content.push_str(&format!(
            "  EmissionVelocityType = {}\n",
            Self::emission_velocity_to_string(system.info.emission_velocity_type)
        ));
        content.push_str(&format!(
            "  IsEmissionVolumeHollow = {}\n",
            system.info.is_emission_volume_hollow as i32
        ));
        content.push_str(&format!(
            "  IsGroundAligned = {}\n",
            system.info.is_ground_aligned as i32
        ));
        content.push_str(&format!(
            "  IsEmitAboveGroundOnly = {}\n",
            system.info.is_emit_above_ground_only as i32
        ));
        content.push_str(&format!(
            "  IsParticleUpTowardsEmitter = {}\n",
            system.info.is_particle_up_towards_emitter as i32
        ));

        // System lifetime
        content.push_str(&format!(
            "  SystemLifetime = {}\n",
            system.info.system_lifetime
        ));

        // Slave systems
        if !system.info.slave_system_name.is_empty() {
            content.push_str(&format!(
                "  SlaveSystem = {}\n",
                system.info.slave_system_name
            ));
            content.push_str(&format!(
                "  SlavePosOffset = X:{} Y:{} Z:{}\n",
                system.info.slave_pos_offset.x,
                system.info.slave_pos_offset.y,
                system.info.slave_pos_offset.z
            ));
        }

        if !system.info.attached_system_name.is_empty() {
            content.push_str(&format!(
                "  AttachedSystem = {}\n",
                system.info.attached_system_name
            ));
        }

        // Physics
        content.push_str(&format!(
            "  DriftVelocity = X:{} Y:{} Z:{}\n",
            system.info.drift_velocity.x,
            system.info.drift_velocity.y,
            system.info.drift_velocity.z
        ));
        content.push_str(&format!("  Gravity = {}\n", system.info.gravity));

        // Wind
        content.push_str(&format!(
            "  WindMotion = {}\n",
            Self::wind_motion_to_string(system.info.wind_motion)
        ));
        content.push_str(&format!("  WindAngle = {}\n", system.info.wind_angle));
        content.push_str(&format!(
            "  WindAngleChange = {}\n",
            system.info.wind_angle_change
        ));
        content.push_str(&format!(
            "  WindAngleChangeMin = {}\n",
            system.info.wind_angle_change_min
        ));
        content.push_str(&format!(
            "  WindAngleChangeMax = {}\n",
            system.info.wind_angle_change_max
        ));

        content.push_str("End\n\n");

        // Add emission volume specific settings
        content.push_str(&self.generate_emission_volume_ini(&system.info.emission_volume));

        // Add emission velocity specific settings
        content.push_str(&self.generate_emission_velocity_ini(&system.info.emission_velocity));

        // Add particle parameters
        content.push_str(&self.generate_particle_parameters_ini(&system.info));

        content
    }

    fn generate_emission_volume_ini(&self, volume: &EmissionVolumeData) -> String {
        let mut content = String::new();

        match volume {
            EmissionVolumeData::Point => {
                // No additional parameters for point
            }
            EmissionVolumeData::Line { start, end } => {
                content.push_str("  EmissionLineStart = X:");
                content.push_str(&format!("{} Y:{} Z:{}\n", start.x, start.y, start.z));
                content.push_str("  EmissionLineEnd = X:");
                content.push_str(&format!("{} Y:{} Z:{}\n", end.x, end.y, end.z));
            }
            EmissionVolumeData::Box { half_size } => {
                content.push_str("  EmissionBoxHalfSize = X:");
                content.push_str(&format!(
                    "{} Y:{} Z:{}\n",
                    half_size.x, half_size.y, half_size.z
                ));
            }
            EmissionVolumeData::Sphere { radius } => {
                content.push_str(&format!("  EmissionSphereRadius = {}\n", radius));
            }
            EmissionVolumeData::Cylinder { radius, length } => {
                content.push_str(&format!("  EmissionCylinderRadius = {}\n", radius));
                content.push_str(&format!("  EmissionCylinderLength = {}\n", length));
            }
        }

        content
    }

    fn generate_emission_velocity_ini(&self, velocity: &EmissionVelocityData) -> String {
        let mut content = String::new();

        match velocity {
            EmissionVelocityData::Ortho { x, y, z } => {
                content.push_str("  EmissionVelocityOrthoX = ");
                content.push_str(&self.random_var_to_string(x));
                content.push_str("\n");
                content.push_str("  EmissionVelocityOrthoY = ");
                content.push_str(&self.random_var_to_string(y));
                content.push_str("\n");
                content.push_str("  EmissionVelocityOrthoZ = ");
                content.push_str(&self.random_var_to_string(z));
                content.push_str("\n");
            }
            EmissionVelocityData::Spherical { speed } => {
                content.push_str("  EmissionVelocitySphericalSpeed = ");
                content.push_str(&self.random_var_to_string(speed));
                content.push_str("\n");
            }
            EmissionVelocityData::Hemispherical { speed } => {
                content.push_str("  EmissionVelocityHemisphericalSpeed = ");
                content.push_str(&self.random_var_to_string(speed));
                content.push_str("\n");
            }
            EmissionVelocityData::Cylindrical { radial, normal } => {
                content.push_str("  EmissionVelocityCylindricalRadial = ");
                content.push_str(&self.random_var_to_string(radial));
                content.push_str("\n");
                content.push_str("  EmissionVelocityCylindricalNormal = ");
                content.push_str(&self.random_var_to_string(normal));
                content.push_str("\n");
            }
            EmissionVelocityData::Outward { speed, other_speed } => {
                content.push_str("  EmissionVelocityOutwardSpeed = ");
                content.push_str(&self.random_var_to_string(speed));
                content.push_str("\n");
                content.push_str("  EmissionVelocityOutwardOtherSpeed = ");
                content.push_str(&self.random_var_to_string(other_speed));
                content.push_str("\n");
            }
        }

        content
    }

    fn generate_particle_parameters_ini(&self, info: &ParticleSystemInfo) -> String {
        let mut content = String::new();

        // Basic particle parameters
        content.push_str("  Lifetime = ");
        content.push_str(&self.random_var_to_string(&info.lifetime));
        content.push_str("\n");

        content.push_str("  StartSize = ");
        content.push_str(&self.random_var_to_string(&info.start_size));
        content.push_str("\n");

        content.push_str("  SizeRate = ");
        content.push_str(&self.random_var_to_string(&info.size_rate));
        content.push_str("\n");

        content.push_str("  SizeRateDamping = ");
        content.push_str(&self.random_var_to_string(&info.size_rate_damping));
        content.push_str("\n");

        content.push_str("  AngleZ = ");
        content.push_str(&self.random_var_to_string(&info.angle_z));
        content.push_str("\n");

        content.push_str("  AngularRateZ = ");
        content.push_str(&self.random_var_to_string(&info.angular_rate_z));
        content.push_str("\n");

        content.push_str("  AngularDamping = ");
        content.push_str(&self.random_var_to_string(&info.angular_damping));
        content.push_str("\n");

        content.push_str("  VelocityDamping = ");
        content.push_str(&self.random_var_to_string(&info.vel_damping));
        content.push_str("\n");

        content.push_str("  ColorScale = ");
        content.push_str(&self.random_var_to_string(&info.color_scale));
        content.push_str("\n");

        // Burst parameters
        content.push_str("  BurstDelay = ");
        content.push_str(&self.random_var_to_string(&info.burst_delay));
        content.push_str("\n");

        content.push_str("  BurstCount = ");
        content.push_str(&self.random_var_to_string(&info.burst_count));
        content.push_str("\n");

        content.push_str("  InitialDelay = ");
        content.push_str(&self.random_var_to_string(&info.initial_delay));
        content.push_str("\n");

        content
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
        if let EmissionVelocityData::Spherical { speed } = velocity {
            *speed = self.parse_random_variable(value)?;
        }
        Ok(())
    }

    fn parse_emission_velocity_hemispherical_speed(
        &self,
        value: &str,
        velocity: &mut EmissionVelocityData,
    ) -> Result<()> {
        if let EmissionVelocityData::Hemispherical { speed } = velocity {
            *speed = self.parse_random_variable(value)?;
        }
        Ok(())
    }

    fn parse_emission_velocity_cylindrical_radial(
        &self,
        value: &str,
        velocity: &mut EmissionVelocityData,
    ) -> Result<()> {
        if let EmissionVelocityData::Cylindrical { radial, .. } = velocity {
            *radial = self.parse_random_variable(value)?;
        }
        Ok(())
    }

    fn parse_emission_velocity_cylindrical_normal(
        &self,
        value: &str,
        velocity: &mut EmissionVelocityData,
    ) -> Result<()> {
        if let EmissionVelocityData::Cylindrical { normal, .. } = velocity {
            *normal = self.parse_random_variable(value)?;
        }
        Ok(())
    }

    fn parse_emission_velocity_outward_speed(
        &self,
        value: &str,
        velocity: &mut EmissionVelocityData,
    ) -> Result<()> {
        if let EmissionVelocityData::Outward { speed, .. } = velocity {
            *speed = self.parse_random_variable(value)?;
        }
        Ok(())
    }

    fn parse_emission_velocity_outward_other_speed(
        &self,
        value: &str,
        velocity: &mut EmissionVelocityData,
    ) -> Result<()> {
        if let EmissionVelocityData::Outward { other_speed, .. } = velocity {
            *other_speed = self.parse_random_variable(value)?;
        }
        Ok(())
    }
}

impl Default for ParticleExporter {
    fn default() -> Self {
        Self::new()
    }
}
