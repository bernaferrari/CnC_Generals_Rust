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

/// Wave 449: host-only / missing-owner factory path — fail closed instead of panic.
struct MissingOwnerModule {
    module_name_key: NameKeyType,
    data: Arc<dyn ModuleData>,
}

impl MissingOwnerModule {
    fn new(module_name: &str, data: Arc<dyn ModuleData>) -> Self {
        Self {
            module_name_key: NameKeyGenerator::name_to_key(module_name),
            data,
        }
    }
}

impl Module for MissingOwnerModule {
    fn get_module_name_key(&self) -> NameKeyType {
        self.module_name_key
    }

    fn get_module_tag_name_key(&self) -> NameKeyType {
        self.data.get_module_tag_name_key()
    }

    fn get_module_data(&self) -> &dyn ModuleData {
        self.data.as_ref()
    }
}

impl Snapshotable for MissingOwnerModule {
    fn crc(&self, _xfer: &mut dyn Xfer) -> Result<(), String> {
        Ok(())
    }

    fn xfer(&mut self, _xfer: &mut dyn Xfer) -> Result<(), String> {
        Ok(())
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        Ok(())
    }
}

pub(super) fn missing_owner_module(
    module_name: &str,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    warn!("Wave 449: {module_name} factory missing owner object; installing no-op module");
    Box::new(MissingOwnerModule::new(module_name, module_data))
}

pub(super) fn missing_owner_module_auto(
    module_name: &str,
    module_data: &Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    missing_owner_module(module_name, Arc::clone(module_data))
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

pub(super) fn attach_contain_to_object(
    object_id: ObjectID,
    contain: Arc<Mutex<dyn ContainModuleInterface>>,
) {
    if let Some(object) = TheGameLogic::find_object_by_id(object_id) {
        if let Ok(mut guard) = object.write() {
            guard.set_contain(Some(contain));
        }
    }
}

pub(super) fn attach_body_to_object(
    object_id: ObjectID,
    body: Arc<Mutex<dyn BodyModuleInterface>>,
) {
    if let Some(object) = TheGameLogic::find_object_by_id(object_id) {
        if let Ok(mut guard) = object.write() {
            guard.set_body_module(Some(body));
        }
    }
}

#[derive(Debug)]
pub(crate) struct ActiveBehaviorModule<T: BehaviorModuleInterface + Snapshotable + 'static> {
    module_name_key: NameKeyType,
    data: Arc<dyn ModuleData>,
    behavior: T,
}

impl<T: BehaviorModuleInterface + Snapshotable + 'static> ActiveBehaviorModule<T> {
    pub(crate) fn new(module_name: &str, data: Arc<dyn ModuleData>, behavior: T) -> Self {
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
    fn get_module_name_key(&self) -> NameKeyType {
        self.module_name_key
    }

    fn get_module_tag_name_key(&self) -> NameKeyType {
        self.data.get_module_tag_name_key()
    }

    fn get_deletion_lifetime_interface(&mut self) -> Option<&mut dyn DeletionLifetimeInterface> {
        self.behavior.get_deletion_lifetime_interface()
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
    module_data: Arc<dyn ModuleData>,
    module_name: &str,
    create: fn(
        Arc<RwLock<crate::object::Object>>,
        Arc<dyn LegacyModuleData>,
    ) -> Result<TBehavior, Box<dyn std::error::Error + Send + Sync>>,
) -> Box<dyn Module>
where
    TBehavior: BehaviorModuleInterface + Snapshotable + 'static,
    TData: ModuleData + LegacyModuleData + Clone + 'static,
{
    let data_arc = cloned_module_data::<TData>(module_name, &module_data);
    let engine_data: Arc<dyn ModuleData> = data_arc.clone();
    let legacy_data: Arc<dyn LegacyModuleData> = data_arc;
    let owner_id = resolve_owner_id(&thing);
    // Wave 449: missing dual-world/host owner → no-op module (no panic).
    let Some(object) = TheGameLogic::find_object_by_id(owner_id) else {
        return missing_owner_module(module_name, engine_data);
    };
    let behavior = match create(object, legacy_data) {
        Ok(behavior) => behavior,
        Err(err) => {
            warn!("{module_name} init failed: {err}; installing no-op module");
            return missing_owner_module(module_name, engine_data);
        }
    };
    Box::new(ActiveBehaviorModule::new(
        module_name,
        engine_data,
        behavior,
    ))
}

pub(super) fn cloned_module_data<TData>(
    module_name: &str,
    module_data: &Arc<dyn ModuleData>,
) -> Arc<TData>
where
    TData: ModuleData + Clone + 'static,
{
    Arc::new(
        module_data
            .as_any()
            .downcast_ref::<TData>()
            .unwrap_or_else(|| panic!("{module_name} module data type expected"))
            .clone(),
    )
}

pub(super) fn cloned_module_data_or_default<TData>(
    module_name: &str,
    module_data: &Arc<dyn ModuleData>,
) -> Arc<TData>
where
    TData: ModuleData + Clone + Default + 'static,
{
    cloned_module_data_or_else(module_name, module_data, TData::default)
}

pub(super) fn cloned_module_data_or_else<TData, F>(
    module_name: &str,
    module_data: &Arc<dyn ModuleData>,
    fallback: F,
) -> Arc<TData>
where
    TData: ModuleData + Clone + 'static,
    F: FnOnce() -> TData,
{
    Arc::new(
        module_data
            .as_any()
            .downcast_ref::<TData>()
            .cloned()
            .unwrap_or_else(|| {
                warn!("{module_name} module data expected; using defaults");
                fallback()
            }),
    )
}

#[derive(Debug)]
struct ActiveCreateModule<T: CreateInterface + Snapshotable + Send + Sync + 'static> {
    module_name_key: NameKeyType,
    data: Arc<dyn ModuleData>,
    create: T,
}

impl<T: CreateInterface + Snapshotable + Send + Sync + 'static> ActiveCreateModule<T> {
    fn new(module_name: &str, data: Arc<dyn ModuleData>, create: T) -> Self {
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
        self.data.get_module_tag_name_key()
    }

    fn get_module_data(&self) -> &dyn ModuleData {
        self.data.as_ref()
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
            module_data: Arc<dyn ModuleData>,
        ) -> Box<dyn Module> {
            let data_arc = cloned_module_data::<$data_ty>($module_name, &module_data);
            let engine_data: Arc<dyn ModuleData> = data_arc.clone();
            Box::new(ActiveCreateModule::new(
                $module_name,
                engine_data,
                <$module_ty>::new(thing, data_arc),
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
            module_data: Arc<dyn ModuleData>,
        ) -> Box<dyn Module> {
            let data_arc = cloned_module_data::<CreateModuleData>($module_name, &module_data);
            let engine_data: Arc<dyn ModuleData> = data_arc;
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
            module_data: Arc<dyn ModuleData>,
        ) -> Box<dyn Module> {
            active_behavior_module::<$behavior_ty, $data_ty>(
                thing,
                module_data,
                $module_name,
                <$behavior_ty>::new,
            )
        }
    };
}
