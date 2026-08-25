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

#[cfg(test)]
include!("tests.rs");
