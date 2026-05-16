// HelicopterSlowDeathUpdate - Handles the slow death behavior of helicopters
// Author: Peter Sauer, August 2003
// Ported to Rust

use crate::prelude::*;
use game_engine::common::system::{Snapshotable, Xfer, XferVersion};

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
}

impl Snapshotable for HelicopterSlowDeathUpdate {
    fn crc(&self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut fall_start_frame = self.fall_start_frame;
        xfer.xfer_unsigned_int(&mut fall_start_frame)
            .map_err(|e| format!("HelicopterSlowDeathUpdate::crc fall_start_frame: {e:?}"))?;
        let mut death_frame = self.death_frame;
        xfer.xfer_unsigned_int(&mut death_frame)
            .map_err(|e| format!("HelicopterSlowDeathUpdate::crc death_frame: {e:?}"))?;
        Ok(())
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let current_version: XferVersion = 1;
        let mut version = current_version;
        xfer.xfer_version(&mut version, current_version)
            .map_err(|e| format!("HelicopterSlowDeathUpdate::xfer version failed: {e:?}"))?;

        xfer.xfer_unsigned_int(&mut self.fall_start_frame)
            .map_err(|e| format!("HelicopterSlowDeathUpdate::xfer fall_start_frame: {e:?}"))?;
        xfer.xfer_unsigned_int(&mut self.death_frame)
            .map_err(|e| format!("HelicopterSlowDeathUpdate::xfer death_frame: {e:?}"))?;
        xfer.xfer_real(&mut self.thrust_direction.x)
            .map_err(|e| format!("HelicopterSlowDeathUpdate::xfer thrust_direction.x: {e:?}"))?;
        xfer.xfer_real(&mut self.thrust_direction.y)
            .map_err(|e| format!("HelicopterSlowDeathUpdate::xfer thrust_direction.y: {e:?}"))?;
        xfer.xfer_real(&mut self.thrust_direction.z)
            .map_err(|e| format!("HelicopterSlowDeathUpdate::xfer thrust_direction.z: {e:?}"))?;
        xfer.xfer_real(&mut self.rotation_rate.x)
            .map_err(|e| format!("HelicopterSlowDeathUpdate::xfer rotation_rate.x: {e:?}"))?;
        xfer.xfer_real(&mut self.rotation_rate.y)
            .map_err(|e| format!("HelicopterSlowDeathUpdate::xfer rotation_rate.y: {e:?}"))?;
        xfer.xfer_real(&mut self.rotation_rate.z)
            .map_err(|e| format!("HelicopterSlowDeathUpdate::xfer rotation_rate.z: {e:?}"))?;
        Ok(())
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        Ok(())
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
