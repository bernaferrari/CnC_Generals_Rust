//! Game type definitions
//!
//! This module provides enumerations and constants for various game types
//! such as time of day and weather conditions.

/// Time of day enumeration
///
/// Represents different times of day that can affect gameplay, lighting, etc.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TimeOfDay {
    Invalid,
    Morning,
    Afternoon,
    Evening,
    Night,
}

impl TimeOfDay {
    /// Get all time of day variants as a slice
    pub const ALL: &'static [TimeOfDay] = &[
        TimeOfDay::Invalid,
        TimeOfDay::Morning,
        TimeOfDay::Afternoon,
        TimeOfDay::Evening,
        TimeOfDay::Night,
    ];

    /// Get the string name of this time of day
    pub fn name(&self) -> &'static str {
        match self {
            TimeOfDay::Invalid => "NONE",
            TimeOfDay::Morning => "MORNING",
            TimeOfDay::Afternoon => "AFTERNOON",
            TimeOfDay::Evening => "EVENING",
            TimeOfDay::Night => "NIGHT",
        }
    }

    /// Parse a time of day from a string name
    pub fn from_name(name: &str) -> Option<TimeOfDay> {
        match name.to_uppercase().as_str() {
            "NONE" => Some(TimeOfDay::Invalid),
            "MORNING" => Some(TimeOfDay::Morning),
            "AFTERNOON" => Some(TimeOfDay::Afternoon),
            "EVENING" => Some(TimeOfDay::Evening),
            "NIGHT" => Some(TimeOfDay::Night),
            _ => None,
        }
    }

    /// Get the numeric value of this time of day (for serialization/compatibility)
    pub fn value(&self) -> u32 {
        match self {
            TimeOfDay::Invalid => 0,
            TimeOfDay::Morning => 1,
            TimeOfDay::Afternoon => 2,
            TimeOfDay::Evening => 3,
            TimeOfDay::Night => 4,
        }
    }

    /// Create from numeric value
    pub fn from_value(value: u32) -> Option<TimeOfDay> {
        match value {
            0 => Some(TimeOfDay::Invalid),
            1 => Some(TimeOfDay::Morning),
            2 => Some(TimeOfDay::Afternoon),
            3 => Some(TimeOfDay::Evening),
            4 => Some(TimeOfDay::Night),
            _ => None,
        }
    }
}

impl Default for TimeOfDay {
    fn default() -> Self {
        TimeOfDay::Invalid
    }
}

/// Weather condition enumeration
///
/// Represents different weather conditions that can affect gameplay, visuals, etc.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Weather {
    Normal,
    Snowy,
}

impl Weather {
    /// Get all weather variants as a slice
    pub const ALL: &'static [Weather] = &[Weather::Normal, Weather::Snowy];

    /// Get the string name of this weather condition
    pub fn name(&self) -> &'static str {
        match self {
            Weather::Normal => "NORMAL",
            Weather::Snowy => "SNOWY",
        }
    }

    /// Parse a weather condition from a string name
    pub fn from_name(name: &str) -> Option<Weather> {
        match name.to_uppercase().as_str() {
            "NORMAL" => Some(Weather::Normal),
            "SNOWY" => Some(Weather::Snowy),
            _ => None,
        }
    }

    /// Get the numeric value of this weather condition (for serialization/compatibility)
    pub fn value(&self) -> u32 {
        match self {
            Weather::Normal => 0,
            Weather::Snowy => 1,
        }
    }

    /// Create from numeric value
    pub fn from_value(value: u32) -> Option<Weather> {
        match value {
            0 => Some(Weather::Normal),
            1 => Some(Weather::Snowy),
            _ => None,
        }
    }
}

impl Default for Weather {
    fn default() -> Self {
        Weather::Normal
    }
}

/// Time of day names array (compatible with C++ code)
///
/// This matches the TimeOfDayNames array in the original C++ code.
pub const TIME_OF_DAY_NAMES: [&str; 6] = [
    "NONE",
    "MORNING",
    "AFTERNOON",
    "EVENING",
    "NIGHT",
    "", // NULL terminator equivalent
];

/// Weather names array (compatible with C++ code)
///
/// This matches the WeatherNames array in the original C++ code.
pub const WEATHER_NAMES: [&str; 3] = [
    "NORMAL", "SNOWY", "", // NULL terminator equivalent
];

/// Game environment settings
///
/// This structure combines time of day and weather settings for a complete
/// environmental description.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct GameEnvironment {
    pub time_of_day: TimeOfDay,
    pub weather: Weather,
}

impl GameEnvironment {
    /// Create a new game environment with specified conditions
    pub fn new(time_of_day: TimeOfDay, weather: Weather) -> Self {
        Self {
            time_of_day,
            weather,
        }
    }

    /// Create a default environment (no specific time, normal weather)
    pub fn default_environment() -> Self {
        Self {
            time_of_day: TimeOfDay::Invalid,
            weather: Weather::Normal,
        }
    }

    /// Check if this is a nighttime environment
    pub fn is_night(&self) -> bool {
        matches!(self.time_of_day, TimeOfDay::Night | TimeOfDay::Evening)
    }

    /// Check if this is a daytime environment
    pub fn is_day(&self) -> bool {
        matches!(self.time_of_day, TimeOfDay::Morning | TimeOfDay::Afternoon)
    }

    /// Check if weather affects visibility
    pub fn has_weather_effects(&self) -> bool {
        self.weather != Weather::Normal
    }

    /// Get a description string for this environment
    pub fn description(&self) -> String {
        format!("{} {}", self.time_of_day.name(), self.weather.name())
    }
}

impl Default for GameEnvironment {
    fn default() -> Self {
        Self::default_environment()
    }
}

/// Utility functions for game types

/// Get time of day name by index (for compatibility with C++ array indexing)
pub fn get_time_of_day_name(index: usize) -> Option<&'static str> {
    TIME_OF_DAY_NAMES
        .get(index)
        .filter(|&s| !s.is_empty())
        .map(|&s| s)
}

/// Get weather name by index (for compatibility with C++ array indexing)
pub fn get_weather_name(index: usize) -> Option<&'static str> {
    WEATHER_NAMES
        .get(index)
        .filter(|&s| !s.is_empty())
        .map(|&s| s)
}

/// Find time of day index by name (for compatibility with C++ code)
pub fn find_time_of_day_index(name: &str) -> Option<usize> {
    let upper_name = name.to_uppercase();
    TIME_OF_DAY_NAMES
        .iter()
        .position(|&s| !s.is_empty() && s == upper_name)
}

/// Find weather index by name (for compatibility with C++ code)
pub fn find_weather_index(name: &str) -> Option<usize> {
    let upper_name = name.to_uppercase();
    WEATHER_NAMES
        .iter()
        .position(|&s| !s.is_empty() && s == upper_name)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_time_of_day_names() {
        assert_eq!(TimeOfDay::Invalid.name(), "NONE");
        assert_eq!(TimeOfDay::Morning.name(), "MORNING");
        assert_eq!(TimeOfDay::Afternoon.name(), "AFTERNOON");
        assert_eq!(TimeOfDay::Evening.name(), "EVENING");
        assert_eq!(TimeOfDay::Night.name(), "NIGHT");
    }

    #[test]
    fn test_time_of_day_from_name() {
        assert_eq!(TimeOfDay::from_name("NONE"), Some(TimeOfDay::Invalid));
        assert_eq!(TimeOfDay::from_name("morning"), Some(TimeOfDay::Morning)); // Case insensitive
        assert_eq!(
            TimeOfDay::from_name("AFTERNOON"),
            Some(TimeOfDay::Afternoon)
        );
        assert_eq!(TimeOfDay::from_name("invalid"), None);
    }

    #[test]
    fn test_time_of_day_values() {
        assert_eq!(TimeOfDay::Invalid.value(), 0);
        assert_eq!(TimeOfDay::Morning.value(), 1);
        assert_eq!(TimeOfDay::Night.value(), 4);

        assert_eq!(TimeOfDay::from_value(0), Some(TimeOfDay::Invalid));
        assert_eq!(TimeOfDay::from_value(4), Some(TimeOfDay::Night));
        assert_eq!(TimeOfDay::from_value(99), None);
    }

    #[test]
    fn test_weather_names() {
        assert_eq!(Weather::Normal.name(), "NORMAL");
        assert_eq!(Weather::Snowy.name(), "SNOWY");
    }

    #[test]
    fn test_weather_from_name() {
        assert_eq!(Weather::from_name("NORMAL"), Some(Weather::Normal));
        assert_eq!(Weather::from_name("snowy"), Some(Weather::Snowy)); // Case insensitive
        assert_eq!(Weather::from_name("rainy"), None);
    }

    #[test]
    fn test_weather_values() {
        assert_eq!(Weather::Normal.value(), 0);
        assert_eq!(Weather::Snowy.value(), 1);

        assert_eq!(Weather::from_value(0), Some(Weather::Normal));
        assert_eq!(Weather::from_value(1), Some(Weather::Snowy));
        assert_eq!(Weather::from_value(2), None);
    }

    #[test]
    fn test_game_environment() {
        let env = GameEnvironment::new(TimeOfDay::Night, Weather::Snowy);

        assert!(env.is_night());
        assert!(!env.is_day());
        assert!(env.has_weather_effects());
        assert_eq!(env.description(), "NIGHT SNOWY");
    }

    #[test]
    fn test_default_environment() {
        let env = GameEnvironment::default();
        assert_eq!(env.time_of_day, TimeOfDay::Invalid);
        assert_eq!(env.weather, Weather::Normal);
        assert!(!env.has_weather_effects());
    }

    #[test]
    fn test_day_night_detection() {
        let morning = GameEnvironment::new(TimeOfDay::Morning, Weather::Normal);
        let afternoon = GameEnvironment::new(TimeOfDay::Afternoon, Weather::Normal);
        let evening = GameEnvironment::new(TimeOfDay::Evening, Weather::Normal);
        let night = GameEnvironment::new(TimeOfDay::Night, Weather::Normal);
        let none = GameEnvironment::new(TimeOfDay::Invalid, Weather::Normal);

        assert!(morning.is_day());
        assert!(!morning.is_night());

        assert!(afternoon.is_day());
        assert!(!afternoon.is_night());

        assert!(!evening.is_day());
        assert!(evening.is_night());

        assert!(!night.is_day());
        assert!(night.is_night());

        assert!(!none.is_day());
        assert!(!none.is_night());
    }

    #[test]
    fn test_utility_functions() {
        // Test name lookup by index
        assert_eq!(get_time_of_day_name(0), Some("NONE"));
        assert_eq!(get_time_of_day_name(1), Some("MORNING"));
        assert_eq!(get_time_of_day_name(99), None);

        assert_eq!(get_weather_name(0), Some("NORMAL"));
        assert_eq!(get_weather_name(1), Some("SNOWY"));
        assert_eq!(get_weather_name(99), None);

        // Test index lookup by name
        assert_eq!(find_time_of_day_index("NONE"), Some(0));
        assert_eq!(find_time_of_day_index("MORNING"), Some(1));
        assert_eq!(find_time_of_day_index("invalid"), None);

        assert_eq!(find_weather_index("NORMAL"), Some(0));
        assert_eq!(find_weather_index("SNOWY"), Some(1));
        assert_eq!(find_weather_index("invalid"), None);
    }

    #[test]
    fn test_constants_match_cpp() {
        // Verify that our constants match the original C++ arrays
        assert_eq!(TIME_OF_DAY_NAMES[0], "NONE");
        assert_eq!(TIME_OF_DAY_NAMES[1], "MORNING");
        assert_eq!(TIME_OF_DAY_NAMES[2], "AFTERNOON");
        assert_eq!(TIME_OF_DAY_NAMES[3], "EVENING");
        assert_eq!(TIME_OF_DAY_NAMES[4], "NIGHT");
        assert_eq!(TIME_OF_DAY_NAMES[5], ""); // NULL terminator

        assert_eq!(WEATHER_NAMES[0], "NORMAL");
        assert_eq!(WEATHER_NAMES[1], "SNOWY");
        assert_eq!(WEATHER_NAMES[2], ""); // NULL terminator
    }
}
