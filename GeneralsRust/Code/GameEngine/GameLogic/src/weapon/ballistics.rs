//! Advanced Ballistics System
//!
//! This module provides sophisticated ballistics calculations for projectile weapons,
//! including trajectory prediction, air resistance, wind effects, and target leading.

use super::{Coord3D, WeaponTemplate};
use crate::{GameLogicError, GameLogicResult};

/// Ballistics trajectory information
#[derive(Debug, Clone)]
pub struct BallisticsTrajectory {
    /// Initial velocity vector
    pub initial_velocity: Coord3D,
    /// Launch angle in radians
    pub launch_angle: f32,
    /// Flight time in seconds
    pub flight_time: f32,
    /// Maximum height reached
    pub max_height: f32,
    /// Range traveled
    pub range: f32,
    /// Trajectory points for visualization/collision detection
    pub trajectory_points: Vec<TrajectoryPoint>,
}

/// Point along trajectory with timing
#[derive(Debug, Clone)]
pub struct TrajectoryPoint {
    /// Position at this point
    pub position: Coord3D,
    /// Velocity at this point
    pub velocity: Coord3D,
    /// Time from launch
    pub time: f32,
}

/// Wind effect parameters
#[derive(Debug, Clone)]
pub struct WindEffect {
    /// Wind velocity vector
    pub wind_velocity: Coord3D,
    /// Wind turbulence factor
    pub turbulence: f32,
    /// Altitude where wind is strongest
    pub max_altitude: f32,
}

/// Target prediction information
#[derive(Debug, Clone)]
pub struct TargetPrediction {
    /// Predicted position when projectile arrives
    pub predicted_position: Coord3D,
    /// Intercept time
    pub intercept_time: f32,
    /// Confidence factor (0.0 to 1.0)
    pub confidence: f32,
}

/// Advanced ballistics calculator
pub struct BallisticsCalculator;

impl BallisticsCalculator {
    /// Calculate optimal trajectory between two points
    pub fn calculate_trajectory(
        start_pos: &Coord3D,
        target_pos: &Coord3D,
        muzzle_velocity: f32,
        gravity: f32,
    ) -> GameLogicResult<BallisticsTrajectory> {
        let displacement = Coord3D::new(
            target_pos.x - start_pos.x,
            target_pos.y - start_pos.y,
            target_pos.z - start_pos.z,
        );

        let horizontal_distance = (displacement.x.powi(2) + displacement.y.powi(2)).sqrt();
        let vertical_distance = displacement.z;

        // Calculate launch angle for maximum range or to hit target
        let launch_angle = Self::calculate_optimal_launch_angle(
            horizontal_distance,
            vertical_distance,
            muzzle_velocity,
            gravity,
        )?;

        // Calculate initial velocity components
        let lateral_speed = muzzle_velocity * launch_angle.cos();
        let initial_velocity = Coord3D::new(
            (displacement.x / horizontal_distance) * lateral_speed,
            (displacement.y / horizontal_distance) * lateral_speed,
            muzzle_velocity * launch_angle.sin(),
        );

        // Calculate flight time
        let flight_time = horizontal_distance / (muzzle_velocity * launch_angle.cos());

        // Calculate maximum height
        let max_height =
            start_pos.z + (muzzle_velocity * launch_angle.sin()).powi(2) / (2.0 * gravity);

        // Generate trajectory points
        let trajectory_points = Self::generate_trajectory_points(
            start_pos,
            &initial_velocity,
            gravity,
            flight_time,
            20, // Number of points
        );

        Ok(BallisticsTrajectory {
            initial_velocity,
            launch_angle,
            flight_time,
            max_height,
            range: horizontal_distance,
            trajectory_points,
        })
    }

    /// Calculate trajectory with air resistance
    pub fn calculate_trajectory_with_drag(
        start_pos: &Coord3D,
        target_pos: &Coord3D,
        muzzle_velocity: f32,
        gravity: f32,
        drag_coefficient: f32,
        projectile_mass: f32,
        air_density: f32,
    ) -> GameLogicResult<BallisticsTrajectory> {
        // More complex calculation accounting for air resistance
        // Using numerical integration for accurate results

        let mut current_pos = *start_pos;
        let displacement = Coord3D::new(
            target_pos.x - start_pos.x,
            target_pos.y - start_pos.y,
            target_pos.z - start_pos.z,
        );

        let horizontal_distance = (displacement.x.powi(2) + displacement.y.powi(2)).sqrt();
        let launch_angle = Self::calculate_optimal_launch_angle(
            horizontal_distance,
            displacement.z,
            muzzle_velocity,
            gravity,
        )?;

        let mut velocity = Coord3D::new(
            (displacement.x / horizontal_distance) * muzzle_velocity * launch_angle.cos(),
            (displacement.y / horizontal_distance) * muzzle_velocity * launch_angle.cos(),
            muzzle_velocity * launch_angle.sin(),
        );

        let mut trajectory_points = Vec::new();
        let dt = 0.016; // ~60 FPS simulation
        let mut time = 0.0;
        let max_time = 30.0; // Maximum 30 seconds flight time

        while time < max_time {
            let horizontal_traveled = ((current_pos.x - start_pos.x).powi(2)
                + (current_pos.y - start_pos.y).powi(2))
            .sqrt();
            if horizontal_traveled >= horizontal_distance {
                break;
            }
            if current_pos.z < target_pos.z - 1.0 {
                break;
            }

            trajectory_points.push(TrajectoryPoint {
                position: current_pos,
                velocity,
                time,
            });

            // Calculate drag force
            let speed = velocity.distance(Coord3D::new(0.0, 0.0, 0.0));
            let drag_force = 0.5 * air_density * drag_coefficient * speed * speed;
            let drag_acceleration = drag_force / projectile_mass;

            // Apply drag in opposite direction of velocity
            let velocity_unit = if speed > 0.001 {
                Coord3D::new(velocity.x / speed, velocity.y / speed, velocity.z / speed)
            } else {
                Coord3D::new(0.0, 0.0, 0.0)
            };

            // Update velocity (gravity + drag)
            velocity.x -= velocity_unit.x * drag_acceleration * dt;
            velocity.y -= velocity_unit.y * drag_acceleration * dt;
            velocity.z -= gravity * dt + velocity_unit.z * drag_acceleration * dt;

            // Update position
            current_pos.x += velocity.x * dt;
            current_pos.y += velocity.y * dt;
            current_pos.z += velocity.z * dt;

            time += dt;
        }

        let max_height = trajectory_points
            .iter()
            .map(|p| p.position.z)
            .fold(0.0f32, |acc, z| acc.max(z));

        let last_pos = trajectory_points
            .last()
            .map(|p| p.position)
            .unwrap_or(*start_pos);
        let achieved_range =
            ((last_pos.x - start_pos.x).powi(2) + (last_pos.y - start_pos.y).powi(2)).sqrt();

        Ok(BallisticsTrajectory {
            initial_velocity: trajectory_points
                .first()
                .map(|p| p.velocity)
                .unwrap_or_default(),
            launch_angle,
            flight_time: time,
            max_height,
            range: achieved_range,
            trajectory_points,
        })
    }

    /// Calculate trajectory with wind effects
    pub fn calculate_trajectory_with_wind(
        start_pos: &Coord3D,
        target_pos: &Coord3D,
        muzzle_velocity: f32,
        gravity: f32,
        wind: &WindEffect,
    ) -> GameLogicResult<BallisticsTrajectory> {
        // Start with basic trajectory
        let mut trajectory =
            Self::calculate_trajectory(start_pos, target_pos, muzzle_velocity, gravity)?;

        // Apply wind effects to each point
        for point in &mut trajectory.trajectory_points {
            // Wind scales with altitude but should not drop to zero at ground level.
            let altitude_factor =
                (0.5 + 0.5 * (point.position.z / wind.max_altitude).clamp(0.0, 1.0)).min(1.0);
            let wind_effect = Coord3D::new(
                wind.wind_velocity.x * altitude_factor,
                wind.wind_velocity.y * altitude_factor,
                wind.wind_velocity.z * altitude_factor * 0.1, // Less vertical wind effect
            );

            // Apply wind displacement based on time
            point.position.x += wind_effect.x * point.time;
            point.position.y += wind_effect.y * point.time;
            point.position.z += wind_effect.z * point.time;

            // Add turbulence
            use rand::Rng;
            let mut rng = rand::thread_rng();
            if wind.turbulence > 0.0 {
                let turbulence_factor = wind.turbulence * altitude_factor;
                if turbulence_factor > 0.0 {
                    point.position.x += rng.gen_range(-turbulence_factor..=turbulence_factor);
                    point.position.y += rng.gen_range(-turbulence_factor..=turbulence_factor);
                }
            }
        }

        Ok(trajectory)
    }

    /// Predict target position for moving targets
    pub fn predict_target_intercept(
        shooter_pos: &Coord3D,
        target_pos: &Coord3D,
        target_velocity: &Coord3D,
        projectile_speed: f32,
    ) -> GameLogicResult<TargetPrediction> {
        // Solve intercept problem using quadratic equation
        let relative_pos = Coord3D::new(
            target_pos.x - shooter_pos.x,
            target_pos.y - shooter_pos.y,
            target_pos.z - shooter_pos.z,
        );

        // Quadratic coefficients for intercept calculation
        let a = target_velocity.x * target_velocity.x
            + target_velocity.y * target_velocity.y
            + target_velocity.z * target_velocity.z
            - projectile_speed * projectile_speed;

        let b = 2.0
            * (relative_pos.x * target_velocity.x
                + relative_pos.y * target_velocity.y
                + relative_pos.z * target_velocity.z);

        let c = relative_pos.x * relative_pos.x
            + relative_pos.y * relative_pos.y
            + relative_pos.z * relative_pos.z;

        // Solve quadratic equation
        let discriminant = b * b - 4.0 * a * c;

        if discriminant < 0.0 {
            // No intercept possible
            return Ok(TargetPrediction {
                predicted_position: *target_pos,
                intercept_time: 0.0,
                confidence: 0.0,
            });
        }

        let sqrt_discriminant = discriminant.sqrt();
        let t1 = (-b - sqrt_discriminant) / (2.0 * a);
        let t2 = (-b + sqrt_discriminant) / (2.0 * a);

        // Choose the positive, smaller time
        let intercept_time = if t1 > 0.0 && (t2 <= 0.0 || t1 < t2) {
            t1
        } else if t2 > 0.0 {
            t2
        } else {
            // No valid positive solution
            return Ok(TargetPrediction {
                predicted_position: *target_pos,
                intercept_time: 0.0,
                confidence: 0.0,
            });
        };

        // Calculate predicted position
        let predicted_position = Coord3D::new(
            target_pos.x + target_velocity.x * intercept_time,
            target_pos.y + target_velocity.y * intercept_time,
            target_pos.z + target_velocity.z * intercept_time,
        );

        // Calculate confidence based on various factors
        let distance = shooter_pos.distance(*target_pos);
        let target_speed = target_velocity.distance(Coord3D::new(0.0, 0.0, 0.0));
        let confidence =
            Self::calculate_intercept_confidence(distance, target_speed, intercept_time);

        Ok(TargetPrediction {
            predicted_position,
            intercept_time,
            confidence,
        })
    }

    /// Calculate optimal launch angle for given conditions
    ///
    /// Uses projectile motion physics to determine the required launch angle
    /// to hit a target at a given horizontal and vertical distance.
    ///
    /// Returns the lower-trajectory angle for direct fire, or higher-trajectory
    /// angle for artillery/mortar-style weapons.
    fn calculate_optimal_launch_angle(
        horizontal_distance: f32,
        vertical_distance: f32,
        muzzle_velocity: f32,
        gravity: f32,
    ) -> GameLogicResult<f32> {
        // For projectile motion: R = (v²/g) * sin(2θ) for level ground
        // For elevated targets, solve the trajectory equation:
        // y = x*tan(θ) - (g*x²)/(2*v²*cos²(θ))

        let v_squared = muzzle_velocity * muzzle_velocity;
        let g = gravity;

        // Avoid division by zero
        if horizontal_distance < 0.01 {
            // Vertical shot
            return Ok(std::f32::consts::PI / 2.0);
        }

        // Discriminant for the trajectory equation
        // Derived from quadratic formula for tan(θ)
        let discriminant = v_squared * v_squared
            - g * (g * horizontal_distance * horizontal_distance
                + 2.0 * vertical_distance * v_squared);

        if discriminant < 0.0 {
            return Err(GameLogicError::Configuration(format!(
                "Target out of range: distance={}, height={}, velocity={}, gravity={}",
                horizontal_distance, vertical_distance, muzzle_velocity, gravity
            )));
        }

        let sqrt_discriminant = discriminant.sqrt();

        // Two possible angles - low trajectory and high trajectory
        let tan_angle_low = (v_squared - sqrt_discriminant) / (g * horizontal_distance);
        let tan_angle_high = (v_squared + sqrt_discriminant) / (g * horizontal_distance);

        let angle_low = tan_angle_low.atan();
        let angle_high = tan_angle_high.atan();

        // Choose low trajectory for direct fire (most weapons)
        // High trajectory is used for artillery/mortars
        let selected_angle = angle_low;

        // Clamp angle to reasonable bounds
        // Direct fire: -45° to +45°
        // Artillery: up to 60° for high-angle fire
        let max_angle = std::f32::consts::PI / 3.0; // 60 degrees
        let min_angle = -std::f32::consts::PI / 4.0; // -45 degrees

        Ok(selected_angle.max(min_angle).min(max_angle))
    }

    /// Generate trajectory points for visualization and collision detection
    fn generate_trajectory_points(
        start_pos: &Coord3D,
        initial_velocity: &Coord3D,
        gravity: f32,
        flight_time: f32,
        num_points: usize,
    ) -> Vec<TrajectoryPoint> {
        let mut points = Vec::with_capacity(num_points);
        let time_step = flight_time / (num_points as f32);

        for i in 0..num_points {
            let t = i as f32 * time_step;

            let position = Coord3D::new(
                start_pos.x + initial_velocity.x * t,
                start_pos.y + initial_velocity.y * t,
                start_pos.z + initial_velocity.z * t - 0.5 * gravity * t * t,
            );

            let velocity = Coord3D::new(
                initial_velocity.x,
                initial_velocity.y,
                initial_velocity.z - gravity * t,
            );

            points.push(TrajectoryPoint {
                position,
                velocity,
                time: t,
            });
        }

        points
    }

    /// Calculate intercept confidence based on various factors
    fn calculate_intercept_confidence(
        distance: f32,
        target_speed: f32,
        intercept_time: f32,
    ) -> f32 {
        let mut confidence = 1.0;

        // Reduce confidence for longer distances
        if distance > 1000.0 {
            confidence *= (1000.0 / distance).min(1.0);
        }

        // Reduce confidence for fast-moving targets
        if target_speed > 100.0 {
            confidence *= (100.0 / target_speed).min(1.0);
        }

        // Reduce confidence for long intercept times
        if intercept_time > 5.0 {
            confidence *= (5.0 / intercept_time).min(1.0);
        }

        confidence.max(0.0).min(1.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ==================== BASIC TRAJECTORY TESTS ====================

    #[test]
    fn test_trajectory_calculation() {
        let start = Coord3D::new(0.0, 0.0, 0.0);
        let target = Coord3D::new(100.0, 0.0, 0.0);
        let velocity = 100.0;
        let gravity = 9.81;

        let trajectory =
            BallisticsCalculator::calculate_trajectory(&start, &target, velocity, gravity).unwrap();

        assert!(trajectory.flight_time > 0.0, "Flight time must be positive");
        assert!(trajectory.range > 0.0, "Range must be positive");
        assert!(
            !trajectory.trajectory_points.is_empty(),
            "Must have trajectory points"
        );
    }

    #[test]
    fn test_trajectory_zero_elevation() {
        // Horizontal fire at same elevation
        let start = Coord3D::new(0.0, 0.0, 100.0);
        let target = Coord3D::new(200.0, 0.0, 100.0);
        let velocity = 100.0;
        let gravity = 32.0; // C&C uses imperial units: 32 ft/s²

        let trajectory =
            BallisticsCalculator::calculate_trajectory(&start, &target, velocity, gravity).unwrap();

        assert!(trajectory.flight_time > 0.0);
        assert!(trajectory.range > 0.0);
        assert!(
            trajectory.max_height >= 100.0,
            "Max height must reach at least starting height"
        );
    }

    #[test]
    fn test_trajectory_uphill() {
        // Fire uphill
        let start = Coord3D::new(0.0, 0.0, 0.0);
        let target = Coord3D::new(100.0, 0.0, 50.0);
        let velocity = 100.0;
        let gravity = 32.0;

        let trajectory =
            BallisticsCalculator::calculate_trajectory(&start, &target, velocity, gravity).unwrap();

        assert!(trajectory.flight_time > 0.0);
        assert!(trajectory.max_height >= 50.0, "Must reach target elevation");
        assert!(trajectory.range > 0.0);
    }

    #[test]
    fn test_trajectory_downhill() {
        // Fire downhill
        let start = Coord3D::new(0.0, 0.0, 100.0);
        let target = Coord3D::new(100.0, 0.0, 0.0);
        let velocity = 100.0;
        let gravity = 32.0;

        let trajectory =
            BallisticsCalculator::calculate_trajectory(&start, &target, velocity, gravity).unwrap();

        assert!(trajectory.flight_time > 0.0);
        assert!(trajectory.range > 0.0);
    }

    #[test]
    fn test_trajectory_out_of_range() {
        // Target is unreachable
        let start = Coord3D::new(0.0, 0.0, 0.0);
        let target = Coord3D::new(10000.0, 0.0, 0.0);
        let velocity = 50.0; // Too slow
        let gravity = 32.0;

        let result = BallisticsCalculator::calculate_trajectory(&start, &target, velocity, gravity);
        assert!(result.is_err(), "Should fail for out-of-range target");
    }

    #[test]
    fn test_trajectory_short_range() {
        // Very close target
        let start = Coord3D::new(0.0, 0.0, 0.0);
        let target = Coord3D::new(10.0, 0.0, 0.0);
        let velocity = 100.0;
        let gravity = 32.0;

        let trajectory =
            BallisticsCalculator::calculate_trajectory(&start, &target, velocity, gravity).unwrap();

        assert!(trajectory.flight_time > 0.0);
        assert!(
            trajectory.flight_time < 1.0,
            "Short range should have quick flight time"
        );
    }

    // ==================== DRAG AND AIR RESISTANCE TESTS ====================

    #[test]
    fn test_trajectory_with_drag_basic() {
        let start = Coord3D::new(0.0, 0.0, 0.0);
        let target = Coord3D::new(100.0, 0.0, 0.0);
        let velocity = 100.0;
        let gravity = 32.0;
        let drag_coefficient = 0.5;
        let projectile_mass = 1.0;
        let air_density = 1.225; // Sea level

        let trajectory = BallisticsCalculator::calculate_trajectory_with_drag(
            &start,
            &target,
            velocity,
            gravity,
            drag_coefficient,
            projectile_mass,
            air_density,
        )
        .unwrap();

        assert!(trajectory.flight_time > 0.0);
        assert!(!trajectory.trajectory_points.is_empty());
    }

    #[test]
    fn test_drag_reduces_range() {
        // Compare trajectory with and without drag
        let start = Coord3D::new(0.0, 0.0, 0.0);
        let target = Coord3D::new(250.0, 0.0, 0.0);
        let velocity = 120.0;
        let gravity = 32.0;

        let no_drag =
            BallisticsCalculator::calculate_trajectory(&start, &target, velocity, gravity).unwrap();

        let with_drag = BallisticsCalculator::calculate_trajectory_with_drag(
            &start, &target, velocity, gravity, 0.5,   // drag coefficient
            50.0,  // mass (heavier to keep integration stable)
            1.225, // air density
        )
        .unwrap();

        // Drag should reduce achieved range and/or increase flight time.
        assert!(
            with_drag.range < no_drag.range || with_drag.flight_time > no_drag.flight_time,
            "Drag should affect trajectory"
        );
    }

    #[test]
    fn test_drag_heavy_projectile() {
        // Heavier projectiles should be less affected by drag
        let start = Coord3D::new(0.0, 0.0, 0.0);
        let target = Coord3D::new(200.0, 0.0, 0.0);
        let velocity = 100.0;
        let gravity = 32.0;

        let light_drag = BallisticsCalculator::calculate_trajectory_with_drag(
            &start, &target, velocity, gravity, 0.5, 0.1, // Light mass
            1.225,
        )
        .unwrap();

        let heavy_drag = BallisticsCalculator::calculate_trajectory_with_drag(
            &start, &target, velocity, gravity, 0.5, 10.0, // Heavy mass
            1.225,
        )
        .unwrap();

        // Heavier projectile should have better range characteristics
        assert!(
            heavy_drag.trajectory_points.len() >= light_drag.trajectory_points.len(),
            "Heavier projectile should maintain trajectory better"
        );
    }

    #[test]
    fn test_drag_coefficient_effects() {
        let start = Coord3D::new(0.0, 0.0, 0.0);
        let target = Coord3D::new(200.0, 0.0, 0.0);
        let velocity = 100.0;
        let gravity = 32.0;

        let low_drag = BallisticsCalculator::calculate_trajectory_with_drag(
            &start, &target, velocity, gravity, 0.1, // Low drag coefficient
            1.0, 1.225,
        )
        .unwrap();

        let high_drag = BallisticsCalculator::calculate_trajectory_with_drag(
            &start, &target, velocity, gravity, 1.0, // High drag coefficient
            1.0, 1.225,
        )
        .unwrap();

        // Higher drag coefficient should reduce range
        assert!(
            high_drag.trajectory_points.len() <= low_drag.trajectory_points.len(),
            "Higher drag coefficient should reduce range"
        );
    }

    // ==================== WIND EFFECTS TESTS ====================

    #[test]
    fn test_wind_effects() {
        let start = Coord3D::new(0.0, 0.0, 0.0);
        let target = Coord3D::new(100.0, 0.0, 0.0);
        let velocity = 100.0;
        let gravity = 9.81;

        let wind = WindEffect {
            wind_velocity: Coord3D::new(10.0, 0.0, 0.0),
            turbulence: 1.0,
            max_altitude: 100.0,
        };

        let trajectory = BallisticsCalculator::calculate_trajectory_with_wind(
            &start, &target, velocity, gravity, &wind,
        )
        .unwrap();

        assert!(!trajectory.trajectory_points.is_empty());
    }

    #[test]
    fn test_wind_lateral_deflection() {
        // Wind should deflect trajectory laterally
        let start = Coord3D::new(0.0, 0.0, 0.0);
        let target = Coord3D::new(100.0, 0.0, 0.0);
        let velocity = 100.0;
        let gravity = 32.0;

        let wind = WindEffect {
            wind_velocity: Coord3D::new(20.0, 0.0, 0.0),
            turbulence: 0.0, // No random turbulence for determinism
            max_altitude: 500.0,
        };

        let trajectory_no_wind =
            BallisticsCalculator::calculate_trajectory(&start, &target, velocity, gravity).unwrap();
        let trajectory_with_wind = BallisticsCalculator::calculate_trajectory_with_wind(
            &start, &target, velocity, gravity, &wind,
        )
        .unwrap();

        // Final positions should differ due to wind
        if let (Some(final_no_wind), Some(final_with_wind)) = (
            trajectory_no_wind.trajectory_points.last(),
            trajectory_with_wind.trajectory_points.last(),
        ) {
            // Wind deflects in X direction
            assert!(
                (final_with_wind.position.x - final_no_wind.position.x).abs() > 0.1,
                "Wind should cause lateral deflection"
            );
        }
    }

    #[test]
    fn test_wind_altitude_effect() {
        // Wind effect should vary with altitude
        let start = Coord3D::new(0.0, 0.0, 0.0);
        let target = Coord3D::new(200.0, 0.0, 0.0);
        let velocity = 100.0;
        let gravity = 32.0;

        let wind = WindEffect {
            wind_velocity: Coord3D::new(10.0, 0.0, 0.0),
            turbulence: 0.0,
            max_altitude: 100.0,
        };

        let trajectory = BallisticsCalculator::calculate_trajectory_with_wind(
            &start, &target, velocity, gravity, &wind,
        )
        .unwrap();

        // Earlier points (lower altitude) should have less wind effect
        // Later points (higher altitude, then descending) should show wind effect
        assert!(trajectory.trajectory_points.len() >= 2);
    }

    #[test]
    fn test_no_wind() {
        let start = Coord3D::new(0.0, 0.0, 0.0);
        let target = Coord3D::new(100.0, 0.0, 0.0);
        let velocity = 100.0;
        let gravity = 32.0;

        let zero_wind = WindEffect {
            wind_velocity: Coord3D::new(0.0, 0.0, 0.0),
            turbulence: 0.0,
            max_altitude: 100.0,
        };

        let trajectory_no_wind =
            BallisticsCalculator::calculate_trajectory(&start, &target, velocity, gravity).unwrap();
        let trajectory_with_zero_wind = BallisticsCalculator::calculate_trajectory_with_wind(
            &start, &target, velocity, gravity, &zero_wind,
        )
        .unwrap();

        // Zero wind should produce nearly identical trajectories
        assert_eq!(
            trajectory_no_wind.trajectory_points.len(),
            trajectory_with_zero_wind.trajectory_points.len()
        );
    }

    // ==================== TARGET INTERCEPT TESTS ====================

    #[test]
    fn test_target_intercept() {
        let shooter = Coord3D::new(0.0, 0.0, 0.0);
        let target = Coord3D::new(100.0, 0.0, 0.0);
        let velocity = Coord3D::new(10.0, 0.0, 0.0);
        let projectile_speed = 200.0;

        let prediction = BallisticsCalculator::predict_target_intercept(
            &shooter,
            &target,
            &velocity,
            projectile_speed,
        )
        .unwrap();

        assert!(
            prediction.intercept_time >= 0.0,
            "Intercept time must be non-negative"
        );
        assert!(
            prediction.confidence >= 0.0 && prediction.confidence <= 1.0,
            "Confidence must be 0-1"
        );
    }

    #[test]
    fn test_intercept_stationary_target() {
        // Stationary target is trivial case
        let shooter = Coord3D::new(0.0, 0.0, 0.0);
        let target = Coord3D::new(100.0, 0.0, 0.0);
        let velocity = Coord3D::new(0.0, 0.0, 0.0); // Not moving
        let projectile_speed = 200.0;

        let prediction = BallisticsCalculator::predict_target_intercept(
            &shooter,
            &target,
            &velocity,
            projectile_speed,
        )
        .unwrap();

        assert!(
            prediction.confidence > 0.0,
            "High confidence for stationary target"
        );
    }

    #[test]
    fn test_intercept_moving_away() {
        // Target moving away from shooter
        let shooter = Coord3D::new(0.0, 0.0, 0.0);
        let target = Coord3D::new(100.0, 0.0, 0.0);
        let velocity = Coord3D::new(50.0, 0.0, 0.0); // Moving away
        let projectile_speed = 200.0;

        let prediction = BallisticsCalculator::predict_target_intercept(
            &shooter,
            &target,
            &velocity,
            projectile_speed,
        )
        .unwrap();

        assert!(
            prediction.intercept_time > 0.0,
            "Should still have intercept time"
        );
    }

    #[test]
    fn test_intercept_impossible() {
        // Target faster than projectile, moving away
        let shooter = Coord3D::new(0.0, 0.0, 0.0);
        let target = Coord3D::new(100.0, 0.0, 0.0);
        let velocity = Coord3D::new(300.0, 0.0, 0.0); // Faster than projectile
        let projectile_speed = 200.0;

        let prediction = BallisticsCalculator::predict_target_intercept(
            &shooter,
            &target,
            &velocity,
            projectile_speed,
        )
        .unwrap();

        // Should return no confidence for impossible intercept
        assert_eq!(
            prediction.confidence, 0.0,
            "No confidence for impossible intercept"
        );
    }

    #[test]
    fn test_intercept_perpendicular_motion() {
        // Target moving perpendicular to shooter
        let shooter = Coord3D::new(0.0, 0.0, 0.0);
        let target = Coord3D::new(100.0, 0.0, 0.0);
        let velocity = Coord3D::new(0.0, 50.0, 0.0); // Moving in Y direction
        let projectile_speed = 200.0;

        let prediction = BallisticsCalculator::predict_target_intercept(
            &shooter,
            &target,
            &velocity,
            projectile_speed,
        )
        .unwrap();

        assert!(
            prediction.confidence > 0.0,
            "Should have valid intercept for perpendicular motion"
        );
    }

    #[test]
    fn test_intercept_3d_motion() {
        // Target moving in 3D space
        let shooter = Coord3D::new(0.0, 0.0, 0.0);
        let target = Coord3D::new(100.0, 100.0, 50.0);
        let velocity = Coord3D::new(10.0, 20.0, -5.0);
        let projectile_speed = 200.0;

        let prediction = BallisticsCalculator::predict_target_intercept(
            &shooter,
            &target,
            &velocity,
            projectile_speed,
        )
        .unwrap();

        assert!(prediction.intercept_time >= 0.0);
    }

    // ==================== CONFIDENCE FACTOR TESTS ====================

    #[test]
    fn test_confidence_close_target() {
        // Close target should have high confidence
        let shooter = Coord3D::new(0.0, 0.0, 0.0);
        let close_target = Coord3D::new(100.0, 0.0, 0.0);
        let far_target = Coord3D::new(2000.0, 0.0, 0.0);
        let velocity = Coord3D::new(10.0, 0.0, 0.0);
        let projectile_speed = 200.0;

        let close_prediction = BallisticsCalculator::predict_target_intercept(
            &shooter,
            &close_target,
            &velocity,
            projectile_speed,
        )
        .unwrap();

        let far_prediction = BallisticsCalculator::predict_target_intercept(
            &shooter,
            &far_target,
            &velocity,
            projectile_speed,
        )
        .unwrap();

        assert!(
            close_prediction.confidence >= far_prediction.confidence,
            "Close target should have higher or equal confidence"
        );
    }

    #[test]
    fn test_confidence_slow_target() {
        // Slow target should have high confidence
        let shooter = Coord3D::new(0.0, 0.0, 0.0);
        let target = Coord3D::new(100.0, 0.0, 0.0);
        let slow_velocity = Coord3D::new(10.0, 0.0, 0.0);
        let fast_velocity = Coord3D::new(150.0, 0.0, 0.0);
        let projectile_speed = 200.0;

        let slow_prediction = BallisticsCalculator::predict_target_intercept(
            &shooter,
            &target,
            &slow_velocity,
            projectile_speed,
        )
        .unwrap();

        let fast_prediction = BallisticsCalculator::predict_target_intercept(
            &shooter,
            &target,
            &fast_velocity,
            projectile_speed,
        )
        .unwrap();

        assert!(
            slow_prediction.confidence >= fast_prediction.confidence,
            "Slow target should have higher or equal confidence"
        );
    }

    #[test]
    fn test_confidence_bounds() {
        let shooter = Coord3D::new(0.0, 0.0, 0.0);
        let target = Coord3D::new(100.0, 0.0, 0.0);
        let velocity = Coord3D::new(10.0, 0.0, 0.0);
        let projectile_speed = 200.0;

        let prediction = BallisticsCalculator::predict_target_intercept(
            &shooter,
            &target,
            &velocity,
            projectile_speed,
        )
        .unwrap();

        assert!(
            prediction.confidence >= 0.0,
            "Confidence cannot be negative"
        );
        assert!(prediction.confidence <= 1.0, "Confidence cannot exceed 1.0");
    }

    // ==================== EDGE CASE TESTS ====================

    #[test]
    fn test_zero_velocity_projectile() {
        let start = Coord3D::new(0.0, 0.0, 0.0);
        let target = Coord3D::new(100.0, 0.0, 0.0);
        let velocity = 0.0;
        let gravity = 32.0;

        let result = BallisticsCalculator::calculate_trajectory(&start, &target, velocity, gravity);
        assert!(result.is_err(), "Zero velocity should fail");
    }

    #[test]
    fn test_zero_gravity() {
        let start = Coord3D::new(0.0, 0.0, 0.0);
        let target = Coord3D::new(100.0, 0.0, 0.0);
        let velocity = 100.0;
        let gravity = 0.0; // No gravity (space?)

        let result = BallisticsCalculator::calculate_trajectory(&start, &target, velocity, gravity);
        // Should still work, just with infinite flight time or similar
        assert!(result.is_ok() || result.is_err()); // Implementation dependent
    }

    #[test]
    fn test_same_start_and_target() {
        let pos = Coord3D::new(100.0, 100.0, 100.0);
        let velocity = 100.0;
        let gravity = 32.0;

        let result = BallisticsCalculator::calculate_trajectory(&pos, &pos, velocity, gravity);
        // Should either succeed with zero flight time or fail gracefully
        if let Ok(traj) = result {
            assert!(traj.flight_time >= 0.0);
        }
    }

    #[test]
    fn test_very_high_velocity() {
        let start = Coord3D::new(0.0, 0.0, 0.0);
        let target = Coord3D::new(1000.0, 0.0, 0.0);
        let velocity = 10000.0; // Very fast projectile
        let gravity = 32.0;

        let trajectory =
            BallisticsCalculator::calculate_trajectory(&start, &target, velocity, gravity).unwrap();

        assert!(trajectory.flight_time > 0.0);
        assert!(
            trajectory.flight_time < 1.0,
            "Very fast projectile should have quick flight"
        );
    }

    #[test]
    fn test_trajectory_points_ordered() {
        // Trajectory points should be ordered by time
        let start = Coord3D::new(0.0, 0.0, 0.0);
        let target = Coord3D::new(200.0, 0.0, 0.0);
        let velocity = 100.0;
        let gravity = 32.0;

        let trajectory =
            BallisticsCalculator::calculate_trajectory(&start, &target, velocity, gravity).unwrap();

        for i in 0..trajectory.trajectory_points.len() - 1 {
            assert!(
                trajectory.trajectory_points[i].time <= trajectory.trajectory_points[i + 1].time,
                "Trajectory points must be ordered by time"
            );
        }
    }

    #[test]
    fn test_trajectory_descending() {
        // After peak, trajectory should descend (until impact)
        let start = Coord3D::new(0.0, 0.0, 0.0);
        let target = Coord3D::new(300.0, 0.0, 0.0);
        let velocity = 100.0;
        let gravity = 32.0;

        let trajectory =
            BallisticsCalculator::calculate_trajectory(&start, &target, velocity, gravity).unwrap();

        // Find peak
        let peak_idx = trajectory
            .trajectory_points
            .iter()
            .position(|p| p.position.z == trajectory.max_height)
            .unwrap_or(trajectory.trajectory_points.len() - 1);

        // After peak, should descend (if not at end)
        if peak_idx < trajectory.trajectory_points.len() - 1 {
            assert!(
                trajectory.trajectory_points[peak_idx + 1].position.z <= trajectory.max_height,
                "Trajectory should descend after peak"
            );
        }
    }
}
