//! # Particle Effect Presets
//!
//! Pre-configured particle system templates for common effects in C&C Generals:
//! - Explosions (small, medium, large, nuclear)
//! - Weapon effects (muzzle flashes, tracers, impacts)
//! - Building destruction effects
//! - Environmental effects (smoke, fire, dust)
//! - Weather effects integration
//!
//! All presets match C++ ParticleSystem templates from INI files.

use super::particle_manager::*;
use nalgebra::Vector3;
use std::sync::Arc;

/// Preset explosion effects
pub mod explosions {
    use super::*;

    /// Small explosion (grenades, small arms)
    pub fn create_small_explosion() -> Arc<ParticleSystemTemplate> {
        let mut template = ParticleSystemTemplate::new("SmallExplosion".to_string());
        let info = template.info_mut();

        // Basic properties
        info.is_one_shot = true;
        info.shader_type = ParticleShaderType::Additive;
        info.particle_type = ParticleType::Particle;
        info.priority = ParticlePriorityType::WeaponExplosion;

        // Emission
        info.burst_count = GameClientRandomVariable::new(20.0, 30.0);
        info.burst_delay = GameClientRandomVariable::new(0.0, 0.0);
        info.initial_delay = GameClientRandomVariable::new(0.0, 0.0);

        // Volume
        info.emission_volume = EmissionVolume::Sphere { radius: 5.0 };
        info.emission_velocity = EmissionVelocity::Spherical {
            speed: GameClientRandomVariable::new(15.0, 25.0),
        };

        // Particle properties
        info.lifetime = GameClientRandomVariable::new(15.0, 25.0); // frames
        info.start_size = GameClientRandomVariable::new(3.0, 6.0);
        info.size_rate = GameClientRandomVariable::new(0.3, 0.5);
        info.size_rate_damping = GameClientRandomVariable::new(0.95, 0.98);

        // Physics
        info.gravity = 0.5;
        info.vel_damping = GameClientRandomVariable::new(0.92, 0.95);

        // Colors - Orange to red to black
        info.color_keys[0] = RGBColorKeyframe {
            color: [1.0, 0.8, 0.3],
            frame: 0,
        };
        info.color_keys[1] = RGBColorKeyframe {
            color: [1.0, 0.3, 0.1],
            frame: 10,
        };
        info.color_keys[2] = RGBColorKeyframe {
            color: [0.2, 0.2, 0.2],
            frame: 20,
        };

        Arc::new(template)
    }

    /// Medium explosion (rockets, vehicle destruction)
    pub fn create_medium_explosion() -> Arc<ParticleSystemTemplate> {
        let mut template = ParticleSystemTemplate::new("MediumExplosion".to_string());
        let info = template.info_mut();

        info.is_one_shot = true;
        info.shader_type = ParticleShaderType::Additive;
        info.particle_type = ParticleType::Particle;
        info.priority = ParticlePriorityType::DeathExplosion;

        // More particles than small explosion
        info.burst_count = GameClientRandomVariable::new(40.0, 60.0);
        info.burst_delay = GameClientRandomVariable::new(0.0, 0.0);

        info.emission_volume = EmissionVolume::Sphere { radius: 10.0 };
        info.emission_velocity = EmissionVelocity::Spherical {
            speed: GameClientRandomVariable::new(20.0, 35.0),
        };

        info.lifetime = GameClientRandomVariable::new(20.0, 35.0);
        info.start_size = GameClientRandomVariable::new(5.0, 10.0);
        info.size_rate = GameClientRandomVariable::new(0.5, 0.8);
        info.size_rate_damping = GameClientRandomVariable::new(0.93, 0.96);

        info.gravity = 0.8;
        info.vel_damping = GameClientRandomVariable::new(0.90, 0.93);

        // Brighter colors
        info.color_keys[0] = RGBColorKeyframe {
            color: [1.0, 0.9, 0.4],
            frame: 0,
        };
        info.color_keys[1] = RGBColorKeyframe {
            color: [1.0, 0.4, 0.1],
            frame: 15,
        };
        info.color_keys[2] = RGBColorKeyframe {
            color: [0.3, 0.1, 0.1],
            frame: 30,
        };

        Arc::new(template)
    }

    /// Large explosion (artillery, heavy ordnance)
    pub fn create_large_explosion() -> Arc<ParticleSystemTemplate> {
        let mut template = ParticleSystemTemplate::new("LargeExplosion".to_string());
        let info = template.info_mut();

        info.is_one_shot = true;
        info.shader_type = ParticleShaderType::Additive;
        info.particle_type = ParticleType::Particle;
        info.priority = ParticlePriorityType::DeathExplosion;

        info.burst_count = GameClientRandomVariable::new(80.0, 120.0);
        info.burst_delay = GameClientRandomVariable::new(0.0, 0.0);

        info.emission_volume = EmissionVolume::Sphere { radius: 20.0 };
        info.emission_velocity = EmissionVelocity::Spherical {
            speed: GameClientRandomVariable::new(25.0, 50.0),
        };

        info.lifetime = GameClientRandomVariable::new(30.0, 50.0);
        info.start_size = GameClientRandomVariable::new(8.0, 15.0);
        info.size_rate = GameClientRandomVariable::new(0.8, 1.2);
        info.size_rate_damping = GameClientRandomVariable::new(0.91, 0.94);

        info.gravity = 1.0;
        info.vel_damping = GameClientRandomVariable::new(0.88, 0.91);

        info.color_keys[0] = RGBColorKeyframe {
            color: [1.0, 1.0, 0.6],
            frame: 0,
        };
        info.color_keys[1] = RGBColorKeyframe {
            color: [1.0, 0.5, 0.1],
            frame: 20,
        };
        info.color_keys[2] = RGBColorKeyframe {
            color: [0.4, 0.2, 0.1],
            frame: 40,
        };

        Arc::new(template)
    }

    /// Nuclear explosion (superweapons)
    pub fn create_nuclear_explosion() -> Arc<ParticleSystemTemplate> {
        let mut template = ParticleSystemTemplate::new("NuclearExplosion".to_string());
        let info = template.info_mut();

        info.is_one_shot = true;
        info.shader_type = ParticleShaderType::Additive;
        info.particle_type = ParticleType::Particle;
        info.priority = ParticlePriorityType::Critical;

        // Massive particle count
        info.burst_count = GameClientRandomVariable::new(200.0, 300.0);
        info.burst_delay = GameClientRandomVariable::new(0.0, 0.0);

        info.emission_volume = EmissionVolume::Sphere { radius: 50.0 };
        info.emission_velocity = EmissionVelocity::Spherical {
            speed: GameClientRandomVariable::new(40.0, 80.0),
        };

        info.lifetime = GameClientRandomVariable::new(60.0, 90.0);
        info.start_size = GameClientRandomVariable::new(15.0, 30.0);
        info.size_rate = GameClientRandomVariable::new(1.5, 2.5);
        info.size_rate_damping = GameClientRandomVariable::new(0.89, 0.92);

        info.gravity = 0.3; // Slower settling
        info.vel_damping = GameClientRandomVariable::new(0.85, 0.88);

        // Bright flash, white to orange to dark
        info.color_keys[0] = RGBColorKeyframe {
            color: [1.0, 1.0, 1.0],
            frame: 0,
        };
        info.color_keys[1] = RGBColorKeyframe {
            color: [1.0, 0.7, 0.2],
            frame: 30,
        };
        info.color_keys[2] = RGBColorKeyframe {
            color: [0.5, 0.3, 0.1],
            frame: 60,
        };

        Arc::new(template)
    }
}

/// Weapon effect presets
pub mod weapons {
    use super::*;

    /// Muzzle flash effect
    pub fn create_muzzle_flash() -> Arc<ParticleSystemTemplate> {
        let mut template = ParticleSystemTemplate::new("MuzzleFlash".to_string());
        let info = template.info_mut();

        info.is_one_shot = true;
        info.shader_type = ParticleShaderType::Additive;
        info.particle_type = ParticleType::Particle;
        info.priority = ParticlePriorityType::WeaponTrail;

        // Quick, small burst
        info.burst_count = GameClientRandomVariable::new(5.0, 8.0);
        info.burst_delay = GameClientRandomVariable::new(0.0, 0.0);

        info.emission_volume = EmissionVolume::Point;
        info.emission_velocity = EmissionVelocity::Hemispherical {
            speed: GameClientRandomVariable::new(5.0, 10.0),
        };

        // Very short lifetime
        info.lifetime = GameClientRandomVariable::new(3.0, 5.0);
        info.start_size = GameClientRandomVariable::new(2.0, 4.0);
        info.size_rate = GameClientRandomVariable::new(0.5, 1.0);

        // Bright yellow-orange
        info.color_keys[0] = RGBColorKeyframe {
            color: [1.0, 0.9, 0.5],
            frame: 0,
        };
        info.color_keys[1] = RGBColorKeyframe {
            color: [1.0, 0.6, 0.2],
            frame: 3,
        };

        Arc::new(template)
    }

    /// Bullet impact sparks
    pub fn create_bullet_impact() -> Arc<ParticleSystemTemplate> {
        let mut template = ParticleSystemTemplate::new("BulletImpact".to_string());
        let info = template.info_mut();

        info.is_one_shot = true;
        info.shader_type = ParticleShaderType::Additive;
        info.particle_type = ParticleType::Particle;
        info.priority = ParticlePriorityType::WeaponExplosion;

        info.burst_count = GameClientRandomVariable::new(10.0, 15.0);

        info.emission_volume = EmissionVolume::Point;
        info.emission_velocity = EmissionVelocity::Hemispherical {
            speed: GameClientRandomVariable::new(15.0, 25.0),
        };

        info.lifetime = GameClientRandomVariable::new(5.0, 10.0);
        info.start_size = GameClientRandomVariable::new(0.5, 1.5);

        info.gravity = 2.0; // Sparks fall quickly
        info.vel_damping = GameClientRandomVariable::new(0.85, 0.90);

        // Yellow-white sparks
        info.color_keys[0] = RGBColorKeyframe {
            color: [1.0, 1.0, 0.8],
            frame: 0,
        };
        info.color_keys[1] = RGBColorKeyframe {
            color: [1.0, 0.7, 0.3],
            frame: 5,
        };

        Arc::new(template)
    }

    /// Shell casing ejection smoke
    pub fn create_shell_casing_smoke() -> Arc<ParticleSystemTemplate> {
        let mut template = ParticleSystemTemplate::new("ShellCasingSmoke".to_string());
        let info = template.info_mut();

        info.is_one_shot = true;
        info.shader_type = ParticleShaderType::Alpha;
        info.particle_type = ParticleType::Particle;
        info.priority = ParticlePriorityType::WeaponTrail;

        info.burst_count = GameClientRandomVariable::new(3.0, 5.0);

        info.emission_volume = EmissionVolume::Point;
        info.emission_velocity = EmissionVelocity::Ortho {
            x: GameClientRandomVariable::new(-3.0, 3.0),
            y: GameClientRandomVariable::new(-3.0, 3.0),
            z: GameClientRandomVariable::new(5.0, 10.0),
        };

        info.lifetime = GameClientRandomVariable::new(15.0, 25.0);
        info.start_size = GameClientRandomVariable::new(1.0, 2.0);
        info.size_rate = GameClientRandomVariable::new(0.1, 0.2);

        info.gravity = -0.2; // Smoke rises
        info.vel_damping = GameClientRandomVariable::new(0.95, 0.98);

        // Gray smoke
        info.color_keys[0] = RGBColorKeyframe {
            color: [0.7, 0.7, 0.7],
            frame: 0,
        };

        // Fade out with alpha
        info.alpha_keys[0] = RandomKeyframe {
            min_value: 0.8,
            max_value: 1.0,
            frame: 0,
        };
        info.alpha_keys[1] = RandomKeyframe {
            min_value: 0.0,
            max_value: 0.1,
            frame: 20,
        };

        Arc::new(template)
    }
}

/// Environmental effect presets
pub mod environment {
    use super::*;

    /// Continuous smoke plume
    pub fn create_smoke_plume() -> Arc<ParticleSystemTemplate> {
        let mut template = ParticleSystemTemplate::new("SmokePlume".to_string());
        let info = template.info_mut();

        info.is_one_shot = false; // Continuous
        info.shader_type = ParticleShaderType::Alpha;
        info.particle_type = ParticleType::Particle;
        info.priority = ParticlePriorityType::SemiConstant;

        info.burst_count = GameClientRandomVariable::new(3.0, 5.0);
        info.burst_delay = GameClientRandomVariable::new(3.0, 5.0);

        info.emission_volume = EmissionVolume::Cylinder {
            radius: 2.0,
            length: 5.0,
        };
        info.emission_velocity = EmissionVelocity::Cylindrical {
            radial: GameClientRandomVariable::new(1.0, 3.0),
            normal: GameClientRandomVariable::new(8.0, 12.0),
        };

        info.lifetime = GameClientRandomVariable::new(60.0, 90.0);
        info.start_size = GameClientRandomVariable::new(3.0, 5.0);
        info.size_rate = GameClientRandomVariable::new(0.2, 0.4);
        info.size_rate_damping = GameClientRandomVariable::new(0.98, 0.99);

        info.gravity = -0.5; // Rises
        info.vel_damping = GameClientRandomVariable::new(0.97, 0.99);
        info.drift_velocity = Vector3::new(0.0, 0.0, 3.0); // Upward drift

        // Dark gray to light gray smoke
        info.color_keys[0] = RGBColorKeyframe {
            color: [0.3, 0.3, 0.3],
            frame: 0,
        };
        info.color_keys[1] = RGBColorKeyframe {
            color: [0.6, 0.6, 0.6],
            frame: 60,
        };

        info.alpha_keys[0] = RandomKeyframe {
            min_value: 0.7,
            max_value: 0.9,
            frame: 0,
        };
        info.alpha_keys[1] = RandomKeyframe {
            min_value: 0.0,
            max_value: 0.1,
            frame: 80,
        };

        Arc::new(template)
    }

    /// Fire effect
    pub fn create_fire() -> Arc<ParticleSystemTemplate> {
        let mut template = ParticleSystemTemplate::new("Fire".to_string());
        let info = template.info_mut();

        info.is_one_shot = false;
        info.shader_type = ParticleShaderType::Additive;
        info.particle_type = ParticleType::Particle;
        info.priority = ParticlePriorityType::Constant;

        info.burst_count = GameClientRandomVariable::new(8.0, 12.0);
        info.burst_delay = GameClientRandomVariable::new(1.0, 2.0);

        info.emission_volume = EmissionVolume::Cylinder {
            radius: 3.0,
            length: 2.0,
        };
        info.emission_velocity = EmissionVelocity::Cylindrical {
            radial: GameClientRandomVariable::new(2.0, 5.0),
            normal: GameClientRandomVariable::new(15.0, 25.0),
        };

        info.lifetime = GameClientRandomVariable::new(15.0, 25.0);
        info.start_size = GameClientRandomVariable::new(2.0, 4.0);
        info.size_rate = GameClientRandomVariable::new(0.3, 0.6);

        info.gravity = -1.5; // Flames rise quickly
        info.vel_damping = GameClientRandomVariable::new(0.92, 0.95);

        // Yellow-orange to red flames
        info.color_keys[0] = RGBColorKeyframe {
            color: [1.0, 0.9, 0.3],
            frame: 0,
        };
        info.color_keys[1] = RGBColorKeyframe {
            color: [1.0, 0.5, 0.1],
            frame: 10,
        };
        info.color_keys[2] = RGBColorKeyframe {
            color: [0.5, 0.1, 0.0],
            frame: 20,
        };

        Arc::new(template)
    }

    /// Ground dust kick-up
    pub fn create_dust_cloud() -> Arc<ParticleSystemTemplate> {
        let mut template = ParticleSystemTemplate::new("DustCloud".to_string());
        let info = template.info_mut();

        info.is_one_shot = true;
        info.shader_type = ParticleShaderType::Alpha;
        info.particle_type = ParticleType::Particle;
        info.priority = ParticlePriorityType::DustTrail;

        info.burst_count = GameClientRandomVariable::new(15.0, 25.0);

        info.emission_volume = EmissionVolume::Box {
            half_size: Vector3::new(5.0, 5.0, 1.0),
        };
        info.emission_velocity = EmissionVelocity::Hemispherical {
            speed: GameClientRandomVariable::new(5.0, 15.0),
        };

        info.lifetime = GameClientRandomVariable::new(30.0, 50.0);
        info.start_size = GameClientRandomVariable::new(3.0, 6.0);
        info.size_rate = GameClientRandomVariable::new(0.2, 0.4);

        info.gravity = -0.3; // Slight rise
        info.vel_damping = GameClientRandomVariable::new(0.93, 0.96);

        // Brown-tan dust
        info.color_keys[0] = RGBColorKeyframe {
            color: [0.7, 0.6, 0.4],
            frame: 0,
        };
        info.color_keys[1] = RGBColorKeyframe {
            color: [0.6, 0.5, 0.3],
            frame: 40,
        };

        info.alpha_keys[0] = RandomKeyframe {
            min_value: 0.6,
            max_value: 0.8,
            frame: 0,
        };
        info.alpha_keys[1] = RandomKeyframe {
            min_value: 0.0,
            max_value: 0.1,
            frame: 45,
        };

        Arc::new(template)
    }
}

/// Building destruction effect presets
pub mod destruction {
    use super::*;

    /// Building collapse dust
    pub fn create_building_collapse_dust() -> Arc<ParticleSystemTemplate> {
        let mut template = ParticleSystemTemplate::new("BuildingCollapseDust".to_string());
        let info = template.info_mut();

        info.is_one_shot = true;
        info.shader_type = ParticleShaderType::Alpha;
        info.particle_type = ParticleType::Particle;
        info.priority = ParticlePriorityType::DeathExplosion;

        info.burst_count = GameClientRandomVariable::new(60.0, 100.0);

        info.emission_volume = EmissionVolume::Box {
            half_size: Vector3::new(15.0, 15.0, 5.0),
        };
        info.emission_velocity = EmissionVelocity::Outward {
            speed: GameClientRandomVariable::new(10.0, 25.0),
            other_speed: GameClientRandomVariable::new(15.0, 30.0),
        };

        info.lifetime = GameClientRandomVariable::new(60.0, 120.0);
        info.start_size = GameClientRandomVariable::new(5.0, 12.0);
        info.size_rate = GameClientRandomVariable::new(0.3, 0.6);

        info.gravity = 0.5;
        info.vel_damping = GameClientRandomVariable::new(0.90, 0.93);

        // Brown-gray debris dust
        info.color_keys[0] = RGBColorKeyframe {
            color: [0.5, 0.4, 0.3],
            frame: 0,
        };
        info.color_keys[1] = RGBColorKeyframe {
            color: [0.6, 0.6, 0.5],
            frame: 80,
        };

        info.alpha_keys[0] = RandomKeyframe {
            min_value: 0.8,
            max_value: 1.0,
            frame: 0,
        };
        info.alpha_keys[1] = RandomKeyframe {
            min_value: 0.0,
            max_value: 0.1,
            frame: 100,
        };

        Arc::new(template)
    }

    /// Debris chunks
    pub fn create_building_debris() -> Arc<ParticleSystemTemplate> {
        let mut template = ParticleSystemTemplate::new("BuildingDebris".to_string());
        let info = template.info_mut();

        info.is_one_shot = true;
        info.shader_type = ParticleShaderType::AlphaTest;
        info.particle_type = ParticleType::Particle;
        info.priority = ParticlePriorityType::DebrisTrail;

        info.burst_count = GameClientRandomVariable::new(30.0, 50.0);

        info.emission_volume = EmissionVolume::Box {
            half_size: Vector3::new(10.0, 10.0, 10.0),
        };
        info.emission_velocity = EmissionVelocity::Spherical {
            speed: GameClientRandomVariable::new(20.0, 40.0),
        };

        info.lifetime = GameClientRandomVariable::new(60.0, 90.0);
        info.start_size = GameClientRandomVariable::new(1.0, 3.0);

        info.gravity = 3.0; // Heavy debris
        info.vel_damping = GameClientRandomVariable::new(0.85, 0.90);

        // Rotating debris
        info.angular_rate_z = GameClientRandomVariable::new(-5.0, 5.0);
        info.angular_damping = GameClientRandomVariable::new(0.98, 0.99);

        // Gray concrete color
        info.color_keys[0] = RGBColorKeyframe {
            color: [0.6, 0.6, 0.6],
            frame: 0,
        };

        Arc::new(template)
    }
}

/// Get preset by name
pub fn get_preset_by_name(name: &str) -> Option<Arc<ParticleSystemTemplate>> {
    match name {
        "SmallExplosion" => Some(explosions::create_small_explosion()),
        "MediumExplosion" => Some(explosions::create_medium_explosion()),
        "LargeExplosion" => Some(explosions::create_large_explosion()),
        "NuclearExplosion" => Some(explosions::create_nuclear_explosion()),

        "MuzzleFlash" => Some(weapons::create_muzzle_flash()),
        "BulletImpact" => Some(weapons::create_bullet_impact()),
        "ShellCasingSmoke" => Some(weapons::create_shell_casing_smoke()),

        "SmokePlume" => Some(environment::create_smoke_plume()),
        "Fire" => Some(environment::create_fire()),
        "DustCloud" => Some(environment::create_dust_cloud()),

        "BuildingCollapseDust" => Some(destruction::create_building_collapse_dust()),
        "BuildingDebris" => Some(destruction::create_building_debris()),

        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_explosion_presets() {
        let small = explosions::create_small_explosion();
        assert_eq!(small.name(), "SmallExplosion");
        assert!(small.info().is_one_shot);

        let nuclear = explosions::create_nuclear_explosion();
        assert_eq!(nuclear.name(), "NuclearExplosion");
        assert_eq!(nuclear.info().priority, ParticlePriorityType::Critical);
    }

    #[test]
    fn test_weapon_presets() {
        let muzzle = weapons::create_muzzle_flash();
        assert_eq!(muzzle.name(), "MuzzleFlash");
        assert_eq!(muzzle.info().shader_type, ParticleShaderType::Additive);

        let impact = weapons::create_bullet_impact();
        assert_eq!(impact.name(), "BulletImpact");
    }

    #[test]
    fn test_environment_presets() {
        let smoke = environment::create_smoke_plume();
        assert_eq!(smoke.name(), "SmokePlume");
        assert!(!smoke.info().is_one_shot); // Continuous

        let fire = environment::create_fire();
        assert_eq!(fire.name(), "Fire");
    }

    #[test]
    fn test_destruction_presets() {
        let dust = destruction::create_building_collapse_dust();
        assert_eq!(dust.name(), "BuildingCollapseDust");

        let debris = destruction::create_building_debris();
        assert_eq!(debris.name(), "BuildingDebris");
    }

    #[test]
    fn test_get_preset_by_name() {
        assert!(get_preset_by_name("SmallExplosion").is_some());
        assert!(get_preset_by_name("MuzzleFlash").is_some());
        assert!(get_preset_by_name("NonExistent").is_none());
    }
}
