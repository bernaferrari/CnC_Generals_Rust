//! Shared factory helpers/wrappers for module-override registration.
//! Split from `contain_module_overrides.rs`. Observable factory behavior is unchanged.

use super::*;

pub(super) fn resolve_owner_id(thing: &Arc<dyn ModuleThing>) -> ObjectID {
    thing
        .as_object()
        .map(ModuleObjectTrait::get_object_id)
        .unwrap_or(INVALID_ID)
}

pub(super) fn resolve_owner_info(thing: &Arc<dyn ModuleThing>) -> (ObjectID, Coord3D) {
    let owner_id = resolve_owner_id(thing);
    let position = TheGameLogic::find_object_by_id(owner_id)
        .and_then(|object| object.read().ok().map(|guard| *guard.get_position()))
        .unwrap_or_default();
    (owner_id, position)
}

/// Error carrier for the engine's infallible create-proc ABI. ModuleFactory
/// rejects it before installation or scheduling; it never substitutes behavior.
struct MissingOwnerModule {
    module_name_key: NameKeyType,
    data: ModuleDataHandle,
    error: String,
}

impl MissingOwnerModule {
    fn new(module_name: &str, data: ModuleDataHandle, error: String) -> Self {
        Self {
            module_name_key: NameKeyGenerator::name_to_key(module_name),
            data,
            error,
        }
    }
}

impl Module for MissingOwnerModule {
    fn validate_initialization(&self) -> Result<(), String> {
        Err(self.error.clone())
    }

    fn get_module_name_key(&self) -> NameKeyType {
        self.module_name_key
    }

    fn get_module_tag_name_key(&self) -> NameKeyType {
        self.data.read().get_module_tag_name_key()
    }

    fn get_module_data(&self) -> ModuleDataRead<'_> {
        ModuleDataRead::shared(&self.data)
    }
}

impl Snapshotable for MissingOwnerModule {
    fn crc(&self, _xfer: &mut dyn Xfer) -> Result<(), String> {
        self.validate_initialization()
    }

    fn xfer(&mut self, _xfer: &mut dyn Xfer) -> Result<(), String> {
        self.validate_initialization()
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        self.validate_initialization()
    }
}

pub(super) fn failed_initialization_module(
    module_name: &str,
    module_data: ModuleDataHandle,
    error: String,
) -> Box<dyn Module> {
    Box::new(MissingOwnerModule::new(module_name, module_data, error))
}

pub(super) fn missing_owner_module(
    module_name: &str,
    module_data: ModuleDataHandle,
) -> Box<dyn Module> {
    failed_initialization_module(
        module_name,
        module_data,
        "supplied module owner unavailable".into(),
    )
}

pub(super) fn missing_owner_module_auto(
    module_name: &str,
    module_data: &ModuleDataHandle,
) -> Box<dyn Module> {
    missing_owner_module(module_name, module_data.clone())
}

pub(super) fn resolve_drawable_id(thing: &Arc<dyn ModuleThing>) -> u32 {
    thing
        .as_drawable()
        .map(ModuleDrawableTrait::get_drawable_id)
        .unwrap_or(INVALID_ID)
}

pub(super) fn owner_weak(owner_id: ObjectID) -> Weak<RwLock<crate::object::Object>> {
    TheGameLogic::find_object_by_id(owner_id)
        .map(|arc| Arc::downgrade(&arc))
        .unwrap_or_else(Weak::new)
}

#[derive(Debug)]
pub(crate) struct ActiveBehaviorModule<T: BehaviorModuleInterface + Snapshotable + 'static> {
    module_name_key: NameKeyType,
    data: ModuleDataHandle,
    behavior: T,
}

impl<T: BehaviorModuleInterface + Snapshotable + 'static> ActiveBehaviorModule<T> {
    pub(crate) fn new(module_name: &str, data: ModuleDataHandle, behavior: T) -> Self {
        Self {
            module_name_key: NameKeyGenerator::name_to_key(module_name),
            data,
            behavior,
        }
    }

    pub(crate) fn behavior(&self) -> &T {
        &self.behavior
    }

    pub(crate) fn behavior_mut(&mut self) -> &mut T {
        &mut self.behavior
    }
}

impl<T: BehaviorModuleInterface + Snapshotable + 'static> Module for ActiveBehaviorModule<T> {
    fn get_module_data(&self) -> ModuleDataRead<'_> {
        ModuleDataRead::shared(&self.data)
    }

    fn get_module_name_key(&self) -> NameKeyType {
        self.module_name_key
    }

    fn get_module_tag_name_key(&self) -> NameKeyType {
        self.data.read().get_module_tag_name_key()
    }

    fn get_deletion_lifetime_interface(&mut self) -> Option<&mut dyn DeletionLifetimeInterface> {
        self.behavior.get_deletion_lifetime_interface()
    }

    /// Sleepy-update dispatch delegates to the inner behavior's explicit
    /// opt-in, so only the wrapper instantiations that the previous type-test
    /// table listed take part (C++: only classes deriving `UpdateModule`
    /// answer the `DynamicInterfaceCast(UPDATE)` query).
    fn get_update_module_interface(
        &mut self,
    ) -> Option<&mut dyn game_engine::common::thing::update_module::UpdateModuleInterface> {
        self.behavior.get_sleepy_update_interface()
    }

    fn get_sleepy_update_interface(
        &mut self,
    ) -> Option<&mut dyn game_engine::common::thing::update_module::UpdateModuleInterface> {
        // Preserve the inner class's explicit opt-in for phase and disabled
        // metadata, just as the update dispatcher does. Physics is phase 1.
        self.behavior.get_sleepy_update_interface()
    }

    fn get_initial_wake_frame(&mut self) -> Option<u32> {
        self.behavior.behavior_initial_wake_frame()
    }
}

impl<T: BehaviorModuleInterface + Snapshotable + 'static> Snapshotable for ActiveBehaviorModule<T> {
    fn crc(&self, xfer: &mut dyn Xfer) -> Result<(), String> {
        self.behavior.crc(xfer)
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        self.behavior.xfer(xfer)
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        self.behavior.load_post_process()
    }
}

pub(super) fn active_behavior_module<TBehavior, TData>(
    thing: Arc<dyn ModuleThing>,
    module_data: ModuleDataHandle,
    module_name: &str,
    create: fn(
        Arc<RwLock<crate::object::Object>>,
        ModuleDataHandle,
    ) -> Result<TBehavior, Box<dyn std::error::Error + Send + Sync>>,
) -> Box<dyn Module>
where
    TBehavior: BehaviorModuleInterface + Snapshotable + 'static,
    TData: ModuleData + LegacyModuleData + Clone + 'static,
{
    if !module_data.read().as_any().is::<TData>() {
        return failed_initialization_module(
            module_name,
            module_data,
            format!("expected module data {}", std::any::type_name::<TData>()),
        );
    }
    // C++ constructs the module for the supplied Thing. A numeric ID cannot
    // select another match's owner, including when a retired ID is reused.
    let object = match crate::object::supplied_module_owner(thing.as_ref()) {
        Ok(object) => object,
        Err(error) => {
            return failed_initialization_module(module_name, module_data, error);
        }
    };
    let behavior = match create(object, module_data.clone()) {
        Ok(behavior) => behavior,
        Err(err) => {
            return failed_initialization_module(module_name, module_data, err.to_string());
        }
    };
    Box::new(ActiveBehaviorModule::new(
        module_name,
        module_data,
        behavior,
    ))
}

#[derive(Debug)]
struct ActiveCreateModule<T: CreateInterface + Snapshotable + Send + Sync + 'static> {
    module_name_key: NameKeyType,
    data: ModuleDataHandle,
    create: T,
}

impl<T: CreateInterface + Snapshotable + Send + Sync + 'static> ActiveCreateModule<T> {
    fn new(module_name: &str, data: ModuleDataHandle, create: T) -> Self {
        Self {
            module_name_key: NameKeyGenerator::name_to_key(module_name),
            data,
            create,
        }
    }
}

impl<T: CreateInterface + Snapshotable + Send + Sync + 'static> Module for ActiveCreateModule<T> {
    fn get_module_name_key(&self) -> NameKeyType {
        self.module_name_key
    }

    fn get_module_tag_name_key(&self) -> NameKeyType {
        self.data.read().get_module_tag_name_key()
    }

    fn get_module_data(&self) -> ModuleDataRead<'_> {
        ModuleDataRead::shared(&self.data)
    }

    fn get_create_interface(&self) -> Option<&dyn CreateInterface> {
        Some(&self.create)
    }
}

impl<T: CreateInterface + Snapshotable + Send + Sync + 'static> Snapshotable
    for ActiveCreateModule<T>
{
    fn crc(&self, xfer: &mut dyn Xfer) -> Result<(), String> {
        self.create.crc(xfer)
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        self.create.xfer(xfer)
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        self.create.load_post_process()
    }
}

macro_rules! parsed_create_factories {
    ($data_factory:ident, $module_factory:ident, $data_ty:ty, $module_ty:ty, $module_name:literal) => {
        pub(super) fn $data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
            let mut data = <$data_ty>::default();
            if let Some(ini) = ini {
                if let Err(err) = data.parse_from_ini(ini) {
                    warn!(
                        "Failed to parse {} module data at line {}: {}",
                        $module_name,
                        ini.get_line_num(),
                        err
                    );
                }
            }
            Box::new(data)
        }

        pub(super) fn $module_factory(
            thing: Arc<dyn ModuleThing>,
            module_data: ModuleDataHandle,
        ) -> Box<dyn Module> {
            Box::new(ActiveCreateModule::new(
                $module_name,
                module_data.clone(),
                <$module_ty>::new(thing, module_data),
            ))
        }
    };
}

macro_rules! empty_create_factories {
    ($data_factory:ident, $module_factory:ident, $module_ty:ty, $module_name:literal) => {
        pub(super) fn $data_factory(_ini: Option<&mut INI>) -> Box<dyn ModuleData> {
            Box::new(CreateModuleData::default())
        }

        pub(super) fn $module_factory(
            thing: Arc<dyn ModuleThing>,
            module_data: ModuleDataHandle,
        ) -> Box<dyn Module> {
            assert!(module_data.read().as_any().is::<CreateModuleData>(), "{} module data type expected", $module_name);
            let engine_data = module_data;
            Box::new(ActiveCreateModule::new(
                $module_name,
                engine_data,
                <$module_ty>::new(thing),
            ))
        }
    };
}

parsed_create_factories!(
    grant_upgrade_create_data_factory,
    grant_upgrade_create_module_factory,
    GrantUpgradeCreateModuleData,
    GrantUpgradeCreate,
    "GrantUpgradeCreate"
);
parsed_create_factories!(
    lock_weapon_create_data_factory,
    lock_weapon_create_module_factory,
    LockWeaponCreateModuleData,
    LockWeaponCreate,
    "LockWeaponCreate"
);
empty_create_factories!(
    preorder_create_data_factory,
    preorder_create_module_factory,
    PreorderCreate,
    "PreorderCreate"
);
empty_create_factories!(
    special_power_create_data_factory,
    special_power_create_module_factory,
    SpecialPowerCreate,
    "SpecialPowerCreate"
);
empty_create_factories!(
    supply_center_create_data_factory,
    supply_center_create_module_factory,
    SupplyCenterCreate,
    "SupplyCenterCreate"
);
empty_create_factories!(
    supply_warehouse_create_data_factory,
    supply_warehouse_create_module_factory,
    SupplyWarehouseCreate,
    "SupplyWarehouseCreate"
);
parsed_create_factories!(
    veterancy_gain_create_data_factory,
    veterancy_gain_create_module_factory,
    VeterancyGainCreateModuleData,
    VeterancyGainCreate,
    "VeterancyGainCreate"
);

macro_rules! active_behavior_factories {
    ($data_factory:ident, $module_factory:ident, $data_ty:ty, $behavior_ty:ty, $module_name:literal) => {
        pub(super) fn $data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
            let mut data = <$data_ty>::default();
            if let Some(ini) = ini {
                if let Err(err) = data.parse_from_ini(ini) {
                    warn!(
                        "Failed to parse {} module data at line {}: {}",
                        $module_name,
                        ini.get_line_num(),
                        err
                    );
                }
            }
            Box::new(data)
        }

        pub(super) fn $module_factory(
            thing: Arc<dyn ModuleThing>,
            module_data: ModuleDataHandle,
        ) -> Box<dyn Module> {
            active_behavior_module::<$behavior_ty, $data_ty>(
                thing,
                module_data,
                $module_name,
                <$behavior_ty>::new_with_module_data,
            )
        }
    };
}

#[cfg(test)]
#[path = "factory_initialization_tests.rs"]
mod factory_initialization_tests;
