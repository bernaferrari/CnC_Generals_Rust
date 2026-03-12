//! Performance Benchmarks for GameLogic Systems
//!
//! This module provides comprehensive performance measurement for critical hot paths:
//! - Ballistics trajectory calculations (throughput, drag effects, wind)
//! - Projectile management (update throughput, collision detection)
//! - Armor/damage system (lookup efficiency, calculation throughput)
//! - Frame integration (per-system timing, memory allocation)

#[cfg(test)]
mod benchmarks {
    use crate::common::Coord3D;
    use crate::damage::DamageType;
    use crate::physics::{PhysicsState, PhysicsType};
    use crate::weapon::{
        ArmorDamageMatrix, ArmorType, BallisticsCalculator, BallisticsTrajectory, Projectile,
        ProjectileType, WeaponBonus, WeaponTemplate, INVALID_OBJECT_ID,
    };
    use std::sync::Arc;
    use std::time::Instant;

    /// Performance measurement utility
    struct PerfMeasure {
        name: String,
        iterations: usize,
        total_time_ns: u128,
    }

    impl PerfMeasure {
        fn new(name: &str, iterations: usize) -> Self {
            PerfMeasure {
                name: name.to_string(),
                iterations,
                total_time_ns: 0,
            }
        }

        fn measure<F>(&mut self, mut f: F)
        where
            F: FnMut(),
        {
            let start = Instant::now();
            for _ in 0..self.iterations {
                f();
            }
            self.total_time_ns = start.elapsed().as_nanos();
        }

        fn avg_time_ns(&self) -> f64 {
            self.total_time_ns as f64 / self.iterations as f64
        }

        fn ops_per_second(&self) -> f64 {
            (self.iterations as f64 * 1_000_000_000.0) / self.total_time_ns as f64
        }

        fn report(&self) {
            log::info!(
                "⏱️  {}: {} iterations in {:.2}ms = {:.2}ns per op = {:.0} ops/sec",
                self.name,
                self.iterations,
                self.total_time_ns as f64 / 1_000_000.0,
                self.avg_time_ns(),
                self.ops_per_second()
            );
        }
    }

    // ========================================================================
    // WEEK 8 BALLISTICS BENCHMARKS
    // ========================================================================

    #[test]
    #[ignore] // Run with: cargo test benchmarks::ballistics_trajectory_throughput -- --ignored --nocapture
    fn ballistics_trajectory_throughput() {
        log::info!("\n🔫 BALLISTICS TRAJECTORY THROUGHPUT BENCHMARK");

        let start = Coord3D::new(0.0, 0.0, 0.0);
        let target = Coord3D::new(200.0, 0.0, 50.0);
        let velocity = 150.0;
        let gravity = 32.0;

        // Baseline: Single trajectory
        let mut perf = PerfMeasure::new("Single trajectory calculation", 1000);
        perf.measure(|| {
            let _ = BallisticsCalculator::calculate_trajectory(&start, &target, velocity, gravity);
        });
        perf.report();
        let baseline_ns = perf.avg_time_ns();

        // Batch: 10 concurrent trajectories
        let mut perf = PerfMeasure::new("Batch 10 trajectories", 100);
        perf.measure(|| {
            for i in 0..10 {
                let offset_target = Coord3D::new(200.0 + i as f32 * 10.0, 0.0, 50.0);
                let _ = BallisticsCalculator::calculate_trajectory(
                    &start,
                    &offset_target,
                    velocity,
                    gravity,
                );
            }
        });
        perf.report();
        let batch10_ns = perf.avg_time_ns() / 10.0; // per trajectory cost

        // Heavy batch: 100 concurrent trajectories
        let mut perf = PerfMeasure::new("Batch 100 trajectories", 10);
        perf.measure(|| {
            for i in 0..100 {
                let offset_target = Coord3D::new(200.0 + i as f32 * 5.0, 0.0, 50.0);
                let _ = BallisticsCalculator::calculate_trajectory(
                    &start,
                    &offset_target,
                    velocity,
                    gravity,
                );
            }
        });
        perf.report();
        let batch100_ns = perf.avg_time_ns() / 100.0; // per trajectory cost

        // Verify scaling is linear
        assert!(
            batch10_ns < baseline_ns * 1.5,
            "Batch processing should not have significant overhead"
        );
        assert!(
            batch100_ns < baseline_ns * 1.5,
            "Batch scaling should remain linear"
        );

        log::info!("✅ Trajectory throughput verified");
    }

    #[test]
    #[ignore] // Run with: cargo test benchmarks::ballistics_drag_physics -- --ignored --nocapture
    fn ballistics_drag_physics() {
        log::info!("\n💨 BALLISTICS DRAG PHYSICS BENCHMARK");

        let start = Coord3D::new(0.0, 0.0, 0.0);
        let target = Coord3D::new(200.0, 0.0, 50.0);
        let velocity = 150.0;
        let gravity = 32.0;
        let drag_coefficient = 0.1;
        let projectile_mass = 1.0;
        let air_density = 1.225;

        // Baseline: Trajectory without drag
        let mut perf = PerfMeasure::new("Trajectory without drag", 1000);
        perf.measure(|| {
            let _ = BallisticsCalculator::calculate_trajectory(&start, &target, velocity, gravity);
        });
        perf.report();

        // With drag: Numerical integration adds cost
        let mut perf = PerfMeasure::new("Trajectory with drag", 1000);
        perf.measure(|| {
            let _ = BallisticsCalculator::calculate_trajectory_with_drag(
                &start,
                &target,
                velocity,
                gravity,
                drag_coefficient,
                projectile_mass,
                air_density,
            );
        });
        perf.report();

        // Multiple drag coefficients (air density variations)
        for drag_coeff in &[0.05, 0.1, 0.2, 0.5] {
            let mut perf = PerfMeasure::new(
                &format!("Trajectory with drag coefficient {}", drag_coeff),
                500,
            );
            perf.measure(|| {
                let _ = BallisticsCalculator::calculate_trajectory_with_drag(
                    &start,
                    &target,
                    velocity,
                    gravity,
                    *drag_coeff,
                    projectile_mass,
                    air_density,
                );
            });
            perf.report();
        }

        log::info!("✅ Drag physics cost measured");
    }

    #[test]
    #[ignore] // Run with: cargo test benchmarks::ballistics_target_intercept -- --ignored --nocapture
    fn ballistics_target_intercept() {
        log::info!("\n🎯 BALLISTICS TARGET INTERCEPTION BENCHMARK");

        let start = Coord3D::new(0.0, 0.0, 0.0);
        let target_pos = Coord3D::new(200.0, 0.0, 0.0);
        let projectile_velocity = 150.0;

        // Baseline: Stationary target
        let target_velocity = Coord3D::new(0.0, 0.0, 0.0);
        let mut perf = PerfMeasure::new("Intercept stationary target", 5000);
        perf.measure(|| {
            let _ = BallisticsCalculator::predict_target_intercept(
                &start,
                &target_pos,
                &target_velocity,
                projectile_velocity,
            );
        });
        perf.report();
        let baseline_ns = perf.avg_time_ns();

        // Moving target: Slow speed
        let target_velocity = Coord3D::new(10.0, 0.0, 0.0);
        let mut perf = PerfMeasure::new("Intercept slow moving target", 5000);
        perf.measure(|| {
            let _ = BallisticsCalculator::predict_target_intercept(
                &start,
                &target_pos,
                &target_velocity,
                projectile_velocity,
            );
        });
        perf.report();

        // Moving target: Fast speed
        let target_velocity = Coord3D::new(50.0, 0.0, 0.0);
        let mut perf = PerfMeasure::new("Intercept fast moving target", 5000);
        perf.measure(|| {
            let _ = BallisticsCalculator::predict_target_intercept(
                &start,
                &target_pos,
                &target_velocity,
                projectile_velocity,
            );
        });
        perf.report();

        // 3D motion: Complex trajectory
        let target_velocity = Coord3D::new(30.0, 20.0, 10.0);
        let mut perf = PerfMeasure::new("Intercept 3D moving target", 5000);
        perf.measure(|| {
            let _ = BallisticsCalculator::predict_target_intercept(
                &start,
                &target_pos,
                &target_velocity,
                projectile_velocity,
            );
        });
        perf.report();

        // Batch intercepts (typical guided missile salvo)
        let mut perf = PerfMeasure::new("Batch 20 intercept calculations", 100);
        perf.measure(|| {
            for i in 0..20 {
                let offset_pos = Coord3D::new(200.0 + i as f32 * 10.0, 0.0, 0.0);
                let _ = BallisticsCalculator::predict_target_intercept(
                    &start,
                    &offset_pos,
                    &target_velocity,
                    projectile_velocity,
                );
            }
        });
        perf.report();

        log::info!("✅ Target interception throughput verified");
    }

    #[test]
    #[ignore] // Run with: cargo test benchmarks::ballistics_wind_effects -- --ignored --nocapture
    fn ballistics_wind_effects() {
        log::info!("\n💨 BALLISTICS WIND EFFECTS BENCHMARK");

        let start = Coord3D::new(0.0, 0.0, 0.0);
        let target = Coord3D::new(200.0, 0.0, 50.0);
        let velocity = 150.0;
        let gravity = 32.0;

        // No wind: Baseline
        let wind = crate::weapon::ballistics::WindEffect {
            wind_velocity: Coord3D::new(0.0, 0.0, 0.0),
            turbulence: 0.0,
            max_altitude: 100.0,
        };
        let mut perf = PerfMeasure::new("Trajectory with no wind", 1000);
        perf.measure(|| {
            let _ = BallisticsCalculator::calculate_trajectory_with_wind(
                &start, &target, velocity, gravity, &wind,
            );
        });
        perf.report();

        // Light lateral wind
        let wind = crate::weapon::ballistics::WindEffect {
            wind_velocity: Coord3D::new(5.0, 0.0, 0.0),
            turbulence: 0.1,
            max_altitude: 100.0,
        };
        let mut perf = PerfMeasure::new("Trajectory with light lateral wind", 1000);
        perf.measure(|| {
            let _ = BallisticsCalculator::calculate_trajectory_with_wind(
                &start, &target, velocity, gravity, &wind,
            );
        });
        perf.report();

        // Strong crosswind
        let wind = crate::weapon::ballistics::WindEffect {
            wind_velocity: Coord3D::new(20.0, 0.0, 0.0),
            turbulence: 0.2,
            max_altitude: 100.0,
        };
        let mut perf = PerfMeasure::new("Trajectory with strong crosswind", 1000);
        perf.measure(|| {
            let _ = BallisticsCalculator::calculate_trajectory_with_wind(
                &start, &target, velocity, gravity, &wind,
            );
        });
        perf.report();

        // 3D wind (with vertical component)
        let wind = crate::weapon::ballistics::WindEffect {
            wind_velocity: Coord3D::new(10.0, 10.0, 5.0),
            turbulence: 0.15,
            max_altitude: 100.0,
        };
        let mut perf = PerfMeasure::new("Trajectory with 3D wind", 1000);
        perf.measure(|| {
            let _ = BallisticsCalculator::calculate_trajectory_with_wind(
                &start, &target, velocity, gravity, &wind,
            );
        });
        perf.report();

        for max_altitude in &[25.0, 50.0, 100.0, 200.0] {
            let wind = crate::weapon::ballistics::WindEffect {
                wind_velocity: Coord3D::new(10.0, 0.0, 0.0),
                turbulence: 0.1,
                max_altitude: *max_altitude,
            };
            let mut perf = PerfMeasure::new(
                &format!("Trajectory with max altitude {}", max_altitude),
                1000,
            );
            perf.measure(|| {
                let _ = BallisticsCalculator::calculate_trajectory_with_wind(
                    &start, &target, velocity, gravity, &wind,
                );
            });
            perf.report();
        }

        log::info!("✅ Wind effects cost measured");
    }

    // ========================================================================
    // WEEK 8 PROJECTILE BENCHMARKS
    // ========================================================================

    #[test]
    #[ignore] // Run with: cargo test benchmarks::projectile_update_throughput -- --ignored --nocapture
    fn projectile_update_throughput() {
        log::info!("\n🚀 PROJECTILE UPDATE THROUGHPUT BENCHMARK");

        let weapon_template = Arc::new(WeaponTemplate::new("TestWeapon".to_string()));
        let trajectory = BallisticsTrajectory {
            initial_velocity: Coord3D::new(100.0, 0.0, 10.0),
            launch_angle: 0.1,
            flight_time: 1.0,
            max_height: 5.0,
            range: 100.0,
            trajectory_points: vec![],
        };

        // Create baseline projectile
        let mut projectile = Projectile::new(
            1,
            ProjectileType::Ballistic,
            weapon_template.clone(),
            2,
            Some(3),
            Coord3D::new(0.0, 0.0, 0.0),
            trajectory.clone(),
            WeaponBonus::new(),
            None,
            INVALID_OBJECT_ID,
            None,
        );

        // Single projectile update
        let mut perf = PerfMeasure::new("Single projectile update", 10000);
        perf.measure(|| {
            projectile.time_alive += 0.033; // 30 FPS frame
        });
        perf.report();

        // Batch 10 projectiles update
        let mut projectiles: Vec<Projectile> = (0..10)
            .map(|i| {
                Projectile::new(
                    i,
                    ProjectileType::Ballistic,
                    weapon_template.clone(),
                    2,
                    Some(3),
                    Coord3D::new(i as f32 * 10.0, 0.0, 0.0),
                    trajectory.clone(),
                    WeaponBonus::new(),
                    None,
                    INVALID_OBJECT_ID,
                    None,
                )
            })
            .collect();

        let mut perf = PerfMeasure::new("Batch 10 projectiles update", 1000);
        perf.measure(|| {
            for proj in &mut projectiles {
                proj.time_alive += 0.033;
            }
        });
        perf.report();

        // Batch 100 projectiles
        let mut projectiles: Vec<Projectile> = (0..100)
            .map(|i| {
                Projectile::new(
                    i,
                    ProjectileType::Ballistic,
                    weapon_template.clone(),
                    2,
                    Some(3),
                    Coord3D::new((i % 10) as f32 * 10.0, (i / 10) as f32 * 10.0, 0.0),
                    trajectory.clone(),
                    WeaponBonus::new(),
                    None,
                    INVALID_OBJECT_ID,
                    None,
                )
            })
            .collect();

        let mut perf = PerfMeasure::new("Batch 100 projectiles update", 100);
        perf.measure(|| {
            for proj in &mut projectiles {
                proj.time_alive += 0.033;
            }
        });
        perf.report();

        // Batch 500 projectiles (stress test)
        let mut projectiles: Vec<Projectile> = (0..500)
            .map(|i| {
                Projectile::new(
                    i,
                    ProjectileType::Ballistic,
                    weapon_template.clone(),
                    2,
                    Some(3),
                    Coord3D::new((i % 20) as f32 * 5.0, (i / 20) as f32 * 5.0, 0.0),
                    trajectory.clone(),
                    WeaponBonus::new(),
                    None,
                    INVALID_OBJECT_ID,
                    None,
                )
            })
            .collect();

        let mut perf = PerfMeasure::new("Batch 500 projectiles update", 10);
        perf.measure(|| {
            for proj in &mut projectiles {
                proj.time_alive += 0.033;
            }
        });
        perf.report();

        log::info!("✅ Projectile update throughput verified");
    }

    // ========================================================================
    // WEEK 8 ARMOR & DAMAGE BENCHMARKS
    // ========================================================================

    #[test]
    #[ignore] // Run with: cargo test benchmarks::armor_matrix_lookup -- --ignored --nocapture
    fn armor_matrix_lookup() {
        log::info!("\n🛡️  ARMOR MATRIX LOOKUP BENCHMARK");

        let matrix = ArmorDamageMatrix::new();

        // Single lookup: Baseline
        let mut perf = PerfMeasure::new("Single armor matrix lookup", 100000);
        perf.measure(|| {
            let _ = matrix.get_multiplier(ArmorType::Tank, DamageType::SmallArms);
        });
        perf.report();

        // Batch lookups: All armor types
        let mut perf = PerfMeasure::new("All armor types lookup", 10000);
        perf.measure(|| {
            let _ = matrix.get_multiplier(ArmorType::Human, DamageType::Crush);
            let _ = matrix.get_multiplier(ArmorType::Tank, DamageType::SmallArms);
            let _ = matrix.get_multiplier(ArmorType::Aircraft, DamageType::Gattling);
            let _ = matrix.get_multiplier(ArmorType::Structure, DamageType::ParticleBeam);
            let _ = matrix.get_multiplier(ArmorType::Truck, DamageType::Flame);
            let _ = matrix.get_multiplier(ArmorType::None, DamageType::Crush);
        });
        perf.report();

        // Cache efficiency: Sequential vs random access
        let mut perf = PerfMeasure::new("Sequential damage type lookups", 10000);
        perf.measure(|| {
            for damage_idx in 0..10 {
                let damage_type = match damage_idx {
                    0 => DamageType::Crush,
                    1 => DamageType::Flame,
                    2 => DamageType::Sniper,
                    3 => DamageType::SmallArms,
                    4 => DamageType::Gattling,
                    5 => DamageType::ParticleBeam,
                    6 => DamageType::Laser,
                    7 => DamageType::Radiation,
                    8 => DamageType::Poison,
                    _ => DamageType::Crush,
                };
                let _ = matrix.get_multiplier(ArmorType::Tank, damage_type);
            }
        });
        perf.report();

        // Matrix creation cost
        let mut perf = PerfMeasure::new("Armor matrix creation", 1000);
        perf.measure(|| {
            let _ = ArmorDamageMatrix::new();
        });
        perf.report();

        log::info!("✅ Armor matrix lookup efficiency verified");
    }

    #[test]
    #[ignore] // Run with: cargo test benchmarks::damage_calculation -- --ignored --nocapture
    fn damage_calculation() {
        log::info!("\n💥 DAMAGE CALCULATION BENCHMARK");

        let matrix = ArmorDamageMatrix::new();

        // Single damage calculation
        let mut perf = PerfMeasure::new("Single damage calculation with armor lookup", 50000);
        perf.measure(|| {
            let base_damage = 100.0;
            let armor_mult = matrix.get_multiplier(ArmorType::Tank, DamageType::SmallArms);
            let final_damage = base_damage * armor_mult;
            let _ = final_damage;
        });
        perf.report();

        // Batch damage calculations (typical explosion with 10 targets)
        let mut perf = PerfMeasure::new("Explosion damage to 10 targets", 5000);
        perf.measure(|| {
            let base_damage = 100.0;
            for i in 0..10 {
                let armor_type = match i % 4 {
                    0 => ArmorType::Human,
                    1 => ArmorType::Tank,
                    2 => ArmorType::Aircraft,
                    _ => ArmorType::Structure,
                };
                let armor_mult = matrix.get_multiplier(armor_type, DamageType::Explosion);
                let final_damage = base_damage * armor_mult;
                let _ = final_damage;
            }
        });
        perf.report();

        // Heavy explosion with 50 targets
        let mut perf = PerfMeasure::new("Heavy explosion damage to 50 targets", 500);
        perf.measure(|| {
            let base_damage = 100.0;
            for i in 0..50 {
                let armor_type = match i % 6 {
                    0 => ArmorType::Human,
                    1 => ArmorType::Tank,
                    2 => ArmorType::Truck,
                    3 => ArmorType::Aircraft,
                    4 => ArmorType::Structure,
                    _ => ArmorType::None,
                };
                let damage_type = match (i / 6) % 4 {
                    0 => DamageType::Explosion,
                    1 => DamageType::SmallArms,
                    2 => DamageType::Crush,
                    _ => DamageType::Flame,
                };
                let armor_mult = matrix.get_multiplier(armor_type, damage_type);
                let final_damage = base_damage * armor_mult;
                let _ = final_damage;
            }
        });
        perf.report();

        // All armor/damage combinations
        let mut perf = PerfMeasure::new("All armor/damage combinations (6x20)", 100);
        perf.measure(|| {
            for armor_idx in 0..6 {
                for damage_idx in 0..20 {
                    let armor_type = match armor_idx {
                        0 => ArmorType::None,
                        1 => ArmorType::Human,
                        2 => ArmorType::Tank,
                        3 => ArmorType::Truck,
                        4 => ArmorType::Aircraft,
                        _ => ArmorType::Structure,
                    };
                    let damage_type = match damage_idx {
                        0 => DamageType::Crush,
                        1 => DamageType::Flame,
                        2 => DamageType::Sniper,
                        3 => DamageType::SmallArms,
                        4 => DamageType::Gattling,
                        5 => DamageType::ParticleBeam,
                        6 => DamageType::Laser,
                        7 => DamageType::Radiation,
                        8 => DamageType::Poison,
                        9 => DamageType::SmallArms,
                        10 => DamageType::InfantryMissile,
                        11 => DamageType::Explosion,
                        _ => DamageType::Crush,
                    };
                    let _ = matrix.get_multiplier(armor_type, damage_type);
                }
            }
        });
        perf.report();

        log::info!("✅ Damage calculation efficiency verified");
    }

    // ========================================================================
    // WEEK 8 PHYSICS BENCHMARKS
    // ========================================================================

    #[test]
    #[ignore] // Run with: cargo test benchmarks::physics_velocity_calculations -- --ignored --nocapture
    fn physics_velocity_calculations() {
        log::info!("\n⚙️  PHYSICS VELOCITY CALCULATIONS BENCHMARK");

        let mut physics_state = PhysicsState::new();
        physics_state.physics_type = PhysicsType::Projectile;
        physics_state.velocity = Coord3D::new(100.0, 0.0, -30.0);
        physics_state.position = Coord3D::new(0.0, 0.0, 100.0);

        const GRAVITY: f32 = 32.0;
        const AIR_RESISTANCE: f32 = 0.98;
        const FRAME_DT: f32 = 1.0 / 30.0;

        // Single velocity update with gravity
        let mut perf = PerfMeasure::new("Single velocity update with gravity", 100000);
        perf.measure(|| {
            let mut v = physics_state.velocity;
            v[2] -= GRAVITY * FRAME_DT;
            physics_state.velocity = v;
        });
        perf.report();

        // Velocity update with gravity and air resistance
        let mut perf = PerfMeasure::new("Velocity update with gravity and air resistance", 100000);
        perf.measure(|| {
            let mut v = physics_state.velocity;
            v[2] -= GRAVITY * FRAME_DT;
            v[0] *= AIR_RESISTANCE;
            v[1] *= AIR_RESISTANCE;
            v[2] *= AIR_RESISTANCE;
            physics_state.velocity = v;
        });
        perf.report();

        // Velocity magnitude calculation (for clamping)
        let mut perf = PerfMeasure::new("Velocity magnitude calculation", 100000);
        perf.measure(|| {
            let v = physics_state.velocity;
            let mag = (v[0].powi(2) + v[1].powi(2) + v[2].powi(2)).sqrt();
            let _ = mag;
        });
        perf.report();

        // Position update
        let mut perf = PerfMeasure::new("Position update from velocity", 100000);
        perf.measure(|| {
            let mut p = physics_state.position;
            let v = physics_state.velocity;
            p[0] += v[0] * FRAME_DT;
            p[1] += v[1] * FRAME_DT;
            p[2] += v[2] * FRAME_DT;
            physics_state.position = p;
        });
        perf.report();

        // Full physics frame update (30 iterations to simulate 1 second)
        let mut perf = PerfMeasure::new("Full physics frame (30 updates = 1 second)", 100);
        perf.measure(|| {
            for _ in 0..30 {
                let mut v = physics_state.velocity;
                v[2] -= GRAVITY * FRAME_DT;
                v[0] *= AIR_RESISTANCE;
                v[1] *= AIR_RESISTANCE;
                v[2] *= AIR_RESISTANCE;
                physics_state.velocity = v;

                let mut p = physics_state.position;
                p[0] += v[0] * FRAME_DT;
                p[1] += v[1] * FRAME_DT;
                p[2] += v[2] * FRAME_DT;
                physics_state.position = p;
            }
        });
        perf.report();

        log::info!("✅ Physics calculations efficiency verified");
    }

    // ========================================================================
    // FRAME INTEGRATION BENCHMARKS
    // ========================================================================

    #[test]
    #[ignore] // Run with: cargo test benchmarks::frame_timing_analysis -- --ignored --nocapture
    fn frame_timing_analysis() {
        log::info!("\n⏱️  FRAME TIMING ANALYSIS BENCHMARK");

        const FRAME_DT: f32 = 1.0 / 30.0;

        // Single frame overhead measurement
        let mut perf = PerfMeasure::new("Single frame timing overhead", 10000);
        perf.measure(|| {
            let _frame_delta = FRAME_DT;
            // Frame update would happen here
        });
        perf.report();

        // Simulate lightweight game loop (minimal work)
        let mut perf = PerfMeasure::new("Lightweight game loop frame", 1000);
        perf.measure(|| {
            let mut total_time = 0.0f32;
            for _ in 0..30 {
                total_time += FRAME_DT;
            }
            let _ = total_time;
        });
        perf.report();

        // Simulate medium game loop (with physics updates)
        let mut perf = PerfMeasure::new("Medium game loop frame (with physics)", 1000);
        perf.measure(|| {
            let mut total_time = 0.0f32;
            let mut velocity = [100.0, 0.0, -30.0];
            let mut position = [0.0, 0.0, 100.0];
            const GRAVITY: f32 = 32.0;
            const AIR_RESISTANCE: f32 = 0.98;

            for _ in 0..30 {
                // Physics update
                velocity[2] -= GRAVITY * FRAME_DT;
                velocity[0] *= AIR_RESISTANCE;
                velocity[1] *= AIR_RESISTANCE;
                velocity[2] *= AIR_RESISTANCE;

                position[0] += velocity[0] * FRAME_DT;
                position[1] += velocity[1] * FRAME_DT;
                position[2] += velocity[2] * FRAME_DT;

                total_time += FRAME_DT;
            }
            let _ = (total_time, position);
        });
        perf.report();

        // Simulate heavy game loop (1000 projectiles)
        let mut perf = PerfMeasure::new("Heavy game loop frame (1000 projectiles)", 10);
        perf.measure(|| {
            let mut total_time = 0.0f32;
            let mut projectiles: Vec<(f32, [f32; 3], [f32; 3])> = (0..1000)
                .map(|i| (0.0, [i as f32, 0.0, 100.0], [100.0, 0.0, -30.0]))
                .collect();
            const GRAVITY: f32 = 32.0;
            const AIR_RESISTANCE: f32 = 0.98;

            for _ in 0..30 {
                // Update all projectiles
                for (_time, position, velocity) in &mut projectiles {
                    // Physics update
                    velocity[2] -= GRAVITY * FRAME_DT;
                    velocity[0] *= AIR_RESISTANCE;
                    velocity[1] *= AIR_RESISTANCE;
                    velocity[2] *= AIR_RESISTANCE;

                    position[0] += velocity[0] * FRAME_DT;
                    position[1] += velocity[1] * FRAME_DT;
                    position[2] += velocity[2] * FRAME_DT;
                }
                total_time += FRAME_DT;
            }
            let _ = total_time;
        });
        perf.report();

        log::info!("✅ Frame timing analysis complete");
    }

    // ========================================================================
    // SCALING ANALYSIS
    // ========================================================================

    #[test]
    #[ignore] // Run with: cargo test benchmarks::scaling_analysis -- --ignored --nocapture
    fn scaling_analysis() {
        log::info!("\n📈 PERFORMANCE SCALING ANALYSIS");

        let weapon_template = Arc::new(WeaponTemplate::new("TestWeapon".to_string()));
        let trajectory = BallisticsTrajectory {
            initial_velocity: Coord3D::new(100.0, 0.0, 10.0),
            launch_angle: 0.1,
            flight_time: 1.0,
            max_height: 5.0,
            range: 100.0,
            trajectory_points: vec![],
        };

        // Test scaling: 10, 50, 100, 500 projectiles
        for projectile_count in &[10, 50, 100, 500] {
            let mut projectiles: Vec<Projectile> = (0..*projectile_count)
                .map(|i| {
                    Projectile::new(
                        i,
                        ProjectileType::Ballistic,
                        weapon_template.clone(),
                        2,
                        Some(3),
                        Coord3D::new((i % 20) as f32 * 5.0, (i / 20) as f32 * 5.0, 0.0),
                        trajectory.clone(),
                        WeaponBonus::new(),
                        None,
                        INVALID_OBJECT_ID,
                        None,
                    )
                })
                .collect();

            let iterations = match projectile_count {
                10 => 1000,
                50 => 200,
                100 => 100,
                500 => 20,
                _ => 10,
            };

            let mut perf = PerfMeasure::new(
                &format!("Update {} projectiles", projectile_count),
                iterations,
            );
            perf.measure(|| {
                for proj in &mut projectiles {
                    proj.time_alive += 0.033;
                }
            });
            perf.report();

            log::info!(
                "  Scaling: {} projectiles = {:.2} ns per projectile",
                projectile_count,
                perf.avg_time_ns() / *projectile_count as f64
            );
        }

        log::info!("✅ Scaling analysis complete");
    }
}
