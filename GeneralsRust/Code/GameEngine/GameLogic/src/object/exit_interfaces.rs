//! Owned descriptors of the existing installed exit runtime.
//!
//! C++ Object.cpp:2535-2558 returns the first behavior interface, then the
//! containment fallback. Queries retain that exact runtime and allocate no
//! separate synchronized interface wrapper. Every delegated operation still
//! borrows the genuine module/behavior/contain storage, with its existing
//! failure policy. Descriptors contain no mutable gameplay or snapshot state.

use super::*;

pub struct ExitInterfaceHandle {
    kind: ExitInterfaceKind,
}

enum ExitInterfaceKind {
    Module(ModuleExitSource),
    Behavior(BehaviorExitSource),
    Contain(ContainExitSource),
}

impl ExitInterfaceHandle {
    pub(super) fn module(entry: Arc<ModuleEntry>) -> Self {
        Self {
            kind: ExitInterfaceKind::Module(ModuleExitSource { entry }),
        }
    }

    pub(super) fn behavior(behavior: BehaviorInterfaceHandle) -> Self {
        Self {
            kind: ExitInterfaceKind::Behavior(BehaviorExitSource { behavior }),
        }
    }

    pub(super) fn contain(contain: Arc<Mutex<dyn ContainModuleInterface>>) -> Self {
        Self {
            kind: ExitInterfaceKind::Contain(ContainExitSource { contain }),
        }
    }

    fn interface(&self) -> &dyn ExitInterface {
        match &self.kind {
            ExitInterfaceKind::Module(source) => source,
            ExitInterfaceKind::Behavior(source) => source,
            ExitInterfaceKind::Contain(source) => source,
        }
    }

    pub(super) fn interface_mut(&mut self) -> &mut dyn ExitInterface {
        match &mut self.kind {
            ExitInterfaceKind::Module(source) => source,
            ExitInterfaceKind::Behavior(source) => source,
            ExitInterfaceKind::Contain(source) => source,
        }
    }

    /// Existing production reservation adapter for a template name and optional
    /// spawned ObjectID. Name/ID discovery is a separate legacy dependency;
    /// the descriptor itself needs no lock. Borrowed-object callers use the
    /// ordinary ExitInterface operation instead.
    pub(crate) fn reserve_door_for_template(
        &mut self,
        spawner: Option<&str>,
        spawn: Option<ObjectID>,
    ) -> crate::modules::ExitDoorType {
        if let Some(name) = spawner {
            if crate::helpers::TheThingFactory::find_template(name).is_some_and(|template| {
                template.is_kind_of(crate::common::KindOf::ProducedAtHelipad)
            }) {
                return crate::modules::ExitDoorType::None;
            }
        }
        let spawn_obj = match spawn {
            Some(id) => {
                let Some(obj) = crate::helpers::TheGameLogic::find_object_by_id(id)
                    .or_else(|| crate::object::registry::OBJECT_REGISTRY.get_object(id))
                else {
                    return crate::modules::ExitDoorType::NoneAvailable;
                };
                Some(obj)
            }
            None => None,
        };
        match spawn_obj.as_ref() {
            Some(obj) => {
                let Ok(spawn_guard) = obj.read() else {
                    return crate::modules::ExitDoorType::NoneAvailable;
                };
                self.reserve_door_for_exit(None, Some(&*spawn_guard))
            }
            None => self.reserve_door_for_exit(None, None),
        }
    }
}

impl ExitInterface for ExitInterfaceHandle {
    fn can_exit(&self, object_id: ObjectID) -> bool {
        self.interface().can_exit(object_id)
    }
    fn exit(&mut self, object_id: ObjectID) -> bool {
        self.interface_mut().exit(object_id)
    }
    fn get_rally_point(&self) -> Result<Option<Coord3D>, Box<dyn std::error::Error + Send + Sync>> {
        self.interface().get_rally_point()
    }
    fn get_exit_position(&self, exit_position: &mut Coord3D) -> bool {
        self.interface().get_exit_position(exit_position)
    }
    fn get_natural_rally_point(&self, rally_point: &mut Coord3D, offset: bool) -> bool {
        self.interface()
            .get_natural_rally_point(rally_point, offset)
    }
    fn reserve_door_for_exit(
        &mut self,
        spawner: Option<&Object>,
        spawn: Option<&Object>,
    ) -> crate::modules::ExitDoorType {
        self.interface_mut().reserve_door_for_exit(spawner, spawn)
    }
    fn unreserve_door_for_exit(&mut self, door: crate::modules::ExitDoorType) {
        self.interface_mut().unreserve_door_for_exit(door)
    }
    fn exit_object_via_door(
        &mut self,
        obj_id: ObjectID,
        door: crate::modules::ExitDoorType,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.interface_mut().exit_object_via_door(obj_id, door)
    }
    fn exit_object_in_a_hurry(
        &mut self,
        obj_id: ObjectID,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.interface_mut().exit_object_in_a_hurry(obj_id)
    }
    fn exit_object_by_budding(
        &mut self,
        obj_id: ObjectID,
        host_id: Option<ObjectID>,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.interface_mut().exit_object_by_budding(obj_id, host_id)
    }
    fn use_spawn_rally_point(&self) -> bool {
        self.interface().use_spawn_rally_point()
    }
    fn is_exit_busy(&self) -> bool {
        self.interface().is_exit_busy()
    }
}

struct BehaviorExitSource {
    behavior: BehaviorInterfaceHandle,
}

struct ContainExitSource {
    contain: Arc<Mutex<dyn ContainModuleInterface>>,
}

struct ModuleExitSource {
    entry: Arc<ModuleEntry>,
}

impl ModuleExitSource {
    fn with_exit_behavior<F, R>(&self, func: F) -> Option<R>
    where
        F: FnOnce(&mut dyn ExitInterface) -> R,
    {
        self.entry.with_module(|module| {
            module_production_behavior_kind(module)
                .and_then(ProductionBehaviorModuleKindMut::into_exit_interface)
                .map(func)
        })
    }
}

impl ExitInterface for BehaviorExitSource {
    fn can_exit(&self, object_id: ObjectID) -> bool {
        let mut behavior = self.behavior.clone();
        if let Ok(mut guard) = behavior.access() {
            if let Some(exit_interface) = guard.get_update_exit_interface() {
                return exit_interface.can_exit(object_id);
            }
        }
        false
    }

    fn exit(&mut self, object_id: ObjectID) -> bool {
        if let Ok(mut guard) = self.behavior.access() {
            if let Some(exit_interface) = guard.get_update_exit_interface() {
                return exit_interface.exit(object_id);
            }
        }
        false
    }

    fn get_rally_point(&self) -> Result<Option<Coord3D>, Box<dyn std::error::Error + Send + Sync>> {
        let mut behavior = self.behavior.clone();
        if let Ok(mut guard) = behavior.access() {
            if let Some(exit_interface) = guard.get_update_exit_interface() {
                return exit_interface.get_rally_point();
            }
        }
        Ok(None)
    }

    fn reserve_door_for_exit(
        &mut self,
        spawner: Option<&crate::object::Object>,
        spawn: Option<&crate::object::Object>,
    ) -> crate::modules::ExitDoorType {
        if let Ok(mut guard) = self.behavior.access() {
            if let Some(exit_interface) = guard.get_update_exit_interface() {
                return exit_interface.reserve_door_for_exit(spawner, spawn);
            }
        }
        crate::modules::DOOR_NONE_AVAILABLE
    }

    fn unreserve_door_for_exit(&mut self, door: crate::modules::ExitDoorType) {
        if let Ok(mut guard) = self.behavior.access() {
            if let Some(exit_interface) = guard.get_update_exit_interface() {
                exit_interface.unreserve_door_for_exit(door);
            }
        }
    }

    fn exit_object_via_door(
        &mut self,
        obj_id: ObjectID,
        door: crate::modules::ExitDoorType,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        // Wave 264: empty dual-world → Ok(()).
        if dual_world_registry_unavailable() {
            return Ok(());
        }

        if crate::object::registry::OBJECT_REGISTRY
            .get_object(obj_id)
            .is_none()
        {
            return Ok(());
        }

        if let Ok(mut guard) = self.behavior.access() {
            if let Some(exit_interface) = guard.get_update_exit_interface() {
                return exit_interface.exit_object_via_door(obj_id, door);
            }
        }
        Ok(())
    }

    fn exit_object_in_a_hurry(
        &mut self,
        obj_id: ObjectID,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        // Wave 264: empty dual-world → Ok(()).
        if dual_world_registry_unavailable() {
            return Ok(());
        }

        let Some(obj) = crate::object::registry::OBJECT_REGISTRY.get_object(obj_id) else {
            return Ok(());
        };

        if let Ok(mut guard) = self.behavior.access() {
            if let Some(exit_interface) = guard.get_update_exit_interface() {
                return exit_interface.exit_object_in_a_hurry(obj_id);
            }
        }
        Ok(())
    }

    fn exit_object_by_budding(
        &mut self,
        obj_id: ObjectID,
        host_id: Option<ObjectID>,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        // Wave 264: empty dual-world → Ok(()).
        if dual_world_registry_unavailable() {
            return Ok(());
        }

        let Some(obj) = crate::object::registry::OBJECT_REGISTRY.get_object(obj_id) else {
            return Ok(());
        };

        if let Ok(mut guard) = self.behavior.access() {
            if let Some(exit_interface) = guard.get_update_exit_interface() {
                return exit_interface.exit_object_by_budding(obj_id, host_id);
            }
        }
        Ok(())
    }
}

impl ExitInterface for ContainExitSource {
    fn can_exit(&self, object_id: ObjectID) -> bool {
        self.contain
            .lock()
            .map(|guard| guard.can_exit(object_id))
            .unwrap_or(false)
    }

    fn exit(&mut self, object_id: ObjectID) -> bool {
        let Some(obj) = TheGameLogic::find_object_by_id(object_id) else {
            return false;
        };
        let exit_id = obj.read().map(|g| g.get_id()).unwrap_or(0);
        self.exit_object_via_door(exit_id, crate::modules::ExitDoorType::Primary)
            .is_ok()
    }

    fn get_rally_point(&self) -> Result<Option<Coord3D>, Box<dyn std::error::Error + Send + Sync>> {
        Ok(self
            .contain
            .lock()
            .ok()
            .and_then(|guard| guard.get_rally_point()))
    }

    fn reserve_door_for_exit(
        &mut self,
        spawner: Option<&crate::object::Object>,
        spawn: Option<&crate::object::Object>,
    ) -> crate::modules::ExitDoorType {
        self.contain
            .lock()
            .map(|mut guard| guard.reserve_door_for_exit(spawner, spawn))
            .unwrap_or(crate::modules::ExitDoorType::NoneAvailable)
    }

    fn unreserve_door_for_exit(&mut self, door: crate::modules::ExitDoorType) {
        if let Ok(mut guard) = self.contain.lock() {
            guard.unreserve_door_for_exit(door);
        }
    }

    fn exit_object_via_door(
        &mut self,
        obj_id: ObjectID,
        door: crate::modules::ExitDoorType,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        // Wave 264: empty dual-world → Ok(()).
        if dual_world_registry_unavailable() {
            return Ok(());
        }

        if crate::object::registry::OBJECT_REGISTRY
            .get_object(obj_id)
            .is_none()
        {
            return Ok(());
        }

        self.contain
            .lock()
            .map_err(|_| "failed to lock contain exit interface".into())
            .and_then(|mut guard| guard.exit_object_via_door(obj_id, door))
    }

    fn exit_object_in_a_hurry(
        &mut self,
        obj_id: ObjectID,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        // Wave 264: empty dual-world → Ok(()).
        if dual_world_registry_unavailable() {
            return Ok(());
        }

        let Some(obj) = crate::object::registry::OBJECT_REGISTRY.get_object(obj_id) else {
            return Ok(());
        };

        self.contain
            .lock()
            .map_err(|_| "failed to lock contain exit interface".into())
            .and_then(|mut guard| guard.exit_object_in_a_hurry(obj_id))
    }
}

impl ExitInterface for ModuleExitSource {
    fn can_exit(&self, object_id: ObjectID) -> bool {
        self.with_exit_behavior(|module| module.can_exit(object_id))
            .unwrap_or(false)
    }

    fn exit(&mut self, object_id: ObjectID) -> bool {
        self.with_exit_behavior(|module| module.exit(object_id))
            .unwrap_or(false)
    }

    fn get_rally_point(&self) -> Result<Option<Coord3D>, Box<dyn std::error::Error + Send + Sync>> {
        Ok(self
            .with_exit_behavior(|module| module.get_rally_point())
            .transpose()?
            .flatten())
    }

    fn get_exit_position(&self, exit_position: &mut Coord3D) -> bool {
        self.with_exit_behavior(|module| module.get_exit_position(exit_position))
            .unwrap_or(false)
    }

    fn get_natural_rally_point(&self, rally_point: &mut Coord3D, offset: bool) -> bool {
        self.with_exit_behavior(|module| module.get_natural_rally_point(rally_point, offset))
            .unwrap_or(false)
    }

    fn reserve_door_for_exit(
        &mut self,
        spawner: Option<&crate::object::Object>,
        spawn: Option<&crate::object::Object>,
    ) -> crate::modules::ExitDoorType {
        self.with_exit_behavior(|module| module.reserve_door_for_exit(spawner, spawn))
            .unwrap_or(crate::modules::DOOR_NONE_AVAILABLE)
    }

    fn unreserve_door_for_exit(&mut self, door: crate::modules::ExitDoorType) {
        let _ = self.with_exit_behavior(|module| {
            module.unreserve_door_for_exit(door);
        });
    }

    fn exit_object_via_door(
        &mut self,
        obj_id: ObjectID,
        door: crate::modules::ExitDoorType,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        // Wave 264: empty dual-world → Ok(()).
        if dual_world_registry_unavailable() {
            return Ok(());
        }

        if crate::object::registry::OBJECT_REGISTRY
            .get_object(obj_id)
            .is_none()
        {
            return Ok(());
        }

        self.with_exit_behavior(|module| module.exit_object_via_door(obj_id, door))
            .unwrap_or(Ok(()))
    }

    fn exit_object_in_a_hurry(
        &mut self,
        obj_id: ObjectID,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        // Wave 264: empty dual-world → Ok(()).
        if dual_world_registry_unavailable() {
            return Ok(());
        }

        let Some(obj) = crate::object::registry::OBJECT_REGISTRY.get_object(obj_id) else {
            return Ok(());
        };

        self.with_exit_behavior(|module| module.exit_object_in_a_hurry(obj_id))
            .unwrap_or(Ok(()))
    }

    fn exit_object_by_budding(
        &mut self,
        obj_id: ObjectID,
        host_id: Option<ObjectID>,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        // Wave 264: empty dual-world → Ok(()).
        if dual_world_registry_unavailable() {
            return Ok(());
        }

        let Some(obj) = crate::object::registry::OBJECT_REGISTRY.get_object(obj_id) else {
            return Ok(());
        };

        self.with_exit_behavior(|module| module.exit_object_by_budding(obj_id, host_id))
            .unwrap_or(Ok(()))
    }
}
