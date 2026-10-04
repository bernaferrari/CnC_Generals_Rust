//! Body-module factory helpers (Active/Structure/Highlander/etc).
//! Split from `contain_module_overrides.rs`. Factory names stay identical.

use super::helpers::*;
use super::*;

struct BodyBindingModule<T, B>
where
    T: ModuleData + Clone + Send + Sync + std::fmt::Debug + 'static,
    B: BodyModuleInterface + Snapshotable + 'static,
{
    module_name_key: NameKeyType,
    data: Arc<T>,
    body: Arc<Mutex<B>>,
}

impl<T, B> BodyBindingModule<T, B>
where
    T: ModuleData + Clone + Send + Sync + std::fmt::Debug + 'static,
    B: BodyModuleInterface + Snapshotable + 'static,
{
    fn new(
        module_name: &str,
        owner_id: ObjectID,
        data: Arc<T>,
        create_body: fn(T, ObjectID) -> Arc<Mutex<B>>,
    ) -> Self {
        Self {
            module_name_key: NameKeyGenerator::name_to_key(module_name),
            body: create_body((*data).clone(), owner_id),
            data,
        }
    }

    fn snapshot_body(&self) -> Result<std::sync::MutexGuard<'_, B>, String> {
        self.body
            .lock()
            .map_err(|_| "body lock poisoned during snapshot".to_string())
    }
}

impl<T, B> Module for BodyBindingModule<T, B>
where
    T: ModuleData + Clone + Send + Sync + std::fmt::Debug + 'static,
    B: BodyModuleInterface + Snapshotable + 'static,
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
}

/// C++ Object.cpp:396-403 caches the body from the actual new module before
/// onObjectCreated. Borrow the driving owner; never rediscover it by numeric ID.
/// The same concrete body remains behind both Object's cache and module Xfer.
pub(crate) fn install_body_for_module(
    module: &dyn Module,
    owner: &mut crate::object::Object,
) -> Result<bool, String> {
    macro_rules! install_body {
        ($data:ty, $body:ty $(, $active:ident)*) => {
            if let Some(binding) = module
                .as_any()
                .downcast_ref::<BodyBindingModule<$data, $body>>()
            {
                {
                    let mut runtime = binding.snapshot_body()?;
                    let active: &mut ActiveBody = (&mut *runtime)$(.$active())*;
                    active
                        .bind_object_template(owner.get_template().clone())
                        .map_err(|err| err.to_string())?;
                }
                let body: Arc<Mutex<dyn BodyModuleInterface>> = binding.body.clone();
                owner.set_body_module(Some(body));
                owner.apply_structure_rubble_pose();
                return Ok(true);
            }
        };
    }
    if let Some(binding) = module
        .as_any()
        .downcast_ref::<BodyBindingModule<BodyModuleData, InactiveBody>>()
    {
        // InactiveBody.cpp:22-26 modifies its exact construction owner. The
        // standalone constructor is inert; installation performs this effect.
        owner.set_effectively_dead(true);
        let body: Arc<Mutex<dyn BodyModuleInterface>> = binding.body.clone();
        owner.set_body_module(Some(body));
        return Ok(true);
    }
    install_body!(ActiveBodyModuleData, ActiveBody);
    install_body!(StructureBodyModuleData, StructureBody, active_body_mut);
    install_body!(ActiveBodyModuleData, HighlanderBody, active_body_mut);
    install_body!(ActiveBodyModuleData, ImmortalBody, active_body_mut);
    install_body!(
        HiveStructureBodyModuleData,
        HiveStructureBody,
        structure_body_mut,
        active_body_mut
    );
    install_body!(UndeadBodyModuleData, UndeadBody, active_body_mut);
    Ok(false)
}

impl<T, B> Snapshotable for BodyBindingModule<T, B>
where
    T: ModuleData + Clone + Send + Sync + std::fmt::Debug + 'static,
    B: BodyModuleInterface + Snapshotable + 'static,
{
    fn crc(&self, xfer: &mut dyn Xfer) -> Result<(), String> {
        self.snapshot_body()?.crc(xfer)
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        // No adapter version byte: each concrete body owns the C++ version
        // chain and payload, including ActiveBody's health and damage state.
        self.snapshot_body()?.xfer(xfer)
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        self.snapshot_body()?.load_post_process()
    }
}

pub(super) fn inactive_body_instance(
    data: BodyModuleData,
    owner_id: ObjectID,
) -> Arc<Mutex<InactiveBody>> {
    Arc::new(Mutex::new(InactiveBody::new_with_owner(data, owner_id)))
}

pub(super) fn active_body_instance(
    data: ActiveBodyModuleData,
    owner_id: ObjectID,
) -> Arc<Mutex<ActiveBody>> {
    Arc::new(Mutex::new(ActiveBody::new_with_owner(data, owner_id)))
}

pub(super) fn structure_body_instance(
    data: StructureBodyModuleData,
    owner_id: ObjectID,
) -> Arc<Mutex<StructureBody>> {
    Arc::new(Mutex::new(StructureBody::new(data, owner_id)))
}

pub(super) fn highlander_body_instance(
    data: ActiveBodyModuleData,
    owner_id: ObjectID,
) -> Arc<Mutex<HighlanderBody>> {
    Arc::new(Mutex::new(HighlanderBody::new(data, owner_id)))
}

pub(super) fn immortal_body_instance(
    data: ActiveBodyModuleData,
    owner_id: ObjectID,
) -> Arc<Mutex<ImmortalBody>> {
    Arc::new(Mutex::new(ImmortalBody::new(data, owner_id)))
}

pub(super) fn hive_structure_body_instance(
    data: HiveStructureBodyModuleData,
    owner_id: ObjectID,
) -> Arc<Mutex<HiveStructureBody>> {
    Arc::new(Mutex::new(HiveStructureBody::new(data, owner_id)))
}

pub(super) fn undead_body_instance(
    data: UndeadBodyModuleData,
    owner_id: ObjectID,
) -> Arc<Mutex<UndeadBody>> {
    Arc::new(Mutex::new(UndeadBody::new(data, owner_id)))
}

pub(super) fn parse_active_body_data(
    ini: &mut INI,
    data: &mut ActiveBodyModuleData,
) -> Result<(), String> {
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

pub(super) fn parse_undead_body_data(
    ini: &mut INI,
    data: &mut UndeadBodyModuleData,
) -> Result<(), String> {
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

#[cfg(test)]
mod armor_binding_probe;
#[cfg(test)]
pub(crate) use armor_binding_probe::with_active;
