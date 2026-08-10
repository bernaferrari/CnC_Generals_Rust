//! Collide and crate-collide module factories/wrappers.
//! Split from `contain_module_overrides.rs`. Factory names stay identical.

use super::*;
use super::helpers::*;

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
struct CrateCollideDataAdapter<T: Clone + Send + Sync + std::fmt::Debug + 'static> {
    base: BaseModuleData,
    data: T,
}

impl<T: Clone + Send + Sync + std::fmt::Debug + 'static> CrateCollideDataAdapter<T> {
    fn new(data: T) -> Self {
        Self {
            base: BaseModuleData::new(),
            data,
        }
    }
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

struct LegacyCrateCollideModule<T, TData>
where
    T: CollideModuleTrait + Snapshotable + Send + Sync + 'static,
    TData: Clone + Send + Sync + std::fmt::Debug + 'static,
{
    module_name_key: NameKeyType,
    data: Arc<CrateCollideDataAdapter<TData>>,
    collide: Arc<Mutex<T>>,
    object_id: ObjectID,
}

impl<T, TData> LegacyCrateCollideModule<T, TData>
where
    T: CollideModuleTrait + Snapshotable + Send + Sync + 'static,
    TData: Clone + Send + Sync + std::fmt::Debug + 'static,
{
    fn new(
        module_name: &str,
        data: Arc<CrateCollideDataAdapter<TData>>,
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

impl<T, TData> Module for LegacyCrateCollideModule<T, TData>
where
    T: CollideModuleTrait + Snapshotable + Send + Sync + 'static,
    TData: Clone + Send + Sync + std::fmt::Debug + 'static,
{
    fn get_module_name_key(&self) -> NameKeyType {
        self.module_name_key
    }

    fn get_module_tag_name_key(&self) -> NameKeyType {
        self.data.get_module_tag_name_key()
    }

    fn get_module_data(&self) -> &dyn ModuleData {
        self.data.as_ref()
    }

    fn on_object_created(&mut self) {
        if let Err(err) = COLLISION_MANAGER.register_collide_module(
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
        let _ = COLLISION_MANAGER.unregister_object(self.object_id);
    }
}

impl<T, TData> Snapshotable for LegacyCrateCollideModule<T, TData>
where
    T: CollideModuleTrait + Snapshotable + Send + Sync + 'static,
    TData: Clone + Send + Sync + std::fmt::Debug + 'static,
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

pub(super) fn convert_to_car_bomb_crate_collide_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
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
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let data_arc = cloned_module_data::<
        CrateCollideDataAdapter<ConvertToCarBombCrateCollideModuleData>,
    >("ConvertToCarBombCrateCollide", &module_data);
    let object_id = resolve_owner_id(&thing);
    let Some(object) = TheGameLogic::find_object_by_id(object_id) else {
        // Wave 449: missing owner → no-op module.
        return missing_owner_module_auto("ConvertToCarBombCrateCollide", &module_data);
    };
    let collide = ConvertToCarBombCrateCollide::new(&object, data_arc.data.clone());
    Box::new(LegacyCrateCollideModule::new(
        "ConvertToCarBombCrateCollide",
        data_arc,
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
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let data_arc = cloned_module_data::<
        CrateCollideDataAdapter<ConvertToHijackedVehicleCrateCollideModuleData>,
    >("ConvertToHijackedVehicleCrateCollide", &module_data);
    let object_id = resolve_owner_id(&thing);
    let Some(object) = TheGameLogic::find_object_by_id(object_id) else {
        // Wave 449: missing owner → no-op module.
        let data_for_missing: Arc<dyn ModuleData> = data_arc.clone();
        return missing_owner_module("ConvertToHijackedVehicleCrateCollide", data_for_missing);
    };
    let collide = ConvertToHijackedVehicleCrateCollide::new(&object, data_arc.data.clone());
    Box::new(LegacyCrateCollideModule::new(
        "ConvertToHijackedVehicleCrateCollide",
        data_arc,
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
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let data_arc = cloned_module_data::<CrateCollideDataAdapter<HealCrateCollideModuleData>>(
        "HealCrateCollide",
        &module_data,
    );
    let object_id = resolve_owner_id(&thing);
    let collide = HealCrateCollide::new(object_id, data_arc.data.clone());
    Box::new(LegacyCrateCollideModule::new(
        "HealCrateCollide",
        data_arc,
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
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let data_arc = cloned_module_data::<
        CrateCollideDataAdapter<SabotageCommandCenterCrateCollideModuleData>,
    >("SabotageCommandCenterCrateCollide", &module_data);
    let object_id = resolve_owner_id(&thing);
    let Some(object) = TheGameLogic::find_object_by_id(object_id) else {
        // Wave 449: missing owner → no-op module.
        let data_for_missing: Arc<dyn ModuleData> = data_arc.clone();
        return missing_owner_module("SabotageCommandCenterCrateCollide", data_for_missing);
    };
    let collide = SabotageCommandCenterCrateCollide::new(&object, data_arc.data.clone());
    Box::new(LegacyCrateCollideModule::new(
        "SabotageCommandCenterCrateCollide",
        data_arc,
        collide,
        object_id,
    ))
}
pub(super) fn sabotage_fake_building_crate_collide_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
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
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let data_arc = cloned_module_data::<
        CrateCollideDataAdapter<SabotageFakeBuildingCrateCollideModuleData>,
    >("SabotageFakeBuildingCrateCollide", &module_data);
    let object_id = resolve_owner_id(&thing);
    let Some(object) = TheGameLogic::find_object_by_id(object_id) else {
        // Wave 449: missing owner → no-op module.
        let data_for_missing: Arc<dyn ModuleData> = data_arc.clone();
        return missing_owner_module("SabotageFakeBuildingCrateCollide", data_for_missing);
    };
    let collide = SabotageFakeBuildingCrateCollide::new(&object, data_arc.data.clone());
    Box::new(LegacyCrateCollideModule::new(
        "SabotageFakeBuildingCrateCollide",
        data_arc,
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
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let data_arc = cloned_module_data::<
        CrateCollideDataAdapter<SabotageInternetCenterCrateCollideModuleData>,
    >("SabotageInternetCenterCrateCollide", &module_data);
    let object_id = resolve_owner_id(&thing);
    let Some(object) = TheGameLogic::find_object_by_id(object_id) else {
        // Wave 449: missing owner → no-op module.
        return missing_owner_module_auto("SabotageInternetCenterCrateCollide", &module_data);
    };
    let collide = SabotageInternetCenterCrateCollide::new(&object, data_arc.data.clone());
    Box::new(LegacyCrateCollideModule::new(
        "SabotageInternetCenterCrateCollide",
        data_arc,
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
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let data_arc = cloned_module_data::<
        CrateCollideDataAdapter<SabotageMilitaryFactoryCrateCollideModuleData>,
    >("SabotageMilitaryFactoryCrateCollide", &module_data);
    let object_id = resolve_owner_id(&thing);
    let Some(object) = TheGameLogic::find_object_by_id(object_id) else {
        // Wave 449: missing owner → no-op module.
        return missing_owner_module_auto("SabotageMilitaryFactoryCrateCollide", &module_data);
    };
    let collide = SabotageMilitaryFactoryCrateCollide::new(&object, data_arc.data.clone());
    Box::new(LegacyCrateCollideModule::new(
        "SabotageMilitaryFactoryCrateCollide",
        data_arc,
        collide,
        object_id,
    ))
}
pub(super) fn sabotage_power_plant_crate_collide_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
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
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let data_arc = cloned_module_data::<
        CrateCollideDataAdapter<SabotagePowerPlantCrateCollideModuleData>,
    >("SabotagePowerPlantCrateCollide", &module_data);
    let object_id = resolve_owner_id(&thing);
    let Some(object) = TheGameLogic::find_object_by_id(object_id) else {
        // Wave 449: missing owner → no-op module.
        return missing_owner_module_auto("SabotagePowerPlantCrateCollide", &module_data);
    };
    let collide = SabotagePowerPlantCrateCollide::new(&object, data_arc.data.clone());
    Box::new(LegacyCrateCollideModule::new(
        "SabotagePowerPlantCrateCollide",
        data_arc,
        collide,
        object_id,
    ))
}
pub(super) fn sabotage_superweapon_crate_collide_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
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
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let data_arc = cloned_module_data::<
        CrateCollideDataAdapter<SabotageSuperweaponCrateCollideModuleData>,
    >("SabotageSuperweaponCrateCollide", &module_data);
    let object_id = resolve_owner_id(&thing);
    let Some(object) = TheGameLogic::find_object_by_id(object_id) else {
        // Wave 449: missing owner → no-op module.
        let data_for_missing: Arc<dyn ModuleData> = data_arc.clone();
        return missing_owner_module("SabotageSuperweaponCrateCollide", data_for_missing);
    };
    let collide = SabotageSuperweaponCrateCollide::new(&object, data_arc.data.clone());
    Box::new(LegacyCrateCollideModule::new(
        "SabotageSuperweaponCrateCollide",
        data_arc,
        collide,
        object_id,
    ))
}
pub(super) fn sabotage_supply_center_crate_collide_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
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
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let data_arc = cloned_module_data::<
        CrateCollideDataAdapter<SabotageSupplyCenterCrateCollideModuleData>,
    >("SabotageSupplyCenterCrateCollide", &module_data);
    let object_id = resolve_owner_id(&thing);
    let Some(object) = TheGameLogic::find_object_by_id(object_id) else {
        // Wave 449: missing owner → no-op module.
        return missing_owner_module_auto("SabotageSupplyCenterCrateCollide", &module_data);
    };
    let collide = SabotageSupplyCenterCrateCollide::new(&object, data_arc.data.clone());
    Box::new(LegacyCrateCollideModule::new(
        "SabotageSupplyCenterCrateCollide",
        data_arc,
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
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let data_arc = cloned_module_data::<
        CrateCollideDataAdapter<SabotageSupplyDropzoneCrateCollideModuleData>,
    >("SabotageSupplyDropzoneCrateCollide", &module_data);
    let object_id = resolve_owner_id(&thing);
    let Some(object) = TheGameLogic::find_object_by_id(object_id) else {
        // Wave 449: missing owner → no-op module.
        return missing_owner_module_auto("SabotageSupplyDropzoneCrateCollide", &module_data);
    };
    let collide = SabotageSupplyDropzoneCrateCollide::new(&object, data_arc.data.clone());
    Box::new(LegacyCrateCollideModule::new(
        "SabotageSupplyDropzoneCrateCollide",
        data_arc,
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
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let data_arc = cloned_module_data::<CrateCollideDataAdapter<MoneyCrateCollideModuleData>>(
        "MoneyCrateCollide",
        &module_data,
    );
    let object_id = resolve_owner_id(&thing);
    let collide = MoneyCrateCollide::new(object_id, data_arc.data.clone());
    Box::new(LegacyCrateCollideModule::new(
        "MoneyCrateCollide",
        data_arc,
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
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let data_arc = cloned_module_data::<CrateCollideDataAdapter<SalvageCrateCollideModuleData>>(
        "SalvageCrateCollide",
        &module_data,
    );
    let object_id = resolve_owner_id(&thing);
    let collide = SalvageCrateCollide::new(object_id, data_arc.data.clone());
    Box::new(LegacyCrateCollideModule::new(
        "SalvageCrateCollide",
        data_arc,
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
    Box::new(CrateCollideDataAdapter::new(data))
}

pub(super) fn shroud_crate_collide_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let data_arc = cloned_module_data::<CrateCollideDataAdapter<ShroudCrateCollideModuleData>>(
        "ShroudCrateCollide",
        &module_data,
    );
    let object_id = resolve_owner_id(&thing);
    let collide = ShroudCrateCollide::new(object_id, data_arc.data.crate_data());
    Box::new(LegacyCrateCollideModule::new(
        "ShroudCrateCollide",
        data_arc,
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
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let data_arc = cloned_module_data::<CrateCollideDataAdapter<UnitCrateCollideModuleData>>(
        "UnitCrateCollide",
        &module_data,
    );
    let object_id = resolve_owner_id(&thing);
    let collide = UnitCrateCollide::new(object_id, data_arc.data.clone());
    Box::new(LegacyCrateCollideModule::new(
        "UnitCrateCollide",
        data_arc,
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
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let data_arc = cloned_module_data::<CrateCollideDataAdapter<VeterancyCrateCollideModuleData>>(
        "VeterancyCrateCollide",
        &module_data,
    );
    let object_id = resolve_owner_id(&thing);
    let collide = VeterancyCrateCollide::new(object_id, data_arc.data.clone());
    Box::new(LegacyCrateCollideModule::new(
        "VeterancyCrateCollide",
        data_arc,
        collide,
        object_id,
    ))
}

struct FireWeaponCollideModule {
    module_name_key: NameKeyType,
    data: Arc<FireWeaponCollideModuleData>,
    collide: Arc<Mutex<FireWeaponCollide>>,
    object_id: ObjectID,
}

impl FireWeaponCollideModule {
    fn new(
        module_name_key: NameKeyType,
        data: Arc<FireWeaponCollideModuleData>,
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
        COLLISION_MANAGER.register_collide_module(
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
        self.data.get_module_tag_name_key()
    }

    fn get_module_data(&self) -> &dyn ModuleData {
        self.data.as_ref()
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
        let _ = COLLISION_MANAGER.unregister_object(self.object_id);
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
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let data_arc =
        cloned_module_data::<FireWeaponCollideModuleData>("FireWeaponCollide", &module_data);
    let object_id = resolve_owner_id(&thing);
    let collide = FireWeaponCollide::new(object_id, Arc::clone(&data_arc))
        .expect("FireWeaponCollide requires a valid collide weapon template");
    let module_name_key = NameKeyGenerator::name_to_key("FireWeaponCollide");
    Box::new(FireWeaponCollideModule::new(
        module_name_key,
        data_arc,
        collide,
        object_id,
    ))
}

struct SquishCollideModule {
    module_name_key: NameKeyType,
    data: Arc<SquishCollideModuleData>,
    collide: Arc<Mutex<SquishCollide>>,
    object_id: ObjectID,
}

impl SquishCollideModule {
    fn new(
        module_name_key: NameKeyType,
        data: Arc<SquishCollideModuleData>,
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
        self.data.get_module_tag_name_key()
    }

    fn get_module_data(&self) -> &dyn ModuleData {
        self.data.as_ref()
    }

    fn on_object_created(&mut self) {
        if let Err(err) = COLLISION_MANAGER.register_collide_module(
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
        let _ = COLLISION_MANAGER.unregister_object(self.object_id);
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
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let data_arc = cloned_module_data::<SquishCollideModuleData>("SquishCollide", &module_data);
    let object_id = resolve_owner_id(&thing);
    let collide = SquishCollide::new(object_id, Arc::clone(&data_arc));
    let module_name_key = NameKeyGenerator::name_to_key("SquishCollide");
    Box::new(SquishCollideModule::new(
        module_name_key,
        data_arc,
        collide,
        object_id,
    ))
}
