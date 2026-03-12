//! Client update modules for drawables.
//!
//! These are thin wrappers around the GameLogic implementations to keep
//! file parity with the original GameClient/Drawable/Update sources.

pub mod animated_particle_sys_bone_client_update;
pub mod beacon_client_update;
pub mod sway_client_update;

pub use animated_particle_sys_bone_client_update::AnimatedParticleSysBoneClientUpdateModule;
pub use beacon_client_update::{BeaconClientUpdateModule, BeaconClientUpdateModuleData};
pub use sway_client_update::SwayClientUpdateModule;
