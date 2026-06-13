//! INI parsing for DamageFX definitions
//!
//! This module handles parsing DamageFX entries from INI files.
//! DamageFX describes how objects react to taking damage (audio/visual effects).
//!
//! Author: Steven Johnson, November 2001
//! Rust port: 2025

use once_cell::sync::OnceCell;
use std::collections::HashMap;
use std::sync::{RwLock, RwLockReadGuard, RwLockWriteGuard};

use crate::common::audio::game_audio::{get_global_audio_manager, initialize_global_audio_manager};
use crate::common::audio::AudioEventRts;
use crate::common::ini::ini::{FieldParse, INIError, INIResult, INI};
use crate::common::ini::ini_fx_list::get_fx_list_store;

/// Damage types enum
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DamageType {
    Explosion = 0,
    Crush = 1,
    ArmorPiercing = 2,
    SmallArms = 3,
    Gattling = 4,
    Radiation = 5,
    Flame = 6,
    Laser = 7,
    Sniper = 8,
    Poison = 9,
    Healing = 10,
    Unresistable = 11,
    Water = 12,
    Deploy = 13,
    Surrender = 14,
    Hack = 15,
    KillPilot = 16,
    Penalty = 17,
    Falling = 18,
    Melee = 19,
    Disarm = 20,
    HazardCleanup = 21,
    ParticleBeam = 22,
    Toppling = 23,
    InfantryMissile = 24,
    AuroraBomb = 25,
    LandMine = 26,
    JetMissiles = 27,
    StealthJetMissiles = 28,
    MolotovCocktail = 29,
    ComancheVulcan = 30,
    SubdualMissile = 31,
    SubdualVehicle = 32,
    SubdualBuilding = 33,
    SubdualUnresistable = 34,
    Microwave = 35,
    KillGarrisoned = 36,
    Status = 37,
}

/// Level count for different damage intensities
pub const LEVEL_COUNT: usize = 4;
pub const DAMAGE_NUM_TYPES: usize = 38;

/// FX list reference (stores FX list name for lazy resolution against FXListStore).
pub type ConstFXListPtr = Option<String>;

/// Object trait for damage source/victim
pub trait Object {
    fn get_name(&self) -> &str;
    fn get_id(&self) -> u32;
    fn get_veterancy_level(&self) -> usize {
        0
    }
}

/// Individual damage FX configuration for a specific damage type and level
#[derive(Debug, Clone)]
pub struct DamageFXEntry {
    /// Damage threshold for using major FX instead of minor FX
    pub amount_for_major_fx: f32,
    /// Major damage FX list to execute
    pub major_damage_fx_list: ConstFXListPtr,
    /// Minor damage FX list to execute
    pub minor_damage_fx_list: ConstFXListPtr,
    /// Throttle time to prevent FX spam
    pub damage_fx_throttle_time: u32,
}

impl Default for DamageFXEntry {
    fn default() -> Self {
        Self {
            amount_for_major_fx: 0.0,
            major_damage_fx_list: None,
            minor_damage_fx_list: None,
            damage_fx_throttle_time: 0,
        }
    }
}

/// A DamageFX object describes how an object reacts to taking damage
///
/// Every unit with a Body module has a DamageFX object. When it receives damage,
/// it asks its DamageFX module to produce an appropriate a/v effect, which can
/// vary by type of damage and amount ("minor" or "major").
///
/// Notes:
/// - Every damage type can have a "minor" and/or "major" effect
/// - If damage exceeds threshold or no "minor" effect exists, major effect is used
/// - DamageFX is shared between multiple units; there should be only one instance
/// - All methods are immutable to enforce thread safety
#[derive(Debug, Clone)]
pub struct DamageFX {
    /// FX entries for each damage type and level
    dfx: [[DamageFXEntry; LEVEL_COUNT]; DAMAGE_NUM_TYPES],
}

impl Default for DamageFX {
    fn default() -> Self {
        Self {
            dfx: std::array::from_fn(|_| std::array::from_fn(|_| DamageFXEntry::default())),
        }
    }
}

impl DamageFX {
    /// Create a new DamageFX instance
    pub fn new() -> Self {
        Self::default()
    }

    /// Clear all damage FX data
    pub fn clear(&mut self) {
        self.dfx = std::array::from_fn(|_| std::array::from_fn(|_| DamageFXEntry::default()));
    }

    /// Execute damage FX for specified damage type and amount
    ///
    /// # Arguments
    /// * `damage_type` - Type of damage taken
    /// * `damage_amount` - Amount of damage
    /// * `source` - Optional damage source object
    /// * `victim` - Optional victim object
    pub fn do_damage_fx(
        &self,
        damage_type: DamageType,
        damage_amount: f32,
        source: Option<&dyn Object>,
        victim: Option<&dyn Object>,
    ) {
        if let Some(fx_list) = self.get_damage_fx_list(damage_type, damage_amount, source) {
            self.execute_fx_list(&fx_list, source, victim);
        }
    }

    fn execute_fx_list(
        &self,
        fx_list_name: &str,
        source: Option<&dyn Object>,
        victim: Option<&dyn Object>,
    ) {
        let store = get_fx_list_store();
        let Some(fx_list) = store.find_fx_list(fx_list_name) else {
            return;
        };

        for nugget in &fx_list.nuggets {
            match nugget {
                crate::common::ini::ini_fx_list::FXNugget::Sound { name } => {
                    self.play_sound_nugget(name, source, victim);
                }
                _ => {
                    // Other nugget types are ignored until their subsystems are wired.
                }
            }
        }
    }

    fn play_sound_nugget(
        &self,
        sound_name: &str,
        source: Option<&dyn Object>,
        victim: Option<&dyn Object>,
    ) {
        if sound_name.is_empty() || sound_name.eq_ignore_ascii_case("NoSound") {
            return;
        }

        let mut event = AudioEventRts::with_event_name(sound_name);
        if let Some(target) = victim.or(source) {
            event.set_object_id(target.get_id());
        }

        let manager = get_global_audio_manager().unwrap_or_else(initialize_global_audio_manager);
        let mut manager = manager.lock().expect("audio manager mutex poisoned");
        if let Some(info) = manager
            .find_audio_event_info(event.get_event_name())
            .or_else(|| manager.new_audio_event_info(event.get_event_name().to_string()))
        {
            event.set_audio_event_info(info.clone());
            event.set_volume(info.volume);
        }
        let _ = manager.add_audio_event(&event);
    }

    /// Get throttle time for damage FX
    pub fn get_damage_fx_throttle_time(
        &self,
        damage_type: DamageType,
        source: Option<&dyn Object>,
    ) -> u32 {
        let type_index = self.damage_type_to_index(damage_type);
        if type_index < DAMAGE_NUM_TYPES {
            let level_index = source
                .map(|obj| obj.get_veterancy_level())
                .unwrap_or(0)
                .min(LEVEL_COUNT.saturating_sub(1));
            self.dfx[type_index][level_index].damage_fx_throttle_time
        } else {
            0
        }
    }

    /// Get appropriate FX list for damage type and amount
    fn get_damage_fx_list(
        &self,
        damage_type: DamageType,
        damage_amount: f32,
        source: Option<&dyn Object>,
    ) -> ConstFXListPtr {
        if damage_amount == 0.0 {
            return None;
        }

        let type_index = self.damage_type_to_index(damage_type);
        if type_index >= DAMAGE_NUM_TYPES {
            return None;
        }

        let level_index = source
            .map(|obj| obj.get_veterancy_level())
            .unwrap_or(0)
            .min(LEVEL_COUNT.saturating_sub(1));
        let entry = &self.dfx[type_index][level_index];

        // Choose major or minor FX based on damage amount
        if damage_amount >= entry.amount_for_major_fx {
            entry.major_damage_fx_list.clone()
        } else {
            entry.minor_damage_fx_list.clone()
        }
    }

    /// Convert damage type to array index
    fn damage_type_to_index(&self, damage_type: DamageType) -> usize {
        damage_type as usize
    }

    /// Parse damage FX entry from INI.
    pub fn parse_from_ini(&mut self, ini: &mut INI) -> Result<(), String> {
        ini.init_from_ini_with_fields(self, FIELD_PARSE_TABLE)
            .map_err(|error| error.to_string())
    }
}

const DAMAGE_TYPE_NAMES: &[(&str, DamageType)] = &[
    ("EXPLOSION", DamageType::Explosion),
    ("CRUSH", DamageType::Crush),
    ("ARMOR_PIERCING", DamageType::ArmorPiercing),
    ("SMALL_ARMS", DamageType::SmallArms),
    ("GATTLING", DamageType::Gattling),
    ("RADIATION", DamageType::Radiation),
    ("FLAME", DamageType::Flame),
    ("LASER", DamageType::Laser),
    ("SNIPER", DamageType::Sniper),
    ("POISON", DamageType::Poison),
    ("HEALING", DamageType::Healing),
    ("UNRESISTABLE", DamageType::Unresistable),
    ("WATER", DamageType::Water),
    ("DEPLOY", DamageType::Deploy),
    ("SURRENDER", DamageType::Surrender),
    ("HACK", DamageType::Hack),
    ("KILLPILOT", DamageType::KillPilot),
    ("KILL_PILOT", DamageType::KillPilot),
    ("PENALTY", DamageType::Penalty),
    ("FALLING", DamageType::Falling),
    ("MELEE", DamageType::Melee),
    ("DISARM", DamageType::Disarm),
    ("HAZARD_CLEANUP", DamageType::HazardCleanup),
    ("PARTICLE_BEAM", DamageType::ParticleBeam),
    ("TOPPLING", DamageType::Toppling),
    ("INFANTRY_MISSILE", DamageType::InfantryMissile),
    ("AURORA_BOMB", DamageType::AuroraBomb),
    ("LAND_MINE", DamageType::LandMine),
    ("JET_MISSILES", DamageType::JetMissiles),
    ("STEALTHJET_MISSILES", DamageType::StealthJetMissiles),
    ("MOLOTOV_COCKTAIL", DamageType::MolotovCocktail),
    ("COMANCHE_VULCAN", DamageType::ComancheVulcan),
    ("SUBDUAL_MISSILE", DamageType::SubdualMissile),
    ("SUBDUAL_VEHICLE", DamageType::SubdualVehicle),
    ("SUBDUAL_BUILDING", DamageType::SubdualBuilding),
    ("SUBDUAL_UNRESISTABLE", DamageType::SubdualUnresistable),
    ("MICROWAVE", DamageType::Microwave),
    ("KILL_GARRISONED", DamageType::KillGarrisoned),
    ("STATUS", DamageType::Status),
];

const VETERANCY_NAMES: &[&str] = &["REGULAR", "VETERAN", "ELITE", "HEROIC"];

fn parse_damage_index(name: &str) -> Option<usize> {
    let name = name.trim();
    for (candidate, damage_type) in DAMAGE_TYPE_NAMES {
        if name.eq_ignore_ascii_case(candidate) {
            return Some(*damage_type as usize);
        }
    }
    None
}

fn parse_veterancy_index(name: &str) -> Option<usize> {
    for (index, candidate) in VETERANCY_NAMES.iter().enumerate() {
        if name.eq_ignore_ascii_case(candidate) {
            return Some(index.min(LEVEL_COUNT.saturating_sub(1)));
        }
    }
    None
}

fn parse_common_stuff(
    args: &[&str],
    expect_veterancy: bool,
) -> INIResult<(usize, usize, usize, usize, usize)> {
    let mut idx = 0;
    let (vet_first, vet_last) = if expect_veterancy {
        let vet_name = args.get(idx).ok_or(INIError::InvalidData)?;
        idx += 1;
        let vet = parse_veterancy_index(vet_name).ok_or(INIError::InvalidData)?;
        (vet, vet)
    } else {
        (0, LEVEL_COUNT.saturating_sub(1))
    };

    let damage_name = args.get(idx).ok_or(INIError::InvalidData)?;
    idx += 1;

    let (damage_first, damage_last) = if damage_name.eq_ignore_ascii_case("Default") {
        (0, DAMAGE_NUM_TYPES.saturating_sub(1))
    } else {
        let damage = parse_damage_index(damage_name).ok_or(INIError::InvalidData)?;
        (damage, damage)
    };

    Ok((vet_first, vet_last, damage_first, damage_last, idx))
}

fn parse_fx_list_name(args: &[&str]) -> INIResult<Option<String>> {
    if args.is_empty() {
        return Err(INIError::InvalidData);
    }

    let joined = args.join(" ");
    let name = INI::parse_ascii_string(&joined)?;
    if name.eq_ignore_ascii_case("None") || name.eq_ignore_ascii_case("NULL") {
        return Ok(None);
    }
    Ok(Some(name))
}

fn parse_amount_common(
    damage_fx: &mut DamageFX,
    args: &[&str],
    expect_veterancy: bool,
) -> INIResult<()> {
    let (vet_first, vet_last, damage_first, damage_last, idx) =
        parse_common_stuff(args, expect_veterancy)?;
    let value = args.get(idx).ok_or(INIError::InvalidData)?;
    let amount = value.parse::<f32>().map_err(|_| INIError::InvalidData)?;

    for damage_index in damage_first..=damage_last {
        for vet_index in vet_first..=vet_last {
            damage_fx.dfx[damage_index][vet_index].amount_for_major_fx = amount;
        }
    }
    Ok(())
}

fn parse_major_common(
    damage_fx: &mut DamageFX,
    args: &[&str],
    expect_veterancy: bool,
) -> INIResult<()> {
    let (vet_first, vet_last, damage_first, damage_last, idx) =
        parse_common_stuff(args, expect_veterancy)?;
    let fx_list = parse_fx_list_name(&args[idx..])?;

    for damage_index in damage_first..=damage_last {
        for vet_index in vet_first..=vet_last {
            damage_fx.dfx[damage_index][vet_index].major_damage_fx_list = fx_list.clone();
        }
    }
    Ok(())
}

fn parse_minor_common(
    damage_fx: &mut DamageFX,
    args: &[&str],
    expect_veterancy: bool,
) -> INIResult<()> {
    let (vet_first, vet_last, damage_first, damage_last, idx) =
        parse_common_stuff(args, expect_veterancy)?;
    let fx_list = parse_fx_list_name(&args[idx..])?;

    for damage_index in damage_first..=damage_last {
        for vet_index in vet_first..=vet_last {
            damage_fx.dfx[damage_index][vet_index].minor_damage_fx_list = fx_list.clone();
        }
    }
    Ok(())
}

fn parse_time_common(
    damage_fx: &mut DamageFX,
    args: &[&str],
    expect_veterancy: bool,
) -> INIResult<()> {
    let (vet_first, vet_last, damage_first, damage_last, idx) =
        parse_common_stuff(args, expect_veterancy)?;
    let value = args.get(idx).ok_or(INIError::InvalidData)?;
    let throttle = INI::parse_unsigned_int(value)?;

    for damage_index in damage_first..=damage_last {
        for vet_index in vet_first..=vet_last {
            damage_fx.dfx[damage_index][vet_index].damage_fx_throttle_time = throttle;
        }
    }
    Ok(())
}

pub fn parse_amount(_ini: &mut INI, damage_fx: &mut DamageFX, args: &[&str]) -> INIResult<()> {
    parse_amount_common(damage_fx, args, false)
}

pub fn parse_major_fx_list(
    _ini: &mut INI,
    damage_fx: &mut DamageFX,
    args: &[&str],
) -> INIResult<()> {
    parse_major_common(damage_fx, args, false)
}

pub fn parse_minor_fx_list(
    _ini: &mut INI,
    damage_fx: &mut DamageFX,
    args: &[&str],
) -> INIResult<()> {
    parse_minor_common(damage_fx, args, false)
}

pub fn parse_time(_ini: &mut INI, damage_fx: &mut DamageFX, args: &[&str]) -> INIResult<()> {
    parse_time_common(damage_fx, args, false)
}

pub fn parse_veterancy_amount(
    _ini: &mut INI,
    damage_fx: &mut DamageFX,
    args: &[&str],
) -> INIResult<()> {
    parse_amount_common(damage_fx, args, true)
}

pub fn parse_veterancy_major_fx_list(
    _ini: &mut INI,
    damage_fx: &mut DamageFX,
    args: &[&str],
) -> INIResult<()> {
    parse_major_common(damage_fx, args, true)
}

pub fn parse_veterancy_minor_fx_list(
    _ini: &mut INI,
    damage_fx: &mut DamageFX,
    args: &[&str],
) -> INIResult<()> {
    parse_minor_common(damage_fx, args, true)
}

pub fn parse_veterancy_time(
    _ini: &mut INI,
    damage_fx: &mut DamageFX,
    args: &[&str],
) -> INIResult<()> {
    parse_time_common(damage_fx, args, true)
}

pub const FIELD_PARSE_TABLE: &[FieldParse<DamageFX>] = &[
    FieldParse {
        token: "AmountForMajorFX",
        parse: parse_amount,
    },
    FieldParse {
        token: "MajorFX",
        parse: parse_major_fx_list,
    },
    FieldParse {
        token: "MinorFX",
        parse: parse_minor_fx_list,
    },
    FieldParse {
        token: "ThrottleTime",
        parse: parse_time,
    },
    FieldParse {
        token: "VeterancyAmountForMajorFX",
        parse: parse_veterancy_amount,
    },
    FieldParse {
        token: "VeterancyMajorFX",
        parse: parse_veterancy_major_fx_list,
    },
    FieldParse {
        token: "VeterancyMinorFX",
        parse: parse_veterancy_minor_fx_list,
    },
    FieldParse {
        token: "VeterancyThrottleTime",
        parse: parse_veterancy_time,
    },
];

/// Store for managing all DamageFX instances in the game
///
/// The "store" used to hold all DamageFXs in existence. Usually used when creating
/// an Object (actually, a Body module), but can be used at any time after that.
/// It is explicitly OK to swap an Object's DamageFX out at any given time.
#[derive(Debug)]
pub struct DamageFXStore {
    /// Map of DamageFX instances by name
    damage_fx_map: HashMap<String, DamageFX>,
}

impl Default for DamageFXStore {
    fn default() -> Self {
        Self::new()
    }
}

impl DamageFXStore {
    /// Create a new DamageFXStore
    pub fn new() -> Self {
        Self {
            damage_fx_map: HashMap::new(),
        }
    }

    /// Initialize the damage FX store
    pub fn init(&mut self) {
        self.damage_fx_map.clear();
    }

    /// Reset the damage FX store
    pub fn reset(&mut self) {
        self.damage_fx_map.clear();
    }

    /// Update the damage FX store (called per frame)
    pub fn update(&mut self) {
        // Update logic here if needed
    }

    /// Find DamageFX by name
    ///
    /// # Arguments
    /// * `name` - Name of the DamageFX to find
    ///
    /// # Returns
    /// Reference to the DamageFX if found, None otherwise
    pub fn find_damage_fx(&self, name: &str) -> Option<&DamageFX> {
        self.damage_fx_map.get(name)
    }

    /// Add or update a DamageFX in the store
    pub fn add_damage_fx(&mut self, name: String, damage_fx: DamageFX) {
        self.damage_fx_map.insert(name, damage_fx);
    }

    /// Remove a DamageFX from the store
    pub fn remove_damage_fx(&mut self, name: &str) -> Option<DamageFX> {
        self.damage_fx_map.remove(name)
    }

    /// Get all DamageFX names
    pub fn get_damage_fx_names(&self) -> Vec<&String> {
        self.damage_fx_map.keys().collect()
    }

    /// Parse DamageFX definition from INI
    pub fn parse_damage_fx_definition(ini: &mut INI) -> Result<(), String> {
        let tokens = ini.get_line_tokens();
        let name = tokens
            .iter()
            .skip(1)
            .find(|token| **token != "=")
            .ok_or_else(|| "Expected DamageFX name".to_string())?
            .to_string();

        let mut damage_fx = DamageFX::new();
        damage_fx.clear();
        damage_fx.parse_from_ini(ini)?;

        if DAMAGE_FX_STORE.get().is_none() {
            init_global_damage_fx_store();
        }

        if let Some(mut store) = get_damage_fx_store_mut() {
            store.add_damage_fx(name, damage_fx);
        }

        Ok(())
    }
}

/// Global DamageFXStore instance, initialized via `init_global_damage_fx_store`.
static DAMAGE_FX_STORE: OnceCell<RwLock<DamageFXStore>> = OnceCell::new();

/// Initialize the global damage FX store
pub fn init_global_damage_fx_store() {
    if DAMAGE_FX_STORE.get().is_none() {
        let store = DamageFXStore::new();
        let _ = DAMAGE_FX_STORE.set(RwLock::new(store));
    }

    if let Some(store) = DAMAGE_FX_STORE.get() {
        if let Ok(mut guard) = store.write() {
            guard.init();
        }
    }
}

/// Get reference to global damage FX store
pub fn get_damage_fx_store() -> Option<RwLockReadGuard<'static, DamageFXStore>> {
    DAMAGE_FX_STORE
        .get()
        .map(|store| store.read().expect("DamageFXStore poisoned"))
}

/// Get mutable reference to global damage FX store
pub fn get_damage_fx_store_mut() -> Option<RwLockWriteGuard<'static, DamageFXStore>> {
    DAMAGE_FX_STORE
        .get()
        .map(|store| store.write().expect("DamageFXStore poisoned"))
}

/// INI parsing function (matches C++ interface)
///
/// This is the main entry point for parsing DamageFX definitions from INI files
pub fn parse_damage_fx_definition(ini: &mut INI) -> Result<(), String> {
    DamageFXStore::parse_damage_fx_definition(ini)
}

#[cfg(test)]
mod tests {
    use super::*;

    struct TestObject {
        name: String,
        id: u32,
    }

    impl Object for TestObject {
        fn get_name(&self) -> &str {
            &self.name
        }

        fn get_id(&self) -> u32 {
            self.id
        }
    }

    #[test]
    fn test_damage_fx_creation() {
        let damage_fx = DamageFX::new();
        assert_eq!(
            damage_fx.get_damage_fx_throttle_time(DamageType::Explosion, None),
            0
        );
    }

    #[test]
    fn test_damage_fx_store() {
        let mut store = DamageFXStore::new();
        store.init();

        let damage_fx = DamageFX::new();
        store.add_damage_fx("test_fx".to_string(), damage_fx);

        assert!(store.find_damage_fx("test_fx").is_some());
        assert!(store.find_damage_fx("nonexistent").is_none());

        let names = store.get_damage_fx_names();
        assert_eq!(names.len(), 1);
        assert_eq!(names[0], "test_fx");
    }

    #[test]
    fn test_damage_type_mapping() {
        let damage_fx = DamageFX::new();

        // Test all damage types map to valid indices
        assert!(damage_fx.damage_type_to_index(DamageType::Explosion) < DAMAGE_NUM_TYPES);
        assert!(damage_fx.damage_type_to_index(DamageType::ArmorPiercing) < DAMAGE_NUM_TYPES);
        assert!(damage_fx.damage_type_to_index(DamageType::Flame) < DAMAGE_NUM_TYPES);
        assert!(damage_fx.damage_type_to_index(DamageType::Laser) < DAMAGE_NUM_TYPES);
        assert!(damage_fx.damage_type_to_index(DamageType::Sniper) < DAMAGE_NUM_TYPES);
        assert!(damage_fx.damage_type_to_index(DamageType::Poison) < DAMAGE_NUM_TYPES);
        assert!(damage_fx.damage_type_to_index(DamageType::Healing) < DAMAGE_NUM_TYPES);
        assert!(damage_fx.damage_type_to_index(DamageType::Unresistable) < DAMAGE_NUM_TYPES);
    }

    #[test]
    fn zero_damage_returns_no_fx_like_cpp() {
        let mut damage_fx = DamageFX::new();
        let entry = &mut damage_fx.dfx[DamageType::Explosion as usize][0];
        entry.amount_for_major_fx = 0.0;
        entry.major_damage_fx_list = Some("MajorExplosionFX".to_string());
        entry.minor_damage_fx_list = Some("MinorExplosionFX".to_string());

        assert_eq!(
            damage_fx.get_damage_fx_list(DamageType::Explosion, 0.0, None),
            None
        );
    }

    #[test]
    fn test_do_damage_fx() {
        let damage_fx = DamageFX::new();
        let source = TestObject {
            name: "Attacker".to_string(),
            id: 1,
        };
        let victim = TestObject {
            name: "Victim".to_string(),
            id: 2,
        };

        // This should not panic
        damage_fx.do_damage_fx(DamageType::Explosion, 50.0, Some(&source), Some(&victim));
    }

    #[test]
    fn test_global_store_init() {
        init_global_damage_fx_store();

        assert!(get_damage_fx_store().is_some());

        if let Some(mut store) = get_damage_fx_store_mut() {
            let damage_fx = DamageFX::new();
            store.add_damage_fx("global_test".to_string(), damage_fx);
        }

        if let Some(store) = get_damage_fx_store() {
            assert!(store.find_damage_fx("global_test").is_some());
        }
    }
}
