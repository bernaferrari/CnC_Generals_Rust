//! Named behavior suites; tests keep their original relative source order.
use super::*;

#[path = "specialist_weapons_and_emp.rs"]
mod specialist_weapons_and_emp;

#[path = "chemical_upgrades_and_sciences.rs"]
mod chemical_upgrades_and_sciences;

#[path = "demolition_weapons.rs"]
mod demolition_weapons;

#[path = "attack_paths_and_terrain.rs"]
mod attack_paths_and_terrain;

#[path = "airfield_parking.rs"]
mod airfield_parking;

#[path = "combat_source_contract_tests.rs"]
mod combat_source_contract_tests;
