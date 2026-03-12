// FILE: animated_particle_sys_bone_client_update.rs
// Author: Mark Lorenzen, October 2002
// Desc: Client update module to translate particle systems with animation
// Ported from C++ to Rust

use crate::GameClient::drawable::Drawable;
use crate::GameClient::draw_module::{DrawModule, ObjectDrawInterface};

// Type aliases matching C++ base types
pub type UnsignedInt = u32;
pub type Bool = bool;

/// Client update module data - base configuration for modules
/// Matches C++ ClientUpdateModuleData from ClientUpdateModule.h line 20
pub trait ClientUpdateModuleData {}

/// Base trait for client update modules
/// Matches C++ ClientUpdateModule from ClientUpdateModule.h line 25
pub trait ClientUpdateModule {
    /// The client update callback - must be implemented by derived modules
    /// Matches C++ ClientUpdateModule::clientUpdate from ClientUpdateModule.h line 36
    fn client_update(&mut self);

    /// CRC calculation for save game verification
    /// Matches C++ ClientUpdateModule::crc
    fn crc(&self, xfer: &mut dyn XferInterface);

    /// Serialization/deserialization
    /// Matches C++ ClientUpdateModule::xfer
    fn xfer(&mut self, xfer: &mut dyn XferInterface);

    /// Load post process - resolve references after loading
    /// Matches C++ ClientUpdateModule::loadPostProcess
    fn load_post_process(&mut self);

    /// Get the drawable this module is attached to
    fn get_drawable(&mut self) -> Option<&mut Drawable>;
}

/// Xfer interface for serialization
/// Matches C++ Xfer class from Common/Xfer.h
pub trait XferInterface {
    fn xfer_version(&mut self, version: &mut u32, current_version: u32);
    fn xfer_unsigned_int(&mut self, value: &mut UnsignedInt);
    fn xfer_real(&mut self, value: &mut f32);
    fn xfer_bool(&mut self, value: &mut Bool);
    fn xfer_short(&mut self, value: &mut i16);
    fn xfer_user(&mut self, data: &mut [u8]);
}

/// Animated particle system bone client update module
/// Updates particle systems attached to animated bones
/// Matches C++ AnimatedParticleSysBoneClientUpdate from AnimatedParticleSysBoneClientUpdate.h line 17
pub struct AnimatedParticleSysBoneClientUpdate {
    /// Pointer to the drawable this module is attached to
    /// Matches C++ Thing* in base class
    drawable: Option<*mut Drawable>,

    /// Module configuration data
    /// Matches C++ const ModuleData* moduleData in base class
    module_data: Option<*const dyn ClientUpdateModuleData>,

    /// Life counter - increments each frame
    /// Matches C++ AnimatedParticleSysBoneClientUpdate::m_life line 35
    life: UnsignedInt,
}

impl AnimatedParticleSysBoneClientUpdate {
    /// Constructor
    /// Matches C++ AnimatedParticleSysBoneClientUpdate::AnimatedParticleSysBoneClientUpdate
    /// from AnimatedParticleSysBoneClientUpdate.cpp line 24
    pub fn new(
        drawable: Option<*mut Drawable>,
        module_data: Option<*const dyn ClientUpdateModuleData>,
    ) -> Self {
        Self {
            drawable,
            module_data,
            life: 0,
        }
    }

    /// Get life counter value
    pub fn get_life(&self) -> UnsignedInt {
        self.life
    }
}

impl ClientUpdateModule for AnimatedParticleSysBoneClientUpdate {
    /// The client update callback
    /// Matches C++ AnimatedParticleSysBoneClientUpdate::clientUpdate
    /// from AnimatedParticleSysBoneClientUpdate.cpp line 42
    fn client_update(&mut self) {
        // THIS IS HAPPENING CLIENT-SIDE
        // I CAN DO WHAT I NEED HERE AND NOT HAVE TO BE LOGIC SYNC-SAFE

        // Increment life counter
        // Matches C++ line 48
        self.life = self.life.wrapping_add(1);

        // Get the drawable
        // Matches C++ line 50
        if let Some(drawable_ptr) = self.drawable {
            let draw = unsafe { &mut *drawable_ptr };

            // Iterate through draw modules and update bones for client particle systems
            // Matches C++ lines 54-62
            if let Some(draw_modules) = draw.get_draw_modules_mut() {
                for dm in draw_modules.iter_mut() {
                    if let Some(di) = dm.get_object_draw_interface() {
                        // Update bones for client particle systems
                        // If successful, break out of the loop
                        // Matches C++ lines 59-60
                        if di.update_bones_for_client_particle_systems() {
                            break;
                        }
                    }
                }
            }
        }
    }

    /// CRC calculation for save game verification
    /// Matches C++ AnimatedParticleSysBoneClientUpdate::crc
    /// from AnimatedParticleSysBoneClientUpdate.cpp line 73
    fn crc(&self, xfer: &mut dyn XferInterface) {
        // Extend base class
        // In C++ this calls ClientUpdateModule::crc(xfer)
        // Base implementation is empty, so nothing to do here
    }

    /// Serialization/deserialization
    /// Version Info:
    /// 1: Initial version
    /// Matches C++ AnimatedParticleSysBoneClientUpdate::xfer
    /// from AnimatedParticleSysBoneClientUpdate.cpp line 86
    fn xfer(&mut self, xfer: &mut dyn XferInterface) {
        // Version tracking
        // Matches C++ lines 90-92
        let current_version: u32 = 1;
        let mut version = current_version;
        xfer.xfer_version(&mut version, current_version);

        // Extend base class
        // In C++ this calls ClientUpdateModule::xfer(xfer)
        // Base class handles thing and moduleData, which are already in our struct
    }

    /// Load post process - resolve references after loading
    /// Matches C++ AnimatedParticleSysBoneClientUpdate::loadPostProcess
    /// from AnimatedParticleSysBoneClientUpdate.cpp line 103
    fn load_post_process(&mut self) {
        // Extend base class
        // In C++ this calls ClientUpdateModule::loadPostProcess()
        // Base implementation handles reference resolution
    }

    /// Get the drawable this module is attached to
    fn get_drawable(&mut self) -> Option<&mut Drawable> {
        self.drawable.map(|ptr| unsafe { &mut *ptr })
    }
}

// Note: In C++ this uses MEMORY_POOL_GLUE_WITH_USERLOOKUP_CREATE macro
// In Rust, we would typically use an allocator or Box, but for now we keep raw pointers
// to match C++ memory management patterns

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_construction() {
        let module = AnimatedParticleSysBoneClientUpdate::new(None, None);
        assert_eq!(module.get_life(), 0);
    }

    #[test]
    fn test_life_increments() {
        let mut module = AnimatedParticleSysBoneClientUpdate::new(None, None);

        // Initial life is 0
        assert_eq!(module.get_life(), 0);

        // After one update, life should be 1
        module.client_update();
        assert_eq!(module.get_life(), 1);

        // After another update, life should be 2
        module.client_update();
        assert_eq!(module.get_life(), 2);
    }

    #[test]
    fn test_life_wraps_on_overflow() {
        let mut module = AnimatedParticleSysBoneClientUpdate::new(None, None);
        module.life = UnsignedInt::MAX;

        // Should wrap to 0
        module.client_update();
        assert_eq!(module.get_life(), 0);
    }
}
