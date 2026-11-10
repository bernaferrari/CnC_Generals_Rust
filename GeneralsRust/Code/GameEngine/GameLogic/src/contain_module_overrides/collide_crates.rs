//! Collide and crate-collide module factories/wrappers.
//! Split from `contain_module_overrides.rs`. Factory names stay identical.

use super::helpers::*;
use super::*;

#[derive(Clone)]
struct SharedCollideModule<T> {
    inner: Arc<Mutex<T>>,
}

impl<T> SharedCollideModule<T> {
    fn new(inner: Arc<Mutex<T>>) -> Self {
        Self { inner }
    }

    fn lock_inner(&self) -> Result<std::sync::MutexGuard<'_, T>, CollisionError> {
        self.inner.lock().map_err(|_| {
            CollisionError::InvalidObject("SharedCollideModule lock poisoned".to_string())
        })
    }
}

impl<T> CollideModuleTrait for SharedCollideModule<T>
where
    T: CollideModuleTrait + Send + Sync + 'static,
{
    fn on_collide(
        &mut self,
        other: Option<&dyn GameObject>,
        loc: &CollisionCoord3D,
        normal: &CollisionCoord3D,
    ) -> Result<(), CollisionError> {
        let mut inner = self.lock_inner()?;
        inner.on_collide(other, loc, normal)
    }

    fn would_like_to_collide_with(&self, other: &dyn GameObject) -> bool {
        self.lock_inner()
            .map(|inner| inner.would_like_to_collide_with(other))
            .unwrap_or(false)
    }

    fn is_hijacked_vehicle_crate_collide(&self) -> bool {
        self.lock_inner()
            .map(|inner| inner.is_hijacked_vehicle_crate_collide())
            .unwrap_or(false)
    }

    fn is_sabotage_building_crate_collide(&self) -> bool {
        self.lock_inner()
            .map(|inner| inner.is_sabotage_building_crate_collide())
            .unwrap_or(false)
    }

    fn is_car_bomb_crate_collide(&self) -> bool {
        self.lock_inner()
            .map(|inner| inner.is_car_bomb_crate_collide())
            .unwrap_or(false)
    }

    fn is_railroad(&self) -> bool {
        self.lock_inner()
            .map(|inner| inner.is_railroad())
            .unwrap_or(false)
    }

    fn is_salvage_crate_collide(&self) -> bool {
        self.lock_inner()
            .map(|inner| inner.is_salvage_crate_collide())
            .unwrap_or(false)
    }
}

impl ModuleData for FireWeaponCollideModuleData {
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn set_module_tag_name_key(&mut self, key: NameKeyType) {
        crate::common::LegacyModuleData::set_module_tag_name_key(self, key);
    }

    fn get_module_tag_name_key(&self) -> NameKeyType {
        crate::common::LegacyModuleData::get_module_tag_name_key(self)
    }
}

impl ModuleData for SquishCollideModuleData {
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn set_module_tag_name_key(&mut self, key: NameKeyType) {
        crate::common::LegacyModuleData::set_module_tag_name_key(self, key);
    }

    fn get_module_tag_name_key(&self) -> NameKeyType {
        crate::common::LegacyModuleData::get_module_tag_name_key(self)
    }
}

#[derive(Debug, Clone)]
pub(crate) struct CrateCollideDataAdapter<T: Clone + Send + Sync + std::fmt::Debug + 'static> {
    base: BaseModuleData,
    pub(crate) data: T,
}

impl<T: Clone + Send + Sync + std::fmt::Debug + 'static> CrateCollideDataAdapter<T> {
    pub(crate) fn new(data: T) -> Self {
        Self {
            base: BaseModuleData::new(),
            data,
        }
    }
}

/// Keep strict admission data in the same adapter consumed by the real
/// registered crate module constructors. No module instance is constructed.
pub(super) fn adapted_crate_data<T>(data: T) -> Box<dyn ModuleData>
where
    T: Clone + Send + Sync + std::fmt::Debug + 'static,
{
    Box::new(CrateCollideDataAdapter::new(data))
}

impl<T: Clone + Send + Sync + std::fmt::Debug + 'static> ModuleData for CrateCollideDataAdapter<T> {
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn set_module_tag_name_key(&mut self, key: NameKeyType) {
        self.base.set_module_tag_name_key(key);
    }

    fn get_module_tag_name_key(&self) -> NameKeyType {
        self.base.get_module_tag_name_key()
    }
}

impl<T: Clone + Send + Sync + std::fmt::Debug + 'static> Snapshotable
    for CrateCollideDataAdapter<T>
{
    fn crc(&self, xfer: &mut dyn Xfer) -> Result<(), String> {
        self.base.crc(xfer)
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        self.base.xfer(xfer)
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        self.base.load_post_process()
    }
}

struct LegacyCrateCollideModule<T>
where
    T: CollideModuleTrait + Snapshotable + Send + Sync + 'static,
{
    module_name_key: NameKeyType,
    data: ModuleDataHandle,
    collide: Arc<Mutex<T>>,
    object_id: ObjectID,
}

impl<T> LegacyCrateCollideModule<T>
where
    T: CollideModuleTrait + Snapshotable + Send + Sync + 'static,
{
    fn new(
        module_name: &str,
        data: ModuleDataHandle,
        collide: T,
        object_id: ObjectID,
    ) -> Self {
        Self {
            module_name_key: NameKeyGenerator::name_to_key(module_name),
            data,
            collide: Arc::new(Mutex::new(collide)),
            object_id,
        }
    }
}

impl<T> Module for LegacyCrateCollideModule<T>
where
    T: CollideModuleTrait + Snapshotable + Send + Sync + 'static,
{
    fn get_module_name_key(&self) -> NameKeyType {
        self.module_name_key
    }

    fn get_module_tag_name_key(&self) -> NameKeyType {
        self.data.read().get_module_tag_name_key()
    }

    fn get_module_data(&self) -> ModuleDataRead<'_> {
        ModuleDataRead::shared(&self.data)
    }

    fn on_object_created(&mut self) {
        if let Err(err) = world_collision_manager().register_collide_module(
            self.object_id,
            Box::new(SharedCollideModule::new(Arc::clone(&self.collide))),
        ) {
            warn!(
                "Failed to register crate collide module for object {}: {}",
                self.object_id, err
            );
        }
    }

    fn on_delete(&mut self) {
        let _ = world_collision_manager().unregister_object(self.object_id);
    }
}

impl<T> Snapshotable for LegacyCrateCollideModule<T>
where
    T: CollideModuleTrait + Snapshotable + Send + Sync + 'static,
{
    fn crc(&self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let collide = self
            .collide
            .lock()
            .map_err(|_| "crate collide lock poisoned".to_string())?;
        collide.crc(xfer)
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut collide = self
            .collide
            .lock()
            .map_err(|_| "crate collide lock poisoned".to_string())?;
        collide.xfer(xfer)
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        let mut collide = self
            .collide
            .lock()
            .map_err(|_| "crate collide lock poisoned".to_string())?;
        collide.load_post_process()
    }
}

pub(super) fn convert_to_car_bomb_crate_collide_data_factory(
    ini: Option<&mut INI>,
) -> Box<dyn ModuleData> {
    let mut data = ConvertToCarBombCrateCollideModuleData::default();
    if let Some(ini) = ini {
        if let Err(err) = data.parse_from_ini(ini) {
            warn!(
                "Failed to parse ConvertToCarBombCrateCollide module data at line {}: {}",
                ini.get_line_num(),
                err
            );
        }
    }
    Box::new(CrateCollideDataAdapter::new(data))
}

pub(super) fn convert_to_car_bomb_crate_collide_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: ModuleDataHandle,
) -> Box<dyn Module> {
    assert!(module_data.is::<CrateCollideDataAdapter<ConvertToCarBombCrateCollideModuleData>>(), "invalid module data for ConvertToCarBombCrateCollide");
    let object_id = resolve_owner_id(&thing);
    let Some(object) = TheGameLogic::find_object_by_id(object_id) else {
        // Wave 449: missing owner → no-op module.
        return missing_owner_module_auto("ConvertToCarBombCrateCollide", &module_data);
    };
    let collide = ConvertToCarBombCrateCollide::new(&object, module_data.clone());
    Box::new(LegacyCrateCollideModule::new(
        "ConvertToCarBombCrateCollide",
        module_data,
        collide,
        object_id,
    ))
}
pub(super) fn convert_to_hijacked_vehicle_crate_collide_data_factory(
    ini: Option<&mut INI>,
) -> Box<dyn ModuleData> {
    let mut data = ConvertToHijackedVehicleCrateCollideModuleData::default();
    if let Some(ini) = ini {
        if let Err(err) = data.parse_from_ini(ini) {
            warn!(
                "Failed to parse ConvertToHijackedVehicleCrateCollide module data at line {}: {}",
                ini.get_line_num(),
                err
            );
        }
    }
    Box::new(CrateCollideDataAdapter::new(data))
}

pub(super) fn convert_to_hijacked_vehicle_crate_collide_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: ModuleDataHandle,
) -> Box<dyn Module> {
    assert!(module_data.is::<CrateCollideDataAdapter<ConvertToHijackedVehicleCrateCollideModuleData>>(), "invalid module data for ConvertToHijackedVehicleCrateCollide");
    let object_id = resolve_owner_id(&thing);
    let Some(object) = TheGameLogic::find_object_by_id(object_id) else {
        // Wave 449: missing owner → no-op module.
        return missing_owner_module_auto("ConvertToHijackedVehicleCrateCollide", &module_data);
    };
    let mut collide = ConvertToHijackedVehicleCrateCollide::new(&object, module_data.clone());
    collide.base.bind_owning_world(crate::object::supplied_module_world(thing.as_ref()));
    Box::new(LegacyCrateCollideModule::new(
        "ConvertToHijackedVehicleCrateCollide",
        module_data,
        collide,
        object_id,
    ))
}
pub(super) fn heal_crate_collide_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
    let mut data = HealCrateCollideModuleData::default();
    if let Some(ini) = ini {
        if let Err(err) = data.parse_from_ini(ini) {
            warn!(
                "Failed to parse HealCrateCollide module data at line {}: {}",
                ini.get_line_num(),
                err
            );
        }
    }
    Box::new(CrateCollideDataAdapter::new(data))
}

pub(super) fn heal_crate_collide_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: ModuleDataHandle,
) -> Box<dyn Module> {
    assert!(module_data.is::<CrateCollideDataAdapter<HealCrateCollideModuleData>>(), "invalid module data for HealCrateCollide");
    let object_id = resolve_owner_id(&thing);
    let collide = HealCrateCollide::new(object_id, module_data.clone());
    Box::new(LegacyCrateCollideModule::new(
        "HealCrateCollide",
        module_data,
        collide,
        object_id,
    ))
}
pub(super) fn sabotage_command_center_crate_collide_data_factory(
    ini: Option<&mut INI>,
) -> Box<dyn ModuleData> {
    let mut data = SabotageCommandCenterCrateCollideModuleData::default();
    if let Some(ini) = ini {
        if let Err(err) = data.parse_from_ini(ini) {
            warn!(
                "Failed to parse SabotageCommandCenterCrateCollide module data at line {}: {}",
                ini.get_line_num(),
                err
            );
        }
    }
    Box::new(CrateCollideDataAdapter::new(data))
}

pub(super) fn sabotage_command_center_crate_collide_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: ModuleDataHandle,
) -> Box<dyn Module> {
    assert!(module_data.is::<CrateCollideDataAdapter<SabotageCommandCenterCrateCollideModuleData>>(), "invalid module data for SabotageCommandCenterCrateCollide");
    let object_id = resolve_owner_id(&thing);
    let Some(object) = TheGameLogic::find_object_by_id(object_id) else {
        // Wave 449: missing owner → no-op module.
        return missing_owner_module_auto("SabotageCommandCenterCrateCollide", &module_data);
    };
    let mut collide = SabotageCommandCenterCrateCollide::new(&object, module_data.clone());
    collide.base.bind_owning_world(crate::object::supplied_module_world(thing.as_ref()));
    Box::new(LegacyCrateCollideModule::new(
        "SabotageCommandCenterCrateCollide",
        module_data,
        collide,
        object_id,
    ))
}
pub(super) fn sabotage_fake_building_crate_collide_data_factory(
    ini: Option<&mut INI>,
) -> Box<dyn ModuleData> {
    let mut data = SabotageFakeBuildingCrateCollideModuleData::default();
    if let Some(ini) = ini {
        if let Err(err) = data.parse_from_ini(ini) {
            warn!(
                "Failed to parse SabotageFakeBuildingCrateCollide module data at line {}: {}",
                ini.get_line_num(),
                err
            );
        }
    }
    Box::new(CrateCollideDataAdapter::new(data))
}

pub(super) fn sabotage_fake_building_crate_collide_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: ModuleDataHandle,
) -> Box<dyn Module> {
    assert!(module_data.is::<CrateCollideDataAdapter<SabotageFakeBuildingCrateCollideModuleData>>(), "invalid module data for SabotageFakeBuildingCrateCollide");
    let object_id = resolve_owner_id(&thing);
    let Some(object) = TheGameLogic::find_object_by_id(object_id) else {
        // Wave 449: missing owner → no-op module.
        return missing_owner_module_auto("SabotageFakeBuildingCrateCollide", &module_data);
    };
    let mut collide = SabotageFakeBuildingCrateCollide::new(&object, module_data.clone());
    collide.base.bind_owning_world(crate::object::supplied_module_world(thing.as_ref()));
    Box::new(LegacyCrateCollideModule::new(
        "SabotageFakeBuildingCrateCollide",
        module_data,
        collide,
        object_id,
    ))
}
pub(super) fn sabotage_internet_center_crate_collide_data_factory(
    ini: Option<&mut INI>,
) -> Box<dyn ModuleData> {
    let mut data = SabotageInternetCenterCrateCollideModuleData::default();
    if let Some(ini) = ini {
        if let Err(err) = data.parse_from_ini(ini) {
            warn!(
                "Failed to parse SabotageInternetCenterCrateCollide module data at line {}: {}",
                ini.get_line_num(),
                err
            );
        }
    }
    Box::new(CrateCollideDataAdapter::new(data))
}

pub(super) fn sabotage_internet_center_crate_collide_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: ModuleDataHandle,
) -> Box<dyn Module> {
    assert!(module_data.is::<CrateCollideDataAdapter<SabotageInternetCenterCrateCollideModuleData>>(), "invalid module data for SabotageInternetCenterCrateCollide");
    let object_id = resolve_owner_id(&thing);
    let Some(object) = TheGameLogic::find_object_by_id(object_id) else {
        // Wave 449: missing owner → no-op module.
        return missing_owner_module_auto("SabotageInternetCenterCrateCollide", &module_data);
    };
    let mut collide = SabotageInternetCenterCrateCollide::new(&object, module_data.clone());
    collide.base.bind_owning_world(crate::object::supplied_module_world(thing.as_ref()));
    Box::new(LegacyCrateCollideModule::new(
        "SabotageInternetCenterCrateCollide",
        module_data,
        collide,
        object_id,
    ))
}
pub(super) fn sabotage_military_factory_crate_collide_data_factory(
    ini: Option<&mut INI>,
) -> Box<dyn ModuleData> {
    let mut data = SabotageMilitaryFactoryCrateCollideModuleData::default();
    if let Some(ini) = ini {
        if let Err(err) = data.parse_from_ini(ini) {
            warn!(
                "Failed to parse SabotageMilitaryFactoryCrateCollide module data at line {}: {}",
                ini.get_line_num(),
                err
            );
        }
    }
    Box::new(CrateCollideDataAdapter::new(data))
}

pub(super) fn sabotage_military_factory_crate_collide_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: ModuleDataHandle,
) -> Box<dyn Module> {
    assert!(module_data.is::<CrateCollideDataAdapter<SabotageMilitaryFactoryCrateCollideModuleData>>(), "invalid module data for SabotageMilitaryFactoryCrateCollide");
    let object_id = resolve_owner_id(&thing);
    let Some(object) = TheGameLogic::find_object_by_id(object_id) else {
        // Wave 449: missing owner → no-op module.
        return missing_owner_module_auto("SabotageMilitaryFactoryCrateCollide", &module_data);
    };
    let mut collide = SabotageMilitaryFactoryCrateCollide::new(&object, module_data.clone());
    collide.base.bind_owning_world(crate::object::supplied_module_world(thing.as_ref()));
    Box::new(LegacyCrateCollideModule::new(
        "SabotageMilitaryFactoryCrateCollide",
        module_data,
        collide,
        object_id,
    ))
}
pub(super) fn sabotage_power_plant_crate_collide_data_factory(
    ini: Option<&mut INI>,
) -> Box<dyn ModuleData> {
    let mut data = SabotagePowerPlantCrateCollideModuleData::default();
    if let Some(ini) = ini {
        if let Err(err) = data.parse_from_ini(ini) {
            warn!(
                "Failed to parse SabotagePowerPlantCrateCollide module data at line {}: {}",
                ini.get_line_num(),
                err
            );
        }
    }
    Box::new(CrateCollideDataAdapter::new(data))
}

pub(super) fn sabotage_power_plant_crate_collide_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: ModuleDataHandle,
) -> Box<dyn Module> {
    assert!(module_data.is::<CrateCollideDataAdapter<SabotagePowerPlantCrateCollideModuleData>>(), "invalid module data for SabotagePowerPlantCrateCollide");
    let object_id = resolve_owner_id(&thing);
    let Some(object) = TheGameLogic::find_object_by_id(object_id) else {
        // Wave 449: missing owner → no-op module.
        return missing_owner_module_auto("SabotagePowerPlantCrateCollide", &module_data);
    };
    let mut collide = SabotagePowerPlantCrateCollide::new(&object, module_data.clone());
    collide.base.bind_owning_world(crate::object::supplied_module_world(thing.as_ref()));
    Box::new(LegacyCrateCollideModule::new(
        "SabotagePowerPlantCrateCollide",
        module_data,
        collide,
        object_id,
    ))
}
pub(super) fn sabotage_superweapon_crate_collide_data_factory(
    ini: Option<&mut INI>,
) -> Box<dyn ModuleData> {
    let mut data = SabotageSuperweaponCrateCollideModuleData::default();
    if let Some(ini) = ini {
        if let Err(err) = data.parse_from_ini(ini) {
            warn!(
                "Failed to parse SabotageSuperweaponCrateCollide module data at line {}: {}",
                ini.get_line_num(),
                err
            );
        }
    }
    Box::new(CrateCollideDataAdapter::new(data))
}

pub(super) fn sabotage_superweapon_crate_collide_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: ModuleDataHandle,
) -> Box<dyn Module> {
    assert!(module_data.is::<CrateCollideDataAdapter<SabotageSuperweaponCrateCollideModuleData>>(), "invalid module data for SabotageSuperweaponCrateCollide");
    let object_id = resolve_owner_id(&thing);
    let Some(object) = TheGameLogic::find_object_by_id(object_id) else {
        // Wave 449: missing owner → no-op module.
        return missing_owner_module_auto("SabotageSuperweaponCrateCollide", &module_data);
    };
    let mut collide = SabotageSuperweaponCrateCollide::new(&object, module_data.clone());
    collide.base.bind_owning_world(crate::object::supplied_module_world(thing.as_ref()));
    Box::new(LegacyCrateCollideModule::new(
        "SabotageSuperweaponCrateCollide",
        module_data,
        collide,
        object_id,
    ))
}
pub(super) fn sabotage_supply_center_crate_collide_data_factory(
    ini: Option<&mut INI>,
) -> Box<dyn ModuleData> {
    let mut data = SabotageSupplyCenterCrateCollideModuleData::default();
    if let Some(ini) = ini {
        if let Err(err) = data.parse_from_ini(ini) {
            warn!(
                "Failed to parse SabotageSupplyCenterCrateCollide module data at line {}: {}",
                ini.get_line_num(),
                err
            );
        }
    }
    Box::new(CrateCollideDataAdapter::new(data))
}

pub(super) fn sabotage_supply_center_crate_collide_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: ModuleDataHandle,
) -> Box<dyn Module> {
    assert!(module_data.is::<CrateCollideDataAdapter<SabotageSupplyCenterCrateCollideModuleData>>(), "invalid module data for SabotageSupplyCenterCrateCollide");
    let object_id = resolve_owner_id(&thing);
    let Some(object) = TheGameLogic::find_object_by_id(object_id) else {
        // Wave 449: missing owner → no-op module.
        return missing_owner_module_auto("SabotageSupplyCenterCrateCollide", &module_data);
    };
    let mut collide = SabotageSupplyCenterCrateCollide::new(&object, module_data.clone());
    collide.base.bind_owning_world(crate::object::supplied_module_world(thing.as_ref()));
    Box::new(LegacyCrateCollideModule::new(
        "SabotageSupplyCenterCrateCollide",
        module_data,
        collide,
        object_id,
    ))
}

pub(super) fn sabotage_supply_dropzone_crate_collide_data_factory(
    ini: Option<&mut INI>,
) -> Box<dyn ModuleData> {
    let mut data = SabotageSupplyDropzoneCrateCollideModuleData::default();
    if let Some(ini) = ini {
        if let Err(err) = data.parse_from_ini(ini) {
            warn!(
                "Failed to parse SabotageSupplyDropzoneCrateCollide module data at line {}: {}",
                ini.get_line_num(),
                err
            );
        }
    }
    Box::new(CrateCollideDataAdapter::new(data))
}

pub(super) fn sabotage_supply_dropzone_crate_collide_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: ModuleDataHandle,
) -> Box<dyn Module> {
    assert!(module_data.is::<CrateCollideDataAdapter<SabotageSupplyDropzoneCrateCollideModuleData>>(), "invalid module data for SabotageSupplyDropzoneCrateCollide");
    let object_id = resolve_owner_id(&thing);
    let Some(object) = TheGameLogic::find_object_by_id(object_id) else {
        // Wave 449: missing owner → no-op module.
        return missing_owner_module_auto("SabotageSupplyDropzoneCrateCollide", &module_data);
    };
    let mut collide = SabotageSupplyDropzoneCrateCollide::new(&object, module_data.clone());
    collide.base.bind_owning_world(crate::object::supplied_module_world(thing.as_ref()));
    Box::new(LegacyCrateCollideModule::new(
        "SabotageSupplyDropzoneCrateCollide",
        module_data,
        collide,
        object_id,
    ))
}
pub(super) fn money_crate_collide_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
    let mut data = MoneyCrateCollideModuleData::default();
    if let Some(ini) = ini {
        if let Err(err) = data.parse_from_ini(ini) {
            warn!(
                "Failed to parse MoneyCrateCollide module data at line {}: {}",
                ini.get_line_num(),
                err
            );
        }
    }
    Box::new(CrateCollideDataAdapter::new(data))
}

pub(super) fn money_crate_collide_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: ModuleDataHandle,
) -> Box<dyn Module> {
    assert!(module_data.is::<CrateCollideDataAdapter<MoneyCrateCollideModuleData>>(), "invalid module data for MoneyCrateCollide");
    let object_id = resolve_owner_id(&thing);
    let collide = MoneyCrateCollide::new(object_id, module_data.clone());
    Box::new(LegacyCrateCollideModule::new(
        "MoneyCrateCollide",
        module_data,
        collide,
        object_id,
    ))
}
pub(super) fn salvage_crate_collide_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
    let mut data = SalvageCrateCollideModuleData::default();
    if let Some(ini) = ini {
        if let Err(err) = data.parse_from_ini(ini) {
            warn!(
                "Failed to parse SalvageCrateCollide module data at line {}: {}",
                ini.get_line_num(),
                err
            );
        }
    }
    Box::new(CrateCollideDataAdapter::new(data))
}

pub(super) fn salvage_crate_collide_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: ModuleDataHandle,
) -> Box<dyn Module> {
    assert!(module_data.is::<CrateCollideDataAdapter<SalvageCrateCollideModuleData>>(), "invalid module data for SalvageCrateCollide");
    let object_id = resolve_owner_id(&thing);
    let collide = SalvageCrateCollide::new(object_id, module_data.clone());
    Box::new(LegacyCrateCollideModule::new(
        "SalvageCrateCollide",
        module_data,
        collide,
        object_id,
    ))
}
pub(super) fn shroud_crate_collide_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
    let mut data = ShroudCrateCollideModuleData::default();
    if let Some(ini) = ini {
        if let Err(err) = data.parse_from_ini(ini) {
            warn!(
                "Failed to parse ShroudCrateCollide module data at line {}: {}",
                ini.get_line_num(),
                err
            );
        }
    }
    Box::new(data)
}

pub(super) fn shroud_crate_collide_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: ModuleDataHandle,
) -> Box<dyn Module> {
    assert!(module_data.is::<ShroudCrateCollideModuleData>(), "invalid module data for ShroudCrateCollide");
    let object_id = resolve_owner_id(&thing);
    let collide = ShroudCrateCollide::new(object_id, module_data.clone());
    Box::new(LegacyCrateCollideModule::new(
        "ShroudCrateCollide",
        module_data,
        collide,
        object_id,
    ))
}
pub(super) fn unit_crate_collide_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
    let mut data = UnitCrateCollideModuleData::default();
    if let Some(ini) = ini {
        if let Err(err) = data.parse_from_ini(ini) {
            warn!(
                "Failed to parse UnitCrateCollide module data at line {}: {}",
                ini.get_line_num(),
                err
            );
        }
    }
    Box::new(CrateCollideDataAdapter::new(data))
}

pub(super) fn unit_crate_collide_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: ModuleDataHandle,
) -> Box<dyn Module> {
    assert!(module_data.is::<CrateCollideDataAdapter<UnitCrateCollideModuleData>>(), "invalid module data for UnitCrateCollide");
    let object_id = resolve_owner_id(&thing);
    let collide = UnitCrateCollide::new(object_id, module_data.clone());
    Box::new(LegacyCrateCollideModule::new(
        "UnitCrateCollide",
        module_data,
        collide,
        object_id,
    ))
}
pub(super) fn veterancy_crate_collide_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
    let mut data = VeterancyCrateCollideModuleData::default();
    if let Some(ini) = ini {
        if let Err(err) = data.parse_from_ini(ini) {
            warn!(
                "Failed to parse VeterancyCrateCollide module data at line {}: {}",
                ini.get_line_num(),
                err
            );
        }
    }
    Box::new(CrateCollideDataAdapter::new(data))
}

pub(super) fn veterancy_crate_collide_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: ModuleDataHandle,
) -> Box<dyn Module> {
    assert!(module_data.is::<CrateCollideDataAdapter<VeterancyCrateCollideModuleData>>(), "invalid module data for VeterancyCrateCollide");
    let object_id = resolve_owner_id(&thing);
    let collide = VeterancyCrateCollide::new(object_id, module_data.clone());
    Box::new(LegacyCrateCollideModule::new(
        "VeterancyCrateCollide",
        module_data,
        collide,
        object_id,
    ))
}

struct FireWeaponCollideModule {
    module_name_key: NameKeyType,
    data: ModuleDataHandle,
    collide: Arc<Mutex<FireWeaponCollide>>,
    object_id: ObjectID,
}

impl FireWeaponCollideModule {
    fn new(
        module_name_key: NameKeyType,
        data: ModuleDataHandle,
        collide: FireWeaponCollide,
        object_id: ObjectID,
    ) -> Self {
        Self {
            module_name_key,
            data,
            collide: Arc::new(Mutex::new(collide)),
            object_id,
        }
    }

    fn register_collide_module(&self) -> Result<(), CollisionError> {
        world_collision_manager().register_collide_module(
            self.object_id,
            Box::new(SharedCollideModule::new(Arc::clone(&self.collide))),
        )
    }
}

impl Module for FireWeaponCollideModule {
    fn get_module_name_key(&self) -> NameKeyType {
        self.module_name_key
    }

    fn get_module_tag_name_key(&self) -> NameKeyType {
        self.data.read().get_module_tag_name_key()
    }

    fn get_module_data(&self) -> ModuleDataRead<'_> {
        ModuleDataRead::shared(&self.data)
    }

    fn on_object_created(&mut self) {
        if let Err(err) = self.register_collide_module() {
            warn!(
                "Failed to register FireWeaponCollide module for object {}: {}",
                self.object_id, err
            );
        }
    }

    fn on_delete(&mut self) {
        let _ = world_collision_manager().unregister_object(self.object_id);
    }
}

impl Snapshotable for FireWeaponCollideModule {
    fn crc(&self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let collide = self
            .collide
            .lock()
            .map_err(|_| "FireWeaponCollide lock poisoned".to_string())?;
        collide.crc(xfer)
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut collide = self
            .collide
            .lock()
            .map_err(|_| "FireWeaponCollide lock poisoned".to_string())?;
        collide.xfer(xfer)
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        let mut collide = self
            .collide
            .lock()
            .map_err(|_| "FireWeaponCollide lock poisoned".to_string())?;
        collide.load_post_process()
    }
}

pub(super) fn fire_weapon_collide_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
    let mut data = FireWeaponCollideModuleData::default();
    if let Some(ini) = ini {
        if let Err(err) = data.parse_from_ini(ini) {
            warn!(
                "Failed to parse FireWeaponCollide module data at line {}: {}",
                ini.get_line_num(),
                err
            );
        }
    }
    Box::new(data)
}

pub(super) fn fire_weapon_collide_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: ModuleDataHandle,
) -> Box<dyn Module> {
    assert!(module_data.is::<FireWeaponCollideModuleData>(), "invalid module data for FireWeaponCollide");
    let object_id = resolve_owner_id(&thing);
    let collide = FireWeaponCollide::new(object_id, module_data.clone())
        .expect("FireWeaponCollide requires a valid collide weapon template");
    let module_name_key = NameKeyGenerator::name_to_key("FireWeaponCollide");
    Box::new(FireWeaponCollideModule::new(
        module_name_key,
        module_data,
        collide,
        object_id,
    ))
}

struct SquishCollideModule {
    module_name_key: NameKeyType,
    data: ModuleDataHandle,
    collide: Arc<Mutex<SquishCollide>>,
    object_id: ObjectID,
}

impl SquishCollideModule {
    fn new(
        module_name_key: NameKeyType,
        data: ModuleDataHandle,
        collide: SquishCollide,
        object_id: ObjectID,
    ) -> Self {
        Self {
            module_name_key,
            data,
            collide: Arc::new(Mutex::new(collide)),
            object_id,
        }
    }
}

impl Module for SquishCollideModule {
    fn get_module_name_key(&self) -> NameKeyType {
        self.module_name_key
    }

    fn get_module_tag_name_key(&self) -> NameKeyType {
        self.data.read().get_module_tag_name_key()
    }

    fn get_module_data(&self) -> ModuleDataRead<'_> {
        ModuleDataRead::shared(&self.data)
    }

    fn on_object_created(&mut self) {
        if let Err(err) = world_collision_manager().register_collide_module(
            self.object_id,
            Box::new(SharedCollideModule::new(Arc::clone(&self.collide))),
        ) {
            warn!(
                "Failed to register SquishCollide module for object {}: {}",
                self.object_id, err
            );
        }
    }

    fn on_delete(&mut self) {
        let _ = world_collision_manager().unregister_object(self.object_id);
    }
}

impl Snapshotable for SquishCollideModule {
    fn crc(&self, xfer: &mut dyn Xfer) -> Result<(), String> {
        if let Ok(collide) = self.collide.lock() {
            collide.crc(xfer)
        } else {
            Ok(())
        }
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        if let Ok(mut collide) = self.collide.lock() {
            collide.xfer(xfer)
        } else {
            Ok(())
        }
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        if let Ok(mut collide) = self.collide.lock() {
            collide.load_post_process()
        } else {
            Ok(())
        }
    }
}

pub(super) fn squish_collide_data_factory(_ini: Option<&mut INI>) -> Box<dyn ModuleData> {
    Box::new(SquishCollideModuleData::default())
}

pub(super) fn squish_collide_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: ModuleDataHandle,
) -> Box<dyn Module> {
    assert!(module_data.is::<SquishCollideModuleData>(), "invalid module data for SquishCollide");
    let object_id = resolve_owner_id(&thing);
    let collide = SquishCollide::new(object_id, module_data.clone());
    let module_name_key = NameKeyGenerator::name_to_key("SquishCollide");
    Box::new(SquishCollideModule::new(
        module_name_key,
        module_data,
        collide,
        object_id,
    ))
}
