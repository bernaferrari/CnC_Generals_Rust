// PhysicsBehavior interface and Arc extension
//
// Split from `modules.rs` for module-size parity.
// Observable behavior is unchanged.

/// Physics behavior interface (matching C++ PhysicsBehavior)
pub trait PhysicsBehavior: Send + Sync + std::fmt::Debug {
    /// Update physics simulation
    fn update(&mut self, dt: f32) -> Result<(), Box<dyn std::error::Error + Send + Sync>>;
    /// Get current velocity
    fn get_velocity(&self) -> Vec3D;
    /// Set velocity
    fn set_velocity(&mut self, velocity: &Vec3D);
    /// Check if the object is on ground
    fn is_on_ground(&self) -> bool;

    /// Apply force to the object
    fn apply_force(&mut self, _force: &Vec3D) {
        // Default implementation - subclasses should override
    }

    /// Set yaw rotation rate (rotation around vertical axis)
    fn set_yaw_rate(&mut self, _rate: Real) {
        // Default implementation - subclasses should override
    }

    /// Set roll rotation rate (rotation around forward axis)
    fn set_roll_rate(&mut self, _rate: Real) {
        // Default implementation - subclasses should override
    }

    /// Set pitch rotation rate (rotation around lateral axis)
    fn set_pitch_rate(&mut self, _rate: Real) {
        // Default implementation - subclasses should override
    }

    /// Set turning state (matches C++ PhysicsBehavior::setTurning).
    fn set_turning(&mut self, _turning: i32) {
        // Default implementation - subclasses should override
    }

    /// Set mass of the physics object
    fn set_mass(&mut self, _mass: Real) {
        // Default implementation - subclasses should override
    }

    /// Set extra friction coefficient
    fn set_extra_friction(&mut self, _friction: Real) {
        // Default implementation - subclasses should override
    }

    /// Set extra bounciness coefficient
    fn set_extra_bounciness(&mut self, _bounciness: Real) {
        // Default implementation - subclasses should override
    }

    /// Enable or disable bouncing
    fn set_allow_bouncing(&mut self, _allow: bool) {
        // Default implementation - subclasses should override
    }

    /// Allow friction while airborne (matches C++ setAllowAirborneFriction).
    fn set_allow_airborne_friction(&mut self, allow: bool) {
        let _ = allow;
    }

    /// Add to current velocity (matches C++ addVelocityTo).
    fn add_velocity_to(&mut self, velocity: &Vec3D) {
        let mut current = self.get_velocity();
        current += *velocity;
        self.set_velocity(&current);
    }

    /// Set rotation angles (yaw, pitch, roll)
    fn set_angles(&mut self, _yaw: Real, _pitch: Real, _roll: Real) {
        // Default implementation - subclasses should override
    }

    /// Get mass of the physics object
    fn get_mass(&self) -> Real {
        // Default implementation - return default mass
        1.0
    }

    /// Set or clear the bounce sound used by collisions.
    fn set_bounce_sound(&mut self, _sound: Option<AudioEventRts>) {}

    /// Get the bounce sound for collision audio.
    fn get_bounce_sound(&self) -> Option<AudioEventRts> {
        None
    }

    /// Apply angular velocity (rotational forces)
    fn apply_angular_velocity(&mut self, _angular_velocity: &Vec3D) {
        // Default implementation - subclasses should override
    }

    /// Apply motive force (propulsion)
    fn apply_motive_force(&mut self, _force: &Vec3D) {
        // Default implementation - subclasses should override
    }

    /// Get current turning rate
    fn get_turning(&self) -> Real {
        // Default implementation - return zero
        0.0
    }

    /// Apply impulse/shock force (lightweight default).
    fn apply_shock(&mut self, force: &Coord3D) {
        let mass = self.get_mass().max(0.001);
        let impulse = Vec3D::new(force.x / mass, force.y / mass, force.z / mass);
        self.add_velocity_to(&impulse);
    }
    /// Apply a random rotation (lightweight default).
    fn apply_random_rotation(&mut self) {
        let yaw = crate::helpers::get_game_logic_random_value_real(
            -std::f32::consts::PI,
            std::f32::consts::PI,
        );
        let pitch = crate::helpers::get_game_logic_random_value_real(-0.25, 0.25);
        let roll = crate::helpers::get_game_logic_random_value_real(-0.25, 0.25);
        self.set_angles(yaw, pitch, roll);
    }
    /// Toggle stunned state.
    fn set_stunned(&mut self, stunned: bool) {
        let _ = stunned;
    }

    /// Allow object to fall under gravity.
    /// C++ PhysicsBehavior::setAllowToFall — sets ALLOW_TO_FALL (default unset/false).
    fn set_allow_to_fall(&mut self, allow: bool) {
        let _ = allow;
    }

    /// Whether this object is currently allowed to fall under gravity.
    /// C++ PhysicsBehavior::getAllowToFall — defaults false (flag not set).
    fn get_allow_to_fall(&self) -> bool {
        false
    }

    /// Readable alias for [`Self::get_allow_to_fall`].
    fn allow_to_fall(&self) -> bool {
        self.get_allow_to_fall()
    }

    /// Clear current acceleration (matches C++ clearAcceleration).
    fn clear_acceleration(&mut self) {}

    /// Scrub horizontal velocity to desired speed (matches C++ scrubVelocity2D).
    fn scrub_velocity_2d(&mut self, desired_velocity: Real) {
        let mut velocity = self.get_velocity();
        if desired_velocity.abs() < 0.001 {
            velocity.x = 0.0;
            velocity.y = 0.0;
            self.set_velocity(&velocity);
            return;
        }
        let cur = (velocity.x * velocity.x + velocity.y * velocity.y).sqrt();
        if cur <= 0.0 || desired_velocity > cur {
            return;
        }
        let scale = desired_velocity / cur;
        velocity.x *= scale;
        velocity.y *= scale;
        self.set_velocity(&velocity);
    }

    /// Scrub vertical velocity to desired speed (matches C++ scrubVelocityZ).
    fn scrub_velocity_z(&mut self, desired_velocity: Real) {
        let mut velocity = self.get_velocity();
        if desired_velocity.abs() < 0.001 {
            velocity.z = 0.0;
            self.set_velocity(&velocity);
            return;
        }
        if (desired_velocity < 0.0 && velocity.z < desired_velocity)
            || (desired_velocity > 0.0 && velocity.z > desired_velocity)
        {
            velocity.z = desired_velocity;
            self.set_velocity(&velocity);
        }
    }

    /// Reset dynamic physics state (matches C++ PhysicsBehavior::resetDynamicPhysics).
    fn reset_dynamic_physics(&mut self) {
        self.set_velocity(&Vec3D::ZERO);
        self.set_yaw_rate(0.0);
        self.set_pitch_rate(0.0);
        self.set_roll_rate(0.0);
        self.set_angles(0.0, 0.0, 0.0);
    }

    /// Get the ID of the last object this physics object collided with
    fn get_last_collidee(&self) -> ObjectID {
        // Default implementation - return invalid ID (no collision)
        INVALID_ID
    }

    /// Get the ID of the object to ignore collisions with (matches C++ PhysicsBehavior::getIgnoreCollisionsWith).
    fn get_ignore_collisions_with(&self) -> ObjectID {
        INVALID_ID
    }

    /// Ignore collisions with a specific object (matches C++ PhysicsBehavior::setIgnoreCollisionsWith).
    fn set_ignore_collisions_with(&mut self, _obj_id: ObjectID) {
        // Default implementation - subclasses should override if supported
    }
}

/// Extension trait for Arc<Mutex<dyn PhysicsBehavior>> to provide convenient methods
pub trait PhysicsBehaviorExt {
    fn get_velocity(&self) -> Vec3D;
    fn set_velocity(&self, velocity: &Vec3D);
    fn apply_force(&self, force: &Vec3D);
    fn add_velocity_to(&self, velocity: &Vec3D);
    fn set_yaw_rate(&self, rate: Real);
    fn set_roll_rate(&self, rate: Real);
    fn set_pitch_rate(&self, rate: Real);
    fn set_mass(&self, mass: Real);
    fn get_mass(&self) -> Real;
    fn set_extra_friction(&self, friction: Real);
    fn set_extra_bounciness(&self, bounciness: Real);
    fn set_allow_bouncing(&self, allow: bool);
    fn set_allow_airborne_friction(&self, allow: bool);
    fn set_allow_to_fall(&self, allow: bool);
    fn get_allow_to_fall(&self) -> bool;
    fn allow_to_fall(&self) -> bool;
    fn set_turning(&self, turning: i32);
    fn set_angles(&self, yaw: Real, pitch: Real, roll: Real);
    fn apply_angular_velocity(&self, angular_velocity: &Vec3D);
    fn apply_motive_force(&self, force: &Vec3D);
    fn get_turning(&self) -> Real;
    fn get_last_collidee(&self) -> ObjectID;
    fn set_bounce_sound(&self, sound: Option<AudioEventRts>);
    fn get_bounce_sound(&self) -> Option<AudioEventRts>;
    fn set_ignore_collisions_with(&self, obj_id: ObjectID);
    fn clear_acceleration(&self);
    fn scrub_velocity_2d(&self, desired_velocity: Real);
    fn scrub_velocity_z(&self, desired_velocity: Real);
    fn reset_dynamic_physics(&self);
}

impl PhysicsBehaviorExt for Arc<Mutex<dyn PhysicsBehavior>> {
    fn get_velocity(&self) -> Vec3D {
        if let Ok(guard) = self.try_lock() {
            guard.get_velocity()
        } else {
            Vec3D::ZERO
        }
    }

    fn set_velocity(&self, velocity: &Vec3D) {
        if let Ok(mut guard) = self.try_lock() {
            guard.set_velocity(velocity);
        }
    }

    fn apply_force(&self, force: &Vec3D) {
        if let Ok(mut guard) = self.try_lock() {
            guard.apply_force(force);
        }
    }

    fn add_velocity_to(&self, velocity: &Vec3D) {
        if let Ok(mut guard) = self.try_lock() {
            guard.add_velocity_to(velocity);
        }
    }

    fn set_yaw_rate(&self, rate: Real) {
        if let Ok(mut guard) = self.try_lock() {
            guard.set_yaw_rate(rate);
        }
    }

    fn set_roll_rate(&self, rate: Real) {
        if let Ok(mut guard) = self.try_lock() {
            guard.set_roll_rate(rate);
        }
    }

    fn set_pitch_rate(&self, rate: Real) {
        if let Ok(mut guard) = self.try_lock() {
            guard.set_pitch_rate(rate);
        }
    }

    fn set_mass(&self, mass: Real) {
        if let Ok(mut guard) = self.try_lock() {
            guard.set_mass(mass);
        }
    }

    fn set_extra_friction(&self, friction: Real) {
        if let Ok(mut guard) = self.try_lock() {
            guard.set_extra_friction(friction);
        }
    }

    fn set_extra_bounciness(&self, bounciness: Real) {
        if let Ok(mut guard) = self.try_lock() {
            guard.set_extra_bounciness(bounciness);
        }
    }

    fn set_allow_bouncing(&self, allow: bool) {
        if let Ok(mut guard) = self.try_lock() {
            guard.set_allow_bouncing(allow);
        }
    }

    fn set_allow_airborne_friction(&self, allow: bool) {
        if let Ok(mut guard) = self.try_lock() {
            guard.set_allow_airborne_friction(allow);
        }
    }

    fn set_allow_to_fall(&self, allow: bool) {
        if let Ok(mut guard) = self.try_lock() {
            guard.set_allow_to_fall(allow);
        }
    }

    fn get_allow_to_fall(&self) -> bool {
        if let Ok(guard) = self.try_lock() {
            guard.get_allow_to_fall()
        } else {
            false
        }
    }

    fn allow_to_fall(&self) -> bool {
        self.get_allow_to_fall()
    }

    fn set_turning(&self, turning: i32) {
        if let Ok(mut guard) = self.try_lock() {
            guard.set_turning(turning);
        }
    }

    fn set_angles(&self, yaw: Real, pitch: Real, roll: Real) {
        if let Ok(mut guard) = self.try_lock() {
            guard.set_angles(yaw, pitch, roll);
        }
    }

    fn get_mass(&self) -> Real {
        if let Ok(guard) = self.try_lock() {
            guard.get_mass()
        } else {
            1.0
        }
    }

    fn apply_angular_velocity(&self, angular_velocity: &Vec3D) {
        if let Ok(mut guard) = self.try_lock() {
            guard.apply_angular_velocity(angular_velocity);
        }
    }

    fn apply_motive_force(&self, force: &Vec3D) {
        if let Ok(mut guard) = self.try_lock() {
            guard.apply_motive_force(force);
        }
    }

    fn get_turning(&self) -> Real {
        if let Ok(guard) = self.try_lock() {
            guard.get_turning()
        } else {
            0.0
        }
    }

    fn get_last_collidee(&self) -> ObjectID {
        if let Ok(guard) = self.try_lock() {
            guard.get_last_collidee()
        } else {
            INVALID_ID
        }
    }

    fn set_ignore_collisions_with(&self, obj_id: ObjectID) {
        if let Ok(mut guard) = self.try_lock() {
            guard.set_ignore_collisions_with(obj_id);
        }
    }

    fn set_bounce_sound(&self, sound: Option<AudioEventRts>) {
        if let Ok(mut guard) = self.try_lock() {
            guard.set_bounce_sound(sound);
        }
    }

    fn get_bounce_sound(&self) -> Option<AudioEventRts> {
        if let Ok(guard) = self.try_lock() {
            guard.get_bounce_sound()
        } else {
            None
        }
    }

    fn clear_acceleration(&self) {
        if let Ok(mut guard) = self.try_lock() {
            guard.clear_acceleration();
        }
    }

    fn scrub_velocity_2d(&self, desired_velocity: Real) {
        if let Ok(mut guard) = self.try_lock() {
            guard.scrub_velocity_2d(desired_velocity);
        }
    }

    fn scrub_velocity_z(&self, desired_velocity: Real) {
        if let Ok(mut guard) = self.try_lock() {
            guard.scrub_velocity_z(desired_velocity);
        }
    }

    fn reset_dynamic_physics(&self) {
        if let Ok(mut guard) = self.try_lock() {
            guard.reset_dynamic_physics();
        }
    }
}
