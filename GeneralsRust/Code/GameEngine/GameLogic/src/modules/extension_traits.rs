// Body/behavior/experience/stealth/flammable extensions
//
// Split from `modules.rs` for module-size parity.
// Observable behavior is unchanged.
//
// The former body/contain/physics extension traits for `Arc<Mutex<dyn ..>>`
// impls targeted `Arc<Mutex<dyn ..>>` / `MutexGuard<dyn ..>` and were pure
// delegation with silent try_lock-failure fallbacks. The module containers are
// owned values now, so callers invoke `BodyModuleInterface` /
// `BehaviorModuleInterface` methods directly on the borrowed trait object.

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

/// Extension trait for the borrowed special-ability adapter to provide the
/// C++ `SpecialAbilityUpdate::isActive` spelling.
pub trait SpecialAbilityUpdateExt {
    fn is_active(&self) -> bool;
}

impl SpecialAbilityUpdateExt for crate::object::SpecialAbilityUpdateRef<'_> {
    fn is_active(&self) -> bool {
        crate::modules::SpecialAbilityUpdate::is_ability_active(self)
    }
}

/// Extension trait for flammable behavior modules to provide convenient methods
pub trait FlammableUpdateExt {
    fn try_to_ignite(&mut self, ctx: &mut crate::common::UpdateContext<'_>);
}

impl FlammableUpdateExt for dyn BehaviorModuleInterface {
    fn try_to_ignite(&mut self, _ctx: &mut crate::common::UpdateContext<'_>) {
        self.try_to_ignite_flammable();
    }
}
