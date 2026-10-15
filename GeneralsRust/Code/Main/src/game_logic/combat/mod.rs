//! Live host weapon/projectile combat, split by original C++ behavior ownership.
//!
//! The fragments are textual members of this module so item visibility, call order,
//! RNG consumption, frame timing, serialization, and the public API remain unchanged.

use super::*;
use glam::Vec3;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

include!("damage.rs");
include!("projectile.rs");
include!("weapon_fire.rs");
include!("resolution.rs");
include!("queued_area.rs");
include!("pending_snapshot.rs");

mod live_projectile_snapshot;
pub(crate) use live_projectile_snapshot::{
    LiveProjectileArenaSnapshot, ValidatedLiveProjectileArena,
};

#[cfg(test)]
include!("tests.rs");

#[cfg(test)]
mod projectile_clock_owner_tests;

#[cfg(test)]
mod projectile_callback_owner_tests;

#[cfg(test)]
mod projectile_fx_owner_tests;
