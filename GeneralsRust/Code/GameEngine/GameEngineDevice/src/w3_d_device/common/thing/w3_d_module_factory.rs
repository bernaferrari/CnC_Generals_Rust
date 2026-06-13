// FILE: w3_d_module_factory.rs
// Ported from C++ W3DModuleFactory.h/.cpp

use game_engine::common::{
    system::subsystem_interface::{SubsystemInterface, SubsystemResult},
    thing::{
        module::{ModuleInterfaceType, ModuleType},
        ModuleFactory,
    },
};

const W3D_DRAW_MODULES: [&str; 19] = [
    "W3DDefaultDraw",
    "W3DDebrisDraw",
    "W3DModelDraw",
    "W3DLaserDraw",
    "W3DOverlordTankDraw",
    "W3DOverlordTruckDraw",
    "W3DOverlordAircraftDraw",
    "W3DProjectileStreamDraw",
    "W3DPoliceCarDraw",
    "W3DRopeDraw",
    "W3DScienceModelDraw",
    "W3DSupplyDraw",
    "W3DDependencyModelDraw",
    "W3DTankDraw",
    "W3DTruckDraw",
    "W3DTracerDraw",
    "W3DTankTruckDraw",
    "W3DTreeDraw",
    "W3DPropDraw",
];

/// W3D-specific module factory.
///
/// The C++ class extends `ModuleFactory::init()` by registering the W3D draw
/// module templates in a fixed order.
pub struct W3DModuleFactory {
    base: ModuleFactory,
    registered_w3d_draw_modules: Vec<&'static str>,
}

impl W3DModuleFactory {
    /// Creates a W3D module factory with a fresh base `ModuleFactory`.
    pub fn new() -> Self {
        Self {
            base: ModuleFactory::new(),
            registered_w3d_draw_modules: Vec::new(),
        }
    }

    /// Initializes the base factory and registers the W3D draw modules in C++ order.
    pub fn init(&mut self) -> SubsystemResult<()> {
        self.base.init()?;
        self.registered_w3d_draw_modules.clear();

        for module_name in W3D_DRAW_MODULES {
            self.base.add_module_internal(
                None,
                None,
                ModuleType::Draw,
                module_name,
                ModuleInterfaceType::DRAW,
            );
            self.registered_w3d_draw_modules.push(module_name);
        }

        Ok(())
    }

    /// Returns the underlying shared module factory.
    pub fn base(&self) -> &ModuleFactory {
        &self.base
    }

    /// Returns the underlying shared module factory mutably.
    pub fn base_mut(&mut self) -> &mut ModuleFactory {
        &mut self.base
    }

    /// Returns the W3D draw module names registered by the last `init()` call.
    pub fn registered_w3d_draw_modules(&self) -> &[&'static str] {
        &self.registered_w3d_draw_modules
    }
}

impl Default for W3DModuleFactory {
    fn default() -> Self {
        Self::new()
    }
}

/// Returns the C++ `W3DModuleFactory::init()` draw-module registration order.
pub fn w3d_draw_modules_in_cpp_registration_order() -> &'static [&'static str] {
    &W3D_DRAW_MODULES
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn w3d_module_factory_registers_cpp_draw_modules_in_order() {
        let mut factory = W3DModuleFactory::new();
        factory.init().expect("W3DModuleFactory init");

        assert_eq!(
            factory.registered_w3d_draw_modules(),
            w3d_draw_modules_in_cpp_registration_order()
        );
    }
}
