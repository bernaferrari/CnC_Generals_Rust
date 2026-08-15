//! Stale ModuleFactory override family extracted from `module_overrides.rs`.
//!
//! Create-module factory wrappers (LockWeapon/GrantUpgrade/Veterancy/Preorder/Supply*).
//!
//! Not part of the active crate build. Live implementation:
//! `contain_module_overrides/`. This dump is kept for archival split / LOC cap.
//! C++ counterpart: ModuleFactory.cpp plus per-module factory wrappers.

use super::*;

#[derive(Debug)]
struct LockWeaponCreateModule {
    module_name_key: NameKeyType,
    data: Arc<LockWeaponCreateModuleData>,
    create: LockWeaponCreate,
}

impl LockWeaponCreateModule {
    fn new(
        module_name_key: NameKeyType,
        data: Arc<LockWeaponCreateModuleData>,
        create: LockWeaponCreate,
    ) -> Self {
        Self {
            module_name_key,
            data,
            create,
        }
    }
}

impl Module for LockWeaponCreateModule {
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

impl Snapshotable for LockWeaponCreateModule {
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

fn lock_weapon_create_module_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
    let mut data = LockWeaponCreateModuleData::default();
    if let Some(ini) = ini {
        if let Err(err) = data.parse_from_ini(ini) {
            warn!(
                "Failed to parse LockWeaponCreate module data at line {}: {}",
                ini.get_line_num(),
                err
            );
        }
    }
    Box::new(data)
}

fn lock_weapon_create_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let data_arc =
        cloned_module_data::<LockWeaponCreateModuleData>("LockWeaponCreate", &module_data);
    let module_name_key = NameKeyGenerator::name_to_key("LockWeaponCreate");
    let create = LockWeaponCreate::new(thing, Arc::clone(&data_arc));
    Box::new(LockWeaponCreateModule::new(
        module_name_key,
        data_arc,
        create,
    ))
}

#[derive(Debug)]
struct GrantUpgradeCreateModule {
    module_name_key: NameKeyType,
    data: Arc<GrantUpgradeCreateModuleData>,
    create: GrantUpgradeCreate,
}

impl GrantUpgradeCreateModule {
    fn new(
        module_name_key: NameKeyType,
        data: Arc<GrantUpgradeCreateModuleData>,
        create: GrantUpgradeCreate,
    ) -> Self {
        Self {
            module_name_key,
            data,
            create,
        }
    }
}

impl Module for GrantUpgradeCreateModule {
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

impl Snapshotable for GrantUpgradeCreateModule {
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

fn grant_upgrade_create_module_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
    let mut data = GrantUpgradeCreateModuleData::default();
    if let Some(ini) = ini {
        if let Err(err) = data.parse_from_ini(ini) {
            warn!(
                "Failed to parse GrantUpgradeCreate module data at line {}: {}",
                ini.get_line_num(),
                err
            );
        }
    }
    Box::new(data)
}

fn grant_upgrade_create_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let data_arc =
        cloned_module_data::<GrantUpgradeCreateModuleData>("GrantUpgradeCreate", &module_data);
    let module_name_key = NameKeyGenerator::name_to_key("GrantUpgradeCreate");
    let create = GrantUpgradeCreate::new(thing, Arc::clone(&data_arc));
    Box::new(GrantUpgradeCreateModule::new(
        module_name_key,
        data_arc,
        create,
    ))
}

#[derive(Debug)]
struct VeterancyGainCreateModule {
    module_name_key: NameKeyType,
    data: Arc<VeterancyGainCreateModuleData>,
    create: VeterancyGainCreate,
}

impl VeterancyGainCreateModule {
    fn new(
        module_name_key: NameKeyType,
        data: Arc<VeterancyGainCreateModuleData>,
        create: VeterancyGainCreate,
    ) -> Self {
        Self {
            module_name_key,
            data,
            create,
        }
    }
}

impl Module for VeterancyGainCreateModule {
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

impl Snapshotable for VeterancyGainCreateModule {
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

fn veterancy_gain_create_module_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
    let mut data = VeterancyGainCreateModuleData::default();
    if let Some(ini) = ini {
        if let Err(err) = data.parse_from_ini(ini) {
            warn!(
                "Failed to parse VeterancyGainCreate module data at line {}: {}",
                ini.get_line_num(),
                err
            );
        }
    }
    Box::new(data)
}

fn veterancy_gain_create_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let data_arc =
        cloned_module_data::<VeterancyGainCreateModuleData>("VeterancyGainCreate", &module_data);
    let module_name_key = NameKeyGenerator::name_to_key("VeterancyGainCreate");
    let create = VeterancyGainCreate::new(thing, Arc::clone(&data_arc));
    Box::new(VeterancyGainCreateModule::new(
        module_name_key,
        data_arc,
        create,
    ))
}

#[derive(Debug)]
struct SimpleCreateModule<T> {
    module_name_key: NameKeyType,
    data: Arc<CreateModuleData>,
    create: T,
}

impl<T> SimpleCreateModule<T> {
    fn new(module_name_key: NameKeyType, data: Arc<CreateModuleData>, create: T) -> Self {
        Self {
            module_name_key,
            data,
            create,
        }
    }
}

impl<T> Module for SimpleCreateModule<T>
where
    T: CreateInterface + Snapshotable + 'static,
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

    fn get_create_interface(&self) -> Option<&dyn CreateInterface> {
        Some(&self.create)
    }
}

impl<T> Snapshotable for SimpleCreateModule<T>
where
    T: CreateInterface + Snapshotable + 'static,
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

fn simple_create_module_data_factory(_ini: Option<&mut INI>) -> Box<dyn ModuleData> {
    Box::new(CreateModuleData::default())
}

fn preorder_create_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let data_arc = cloned_module_data::<CreateModuleData>("PreorderCreate", &module_data);
    let module_name_key = NameKeyGenerator::name_to_key("PreorderCreate");
    let create = PreorderCreate::new(thing);
    Box::new(SimpleCreateModule::new(module_name_key, data_arc, create))
}

fn special_power_create_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let data_arc = cloned_module_data::<CreateModuleData>("SpecialPowerCreate", &module_data);
    let module_name_key = NameKeyGenerator::name_to_key("SpecialPowerCreate");
    let create = SpecialPowerCreate::new(thing);
    Box::new(SimpleCreateModule::new(module_name_key, data_arc, create))
}

fn supply_center_create_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let data_arc = cloned_module_data::<CreateModuleData>("SupplyCenterCreate", &module_data);
    let module_name_key = NameKeyGenerator::name_to_key("SupplyCenterCreate");
    let create = SupplyCenterCreate::new(thing);
    Box::new(SimpleCreateModule::new(module_name_key, data_arc, create))
}

fn supply_warehouse_create_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let data_arc = cloned_module_data::<CreateModuleData>("SupplyWarehouseCreate", &module_data);
    let module_name_key = NameKeyGenerator::name_to_key("SupplyWarehouseCreate");
    let create = SupplyWarehouseCreate::new(thing);
    Box::new(SimpleCreateModule::new(module_name_key, data_arc, create))
}

