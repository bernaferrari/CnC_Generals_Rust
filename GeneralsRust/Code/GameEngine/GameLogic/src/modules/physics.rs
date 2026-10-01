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

    /// C++ PhysicsBehavior::getCenterOfMassOffset.
    fn get_center_of_mass_offset(&self) -> Real {
        0.0
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

    /// C++ `PhysicsBehavior::isMotive`: motive force still inside its expiry frame.
    fn is_motive(&self) -> bool {
        false
    }

    /// C++ `getAcceleration` returns last frame's acceleration (`m_prevAccel`).
    fn get_acceleration(&self) -> crate::common::Coord3D {
        crate::common::Coord3D::ZERO
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

    /// C++ PhysicsBehavior::setIsInFreeFall.
    fn set_is_in_freefall(&mut self, allow: bool) {
        let _ = allow;
    }

    /// C++ PhysicsBehavior::getIsInFreeFall.
    fn get_is_in_freefall(&self) -> bool {
        false
    }

    /// C++ PhysicsBehavior::setStickToGround (PhysicsUpdate.h:155).
    fn set_stick_to_ground(&mut self, stick: bool) {
        let _ = stick;
    }

    /// Readable STICK_TO_GROUND flag (C++ PhysicsFlagsType::STICK_TO_GROUND).
    fn get_stick_to_ground(&self) -> bool {
        false
    }

    /// C++ PhysicsBehavior::getForwardSpeed2D (PhysicsUpdate.cpp:939-957).
    /// Signed speed along facing; negative when moving backwards.
    fn get_forward_speed_2d(&self) -> Real {
        let vel = self.get_velocity();
        signed_forward_speed_2d(vel.x, vel.y, 1.0, 0.0)
    }

    /// C++ PhysicsBehavior::getForwardSpeed3D (PhysicsUpdate.cpp:964-980).
    fn get_forward_speed_3d(&self) -> Real {
        let vel = self.get_velocity();
        signed_forward_speed_3d(vel.x, vel.y, vel.z, 1.0, 0.0, 0.0)
    }



    /// Clear current acceleration (matches C++ clearAcceleration).
    fn clear_acceleration(&mut self) {}

    /// Scrub horizontal velocity to desired speed (matches C++ scrubVelocity2D).
    fn scrub_velocity_2d(&mut self, desired_velocity: Real) {
        let mut velocity = self.get_velocity();
        if desired_velocity < 0.001 {
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

/// C++ PhysicsUpdate.cpp:939-957 — signed 2D speed along `dir`.
pub fn signed_forward_speed_2d(vel_x: Real, vel_y: Real, dir_x: Real, dir_y: Real) -> Real {
    let vx = vel_x * dir_x;
    let vy = vel_y * dir_y;
    let speed = (vx * vx + vy * vy).sqrt();
    if vx + vy >= 0.0 {
        speed
    } else {
        -speed
    }
}

/// C++ PhysicsUpdate.cpp:964-980 — signed 3D speed along `dir`.
pub fn signed_forward_speed_3d(
    vel_x: Real,
    vel_y: Real,
    vel_z: Real,
    dir_x: Real,
    dir_y: Real,
    dir_z: Real,
) -> Real {
    let vx = vel_x * dir_x;
    let vy = vel_y * dir_y;
    let vz = vel_z * dir_z;
    let speed = (vx * vx + vy * vy + vz * vz).sqrt();
    if vx + vy + vz >= 0.0 {
        speed
    } else {
        -speed
    }
}


// The former `PhysicsBehaviorExt for Arc<Mutex<dyn PhysicsBehavior>>` adapter
// duplicated this trait's own surface through `with_physics_ref`/`with_physics_mut`
// lock helpers. Physics modules are owned values now: call `PhysicsBehavior`
// methods directly on the borrowed trait object.


#[cfg(test)]
mod physics_behavior_default_tests {
    use super::*;

    #[derive(Debug)]
    struct DummyPhysics {
        vel: Vec3D,
        stick: bool,
    }

    impl PhysicsBehavior for DummyPhysics {
        fn update(&mut self, _dt: f32) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
            Ok(())
        }
        fn get_velocity(&self) -> Vec3D {
            self.vel
        }
        fn set_velocity(&mut self, velocity: &Vec3D) {
            self.vel = *velocity;
        }
        fn is_on_ground(&self) -> bool {
            true
        }
        fn set_stick_to_ground(&mut self, stick: bool) {
            self.stick = stick;
        }
        fn get_stick_to_ground(&self) -> bool {
            self.stick
        }
    }

    #[test]
    fn scrub_velocity_2d_negative_desired_zeros_xy_not_reverse() {
        // C++ PhysicsUpdate.cpp:1012-1027 — desired < 0.001 zeroes x/y.
        let mut physics = DummyPhysics {
            vel: Vec3D::new(5.0, 3.0, 1.0),
            stick: false,
        };
        physics.scrub_velocity_2d(-2.0);
        assert_eq!(physics.vel.x, 0.0);
        assert_eq!(physics.vel.y, 0.0);
        assert_eq!(physics.vel.z, 1.0);
    }

    #[test]
    fn get_forward_speed_2d_is_signed_along_facing() {
        let mut physics = DummyPhysics {
            vel: Vec3D::new(-4.0, 0.0, 0.0),
            stick: false,
        };
        // Default facing +X, so backward vel is negative.
        assert!((physics.get_forward_speed_2d() + 4.0).abs() < 1.0e-5);
        physics.vel = Vec3D::new(3.0, 0.0, 0.0);
        assert!((physics.get_forward_speed_2d() - 3.0).abs() < 1.0e-5);
    }

    #[test]
    fn borrowed_physics_object_scrubs_and_reads_directly() {
        // C++ operates on the physics object directly; the owned-module world
        // hands callers `&mut dyn PhysicsBehavior` with no intermediate lock.
        let mut physics = DummyPhysics {
            vel: Vec3D::new(1.5, -2.0, 0.5),
            stick: true,
        };
        assert_eq!(PhysicsBehavior::get_velocity(&physics).x, 1.5);
        physics.set_velocity(&Vec3D::new(0.0, 4.0, 0.5));
        physics.scrub_velocity_2d(0.0);
        let vel = physics.get_velocity();
        assert_eq!(vel.x, 0.0);
        assert_eq!(vel.y, 0.0);
        assert_eq!(vel.z, 0.5);
        assert!(physics.get_stick_to_ground());
    }



}

