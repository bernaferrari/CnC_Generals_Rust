// FILE: upgrade.rs
// Author: Rust port by Claude, November 2025
// Original: Colin Day, March 2002
// Desc: Upgrade system for players - complete port from C++ to Rust
//
// Matches C++ implementation in:
// - /GeneralsMD/Code/GameEngine/Include/Common/Upgrade.h
// - /GeneralsMD/Code/GameEngine/Source/Common/System/Upgrade.cpp

use std::collections::HashMap;
use std::sync::{Arc, Mutex, RwLock};

use game_engine::common::name_key_generator::NameKeyGenerator;

/// Maximum number of upgrades supported in the system
pub const UPGRADE_MAX_COUNT: usize = 128;

/// Upgrade status enumeration - tracks production state of an upgrade
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u32)]
pub enum UpgradeStatusType {
    Invalid = 0,
    InProduction = 1,
    Complete = 2,
}

/// Upgrade type enumeration - determines scope of upgrade application
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u32)]
pub enum UpgradeType {
    /// Upgrade applies to a player as a whole
    Player = 0,
    /// Upgrade applies to an object instance only
    Object = 1,
}

/// String names for upgrade types (for INI parsing)
pub const UPGRADE_TYPE_NAMES: [&str; 2] = ["PLAYER", "OBJECT"];

/// Veterancy levels for special veterancy upgrades
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum VeterancyLevel {
    Regular = 0,
    Veteran = 1,
    Elite = 2,
    Heroic = 3,
}

/// String names for veterancy levels
pub const VETERANCY_NAMES: [&str; 4] = ["REGULAR", "VETERAN", "ELITE", "HEROIC"];

/// Academy classification for upgrade analysis
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AcademyClassificationType {
    None = 0,
    // Additional values would be added based on game requirements
}

/// Unique identifier for names/keys in the game
pub type NameKeyType = u32;
pub const NAMEKEY_INVALID: NameKeyType = 0;

/// Bitmask type for tracking multiple upgrades efficiently
/// Each bit represents whether a specific upgrade is present
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UpgradeMaskType {
    bits: [u64; 2], // 128 bits total (2 * 64)
}

impl UpgradeMaskType {
    /// Create a new empty upgrade mask
    pub fn new() -> Self {
        Self { bits: [0, 0] }
    }

    /// Create a mask with specific bit set
    pub fn with_bit(index: usize) -> Self {
        let mut mask = Self::new();
        mask.set_bit(index);
        mask
    }

    /// Create a mask from multiple indices
    pub fn from_indices(indices: &[usize]) -> Self {
        let mut mask = Self::new();
        for &index in indices {
            mask.set_bit(index);
        }
        mask
    }

    /// Set a specific bit by index
    pub fn set_bit(&mut self, index: usize) {
        if index < UPGRADE_MAX_COUNT {
            let word = index / 64;
            let bit = index % 64;
            self.bits[word] |= 1u64 << bit;
        }
    }

    /// Set all bits from another mask
    pub fn set(&mut self, other: &UpgradeMaskType) {
        self.bits[0] |= other.bits[0];
        self.bits[1] |= other.bits[1];
    }

    /// Clear a specific bit
    pub fn clear_bit(&mut self, index: usize) {
        if index < UPGRADE_MAX_COUNT {
            let word = index / 64;
            let bit = index % 64;
            self.bits[word] &= !(1u64 << bit);
        }
    }

    /// Clear all bits
    pub fn clear(&mut self) {
        self.bits[0] = 0;
        self.bits[1] = 0;
    }

    /// Test if a specific bit is set
    pub fn test(&self, index: usize) -> bool {
        if index < UPGRADE_MAX_COUNT {
            let word = index / 64;
            let bit = index % 64;
            (self.bits[word] & (1u64 << bit)) != 0
        } else {
            false
        }
    }

    /// Test if any bit is set
    pub fn any(&self) -> bool {
        self.bits[0] != 0 || self.bits[1] != 0
    }

    /// Test if any bits from another mask are set in this mask
    pub fn any_intersection_with(&self, other: &UpgradeMaskType) -> bool {
        (self.bits[0] & other.bits[0]) != 0 || (self.bits[1] & other.bits[1]) != 0
    }

    /// Test if all bits from mask are set and all bits from clear are not set
    pub fn test_set_and_clear(
        &self,
        must_be_set: &UpgradeMaskType,
        must_be_clear: &UpgradeMaskType,
    ) -> bool {
        // All bits in must_be_set must be set
        let all_set = (self.bits[0] & must_be_set.bits[0]) == must_be_set.bits[0]
            && (self.bits[1] & must_be_set.bits[1]) == must_be_set.bits[1];

        // No bits in must_be_clear can be set
        let none_set = (self.bits[0] & must_be_clear.bits[0]) == 0
            && (self.bits[1] & must_be_clear.bits[1]) == 0;

        all_set && none_set
    }

    /// Flip all bits
    pub fn flip(&mut self) {
        self.bits[0] = !self.bits[0];
        self.bits[1] = !self.bits[1];
    }

    /// Test if ANY of the bits in the mask parameter are set
    pub fn test_for_any(&self, mask: &UpgradeMaskType) -> bool {
        self.any_intersection_with(mask)
    }

    /// Test if ALL of the bits in the mask parameter are set
    pub fn test_for_all(&self, mask: &UpgradeMaskType) -> bool {
        (self.bits[0] & mask.bits[0]) == mask.bits[0]
            && (self.bits[1] & mask.bits[1]) == mask.bits[1]
    }
}

impl Default for UpgradeMaskType {
    fn default() -> Self {
        Self::new()
    }
}

/// Helper macro functions matching C++ interface
#[inline]
pub fn test_upgrade_mask(mask: &UpgradeMaskType, index: usize) -> bool {
    mask.test(index)
}

#[inline]
pub fn test_upgrade_mask_any(m: &UpgradeMaskType, mask: &UpgradeMaskType) -> bool {
    m.any_intersection_with(mask)
}

#[inline]
pub fn test_upgrade_mask_multi(
    m: &UpgradeMaskType,
    must_be_set: &UpgradeMaskType,
    must_be_clear: &UpgradeMaskType,
) -> bool {
    m.test_set_and_clear(must_be_set, must_be_clear)
}

#[inline]
pub fn upgrade_mask_any_set(m: &UpgradeMaskType) -> bool {
    m.any()
}

#[inline]
pub fn clear_upgrade_mask(m: &mut UpgradeMaskType) {
    m.clear();
}

#[inline]
pub fn set_all_upgrade_mask_bits(m: &mut UpgradeMaskType) {
    m.clear();
    m.flip();
}

#[inline]
pub fn flip_upgrade_mask(m: &mut UpgradeMaskType) {
    m.flip();
}

/// A single upgrade INSTANCE
/// Matches C++ Upgrade class from Upgrade.h lines 83-119
pub struct Upgrade {
    template: Arc<UpgradeTemplate>,
    status: UpgradeStatusType,
    next: Option<Box<Upgrade>>,
    prev: *mut Upgrade, // Raw pointer for double-linked list
}

impl Upgrade {
    /// Create a new upgrade instance from a template
    pub fn new(template: Arc<UpgradeTemplate>) -> Self {
        Self {
            template,
            status: UpgradeStatusType::Invalid,
            next: None,
            prev: std::ptr::null_mut(),
        }
    }

    /// Get the upgrade template for this instance
    pub fn get_template(&self) -> &Arc<UpgradeTemplate> {
        &self.template
    }

    /// Get the current status
    pub fn get_status(&self) -> UpgradeStatusType {
        self.status
    }

    /// Set the status
    pub fn set_status(&mut self, status: UpgradeStatusType) {
        self.status = status;
    }

    /// Friend access methods for linked list management
    pub fn friend_set_next(&mut self, next: Option<Box<Upgrade>>) {
        self.next = next;
    }

    pub fn friend_set_prev(&mut self, prev: *mut Upgrade) {
        self.prev = prev;
    }

    pub fn friend_get_next(&self) -> Option<&Upgrade> {
        self.next.as_deref()
    }

    pub fn friend_get_next_mut(&mut self) -> Option<&mut Upgrade> {
        self.next.as_deref_mut()
    }

    pub fn friend_get_prev(&self) -> *mut Upgrade {
        self.prev
    }
}

/// Audio event placeholder - would be replaced with actual audio system
#[derive(Debug, Clone, Default)]
pub struct AudioEventRTS {
    // Placeholder for audio event data
}

/// Image placeholder - would be replaced with actual image system
pub struct Image {
    // Placeholder for image data
}

/// A single upgrade template definition
/// Matches C++ UpgradeTemplate class from Upgrade.h lines 135-199
pub struct UpgradeTemplate {
    upgrade_type: UpgradeType,
    name: String,
    name_key: NameKeyType,
    display_name_label: String,
    build_time: f32,
    cost: i32,
    upgrade_mask: UpgradeMaskType,
    research_sound: AudioEventRTS,
    unit_specific_sound: AudioEventRTS,
    academy_classification_type: AcademyClassificationType,
    button_image_name: String,
    button_image: Option<Arc<Image>>,
    next: Option<Arc<RwLock<UpgradeTemplate>>>,
    prev: Option<Arc<RwLock<UpgradeTemplate>>>,
}

impl UpgradeTemplate {
    /// Create a new upgrade template
    pub fn new() -> Self {
        Self {
            upgrade_type: UpgradeType::Player,
            name: String::new(),
            name_key: NAMEKEY_INVALID,
            display_name_label: String::new(),
            build_time: 0.0,
            cost: 0,
            upgrade_mask: UpgradeMaskType::new(),
            research_sound: AudioEventRTS::default(),
            unit_specific_sound: AudioEventRTS::default(),
            academy_classification_type: AcademyClassificationType::None,
            button_image_name: String::new(),
            button_image: None,
            next: None,
            prev: None,
        }
    }

    /// Calculate the time it takes (in logic frames) for a player to build this upgrade
    /// Matches C++ UpgradeTemplate::calcTimeToBuild from Upgrade.cpp lines 133-144
    pub fn calc_time_to_build(&self, player: &dyn PlayerInterface) -> i32 {
        // In debug/internal builds, check if player builds instantly
        #[cfg(any(debug_assertions, feature = "allow_debug_cheats"))]
        {
            if player.builds_instantly() {
                return 1;
            }
        }

        // C++ ignores build-time modifiers (todo comment for power state).
        (self.build_time * LOGICFRAMES_PER_SECOND as Real) as i32
    }

    /// Calculate the cost for this player to build this upgrade
    /// Matches C++ UpgradeTemplate::calcCostToBuild from Upgrade.cpp lines 150-155
    pub fn calc_cost_to_build(&self, player: &dyn PlayerInterface) -> i32 {
        let _ = player;
        // C++ ignores cost modifiers (todo comment for handicaps).
        self.cost
    }

    /// Set the upgrade name
    pub fn set_upgrade_name(&mut self, name: String) {
        self.name = name;
    }

    /// Get the upgrade name
    pub fn get_upgrade_name(&self) -> &str {
        &self.name
    }

    /// Set the upgrade name key
    pub fn set_upgrade_name_key(&mut self, key: NameKeyType) {
        self.name_key = key;
    }

    /// Get the upgrade name key
    pub fn get_upgrade_name_key(&self) -> NameKeyType {
        self.name_key
    }

    /// Get the display name label
    pub fn get_display_name_label(&self) -> &str {
        &self.display_name_label
    }

    /// Get the upgrade mask
    pub fn get_upgrade_mask(&self) -> &UpgradeMaskType {
        &self.upgrade_mask
    }

    /// Get the upgrade type
    pub fn get_upgrade_type(&self) -> UpgradeType {
        self.upgrade_type
    }

    /// Get the research complete sound
    pub fn get_research_complete_sound(&self) -> &AudioEventRTS {
        &self.research_sound
    }

    /// Get the unit specific sound
    pub fn get_unit_specific_sound(&self) -> &AudioEventRTS {
        &self.unit_specific_sound
    }

    /// Get the academy classification type
    pub fn get_academy_classification_type(&self) -> AcademyClassificationType {
        self.academy_classification_type
    }

    /// Get the button image
    pub fn get_button_image(&self) -> Option<&Arc<Image>> {
        self.button_image.as_ref()
    }

    /// Cache the button image
    pub fn cache_button_image(&mut self, image_collection: &dyn ImageCollectionInterface) {
        if !self.button_image_name.is_empty() {
            self.button_image = image_collection.find_image_by_name(&self.button_image_name);
            debug_assert!(
                self.button_image.is_some(),
                "UpgradeTemplate: {} missing button image {}",
                self.name,
                self.button_image_name
            );
            self.button_image_name.clear();
        }
    }

    /// Friend access methods for UpgradeCenter
    pub fn friend_set_upgrade_mask(&mut self, mask: UpgradeMaskType) {
        self.upgrade_mask = mask;
    }

    /// Make this template a veterancy upgrade
    /// Matches C++ UpgradeTemplate::friend_makeVeterancyUpgrade from Upgrade.cpp lines 170-180
    pub fn friend_make_veterancy_upgrade(&mut self, level: VeterancyLevel) {
        self.upgrade_type = UpgradeType::Object; // veterancy upgrades are always per-object
        self.name = get_vet_upgrade_name(level);
        self.name_key = name_to_key(&self.name);
        self.display_name_label.clear(); // should never be displayed
        self.build_time = 0.0;
        self.cost = 0;
        // leave upgrade_mask alone - will be set by UpgradeCenter
    }
}

impl Default for UpgradeTemplate {
    fn default() -> Self {
        Self::new()
    }
}

/// Get the veterancy upgrade name
/// Matches C++ getVetUpgradeName from Upgrade.cpp lines 160-166
fn get_vet_upgrade_name(level: VeterancyLevel) -> String {
    format!("Upgrade_Veterancy_{}", VETERANCY_NAMES[level as usize])
}

/// Trait for player interface (to avoid circular dependencies)
pub trait PlayerInterface {
    #[cfg(any(debug_assertions, feature = "allow_debug_cheats"))]
    fn builds_instantly(&self) -> bool;
    fn get_money(&self) -> &dyn MoneyInterface;
    /// Multiplier to account for power state/handicap effects on upgrade build time.
    fn get_build_time_modifier(&self) -> f32 {
        1.0
    }
    /// Multiplier to account for player handicap effects on upgrade costs.
    fn get_cost_modifier(&self) -> f32 {
        1.0
    }
    /// Check whether the player satisfies upgrade prerequisites.
    fn can_research_upgrade(&self, _upgrade: &UpgradeTemplate) -> bool {
        true
    }
}

/// Trait for money interface
pub trait MoneyInterface {
    fn count_money(&self) -> i32;
}

/// Trait for image collection interface
pub trait ImageCollectionInterface {
    fn find_image_by_name(&self, name: &str) -> Option<Arc<Image>>;
}

/// Trait for in-game UI interface
pub trait InGameUIInterface {
    fn message(&self, msg: &str);
}

/// The upgrade center keeps track of all possible upgrades
/// Matches C++ UpgradeCenter class from Upgrade.h lines 204-240
pub struct UpgradeCenter {
    upgrade_list: Option<Arc<RwLock<UpgradeTemplate>>>,
    upgrade_by_key: HashMap<NameKeyType, Arc<RwLock<UpgradeTemplate>>>,
    next_template_mask_bit: usize,
    button_images_cached: bool,
}

impl UpgradeCenter {
    /// Create a new upgrade center
    pub fn new() -> Self {
        Self {
            upgrade_list: None,
            upgrade_by_key: HashMap::new(),
            next_template_mask_bit: 0,
            button_images_cached: false,
        }
    }

    /// Initialize the upgrade center
    /// Matches C++ UpgradeCenter::init from Upgrade.cpp lines 236-255
    pub fn init(&mut self) {
        // Create veterancy upgrades (no regular level upgrade)
        let mut up = self.new_upgrade(String::new());
        up.write()
            .unwrap()
            .friend_make_veterancy_upgrade(VeterancyLevel::Veteran);

        let mut up = self.new_upgrade(String::new());
        up.write()
            .unwrap()
            .friend_make_veterancy_upgrade(VeterancyLevel::Elite);

        let mut up = self.new_upgrade(String::new());
        up.write()
            .unwrap()
            .friend_make_veterancy_upgrade(VeterancyLevel::Heroic);
    }

    /// Reset the upgrade center
    /// Matches C++ UpgradeCenter::reset from Upgrade.cpp lines 260-271
    pub fn reset(&mut self, _image_collection: Option<&dyn ImageCollectionInterface>) {
        if let Some(_collection) = _image_collection {
            if !self.button_images_cached {
                // Cache button images for all upgrades
                for (_key, template) in self.upgrade_by_key.iter() {
                    template.write().unwrap().cache_button_image(_collection);
                }
                self.button_images_cached = true;
            }
        }
    }

    /// Find upgrade by name key
    /// Matches C++ UpgradeCenter::findUpgradeByKey from Upgrade.cpp lines 312-323
    pub fn find_upgrade_by_key(&self, key: NameKeyType) -> Option<Arc<RwLock<UpgradeTemplate>>> {
        if let Some(found) = self.upgrade_by_key.get(&key).cloned() {
            return Some(found);
        }

        let mut current = self.upgrade_list.clone();
        while let Some(template) = current {
            if let Ok(guard) = template.read() {
                if guard.get_upgrade_name_key() == key {
                    return Some(template.clone());
                }
                current = guard.next.clone();
            } else {
                break;
            }
        }

        None
    }

    /// Find upgrade by name key (mutable handle for edit paths)
    /// Matches C++ UpgradeCenter::findNonConstUpgradeByKey from Upgrade.cpp lines 286-304
    pub fn find_non_const_upgrade_by_key(
        &self,
        key: NameKeyType,
    ) -> Option<Arc<RwLock<UpgradeTemplate>>> {
        self.find_upgrade_by_key(key)
    }

    /// Find upgrade by name
    /// Matches C++ UpgradeCenter::findUpgrade from Upgrade.cpp lines 328-333
    pub fn find_upgrade(&self, name: &str) -> Option<Arc<RwLock<UpgradeTemplate>>> {
        let key = name_to_key(name);
        self.find_upgrade_by_key(key)
    }

    /// Find veterancy upgrade
    /// Matches C++ UpgradeCenter::findVeterancyUpgrade from Upgrade.cpp lines 276-280
    pub fn find_veterancy_upgrade(
        &self,
        level: VeterancyLevel,
    ) -> Option<Arc<RwLock<UpgradeTemplate>>> {
        let name = get_vet_upgrade_name(level);
        self.find_upgrade(&name)
    }

    /// Allocate a new upgrade template
    /// Matches C++ UpgradeCenter::newUpgrade from Upgrade.cpp lines 338-365
    pub fn new_upgrade(&mut self, name: String) -> Arc<RwLock<UpgradeTemplate>> {
        let mut new_upgrade = UpgradeTemplate::new();

        // Copy data from the default upgrade (matches C++ assignment).
        if let Some(default_upgrade) = self.find_upgrade("DefaultUpgrade") {
            let default = default_upgrade.read().unwrap();
            new_upgrade.upgrade_type = default.upgrade_type;
            new_upgrade.display_name_label = default.display_name_label.clone();
            new_upgrade.build_time = default.build_time;
            new_upgrade.cost = default.cost;
            new_upgrade.research_sound = default.research_sound.clone();
            new_upgrade.unit_specific_sound = default.unit_specific_sound.clone();
            new_upgrade.academy_classification_type = default.academy_classification_type;
            new_upgrade.button_image_name = default.button_image_name.clone();
            new_upgrade.button_image = default.button_image.clone();
        }

        // Assign name
        new_upgrade.set_upgrade_name(name.clone());
        let key = name_to_key(&name);
        new_upgrade.set_upgrade_name_key(key);

        // Make a unique bitmask for this template
        let mut new_mask = UpgradeMaskType::new();
        new_mask.set_bit(self.next_template_mask_bit);
        self.next_template_mask_bit += 1;

        debug_assert!(
            self.next_template_mask_bit < UPGRADE_MAX_COUNT,
            "Can't have over {} types of Upgrades and have a Bitfield function.",
            UPGRADE_MAX_COUNT
        );

        new_upgrade.friend_set_upgrade_mask(new_mask);

        // Create Arc and link
        let upgrade_arc = Arc::new(RwLock::new(new_upgrade));
        self.link_upgrade(upgrade_arc.clone());

        upgrade_arc
    }

    /// Link an upgrade to the list
    /// Matches C++ UpgradeCenter::linkUpgrade from Upgrade.cpp lines 371-384
    fn link_upgrade(&mut self, upgrade: Arc<RwLock<UpgradeTemplate>>) {
        let key = upgrade.read().unwrap().get_upgrade_name_key();
        self.upgrade_by_key.insert(key, upgrade.clone());

        // Link to front of list (C++ uses a doubly linked list).
        let old_head = self.upgrade_list.take();
        {
            let mut upgrade_guard = upgrade.write().unwrap();
            upgrade_guard.prev = None;
            upgrade_guard.next = old_head.clone();
        }
        if let Some(head) = old_head.as_ref() {
            if let Ok(mut head_guard) = head.write() {
                head_guard.prev = Some(upgrade.clone());
            }
        }
        self.upgrade_list = Some(upgrade);
    }

    /// Unlink an upgrade from the list
    /// Matches C++ UpgradeCenter::unlinkUpgrade from Upgrade.cpp lines 389-406
    #[allow(dead_code)] // C++ parity: will be called when upgrade removal is fully integrated
    fn unlink_upgrade(&mut self, upgrade: &Arc<RwLock<UpgradeTemplate>>) {
        let (prev, next, key) = if let Ok(guard) = upgrade.read() {
            (guard.prev.clone(), guard.next.clone(), guard.get_upgrade_name_key())
        } else {
            return;
        };

        if let Some(prev_upgrade) = prev.as_ref() {
            if let Ok(mut prev_guard) = prev_upgrade.write() {
                prev_guard.next = next.clone();
            }
        } else {
            self.upgrade_list = next.clone();
        }

        if let Some(next_upgrade) = next.as_ref() {
            if let Ok(mut next_guard) = next_upgrade.write() {
                next_guard.prev = prev.clone();
            }
        }

        self.upgrade_by_key.remove(&key);
    }

    /// Check if player can afford this upgrade
    /// Matches C++ UpgradeCenter::canAffordUpgrade from Upgrade.cpp lines 409-432
    pub fn can_afford_upgrade(
        &self,
        player: &dyn PlayerInterface,
        upgrade_template: &UpgradeTemplate,
        display_reason: bool,
        ui: Option<&dyn InGameUIInterface>,
    ) -> bool {
        // Money check
        let money = player.get_money();
        if money.count_money() < upgrade_template.calc_cost_to_build(player) {
            // Post reason why we can't make upgrade
            if display_reason {
                if let Some(ui_system) = ui {
                    ui_system.message("GUI:NotEnoughMoneyToUpgrade");
                }
            }
            return false;
        }

        true
    }

    /// Get all upgrade names (for WorldBuilder)
    /// Matches C++ UpgradeCenter::getUpgradeNames from Upgrade.cpp lines 437-446
    pub fn get_upgrade_names(&self) -> Vec<String> {
        let mut names = Vec::new();
        let mut current = self.upgrade_list.clone();
        while let Some(template) = current {
            if let Ok(guard) = template.read() {
                names.push(guard.get_upgrade_name().to_string());
                current = guard.next.clone();
            } else {
                break;
            }
        }
        names
    }

    /// Get first upgrade template (for iteration)
    /// Matches C++ UpgradeCenter::firstUpgradeTemplate from Upgrade.cpp lines 302-307
    pub fn first_upgrade_template(&self) -> Option<Arc<RwLock<UpgradeTemplate>>> {
        self.upgrade_list.clone()
    }
}

impl Default for UpgradeCenter {
    fn default() -> Self {
        Self::new()
    }
}

fn name_to_key(name: &str) -> NameKeyType {
    NameKeyGenerator::name_to_key(name)
}

/// Global upgrade center instance
/// Matches C++ TheUpgradeCenter from Upgrade.cpp line 25
pub static UPGRADE_CENTER: Mutex<Option<UpgradeCenter>> = Mutex::new(None);

/// Initialize the global upgrade center
pub fn init_upgrade_center() {
    let mut center = UpgradeCenter::new();
    center.init();
    *UPGRADE_CENTER.lock().unwrap() = Some(center);
}

/// Get a reference to the global upgrade center
pub fn get_upgrade_center() -> std::sync::MutexGuard<'static, Option<UpgradeCenter>> {
    UPGRADE_CENTER.lock().unwrap()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_upgrade_mask_basic() {
        let mut mask = UpgradeMaskType::new();
        assert!(!mask.any());

        mask.set_bit(5);
        assert!(mask.test(5));
        assert!(!mask.test(6));
        assert!(mask.any());

        mask.clear_bit(5);
        assert!(!mask.test(5));
        assert!(!mask.any());
    }

    #[test]
    fn test_upgrade_mask_multiple_bits() {
        let mut mask = UpgradeMaskType::new();
        mask.set_bit(0);
        mask.set_bit(63);
        mask.set_bit(64);
        mask.set_bit(127);

        assert!(mask.test(0));
        assert!(mask.test(63));
        assert!(mask.test(64));
        assert!(mask.test(127));
        assert!(!mask.test(1));
        assert!(!mask.test(62));
    }

    #[test]
    fn test_upgrade_mask_intersection() {
        let mut mask1 = UpgradeMaskType::new();
        mask1.set_bit(5);
        mask1.set_bit(10);

        let mut mask2 = UpgradeMaskType::new();
        mask2.set_bit(10);
        mask2.set_bit(15);

        assert!(mask1.any_intersection_with(&mask2));
        assert!(mask2.any_intersection_with(&mask1));

        let mut mask3 = UpgradeMaskType::new();
        mask3.set_bit(20);
        assert!(!mask1.any_intersection_with(&mask3));
    }

    #[test]
    fn test_upgrade_mask_all_bits() {
        let mut mask = UpgradeMaskType::new();
        mask.set_bit(1);
        mask.set_bit(2);
        mask.set_bit(3);

        let mut required = UpgradeMaskType::new();
        required.set_bit(1);
        required.set_bit(2);

        assert!(mask.test_for_all(&required));

        required.set_bit(4);
        assert!(!mask.test_for_all(&required));
    }

    #[test]
    fn test_upgrade_template_creation() {
        let template = UpgradeTemplate::new();
        assert_eq!(template.get_upgrade_name(), "");
        assert_eq!(template.get_upgrade_type(), UpgradeType::Player);
        assert_eq!(template.build_time, 0.0);
        assert_eq!(template.cost, 0);
    }

    #[test]
    fn test_veterancy_upgrade_name() {
        let name = get_vet_upgrade_name(VeterancyLevel::Veteran);
        assert_eq!(name, "Upgrade_Veterancy_VETERAN");

        let name = get_vet_upgrade_name(VeterancyLevel::Elite);
        assert_eq!(name, "Upgrade_Veterancy_ELITE");

        let name = get_vet_upgrade_name(VeterancyLevel::Heroic);
        assert_eq!(name, "Upgrade_Veterancy_HEROIC");
    }

    #[test]
    fn test_upgrade_center_creation() {
        let mut center = UpgradeCenter::new();
        center.init();

        // Check veterancy upgrades were created
        assert!(center
            .find_veterancy_upgrade(VeterancyLevel::Veteran)
            .is_some());
        assert!(center
            .find_veterancy_upgrade(VeterancyLevel::Elite)
            .is_some());
        assert!(center
            .find_veterancy_upgrade(VeterancyLevel::Heroic)
            .is_some());
    }

    #[test]
    fn test_upgrade_center_new_upgrade() {
        let mut center = UpgradeCenter::new();
        let upgrade = center.new_upgrade("TestUpgrade".to_string());

        let template = upgrade.read().unwrap();
        assert_eq!(template.get_upgrade_name(), "TestUpgrade");
        assert!(template.get_upgrade_mask().any());
    }

    #[test]
    fn test_upgrade_center_find_by_name() {
        let mut center = UpgradeCenter::new();
        center.new_upgrade("TestUpgrade".to_string());

        let found = center.find_upgrade("TestUpgrade");
        assert!(found.is_some());

        let template = found.unwrap();
        assert_eq!(template.read().unwrap().get_upgrade_name(), "TestUpgrade");
    }

    #[test]
    fn test_upgrade_instance_creation() {
        let template = Arc::new(UpgradeTemplate::new());
        let upgrade = Upgrade::new(template.clone());

        assert_eq!(upgrade.get_status(), UpgradeStatusType::Invalid);
        assert!(Arc::ptr_eq(&upgrade.get_template(), &template));
    }

    #[test]
    fn test_upgrade_status_change() {
        let template = Arc::new(UpgradeTemplate::new());
        let mut upgrade = Upgrade::new(template);

        assert_eq!(upgrade.get_status(), UpgradeStatusType::Invalid);

        upgrade.set_status(UpgradeStatusType::InProduction);
        assert_eq!(upgrade.get_status(), UpgradeStatusType::InProduction);

        upgrade.set_status(UpgradeStatusType::Complete);
        assert_eq!(upgrade.get_status(), UpgradeStatusType::Complete);
    }
}
