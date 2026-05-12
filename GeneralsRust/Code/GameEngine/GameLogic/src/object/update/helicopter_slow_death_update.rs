// HelicopterSlowDeathUpdate - Handles the slow death behavior of helicopters
// Author: Peter Sauer, August 2003
// Ported to Rust

use crate::prelude::*;

#[derive(Debug, Clone)]
pub struct HelicopterSlowDeathUpdateModuleData {
    pub min_flight_time: u32,
    pub max_flight_time: u32,
    pub explosion_delay: u32,
    pub max_thrust_angle: f32,
    pub min_thrust: f32,
    pub max_thrust: f32,
    pub forward_velocity_pitch_factor: f32,
    pub lateral_velocity_roll_factor: f32,
}

impl Default for HelicopterSlowDeathUpdateModuleData {
    fn default() -> Self {
        Self {
            min_flight_time: 0,
            max_flight_time: 0,
            explosion_delay: 0,
            max_thrust_angle: 0.5,
            min_thrust: 0.0,
            max_thrust: 100.0,
            forward_velocity_pitch_factor: 0.001,
            lateral_velocity_roll_factor: 0.002,
        }
    }
}

#[derive(Debug, Clone)]
pub struct HelicopterSlowDeathUpdate {
    thing: ThingId,
    module_data: HelicopterSlowDeathUpdateModuleData,
    fall_start_frame: u32,
    death_frame: u32,
    thrust_direction: Coord3D,
    rotation_rate: Coord3D,
}

impl HelicopterSlowDeathUpdate {
    pub fn new(
        thing: ThingId,
        module_data: HelicopterSlowDeathUpdateModuleData,
        ctx: &GameLogicContext<'_>,
    ) -> Self {
        // Calculate random flight time
        let flight_time =
            game_logic_random_value(module_data.min_flight_time, module_data.max_flight_time);
        let death_frame = ctx.get_frame() + flight_time;

        // Initialize random thrust direction
        let angle_xz = game_logic_random_value_real(0.0, 2.0 * std::f32::consts::PI);
        let angle_z = game_logic_random_value_real(
            -module_data.max_thrust_angle,
            module_data.max_thrust_angle,
        );

        let thrust_direction = Coord3D {
            x: angle_xz.cos() * angle_z.cos(),
            y: angle_xz.sin() * angle_z.cos(),
            z: angle_z.sin(),
        };

        // Initialize random rotation rates
        let rotation_rate = Coord3D {
            x: game_logic_random_value_real(-0.05, 0.05),
            y: game_logic_random_value_real(-0.05, 0.05),
            z: game_logic_random_value_real(-0.1, 0.1),
        };

        Self {
            thing,
            module_data,
            fall_start_frame: ctx.get_frame(),
            death_frame,
            thrust_direction,
            rotation_rate,
        }
    }

    pub fn update(&mut self, ctx: &mut UpdateContext<'_>) -> UpdateSleepTime {
        let now = ctx.game_logic.get_frame();

        let Some(object) = ctx.game_logic.find_object_mut(self.thing) else {
            return UpdateSleepTime::Forever;
        };

        // Check if it's time to explode
        if now >= self.death_frame {
            // Explode and die
            object.kill(None, None);
            return UpdateSleepTime::Forever;
        }

        // Calculate thrust based on time remaining
        let time_remaining = (self.death_frame - now) as f32;
        let total_time = (self.death_frame - self.fall_start_frame) as f32;
        let thrust_factor = (time_remaining / total_time).max(0.0).min(1.0);

        let thrust_magnitude = self.module_data.min_thrust
            + (self.module_data.max_thrust - self.module_data.min_thrust) * thrust_factor;

        // Apply thrust force
        if let Some(physics) = object.get_physics_mut() {
            let thrust_force = Coord3D {
                x: self.thrust_direction.x * thrust_magnitude,
                y: self.thrust_direction.y * thrust_magnitude,
                z: self.thrust_direction.z * thrust_magnitude,
            };

            physics.apply_force(&thrust_force);

            // Apply rotational forces
            let current_vel = physics.get_velocity();

            // Pitch based on forward velocity
            let pitch = current_vel.x * self.module_data.forward_velocity_pitch_factor;

            // Roll based on lateral velocity
            let roll = current_vel.y * self.module_data.lateral_velocity_roll_factor;

            // Apply rotation
            let mut rotation = self.rotation_rate;
            rotation.x += pitch;
            rotation.y += roll;

            physics.apply_angular_velocity(&rotation);
        }

        UpdateSleepTime::None
    }

    pub fn save(&self, xfer: &mut dyn Xfer) {
        let xfer_io = |result: std::io::Result<()>, field: &str| {
            if let Err(err) = result {
                panic!("HelicopterSlowDeathUpdate::save failed to xfer {field}: {err}");
            }
        };

        xfer.xfer_version_write(1);
        let mut fall_start_frame = self.fall_start_frame;
        xfer_io(xfer.xfer_u32(&mut fall_start_frame), "fall_start_frame");
        let mut death_frame = self.death_frame;
        xfer_io(xfer.xfer_u32(&mut death_frame), "death_frame");
        let mut thrust_direction = self.thrust_direction;
        xfer.xfer_coord3d(&mut thrust_direction);
        let mut rotation_rate = self.rotation_rate;
        xfer.xfer_coord3d(&mut rotation_rate);
    }

    pub fn load(&mut self, xfer: &mut dyn Xfer) {
        let xfer_io = |result: std::io::Result<()>, field: &str| {
            if let Err(err) = result {
                panic!("HelicopterSlowDeathUpdate::load failed to xfer {field}: {err}");
            }
        };

        let version = xfer.xfer_version_read();
        if version >= 1 {
            xfer_io(
                xfer.xfer_u32(&mut self.fall_start_frame),
                "fall_start_frame",
            );
            xfer_io(xfer.xfer_u32(&mut self.death_frame), "death_frame");
            xfer.xfer_coord3d(&mut self.thrust_direction);
            xfer.xfer_coord3d(&mut self.rotation_rate);
        }
    }
}

fn game_logic_random_value(min: u32, max: u32) -> u32 {
    if min >= max {
        return min;
    }
    crate::helpers::game_logic_random_value(min, max)
}

fn game_logic_random_value_real(min: f32, max: f32) -> f32 {
    if min >= max {
        return min;
    }
    crate::helpers::get_game_logic_random_value_real(min, max)
}
