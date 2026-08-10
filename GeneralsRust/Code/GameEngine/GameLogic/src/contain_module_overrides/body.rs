//! Body-module factory helpers (Active/Structure/Highlander/etc).
//! Split from `contain_module_overrides.rs`. Factory names stay identical.

use super::*;
use super::helpers::*;

struct BodyBindingModule<T>
where
    T: ModuleData + Clone + Send + Sync + std::fmt::Debug + 'static,
{
    module_name_key: NameKeyType,
    owner_id: ObjectID,
    data: Arc<T>,
    create_body: fn(T, ObjectID) -> Arc<Mutex<dyn BodyModuleInterface>>,
}

impl<T> BodyBindingModule<T>
where
    T: ModuleData + Clone + Send + Sync + std::fmt::Debug + 'static,
{
    fn new(
        module_name: &str,
        owner_id: ObjectID,
        data: Arc<T>,
        create_body: fn(T, ObjectID) -> Arc<Mutex<dyn BodyModuleInterface>>,
    ) -> Self {
        Self {
            module_name_key: NameKeyGenerator::name_to_key(module_name),
            owner_id,
            data,
            create_body,
        }
    }
}

impl<T> Module for BodyBindingModule<T>
where
    T: ModuleData + Clone + Send + Sync + std::fmt::Debug + 'static,
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
        let body = (self.create_body)((*self.data).clone(), self.owner_id);
        attach_body_to_object(self.owner_id, body);
    }
}

impl<T> Snapshotable for BodyBindingModule<T>
where
    T: ModuleData + Clone + Send + Sync + std::fmt::Debug + 'static,
{
    fn crc(&self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut version: u8 = 0;
        xfer.xfer_version(&mut version, 1)
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut version: u8 = 0;
        xfer.xfer_version(&mut version, 1)
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        Ok(())
    }
}

pub(super) fn inactive_body_instance(
    data: BodyModuleData,
    owner_id: ObjectID,
) -> Arc<Mutex<dyn BodyModuleInterface>> {
    Arc::new(Mutex::new(InactiveBody::new_with_owner(data, owner_id)))
}

pub(super) fn active_body_instance(
    data: ActiveBodyModuleData,
    owner_id: ObjectID,
) -> Arc<Mutex<dyn BodyModuleInterface>> {
    Arc::new(Mutex::new(ActiveBody::new_with_owner(data, owner_id)))
}

pub(super) fn structure_body_instance(
    data: StructureBodyModuleData,
    owner_id: ObjectID,
) -> Arc<Mutex<dyn BodyModuleInterface>> {
    Arc::new(Mutex::new(StructureBody::new(data, owner_id)))
}

pub(super) fn highlander_body_instance(
    data: ActiveBodyModuleData,
    owner_id: ObjectID,
) -> Arc<Mutex<dyn BodyModuleInterface>> {
    Arc::new(Mutex::new(HighlanderBody::new(data, owner_id)))
}

pub(super) fn immortal_body_instance(
    data: ActiveBodyModuleData,
    owner_id: ObjectID,
) -> Arc<Mutex<dyn BodyModuleInterface>> {
    Arc::new(Mutex::new(ImmortalBody::new(data, owner_id)))
}

pub(super) fn hive_structure_body_instance(
    data: HiveStructureBodyModuleData,
    owner_id: ObjectID,
) -> Arc<Mutex<dyn BodyModuleInterface>> {
    Arc::new(Mutex::new(HiveStructureBody::new(data, owner_id)))
}

pub(super) fn undead_body_instance(
    data: UndeadBodyModuleData,
    owner_id: ObjectID,
) -> Arc<Mutex<dyn BodyModuleInterface>> {
    Arc::new(Mutex::new(UndeadBody::new(data, owner_id)))
}

pub(super) fn parse_active_body_data(ini: &mut INI, data: &mut ActiveBodyModuleData) -> Result<(), String> {
    data.parse_from_ini(ini)
        .map_err(|err| format!("{} at line {}", err, ini.get_line_num()))
}

pub(super) fn parse_structure_body_data(
    ini: &mut INI,
    data: &mut StructureBodyModuleData,
) -> Result<(), String> {
    data.parse_from_ini(ini)
        .map_err(|err| format!("{} at line {}", err, ini.get_line_num()))
}

pub(super) fn parse_hive_structure_body_data(
    ini: &mut INI,
    data: &mut HiveStructureBodyModuleData,
) -> Result<(), String> {
    data.parse_from_ini(ini)
        .map_err(|err| format!("{} at line {}", err, ini.get_line_num()))
}

pub(super) fn parse_undead_body_data(ini: &mut INI, data: &mut UndeadBodyModuleData) -> Result<(), String> {
    data.parse_from_ini(ini)
        .map_err(|err| format!("{} at line {}", err, ini.get_line_num()))
}

pub(super) fn parse_slow_death_behavior_data(
    ini: &mut INI,
    data: &mut SlowDeathBehaviorModuleData,
) -> Result<(), String> {
    data.parse_from_ini(ini)
        .map_err(|err| format!("{} at line {}", err, ini.get_line_num()))
}

pub(super) fn parse_instant_death_behavior_data(
    ini: &mut INI,
    data: &mut InstantDeathBehaviorModuleData,
) -> Result<(), String> {
    data.parse_from_ini(ini)
        .map_err(|err| format!("{} at line {}", err, ini.get_line_num()))
}

macro_rules! body_factories {
    (
        $data_factory:ident,
        $module_factory:ident,
        $data_ty:ty,
        $module_name:literal,
        $body_ctor:expr,
        $parse_data:expr
    ) => {
        pub(super) fn $data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
            let mut data = <$data_ty>::default();
            if let Some(ini) = ini {
                if let Some(parse_data) = $parse_data {
                    if let Err(err) = parse_data(ini, &mut data) {
                        warn!("Failed to parse {} module data: {}", $module_name, err);
                    }
                }
            }
            Box::new(data)
        }

        pub(super) fn $module_factory(
            thing: Arc<dyn ModuleThing>,
            module_data: Arc<dyn ModuleData>,
        ) -> Box<dyn Module> {
            let typed_data = cloned_module_data_or_default::<$data_ty>($module_name, &module_data);
            Box::new(BodyBindingModule::new(
                $module_name,
                resolve_owner_id(&thing),
                typed_data,
                $body_ctor,
            ))
        }
    };
}

body_factories!(
    inactive_body_module_data_factory,
    inactive_body_module_factory,
    BodyModuleData,
    "InactiveBody",
    inactive_body_instance,
    None::<fn(&mut INI, &mut BodyModuleData) -> Result<(), String>>
);
body_factories!(
    active_body_module_data_factory,
    active_body_module_factory,
    ActiveBodyModuleData,
    "ActiveBody",
    active_body_instance,
    Some(parse_active_body_data)
);
body_factories!(
    structure_body_module_data_factory,
    structure_body_module_factory,
    StructureBodyModuleData,
    "StructureBody",
    structure_body_instance,
    Some(parse_structure_body_data)
);
body_factories!(
    highlander_body_module_data_factory,
    highlander_body_module_factory,
    ActiveBodyModuleData,
    "HighlanderBody",
    highlander_body_instance,
    Some(parse_active_body_data)
);
body_factories!(
    immortal_body_module_data_factory,
    immortal_body_module_factory,
    ActiveBodyModuleData,
    "ImmortalBody",
    immortal_body_instance,
    Some(parse_active_body_data)
);
body_factories!(
    hive_structure_body_module_data_factory,
    hive_structure_body_module_factory,
    HiveStructureBodyModuleData,
    "HiveStructureBody",
    hive_structure_body_instance,
    Some(parse_hive_structure_body_data)
);
body_factories!(
    undead_body_module_data_factory,
    undead_body_module_factory,
    UndeadBodyModuleData,
    "UndeadBody",
    undead_body_instance,
    Some(parse_undead_body_data)
);
