//! Error-preserving data admission for an unpublished native module catalog.
//!
//! C++ Module.h's friend_newModuleData propagates initFromINIMultiProc errors.
//! The compatibility callbacks retain their historical logging behavior; this
//! registration is used only by the explicit strict content admission path.

use super::*;
use game_engine::common::thing::module_factory::{FallibleModuleDataProc, ModuleFactory};

/// One explicit recipient for the actual parser registration sequence. Tests
/// retain these same callbacks locally without opening Common's opaque candidate.
pub(super) trait DataParserRegistrar {
    fn register_data_parser(
        &mut self,
        name: &'static str,
        module_type: ModuleType,
        parser: FallibleModuleDataProc,
    );
}

impl DataParserRegistrar for ModuleFactory {
    fn register_data_parser(
        &mut self,
        name: &'static str,
        module_type: ModuleType,
        parser: FallibleModuleDataProc,
    ) {
        // Native constructor registration precedes authored data. The class,
        // not the authored field, establishes its actual C++ interface mask.
        use game_engine::common::thing::module::ModuleInterfaceType;
        use game_engine::common::thing::thing_template::{ModuleDescriptor, ModuleDescriptorSet};
        let interface_mask = match module_type {
            ModuleType::Draw => Some(ModuleInterfaceType::DRAW),
            ModuleType::ClientUpdate => Some(ModuleInterfaceType::CLIENT_UPDATE),
            ModuleType::Behavior
                if matches!(
                    name,
                    "CashBountyPower"
                        | "CashHackSpecialPower"
                        | "CleanupAreaPower"
                        | "FireWeaponPower"
                        | "OCLSpecialPower"
                        | "SpecialAbility"
                        | "SpyVisionSpecialPower"
                ) =>
            {
                Some(ModuleInterfaceType::SPECIAL_POWER)
            }
            // Other Behavior classes already have the source-derived builtin
            // masks, including every Body class (BodyModule.h:188).
            _ => None,
        };
        if let Some(interface_mask) = interface_mask {
            let descriptor = ModuleDescriptor {
                name: name.to_owned(),
                module_tag: String::new(),
                interface_mask,
                inheritable: false,
                overrideable_by_like_kind: false,
                copied_from_default: false,
            };
            let mut set = ModuleDescriptorSet::default();
            match module_type {
                ModuleType::Draw => set.draw.push(descriptor),
                ModuleType::ClientUpdate => set.client_update.push(descriptor),
                ModuleType::Behavior => set.behavior.push(descriptor),
                _ => unreachable!("native module family"),
            }
            self.register_descriptor_set(&set);
        }
        self.register_fallible_data_constructor(name, module_type, parser);
    }
}

macro_rules! register_parsed {
    ($factory:ident, $kind:ident, $name:literal, $data:ty, $new:ident) => {
        $factory.register_data_parser($name, ModuleType::$kind, |ini| {
            let mut data = <$data>::$new();
            if let Some(ini) = ini {
                data.parse_from_ini(ini).map_err(|error| {
                    format!(
                        "{} data at line {} [{}]: {}",
                        $name,
                        ini.get_line_num(),
                        ini.get_line_tokens().join(" "),
                        error
                    )
                })?;
            }
            Ok(Box::new(data))
        });
    };
}

macro_rules! register_empty {
    ($factory:ident, $kind:ident, $name:literal, $data:ty) => {
        register_empty!($factory, $kind, $name, $data, new);
    };
    ($factory:ident, $kind:ident, $name:literal, $data:ty, $new:ident) => {
        $factory.register_data_parser($name, ModuleType::$kind, |ini| {
            let mut data = <$data>::$new();
            if let Some(ini) = ini {
                // C++ ModuleData::buildFieldParse is empty. Empty data still
                // consumes End and rejects unknown authored fields.
                ini.init_from_ini_with_fields(&mut data, &[])
                    .map_err(|error| {
                        format!(
                            "{} data at line {} [{}]: {}",
                            $name,
                            ini.get_line_num(),
                            ini.get_line_tokens().join(" "),
                            error
                        )
                    })?;
            }
            Ok(Box::new(data))
        });
    };
}

macro_rules! register_crate {
    ($factory:ident, $kind:ident, $name:literal, $data:ty, $new:ident) => {
        $factory.register_data_parser($name, ModuleType::$kind, |ini| {
            let mut data = <$data>::$new();
            if let Some(ini) = ini {
                data.parse_from_ini(ini).map_err(|error| {
                    format!(
                        "{} data at line {} [{}]: {}",
                        $name,
                        ini.get_line_num(),
                        ini.get_line_tokens().join(" "),
                        error
                    )
                })?;
            }
            Ok(super::super::collide_crates::adapted_crate_data(data))
        });
    };
}

macro_rules! register_contain {
    ($factory:ident, $name:literal, $data:ty) => {
        $factory.register_data_parser($name, ModuleType::Behavior, |ini| {
            let mut data = <$data>::default();
            if let Some(ini) = ini {
                data.parse_from_ini(ini).map_err(|error| {
                    format!(
                        "{} data at line {} [{}]: {}",
                        $name,
                        ini.get_line_num(),
                        ini.get_line_tokens().join(" "),
                        error
                    )
                })?;
            }
            Ok(Box::new(
                super::super::contain::ContainModuleDataAdapter::new(data),
            ))
        });
    };
}

mod ai_upgrades_powers;
mod behaviors;
mod collide;
mod contain;
mod creation;
mod death;
mod production;
mod projectile_updates;
mod updates;

pub(super) fn register_native_data_parsers(factory: &mut impl DataParserRegistrar) {
    register_parsed!(factory, Draw, "W3DModelDraw", W3DModelDrawModuleData, new);
    register_parsed!(
        factory,
        Draw,
        "W3DDefaultDraw",
        W3DDefaultDrawModuleData,
        new
    );
    register_parsed!(
        factory,
        Draw,
        "W3DDependencyModelDraw",
        W3DDependencyModelDrawModuleData,
        new
    );
    register_parsed!(factory, Draw, "W3DTankDraw", W3DTankDrawModuleData, new);
    register_parsed!(
        factory,
        Draw,
        "W3DOverlordTankDraw",
        W3DOverlordTankDrawModuleData,
        new
    );
    register_parsed!(
        factory,
        Draw,
        "W3DOverlordAircraftDraw",
        W3DOverlordAircraftDrawModuleData,
        new
    );
    register_parsed!(
        factory,
        Draw,
        "W3DOverlordTruckDraw",
        W3DOverlordTruckDrawModuleData,
        new
    );
    register_parsed!(
        factory,
        Draw,
        "W3DPoliceCarDraw",
        W3DPoliceCarDrawModuleData,
        new
    );
    register_parsed!(
        factory,
        Draw,
        "W3DProjectileStreamDraw",
        W3DProjectileStreamDrawModuleData,
        new
    );
    register_empty!(factory, Draw, "W3DRopeDraw", W3DRopeDrawModuleData);
    register_parsed!(
        factory,
        Draw,
        "W3DScienceModelDraw",
        W3DScienceModelDrawModuleData,
        new
    );
    register_parsed!(factory, Draw, "W3DSupplyDraw", W3DSupplyDrawModuleData, new);
    register_parsed!(
        factory,
        Draw,
        "W3DTankTruckDraw",
        W3DTankTruckDrawModuleData,
        new
    );
    register_empty!(factory, Draw, "W3DTracerDraw", W3DTracerDrawModuleData);
    register_parsed!(factory, Draw, "W3DTreeDraw", W3DTreeDrawModuleData, new);
    register_parsed!(factory, Draw, "W3DPropDraw", W3DPropDrawModuleData, new);
    register_parsed!(factory, Draw, "W3DTruckDraw", W3DTruckDrawModuleData, new);
    register_parsed!(factory, Draw, "W3DLaserDraw", W3DLaserDrawModuleData, new);
    register_empty!(factory, Draw, "W3DDebrisDraw", W3DDebrisDrawModuleData);
    register_parsed!(
        factory,
        ClientUpdate,
        "LaserUpdate",
        LaserClientUpdateModuleData,
        default
    );
    register_parsed!(
        factory,
        ClientUpdate,
        "BeaconClientUpdate",
        BeaconClientUpdateModuleData,
        default
    );
    register_empty!(factory, ClientUpdate, "SwayClientUpdate", BaseModuleData);
    register_empty!(
        factory,
        ClientUpdate,
        "AnimatedParticleSysBoneClientUpdate",
        BaseModuleData
    );
    register_empty!(factory, Behavior, "InactiveBody", BodyModuleData);
    register_parsed!(
        factory,
        Behavior,
        "ActiveBody",
        ActiveBodyModuleData,
        default
    );
    register_parsed!(
        factory,
        Behavior,
        "StructureBody",
        StructureBodyModuleData,
        default
    );
    register_parsed!(
        factory,
        Behavior,
        "HighlanderBody",
        ActiveBodyModuleData,
        default
    );
    register_parsed!(
        factory,
        Behavior,
        "ImmortalBody",
        ActiveBodyModuleData,
        default
    );
    register_parsed!(
        factory,
        Behavior,
        "HiveStructureBody",
        HiveStructureBodyModuleData,
        default
    );
    register_parsed!(
        factory,
        Behavior,
        "UndeadBody",
        UndeadBodyModuleData,
        default
    );
    death::register(factory);
    projectile_updates::register(factory);
    updates::register(factory);
    production::register(factory);
    ai_upgrades_powers::register(factory);
    behaviors::register(factory);
    collide::register(factory);
    creation::register(factory);
    contain::register(factory);
}

#[cfg(test)]
mod tests;
