// PhysicsBehavior interface and canonical-state access extension
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
    if vx + vy >= 0.0 { speed } else { -speed }
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
    if vx + vy + vz >= 0.0 { speed } else { -speed }
}

/// Physics operations through canonical module descriptors or existing shared storage.
///
/// C++ `PhysicsBehavior` methods return the live velocity and apply writes.
/// They do not turn a busy mutex into zero or a dropped set. `std::sync::Mutex`
/// is not reentrant: a caller that already has `&mut dyn PhysicsBehavior` must
/// use that reference (the fields) and must not call these methods on the same
/// shared descriptor.
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
    fn set_is_in_freefall(&self, allow: bool);
    fn get_is_in_freefall(&self) -> bool;
    fn set_stick_to_ground(&self, stick: bool);
    fn get_stick_to_ground(&self) -> bool;
    fn get_forward_speed_2d(&self) -> Real;
    fn get_forward_speed_3d(&self) -> Real;
    fn get_center_of_mass_offset(&self) -> Real;
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

trait PhysicsStorageAccess {
    fn with_ref<R>(&self, f: impl FnOnce(&dyn PhysicsBehavior) -> R) -> R;
    fn with_mut<R>(&self, f: impl FnOnce(&mut dyn PhysicsBehavior) -> R) -> R;
}

impl PhysicsStorageAccess for Arc<Mutex<dyn PhysicsBehavior>> {
    fn with_ref<R>(&self, f: impl FnOnce(&dyn PhysicsBehavior) -> R) -> R {
        match self.lock() {
            Ok(guard) => f(&*guard),
            Err(poisoned) => f(&*poisoned.into_inner()),
        }
    }

    fn with_mut<R>(&self, f: impl FnOnce(&mut dyn PhysicsBehavior) -> R) -> R {
        match self.lock() {
            Ok(mut guard) => f(&mut *guard),
            Err(poisoned) => f(&mut *poisoned.into_inner()),
        }
    }
}

impl PhysicsStorageAccess for crate::object::PhysicsInterfaceHandle {
    fn with_ref<R>(&self, f: impl FnOnce(&dyn PhysicsBehavior) -> R) -> R {
        self.with_physics(|physics| f(physics))
    }

    fn with_mut<R>(&self, f: impl FnOnce(&mut dyn PhysicsBehavior) -> R) -> R {
        self.with_physics(f)
    }
}

fn with_physics_ref<R>(
    physics: &impl PhysicsStorageAccess,
    f: impl FnOnce(&dyn PhysicsBehavior) -> R,
) -> R {
    physics.with_ref(f)
}

fn with_physics_mut<R>(
    physics: &impl PhysicsStorageAccess,
    f: impl FnOnce(&mut dyn PhysicsBehavior) -> R,
) -> R {
    physics.with_mut(f)
}

impl<T: PhysicsStorageAccess> PhysicsBehaviorExt for T {
    fn get_velocity(&self) -> Vec3D {
        with_physics_ref(self, |physics| physics.get_velocity())
    }

    fn set_velocity(&self, velocity: &Vec3D) {
        with_physics_mut(self, |physics| physics.set_velocity(velocity));
    }

    fn apply_force(&self, force: &Vec3D) {
        with_physics_mut(self, |physics| physics.apply_force(force));
    }

    fn add_velocity_to(&self, velocity: &Vec3D) {
        with_physics_mut(self, |physics| physics.add_velocity_to(velocity));
    }

    fn set_yaw_rate(&self, rate: Real) {
        with_physics_mut(self, |physics| physics.set_yaw_rate(rate));
    }

    fn set_roll_rate(&self, rate: Real) {
        with_physics_mut(self, |physics| physics.set_roll_rate(rate));
    }

    fn set_pitch_rate(&self, rate: Real) {
        with_physics_mut(self, |physics| physics.set_pitch_rate(rate));
    }

    fn set_mass(&self, mass: Real) {
        with_physics_mut(self, |physics| physics.set_mass(mass));
    }

    fn set_extra_friction(&self, friction: Real) {
        with_physics_mut(self, |physics| physics.set_extra_friction(friction));
    }

    fn set_extra_bounciness(&self, bounciness: Real) {
        with_physics_mut(self, |physics| physics.set_extra_bounciness(bounciness));
    }

    fn set_allow_bouncing(&self, allow: bool) {
        with_physics_mut(self, |physics| physics.set_allow_bouncing(allow));
    }

    fn set_allow_airborne_friction(&self, allow: bool) {
        with_physics_mut(self, |physics| physics.set_allow_airborne_friction(allow));
    }

    fn set_allow_to_fall(&self, allow: bool) {
        with_physics_mut(self, |physics| physics.set_allow_to_fall(allow));
    }

    fn get_allow_to_fall(&self) -> bool {
        with_physics_ref(self, |physics| physics.get_allow_to_fall())
    }

    fn allow_to_fall(&self) -> bool {
        self.get_allow_to_fall()
    }

    fn set_is_in_freefall(&self, allow: bool) {
        with_physics_mut(self, |physics| physics.set_is_in_freefall(allow));
    }

    fn get_is_in_freefall(&self) -> bool {
        with_physics_ref(self, |physics| physics.get_is_in_freefall())
    }

    fn get_center_of_mass_offset(&self) -> Real {
        with_physics_ref(self, |physics| physics.get_center_of_mass_offset())
    }

    fn set_stick_to_ground(&self, stick: bool) {
        with_physics_mut(self, |physics| physics.set_stick_to_ground(stick));
    }

    fn get_stick_to_ground(&self) -> bool {
        with_physics_ref(self, |physics| physics.get_stick_to_ground())
    }

    fn get_forward_speed_2d(&self) -> Real {
        with_physics_ref(self, |physics| physics.get_forward_speed_2d())
    }

    fn get_forward_speed_3d(&self) -> Real {
        with_physics_ref(self, |physics| physics.get_forward_speed_3d())
    }

    fn set_turning(&self, turning: i32) {
        with_physics_mut(self, |physics| physics.set_turning(turning));
    }

    fn set_angles(&self, yaw: Real, pitch: Real, roll: Real) {
        with_physics_mut(self, |physics| physics.set_angles(yaw, pitch, roll));
    }

    fn get_mass(&self) -> Real {
        with_physics_ref(self, |physics| physics.get_mass())
    }

    fn apply_angular_velocity(&self, angular_velocity: &Vec3D) {
        with_physics_mut(self, |physics| {
            physics.apply_angular_velocity(angular_velocity)
        });
    }

    fn apply_motive_force(&self, force: &Vec3D) {
        with_physics_mut(self, |physics| physics.apply_motive_force(force));
    }

    fn get_turning(&self) -> Real {
        with_physics_ref(self, |physics| physics.get_turning())
    }

    fn get_last_collidee(&self) -> ObjectID {
        with_physics_ref(self, |physics| physics.get_last_collidee())
    }

    fn set_ignore_collisions_with(&self, obj_id: ObjectID) {
        with_physics_mut(self, |physics| physics.set_ignore_collisions_with(obj_id));
    }

    fn set_bounce_sound(&self, sound: Option<AudioEventRts>) {
        with_physics_mut(self, |physics| physics.set_bounce_sound(sound));
    }

    fn get_bounce_sound(&self) -> Option<AudioEventRts> {
        with_physics_ref(self, |physics| physics.get_bounce_sound())
    }

    fn clear_acceleration(&self) {
        with_physics_mut(self, |physics| physics.clear_acceleration());
    }

    fn scrub_velocity_2d(&self, desired_velocity: Real) {
        with_physics_mut(self, |physics| physics.scrub_velocity_2d(desired_velocity));
    }

    fn scrub_velocity_z(&self, desired_velocity: Real) {
        with_physics_mut(self, |physics| physics.scrub_velocity_z(desired_velocity));
    }

    fn reset_dynamic_physics(&self) {
        with_physics_mut(self, |physics| physics.reset_dynamic_physics());
    }
}

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

    fn arc_physics(physics: DummyPhysics) -> Arc<Mutex<dyn PhysicsBehavior>> {
        Arc::new(Mutex::new(physics))
    }

    #[test]
    fn ext_applies_velocity_while_another_thread_holds_the_mutex() {
        let physics = arc_physics(DummyPhysics {
            vel: Vec3D::new(8.0, 0.0, 1.0),
            stick: false,
        });
        let physics_for_setter = physics.clone();
        let entered = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let entered_flag = Arc::clone(&entered);
        let _guard = physics.lock().unwrap_or_else(|err| err.into_inner());
        let setter = std::thread::spawn(move || {
            entered_flag.store(true, std::sync::atomic::Ordering::SeqCst);
            physics_for_setter.set_velocity(&Vec3D::new(2.0, 3.0, 4.0));
            physics_for_setter.set_stick_to_ground(true);
        });
        while !entered.load(std::sync::atomic::Ordering::SeqCst) {
            std::thread::yield_now();
        }
        // The setter is inside set_velocity. A try_lock failure would drop it.
        std::thread::sleep(std::time::Duration::from_millis(40));
        drop(_guard);
        setter.join().unwrap();
        let vel = physics.get_velocity();
        assert_eq!((vel.x, vel.y, vel.z), (2.0, 3.0, 4.0));
        assert!(physics.get_stick_to_ground());
        physics.scrub_velocity_2d(0.0);
        let vel = physics.get_velocity();
        assert_eq!((vel.x, vel.y, vel.z), (0.0, 0.0, 4.0));
    }

    #[test]
    fn held_physics_guard_reads_velocity_without_the_arc() {
        let physics = arc_physics(DummyPhysics {
            vel: Vec3D::new(1.5, -2.0, 0.5),
            stick: true,
        });
        let mut guard = physics.lock().unwrap_or_else(|err| err.into_inner());
        assert_eq!(PhysicsBehavior::get_velocity(&*guard).x, 1.5);
        guard.set_velocity(&Vec3D::new(0.0, 4.0, 0.5));
        guard.scrub_velocity_2d(0.0);
        let vel = guard.get_velocity();
        assert_eq!(vel.x, 0.0);
        assert_eq!(vel.y, 0.0);
        assert_eq!(vel.z, 0.5);
        assert!(guard.get_stick_to_ground());
    }
}
