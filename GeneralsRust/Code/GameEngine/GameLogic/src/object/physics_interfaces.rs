//! Borrowed access to the one physics state owned by an installed module.
//!
//! The descriptor owns no mutable physics and adds no lock or allocation. The
//! lease holds the canonical module guard. Independently shared railroad and
//! injected physics keep their existing shared storage explicitly.

use super::{BehaviorModuleHandle, Module};
use crate::modules::PhysicsBehavior;
use crate::object::behavior::physics_update::PhysicsBehaviorUpdate;
use std::ops::{Deref, DerefMut};
use std::sync::{Arc, LockResult, Mutex, MutexGuard, PoisonError, TryLockError, TryLockResult};

impl super::Object {
    /// Resolve one installed update proxy from its exact module entry, rejecting
    /// duplicate descriptors and registrations that only share a class name.
    pub(crate) fn installed_update_proxy_named(
        &self,
        name: &str,
    ) -> Result<game_engine::common::thing::update_module::UpdateModulePtr, String> {
        let mut entries = self.modules.iter().enumerate().filter(|(_, entry)| {
            entry.name().as_str() == name
                && (entry.mask().0
                    & game_engine::common::thing::module::ModuleInterfaceType::UPDATE.0)
                    != 0
        });
        let (index, _) = entries.next().ok_or("installed update descriptor absent")?;
        if entries.next().is_some() {
            return Err("duplicate installed update descriptors".into());
        }
        let mut registrations = self
            .update_module_registrations
            .iter()
            .filter(|registration| registration.module_index == Some(index));
        let proxy = registrations
            .next()
            .ok_or("installed update proxy absent")?;
        if registrations.next().is_some() {
            return Err("duplicate update proxy registrations".into());
        }
        Ok(proxy.module.clone())
    }
}

#[derive(Clone, Debug)]
pub struct PhysicsInterfaceHandle {
    kind: PhysicsInterfaceKind,
}

#[derive(Clone, Debug)]
enum PhysicsInterfaceKind {
    Module(BehaviorModuleHandle),
    Shared(Arc<Mutex<dyn PhysicsBehavior>>),
}

/// Exclusive borrow of the original module's physics state. This contains the
/// actual storage guard; access cannot outlive the descriptor it borrows.
pub struct PhysicsInterfaceLease<'a> {
    kind: PhysicsInterfaceLeaseKind<'a>,
}

enum PhysicsInterfaceLeaseKind<'a> {
    Module(MutexGuard<'a, Box<dyn Module>>),
    Shared(MutexGuard<'a, dyn PhysicsBehavior + 'static>),
}

fn module_physics(module: &dyn Module) -> &(dyn PhysicsBehavior + 'static) {
    module
        .as_any()
        .downcast_ref::<crate::contain_module_overrides::ActiveBehaviorModule<PhysicsBehaviorUpdate>>()
        .expect("physics descriptor must reference its installed PhysicsBehavior module")
        .behavior()
        .physics_interface()
}

fn module_physics_mut(module: &mut dyn Module) -> &mut (dyn PhysicsBehavior + 'static) {
    module
        .as_any_mut()
        .downcast_mut::<crate::contain_module_overrides::ActiveBehaviorModule<PhysicsBehaviorUpdate>>()
        .expect("physics descriptor must reference its installed PhysicsBehavior module")
        .behavior_mut()
        .physics_interface_mut()
}

impl Deref for PhysicsInterfaceLease<'_> {
    type Target = dyn PhysicsBehavior;

    fn deref(&self) -> &Self::Target {
        match &self.kind {
            PhysicsInterfaceLeaseKind::Module(module) => module_physics(module.as_ref()),
            PhysicsInterfaceLeaseKind::Shared(physics) => &**physics,
        }
    }
}

impl DerefMut for PhysicsInterfaceLease<'_> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        match &mut self.kind {
            PhysicsInterfaceLeaseKind::Module(module) => module_physics_mut(module.as_mut()),
            PhysicsInterfaceLeaseKind::Shared(physics) => &mut **physics,
        }
    }
}

impl PhysicsInterfaceHandle {
    pub(super) fn from_module(module: BehaviorModuleHandle) -> Self {
        Self {
            kind: PhysicsInterfaceKind::Module(module),
        }
    }

    /// Borrow canonical physics state, reporting poison exactly as its storage
    /// does. No independently synchronized interface wrapper is involved.
    pub fn access(&self) -> LockResult<PhysicsInterfaceLease<'_>> {
        match &self.kind {
            PhysicsInterfaceKind::Module(module) => map_access(
                module.entry.module.lock(),
                PhysicsInterfaceLeaseKind::Module,
            ),
            PhysicsInterfaceKind::Shared(physics) => {
                map_access(physics.lock(), PhysicsInterfaceLeaseKind::Shared)
            }
        }
    }

    /// Nonblocking canonical-state borrow. Busy module access is not a zero
    /// velocity or a successful dropped mutation.
    pub fn try_access(&self) -> TryLockResult<PhysicsInterfaceLease<'_>> {
        match &self.kind {
            PhysicsInterfaceKind::Module(module) => map_try_access(
                module.entry.module.try_lock(),
                PhysicsInterfaceLeaseKind::Module,
            ),
            PhysicsInterfaceKind::Shared(physics) => {
                map_try_access(physics.try_lock(), PhysicsInterfaceLeaseKind::Shared)
            }
        }
    }

    /// Single operation on canonical state. Shared fixture/railroad poison is
    /// recovered as before; module poison retains ModuleEntry's failure policy.
    pub(crate) fn with_physics<R>(&self, f: impl FnOnce(&mut dyn PhysicsBehavior) -> R) -> R {
        match &self.kind {
            PhysicsInterfaceKind::Module(module) => {
                module.with_module(|module| f(module_physics_mut(module)))
            }
            PhysicsInterfaceKind::Shared(physics) => match physics.lock() {
                Ok(mut physics) => f(&mut *physics),
                Err(poisoned) => f(&mut *poisoned.into_inner()),
            },
        }
    }

    /// CPP reset effects remain synchronous, but wake registration happens
    /// after the installed module loan ends so it can record its actual wake.
    pub(crate) fn reset_dynamic_physics_borrowed(&self) {
        match &self.kind {
            PhysicsInterfaceKind::Module(module) => {
                let wake = module.with_module(|module| {
                    module
                            .as_any_mut()
                            .downcast_mut::<crate::contain_module_overrides::ActiveBehaviorModule<
                                PhysicsBehaviorUpdate,
                            >>()
                            .expect("installed PhysicsBehavior descriptor")
                            .behavior_mut()
                            .reset_dynamic_physics_borrowed()
                });
                if let Some(wake) = wake {
                    wake.apply();
                }
            }
            PhysicsInterfaceKind::Shared(_) => {
                self.with_physics(|physics| physics.reset_dynamic_physics())
            }
        }
    }

    /// C++ applyMotiveForce: clear expiry and apply force, wake synchronously,
    /// then stamp expiry. The wake proxy may borrow this same module to record
    /// its state, so each canonical Physics loan ends before that callback.
    pub(crate) fn apply_motive_force_with_wake(
        &self,
        force: &super::Coord3D,
        owner: &super::Object,
        frame: u32,
        wake: impl FnOnce() -> Result<(), String>,
    ) -> Result<(), String> {
        let PhysicsInterfaceKind::Module(handle) = &self.kind else {
            return Err("motive wake requires installed PhysicsBehavior storage".into());
        };
        // The implicit C++ getObject() cannot name a peer's same-ID Object.
        self.installed_update_proxy(owner)?;
        let should_wake = {
            let mut module = handle
                .entry
                .module
                .lock()
                .map_err(|_| "Physics module poisoned")?;
            let physics = module.as_any_mut()
                .downcast_mut::<crate::contain_module_overrides::ActiveBehaviorModule<PhysicsBehaviorUpdate>>()
                .ok_or("Physics descriptor no longer references its installed module")?;
            physics
                .behavior_mut()
                .prepare_motive_force_at(force, owner, frame)
        };
        let result = if should_wake { wake() } else { Ok(()) };
        {
            let mut module = handle
                .entry
                .module
                .lock()
                .map_err(|_| "Physics module poisoned")?;
            let physics = module.as_any_mut()
                .downcast_mut::<crate::contain_module_overrides::ActiveBehaviorModule<PhysicsBehaviorUpdate>>()
                .ok_or("Physics descriptor no longer references its installed module")?;
            physics.behavior_mut().finish_motive_force_at(frame);
        }
        result
    }

    pub(crate) fn validate_shock_at(&self, owner: &super::Object) -> Result<(), String> {
        self.installed_update_proxy(owner)?;
        self.with_installed_update(|physics| physics.validate_shock_at(owner))
    }

    pub(crate) fn prepare_shock_at(
        &self,
        force: &super::Coord3D,
        owner: &super::Object,
        frame: u32,
    ) -> Result<bool, String> {
        self.installed_update_proxy(owner)?;
        self.with_installed_update(|physics| physics.prepare_shock_at(force, owner, frame))
    }

    pub(crate) fn prepare_random_rotation_at(
        &self,
        owner: &super::Object,
        random: &mut game_engine::common::random_value::RandomState,
    ) -> Result<bool, String> {
        self.installed_update_proxy(owner)?;
        self.with_installed_update(|physics| physics.prepare_random_rotation_at(owner, random))
    }

    pub(crate) fn finish_stunned_at(&self, owner: &mut super::Object) -> Result<(), String> {
        self.installed_update_proxy(owner)?;
        self.with_installed_update(|physics| physics.finish_stunned_at(owner))
    }

    /// The descriptor validates owner identity before this bounded module loan;
    /// wake callbacks execute only after the returned value releases that loan.
    fn with_installed_update<R>(
        &self,
        operation: impl FnOnce(&mut PhysicsBehaviorUpdate) -> Result<R, String>,
    ) -> Result<R, String> {
        let PhysicsInterfaceKind::Module(handle) = &self.kind else {
            return Err("operation requires installed PhysicsBehavior storage".into());
        };
        handle.try_with_module(|module| {
            let module = module.as_any_mut()
                .downcast_mut::<crate::contain_module_overrides::ActiveBehaviorModule<PhysicsBehaviorUpdate>>()
                .ok_or("installed Physics module absent")?;
            operation(module.behavior_mut())
        }).ok_or("installed Physics module unavailable")?
    }

    #[cfg(test)]
    pub(crate) fn set_freefall_for_test(&self, owner: &super::Object) -> Result<(), String> {
        self.installed_update_proxy(owner)?;
        self.with_installed_update(|physics| {
            physics.set_freefall_for_test();
            Ok(())
        })
    }

    #[cfg(test)]
    pub(crate) fn set_immune_freefall_for_test(&self, owner: &super::Object) -> Result<(), String> {
        self.installed_update_proxy(owner)?;
        self.with_installed_update(|physics| {
            physics.set_immune_freefall_for_test();
            Ok(())
        })
    }

    pub(crate) fn prepare_owned_at(
        &self,
        owner: &super::Object,
        frame: u32,
        gravity: f32,
        start_ground: f32,
        ground_height: impl FnMut(f32, f32) -> f32,
    ) -> Result<super::behavior::physics_update::PreparedOwnedPhysicsStep, String> {
        self.installed_update_proxy(owner)?;
        self.with_installed_update(|physics| {
            physics.prepare_owned_at(owner, frame, gravity, start_ground, ground_height)
        })
    }

    pub(crate) fn commit_owned_at(
        &self,
        owner: &mut super::Object,
        prepared: super::behavior::physics_update::PreparedOwnedPhysicsStep,
    ) -> Result<super::behavior::physics_update::OwnedPhysicsStep, String> {
        self.installed_update_proxy(owner)?;
        self.with_installed_update(|physics| physics.commit_owned_at(owner, prepared))
    }

    pub(crate) fn begin_owned_landing_at(
        &self,
        owner: &super::Object,
        airborne_at_end: bool,
    ) -> Result<Option<super::behavior::physics_update::OwnedLanding>, String> {
        self.installed_update_proxy(owner)?;
        self.with_installed_update(|physics| {
            Ok(physics.begin_owned_landing_at(owner, airborne_at_end))
        })
    }

    pub(crate) fn ground_collision_owned_at(&self, owner: &super::Object) -> Result<(), String> {
        self.installed_update_proxy(owner)?;
        self.with_installed_update(|physics| physics.ground_collision_owned_at(owner))
    }

    pub(crate) fn falling_damage_owned_at(
        &self,
        owner: &super::Object,
        active_velocity_z: f32,
    ) -> Result<Option<crate::damage::DamageInfo>, String> {
        self.installed_update_proxy(owner)?;
        self.with_installed_update(|physics| {
            Ok(physics.falling_damage_owned_at(owner, active_velocity_z))
        })
    }

    pub(crate) fn should_kill_resting_owned(
        &self,
        owner: &super::Object,
        airborne: bool,
    ) -> Result<bool, String> {
        self.installed_update_proxy(owner)?;
        self.with_installed_update(|physics| Ok(physics.should_kill_resting_owned(owner, airborne)))
    }

    pub(crate) fn prepare_freefall_cleanup_at(&self, owner: &super::Object) -> Result<(), String> {
        self.installed_update_proxy(owner)?;
        self.with_installed_update(|physics| {
            physics.prepare_freefall_cleanup_at();
            Ok(())
        })
    }

    pub(crate) fn finish_owned_at(
        &self,
        owner: &mut super::Object,
        frame: u32,
        airborne_at_end: bool,
    ) -> Result<crate::modules::UpdateSleepTime, String> {
        self.installed_update_proxy(owner)?;
        let PhysicsInterfaceKind::Module(handle) = &self.kind else {
            unreachable!()
        };
        handle.with_module(|module| Ok(module.as_any_mut()
            .downcast_mut::<crate::contain_module_overrides::ActiveBehaviorModule<PhysicsBehaviorUpdate>>()
            .ok_or("installed Physics module absent")?.behavior_mut().finish_owned_at(owner, frame, airborne_at_end)))
    }

    pub(crate) fn installed_is_stunned(&self) -> Result<bool, String> {
        let PhysicsInterfaceKind::Module(handle) = &self.kind else {
            return Err("stun observation requires installed Physics storage".into());
        };
        handle.with_module(|module| module.as_any()
            .downcast_ref::<crate::contain_module_overrides::ActiveBehaviorModule<PhysicsBehaviorUpdate>>()
            .map(|module| module.behavior().is_stunned())
            .ok_or("Physics descriptor no longer references its installed module".into()))
    }

    #[cfg(test)]
    pub(crate) fn accumulated_force_acceleration(&self) -> super::Coord3D {
        let PhysicsInterfaceKind::Module(handle) = &self.kind else {
            panic!("force observation requires actual installed Physics storage");
        };
        handle.with_module(|module| module.as_any()
            .downcast_ref::<crate::contain_module_overrides::ActiveBehaviorModule<PhysicsBehaviorUpdate>>()
            .expect("actual installed Physics module")
            .behavior().accumulated_force_acceleration())
    }

    /// Resolve this descriptor's installed update proxy on the supplied owner.
    /// Class-name matching alone cannot distinguish a peer or duplicate module.
    pub(crate) fn installed_update_proxy(
        &self,
        owner: &super::Object,
    ) -> Result<game_engine::common::thing::update_module::UpdateModulePtr, String> {
        let PhysicsInterfaceKind::Module(handle) = &self.kind else {
            return Err("native locomotion requires installed PhysicsBehavior storage".into());
        };
        let index = owner
            .modules
            .iter()
            .position(|entry| Arc::ptr_eq(entry, &handle.entry))
            .ok_or("Physics descriptor belongs to another Object")?;
        let mut matching = owner
            .update_module_registrations
            .iter()
            .filter(|entry| entry.module_index == Some(index));
        let proxy = matching
            .next()
            .ok_or("installed Physics update proxy absent")?;
        if matching.next().is_some() {
            return Err("installed Physics has duplicate update proxies".into());
        }
        Ok(proxy.module.clone())
    }

    /// Identity comparison without locking. Container and rider can reference
    /// the same physics; callers must not borrow that state twice concurrently.
    pub fn same_instance(&self, other: &Self) -> bool {
        match (&self.kind, &other.kind) {
            (PhysicsInterfaceKind::Module(a), PhysicsInterfaceKind::Module(b)) => {
                Arc::ptr_eq(&a.entry, &b.entry)
            }
            (PhysicsInterfaceKind::Shared(a), PhysicsInterfaceKind::Shared(b)) => Arc::ptr_eq(a, b),
            _ => false,
        }
    }
}

fn map_access<'a, G>(
    result: LockResult<G>,
    map: impl FnOnce(G) -> PhysicsInterfaceLeaseKind<'a>,
) -> LockResult<PhysicsInterfaceLease<'a>> {
    match result {
        Ok(guard) => Ok(PhysicsInterfaceLease { kind: map(guard) }),
        Err(poisoned) => Err(PoisonError::new(PhysicsInterfaceLease {
            kind: map(poisoned.into_inner()),
        })),
    }
}

fn map_try_access<'a, G>(
    result: TryLockResult<G>,
    map: impl FnOnce(G) -> PhysicsInterfaceLeaseKind<'a>,
) -> TryLockResult<PhysicsInterfaceLease<'a>> {
    match result {
        Ok(guard) => Ok(PhysicsInterfaceLease { kind: map(guard) }),
        Err(TryLockError::WouldBlock) => Err(TryLockError::WouldBlock),
        Err(TryLockError::Poisoned(poisoned)) => Err(TryLockError::Poisoned(PoisonError::new(
            PhysicsInterfaceLease {
                kind: map(poisoned.into_inner()),
            },
        ))),
    }
}

impl From<Arc<Mutex<dyn PhysicsBehavior>>> for PhysicsInterfaceHandle {
    fn from(physics: Arc<Mutex<dyn PhysicsBehavior>>) -> Self {
        Self {
            kind: PhysicsInterfaceKind::Shared(physics),
        }
    }
}

impl<T: PhysicsBehavior + 'static> From<Arc<Mutex<T>>> for PhysicsInterfaceHandle {
    fn from(physics: Arc<Mutex<T>>) -> Self {
        let physics: Arc<Mutex<dyn PhysicsBehavior>> = physics;
        Self::from(physics)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::{AsciiString, Coord3D};
    use crate::object::Object;
    use crate::object::behavior::physics_update::PhysicsBehaviorModuleData;
    use game_engine::common::system::{xfer_load::XferLoad, xfer_save::XferSave};
    use game_engine::common::thing::module::ModuleInterfaceType;
    use std::io::Cursor;
    use std::sync::RwLock;

    fn installed_physics(
        owner: &Arc<RwLock<Object>>,
    ) -> (PhysicsInterfaceHandle, BehaviorModuleHandle) {
        let data = Arc::new(PhysicsBehaviorModuleData::default());
        let behavior_data: Arc<dyn crate::common::ModuleData> = data.clone();
        let data: Arc<dyn game_engine::common::thing::module::ModuleData> = data;
        let update = PhysicsBehaviorUpdate::new(Arc::clone(owner), behavior_data).unwrap();
        let module = crate::contain_module_overrides::ActiveBehaviorModule::new(
            "PhysicsBehavior",
            Arc::clone(&data),
            update,
        );
        let mut owner = owner.write().unwrap();
        owner.install_module_for_test(
            "PhysicsBehavior",
            Box::new(module),
            data,
            ModuleInterfaceType::UPDATE,
        );
        let entry = owner
            .module_by_name(&AsciiString::from("PhysicsBehavior"))
            .unwrap();
        let physics = PhysicsInterfaceHandle::from_module(entry.clone());
        owner.set_physics(Some(physics.clone()));
        (physics, entry)
    }

    #[test]
    fn interface_borrows_the_same_owned_state_as_module_xfer() {
        let owner = Arc::new(RwLock::new(Object::new_test(93_520, 100.0)));
        let (physics, module) = installed_physics(&owner);
        let clone = owner.read().unwrap().get_physics().unwrap();
        assert!(physics.same_instance(&clone));
        {
            let mut state = physics.access().unwrap();
            state.set_velocity(&Coord3D::new(1.0, 2.0, 3.0));
            assert!(
                matches!(clone.try_access(), Err(TryLockError::WouldBlock)),
                "both descriptors borrow the canonical state"
            );
        }
        module.with_module(|module| {
            assert_eq!(
                module_physics(module).get_velocity(),
                Coord3D::new(1.0, 2.0, 3.0)
            );
        });
        let mut bytes = Vec::new();
        module.with_module(|module| {
            module
                .xfer(&mut XferSave::new(Cursor::new(&mut bytes), 1))
                .unwrap()
        });
        physics.access().unwrap().set_velocity(&Coord3D::ZERO);
        module.with_module(|module| {
            module
                .xfer(&mut XferLoad::new(Cursor::new(&bytes), 1))
                .unwrap()
        });
        assert_eq!(
            clone.access().unwrap().get_velocity(),
            Coord3D::new(1.0, 2.0, 3.0)
        );
    }

    #[test]
    fn equal_owner_ids_and_retired_descriptors_preserve_module_identity() {
        let first_owner = Arc::new(RwLock::new(Object::new_test(93_521, 100.0)));
        let (first, _) = installed_physics(&first_owner);
        first
            .access()
            .unwrap()
            .set_velocity(&Coord3D::new(4.0, 0.0, 0.0));
        first_owner.write().unwrap().physics = None;
        drop(first_owner);
        let next_owner = Arc::new(RwLock::new(Object::new_test(93_521, 100.0)));
        let (next, _) = installed_physics(&next_owner);
        next.access()
            .unwrap()
            .set_velocity(&Coord3D::new(9.0, 0.0, 0.0));
        assert!(!first.same_instance(&next));
        assert_eq!(first.access().unwrap().get_velocity().x, 4.0);
        assert_eq!(next.access().unwrap().get_velocity().x, 9.0);
        first
            .access()
            .unwrap()
            .set_velocity(&Coord3D::new(2.0, 0.0, 0.0));
        assert_eq!(next.access().unwrap().get_velocity().x, 9.0);
    }
}
