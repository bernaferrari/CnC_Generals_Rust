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
        let _lock = crate::test_sync::lock();
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
        let _lock = crate::test_sync::lock();
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
