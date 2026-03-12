//! Client Update Modules
//!
//! This module contains client-side update modules that handle visual effects,
//! animations, and other client-only behaviors that don't need to be synced
//! across the network in multiplayer games.
//!
//! Ported from C++ source located at:
//! /GeneralsMD/Code/GameEngine/Source/GameClient/Drawable/Update/
//!
//! ## Module Overview
//!
//! ### AnimatedParticleSysBoneClientUpdate
//! Updates particle systems attached to animated bones, ensuring particles
//! follow skeletal animation correctly. Used for effects like unit exhaust,
//! muzzle flashes, and other bone-attached particle effects.
//!
//! ### BeaconClientUpdate
//! Manages beacon smoke effects and radar pulses. Creates colored smoke plumes
//! at beacon locations and periodically triggers radar events to make the
//! beacon visible on the minimap.
//!
//! ### SwayClientUpdate
//! Handles tree and object swaying in the wind. Applies smooth oscillating
//! rotations based on breeze parameters from the script engine, including
//! intensity, period, randomness, and direction.
//!
//! ## Architecture
//!
//! All client update modules implement the `ClientUpdateModule` trait which
//! provides the following core methods:
//!
//! - `client_update()`: Called each frame to update visual state
//! - `xfer()`: Serialization for save/load
//! - `crc()`: CRC verification for save game integrity
//! - `load_post_process()`: Post-load reference resolution
//!
//! Client update modules are CLIENT-SIDE ONLY and do not need to maintain
//! sync-safe deterministic behavior like logic modules do. They can use
//! random values, access renderer state, and perform other non-deterministic
//! operations safely.

// Module declarations
pub mod animated_particle_sys_bone_client_update;
pub mod beacon_client_update;
pub mod sway_client_update;

// Re-export main types for convenience
pub use animated_particle_sys_bone_client_update::{
    AnimatedParticleSysBoneClientUpdate,
    ClientUpdateModule as AnimatedClientUpdateModule,
    ClientUpdateModuleData as AnimatedClientUpdateModuleData,
};

pub use beacon_client_update::{
    BeaconClientUpdate,
    BeaconClientUpdateModuleData,
    RadarInterface,
    GameLogicInterface,
    RadarEventType,
    Coord3D,
    INVALID_PARTICLE_SYSTEM_ID,
    SECONDS_PER_LOGICFRAME_REAL,
};

pub use sway_client_update::{
    SwayClientUpdate,
    BreezeInfo,
    ScriptEngineInterface,
    Vector3,
    Matrix3D,
    OBJECT_STATUS_BURNED,
    PI,
};

// Common type aliases used across all modules
pub type Real = f32;
pub type Bool = bool;
pub type Int = i32;
pub type UnsignedInt = u32;
pub type Short = i16;
pub type UnsignedShort = u16;

/// Common client update module trait
/// This is the unified interface that all client update modules implement
pub trait ClientUpdateModule {
    /// Called each frame to perform client-side updates
    fn client_update(&mut self);

    /// CRC calculation for save game verification
    fn crc(&self, xfer: &mut dyn XferInterface);

    /// Serialization/deserialization
    fn xfer(&mut self, xfer: &mut dyn XferInterface);

    /// Post-load reference resolution
    fn load_post_process(&mut self);

    /// Get the drawable this module is attached to
    fn get_drawable(&mut self) -> Option<&mut crate::GameClient::drawable::Drawable>;
}

/// Xfer interface for serialization
pub trait XferInterface {
    fn xfer_version(&mut self, version: &mut u32, current_version: u32);
    fn xfer_unsigned_int(&mut self, value: &mut UnsignedInt);
    fn xfer_real(&mut self, value: &mut Real);
    fn xfer_bool(&mut self, value: &mut Bool);
    fn xfer_short(&mut self, value: &mut Short);
    fn xfer_user(&mut self, data: &mut [u8]);
}

/// Client update module data trait
/// Base configuration interface for all module data
pub trait ClientUpdateModuleData {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_module_exports() {
        // Verify that all modules are properly exported
        // This is a compile-time test - if it compiles, exports work

        // Type aliases should be available
        let _x: Real = 1.0;
        let _y: Bool = true;
        let _z: Int = 42;
        let _w: UnsignedInt = 100;
    }

    #[test]
    fn test_constants() {
        // Verify mathematical constants
        assert!((PI - std::f32::consts::PI).abs() < 0.0001);

        // Verify beacon constants
        assert!((SECONDS_PER_LOGICFRAME_REAL - 1.0 / 30.0).abs() < 0.0001);
        assert_eq!(INVALID_PARTICLE_SYSTEM_ID.get_id(), u32::MAX);
    }
}
