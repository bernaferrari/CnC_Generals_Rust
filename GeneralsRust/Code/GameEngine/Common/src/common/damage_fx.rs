////////////////////////////////////////////////////////////////////////////////
//                                                                            //
//  (c) 2001-2003 Electronic Arts Inc.                                       //
//                                                                            //
////////////////////////////////////////////////////////////////////////////////

// FILE: damage_fx.rs ///////////////////////////////////////////////////////////////////////////////
// Author: Steven Johnson, November 2001
// Desc:   DamageFX descriptions
///////////////////////////////////////////////////////////////////////////////////////////////////

use once_cell::sync::OnceCell;
use std::collections::HashMap;
use std::sync::Mutex;

use crate::common::name_key_generator::{NameKeyGenerator, NameKeyType};

/// Number of damage types (matches C++ Damage.h)
pub const DAMAGE_NUM_TYPES: usize = 38;

/// Veterancy levels
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VeterancyLevel {
    Regular = 0,
    Veteran = 1,
    Elite = 2,
    Hero = 3,
}

pub const LEVEL_FIRST: VeterancyLevel = VeterancyLevel::Regular;
pub const LEVEL_LAST: VeterancyLevel = VeterancyLevel::Hero;
pub const LEVEL_COUNT: usize = 4;

/// Damage types
pub type DamageType = usize;

/// Forward declarations
pub trait Object {
    fn get_veterancy_level(&self) -> VeterancyLevel;
}

pub trait FXList: Send + Sync {
    fn do_fx_obj(&self, victim: Option<&dyn Object>, source: Option<&dyn Object>);
}

pub type ConstFxListPtr = Option<Box<dyn FXList>>;

/// Damage FX structure for a specific damage type and veterancy level
struct DamageFXData {
    /// If damage done is >= this, use major fx
    amount_for_major_fx: f32,
    /// Major damage FX list
    major_damage_fx_list: ConstFxListPtr,
    /// Minor damage FX list  
    minor_damage_fx_list: ConstFxListPtr,
    /// Throttle time for damage FX
    damage_fx_throttle_time: u32,
}

impl Clone for DamageFXData {
    fn clone(&self) -> Self {
        Self {
            amount_for_major_fx: self.amount_for_major_fx,
            // Note: FXList objects cannot be cloned, so we set to None
            major_damage_fx_list: None,
            minor_damage_fx_list: None,
            damage_fx_throttle_time: self.damage_fx_throttle_time,
        }
    }
}

impl Default for DamageFXData {
    fn default() -> Self {
        Self {
            amount_for_major_fx: 0.0,
            major_damage_fx_list: None,
            minor_damage_fx_list: None,
            damage_fx_throttle_time: 0,
        }
    }
}

impl DamageFXData {
    fn clear(&mut self) {
        self.amount_for_major_fx = 0.0;
        self.major_damage_fx_list = None;
        self.minor_damage_fx_list = None;
        self.damage_fx_throttle_time = 0;
    }
}

/// DamageFX class - describes how an object reacts to taking damage
#[derive(Clone)]
pub struct DamageFX {
    /// Damage FX data indexed by [damage_type][veterancy_level]
    dfx: [[DamageFXData; LEVEL_COUNT]; DAMAGE_NUM_TYPES],
}

impl Default for DamageFX {
    fn default() -> Self {
        Self::new()
    }
}

impl DamageFX {
    /// Create a new DamageFX
    pub fn new() -> Self {
        Self {
            dfx: std::array::from_fn(|_| std::array::from_fn(|_| DamageFXData::default())),
        }
    }

    /// Clear all damage FX data
    pub fn clear(&mut self) {
        for dt in 0..DAMAGE_NUM_TYPES {
            for v in 0..LEVEL_COUNT {
                self.dfx[dt][v].clear();
            }
        }
    }

    /// Get damage FX throttle time
    pub fn get_damage_fx_throttle_time(
        &self,
        damage_type: DamageType,
        source: Option<&dyn Object>,
    ) -> u32 {
        let vet_level =
            source.map_or(VeterancyLevel::Regular, |s| s.get_veterancy_level()) as usize;
        self.dfx[damage_type][vet_level].damage_fx_throttle_time
    }

    /// Execute damage FX
    pub fn do_damage_fx(
        &self,
        damage_type: DamageType,
        damage_amount: f32,
        source: Option<&dyn Object>,
        victim: Option<&dyn Object>,
    ) {
        if let Some(fx) = self.get_damage_fx_list(damage_type, damage_amount, source) {
            fx.do_fx_obj(victim, source);
        }
    }

    /// Get the appropriate FX list for the given damage
    fn get_damage_fx_list(
        &self,
        damage_type: DamageType,
        damage_amount: f32,
        source: Option<&dyn Object>,
    ) -> &ConstFxListPtr {
        // If damage is zero, never do damage fx
        if damage_amount == 0.0 {
            return &None;
        }

        let vet_level =
            source.map_or(VeterancyLevel::Regular, |s| s.get_veterancy_level()) as usize;
        let dfx_data = &self.dfx[damage_type][vet_level];

        if damage_amount >= dfx_data.amount_for_major_fx {
            &dfx_data.major_damage_fx_list
        } else {
            &dfx_data.minor_damage_fx_list
        }
    }

    /// Set amount for major FX for a damage type and veterancy level
    pub fn set_amount_for_major_fx(
        &mut self,
        damage_type: DamageType,
        vet_level: VeterancyLevel,
        amount: f32,
    ) {
        self.dfx[damage_type][vet_level as usize].amount_for_major_fx = amount;
    }

    /// Set major damage FX list
    pub fn set_major_damage_fx_list(
        &mut self,
        damage_type: DamageType,
        vet_level: VeterancyLevel,
        fx_list: ConstFxListPtr,
    ) {
        self.dfx[damage_type][vet_level as usize].major_damage_fx_list = fx_list;
    }

    /// Set minor damage FX list
    pub fn set_minor_damage_fx_list(
        &mut self,
        damage_type: DamageType,
        vet_level: VeterancyLevel,
        fx_list: ConstFxListPtr,
    ) {
        self.dfx[damage_type][vet_level as usize].minor_damage_fx_list = fx_list;
    }

    /// Set throttle time
    pub fn set_throttle_time(
        &mut self,
        damage_type: DamageType,
        vet_level: VeterancyLevel,
        time: u32,
    ) {
        self.dfx[damage_type][vet_level as usize].damage_fx_throttle_time = time;
    }
}

/// DamageFX Store - holds all DamageFX instances
pub struct DamageFXStore {
    dfx_map: HashMap<NameKeyType, DamageFX>,
}

impl Default for DamageFXStore {
    fn default() -> Self {
        Self::new()
    }
}

impl DamageFXStore {
    /// Create a new DamageFX store
    pub fn new() -> Self {
        Self {
            dfx_map: HashMap::new(),
        }
    }

    /// Initialize the store
    pub fn init(&mut self) {
        // Initialization logic would go here
    }

    /// Reset the store
    pub fn reset(&mut self) {
        self.dfx_map.clear();
    }

    /// Update the store (per-frame update)
    pub fn update(&mut self) {
        // Update logic would go here
    }

    /// Find a DamageFX by name
    pub fn find_damage_fx(&self, name: &str) -> Option<&DamageFX> {
        let name_key = self.name_to_key(name);
        self.dfx_map.get(&name_key)
    }

    /// Find a mutable DamageFX by name
    pub fn find_damage_fx_mut(&mut self, name: &str) -> Option<&mut DamageFX> {
        let name_key = self.name_to_key(name);
        self.dfx_map.get_mut(&name_key)
    }

    /// Add or update a DamageFX
    pub fn add_damage_fx(&mut self, name: &str, damage_fx: DamageFX) {
        let name_key = self.name_to_key(name);
        self.dfx_map.insert(name_key, damage_fx);
    }

    /// Convert name to key using the global name-key generator.
    fn name_to_key(&self, name: &str) -> NameKeyType {
        NameKeyGenerator::name_to_key(name)
    }
}

/// Global damage FX store instance
static DAMAGE_FX_STORE: OnceCell<Mutex<DamageFXStore>> = OnceCell::new();

/// Initialize the global damage FX store
pub fn initialize_damage_fx_store() {
    let store = DamageFXStore::new();
    if DAMAGE_FX_STORE.set(Mutex::new(store)).is_err() {
        if let Some(existing) = DAMAGE_FX_STORE.get() {
            let mut guard = existing.lock().expect("DamageFX store mutex poisoned");
            *guard = DamageFXStore::new();
        }
    }
}

/// Get reference to the global damage FX store
pub fn get_damage_fx_store() -> std::sync::MutexGuard<'static, DamageFXStore> {
    DAMAGE_FX_STORE
        .get()
        .expect("DamageFX store not initialized")
        .lock()
        .expect("DamageFX store mutex poisoned")
}

/// Get mutable reference to the global damage FX store
pub fn get_damage_fx_store_mut() -> std::sync::MutexGuard<'static, DamageFXStore> {
    get_damage_fx_store()
}

/// Damage type flags (placeholder - would be defined elsewhere)
pub struct DamageTypeFlags;

impl DamageTypeFlags {
    pub fn get_single_bit_from_name(name: &str) -> Option<u32> {
        match name.to_uppercase().as_str() {
            "EXPLOSION" => Some(0),
            "CRUSH" => Some(1),
            "ARMOR_PIERCING" => Some(2),
            "SMALL_ARMS" => Some(3),
            "GATTLING" => Some(4),
            "RADIATION" => Some(5),
            "FLAME" => Some(6),
            "LASER" => Some(7),
            "SNIPER" => Some(8),
            "POISON" => Some(9),
            "HEALING" => Some(10),
            "UNRESISTABLE" => Some(11),
            "WATER" => Some(12),
            "DEPLOY" => Some(13),
            "SURRENDER" => Some(14),
            "HACK" => Some(15),
            "KILLPILOT" => Some(16),
            "PENALTY" => Some(17),
            "FALLING" => Some(18),
            "MELEE" => Some(19),
            "DISARM" => Some(20),
            "HAZARD_CLEANUP" => Some(21),
            "PARTICLE_BEAM" => Some(22),
            "TOPPLING" => Some(23),
            "INFANTRY_MISSILE" => Some(24),
            "AURORA_BOMB" => Some(25),
            "LAND_MINE" => Some(26),
            "JET_MISSILES" => Some(27),
            "STEALTHJET_MISSILES" => Some(28),
            "MOLOTOV_COCKTAIL" => Some(29),
            "COMANCHE_VULCAN" => Some(30),
            "SUBDUAL_MISSILE" => Some(31),
            "SUBDUAL_VEHICLE" => Some(32),
            "SUBDUAL_BUILDING" => Some(33),
            "SUBDUAL_UNRESISTABLE" => Some(34),
            "MICROWAVE" => Some(35),
            "KILL_GARRISONED" => Some(36),
            "STATUS" => Some(37),
            _ => None,
        }
    }
}

/// C++ parity: Initialize global damage type flags.
/// In C++, this sets DAMAGE_TYPE_FLAGS_ALL via SET_ALL_DAMAGE_TYPE_BITS macro.
/// In Rust, the all-flag is a compile-time constant; this call is a no-op but preserved
/// for API parity and potential future mutable state.
pub fn init_damage_type_flags() {
    // No-op: Damage type flags are constants in Rust.
}

/// Veterancy level names for parsing
pub const VETERANCY_NAMES: &[&str] = &["REGULAR", "VETERAN", "ELITE", "HERO"];

/// Parse veterancy level from name
pub fn parse_veterancy_level(name: &str) -> Option<VeterancyLevel> {
    match name.to_uppercase().as_str() {
        "REGULAR" => Some(VeterancyLevel::Regular),
        "VETERAN" => Some(VeterancyLevel::Veteran),
        "ELITE" => Some(VeterancyLevel::Elite),
        "HERO" => Some(VeterancyLevel::Hero),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn damage_fx_store_uses_name_keys() {
        NameKeyGenerator::reset();
        let mut store = DamageFXStore::new();

        let mut fx = DamageFX::new();
        fx.set_throttle_time(0, VeterancyLevel::Regular, 3);
        store.add_damage_fx("TankExplosion", fx.clone());

        assert!(store.find_damage_fx("TankExplosion").is_some());
        assert_eq!(
            store
                .find_damage_fx("TankExplosion")
                .unwrap()
                .get_damage_fx_throttle_time(0, None),
            3
        );

        // NameKeyGenerator::name_to_key is case sensitive; a different case should not match.
        assert!(store.find_damage_fx("tankexplosion").is_none());

        store.reset();
        assert!(store.find_damage_fx("TankExplosion").is_none());
    }
}
