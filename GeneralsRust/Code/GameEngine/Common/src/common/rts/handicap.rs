//! Handicap system for balancing gameplay
//!
//! Handicap encapsulates the sets of modifiers to abilities used to balance
//! the game and give different abilities to different Players.
//! Conceptually, it's a large set of coefficients (typically, but not necessarily,
//! in the range of 0.0...1.0).
//!
//! Reference: C++ implementation at GeneralsMD/Code/GameEngine/Source/Common/RTS/Handicap.cpp

use std::collections::HashMap;

/// Dict structure for configuration reading
///
/// Reference: C++ Dict.h lines 34-297
/// Provides a general utility class for maintaining a sorted key-value pair list.
/// Keys are of type NameKeyType, and data may be Bool, int, real, or string.
pub struct Dict {
    /// Internal storage for key-value pairs
    pairs: HashMap<u32, DictValue>,
}

impl Dict {
    /// Create a new empty Dict
    pub fn new() -> Self {
        Self {
            pairs: HashMap::new(),
        }
    }

    /// Get a real (f32) value from the dict
    ///
    /// Reference: C++ Dict.h line 127: getReal(NameKeyType key, Bool* exists)
    pub fn get_real(&self, key: u32, exists: Option<&mut bool>) -> f32 {
        if let Some(value) = self.pairs.get(&key) {
            if let Some(exists_ref) = exists {
                *exists_ref = true;
            }
            match value {
                DictValue::Real(r) => *r,
                _ => 0.0,
            }
        } else {
            if let Some(exists_ref) = exists {
                *exists_ref = false;
            }
            0.0
        }
    }

    /// Set a real value in the dict
    ///
    /// Reference: C++ Dict.h line 195: setReal(NameKeyType key, Real value)
    pub fn set_real(&mut self, key: u32, value: f32) {
        self.pairs.insert(key, DictValue::Real(value));
    }
}

impl Default for Dict {
    fn default() -> Self {
        Self::new()
    }
}

/// Values that can be stored in a Dict
///
/// Reference: C++ Dict.h lines 42-50: DataType enum
#[derive(Debug, Clone)]
#[allow(dead_code)]
enum DictValue {
    Bool(bool),
    Int(i32),
    Real(f32),
    AsciiString(String),
}

/// Name key generator for converting strings to keys
///
/// Reference: C++ usage in Handicap.cpp line 70: TheNameKeyGenerator->nameToKey(c)
pub struct NameKeyGenerator;

impl NameKeyGenerator {
    /// Convert a string name to a numeric key
    /// Simple hash function for demonstration
    pub fn name_to_key(&self, name: &str) -> u32 {
        // Simple DJB2 hash algorithm
        let mut hash: u32 = 5381;
        for byte in name.bytes() {
            hash = hash.wrapping_mul(33).wrapping_add(byte as u32);
        }
        hash
    }
}

/// Global name key generator instance
///
/// Reference: C++ usage in Handicap.cpp line 70: TheNameKeyGenerator
pub static NAME_KEY_GENERATOR: NameKeyGenerator = NameKeyGenerator;

/// ThingTemplate structure
///
/// Reference: C++ ThingTemplate.h lines 324-745
pub struct ThingTemplate {
    /// KindOf flags for this template
    /// Reference: C++ ThingTemplate.h line 658: KindOfMaskType m_kindof
    kindof_mask: KindOfMask,
}

impl ThingTemplate {
    /// Create a new thing template for testing
    pub fn new() -> Self {
        Self {
            kindof_mask: KindOfMask::new(),
        }
    }

    /// Create a thing template with specific KindOf flags
    pub fn with_kindof(kindof_mask: KindOfMask) -> Self {
        Self { kindof_mask }
    }

    /// Check if this template has a specific KindOf flag set
    ///
    /// Reference: C++ ThingTemplate.h lines 374-377: isKindOf(KindOfType t)
    pub fn is_kind_of(&self, kind: KindOfType) -> bool {
        self.kindof_mask.test(kind)
    }
}

/// KindOf types for identifying groups of things
///
/// Reference: C++ KindOf.h lines 20-152
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u32)]
pub enum KindOfType {
    /// Reference: C++ KindOf.h line 31: KINDOF_STRUCTURE
    Structure = 7,
    Infantry = 32,
    Vehicle = 33,
    Aircraft = 34,
    // Add more as needed
}

/// Mask type for KindOf flags
///
/// Reference: C++ KindOf.h line 154: typedef BitFlags<KINDOF_COUNT> KindOfMaskType
#[derive(Debug, Clone, Default)]
pub struct KindOfMask {
    bits: u128, // Supports up to 128 different KindOf flags
}

impl KindOfMask {
    /// Create a new empty mask
    pub fn new() -> Self {
        Self { bits: 0 }
    }

    /// Set a specific KindOf flag
    pub fn set(&mut self, kind: KindOfType) {
        self.bits |= 1 << (kind as u32);
    }

    /// Test if a specific KindOf flag is set
    ///
    /// Reference: C++ KindOf.h lines 158-161: TEST_KINDOFMASK
    pub fn test(&self, kind: KindOfType) -> bool {
        (self.bits & (1 << (kind as u32))) != 0
    }
}

/// Types of handicaps that can be applied
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum HandicapType {
    /// Affects the cost of building/producing units
    BuildCost,
    /// Affects the time it takes to build/produce units  
    BuildTime,
}

impl HandicapType {
    /// Number of handicap types
    pub const COUNT: usize = 2;

    /// Get all handicap types as an array
    pub const fn all() -> [HandicapType; Self::COUNT] {
        [HandicapType::BuildCost, HandicapType::BuildTime]
    }

    /// Convert to string for configuration parsing
    pub fn as_str(&self) -> &'static str {
        match self {
            HandicapType::BuildCost => "BUILDCOST",
            HandicapType::BuildTime => "BUILDTIME",
        }
    }
}

/// Types of things that can have handicaps applied
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ThingType {
    /// If a thing is nothing else, it's generic
    Generic,
    /// Buildings/structures
    Buildings,
}

impl ThingType {
    /// Number of thing types
    pub const COUNT: usize = 2;

    /// Get all thing types as an array
    pub const fn all() -> [ThingType; Self::COUNT] {
        [ThingType::Generic, ThingType::Buildings]
    }

    /// Convert to string for configuration parsing
    pub fn as_str(&self) -> &'static str {
        match self {
            ThingType::Generic => "GENERIC",
            ThingType::Buildings => "BUILDINGS",
        }
    }
}

/// Handicap system for game balance
///
/// Usage example (conceptual):
/// ```rust
/// let armor_coef = handicap.get_handicap(HandicapType::BuildCost, thing_template);
/// let actual_cost = base_cost * armor_coef;
/// ```
#[derive(Debug)]
pub struct Handicap {
    /// 2D array of handicap multipliers [handicap_type][thing_type]
    handicaps: [[f32; ThingType::COUNT]; HandicapType::COUNT],
}

impl Handicap {
    /// Create a new Handicap system
    pub fn new() -> Self {
        let mut handicap = Self {
            handicaps: [[1.0; ThingType::COUNT]; HandicapType::COUNT],
        };
        handicap.init();
        handicap
    }

    /// Reset all handicaps to default value (1.0 = no handicap)
    pub fn init(&mut self) {
        for i in 0..HandicapType::COUNT {
            for j in 0..ThingType::COUNT {
                self.handicaps[i][j] = 1.0;
            }
        }
    }

    /// Initialize from the fields in the Dict
    ///
    /// Note that this does NOT call init() internally, so only those fields
    /// that are present in the dict will be set. If you want to ensure all
    /// fields are something reasonable, you should call init() prior to calling this.
    ///
    /// Reference: C++ Handicap.cpp lines 35-77: readFromDict(const Dict* d)
    pub fn read_from_dict(&mut self, dict: &Dict) {
        // Reference: C++ Handicap.cpp lines 37-38
        // "this isn't very efficient, but is only called at load times,
        // so it probably doesn't really matter."

        // Reference: C++ Handicap.cpp lines 40-49: handicap type names
        let ht_names = ["BUILDCOST", "BUILDTIME"];

        // Reference: C++ Handicap.cpp lines 51-55: thing type names
        let tt_names = ["GENERIC", "BUILDINGS"];

        // Reference: C++ Handicap.cpp lines 57-58
        // no, you should NOT call init() here.

        // Reference: C++ Handicap.cpp lines 60-76
        // Iterate through all combinations of handicap types and thing types
        for (i, ht_name) in ht_names.iter().enumerate() {
            for (j, tt_name) in tt_names.iter().enumerate() {
                // Reference: C++ Handicap.cpp lines 65-69
                // Construct key like "HANDICAP_BUILDCOST_GENERIC"
                let key_string = format!("HANDICAP_{}_{}", ht_name, tt_name);

                // Reference: C++ Handicap.cpp line 70
                let key = NAME_KEY_GENERATOR.name_to_key(&key_string);

                // Reference: C++ Handicap.cpp lines 71-74
                let mut exists = false;
                let value = dict.get_real(key, Some(&mut exists));

                if exists {
                    self.handicaps[i][j] = value;
                }
            }
        }
    }

    /// Return the multiplier for the given Handicap type for the given thing type.
    ///
    /// The thing_type (unit, building, etc.) will generally be examined
    /// to determine what value to return.
    ///
    /// C++ Reference: Handicap::getHandicap(HandicapType, const ThingTemplate*)
    pub fn get_handicap_for_type(&self, handicap_type: HandicapType, thing_type: ThingType) -> f32 {
        self.handicaps[handicap_type as usize][thing_type as usize]
    }

    /// Return the multiplier for the given Handicap type on the given template
    ///
    /// The template's type (unit, building, etc.) will generally be examined
    /// to determine what value to return.
    pub fn get_handicap(&self, handicap_type: HandicapType, template: &ThingTemplate) -> f32 {
        let thing_type = Self::get_best_thing_type(template);
        self.get_handicap_for_type(handicap_type, thing_type)
    }

    /// Determine the best ThingType for a given template
    ///
    /// If this ends up being too slow, we could cache the information in the template
    ///
    /// Reference: C++ Handicap.cpp lines 80-87: getBestThingType(const ThingTemplate *tmpl)
    fn get_best_thing_type(template: &ThingTemplate) -> ThingType {
        // Reference: C++ Handicap.cpp line 82
        // "if this ends up being too slow, cache the information in the object"

        // Reference: C++ Handicap.cpp lines 83-84
        if template.is_kind_of(KindOfType::Structure) {
            return ThingType::Buildings;
        }

        // Reference: C++ Handicap.cpp line 86
        ThingType::Generic
    }

    /// Set a specific handicap value
    pub fn set_handicap(&mut self, handicap_type: HandicapType, thing_type: ThingType, value: f32) {
        self.handicaps[handicap_type as usize][thing_type as usize] = value;
    }

    /// Get a specific handicap value directly
    pub fn get_handicap_direct(&self, handicap_type: HandicapType, thing_type: ThingType) -> f32 {
        self.handicaps[handicap_type as usize][thing_type as usize]
    }

    /// Create a handicap preset for Easy difficulty (player advantage)
    ///
    /// Easy mode gives the player cheaper and faster units, making the game easier.
    /// These values are balanced for single-player campaigns.
    pub fn preset_easy() -> Self {
        let mut handicap = Self::new();

        // Buildings cost less and build faster
        handicap.set_handicap(HandicapType::BuildCost, ThingType::Buildings, 0.8);
        handicap.set_handicap(HandicapType::BuildTime, ThingType::Buildings, 0.8);

        // Generic units also get a bonus
        handicap.set_handicap(HandicapType::BuildCost, ThingType::Generic, 0.85);
        handicap.set_handicap(HandicapType::BuildTime, ThingType::Generic, 0.85);

        handicap
    }

    /// Create a handicap preset for Medium difficulty (balanced)
    ///
    /// Medium mode is the default balanced experience with no handicap modifications.
    pub fn preset_medium() -> Self {
        Self::new() // All values remain at 1.0 (no modification)
    }

    /// Create a handicap preset for Hard difficulty (player disadvantage)
    ///
    /// Hard mode makes the player's units more expensive and slower to build,
    /// providing a challenge for experienced players.
    pub fn preset_hard() -> Self {
        let mut handicap = Self::new();

        // Buildings cost more and build slower
        handicap.set_handicap(HandicapType::BuildCost, ThingType::Buildings, 1.2);
        handicap.set_handicap(HandicapType::BuildTime, ThingType::Buildings, 1.2);

        // Generic units also get penalized
        handicap.set_handicap(HandicapType::BuildCost, ThingType::Generic, 1.15);
        handicap.set_handicap(HandicapType::BuildTime, ThingType::Generic, 1.15);

        handicap
    }

    /// Create a handicap preset for Brutal difficulty (extreme player disadvantage)
    ///
    /// Brutal mode significantly increases costs and build times, designed for
    /// the most challenging gameplay experience.
    pub fn preset_brutal() -> Self {
        let mut handicap = Self::new();

        // Buildings are significantly more expensive and slower
        handicap.set_handicap(HandicapType::BuildCost, ThingType::Buildings, 1.5);
        handicap.set_handicap(HandicapType::BuildTime, ThingType::Buildings, 1.5);

        // Generic units also face steep penalties
        handicap.set_handicap(HandicapType::BuildCost, ThingType::Generic, 1.35);
        handicap.set_handicap(HandicapType::BuildTime, ThingType::Generic, 1.35);

        handicap
    }

    /// Create a handicap from a Dict configuration
    ///
    /// This is the standard way to load handicaps from game configuration files.
    /// The Dict should contain keys like "HANDICAP_BUILDCOST_GENERIC = 0.8"
    pub fn from_dict(dict: &Dict) -> Self {
        let mut handicap = Self::new();
        handicap.read_from_dict(dict);
        handicap
    }
}

impl Default for Handicap {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Test: Handicap initialization
    ///
    /// Reference: C++ Handicap.cpp lines 21-32: Constructor and init()
    #[test]
    fn test_handicap_init() {
        let handicap = Handicap::new();

        // All values should start at 1.0 (no handicap)
        // Reference: C++ Handicap.cpp line 31: m_handicaps[i][j] = 1.0f
        for ht in HandicapType::all() {
            for tt in ThingType::all() {
                assert_eq!(handicap.get_handicap_direct(ht, tt), 1.0);
            }
        }
    }

    /// Test: Setting and getting handicap values
    #[test]
    fn test_handicap_set_get() {
        let mut handicap = Handicap::new();

        // Set a specific handicap
        handicap.set_handicap(HandicapType::BuildCost, ThingType::Buildings, 0.8);

        // Verify it was set correctly
        assert_eq!(
            handicap.get_handicap_direct(HandicapType::BuildCost, ThingType::Buildings),
            0.8
        );

        // Verify other values are still 1.0
        assert_eq!(
            handicap.get_handicap_direct(HandicapType::BuildTime, ThingType::Buildings),
            1.0
        );
    }

    /// Test: Enum string representations
    #[test]
    fn test_handicap_types() {
        // Reference: C++ Handicap.cpp lines 40-49: htNames array
        assert_eq!(HandicapType::BuildCost.as_str(), "BUILDCOST");
        assert_eq!(HandicapType::BuildTime.as_str(), "BUILDTIME");

        // Reference: C++ Handicap.cpp lines 51-55: ttNames array
        assert_eq!(ThingType::Generic.as_str(), "GENERIC");
        assert_eq!(ThingType::Buildings.as_str(), "BUILDINGS");
    }

    /// Test: Dict parsing functionality
    ///
    /// Reference: C++ Handicap.cpp lines 35-77: readFromDict implementation
    #[test]
    fn test_read_from_dict() {
        let mut dict = Dict::new();

        // Set some handicap values in the dict
        // Reference: C++ Handicap.cpp lines 65-69: Key construction
        let key1 = NAME_KEY_GENERATOR.name_to_key("HANDICAP_BUILDCOST_GENERIC");
        let key2 = NAME_KEY_GENERATOR.name_to_key("HANDICAP_BUILDTIME_BUILDINGS");

        dict.set_real(key1, 0.75);
        dict.set_real(key2, 1.25);

        // Create handicap and read from dict
        let mut handicap = Handicap::new();
        handicap.read_from_dict(&dict);

        // Verify values were loaded
        assert_eq!(
            handicap.get_handicap_direct(HandicapType::BuildCost, ThingType::Generic),
            0.75
        );
        assert_eq!(
            handicap.get_handicap_direct(HandicapType::BuildTime, ThingType::Buildings),
            1.25
        );

        // Verify unset values remain at default
        assert_eq!(
            handicap.get_handicap_direct(HandicapType::BuildCost, ThingType::Buildings),
            1.0
        );
    }

    /// Test: ThingTemplate KindOf checking
    ///
    /// Reference: C++ Handicap.cpp lines 80-87: getBestThingType implementation
    #[test]
    fn test_get_best_thing_type() {
        // Test with a generic template (no KINDOF_STRUCTURE)
        let generic_template = ThingTemplate::new();
        let thing_type = Handicap::get_best_thing_type(&generic_template);
        assert_eq!(thing_type, ThingType::Generic);

        // Test with a building template (has KINDOF_STRUCTURE)
        let mut building_mask = KindOfMask::new();
        building_mask.set(KindOfType::Structure);
        let building_template = ThingTemplate::with_kindof(building_mask);
        let thing_type = Handicap::get_best_thing_type(&building_template);
        assert_eq!(thing_type, ThingType::Buildings);
    }

    /// Test: get_handicap with template
    ///
    /// Reference: C++ Handicap.cpp lines 90-94: getHandicap implementation
    #[test]
    fn test_get_handicap_with_template() {
        let mut handicap = Handicap::new();
        handicap.set_handicap(HandicapType::BuildCost, ThingType::Buildings, 0.8);
        handicap.set_handicap(HandicapType::BuildCost, ThingType::Generic, 0.9);

        // Test with building template
        let mut building_mask = KindOfMask::new();
        building_mask.set(KindOfType::Structure);
        let building_template = ThingTemplate::with_kindof(building_mask);

        let cost_modifier = handicap.get_handicap(HandicapType::BuildCost, &building_template);
        assert_eq!(cost_modifier, 0.8);

        // Test with generic template
        let generic_template = ThingTemplate::new();
        let cost_modifier = handicap.get_handicap(HandicapType::BuildCost, &generic_template);
        assert_eq!(cost_modifier, 0.9);
    }

    /// Test: Easy difficulty preset
    #[test]
    fn test_preset_easy() {
        let handicap = Handicap::preset_easy();

        // Easy mode should have reduced costs and build times
        assert_eq!(
            handicap.get_handicap_direct(HandicapType::BuildCost, ThingType::Buildings),
            0.8
        );
        assert_eq!(
            handicap.get_handicap_direct(HandicapType::BuildTime, ThingType::Buildings),
            0.8
        );
        assert_eq!(
            handicap.get_handicap_direct(HandicapType::BuildCost, ThingType::Generic),
            0.85
        );
        assert_eq!(
            handicap.get_handicap_direct(HandicapType::BuildTime, ThingType::Generic),
            0.85
        );
    }

    /// Test: Medium difficulty preset
    #[test]
    fn test_preset_medium() {
        let handicap = Handicap::preset_medium();

        // Medium mode should have no handicaps (all 1.0)
        for ht in HandicapType::all() {
            for tt in ThingType::all() {
                assert_eq!(handicap.get_handicap_direct(ht, tt), 1.0);
            }
        }
    }

    /// Test: Hard difficulty preset
    #[test]
    fn test_preset_hard() {
        let handicap = Handicap::preset_hard();

        // Hard mode should have increased costs and build times
        assert_eq!(
            handicap.get_handicap_direct(HandicapType::BuildCost, ThingType::Buildings),
            1.2
        );
        assert_eq!(
            handicap.get_handicap_direct(HandicapType::BuildTime, ThingType::Buildings),
            1.2
        );
        assert_eq!(
            handicap.get_handicap_direct(HandicapType::BuildCost, ThingType::Generic),
            1.15
        );
        assert_eq!(
            handicap.get_handicap_direct(HandicapType::BuildTime, ThingType::Generic),
            1.15
        );
    }

    /// Test: Brutal difficulty preset
    #[test]
    fn test_preset_brutal() {
        let handicap = Handicap::preset_brutal();

        // Brutal mode should have significantly increased costs and build times
        assert_eq!(
            handicap.get_handicap_direct(HandicapType::BuildCost, ThingType::Buildings),
            1.5
        );
        assert_eq!(
            handicap.get_handicap_direct(HandicapType::BuildTime, ThingType::Buildings),
            1.5
        );
        assert_eq!(
            handicap.get_handicap_direct(HandicapType::BuildCost, ThingType::Generic),
            1.35
        );
        assert_eq!(
            handicap.get_handicap_direct(HandicapType::BuildTime, ThingType::Generic),
            1.35
        );
    }

    /// Test: from_dict constructor
    #[test]
    fn test_from_dict() {
        let mut dict = Dict::new();
        let key = NAME_KEY_GENERATOR.name_to_key("HANDICAP_BUILDCOST_GENERIC");
        dict.set_real(key, 0.5);

        let handicap = Handicap::from_dict(&dict);

        assert_eq!(
            handicap.get_handicap_direct(HandicapType::BuildCost, ThingType::Generic),
            0.5
        );
    }

    /// Test: KindOfMask operations
    #[test]
    fn test_kindof_mask() {
        let mut mask = KindOfMask::new();

        // Initially empty
        assert!(!mask.test(KindOfType::Structure));

        // Set a flag
        mask.set(KindOfType::Structure);
        assert!(mask.test(KindOfType::Structure));

        // Other flags should still be clear
        assert!(!mask.test(KindOfType::Infantry));
    }

    /// Test: Dict get_real with exists parameter
    #[test]
    fn test_dict_get_real_exists() {
        let mut dict = Dict::new();
        let key = 12345;

        // Test with non-existent key
        let mut exists = true;
        let value = dict.get_real(key, Some(&mut exists));
        assert_eq!(value, 0.0);
        assert!(!exists);

        // Set a value
        dict.set_real(key, 2.5);

        // Test with existing key
        let mut exists = false;
        let value = dict.get_real(key, Some(&mut exists));
        assert_eq!(value, 2.5);
        assert!(exists);
    }

    /// Test: Name key generator consistency
    #[test]
    fn test_name_key_generator() {
        let key1 = NAME_KEY_GENERATOR.name_to_key("HANDICAP_BUILDCOST_GENERIC");
        let key2 = NAME_KEY_GENERATOR.name_to_key("HANDICAP_BUILDCOST_GENERIC");
        let key3 = NAME_KEY_GENERATOR.name_to_key("HANDICAP_BUILDTIME_GENERIC");

        // Same string should generate same key
        assert_eq!(key1, key2);

        // Different strings should generate different keys
        assert_ne!(key1, key3);
    }

    /// Test: Practical usage example - calculating modified cost
    #[test]
    fn test_practical_cost_calculation() {
        let handicap = Handicap::preset_easy();

        // Base cost of a building
        let base_cost = 1000.0;

        // Get handicap for buildings
        let mut building_mask = KindOfMask::new();
        building_mask.set(KindOfType::Structure);
        let building_template = ThingTemplate::with_kindof(building_mask);

        let cost_modifier = handicap.get_handicap(HandicapType::BuildCost, &building_template);

        // Calculate actual cost
        let actual_cost = base_cost * cost_modifier;

        // On easy mode, buildings should cost 80% of base
        assert_eq!(actual_cost, 800.0);
    }

    /// Test: Practical usage example - calculating modified build time
    #[test]
    fn test_practical_build_time_calculation() {
        let handicap = Handicap::preset_hard();

        // Base build time (in seconds)
        let base_time = 60.0;

        // Get handicap for generic units
        let generic_template = ThingTemplate::new();

        let time_modifier = handicap.get_handicap(HandicapType::BuildTime, &generic_template);

        // Calculate actual build time
        let actual_time = base_time * time_modifier;

        // On hard mode, generic units should take 115% of base time
        assert_eq!(actual_time, 69.0);
    }
}
