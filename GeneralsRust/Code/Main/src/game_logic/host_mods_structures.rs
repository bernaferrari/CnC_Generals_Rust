//! Host structure / economy / production modules.
//!
//! `#[path]` keeps the `.rs` files in this directory. Parent types and a
//! few sibling modules are imported so existing `use super::ObjectId`
//! (and similar) paths in those files keep resolving.

#![allow(unused_imports)]

use super::ObjectId;
use super::Team;
use super::VeterancyLevel;
use super::Weapon;
use super::combat;

use super::host_rng_residual;

#[path = "host_active_shroud_upgrade.rs"]
pub mod host_active_shroud_upgrade;

#[path = "host_base_defense.rs"]
pub mod host_base_defense;

#[path = "host_bridge_behavior.rs"]
pub mod host_bridge_behavior;

#[path = "host_cave_system.rs"]
pub mod host_cave_system;

#[path = "host_base_regenerate.rs"]
pub mod host_base_regenerate;

#[path = "host_black_market.rs"]
pub(super) mod host_black_market;

#[path = "host_hacker_income.rs"]
pub mod host_hacker_income;

#[path = "host_listening_outpost.rs"]
pub mod host_listening_outpost;

#[path = "host_model_condition_upgrade.rs"]
pub(super) mod host_model_condition_upgrade;

#[path = "host_money_crate.rs"]
pub mod host_money_crate;

#[path = "host_oil_derrick.rs"]
pub mod host_oil_derrick;

#[path = "host_flight_deck.rs"]
pub mod host_flight_deck;

#[path = "host_overlord_addons.rs"]
pub mod host_overlord_addons;

#[path = "host_overlord_addon_damage.rs"]
pub mod host_overlord_addon_damage;

#[path = "host_overlord_gun.rs"]
pub mod host_overlord_gun;

#[path = "host_preorder_create.rs"]
pub(super) mod host_preorder_create;

#[path = "host_propaganda.rs"]
pub mod host_propaganda;

#[path = "host_radar.rs"]
pub mod host_radar;

#[path = "host_repair.rs"]
pub mod host_repair;

#[path = "host_replace_object_upgrade.rs"]
pub(super) mod host_replace_object_upgrade;

#[path = "host_status_bits_upgrade.rs"]
pub mod host_status_bits_upgrade;

#[path = "host_strategy_center.rs"]
pub mod host_strategy_center;

#[path = "host_structure_collapse.rs"]
pub mod host_structure_collapse;

#[path = "host_sub_objects_upgrade.rs"]
pub mod host_sub_objects_upgrade;

#[path = "host_supply_drop_zone.rs"]
pub mod host_supply_drop_zone;

#[path = "host_supply_gather.rs"]
pub mod host_supply_gather;

#[path = "host_tunnel_network.rs"]
pub mod host_tunnel_network;

#[path = "host_upgrades.rs"]
pub mod host_upgrades;
