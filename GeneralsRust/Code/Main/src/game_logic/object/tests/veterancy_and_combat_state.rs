//! Behavior suite extracted from the original test module.
use super::*;

mod body_damage_and_pre_attack_delay;
mod damage_residuals_and_weapon_status;
mod death_modules_and_rubble;
mod targeting_and_weapon_timing;
mod veterancy_and_object_status;

use veterancy_and_object_status::{assert_promotion_present, promotion_world};
