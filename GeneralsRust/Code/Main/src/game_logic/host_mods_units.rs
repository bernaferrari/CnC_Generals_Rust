//! Host unit / vehicle / hero modules.
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

#[path = "host_angry_mob.rs"]
pub mod host_angry_mob;

#[path = "host_avenger.rs"]
pub mod host_avenger;

#[path = "host_battle_bus.rs"]
pub mod host_battle_bus;

#[path = "host_battlemaster.rs"]
pub mod host_battlemaster;

#[path = "host_colonel_burton.rs"]
pub mod host_colonel_burton;

#[path = "host_dragon_tank.rs"]
pub mod host_dragon_tank;

#[path = "host_ecm_jam.rs"]
pub mod host_ecm_jam;

#[path = "host_firewall.rs"]
pub mod host_firewall;

#[path = "host_gattling_tank.rs"]
pub mod host_gattling_tank;

#[path = "host_gla_rebel.rs"]
pub mod host_gla_rebel;

#[path = "host_gla_worker.rs"]
pub mod host_gla_worker;

#[path = "host_hacker_disable.rs"]
pub mod host_hacker_disable;

#[path = "host_height_die.rs"]
pub mod host_height_die;

#[path = "host_humvee.rs"]
pub mod host_humvee;

#[path = "host_jarmen_kell.rs"]
pub mod host_jarmen_kell;

#[path = "host_lifetime_update.rs"]
pub mod host_lifetime_update;

#[path = "host_marauder.rs"]
pub mod host_marauder;

#[path = "host_microwave.rs"]
pub mod host_microwave;

#[path = "host_mig.rs"]
pub mod host_mig;

#[path = "host_minigunner.rs"]
pub mod host_minigunner;

#[path = "host_pathfinder.rs"]
pub mod host_pathfinder;

#[path = "host_quad_cannon.rs"]
pub mod host_quad_cannon;

#[path = "host_ranger.rs"]
pub mod host_ranger;

#[path = "host_railed_transport.rs"]
pub mod host_railed_transport;
#[path = "host_railroad.rs"]
pub mod host_railroad;

#[path = "host_raptor.rs"]
pub mod host_raptor;

#[path = "host_rocket_buggy.rs"]
pub mod host_rocket_buggy;

#[path = "host_rpg_trooper.rs"]
pub mod host_rpg_trooper;

#[path = "host_saboteur.rs"]
pub mod host_saboteur;

#[path = "host_scorpion.rs"]
pub mod host_scorpion;

#[path = "host_scud_launcher.rs"]
pub mod host_scud_launcher;

#[path = "host_sentry_drone.rs"]
pub mod host_sentry_drone;

#[path = "host_slave_drones.rs"]
pub mod host_slave_drones;

#[path = "host_tank_hunter.rs"]
pub mod host_tank_hunter;

#[path = "host_technical.rs"]
pub mod host_technical;

#[path = "host_tensile_formation.rs"]
pub mod host_tensile_formation;

#[path = "host_terrorist.rs"]
pub mod host_terrorist;

#[path = "host_tomahawk.rs"]
pub mod host_tomahawk;

#[path = "host_troop_crawler.rs"]
pub mod host_troop_crawler;

#[path = "host_usa_pilot.rs"]
pub mod host_usa_pilot;

#[path = "host_usa_tanks.rs"]
pub mod host_usa_tanks;
