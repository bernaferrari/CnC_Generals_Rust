//! Owned compatibility views of the classic Object behavior list.
//!
//! Helpers and template views contain no mutable gameplay state. Detached
//! views retain the authoritative ModuleEntry so callbacks may release the
//! Object borrow without resolving an ObjectId through another world.

use super::*;
use std::ops::{Deref, DerefMut};

#[derive(Clone)]
pub struct BehaviorInterfaceHandle {
    kind: BehaviorInterfaceKind,
}

#[derive(Clone)]
enum BehaviorInterfaceKind {
    Helper(CtorHelperBehavior),
    Template(TemplateModuleBehavior),
    #[cfg(any(test, feature = "internal"))]
    Injected(Arc<Mutex<dyn BehaviorModuleInterface>>),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BehaviorAccessError {
    Poisoned,
    Busy,
}

impl fmt::Display for BehaviorAccessError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Poisoned => "injected behavior mutex poisoned",
            Self::Busy => "injected behavior is already in use",
        })
    }
}

impl std::error::Error for BehaviorAccessError {}

pub struct BehaviorInterfaceLease<'a> {
    kind: BehaviorInterfaceLeaseKind<'a>,
}

enum BehaviorInterfaceLeaseKind<'a> {
    Owned(&'a mut dyn BehaviorModuleInterface),
    #[cfg(any(test, feature = "internal"))]
    Injected(std::sync::MutexGuard<'a, dyn BehaviorModuleInterface>),
}

impl Deref for BehaviorInterfaceLease<'_> {
    type Target = dyn BehaviorModuleInterface;

    fn deref(&self) -> &Self::Target {
        match &self.kind {
            BehaviorInterfaceLeaseKind::Owned(value) => &**value,
            #[cfg(any(test, feature = "internal"))]
            BehaviorInterfaceLeaseKind::Injected(value) => &**value,
        }
    }
}

impl DerefMut for BehaviorInterfaceLease<'_> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        match &mut self.kind {
            BehaviorInterfaceLeaseKind::Owned(value) => &mut **value,
            #[cfg(any(test, feature = "internal"))]
            BehaviorInterfaceLeaseKind::Injected(value) => &mut **value,
        }
    }
}

impl BehaviorInterfaceLease<'_> {
    /// Forward downcasts to the behavior, never the lease's blanket AsAny impl.
    pub fn as_any(&self) -> &dyn Any {
        crate::common::types::AsAny::as_any(&**self)
    }

    pub fn as_any_mut(&mut self) -> &mut dyn Any {
        crate::common::types::AsAny::as_any_mut(&mut **self)
    }
}

impl BehaviorInterfaceHandle {
    pub(super) fn helper(name: &'static str) -> Self {
        Self {
            kind: BehaviorInterfaceKind::Helper(CtorHelperBehavior { name }),
        }
    }

    pub(super) fn template(entry: Arc<ModuleEntry>) -> Self {
        Self {
            kind: BehaviorInterfaceKind::Template(TemplateModuleBehavior { entry }),
        }
    }

    #[cfg(any(test, feature = "internal"))]
    pub(super) fn injected(behavior: Arc<Mutex<dyn BehaviorModuleInterface>>) -> Self {
        Self {
            kind: BehaviorInterfaceKind::Injected(behavior),
        }
    }

    #[cfg(test)]
    pub(super) fn template_entry_for_test(&self) -> Option<&Arc<ModuleEntry>> {
        match &self.kind {
            BehaviorInterfaceKind::Template(value) => Some(&value.entry),
            _ => None,
        }
    }

    /// Match immutable wrapper metadata without constructing a detached view.
    /// Injected test behaviors preserve their previous poisoned-lock policy.
    pub(super) fn matches_name(&self, name: &str) -> bool {
        match &self.kind {
            BehaviorInterfaceKind::Helper(value) => value.name == name,
            BehaviorInterfaceKind::Template(value) => value.entry.name().as_str() == name,
            #[cfg(any(test, feature = "internal"))]
            BehaviorInterfaceKind::Injected(value) => value
                .lock()
                .map(|behavior| behavior.get_module_name() == name)
                .unwrap_or(false),
        }
    }

    /// Borrow this owned view. Template callbacks still enter the one real
    /// module; the lease itself neither locks nor copies gameplay state.
    pub fn access(&mut self) -> Result<BehaviorInterfaceLease<'_>, BehaviorAccessError> {
        let kind = match &mut self.kind {
            BehaviorInterfaceKind::Helper(value) => BehaviorInterfaceLeaseKind::Owned(value),
            BehaviorInterfaceKind::Template(value) => BehaviorInterfaceLeaseKind::Owned(value),
            #[cfg(any(test, feature = "internal"))]
            BehaviorInterfaceKind::Injected(value) => BehaviorInterfaceLeaseKind::Injected(
                value.lock().map_err(|_| BehaviorAccessError::Poisoned)?,
            ),
        };
        Ok(BehaviorInterfaceLease { kind })
    }

    /// Test/internal shared behavior retains its nonblocking access policy.
    /// Production views have only owned metadata and are always available.
    pub fn try_access(&mut self) -> Result<BehaviorInterfaceLease<'_>, BehaviorAccessError> {
        let kind = match &mut self.kind {
            BehaviorInterfaceKind::Helper(value) => BehaviorInterfaceLeaseKind::Owned(value),
            BehaviorInterfaceKind::Template(value) => BehaviorInterfaceLeaseKind::Owned(value),
            #[cfg(any(test, feature = "internal"))]
            BehaviorInterfaceKind::Injected(value) => BehaviorInterfaceLeaseKind::Injected(
                value.try_lock().map_err(|error| match error {
                    std::sync::TryLockError::Poisoned(_) => BehaviorAccessError::Poisoned,
                    std::sync::TryLockError::WouldBlock => BehaviorAccessError::Busy,
                })?,
            ),
        };
        Ok(BehaviorInterfaceLease { kind })
    }
}

/// C++ helper modules live on `m_behaviors` so destroy/damage/xfer walk them.
#[derive(Clone)]
struct CtorHelperBehavior {
    name: &'static str,
}

impl BehaviorModuleInterface for CtorHelperBehavior {
    fn get_module_name(&self) -> &str {
        self.name
    }
}

/// Template `ModuleEntry` listed after helpers on `get_behavior_modules()`.
#[derive(Clone)]
struct TemplateModuleBehavior {
    entry: Arc<ModuleEntry>,
}

impl BehaviorModuleInterface for TemplateModuleBehavior {
    fn get_module_name(&self) -> &str {
        self.entry.name().as_str()
    }

    fn get_destroy(&mut self) -> Option<&mut dyn crate::modules::DestroyModuleInterface> {
        if (self.entry.mask().0 & ModuleInterfaceType::DESTROY.0) != 0 {
            Some(self)
        } else {
            None
        }
    }

    fn get_damage(&mut self) -> Option<&mut dyn crate::modules::DamageModuleInterface> {
        if (self.entry.mask().0 & ModuleInterfaceType::DAMAGE.0) != 0 {
            Some(self)
        } else {
            None
        }
    }
}

impl crate::modules::DestroyModuleInterface for TemplateModuleBehavior {
    fn on_destroy(&mut self, object_id: ObjectID) {
        let _ = object_id;
        self.entry.with_module(|module| module.on_delete());
    }
}

impl crate::modules::DamageModuleInterface for TemplateModuleBehavior {
    fn receive_damage(&mut self, object_id: ObjectID, damage: &DamageInfo) -> Real {
        let _ = (object_id, damage);
        0.0
    }

    fn on_damage(
        &mut self,
        damage_info: &mut DamageInfo,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.entry.with_module(|module| {
            if let Some(auto_heal) = (module as &mut dyn Any).downcast_mut::<
                crate::object::behavior::auto_heal_behavior::AutoHealBehaviorModule,
            >() {
                return auto_heal.behavior_mut().on_damage(damage_info);
            }
            if let Some(bridge) = (module as &mut dyn Any)
                .downcast_mut::<crate::object::behavior::bridge_behavior::BridgeBehaviorModule>(
            ) {
                return bridge.behavior_mut().on_damage(damage_info);
            }
            if let Some(tower) = (module as &mut dyn Any).downcast_mut::<
                crate::object::behavior::bridge_tower_behavior::BridgeTowerBehaviorModule,
            >() {
                return tower.behavior_mut().on_damage(damage_info);
            }
            Ok(())
        })
    }

    fn on_healing(
        &mut self,
        damage_info: &mut DamageInfo,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.entry.with_module(|module| {
            if let Some(bridge) = (module as &mut dyn Any)
                .downcast_mut::<crate::object::behavior::bridge_behavior::BridgeBehaviorModule>(
            ) {
                return bridge.behavior_mut().on_healing(damage_info);
            }
            if let Some(tower) = (module as &mut dyn Any).downcast_mut::<
                crate::object::behavior::bridge_tower_behavior::BridgeTowerBehaviorModule,
            >() {
                return tower.behavior_mut().on_healing(damage_info);
            }
            Ok(())
        })
    }

    fn on_body_damage_state_change(
        &mut self,
        damage_info: &DamageInfo,
        old_state: crate::damage::BodyDamageType,
        new_state: crate::damage::BodyDamageType,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.entry.with_module(|module| {
            if let Some(bridge) = (module as &mut dyn Any)
                .downcast_mut::<crate::object::behavior::bridge_behavior::BridgeBehaviorModule>(
            ) {
                return bridge.behavior_mut().on_body_damage_state_change(
                    damage_info,
                    old_state,
                    new_state,
                );
            }
            if let Some(tower) = (module as &mut dyn Any).downcast_mut::<
                crate::object::behavior::bridge_tower_behavior::BridgeTowerBehaviorModule,
            >() {
                return tower.behavior_mut().on_body_damage_state_change(
                    damage_info,
                    old_state,
                    new_state,
                );
            }
            Ok(())
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct InjectedCounter(u32);
    impl BehaviorModuleInterface for InjectedCounter {}

    #[test]
    fn injected_object_behavior_views_preserve_downcasts_and_busy_poison_policy() {
        let counter: Arc<Mutex<dyn BehaviorModuleInterface>> =
            Arc::new(Mutex::new(InjectedCounter(4)));
        let mut object = Object::new_test(0xD157, 100.0);
        object.push_behavior_module_for_test(counter.clone());
        let mut handle = object.get_behavior_modules().into_iter().last().unwrap();
        let mut detached = handle.clone();
        object.behaviors.clear();
        {
            let _original_guard = counter.lock().unwrap();
            assert!(matches!(
                handle.try_access(),
                Err(BehaviorAccessError::Busy)
            ));
        }
        {
            let mut lease = handle.access().unwrap();
            assert_eq!(
                lease.as_any().downcast_ref::<InjectedCounter>().unwrap().0,
                4
            );
            lease
                .as_any_mut()
                .downcast_mut::<InjectedCounter>()
                .unwrap()
                .0 = 9;
        }
        assert_eq!(
            detached
                .access()
                .unwrap()
                .as_any()
                .downcast_ref::<InjectedCounter>()
                .unwrap()
                .0,
            9
        );
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _guard = counter.lock().unwrap();
            panic!("injected behavior poison probe");
        }));
        assert!(result.is_err());
        assert!(matches!(
            handle.access(),
            Err(BehaviorAccessError::Poisoned)
        ));
        assert!(matches!(
            detached.try_access(),
            Err(BehaviorAccessError::Poisoned)
        ));
    }
}
