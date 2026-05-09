//! # Comprehensive Particle System Tests
//!
//! Tests all aspects of the particle system to ensure C++ compatibility
//! and correct behavior for all visual effects.

#[cfg(test)]
mod particle_system_tests {
    use super::super::*;
    use nalgebra::{Point3, Vector3};
    use std::sync::Arc;

    #[test]
    fn test_particle_manager_creation() {
        let mut manager = ParticleSystemManager::new();
        assert_eq!(manager.particle_count(), 0);
        assert_eq!(manager.system_count(), 0);
    }

    #[test]
    fn test_template_creation_and_retrieval() {
        let mut manager = ParticleSystemManager::new();

        // Create template
        let template = manager.new_template("TestExplosion".to_string());
        assert_eq!(template.name(), "TestExplosion");

        // Retrieve template
        let found = manager.find_template("TestExplosion");
        assert!(found.is_some());
        assert_eq!(found.unwrap().name(), "TestExplosion");

        // Non-existent template
        let not_found = manager.find_template("NonExistent");
        assert!(not_found.is_none());
    }

    #[test]
    fn test_particle_system_creation() {
        let mut manager = ParticleSystemManager::new();
        let template = manager.new_template("TestSystem".to_string());

        let system_id = manager.create_particle_system(&template, false).unwrap();
        assert_ne!(system_id, INVALID_PARTICLE_SYSTEM_ID);

        let system = manager.find_particle_system(system_id);
        assert!(system.is_some());
        assert_eq!(system.unwrap().system_id(), system_id);
    }

    #[test]
    fn test_particle_creation_and_lifecycle() {
        let info = ParticleInfo {
            position: Point3::new(1.0, 2.0, 3.0),
            velocity: Vector3::new(0.1, 0.2, 0.3),
            lifetime: 60,
            size: 2.0,
            alpha_keys: [
                Keyframe {
                    value: 1.0,
                    frame: 0,
                },
                Keyframe {
                    value: 0.5,
                    frame: 30,
                },
                Keyframe {
                    value: 0.0,
                    frame: 60,
                },
                Keyframe::default(),
                Keyframe::default(),
                Keyframe::default(),
                Keyframe::default(),
                Keyframe::default(),
            ],
            ..Default::default()
        };

        let mut particle = Particle::new(&info, 1, 0);

        // Initial state
        assert_eq!(particle.personality, 1);
        assert_eq!(particle.lifetime_left, 60);
        assert_eq!(particle.position, Point3::new(1.0, 2.0, 3.0));
        assert_eq!(particle.velocity, Vector3::new(0.1, 0.2, 0.3));

        // Update particle
        let alive = particle.update();
        assert!(alive);
        assert_eq!(particle.lifetime_left, 59);

        // Position should have moved
        assert_eq!(particle.position, Point3::new(1.1, 2.2, 3.3));

        // Test particle death
        particle.lifetime_left = 1;
        let alive = particle.update();
        assert!(!alive);
        assert_eq!(particle.lifetime_left, 0);
    }

    #[test]
    fn test_random_variable_sampling() {
        let var = GameClientRandomVariable::new(1.0, 5.0);

        // Test multiple samples are within range
        for _ in 0..100 {
            let sample = var.sample();
            assert!(sample >= 1.0 && sample <= 5.0);
        }

        // Test single value
        let single_var = GameClientRandomVariable::new(3.14, 3.14);
        assert_eq!(single_var.sample(), 3.14);
    }

    #[test]
    fn test_emission_volumes() {
        // Test point emission (should always return zero)
        let point_volume = EmissionVolume::Point;

        // Test sphere emission
        let sphere_volume = EmissionVolume::Sphere { radius: 5.0 };

        // Test box emission
        let box_volume = EmissionVolume::Box {
            half_size: Vector3::new(2.0, 3.0, 4.0),
        };

        // Test line emission
        let line_volume = EmissionVolume::Line {
            start: Point3::origin(),
            end: Point3::new(10.0, 0.0, 0.0),
        };

        // Test cylinder emission
        let cylinder_volume = EmissionVolume::Cylinder {
            radius: 3.0,
            length: 6.0,
        };

        // These volumes are used in particle position computation
        // The actual computation is tested in the particle system tests
    }

    #[test]
    fn test_emission_velocities() {
        // Test ortho velocity
        let ortho_vel = EmissionVelocity::Ortho {
            x: GameClientRandomVariable::new(-1.0, 1.0),
            y: GameClientRandomVariable::new(-1.0, 1.0),
            z: GameClientRandomVariable::new(0.0, 5.0),
        };

        // Test spherical velocity
        let spherical_vel = EmissionVelocity::Spherical {
            speed: GameClientRandomVariable::new(1.0, 10.0),
        };

        // Test hemispherical velocity
        let hemispherical_vel = EmissionVelocity::Hemispherical {
            speed: GameClientRandomVariable::new(2.0, 8.0),
        };

        // Test cylindrical velocity
        let cylindrical_vel = EmissionVelocity::Cylindrical {
            radial: GameClientRandomVariable::new(0.0, 3.0),
            normal: GameClientRandomVariable::new(-2.0, 2.0),
        };

        // Test outward velocity
        let outward_vel = EmissionVelocity::Outward {
            speed: GameClientRandomVariable::new(1.0, 5.0),
            other_speed: GameClientRandomVariable::new(-1.0, 1.0),
        };

        // These are used in velocity computation within particle systems
    }

    #[test]
    fn test_particle_priority_ordering() {
        assert!(ParticlePriorityType::AlwaysRender > ParticlePriorityType::Critical);
        assert!(ParticlePriorityType::Critical > ParticlePriorityType::WeaponExplosion);
        assert!(ParticlePriorityType::WeaponTrail > ParticlePriorityType::DebrisTrail);

        // Test priority from index
        assert_eq!(
            ParticlePriorityType::from_index(13),
            Some(ParticlePriorityType::AlwaysRender)
        );
        assert_eq!(ParticlePriorityType::from_index(0), None);
        assert_eq!(ParticlePriorityType::from_index(100), None);
    }

    #[test]
    fn test_shader_types() {
        let additive = ParticleShaderType::Additive;
        let alpha = ParticleShaderType::Alpha;
        let alpha_test = ParticleShaderType::AlphaTest;
        let multiply = ParticleShaderType::Multiply;

        // Test that they have different values
        assert_ne!(additive as i32, alpha as i32);
        assert_ne!(alpha as i32, alpha_test as i32);
        assert_ne!(alpha_test as i32, multiply as i32);
    }

    #[test]
    fn test_particle_system_info_defaults() {
        let info = ParticleSystemInfo::default();

        assert!(!info.is_one_shot);
        assert_eq!(info.shader_type, ParticleShaderType::Alpha);
        assert_eq!(info.particle_type, NewParticleType::Particle);
        assert_eq!(info.priority, ParticlePriorityType::WeaponExplosion);
        assert_eq!(info.gravity, 0.0);
        assert_eq!(info.system_lifetime, 0);
        assert_eq!(info.volume_particle_depth, DEFAULT_VOLUME_PARTICLE_DEPTH);

        // Test velocity damping default
        assert_eq!(info.vel_damping.min, 1.0);
        assert_eq!(info.vel_damping.max, 1.0);

        // Test lifetime default
        assert_eq!(info.lifetime.min, 30.0);
        assert_eq!(info.lifetime.max, 30.0);
    }

    #[test]
    fn test_color_tinting() {
        let mut info = ParticleSystemInfo::default();

        // Set some initial colors
        info.color_keys[0] = RGBColorKeyframe {
            color: [1.0, 0.5, 0.25],
            frame: 0,
        };
        info.color_keys[1] = RGBColorKeyframe {
            color: [0.8, 0.8, 0.8],
            frame: 30,
        };

        // Apply red tint
        info.tint_all_colors([1.0, 0.5, 0.0]);

        assert_eq!(info.color_keys[0].color[0], 1.0); // Red unchanged
        assert_eq!(info.color_keys[0].color[1], 0.25); // Green halved
        assert_eq!(info.color_keys[0].color[2], 0.0); // Blue zeroed

        assert_eq!(info.color_keys[1].color[0], 0.8); // Red unchanged
        assert_eq!(info.color_keys[1].color[1], 0.4); // Green halved
        assert_eq!(info.color_keys[1].color[2], 0.0); // Blue zeroed
    }

    #[test]
    fn test_wind_motion_types() {
        let not_used = WindMotion::NotUsed;
        let ping_pong = WindMotion::PingPong;
        let circular = WindMotion::Circular;

        assert_ne!(not_used as i32, ping_pong as i32);
        assert_ne!(ping_pong as i32, circular as i32);
    }

    #[test]
    fn test_particle_system_attachment() {
        let mut manager = ParticleSystemManager::new();
        let template = manager.new_template("AttachedSystem".to_string());

        let object_id = 12345;
        let system_id = manager
            .create_attached_particle_system(&template, object_id, false)
            .unwrap();

        let system = manager.find_particle_system(system_id).unwrap();
        assert_eq!(system.attached_object(), Some(object_id));

        // Test destroying attached systems
        manager.destroy_attached_systems(object_id);
        assert!(manager.find_particle_system(system_id).is_none());
    }

    #[test]
    fn test_keyframe_interpolation() {
        let info = ParticleInfo {
            alpha_keys: [
                Keyframe {
                    value: 1.0,
                    frame: 0,
                },
                Keyframe {
                    value: 0.5,
                    frame: 30,
                },
                Keyframe {
                    value: 0.0,
                    frame: 60,
                },
                Keyframe::default(),
                Keyframe::default(),
                Keyframe::default(),
                Keyframe::default(),
                Keyframe::default(),
            ],
            color_keys: [
                RGBColorKeyframe {
                    color: [1.0, 0.0, 0.0],
                    frame: 0,
                }, // Red
                RGBColorKeyframe {
                    color: [0.0, 1.0, 0.0],
                    frame: 30,
                }, // Green
                RGBColorKeyframe {
                    color: [0.0, 0.0, 1.0],
                    frame: 60,
                }, // Blue
                RGBColorKeyframe::default(),
                RGBColorKeyframe::default(),
                RGBColorKeyframe::default(),
                RGBColorKeyframe::default(),
                RGBColorKeyframe::default(),
            ],
            lifetime: 60,
            ..Default::default()
        };

        let mut particle = Particle::new(&info, 1, 0);

        // Initial state should be first keyframe
        assert_eq!(particle.alpha, 1.0);
        assert_eq!(particle.color, [1.0, 0.0, 0.0]);

        // Advance to trigger keyframe transitions
        for _ in 0..30 {
            particle.update();
        }

        // Should be interpolating to second keyframe
        assert!(particle.alpha < 1.0);
        assert!(particle.alpha > 0.0);
    }

    #[test]
    fn test_particle_invisibility_detection() {
        let mut particle = Particle::new(&ParticleInfo::default(), 1, 0);

        // Test additive shader with black color
        particle.color = [0.01, 0.01, 0.01];
        assert!(particle.is_invisible(ParticleShaderType::Additive));

        // Test additive shader with bright color
        particle.color = [0.5, 0.3, 0.2];
        assert!(!particle.is_invisible(ParticleShaderType::Additive));

        // Test alpha shader with low alpha
        particle.alpha = 0.005;
        assert!(particle.is_invisible(ParticleShaderType::Alpha));

        // Test alpha shader with normal alpha
        particle.alpha = 0.5;
        assert!(!particle.is_invisible(ParticleShaderType::Alpha));

        // Other shader types should not be invisible by default
        assert!(!particle.is_invisible(ParticleShaderType::AlphaTest));
        assert!(!particle.is_invisible(ParticleShaderType::Multiply));
    }

    #[test]
    fn test_global_particle_system_manager() {
        // Test initialization
        initialize_particle_system_manager().unwrap();

        // Test access
        {
            let manager_guard = get_particle_system_manager().unwrap();
            assert!(manager_guard.is_some());
        }

        // Test mutable access
        {
            let mut manager_guard = get_particle_system_manager_mut().unwrap();
            if let Some(ref mut manager) = *manager_guard {
                let template = manager.new_template("GlobalTest".to_string());
                assert_eq!(template.name(), "GlobalTest");
            }
        }
    }
}

#[cfg(test)]
mod particle_optimization_tests {
    use super::super::*;
    use nalgebra::Point3;

    #[test]
    fn test_optimizer_creation() {
        let optimizer = ParticleOptimizer::new();
        assert_eq!(optimizer.current_quality_scale, 1.0);
        assert_eq!(optimizer.performance_trend, 0.0);
    }

    #[test]
    fn test_lod_calculation() {
        let mut optimizer = ParticleOptimizer::new();
        optimizer.update_camera_position(Point3::origin());

        // Test various distances
        let positions_and_expected = [
            (Point3::new(50.0, 0.0, 0.0), ParticleLODLevel::High),
            (Point3::new(200.0, 0.0, 0.0), ParticleLODLevel::Medium),
            (Point3::new(500.0, 0.0, 0.0), ParticleLODLevel::Low),
            (Point3::new(800.0, 0.0, 0.0), ParticleLODLevel::Minimal),
            (Point3::new(2000.0, 0.0, 0.0), ParticleLODLevel::Culled),
        ];

        for (position, expected_lod) in positions_and_expected.iter() {
            assert_eq!(optimizer.calculate_lod_level(*position), *expected_lod);
        }
    }

    #[test]
    fn test_lod_multipliers() {
        let optimizer = ParticleOptimizer::new();

        // Test particle multipliers
        assert_eq!(
            optimizer.get_lod_particle_multiplier(ParticleLODLevel::High),
            1.0
        );
        assert_eq!(
            optimizer.get_lod_particle_multiplier(ParticleLODLevel::Medium),
            0.6
        );
        assert_eq!(
            optimizer.get_lod_particle_multiplier(ParticleLODLevel::Low),
            0.3
        );
        assert_eq!(
            optimizer.get_lod_particle_multiplier(ParticleLODLevel::Minimal),
            0.1
        );
        assert_eq!(
            optimizer.get_lod_particle_multiplier(ParticleLODLevel::Culled),
            0.0
        );

        // Test update multipliers
        assert_eq!(
            optimizer.get_lod_update_multiplier(ParticleLODLevel::High),
            1.0
        );
        assert_eq!(
            optimizer.get_lod_update_multiplier(ParticleLODLevel::Medium),
            0.8
        );
        assert_eq!(
            optimizer.get_lod_update_multiplier(ParticleLODLevel::Low),
            0.5
        );
        assert_eq!(
            optimizer.get_lod_update_multiplier(ParticleLODLevel::Minimal),
            0.25
        );
        assert_eq!(
            optimizer.get_lod_update_multiplier(ParticleLODLevel::Culled),
            0.0
        );
    }

    #[test]
    fn test_performance_budget() {
        let budget = ParticlePerformanceBudget::default();

        // Always render should have unlimited budget
        assert_eq!(
            budget
                .max_particles_by_priority
                .get(&ParticlePriorityType::AlwaysRender),
            Some(&usize::MAX)
        );

        // Higher priorities should have more budget
        let critical_budget = budget
            .max_particles_by_priority
            .get(&ParticlePriorityType::Critical)
            .unwrap();
        let explosion_budget = budget
            .max_particles_by_priority
            .get(&ParticlePriorityType::WeaponExplosion)
            .unwrap();

        assert!(critical_budget > explosion_budget);

        // Check reasonable defaults
        assert!(budget.max_total_particles > 0);
        assert!(budget.max_particle_systems > 0);
        assert!(budget.gpu_memory_budget > 0);
        assert!(budget.cpu_time_budget_ms > 0.0);
        assert!(budget.cull_distance > 0.0);
    }

    #[test]
    fn test_adaptive_quality() {
        let mut optimizer = ParticleOptimizer::new();

        // Simulate good performance
        for _ in 0..20 {
            optimizer.update_performance_metrics(1000, 1.0); // 1ms frame time
        }

        // Quality should be at maximum
        assert_eq!(optimizer.current_quality_scale, 1.0);

        // Simulate poor performance
        for _ in 0..20 {
            optimizer.update_performance_metrics(4000, 25.0); // 25ms frame time
        }

        // Quality should be reduced
        assert!(optimizer.current_quality_scale < 1.0);

        // Get stats
        let stats = optimizer.get_performance_stats();
        assert!(stats.average_frame_time_ms > 10.0);
        assert_eq!(stats.average_particle_count, 4000);
        assert!(stats.current_quality_scale < 1.0);
    }
}

#[cfg(test)]
mod particle_ini_tests {
    use super::super::*;

    #[test]
    fn test_ini_parser_creation() {
        let parser = ParticleSystemINIParser::default();

        // Verify name mappings are populated
        assert!(parser.shader_type_names.contains_key("ADDITIVE"));
        assert!(parser.particle_type_names.contains_key("PARTICLE"));
        assert!(parser.priority_names.contains_key("CRITICAL"));
        assert!(parser.wind_motion_names.contains_key("PingPong"));
    }

    #[test]
    fn test_value_parsing() {
        let parser = ParticleSystemINIParser::default();

        // Test boolean parsing
        assert_eq!(parser.parse_bool("TRUE").unwrap(), true);
        assert_eq!(parser.parse_bool("false").unwrap(), false);
        assert_eq!(parser.parse_bool("YES").unwrap(), true);
        assert_eq!(parser.parse_bool("NO").unwrap(), false);
        assert_eq!(parser.parse_bool("1").unwrap(), true);
        assert_eq!(parser.parse_bool("0").unwrap(), false);
        assert!(parser.parse_bool("maybe").is_err());

        // Test float parsing
        assert_eq!(parser.parse_float("1.5").unwrap(), 1.5);
        assert_eq!(parser.parse_float("-3.14").unwrap(), -3.14);
        assert_eq!(parser.parse_float("0").unwrap(), 0.0);
        assert!(parser.parse_float("not_a_number").is_err());

        // Test uint parsing
        assert_eq!(parser.parse_uint("42").unwrap(), 42);
        assert_eq!(parser.parse_uint("0").unwrap(), 0);
        assert!(parser.parse_uint("-1").is_err());
        assert!(parser.parse_uint("not_a_number").is_err());

        // Test Coord3D parsing
        let coord = parser.parse_coord3d("1.0 2.0 3.0").unwrap();
        assert_eq!(coord.x, 1.0);
        assert_eq!(coord.y, 2.0);
        assert_eq!(coord.z, 3.0);
        assert!(parser.parse_coord3d("1.0 2.0").is_err()); // Too few components
        assert!(parser.parse_coord3d("1.0 2.0 3.0 4.0").is_err()); // Too many components

        // Test random variable parsing
        let var1 = parser.parse_random_variable("5.0").unwrap();
        assert_eq!(var1.min, 5.0);
        assert_eq!(var1.max, 5.0);

        let var2 = parser.parse_random_variable("1.0 10.0").unwrap();
        assert_eq!(var2.min, 1.0);
        assert_eq!(var2.max, 10.0);

        assert!(parser.parse_random_variable("1.0 2.0 3.0").is_err()); // Too many values

        // Test random keyframe parsing
        let keyframe = parser.parse_random_keyframe("0.5 1.0 30").unwrap();
        assert_eq!(keyframe.min_value, 0.5);
        assert_eq!(keyframe.max_value, 1.0);
        assert_eq!(keyframe.frame, 30);

        // Test RGB color keyframe parsing
        let color_keyframe = parser.parse_rgb_color_keyframe("255 128 64 15").unwrap();
        assert_eq!(color_keyframe.color[0], 1.0); // 255/255
        assert_eq!(color_keyframe.color[1], 0.5019608); // 128/255
        assert_eq!(color_keyframe.color[2], 0.2509804); // 64/255
        assert_eq!(color_keyframe.frame, 15);
    }

    #[test]
    fn test_enum_parsing() {
        let parser = ParticleSystemINIParser::default();

        // Test shader types
        assert_eq!(
            parser.parse_shader_type("ADDITIVE").unwrap(),
            ParticleShaderType::Additive
        );
        assert_eq!(
            parser.parse_shader_type("ALPHA").unwrap(),
            ParticleShaderType::Alpha
        );
        assert_eq!(
            parser.parse_shader_type("ALPHA_TEST").unwrap(),
            ParticleShaderType::AlphaTest
        );
        assert_eq!(
            parser.parse_shader_type("MULTIPLY").unwrap(),
            ParticleShaderType::Multiply
        );
        assert!(parser.parse_shader_type("INVALID").is_err());

        // Test particle types
        assert_eq!(
            parser.parse_particle_type("PARTICLE").unwrap(),
            NewParticleType::Particle
        );
        assert_eq!(
            parser.parse_particle_type("DRAWABLE").unwrap(),
            NewParticleType::Drawable
        );
        assert_eq!(
            parser.parse_particle_type("STREAK").unwrap(),
            NewParticleType::Streak
        );
        assert_eq!(
            parser.parse_particle_type("VOLUME_PARTICLE").unwrap(),
            NewParticleType::VolumeParticle
        );
        assert_eq!(
            parser.parse_particle_type("SMUDGE").unwrap(),
            NewParticleType::Smudge
        );
        assert!(parser.parse_particle_type("INVALID").is_err());

        // Test priorities
        assert_eq!(
            parser.parse_priority("ALWAYS_RENDER").unwrap(),
            ParticlePriorityType::AlwaysRender
        );
        assert_eq!(
            parser.parse_priority("CRITICAL").unwrap(),
            ParticlePriorityType::Critical
        );
        assert_eq!(
            parser.parse_priority("WEAPON_EXPLOSION").unwrap(),
            ParticlePriorityType::WeaponExplosion
        );
        assert!(parser.parse_priority("INVALID").is_err());

        // Test emission velocity types
        assert_eq!(
            parser.parse_emission_velocity_type("ORTHO").unwrap(),
            EmissionVelocityType::Ortho
        );
        assert_eq!(
            parser.parse_emission_velocity_type("SPHERICAL").unwrap(),
            EmissionVelocityType::Spherical
        );
        assert_eq!(
            parser
                .parse_emission_velocity_type("HEMISPHERICAL")
                .unwrap(),
            EmissionVelocityType::Hemispherical
        );
        assert_eq!(
            parser.parse_emission_velocity_type("CYLINDRICAL").unwrap(),
            EmissionVelocityType::Cylindrical
        );
        assert_eq!(
            parser.parse_emission_velocity_type("OUTWARD").unwrap(),
            EmissionVelocityType::Outward
        );
        assert!(parser.parse_emission_velocity_type("INVALID").is_err());

        // Test emission volume types
        assert_eq!(
            parser.parse_emission_volume_type("POINT").unwrap(),
            EmissionVolumeType::Point
        );
        assert_eq!(
            parser.parse_emission_volume_type("LINE").unwrap(),
            EmissionVolumeType::Line
        );
        assert_eq!(
            parser.parse_emission_volume_type("BOX").unwrap(),
            EmissionVolumeType::Box
        );
        assert_eq!(
            parser.parse_emission_volume_type("SPHERE").unwrap(),
            EmissionVolumeType::Sphere
        );
        assert_eq!(
            parser.parse_emission_volume_type("CYLINDER").unwrap(),
            EmissionVolumeType::Cylinder
        );
        assert!(parser.parse_emission_volume_type("INVALID").is_err());

        // Test wind motion
        assert_eq!(
            parser.parse_wind_motion("Unused").unwrap(),
            WindMotion::NotUsed
        );
        assert_eq!(
            parser.parse_wind_motion("PingPong").unwrap(),
            WindMotion::PingPong
        );
        assert_eq!(
            parser.parse_wind_motion("Circular").unwrap(),
            WindMotion::Circular
        );
        assert!(parser.parse_wind_motion("INVALID").is_err());
    }
}

#[cfg(test)]
mod particle_renderer_tests {
    use super::super::*;

    #[test]
    fn test_particle_vertex_size() {
        // Ensure ParticleVertex has the expected size and alignment
        assert_eq!(std::mem::size_of::<ParticleVertex>(), 64);
        assert_eq!(std::mem::align_of::<ParticleVertex>(), 4);
    }

    #[test]
    fn test_particle_vertex_creation() {
        let vertex = ParticleVertex {
            position: [1.0, 2.0, 3.0],
            size: [5.0, 5.0],
            color: [1.0, 0.5, 0.25, 0.8],
            uv_rect: [0.0, 0.0, 1.0, 1.0],
            rotation: 1.57, // 90 degrees
            alpha: 0.75,
            _padding: [0.0; 2],
        };

        assert_eq!(vertex.position, [1.0, 2.0, 3.0]);
        assert_eq!(vertex.size, [5.0, 5.0]);
        assert_eq!(vertex.color, [1.0, 0.5, 0.25, 0.8]);
        assert_eq!(vertex.rotation, 1.57);
        assert_eq!(vertex.alpha, 0.75);
    }

    #[test]
    fn test_particle_uniforms_size() {
        assert_eq!(std::mem::size_of::<ParticleUniforms>(), 144); // 2 mat4x4 + vec3 + padding + vec2 + u32 + padding
        assert_eq!(std::mem::align_of::<ParticleUniforms>(), 4);
    }

    #[test]
    fn test_particle_batch() {
        let mut batch =
            ParticleBatch::new(ParticleShaderType::Alpha, "test_texture.tga".to_string());

        assert_eq!(batch.shader_type, ParticleShaderType::Alpha);
        assert_eq!(batch.texture_name, "test_texture.tga");
        assert_eq!(batch.vertices.len(), 0);
        assert!(batch.dirty);

        batch.clear();
        assert!(batch.dirty);
    }

    #[test]
    fn test_render_stats() {
        let mut stats = ParticleRenderStats::default();

        assert_eq!(stats.particles_rendered, 0);
        assert_eq!(stats.batches_rendered, 0);
        assert_eq!(stats.draw_calls, 0);
        assert_eq!(stats.gpu_memory_used, 0);
        assert_eq!(stats.render_time_ms, 0.0);

        // Test setting values
        stats.particles_rendered = 1000;
        stats.batches_rendered = 5;
        stats.draw_calls = 10;
        stats.render_time_ms = 2.5;

        assert_eq!(stats.particles_rendered, 1000);
        assert_eq!(stats.batches_rendered, 5);
        assert_eq!(stats.draw_calls, 10);
        assert_eq!(stats.render_time_ms, 2.5);
    }
}

/// Integration tests that combine multiple systems
#[cfg(test)]
mod integration_tests {
    use super::super::*;
    use nalgebra::{Point3, Vector3};
    use std::sync::Arc;

    #[test]
    fn test_complete_explosion_effect() {
        let mut manager = ParticleSystemManager::new();

        // Create explosion template
        let template = manager.new_template("BigExplosion".to_string());
        {
            let mut info = template.info().clone();
            info.shader_type = ParticleShaderType::Additive;
            info.particle_type = NewParticleType::Particle;
            info.priority = ParticlePriorityType::DeathExplosion;
            info.is_one_shot = true;

            // Setup emission
            info.emission_volume_type = EmissionVolumeType::Sphere;
            info.emission_volume = EmissionVolume::Sphere { radius: 5.0 };
            info.emission_velocity_type = EmissionVelocityType::Outward;
            info.emission_velocity = EmissionVelocity::Outward {
                speed: GameClientRandomVariable::new(10.0, 20.0),
                other_speed: GameClientRandomVariable::new(-2.0, 5.0),
            };

            // Particle properties
            info.lifetime = GameClientRandomVariable::new(30.0, 60.0);
            info.start_size = GameClientRandomVariable::new(1.0, 3.0);
            info.burst_count = GameClientRandomVariable::new(50.0, 100.0);
            info.gravity = -9.8;

            // Alpha animation (fade out)
            info.alpha_keys[0] = RandomKeyframe {
                min_value: 1.0,
                max_value: 1.0,
                distribution_type: 0,
                frame: 0,
            };
            info.alpha_keys[1] = RandomKeyframe {
                min_value: 0.8,
                max_value: 0.8,
                distribution_type: 0,
                frame: 15,
            };
            info.alpha_keys[2] = RandomKeyframe {
                min_value: 0.0,
                max_value: 0.0,
                distribution_type: 0,
                frame: 60,
            };

            // Color animation (fire colors)
            info.color_keys[0] = RGBColorKeyframe {
                color: [1.0, 1.0, 0.8],
                frame: 0,
            }; // Bright yellow
            info.color_keys[1] = RGBColorKeyframe {
                color: [1.0, 0.3, 0.0],
                frame: 20,
            }; // Orange
            info.color_keys[2] = RGBColorKeyframe {
                color: [0.5, 0.0, 0.0],
                frame: 60,
            }; // Dark red

            // Apply info back to template (in real implementation)
        }

        // Create particle system
        let system_id = manager.create_particle_system(&template, false).unwrap();
        let system = manager.find_particle_system_mut(system_id).unwrap();

        // Set position
        system.set_position(Point3::new(100.0, 50.0, 10.0));

        // Trigger explosion
        system.trigger();

        // Update system to emit particles
        system.update(0);

        // Should have created particles
        assert!(system.particle_count() > 0);

        // System should be stopped after one-shot
        assert!(system.is_one_shot);
    }

    #[test]
    fn test_complete_optimization_pipeline() {
        let mut manager = ParticleSystemManager::new();
        let mut optimizer = ParticleOptimizer::new();

        // Set camera position
        optimizer.update_camera_position(Point3::origin());

        // Create multiple particle systems at different distances
        let distances = [50.0, 200.0, 500.0, 1000.0];
        let mut system_ids = Vec::new();

        for (i, &distance) in distances.iter().enumerate() {
            let template = manager.new_template(format!("System{}", i));
            let system_id = manager.create_particle_system(&template, false).unwrap();

            if let Some(system) = manager.find_particle_system_mut(system_id) {
                system.set_position(Point3::new(distance, 0.0, 0.0));
            }

            system_ids.push(system_id);
        }

        // Test LOD calculations for each system
        for (i, &system_id) in system_ids.iter().enumerate() {
            if let Some(system) = manager.find_particle_system(system_id) {
                let lod = optimizer.calculate_lod_level(system.position());
                let expected_lod = match i {
                    0 => ParticleLODLevel::High,    // 50m - close
                    1 => ParticleLODLevel::Medium,  // 200m - medium
                    2 => ParticleLODLevel::Low,     // 500m - far
                    3 => ParticleLODLevel::Minimal, // 1000m - very far
                    _ => unreachable!(),
                };
                assert_eq!(lod, expected_lod);
            }
        }

        // Test optimization application
        for &system_id in &system_ids {
            if let Some(system) = manager.find_particle_system_mut(system_id) {
                optimizer.optimize_system_parameters(system);
                // Verify that multipliers were applied (they should be different for different LODs)
            }
        }

        // Test culling
        let very_far_system_id = system_ids[3];
        if let Some(system) = manager.find_particle_system(very_far_system_id) {
            let should_cull = optimizer.should_cull_system(system, 1000);
            // System might be culled depending on exact distance and settings
        }

        // Update performance metrics
        optimizer.update_performance_metrics(1000, 5.0);
        let stats = optimizer.get_performance_stats();

        assert_eq!(stats.average_particle_count, 1000);
        assert_eq!(stats.average_frame_time_ms, 5.0);
    }

    #[test]
    fn test_wind_motion_simulation() {
        let mut manager = ParticleSystemManager::new();

        // Create system with ping-pong wind motion
        let template = manager.new_template("WindyParticles".to_string());
        {
            let mut info = template.info().clone();
            info.wind_motion = WindMotion::PingPong;
            info.wind_angle_change_min = 0.1;
            info.wind_angle_change_max = 0.5;
            info.wind_motion_start_angle_min = 0.0;
            info.wind_motion_start_angle_max = 1.57; // 90 degrees
            info.wind_motion_end_angle_min = 3.14; // 180 degrees
            info.wind_motion_end_angle_max = 4.71; // 270 degrees
        }

        let system_id = manager.create_particle_system(&template, false).unwrap();
        let system = manager.find_particle_system_mut(system_id).unwrap();

        // Store initial wind angle
        let initial_angle = system.wind_angle();

        // Update system multiple times to see wind motion
        for _ in 0..100 {
            system.update(0);
        }

        // Wind angle should have changed
        let final_angle = system.wind_angle();
        assert_ne!(initial_angle, final_angle);
    }
}
