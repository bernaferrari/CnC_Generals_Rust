//! # Comprehensive Particle and Effects System Tests
//!
//! Complete test suite for all particle and visual effects systems.
//! Tests particle physics, rendering, FXList integration, weather effects,
//! and performance characteristics.

#[cfg(test)]
mod particle_system_tests {
    use crate::effects::particle_manager::*;
    use crate::effects::particle_system::*;
    use nalgebra::{Point3, Vector3};
    use std::sync::Arc;

    #[test]
    fn test_particle_lifecycle() {
        let template = Arc::new(ParticleSystemTemplate::new("TestSystem".to_string()));
        let system = ParticleSystem::new(template, 1, false);

        assert_eq!(system.system_id(), 1);
        assert_eq!(system.particle_count(), 0);
        assert!(!system.is_destroyed());
    }

    #[test]
    fn test_particle_emission() {
        let mut template = ParticleSystemTemplate::new("EmissionTest".to_string());
        let info = template.info_mut();

        info.burst_count = GameClientRandomVariable::new(10.0, 10.0);
        info.burst_delay = GameClientRandomVariable::new(0.0, 0.0);
        info.lifetime = GameClientRandomVariable::new(30.0, 30.0);

        let template_arc = Arc::new(template);
        let mut system = ParticleSystem::new(template_arc, 1, false);

        system.trigger(); // Force immediate emission
        system.update(0);

        // After trigger and update, should have particles
        assert!(system.particle_count() > 0);
    }

    #[test]
    fn test_particle_physics() {
        let info = ParticleInfo {
            velocity: Vector3::new(10.0, 0.0, 0.0),
            position: Point3::origin(),
            vel_damping: 0.95,
            gravity: 0.0,
            ..Default::default()
        };

        let mut particle = Particle::new(&info, 1, 0);

        let initial_velocity = particle.velocity.clone();
        particle.update();

        // Velocity should be damped
        assert!(particle.velocity.norm() < initial_velocity.norm());

        // Position should have changed
        assert!(particle.position.x > 0.0);
    }

    #[test]
    fn test_particle_gravity() {
        let info = ParticleInfo {
            velocity: Vector3::zeros(),
            position: Point3::new(0.0, 0.0, 100.0),
            vel_damping: 1.0,
            ..Default::default()
        };

        let mut template = ParticleSystemTemplate::new("GravityTest".to_string());
        template.info_mut().gravity = 1.0;

        let template_arc = Arc::new(template);
        let system = ParticleSystem::new(template_arc, 1, false);

        let mut particle = Particle::new(&info, 1, 0);

        let initial_z = particle.position.z;

        // Apply gravity
        particle.apply_force(Vector3::new(0.0, 0.0, -1.0));
        particle.update();

        // Should fall
        assert!(particle.position.z < initial_z);
    }

    #[test]
    fn test_particle_color_animation() {
        let mut info = ParticleInfo::default();

        // Set up color keyframes
        info.color_keys[0] = RGBColorKeyframe {
            color: [1.0, 0.0, 0.0], // Red
            frame: 0,
        };
        info.color_keys[1] = RGBColorKeyframe {
            color: [0.0, 1.0, 0.0], // Green
            frame: 10,
        };

        let mut particle = Particle::new(&info, 1, 0);

        // Initial color should be red
        assert_eq!(particle.color[0], 1.0);
        assert_eq!(particle.color[1], 0.0);

        // Update for 5 frames
        for _ in 0..5 {
            particle.update();
        }

        // Should be transitioning from red to green
        assert!(particle.color[0] < 1.0);
        assert!(particle.color[1] > 0.0);
    }

    #[test]
    fn test_particle_alpha_animation() {
        let mut info = ParticleInfo::default();

        // Fade from opaque to transparent
        info.alpha_keys[0] = Keyframe {
            value: 1.0,
            frame: 0,
        };
        info.alpha_keys[1] = Keyframe {
            value: 0.0,
            frame: 20,
        };

        let mut particle = Particle::new(&info, 1, 0);

        assert_eq!(particle.alpha, 1.0);

        // Update for 10 frames (halfway)
        for _ in 0..10 {
            particle.update();
        }

        // Should be approximately half transparent
        assert!(particle.alpha < 1.0 && particle.alpha > 0.0);
    }
}

#[cfg(test)]
mod particle_manager_tests {
    use crate::effects::particle_manager::*;
    use nalgebra::Point3;
    use std::sync::Arc;

    #[test]
    fn test_manager_creation() {
        let manager = ParticleSystemManager::new();
        assert_eq!(manager.system_count(), 0);
        assert_eq!(manager.particle_count(), 0);
    }

    #[test]
    fn test_template_management() {
        let mut manager = ParticleSystemManager::new();

        let template = manager.new_template("TestTemplate".to_string());
        assert_eq!(template.name(), "TestTemplate");

        let found = manager.find_template("TestTemplate");
        assert!(found.is_some());

        let not_found = manager.find_template("NonExistent");
        assert!(not_found.is_none());
    }

    #[test]
    fn test_system_creation_and_destruction() {
        let mut manager = ParticleSystemManager::new();
        let template = manager.new_template("Test".to_string());

        let system_id = manager.create_particle_system(&template, false).unwrap();
        assert_eq!(manager.system_count(), 1);

        manager.destroy_particle_system(system_id);
        assert_eq!(manager.system_count(), 0);
    }

    #[test]
    fn test_attached_system() {
        let mut manager = ParticleSystemManager::new();
        let template = manager.new_template("Attached".to_string());

        let object_id = 42;
        let system_id = manager
            .create_attached_particle_system(&template, object_id, false)
            .unwrap();

        let system = manager.find_particle_system(system_id).unwrap();
        assert_eq!(system.attached_object(), Some(object_id));

        // Destroy all systems attached to this object
        manager.destroy_attached_systems(object_id);
        assert_eq!(manager.system_count(), 0);
    }

    #[test]
    fn test_manager_update() {
        let mut manager = ParticleSystemManager::new();
        let template = manager.new_template("UpdateTest".to_string());

        manager.create_particle_system(&template, false).unwrap();

        manager.update(0);

        // System should still exist (not destroyed automatically)
        assert_eq!(manager.system_count(), 1);
    }
}

#[cfg(test)]
mod weather_system_tests {
    use crate::effects::weather_complete::*;
    use nalgebra::Point3;

    #[test]
    fn test_rain_system() {
        let mut settings = WeatherSettings::default();
        settings.weather_type = WeatherType::Rain;
        settings.intensity = 0.5;
        settings.spawn_rate = 50.0;

        let mut rain = RainSystem::new(settings);

        rain.update(0.1, Point3::origin());
        assert!(rain.particle_count() > 0);

        rain.set_enabled(false);
        assert_eq!(rain.particle_count(), 0);
    }

    #[test]
    fn test_snow_system() {
        let mut settings = WeatherSettings::default();
        settings.weather_type = WeatherType::Snow;
        settings.intensity = 0.7;
        settings.spawn_rate = 40.0;

        let mut snow = SnowSystem::new(settings);

        snow.update(0.1, Point3::origin());
        assert!(snow.particle_count() > 0);
    }

    #[test]
    fn test_dust_storm_system() {
        let mut settings = WeatherSettings::default();
        settings.weather_type = WeatherType::DustStorm;
        settings.intensity = 0.8;
        settings.spawn_rate = 30.0;

        let mut dust = DustStormSystem::new(settings);

        dust.update(0.1, Point3::origin());
        assert!(dust.particle_count() > 0);

        let visibility = dust.get_visibility_modifier();
        assert!(visibility > 0.0 && visibility <= 1.0);
    }

    #[test]
    fn test_weather_transition() {
        let mut weather = WeatherSystem::new();

        weather.set_weather(WeatherType::Rain, 0.5);
        assert!(weather.is_transitioning());
        assert_eq!(weather.current_weather(), WeatherType::None);

        // Simulate 4 seconds of transitions at 0.1s per frame
        for _ in 0..40 {
            weather.update(0.1, Point3::origin());
        }

        assert!(!weather.is_transitioning());
        assert_eq!(weather.current_weather(), WeatherType::Rain);
    }

    #[test]
    fn test_weather_particle_lifecycle() {
        let spawn_pos = Point3::new(0.0, 0.0, 100.0);
        let wind = Vector3::new(10.0, 0.0, 0.0);

        let mut rain_drop = WeatherParticle::new_rain_drop(spawn_pos, wind);
        assert!(rain_drop.update(0.1, wind, 0.1));

        let mut snowflake = WeatherParticle::new_snowflake(spawn_pos, wind);
        assert!(snowflake.update(0.1, wind, 0.1));

        let mut dust = WeatherParticle::new_dust_particle(spawn_pos, wind);
        assert!(dust.update(0.1, wind, 0.1));
    }
}

#[cfg(test)]
mod particle_presets_tests {
    use crate::effects::particle_presets::*;

    #[test]
    fn test_explosion_presets() {
        let small = explosions::create_small_explosion();
        assert_eq!(small.name(), "SmallExplosion");
        assert!(small.info().is_one_shot);

        let medium = explosions::create_medium_explosion();
        assert_eq!(medium.name(), "MediumExplosion");

        let large = explosions::create_large_explosion();
        assert_eq!(large.name(), "LargeExplosion");

        let nuclear = explosions::create_nuclear_explosion();
        assert_eq!(nuclear.name(), "NuclearExplosion");
        assert_eq!(nuclear.info().priority, ParticlePriorityType::Critical);
    }

    #[test]
    fn test_weapon_presets() {
        let muzzle = weapons::create_muzzle_flash();
        assert_eq!(muzzle.name(), "MuzzleFlash");
        assert_eq!(muzzle.info().shader_type, ParticleShaderType::Additive);
        assert!(muzzle.info().is_one_shot);

        let impact = weapons::create_bullet_impact();
        assert_eq!(impact.name(), "BulletImpact");

        let smoke = weapons::create_shell_casing_smoke();
        assert_eq!(smoke.name(), "ShellCasingSmoke");
        assert_eq!(smoke.info().shader_type, ParticleShaderType::Alpha);
    }

    #[test]
    fn test_environment_presets() {
        let smoke = environment::create_smoke_plume();
        assert_eq!(smoke.name(), "SmokePlume");
        assert!(!smoke.info().is_one_shot); // Continuous

        let fire = environment::create_fire();
        assert_eq!(fire.name(), "Fire");
        assert!(!fire.info().is_one_shot);

        let dust = environment::create_dust_cloud();
        assert_eq!(dust.name(), "DustCloud");
        assert!(dust.info().is_one_shot);
    }

    #[test]
    fn test_destruction_presets() {
        let dust = destruction::create_building_collapse_dust();
        assert_eq!(dust.name(), "BuildingCollapseDust");

        let debris = destruction::create_building_debris();
        assert_eq!(debris.name(), "BuildingDebris");
        assert_eq!(debris.info().shader_type, ParticleShaderType::AlphaTest);
    }

    #[test]
    fn test_preset_lookup() {
        assert!(get_preset_by_name("SmallExplosion").is_some());
        assert!(get_preset_by_name("MuzzleFlash").is_some());
        assert!(get_preset_by_name("SmokePlume").is_some());
        assert!(get_preset_by_name("BuildingDebris").is_some());
        assert!(get_preset_by_name("NonExistent").is_none());
    }
}

#[cfg(test)]
mod fxlist_integration_tests {
    use crate::effects::fxlist_integration::*;
    use crate::effects::particle_manager::*;
    use nalgebra::Point3;

    #[test]
    fn test_particle_system_fx_nugget() {
        let mut nugget = ParticleSystemFXNugget::new("SmallExplosion".to_string());
        nugget.count = 3;
        nugget.radius = GameClientRandomVariable::new(5.0, 10.0);

        assert_eq!(nugget.template_name, "SmallExplosion");
        assert_eq!(nugget.count, 3);
    }

    #[test]
    fn test_fxlist_bridge() {
        let mut bridge = FXListParticleBridge::new();
        let nugget = ParticleSystemFXNugget::new("MuzzleFlash".to_string());

        bridge.register_nugget("TestFX".to_string(), nugget);

        let mut manager = ParticleSystemManager::new();
        let position = Point3::new(10.0, 20.0, 5.0);

        let systems = bridge.execute_fx("TestFX", position, None, &mut manager);
        assert!(!systems.is_empty());

        assert_eq!(bridge.active_system_count(), systems.len());
    }

    #[test]
    fn test_explosion_helper() {
        let mut manager = ParticleSystemManager::new();
        let position = Point3::new(100.0, 200.0, 0.0);

        let system_id = helpers::create_explosion_at(position, "SmallExplosion", &mut manager);
        assert!(system_id.is_some());

        if let Some(id) = system_id {
            let system = manager.find_particle_system(id);
            assert!(system.is_some());

            if let Some(sys) = system {
                assert_eq!(sys.position(), position);
            }
        }
    }

    #[test]
    fn test_weapon_fire_helper() {
        let mut manager = ParticleSystemManager::new();
        let muzzle_pos = Point3::new(10.0, 20.0, 5.0);
        let muzzle_dir = Vector3::new(1.0, 0.0, 0.0);

        let systems = helpers::create_weapon_fire_fx(muzzle_pos, muzzle_dir, &mut manager);

        // Should create at least one system (muzzle flash)
        assert!(!systems.is_empty());
    }

    #[test]
    fn test_building_destruction_helper() {
        let mut manager = ParticleSystemManager::new();
        let building_center = Point3::new(50.0, 50.0, 0.0);

        let systems = helpers::create_building_destruction_fx(building_center, 20.0, &mut manager);

        // Should create multiple systems (explosion, dust, debris)
        assert!(systems.len() >= 2);
    }
}

#[cfg(test)]
mod performance_tests {
    use crate::effects::particle_manager::*;
    use nalgebra::Point3;
    use std::sync::Arc;
    use std::time::Instant;

    #[test]
    fn test_large_particle_count_performance() {
        let mut manager = ParticleSystemManager::new();
        let mut template = ParticleSystemTemplate::new("PerfTest".to_string());
        let info = template.info_mut();

        info.burst_count = GameClientRandomVariable::new(100.0, 100.0);
        info.burst_delay = GameClientRandomVariable::new(0.0, 0.0);
        info.lifetime = GameClientRandomVariable::new(100.0, 100.0);

        let template_arc = Arc::new(template);

        // Create 10 systems with 100 particles each = 1000 particles
        for _ in 0..10 {
            manager.create_particle_system(&template_arc, false).unwrap();
        }

        // Trigger all systems
        for system in manager.all_particle_systems() {
            // Systems would be triggered here
        }

        let start = Instant::now();

        // Update 100 times
        for _ in 0..100 {
            manager.update(0);
        }

        let elapsed = start.elapsed();

        // Should complete in reasonable time (less than 1 second for 100 updates)
        assert!(elapsed.as_secs_f64() < 1.0);

        println!(
            "Performance: {} particles, 100 updates in {:.2}ms",
            manager.particle_count(),
            elapsed.as_secs_f64() * 1000.0
        );
    }

    #[test]
    fn test_system_creation_performance() {
        let mut manager = ParticleSystemManager::new();
        let template = manager.new_template("PerfTest".to_string());

        let start = Instant::now();

        // Create 1000 systems
        for _ in 0..1000 {
            manager.create_particle_system(&template, false).unwrap();
        }

        let elapsed = start.elapsed();

        assert_eq!(manager.system_count(), 1000);

        // Should be fast (less than 100ms)
        assert!(elapsed.as_millis() < 100);

        println!(
            "Created 1000 systems in {:.2}ms",
            elapsed.as_secs_f64() * 1000.0
        );
    }
}
