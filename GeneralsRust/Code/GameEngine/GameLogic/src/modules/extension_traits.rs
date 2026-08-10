// Body/behavior/experience/stealth/flammable extensions
//
// Split from `modules.rs` for module-size parity.
// Observable behavior is unchanged.

/// Extension trait for Arc<Mutex<dyn BodyModuleInterface>> to provide convenient methods
pub trait BodyModuleInterfaceExt {
    fn set_initial_health(&self, health_percent: f32);
    fn get_max_health(&self) -> f32;
    fn get_health(&self) -> f32;
    fn set_health(&self, health: f32);
    fn get_last_damage_info(&self) -> Option<DamageInfo>;
    fn set_max_health(
        &self,
        max_health: f32,
        change_type: crate::object::body::body_module::MaxHealthChangeType,
    );
    fn set_aflame(&self, aflame: bool);
    fn set_damage_state(&self, new_state: BodyDamageType);
    fn attempt_healing(&self, healing_info: &mut DamageInfo);
}

impl BodyModuleInterfaceExt for Arc<Mutex<dyn BodyModuleInterface>> {
    fn set_initial_health(&self, health_percent: f32) {
        if let Ok(mut guard) = self.try_lock() {
            // Convert f32 percent to i32 for the trait method
            let percent_i32 = health_percent.clamp(0.0, 100.0).round() as i32;
            let _ = guard.set_initial_health(percent_i32);
        }
    }

    fn get_max_health(&self) -> f32 {
        if let Ok(guard) = self.try_lock() {
            guard.get_max_health()
        } else {
            0.0
        }
    }

    fn get_last_damage_info(&self) -> Option<DamageInfo> {
        if let Ok(guard) = self.try_lock() {
            guard.get_last_damage_info()
        } else {
            None
        }
    }

    fn set_max_health(
        &self,
        max_health: f32,
        change_type: crate::object::body::body_module::MaxHealthChangeType,
    ) {
        if let Ok(mut guard) = self.try_lock() {
            let _ = guard.set_max_health(max_health, change_type);
        }
    }

    fn get_health(&self) -> f32 {
        if let Ok(guard) = self.try_lock() {
            guard.get_health()
        } else {
            0.0
        }
    }

    fn set_health(&self, health: f32) {
        if let Ok(mut guard) = self.try_lock() {
            let _ = BodyModuleInterface::set_health(&mut *guard, health);
        }
    }

    fn set_aflame(&self, aflame: bool) {
        if let Ok(mut guard) = self.try_lock() {
            let _ = guard.set_aflame(aflame);
        }
    }

    fn set_damage_state(&self, new_state: BodyDamageType) {
        if let Ok(mut guard) = self.try_lock() {
            let _ = guard.set_damage_state(new_state);
        }
    }

    fn attempt_healing(&self, healing_info: &mut DamageInfo) {
        if let Ok(mut guard) = self.try_lock() {
            let _ = guard.attempt_healing(healing_info);
        }
    }
}

/// Extension trait for MutexGuard<dyn BodyModuleInterface> to provide set_health
pub trait BodyModuleGuardExt {
    fn set_health(&mut self, health: f32);
}

impl<'a> BodyModuleGuardExt for std::sync::MutexGuard<'a, dyn BodyModuleInterface> {
    fn set_health(&mut self, health: f32) {
        let _ = BodyModuleInterface::set_health(&mut **self, health);
    }
}

/// Extension trait for Arc<Mutex<dyn BehaviorModuleInterface>> to provide convenient methods
pub trait BehaviorModuleExt {
    fn set_sd_enabled(&self, enabled: bool);
    fn start_fire_spreading(&self);
}

impl BehaviorModuleExt for Arc<Mutex<dyn BehaviorModuleInterface>> {
    fn set_sd_enabled(&self, enabled: bool) {
        if let Ok(mut guard) = self.try_lock() {
            guard.set_sd_enabled(enabled);
        }
    }

    fn start_fire_spreading(&self) {
        // Fire spreading is triggered through FlammableUpdate → FireSpreadUpdate
        // with an UpdateContext (see flammable_update.rs:203). This trait method
        // has no UpdateContext, so the real path handles it.
    }
}

/// Extension trait for Arc<Mutex<ExperienceTracker>> to provide convenient methods
pub trait ExperienceTrackerExt {
    fn set_experience_sink(&self, sink: ObjectID);
}

impl ExperienceTrackerExt for Arc<Mutex<crate::common::ExperienceTracker>> {
    fn set_experience_sink(&self, sink: ObjectID) {
        if let Ok(mut guard) = self.try_lock() {
            guard.set_experience_sink(sink);
        }
    }
}

/// Extension trait for Arc<Mutex<StealthController>> to provide convenient methods
pub trait StealthControllerExt {
    fn receive_grant(&self, grant: bool, frames: UnsignedInt, current_frame: UnsignedInt);
}

impl StealthControllerExt for Arc<Mutex<crate::stealth_update::StealthController>> {
    fn receive_grant(&self, grant: bool, frames: UnsignedInt, current_frame: UnsignedInt) {
        if let Ok(mut guard) = self.try_lock() {
            let _ = guard.receive_grant(grant, frames, current_frame);
        }
    }
}

/// Extension trait for Arc<Mutex<dyn SpecialAbilityUpdate>> to provide convenient methods
pub trait SpecialAbilityUpdateExt {
    fn is_active(&self) -> bool;
}

impl SpecialAbilityUpdateExt for Arc<Mutex<dyn SpecialAbilityUpdate>> {
    fn is_active(&self) -> bool {
        if let Ok(guard) = self.try_lock() {
            guard.is_ability_active()
        } else {
            false
        }
    }
}

/// Extension trait for Arc<Mutex<FlammableUpdate>> to provide convenient methods
pub trait FlammableUpdateExt {
    fn try_to_ignite(&self, ctx: &mut crate::common::UpdateContext<'_>);
}

impl FlammableUpdateExt for Arc<Mutex<dyn BehaviorModuleInterface>> {
    fn try_to_ignite(&self, _ctx: &mut crate::common::UpdateContext<'_>) {
        if let Ok(mut guard) = self.lock() {
            guard.try_to_ignite_flammable();
        }
    }
}
