//! Draw and client-update module factories.
//! Split from `contain_module_overrides.rs`. Factory names stay identical.

use super::*;
use super::helpers::*;

macro_rules! draw_data_factory {
    ($factory:ident, $data_ty:ty, $module_name:literal, parse) => {
        pub(super) fn $factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
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
    };
    ($factory:ident, $data_ty:ty, $module_name:literal, no_parse) => {
        pub(super) fn $factory(_ini: Option<&mut INI>) -> Box<dyn ModuleData> {
            Box::new(<$data_ty>::new())
        }
    };
}

macro_rules! owner_bound_draw_factory {
    ($factory:ident, $data_ty:ty, $module_ty:ty, $module_name:literal) => {
        pub(super) fn $factory(
            thing: Arc<dyn ModuleThing>,
            module_data: Arc<dyn ModuleData>,
        ) -> Box<dyn Module> {
            let data = cloned_module_data_or_else::<$data_ty, _>(
                $module_name,
                &module_data,
                <$data_ty>::new,
            );
            let mut module = <$module_ty>::new(data.as_ref().clone());
            let owner_id = resolve_owner_id(&thing);
            if owner_id != INVALID_ID {
                module.bind_owner_id(owner_id);
            }
            Box::new(module)
        }
    };
}

macro_rules! plain_draw_factory {
    ($factory:ident, $data_ty:ty, $module_ty:ty, $module_name:literal) => {
        pub(super) fn $factory(
            _thing: Arc<dyn ModuleThing>,
            module_data: Arc<dyn ModuleData>,
        ) -> Box<dyn Module> {
            let data = cloned_module_data_or_else::<$data_ty, _>(
                $module_name,
                &module_data,
                <$data_ty>::new,
            );
            Box::new(<$module_ty>::new(data.as_ref().clone()))
        }
    };
}

draw_data_factory!(
    w3d_model_draw_module_data_factory,
    W3DModelDrawModuleData,
    "W3DModelDraw",
    parse
);
owner_bound_draw_factory!(
    w3d_model_draw_module_factory,
    W3DModelDrawModuleData,
    W3DModelDraw,
    "W3DModelDraw"
);

draw_data_factory!(
    w3d_default_draw_module_data_factory,
    W3DDefaultDrawModuleData,
    "W3DDefaultDraw",
    parse
);
owner_bound_draw_factory!(
    w3d_default_draw_module_factory,
    W3DDefaultDrawModuleData,
    W3DDefaultDraw,
    "W3DDefaultDraw"
);

draw_data_factory!(
    w3d_dependency_model_draw_module_data_factory,
    W3DDependencyModelDrawModuleData,
    "W3DDependencyModelDraw",
    parse
);
owner_bound_draw_factory!(
    w3d_dependency_model_draw_module_factory,
    W3DDependencyModelDrawModuleData,
    W3DDependencyModelDraw,
    "W3DDependencyModelDraw"
);

draw_data_factory!(
    w3d_tank_draw_module_data_factory,
    W3DTankDrawModuleData,
    "W3DTankDraw",
    parse
);
owner_bound_draw_factory!(
    w3d_tank_draw_module_factory,
    W3DTankDrawModuleData,
    W3DTankDraw,
    "W3DTankDraw"
);

draw_data_factory!(
    w3d_overlord_tank_draw_module_data_factory,
    W3DOverlordTankDrawModuleData,
    "W3DOverlordTankDraw",
    parse
);
owner_bound_draw_factory!(
    w3d_overlord_tank_draw_module_factory,
    W3DOverlordTankDrawModuleData,
    W3DOverlordTankDraw,
    "W3DOverlordTankDraw"
);

draw_data_factory!(
    w3d_overlord_aircraft_draw_module_data_factory,
    W3DOverlordAircraftDrawModuleData,
    "W3DOverlordAircraftDraw",
    parse
);
owner_bound_draw_factory!(
    w3d_overlord_aircraft_draw_module_factory,
    W3DOverlordAircraftDrawModuleData,
    W3DOverlordAircraftDraw,
    "W3DOverlordAircraftDraw"
);

draw_data_factory!(
    w3d_overlord_truck_draw_module_data_factory,
    W3DOverlordTruckDrawModuleData,
    "W3DOverlordTruckDraw",
    parse
);
owner_bound_draw_factory!(
    w3d_overlord_truck_draw_module_factory,
    W3DOverlordTruckDrawModuleData,
    W3DOverlordTruckDraw,
    "W3DOverlordTruckDraw"
);

draw_data_factory!(
    w3d_police_car_draw_module_data_factory,
    W3DPoliceCarDrawModuleData,
    "W3DPoliceCarDraw",
    parse
);
owner_bound_draw_factory!(
    w3d_police_car_draw_module_factory,
    W3DPoliceCarDrawModuleData,
    W3DPoliceCarDraw,
    "W3DPoliceCarDraw"
);

draw_data_factory!(
    w3d_projectile_stream_draw_module_data_factory,
    W3DProjectileStreamDrawModuleData,
    "W3DProjectileStreamDraw",
    parse
);
owner_bound_draw_factory!(
    w3d_projectile_stream_draw_module_factory,
    W3DProjectileStreamDrawModuleData,
    W3DProjectileStreamDraw,
    "W3DProjectileStreamDraw"
);

draw_data_factory!(
    w3d_rope_draw_module_data_factory,
    W3DRopeDrawModuleData,
    "W3DRopeDraw",
    no_parse
);
plain_draw_factory!(
    w3d_rope_draw_module_factory,
    W3DRopeDrawModuleData,
    W3DRopeDraw,
    "W3DRopeDraw"
);

draw_data_factory!(
    w3d_science_model_draw_module_data_factory,
    W3DScienceModelDrawModuleData,
    "W3DScienceModelDraw",
    parse
);
owner_bound_draw_factory!(
    w3d_science_model_draw_module_factory,
    W3DScienceModelDrawModuleData,
    W3DScienceModelDraw,
    "W3DScienceModelDraw"
);

draw_data_factory!(
    w3d_supply_draw_module_data_factory,
    W3DSupplyDrawModuleData,
    "W3DSupplyDraw",
    parse
);
owner_bound_draw_factory!(
    w3d_supply_draw_module_factory,
    W3DSupplyDrawModuleData,
    W3DSupplyDraw,
    "W3DSupplyDraw"
);

draw_data_factory!(
    w3d_tank_truck_draw_module_data_factory,
    W3DTankTruckDrawModuleData,
    "W3DTankTruckDraw",
    parse
);
owner_bound_draw_factory!(
    w3d_tank_truck_draw_module_factory,
    W3DTankTruckDrawModuleData,
    W3DTankTruckDraw,
    "W3DTankTruckDraw"
);

draw_data_factory!(
    w3d_tracer_draw_module_data_factory,
    W3DTracerDrawModuleData,
    "W3DTracerDraw",
    no_parse
);
plain_draw_factory!(
    w3d_tracer_draw_module_factory,
    W3DTracerDrawModuleData,
    W3DTracerDraw,
    "W3DTracerDraw"
);

draw_data_factory!(
    w3d_tree_draw_module_data_factory,
    W3DTreeDrawModuleData,
    "W3DTreeDraw",
    parse
);
pub(super) fn w3d_tree_draw_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let data = cloned_module_data_or_else::<W3DTreeDrawModuleData, _>(
        "W3DTreeDraw",
        &module_data,
        W3DTreeDrawModuleData::new,
    );
    let mut module = W3DTreeDraw::new(data.as_ref().clone());
    let drawable_id = resolve_drawable_id(&thing);
    if drawable_id != INVALID_ID {
        module.bind_drawable_id(drawable_id);
    }
    Box::new(module)
}

draw_data_factory!(
    w3d_truck_draw_module_data_factory,
    W3DTruckDrawModuleData,
    "W3DTruckDraw",
    parse
);
owner_bound_draw_factory!(
    w3d_truck_draw_module_factory,
    W3DTruckDrawModuleData,
    W3DTruckDraw,
    "W3DTruckDraw"
);

draw_data_factory!(
    w3d_laser_draw_module_data_factory,
    W3DLaserDrawModuleData,
    "W3DLaserDraw",
    parse
);
owner_bound_draw_factory!(
    w3d_laser_draw_module_factory,
    W3DLaserDrawModuleData,
    W3DLaserDraw,
    "W3DLaserDraw"
);

draw_data_factory!(
    w3d_debris_draw_module_data_factory,
    W3DDebrisDrawModuleData,
    "W3DDebrisDraw",
    no_parse
);
owner_bound_draw_factory!(
    w3d_debris_draw_module_factory,
    W3DDebrisDrawModuleData,
    W3DDebrisDraw,
    "W3DDebrisDraw"
);

pub(super) fn laser_update_module_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
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

pub(super) fn laser_update_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let module_data =
        cloned_module_data_or_default::<LaserClientUpdateModuleData>("LaserUpdate", &module_data);
    Box::new(LaserClientUpdateModule::new(
        NameKeyGenerator::name_to_key("LaserUpdate"),
        module_data,
        Some(resolve_owner_id(&thing)),
    ))
}

pub(super) fn beacon_client_update_module_data_factory(ini: Option<&mut INI>) -> Box<dyn ModuleData> {
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

pub(super) fn beacon_client_update_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    let module_data = cloned_module_data_or_default::<BeaconClientUpdateModuleData>(
        "BeaconClientUpdate",
        &module_data,
    );
    Box::new(BeaconClientUpdateModule::new(
        NameKeyGenerator::name_to_key("BeaconClientUpdate"),
        module_data,
        resolve_owner_id(&thing),
    ))
}

pub(super) fn base_client_update_module_data_factory(_ini: Option<&mut INI>) -> Box<dyn ModuleData> {
    Box::new(BaseModuleData::new())
}

pub(super) fn sway_client_update_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    Box::new(SwayClientUpdateModule::new(
        NameKeyGenerator::name_to_key("SwayClientUpdate"),
        module_data,
        resolve_owner_id(&thing),
    ))
}

pub(super) fn animated_particle_sys_bone_client_update_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ModuleData>,
) -> Box<dyn Module> {
    Box::new(AnimatedParticleSysBoneClientUpdateModule::new(
        NameKeyGenerator::name_to_key("AnimatedParticleSysBoneClientUpdate"),
        module_data,
        resolve_owner_id(&thing),
    ))
}
