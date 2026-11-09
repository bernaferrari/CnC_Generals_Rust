//! Exact native Draw/ClientUpdate assembly from an admitted template.
//!
//! Drawable.cpp:436-475 preserves Draw then ClientUpdate declaration order.
//! Validate the entire plan before constructing callbacks or assigning a
//! receiving Drawable identity. Each admitted module retains its shared data
//! handle; invalid data never manufactures a default.
use super::*;
use game_engine::common::name_key_generator::NameKeyNamespace;
use game_engine::common::thing::module::StaticGameLodLevel;
use game_engine::common::thing::module::{BaseModuleData, Module, ModuleDataHandle, ModuleInterfaceType};
use game_engine::common::thing::thing_template::ModuleInfoEntry;
use gamelogic::object::draw::{
    W3DDefaultDraw, W3DDefaultDrawModuleData, W3DDependencyModelDraw,
    W3DDependencyModelDrawModuleData, W3DProjectileStreamDraw, W3DProjectileStreamDrawModuleData,
    W3DPropDraw, W3DPropDrawModuleData, W3DRopeDraw, W3DRopeDrawModuleData, W3DSupplyDraw,
    W3DSupplyDrawModuleData, W3DTracerDraw, W3DTracerDrawModuleData,
};
use gamelogic::object::update::BeaconClientUpdateModuleData;
use std::collections::HashSet;

/// Original Drawable constructor options, captured from the receiving application.
#[derive(Debug, Clone, Copy)]
pub struct ReceivingDrawablePolicy {
    /// Receiving session value; Invalid has no native representation.
    pub time_of_day: Option<crate::system::TimeOfDay>,
    pub use_draw_module_lod: bool,
    pub static_game_lod: StaticGameLodLevel,
    pub use_tree_sway: bool,
}

fn invalid_entry(entry: ModuleInfoEntry<'_>, reason: &str) -> GameClientError {
    GameClientError::InvalidOperation(format!(
        "Drawable module '{}' tag '{}': {reason}",
        entry.name, entry.module_tag
    ))
}

fn require_data<T: 'static>(entry: ModuleInfoEntry<'_>) -> GameClientResult<()> {
    if entry.data.read().as_any().is::<T>() {
        Ok(())
    } else {
        Err(invalid_entry(
            entry,
            &format!("expected typed data {}", std::any::type_name::<T>()),
        ))
    }
}

fn validate_entry<'a>(
    entry: ModuleInfoEntry<'a>,
    interface: ModuleInterfaceType,
    tags: &mut HashSet<&'a str>,
) -> GameClientResult<()> {
    if entry.module_tag.is_empty() {
        return Err(invalid_entry(entry, "missing module tag"));
    }
    if !tags.insert(entry.module_tag.as_str()) {
        return Err(invalid_entry(entry, "duplicate module tag"));
    }
    if !entry.supports(interface) {
        return Err(invalid_entry(
            entry,
            "module interface does not match its Drawable bucket",
        ));
    }
    Ok(())
}

macro_rules! native_draw_data {
    ($(($module:ident, $data:ident)),+ $(,)?) => {
        enum DrawData { $( $module(ModuleDataHandle), )+ }
        impl DrawData {
            fn from_entry(entry: ModuleInfoEntry<'_>) -> GameClientResult<Self> {
                match entry.name.as_str() {
                    $(stringify!($module) => {
                        require_data::<$data>(entry)?;
                        Ok(Self::$module(entry.data.clone()))
                    },)+
                    _ => Err(invalid_entry(entry, "unknown native Draw class")),
                }
            }
            fn construct(self) -> Box<dyn Module> {
                match self { $(Self::$module(data) => Box::new($module::new_with_module_data(data)),)+ }
            }
        }
    };
}

// W3DModuleFactory's actual registered classes, not a rendering-name heuristic.
native_draw_data!(
    (W3DModelDraw, W3DModelDrawModuleData),
    (W3DDefaultDraw, W3DDefaultDrawModuleData),
    (W3DDependencyModelDraw, W3DDependencyModelDrawModuleData),
    (W3DTankDraw, W3DTankDrawModuleData),
    (W3DOverlordTankDraw, W3DOverlordTankDrawModuleData),
    (W3DOverlordAircraftDraw, W3DOverlordAircraftDrawModuleData),
    (W3DOverlordTruckDraw, W3DOverlordTruckDrawModuleData),
    (W3DPoliceCarDraw, W3DPoliceCarDrawModuleData),
    (W3DProjectileStreamDraw, W3DProjectileStreamDrawModuleData),
    (W3DRopeDraw, W3DRopeDrawModuleData),
    (W3DScienceModelDraw, W3DScienceModelDrawModuleData),
    (W3DSupplyDraw, W3DSupplyDrawModuleData),
    (W3DTankTruckDraw, W3DTankTruckDrawModuleData),
    (W3DTracerDraw, W3DTracerDrawModuleData),
    (W3DTreeDraw, W3DTreeDrawModuleData),
    (W3DPropDraw, W3DPropDrawModuleData),
    (W3DTruckDraw, W3DTruckDrawModuleData),
    (W3DLaserDraw, W3DLaserDrawModuleData),
    (W3DDebrisDraw, W3DDebrisDrawModuleData),
);

enum ClientData {
    Laser(ModuleDataHandle),
    Beacon(ModuleDataHandle),
    Sway(ModuleDataHandle),
    AnimatedBones(ModuleDataHandle),
}

impl ClientData {
    fn from_entry(entry: ModuleInfoEntry<'_>) -> GameClientResult<Self> {
        match entry.name.as_str() {
            "LaserUpdate" => {
                let data = entry.data.read();
                if !data.as_any().is::<LaserUpdateModuleData>()
                    && !data
                        .as_any()
                        .is::<gamelogic::object::behavior::laser_update::LaserUpdateModuleData>()
                {
                    return Err(invalid_entry(entry, "LaserUpdate module data type mismatch"));
                }
                Ok(Self::Laser(entry.data.clone()))
            }
            "BeaconClientUpdate" => {
                require_data::<BeaconClientUpdateModuleData>(entry)?;
                if !entry.data.is::<BeaconClientUpdateModuleData>() {
                    return Err(invalid_entry(
                        entry,
                        "expected admitted BeaconClientUpdateModuleData allocation",
                    ));
                }
                Ok(Self::Beacon(entry.data.clone()))
            }
            "SwayClientUpdate" => {
                require_data::<BaseModuleData>(entry)?;
                Ok(Self::Sway(entry.data.clone()))
            }
            "AnimatedParticleSysBoneClientUpdate" => {
                require_data::<BaseModuleData>(entry)?;
                Ok(Self::AnimatedBones(entry.data.clone()))
            }
            _ => Err(invalid_entry(entry, "unknown native ClientUpdate class")),
        }
    }

    fn construct(
        self,
        name_key: u32,
        owner: Option<(u32, u32)>,
    ) -> GameClientResult<Box<dyn Module>> {
        Ok(match self {
            Self::Laser(data) => Box::new(GameClient::make_owned_laser_module(name_key, data)?),
            Self::Beacon(data) => match owner {
                Some((id, frame)) => {
                    Box::new(BeaconClientUpdateModule::new_at_with_module_data(name_key, data, id, frame))
                }
                None => Box::new(BeaconClientUpdateModule::new_with_module_data(name_key, data, INVALID_ID)),
            },
            Self::Sway(data) => Box::new(SwayClientUpdateModule::new_with_module_data(
                name_key,
                data,
                owner.map_or(INVALID_ID, |(id, _)| id),
            )),
            Self::AnimatedBones(data) => Box::new(AnimatedParticleSysBoneClientUpdateModule::new_with_module_data(
                name_key,
                data,
                owner.map_or(INVALID_ID, |(id, _)| id),
            )),
        })
    }
}

impl GameClient {
    pub(super) fn create_snapshot_modules_from_template(
        template: &ThingTemplate,
    ) -> GameClientResult<Vec<Box<dyn DrawModule>>> {
        Self::create_modules_from_entries(
            template.get_draw_module_info(),
            template.get_client_update_module_info(),
            None,
            None,
            false,
            None,
        )
    }

    pub(super) fn create_receiving_modules_from_plan(
        plan: &game_engine::common::thing::thing_template::AdmittedReceivingTemplate,
        keys: &mut NameKeyNamespace,
        policy: ReceivingDrawablePolicy,
        owner: (u32, u32),
    ) -> GameClientResult<Vec<Box<dyn DrawModule>>> {
        Self::create_modules_from_entries(
            plan.draw_modules(),
            plan.client_update_modules(),
            Some(keys),
            Some(policy),
            plan.is_shrubbery(),
            Some(owner),
        )
    }

    /// Drawable.cpp constructor phase before any simulation Object is bound.
    /// Both actor-associated and genuine objectless lifetimes use this same
    /// receiving module factory and original inter-module initialization order.
    pub(super) fn initialize_receiving_drawable_modules_unbound(
        drawable: &mut BasicDrawable,
        admitted: &game_engine::common::thing::thing_template::AdmittedReceivingTemplate,
        keys: &mut NameKeyNamespace,
        policy: ReceivingDrawablePolicy,
        logic_frame: u32,
    ) -> GameClientResult<()> {
        let modules = Self::create_receiving_modules_from_plan(
            admitted,
            keys,
            policy,
            (INVALID_ID, logic_frame),
        )?;
        // Inter-module resolution sees every installed module, as CPP.
        for module in modules {
            drawable.add_draw_module(module);
        }
        for slot in drawable.get_draw_modules_mut() {
            if let Some(module) = slot.logic_module_mut() {
                gamelogic::object::drawable::create_native_draw_module_unbound(
                    module,
                    admitted.per_unit_sound_event("TruckLandingSound"),
                    admitted.per_unit_sound_event("TruckPowerslideSound"),
                );
            }
        }
        Ok(())
    }

    fn create_modules_from_entries(
        draws_info: &game_engine::common::thing::thing_template::ModuleInfo,
        clients_info: &game_engine::common::thing::thing_template::ModuleInfo,
        keys: Option<&mut NameKeyNamespace>,
        policy: Option<ReceivingDrawablePolicy>,
        shrubbery: bool,
        owner: Option<(u32, u32)>,
    ) -> GameClientResult<Vec<Box<dyn DrawModule>>> {
        let mut tags = HashSet::new();
        let mut draws = Vec::new();
        let mut clients = Vec::new();
        let mut lasers = 0;
        for entry in draws_info.iter() {
            validate_entry(entry, ModuleInterfaceType::DRAW, &mut tags)?;
            if policy.is_some_and(|policy| {
                policy.use_draw_module_lod
                    && (entry.data.read().get_minimum_required_game_lod() as i32)
                        > (policy.static_game_lod as i32)
            }) {
                continue;
            }
            draws.push((entry.module_tag.as_str(), DrawData::from_entry(entry)?));
        }
        for entry in clients_info.iter() {
            validate_entry(entry, ModuleInterfaceType::CLIENT_UPDATE, &mut tags)?;
            if shrubbery
                && policy.is_some_and(|policy| !policy.use_tree_sway)
                && entry.name.as_str() == "SwayClientUpdate"
            {
                continue;
            }
            let data = ClientData::from_entry(entry)?;
            if matches!(data, ClientData::Laser(_)) {
                lasers += 1;
                if lasers > 1 {
                    return Err(invalid_entry(
                        entry,
                        "multiple authored LaserUpdate callbacks are not admitted",
                    ));
                }
            }
            clients.push((entry.name.as_str(), entry.module_tag.as_str(), data));
        }

        let mut modules: Vec<Box<dyn DrawModule>> = Vec::with_capacity(draws.len() + clients.len());
        for (tag, data) in draws {
            modules.push(Box::new(LogicDrawModuleSnapshotAdapter::draw_module(
                tag,
                data.construct(),
            )));
        }
        for (name, tag, data) in clients {
            // Module.h getModuleNameKey allocates lazily at an actual query.
            // Receiving callers use get_module_name_key_with_namespace; the
            // context-free compatibility key is never used on that route.
            let key = if keys.is_some() {
                game_engine::common::name_key_generator::NAMEKEY_INVALID
            } else {
                NameKeyGenerator::name_to_key(name)
            };
            modules.push(Box::new(
                LogicDrawModuleSnapshotAdapter::client_update_module(
                    tag,
                    data.construct(key, owner)?,
                ),
            ));
        }
        Ok(modules)
    }
}

#[cfg(test)]
mod tests;

#[cfg(test)]
#[path = "template_modules/receiving_owner_tests.rs"]
mod receiving_owner_tests;
