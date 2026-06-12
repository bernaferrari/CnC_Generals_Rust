//! Crate collision modules
//!
//! This module contains all crate collision behaviors including:
//! - Power-up crates (money, heal, veterancy, shroud, units)
//! - Salvage crates
//! - Conversion crates (car bomb, hijacked vehicle)
//! - Sabotage crates (8 types)

// Base crate collision module
pub mod crate_collide;

// Power-up crates
pub mod heal_crate_collide;
pub mod money_crate_collide;
pub mod shroud_crate_collide;
pub mod unit_crate_collide;
pub mod veterancy_crate_collide;

// Salvage crate
pub mod salvage_crate_collide;

// Conversion crates
pub mod convert_to_car_bomb_crate_collide;
pub mod convert_to_hijacked_vehicle_crate_collide;

// Sabotage crates
pub mod sabotage_command_center_crate_collide;
pub mod sabotage_fake_building;
pub mod sabotage_fake_building_crate_collide;
pub mod sabotage_internet_center_crate_collide;
pub mod sabotage_military_factory_crate_collide;
pub mod sabotage_power_plant_crate_collide;
pub mod sabotage_superweapon_crate_collide;
pub mod sabotage_supply_center_crate_collide;
pub mod sabotage_supply_dropzone_crate_collide;

pub use self::crate_collide::*;

use super::*;
pub use super::{CollisionError, Coord3D, GameObject};
use game_engine::common::ini::INIError;
use game_engine::common::rts::science::{get_science_store, SCIENCE_INVALID};
use std::sync::{Arc, RwLock};

/// Marker trait for crate collide modules (mirrors C++ interface hierarchy).
pub trait CrateCollideModule: CollideModule {
    fn is_valid_to_execute(&self, other: Arc<RwLock<Object>>) -> Result<bool, GameError>;

    fn execute_crate_behavior(&mut self, other: Arc<RwLock<Object>>) -> Result<bool, GameError>;

    fn is_sabotage_building_crate_collide(&self) -> bool {
        false
    }

    fn is_salvage_crate_collide(&self) -> bool {
        false
    }

    fn is_hijacked_vehicle_crate_collide(&self) -> bool {
        false
    }
}

/// Audio event for crate pickup sounds.
#[derive(Debug, Clone)]
pub struct AudioEvent {
    pub object_id: ObjectId,
    pub sound_type: String,
}

impl AudioEvent {
    pub fn new(object_id: ObjectId, sound_type: &str) -> Self {
        Self {
            object_id,
            sound_type: sound_type.to_string(),
        }
    }
}

pub(crate) fn parse_crate_pickup_science(
    data: &mut CrateCollideModuleData,
    science_name: &str,
) -> Result<(), INIError> {
    let science = get_science_store()
        .map(|store| store.get_science_from_internal_name(science_name))
        .unwrap_or(SCIENCE_INVALID);
    if science == SCIENCE_INVALID {
        return Err(INIError::InvalidData);
    }
    data.pickup_science = science as crate::common::science::ScienceType;
    Ok(())
}

pub(crate) fn format_cpp_integer_template(template: &str, amount: u32) -> Option<String> {
    let amount = amount.to_string();
    let mut output = String::with_capacity(template.len() + amount.len());
    let mut chars = template.chars().peekable();
    let mut replaced = false;

    while let Some(ch) = chars.next() {
        if ch != '%' {
            output.push(ch);
            continue;
        }

        if matches!(chars.peek(), Some('%')) {
            chars.next();
            output.push('%');
            continue;
        }

        let mut specifier = String::from("%");
        while let Some(next) = chars.next() {
            specifier.push(next);
            if matches!(next, 'd' | 'i' | 'u') {
                output.push_str(&amount);
                replaced = true;
                break;
            }
            if !matches!(next, '-' | '+' | ' ' | '#' | '0' | '1'..='9' | '.') {
                output.push_str(&specifier);
                break;
            }
        }
    }

    replaced.then_some(output)
}

pub(crate) fn format_cash_template(template: &str, amount: u32, fallback_prefix: &str) -> String {
    format_cpp_integer_template(template, amount)
        .unwrap_or_else(|| format!("{fallback_prefix}${amount}"))
}

pub(crate) fn format_add_cash(amount: u32) -> String {
    format_cash_template(
        &crate::helpers::TheGameText::fetch("GUI:AddCash"),
        amount,
        "+",
    )
}

pub(crate) fn format_lose_cash(amount: u32) -> String {
    format_cash_template(
        &crate::helpers::TheGameText::fetch("GUI:LoseCash"),
        amount,
        "-",
    )
}

/// Sabotage victim types for feedback effects.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SabotageVictimType {
    Generic = 0,
    CommandCenter,
    FakeBuilding,
    InternetCenter,
    MilitaryFactory,
    PowerPlant,
    Superweapon,
    SupplyCenter,
    DropZone,
}
