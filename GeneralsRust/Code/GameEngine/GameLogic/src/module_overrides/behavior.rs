//! Stale ModuleFactory override family extracted from `module_overrides.rs`.
//!
//! Typed behaviors plus GenericBehaviorModule leftover factories.
//!
//! Not part of the active crate build. Live implementation:
//! `contain_module_overrides/`. This dump is kept for archival split / LOC cap.
//! C++ counterpart: ModuleFactory.cpp plus per-module factory wrappers.

use super::*;

fn auto_heal_behavior_module_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
    let mut data = AutoHealBehaviorModuleData::default();

    if let Some(ini) = ini {
        if let Err(err) = data.parse_from_ini(ini) {
            warn!(
                "Failed to parse AutoHealBehavior module data at line {}: {}",
                ini.get_line_num(),
                err
            );
        }
    }

    Box::new(data)
}

fn auto_heal_behavior_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let typed_data = module_data
        .as_ref()
        .downcast_ref::<AutoHealBehaviorModuleData>()
        .expect("AutoHealBehaviorModuleData expected");

    let module_data_arc = Arc::new(typed_data.clone());
    let behavior = AutoHealBehavior::from_module_thing(thing, module_data_arc.clone());

    let module_name = AsciiString::from("AutoHealBehavior");
    Box::new(AutoHealBehaviorModule::new(
        behavior,
        &module_name,
        module_data_arc,
    ))
}

fn horde_update_module_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
    let mut data = HordeUpdateModuleData::default();

    if let Some(ini) = ini {
        if let Err(err) = data.parse_from_ini(ini) {
            warn!(
                "Failed to parse HordeUpdate module data at line {}: {}",
                ini.get_line_num(),
                err
            );
        }
    }

    Box::new(data)
}

fn horde_update_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let typed_data = module_data
        .as_ref()
        .downcast_ref::<HordeUpdateModuleData>()
        .expect("HordeUpdateModuleData expected");

    let module_data_arc = Arc::new(typed_data.clone());
    let (owner_id, _) = resolve_owner_info(&thing);
    let object =
        TheGameLogic::find_object_by_id(owner_id).expect("HordeUpdate requires a valid object");
    let behavior = HordeUpdate::new_from_object_handle(object, module_data_arc.clone());

    let module_name = AsciiString::from("HordeUpdate");
    Box::new(HordeUpdateModule::new(
        behavior,
        &module_name,
        module_data_arc,
    ))
}

fn spawn_behavior_module_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
    let mut data = SpawnBehaviorModuleData::new();

    if let Some(ini) = ini {
        if let Err(err) = data.parse_from_ini(ini) {
            warn!(
                "Failed to parse SpawnBehavior module data at line {}: {}",
                ini.get_line_num(),
                err
            );
        }
    }

    Box::new(data)
}

fn spawn_behavior_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let typed_data = module_data
        .as_ref()
        .downcast_ref::<SpawnBehaviorModuleData>()
        .expect("SpawnBehaviorModuleData expected");

    let module_data_arc = Arc::new(typed_data.clone());
    let (owner_id, _) = resolve_owner_info(&thing);
    let object =
        TheGameLogic::find_object_by_id(owner_id).expect("SpawnBehavior requires a valid object");
    let behavior = SpawnBehavior::new(object.read().ok().map(|g| g.get_id()).unwrap_or(crate::common::INVALID_ID), module_data_arc.clone())
        .expect("SpawnBehavior failed to initialize");

    let module_name = AsciiString::from("SpawnBehavior");
    Box::new(SpawnBehaviorModule::new(
        behavior,
        &module_name,
        module_data_arc,
    ))
}

// ============================================================================
// Generic Behavior Module Wrapper
// ============================================================================

#[derive(Debug)]
struct GenericBehaviorModule<T: crate::modules::BehaviorModuleInterface + 'static> {
    module_name_key: NameKeyType,
    data: Arc<dyn ModuleData>,
    behavior: T,
}

impl<T: crate::modules::BehaviorModuleInterface + 'static> GenericBehaviorModule<T> {
    fn new(module_name: &str, data: Arc<dyn ModuleData>, behavior: T) -> Self {
        let module_name_key = NameKeyGenerator::name_to_key(module_name);
        Self {
            module_name_key,
            data,
            behavior,
        }
    }
}

impl<T: crate::modules::BehaviorModuleInterface + 'static> Module for GenericBehaviorModule<T> {
    fn get_module_name_key(&self) -> NameKeyType {
        self.module_name_key
    }
    fn get_module_tag_name_key(&self) -> NameKeyType {
        self.data.get_module_tag_name_key()
    }

    fn get_deletion_lifetime_interface(
        &mut self,
    ) -> Option<&mut dyn game_engine::common::thing::module::DeletionLifetimeInterface> {
        self.behavior.get_deletion_lifetime_interface()
    }

    fn get_bone_fx_control_interface(
        &mut self,
    ) -> Option<&mut dyn game_engine::common::thing::module::BoneFxControlInterface> {
        self.behavior.get_bone_fx_control_interface()
    }

    fn get_prone_control_interface(
        &mut self,
    ) -> Option<&mut dyn game_engine::common::thing::module::ProneControlInterface> {
        self.behavior.get_prone_control_interface()
    }

    fn get_sticky_bomb_control_interface(
        &mut self,
    ) -> Option<&mut dyn game_engine::common::thing::module::StickyBombControlInterface> {
        self.behavior.get_sticky_bomb_control_interface()
    }

    fn get_hijacker_control_interface(
        &mut self,
    ) -> Option<&mut dyn game_engine::common::thing::module::HijackerControlInterface> {
        self.behavior.get_hijacker_control_interface()
    }

    fn get_spy_vision_control_interface(
        &mut self,
    ) -> Option<&mut dyn game_engine::common::thing::module::SpyVisionControlInterface> {
        self.behavior.get_spy_vision_control_interface()
    }
}

impl<T: crate::modules::BehaviorModuleInterface + Snapshotable + 'static> Snapshotable
    for GenericBehaviorModule<T>
{
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

// ============================================================================
// SlowDeathBehavior Factory Functions
// ============================================================================

fn slow_death_behavior_module_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
    let mut data = SlowDeathBehaviorModuleData::new();
    if let Some(ini) = ini {
        if let Err(err) = data.parse_from_ini(ini) {
            warn!(
                "Failed to parse SlowDeathBehavior module data at line {}: {}",
                ini.get_line_num(),
                err
            );
        }
    }
    Box::new(data)
}

fn slow_death_behavior_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let typed_data = module_data
        .as_ref()
        .downcast_ref::<SlowDeathBehaviorModuleData>()
        .expect("SlowDeathBehaviorModuleData expected");
    let module_data_arc: Arc<dyn ModuleData> = Arc::new(typed_data.clone());
    let (owner_id, _) = resolve_owner_info(&thing);
    let object = TheGameLogic::find_object_by_id(owner_id)
        .expect("SlowDeathBehavior requires a valid object");
    let behavior = SlowDeathBehavior::new(object, module_data_arc.clone())
        .expect("SlowDeathBehavior failed to initialize");
    Box::new(GenericBehaviorModule::new(
        "SlowDeathBehavior",
        module_data_arc,
        behavior,
    ))
}

// ============================================================================
// MinefieldBehavior Factory Functions
// ============================================================================

fn minefield_behavior_module_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
    let mut data = MinefieldBehaviorModuleData::default();
    if let Some(ini) = ini {
        if let Err(err) = data.parse_from_ini(ini) {
            warn!(
                "Failed to parse MinefieldBehavior module data at line {}: {}",
                ini.get_line_num(),
                err
            );
        }
    }
    Box::new(data)
}

fn minefield_behavior_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let typed_data = module_data
        .as_ref()
        .downcast_ref::<MinefieldBehaviorModuleData>()
        .expect("MinefieldBehaviorModuleData expected");
    let module_data_arc: Arc<dyn ModuleData> = Arc::new(typed_data.clone());
    let (owner_id, _) = resolve_owner_info(&thing);
    let object = TheGameLogic::find_object_by_id(owner_id)
        .expect("MinefieldBehavior requires a valid object");
    let behavior = MinefieldBehaviorFactory::create_behavior(object, module_data_arc.clone())
        .expect("MinefieldBehavior failed to initialize");
    Box::new(GenericBehaviorModule::new(
        "MinefieldBehavior",
        module_data_arc,
        behavior,
    ))
}

// ============================================================================
// GrantStealthBehavior Factory Functions
// ============================================================================

fn grant_stealth_behavior_module_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
    let mut data = GrantStealthBehaviorModuleData::default();
    if let Some(ini) = ini {
        if let Err(err) = data.parse_from_ini(ini) {
            warn!(
                "Failed to parse GrantStealthBehavior module data at line {}: {}",
                ini.get_line_num(),
                err
            );
        }
    }
    Box::new(data)
}

fn grant_stealth_behavior_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let typed_data = module_data
        .as_ref()
        .downcast_ref::<GrantStealthBehaviorModuleData>()
        .expect("GrantStealthBehaviorModuleData expected");
    let module_data_arc: Arc<dyn ModuleData> = Arc::new(typed_data.clone());
    let (owner_id, _) = resolve_owner_info(&thing);
    let object = TheGameLogic::find_object_by_id(owner_id)
        .expect("GrantStealthBehavior requires a valid object");
    let behavior = GrantStealthBehaviorFactory::create_behavior(object, module_data_arc.clone())
        .expect("GrantStealthBehavior failed to initialize");
    Box::new(GenericBehaviorModule::new(
        "GrantStealthBehavior",
        module_data_arc,
        behavior,
    ))
}

// ============================================================================
// PhysicsUpdate Factory Functions
// ============================================================================

fn physics_update_module_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
    let mut data = PhysicsBehaviorModuleData::default();
    if let Some(ini) = ini {
        if let Err(err) = data.parse_from_ini(ini) {
            warn!(
                "Failed to parse PhysicsUpdate module data at line {}: {}",
                ini.get_line_num(),
                err
            );
        }
    }
    Box::new(data)
}

fn physics_update_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let typed_data = module_data
        .as_ref()
        .downcast_ref::<PhysicsBehaviorModuleData>()
        .expect("PhysicsBehaviorModuleData expected");
    let module_data_arc: Arc<dyn ModuleData> = Arc::new(typed_data.clone());
    let (owner_id, _) = resolve_owner_info(&thing);
    let object =
        TheGameLogic::find_object_by_id(owner_id).expect("PhysicsUpdate requires a valid object");
    let behavior = PhysicsBehaviorFactory::create_behavior(object, module_data_arc.clone())
        .expect("PhysicsUpdate failed to initialize");
    Box::new(GenericBehaviorModule::new(
        "PhysicsUpdate",
        module_data_arc,
        behavior,
    ))
}

// ============================================================================
// Additional Update Module Factory Functions
// ============================================================================

macro_rules! simple_behavior_factory {
    ($name:ident, $data_type:ty, $factory:ty, $module_name:expr) => {
        fn $name($($arg:ident: $arg_ty:ty),*) -> Box<dyn Module> {
            // Implementation
        }
    };
}

fn height_die_update_module_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
    let mut data = HeightDieUpdateModuleData::default();
    if let Some(ini) = ini {
        if let Err(err) = data.parse_from_ini(ini) {
            warn!(
                "Failed to parse HeightDieUpdate module data at line {}: {}",
                ini.get_line_num(),
                err
            );
        }
    }
    Box::new(data)
}

fn height_die_update_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let typed_data = module_data
        .as_ref()
        .downcast_ref::<HeightDieUpdateModuleData>()
        .expect("HeightDieUpdateModuleData expected");
    let module_data_arc: Arc<dyn ModuleData> = Arc::new(typed_data.clone());
    let (owner_id, _) = resolve_owner_info(&thing);
    let object =
        TheGameLogic::find_object_by_id(owner_id).expect("HeightDieUpdate requires a valid object");
    let behavior = HeightDieUpdateFactory::create_behavior(object, module_data_arc.clone())
        .expect("HeightDieUpdate failed to initialize");
    Box::new(GenericBehaviorModule::new(
        "HeightDieUpdate",
        module_data_arc,
        behavior,
    ))
}

fn deletion_update_module_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
    let mut data = DeletionUpdateModuleData::default();
    if let Some(ini) = ini {
        if let Err(err) = data.parse_from_ini(ini) {
            warn!(
                "Failed to parse DeletionUpdate module data at line {}: {}",
                ini.get_line_num(),
                err
            );
        }
    }
    Box::new(data)
}

fn deletion_update_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let typed_data = module_data
        .as_ref()
        .downcast_ref::<DeletionUpdateModuleData>()
        .expect("DeletionUpdateModuleData expected");
    let module_data_arc: Arc<dyn ModuleData> = Arc::new(typed_data.clone());
    let (owner_id, _) = resolve_owner_info(&thing);
    let object =
        TheGameLogic::find_object_by_id(owner_id).expect("DeletionUpdate requires a valid object");
    let behavior = DeletionUpdateFactory::create_behavior(object, module_data_arc.clone())
        .expect("DeletionUpdate failed to initialize");
    Box::new(GenericBehaviorModule::new(
        "DeletionUpdate",
        module_data_arc,
        behavior,
    ))
}

fn wave_guide_update_module_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
    let mut data = WaveGuideUpdateModuleData::default();
    if let Some(ini) = ini {
        if let Err(err) = data.parse_from_ini(ini) {
            warn!(
                "Failed to parse WaveGuideUpdate module data at line {}: {}",
                ini.get_line_num(),
                err
            );
        }
    }
    Box::new(data)
}

fn wave_guide_update_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let typed_data = module_data
        .as_ref()
        .downcast_ref::<WaveGuideUpdateModuleData>()
        .expect("WaveGuideUpdateModuleData expected");
    let module_data_arc: Arc<dyn ModuleData> = Arc::new(typed_data.clone());
    let (owner_id, _) = resolve_owner_info(&thing);
    let object =
        TheGameLogic::find_object_by_id(owner_id).expect("WaveGuideUpdate requires a valid object");
    let behavior = WaveGuideUpdateFactory::create_behavior(object, module_data_arc.clone())
        .expect("WaveGuideUpdate failed to initialize");
    Box::new(GenericBehaviorModule::new(
        "WaveGuideUpdate",
        module_data_arc,
        behavior,
    ))
}

fn checkpoint_update_module_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
    let mut data = CheckpointUpdateModuleData::default();
    if let Some(ini) = ini {
        if let Err(err) = data.parse_from_ini(ini) {
            warn!(
                "Failed to parse CheckpointUpdate module data at line {}: {}",
                ini.get_line_num(),
                err
            );
        }
    }
    Box::new(data)
}

fn checkpoint_update_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let typed_data = module_data
        .as_ref()
        .downcast_ref::<CheckpointUpdateModuleData>()
        .expect("CheckpointUpdateModuleData expected");
    let module_data_arc: Arc<dyn ModuleData> = Arc::new(typed_data.clone());
    let (owner_id, _) = resolve_owner_info(&thing);
    let object = TheGameLogic::find_object_by_id(owner_id)
        .expect("CheckpointUpdate requires a valid object");
    let behavior = CheckpointUpdateFactory::create_behavior(object, module_data_arc.clone())
        .expect("CheckpointUpdate failed to initialize");
    Box::new(GenericBehaviorModule::new(
        "CheckpointUpdate",
        module_data_arc,
        behavior,
    ))
}

fn animation_steering_update_module_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
    let mut data = AnimationSteeringUpdateModuleData::default();
    if let Some(ini) = ini {
        if let Err(err) = data.parse_from_ini(ini) {
            warn!(
                "Failed to parse AnimationSteeringUpdate module data at line {}: {}",
                ini.get_line_num(),
                err
            );
        }
    }
    Box::new(data)
}

fn animation_steering_update_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let typed_data = module_data
        .as_ref()
        .downcast_ref::<AnimationSteeringUpdateModuleData>()
        .expect("AnimationSteeringUpdateModuleData expected");
    let module_data_arc: Arc<dyn ModuleData> = Arc::new(typed_data.clone());
    let (owner_id, _) = resolve_owner_info(&thing);
    let object = TheGameLogic::find_object_by_id(owner_id)
        .expect("AnimationSteeringUpdate requires a valid object");
    let behavior = AnimationSteeringUpdateFactory::create_behavior(object, module_data_arc.clone())
        .expect("AnimationSteeringUpdate failed to initialize");
    Box::new(GenericBehaviorModule::new(
        "AnimationSteeringUpdate",
        module_data_arc,
        behavior,
    ))
}

fn pilot_find_vehicle_update_module_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
    let mut data = PilotFindVehicleUpdateModuleData::default();
    if let Some(ini) = ini {
        if let Err(err) = data.parse_from_ini(ini) {
            warn!(
                "Failed to parse PilotFindVehicleUpdate module data at line {}: {}",
                ini.get_line_num(),
                err
            );
        }
    }
    Box::new(data)
}

fn pilot_find_vehicle_update_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let typed_data = module_data
        .as_ref()
        .downcast_ref::<PilotFindVehicleUpdateModuleData>()
        .expect("PilotFindVehicleUpdateModuleData expected");
    let module_data_arc: Arc<dyn ModuleData> = Arc::new(typed_data.clone());
    let (owner_id, _) = resolve_owner_info(&thing);
    let object = TheGameLogic::find_object_by_id(owner_id)
        .expect("PilotFindVehicleUpdate requires a valid object");
    let behavior = PilotFindVehicleUpdateFactory::create_behavior(object, module_data_arc.clone())
        .expect("PilotFindVehicleUpdate failed to initialize");
    Box::new(GenericBehaviorModule::new(
        "PilotFindVehicleUpdate",
        module_data_arc,
        behavior,
    ))
}

fn hijacker_update_module_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
    let mut data = HijackerUpdateModuleData::default();
    if let Some(ini) = ini {
        if let Err(err) = data.parse_from_ini(ini) {
            warn!(
                "Failed to parse HijackerUpdate module data at line {}: {}",
                ini.get_line_num(),
                err
            );
        }
    }
    Box::new(data)
}

fn hijacker_update_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let typed_data = module_data
        .as_ref()
        .downcast_ref::<HijackerUpdateModuleData>()
        .expect("HijackerUpdateModuleData expected");
    let module_data_arc: Arc<dyn ModuleData> = Arc::new(typed_data.clone());
    let (owner_id, _) = resolve_owner_info(&thing);
    let object =
        TheGameLogic::find_object_by_id(owner_id).expect("HijackerUpdate requires a valid object");
    let behavior = HijackerUpdateFactory::create_behavior(object, module_data_arc.clone())
        .expect("HijackerUpdate failed to initialize");
    Box::new(GenericBehaviorModule::new(
        "HijackerUpdate",
        module_data_arc,
        behavior,
    ))
}

fn helicopter_slow_death_behavior_module_data_factory(
    ini: Option<&mut INI>,
) -> Box<dyn ModuleData> {
    let mut data = HelicopterSlowDeathBehaviorModuleData::default();
    if let Some(ini) = ini {
        if let Err(err) = data.parse_from_ini(ini) {
            warn!(
                "Failed to parse HelicopterSlowDeathBehavior module data at line {}: {}",
                ini.get_line_num(),
                err
            );
        }
    }
    Box::new(data)
}

fn helicopter_slow_death_behavior_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let typed_data = module_data
        .as_ref()
        .downcast_ref::<HelicopterSlowDeathBehaviorModuleData>()
        .expect("HelicopterSlowDeathBehaviorModuleData expected");
    let module_data_arc: Arc<dyn ModuleData> = Arc::new(typed_data.clone());
    let (owner_id, _) = resolve_owner_info(&thing);
    let object = TheGameLogic::find_object_by_id(owner_id)
        .expect("HelicopterSlowDeathBehavior requires a valid object");
    let behavior =
        HelicopterSlowDeathBehaviorFactory::create_behavior(object, module_data_arc.clone())
            .expect("HelicopterSlowDeathBehavior failed to initialize");
    Box::new(GenericBehaviorModule::new(
        "HelicopterSlowDeathBehavior",
        module_data_arc,
        behavior,
    ))
}

fn neutron_missile_slow_death_update_module_data_factory(
    ini: Option<&mut INI>,
) -> Box<dyn ModuleData> {
    let mut data = NeutronMissileSlowDeathUpdateModuleData::default();
    if let Some(ini) = ini {
        if let Err(err) = data.parse_from_ini(ini) {
            warn!(
                "Failed to parse NeutronMissileSlowDeathUpdate module data at line {}: {}",
                ini.get_line_num(),
                err
            );
        }
    }
    Box::new(data)
}

fn neutron_missile_slow_death_update_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let typed_data = module_data
        .as_ref()
        .downcast_ref::<NeutronMissileSlowDeathUpdateModuleData>()
        .expect("NeutronMissileSlowDeathUpdateModuleData expected");
    let module_data_arc: Arc<dyn ModuleData> = Arc::new(typed_data.clone());
    let (owner_id, _) = resolve_owner_info(&thing);
    let object = TheGameLogic::find_object_by_id(owner_id)
        .expect("NeutronMissileSlowDeathUpdate requires a valid object");
    let behavior =
        NeutronMissileSlowDeathUpdateFactory::create_behavior(object, module_data_arc.clone())
            .expect("NeutronMissileSlowDeathUpdate failed to initialize");
    Box::new(GenericBehaviorModule::new(
        "NeutronMissileSlowDeathUpdate",
        module_data_arc,
        behavior,
    ))
}

fn neutron_missile_update_module_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
    let mut data = NeutronMissileUpdateModuleData::default();
    if let Some(ini) = ini {
        if let Err(err) = data.parse_from_ini(ini) {
            warn!(
                "Failed to parse NeutronMissileUpdate module data at line {}: {}",
                ini.get_line_num(),
                err
            );
        }
    }
    Box::new(data)
}

fn neutron_missile_update_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let typed_data = module_data
        .as_ref()
        .downcast_ref::<NeutronMissileUpdateModuleData>()
        .expect("NeutronMissileUpdateModuleData expected");
    let (owner_id, _) = resolve_owner_info(&thing);
    Box::new(NeutronMissileUpdate::new(
        owner_id,
        typed_data.clone(),
        &AsciiString::from("NeutronMissileUpdate"),
    ))
}

fn firestorm_dynamic_geometry_info_update_module_data_factory(
    ini: Option<&mut INI>,
) -> Box<dyn ModuleData> {
    let mut data = FirestormDynamicGeometryInfoUpdateModuleData::default();
    if let Some(ini) = ini {
        if let Err(err) = data.parse_from_ini(ini) {
            warn!(
                "Failed to parse FirestormDynamicGeometryInfoUpdate module data at line {}: {}",
                ini.get_line_num(),
                err
            );
        }
    }
    Box::new(data)
}

fn firestorm_dynamic_geometry_info_update_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let typed_data = module_data
        .as_ref()
        .downcast_ref::<FirestormDynamicGeometryInfoUpdateModuleData>()
        .expect("FirestormDynamicGeometryInfoUpdateModuleData expected");
    let module_data_arc: Arc<dyn ModuleData> = Arc::new(typed_data.clone());
    let (owner_id, _) = resolve_owner_info(&thing);
    let object = TheGameLogic::find_object_by_id(owner_id)
        .expect("FirestormDynamicGeometryInfoUpdate requires a valid object");
    let behavior =
        FirestormDynamicGeometryInfoUpdateFactory::create_behavior(object, module_data_arc.clone())
            .expect("FirestormDynamicGeometryInfoUpdate failed to initialize");
    Box::new(GenericBehaviorModule::new(
        "FirestormDynamicGeometryInfoUpdate",
        module_data_arc,
        behavior,
    ))
}

fn dynamic_geometry_info_update_module_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
    let mut data = DynamicGeometryInfoUpdateModuleData::default();
    if let Some(ini) = ini {
        if let Err(err) = data.parse_from_ini(ini) {
            warn!(
                "Failed to parse DynamicGeometryInfoUpdate module data at line {}: {}",
                ini.get_line_num(),
                err
            );
        }
    }
    Box::new(data)
}

fn dynamic_geometry_info_update_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let typed_data = module_data
        .as_ref()
        .downcast_ref::<DynamicGeometryInfoUpdateModuleData>()
        .expect("DynamicGeometryInfoUpdateModuleData expected");
    let module_data_arc: Arc<dyn ModuleData> = Arc::new(typed_data.clone());
    let (owner_id, _) = resolve_owner_info(&thing);
    let object = TheGameLogic::find_object_by_id(owner_id)
        .expect("DynamicGeometryInfoUpdate requires a valid object");
    let behavior =
        DynamicGeometryInfoUpdateFactory::create_behavior(object, module_data_arc.clone())
            .expect("DynamicGeometryInfoUpdate failed to initialize");
    Box::new(GenericBehaviorModule::new(
        "DynamicGeometryInfoUpdate",
        module_data_arc,
        behavior,
    ))
}

