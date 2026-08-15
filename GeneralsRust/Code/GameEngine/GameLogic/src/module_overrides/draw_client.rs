//! Stale ModuleFactory override family extracted from `module_overrides.rs`.
//!
//! W3D draw + client-update factory wrappers.
//!
//! Not part of the active crate build. Live implementation:
//! `contain_module_overrides/`. This dump is kept for archival split / LOC cap.
//! C++ counterpart: ModuleFactory.cpp plus per-module factory wrappers.

use super::*;

fn w3d_model_draw_module_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
    // Preserve existing parser proc when available (see install_module_overrides), so this
    // fallback only applies if no prior module-data implementation exists.
    let mut data = W3DModelDrawModuleData::new();
    if let Some(ini) = ini {
        if let Err(err) = data.parse_from_ini(ini) {
            warn!(
                "Failed to parse W3DModelDraw module data at line {}: {}",
                ini.get_line_num(),
                err
            );
        }
    }
    Box::new(data)
}

fn w3d_model_draw_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let data = module_data
        .as_ref()
        .downcast_ref::<W3DModelDrawModuleData>()
        .cloned()
        .or_else(|| {
            module_data
                .get_as_w3d_model_draw_module_data()
                .and_then(|any| any.downcast_ref::<W3DModelDrawModuleData>())
                .cloned()
        })
        .unwrap_or_else(|| {
            warn!("W3DModelDrawModuleData expected; using defaults");
            W3DModelDrawModuleData::new()
        });

    let mut module = W3DModelDraw::new(data);
    let (owner_id, _) = resolve_owner_info(&thing);
    if owner_id != INVALID_ID {
        module.bind_owner_id(owner_id);
    }
    Box::new(module)
}

macro_rules! w3d_owner_bound_draw_factories {
    (
        $data_factory:ident,
        $module_factory:ident,
        $data_ty:ty,
        $module_ty:ty,
        $module_name:literal
    ) => {
        fn $data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
            let mut data = <$data_ty>::new();
            if let Some(ini) = ini {
                if let Err(err) = data.parse_from_ini(ini) {
                    warn!(
                        concat!(
                            "Failed to parse ",
                            $module_name,
                            " module data at line {}: {}"
                        ),
                        ini.get_line_num(),
                        err
                    );
                }
            }
            Box::new(data)
        }

        fn $module_factory(
            thing: Arc<dyn ModuleThing>,
            module_data: Arc<dyn ModuleData>,
        ) -> Box<dyn Module> {
            let data = module_data
                .as_ref()
                .downcast_ref::<$data_ty>()
                .cloned()
                .unwrap_or_else(|| {
                    warn!(concat!($module_name, "ModuleData expected; using defaults"));
                    <$data_ty>::new()
                });

            let mut module = <$module_ty>::new(data);
            let (owner_id, _) = resolve_owner_info(&thing);
            if owner_id != INVALID_ID {
                module.bind_owner_id(owner_id);
            }
            Box::new(module)
        }
    };
}

w3d_owner_bound_draw_factories!(
    w3d_default_draw_module_data_factory,
    w3d_default_draw_module_factory,
    W3DDefaultDrawModuleData,
    W3DDefaultDraw,
    "W3DDefaultDraw"
);

w3d_owner_bound_draw_factories!(
    w3d_dependency_model_draw_module_data_factory,
    w3d_dependency_model_draw_module_factory,
    W3DDependencyModelDrawModuleData,
    W3DDependencyModelDraw,
    "W3DDependencyModelDraw"
);

w3d_owner_bound_draw_factories!(
    w3d_overlord_aircraft_draw_module_data_factory,
    w3d_overlord_aircraft_draw_module_factory,
    W3DOverlordAircraftDrawModuleData,
    W3DOverlordAircraftDraw,
    "W3DOverlordAircraftDraw"
);

w3d_owner_bound_draw_factories!(
    w3d_overlord_truck_draw_module_data_factory,
    w3d_overlord_truck_draw_module_factory,
    W3DOverlordTruckDrawModuleData,
    W3DOverlordTruckDraw,
    "W3DOverlordTruckDraw"
);

w3d_owner_bound_draw_factories!(
    w3d_police_car_draw_module_data_factory,
    w3d_police_car_draw_module_factory,
    W3DPoliceCarDrawModuleData,
    W3DPoliceCarDraw,
    "W3DPoliceCarDraw"
);

w3d_owner_bound_draw_factories!(
    w3d_science_model_draw_module_data_factory,
    w3d_science_model_draw_module_factory,
    W3DScienceModelDrawModuleData,
    W3DScienceModelDraw,
    "W3DScienceModelDraw"
);

w3d_owner_bound_draw_factories!(
    w3d_supply_draw_module_data_factory,
    w3d_supply_draw_module_factory,
    W3DSupplyDrawModuleData,
    W3DSupplyDraw,
    "W3DSupplyDraw"
);

w3d_owner_bound_draw_factories!(
    w3d_truck_draw_module_data_factory,
    w3d_truck_draw_module_factory,
    W3DTruckDrawModuleData,
    W3DTruckDraw,
    "W3DTruckDraw"
);

w3d_owner_bound_draw_factories!(
    w3d_tank_truck_draw_module_data_factory,
    w3d_tank_truck_draw_module_factory,
    W3DTankTruckDrawModuleData,
    W3DTankTruckDraw,
    "W3DTankTruckDraw"
);

fn w3d_tank_draw_module_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
    let mut data = W3DTankDrawModuleData::new();
    if let Some(ini) = ini {
        if let Err(err) = data.parse_from_ini(ini) {
            warn!(
                "Failed to parse W3DTankDraw module data at line {}: {}",
                ini.get_line_num(),
                err
            );
        }
    }
    Box::new(data)
}

fn w3d_tank_draw_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let data = module_data
        .as_ref()
        .downcast_ref::<W3DTankDrawModuleData>()
        .cloned()
        .unwrap_or_else(|| {
            warn!("W3DTankDrawModuleData expected; using defaults");
            W3DTankDrawModuleData::new()
        });

    let mut module = W3DTankDraw::new(data);
    let (owner_id, _) = resolve_owner_info(&thing);
    if owner_id != INVALID_ID {
        module.bind_owner_id(owner_id);
    }
    Box::new(module)
}

fn w3d_overlord_tank_draw_module_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
    let mut data = W3DOverlordTankDrawModuleData::new();
    if let Some(ini) = ini {
        if let Err(err) = data.parse_from_ini(ini) {
            warn!(
                "Failed to parse W3DOverlordTankDraw module data at line {}: {}",
                ini.get_line_num(),
                err
            );
        }
    }
    Box::new(data)
}

fn w3d_overlord_tank_draw_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let data = module_data
        .as_ref()
        .downcast_ref::<W3DOverlordTankDrawModuleData>()
        .cloned()
        .unwrap_or_else(|| {
            warn!("W3DOverlordTankDrawModuleData expected; using defaults");
            W3DOverlordTankDrawModuleData::new()
        });

    let mut module = W3DOverlordTankDraw::new(data);
    let (owner_id, _) = resolve_owner_info(&thing);
    if owner_id != INVALID_ID {
        module.bind_owner_id(owner_id);
    }
    Box::new(module)
}

fn w3d_projectile_draw_module_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
    let mut data = W3DProjectileDrawModuleData::new();
    if let Some(ini) = ini {
        if let Err(err) = data.parse_from_ini(ini) {
            warn!(
                "Failed to parse W3DProjectileDraw module data at line {}: {}",
                ini.get_line_num(),
                err
            );
        }
    }
    Box::new(data)
}

fn w3d_projectile_draw_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let data = module_data
        .as_ref()
        .downcast_ref::<W3DProjectileDrawModuleData>()
        .cloned()
        .unwrap_or_else(|| {
            warn!("W3DProjectileDrawModuleData expected; using defaults");
            W3DProjectileDrawModuleData::new()
        });

    let mut module = W3DProjectileDraw::new(data);
    let (owner_id, _) = resolve_owner_info(&thing);
    if owner_id != INVALID_ID {
        module.bind_owner_id(owner_id);
    }
    Box::new(module)
}

fn w3d_laser_draw_module_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
    let mut data = W3DLaserDrawModuleData::new();
    if let Some(ini) = ini {
        if let Err(err) = data.parse_from_ini(ini) {
            warn!(
                "Failed to parse W3DLaserDraw module data at line {}: {}",
                ini.get_line_num(),
                err
            );
        }
    }
    Box::new(data)
}

fn w3d_laser_draw_module_factory(
    _thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let data = module_data
        .as_ref()
        .downcast_ref::<W3DLaserDrawModuleData>()
        .cloned()
        .unwrap_or_else(|| {
            warn!("W3DLaserDrawModuleData expected; using defaults");
            W3DLaserDrawModuleData::new()
        });
    Box::new(W3DLaserDraw::new(data))
}

fn w3d_rope_draw_module_data_factory(_ini: Option<&mut INI>) -> Box<dyn ModuleData> {
    Box::new(W3DRopeDrawModuleData::new())
}

fn w3d_rope_draw_module_factory(
    _thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let data = module_data
        .as_ref()
        .downcast_ref::<W3DRopeDrawModuleData>()
        .cloned()
        .unwrap_or_else(|| {
            warn!("W3DRopeDrawModuleData expected; using defaults");
            W3DRopeDrawModuleData::new()
        });
    Box::new(W3DRopeDraw::new(data))
}

fn w3d_projectile_stream_draw_module_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
    let mut data = W3DProjectileStreamDrawModuleData::new();
    if let Some(ini) = ini {
        if let Err(err) = data.parse_from_ini(ini) {
            warn!(
                "Failed to parse W3DProjectileStreamDraw module data at line {}: {}",
                ini.get_line_num(),
                err
            );
        }
    }
    Box::new(data)
}

fn w3d_projectile_stream_draw_module_factory(
    _thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let data = module_data
        .as_ref()
        .downcast_ref::<W3DProjectileStreamDrawModuleData>()
        .cloned()
        .unwrap_or_else(|| {
            warn!("W3DProjectileStreamDrawModuleData expected; using defaults");
            W3DProjectileStreamDrawModuleData::new()
        });
    Box::new(W3DProjectileStreamDraw::new(data))
}

fn w3d_tree_draw_module_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
    let mut data = W3DTreeDrawModuleData::new();
    if let Some(ini) = ini {
        if let Err(err) = data.parse_from_ini(ini) {
            warn!(
                "Failed to parse W3DTreeDraw module data at line {}: {}",
                ini.get_line_num(),
                err
            );
        }
    }
    Box::new(data)
}

fn w3d_tree_draw_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let data = module_data
        .as_ref()
        .downcast_ref::<W3DTreeDrawModuleData>()
        .cloned()
        .unwrap_or_else(|| {
            warn!("W3DTreeDrawModuleData expected; using defaults");
            W3DTreeDrawModuleData::new()
        });
    let mut module = W3DTreeDraw::new(data);
    let drawable_id = resolve_drawable_id(&thing);
    if drawable_id != INVALID_ID {
        module.bind_drawable_id(drawable_id);
    }
    Box::new(module)
}

fn w3d_tracer_draw_module_data_factory(_ini: Option<&mut INI>) -> Box<dyn ModuleData> {
    Box::new(W3DTracerDrawModuleData::new())
}

fn w3d_tracer_draw_module_factory(
    _thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let data = module_data
        .as_ref()
        .downcast_ref::<W3DTracerDrawModuleData>()
        .cloned()
        .unwrap_or_else(|| {
            warn!("W3DTracerDrawModuleData expected; using defaults");
            W3DTracerDrawModuleData::new()
        });
    Box::new(W3DTracerDraw::new(data))
}

fn w3d_debris_draw_module_data_factory(_ini: Option<&mut INI>) -> Box<dyn ModuleData> {
    Box::new(W3DDebrisDrawModuleData::new())
}

fn w3d_debris_draw_module_factory(
    _thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let data = module_data
        .as_ref()
        .downcast_ref::<W3DDebrisDrawModuleData>()
        .cloned()
        .unwrap_or_else(|| {
            warn!("W3DDebrisDrawModuleData expected; using defaults");
            W3DDebrisDrawModuleData::new()
        });
    Box::new(W3DDebrisDraw::new(data))
}

fn laser_update_module_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
    let mut data = LaserClientUpdateModuleData::default();
    if let Some(ini) = ini {
        if let Err(err) = data.parse_from_ini(ini) {
            warn!(
                "Failed to parse LaserUpdate module data at line {}: {}",
                ini.get_line_num(),
                err
            );
        }
    }
    Box::new(data)
}

fn laser_update_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let typed_data = module_data
        .as_ref()
        .downcast_ref::<LaserClientUpdateModuleData>()
        .expect("LaserUpdateModuleData expected");
    let module_name_key = NameKeyGenerator::name_to_key("LaserUpdate");
    let (owner_id, _owner_pos) = resolve_owner_info(&thing);
    let module_data_arc = Arc::new(typed_data.clone());
    Box::new(LaserClientUpdateModule::new(
        module_name_key,
        module_data_arc,
        Some(owner_id),
    ))
}

fn ocl_update_module_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
    let mut data = OCLUpdateModuleData::default();
    if let Some(ini) = ini {
        if let Err(err) = data.parse_from_ini(ini) {
            warn!(
                "Failed to parse OCLUpdate module data at line {}: {}",
                ini.get_line_num(),
                err
            );
        }
    }
    Box::new(data)
}

fn ocl_update_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let typed_data = module_data
        .as_ref()
        .downcast_ref::<OCLUpdateModuleData>()
        .expect("OCLUpdateModuleData expected");
    let module_name_key = NameKeyGenerator::name_to_key("OCLUpdate");
    let (owner_id, _owner_pos) = resolve_owner_info(&thing);
    let module_data_arc = Arc::new(typed_data.clone());
    Box::new(OCLUpdateModule::new(
        module_name_key,
        module_data_arc,
        owner_id,
    ))
}

fn special_power_update_module_data_factory(_ini: Option<&mut INI>) -> Box<dyn ModuleData> {
    Box::new(SpecialPowerUpdateModuleData::default())
}

fn special_power_update_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let typed_data = module_data
        .as_ref()
        .downcast_ref::<SpecialPowerUpdateModuleData>()
        .expect("SpecialPowerUpdateModuleData expected");
    let (owner_id, _) = resolve_owner_info(&thing);
    let object = TheGameLogic::find_object_by_id(owner_id)
        .map(Arc::downgrade)
        .unwrap_or_else(std::sync::Weak::new);

    let mut module = SpecialPowerUpdateModule::new(owner_id, object);
    module.set_module_data(typed_data.clone());
    Box::new(module)
}

fn beacon_client_update_module_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
    let mut data = BeaconClientUpdateModuleData::default();
    if let Some(ini) = ini {
        if let Err(err) = data.parse_from_ini(ini) {
            warn!(
                "Failed to parse BeaconClientUpdate module data at line {}: {}",
                ini.get_line_num(),
                err
            );
        }
    }
    Box::new(data)
}

fn beacon_client_update_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let typed_data = module_data
        .as_ref()
        .downcast_ref::<BeaconClientUpdateModuleData>()
        .expect("BeaconClientUpdateModuleData expected");
    let module_name_key = NameKeyGenerator::name_to_key("BeaconClientUpdate");
    let (owner_id, _owner_pos) = resolve_owner_info(&thing);
    let module_data_arc = Arc::new(typed_data.clone());
    Box::new(BeaconClientUpdateModule::new(
        module_name_key,
        module_data_arc,
        owner_id,
    ))
}

fn sway_client_update_module_data_factory(_ini: Option<&mut INI>) -> Box<dyn ModuleData> {
    Box::new(game_engine::common::thing::module::BaseModuleData::new())
}

fn sway_client_update_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let module_name_key = NameKeyGenerator::name_to_key("SwayClientUpdate");
    let (owner_id, _owner_pos) = resolve_owner_info(&thing);
    Box::new(SwayClientUpdateModule::new(
        module_name_key,
        module_data,
        owner_id,
    ))
}

fn animated_particle_sys_bone_client_update_module_data_factory(
    _ini: Option<&mut INI>,
) -> Box<dyn ModuleData> {
    Box::new(game_engine::common::thing::module::BaseModuleData::new())
}

fn animated_particle_sys_bone_client_update_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let module_name_key = NameKeyGenerator::name_to_key("AnimatedParticleSysBoneClientUpdate");
    let (owner_id, _owner_pos) = resolve_owner_info(&thing);
    Box::new(AnimatedParticleSysBoneClientUpdateModule::new(
        module_name_key,
        module_data,
        owner_id,
    ))
}

