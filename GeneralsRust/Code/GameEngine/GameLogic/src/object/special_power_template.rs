// FILE: special_power_template.rs
// Port of SpecialPower.h and SpecialPower.cpp
// Author: Rust Port
// Desc: Special power templates and the system that holds them

use crate::common::science::{ScienceType, SCIENCE_INVALID};
use crate::common::{AsciiString, LOGICFRAMES_PER_SECOND};
use crate::object::special_power_types::SpecialPowerType;
use game_engine::common::ini::ini::INI;
use game_engine::common::ini::ini_special_power::{
    get_special_power_store as get_ini_special_power_store,
    SpecialPowerTemplate as IniSpecialPowerTemplate,
};
use game_engine::common::rts::get_science_store;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, OnceLock, RwLock, RwLockReadGuard, RwLockWriteGuard};

pub use crate::common::audio::AudioEventRts;

/// Academy classification type for tracking player behavior
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum AcademyClassificationType {
    Invalid,
    Tactical,
    Strategic,
    Superweapon,
    Defensive,
    Economic,
}

impl Default for AcademyClassificationType {
    fn default() -> Self {
        AcademyClassificationType::Invalid
    }
}

/// Special power template - defines a special power's properties
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpecialPowerTemplate {
    /// Unique name identifier
    name: String,

    /// Unique numeric identifier
    id: u32,

    /// Type enum for fast type checking
    power_type: SpecialPowerType,

    /// Reload time in frames after using special power
    reload_time: u32,

    /// Science required to execute this power
    required_science: ScienceType,

    /// Sound to play when initiated
    initiate_sound: AudioEventRts,

    /// Sound to play at target location
    initiate_at_location_sound: AudioEventRts,

    /// Academy classification for AI advice
    academy_classification: AcademyClassificationType,

    /// Detection time in frames for infiltration powers
    detection_time: u32,

    /// Lifetime of view object created
    view_object_duration: u32,

    /// Vision range of view object
    view_object_range: f32,

    /// Radius cursor size
    radius_cursor_radius: f32,

    /// Display countdown timer for all players
    public_timer: bool,

    /// Shared between all command centers
    shared_n_sync: bool,

    /// Can be fired from side panel
    shortcut_power: bool,
}

impl SpecialPowerTemplate {
    /// Create a new special power template
    /// Default values match C++ SpecialPowerTemplate constructor (SpecialPower.cpp lines 189-211)
    pub fn new(name: String, id: u32) -> Self {
        Self {
            name,
            id,
            power_type: SpecialPowerType::Invalid,
            reload_time: 0,
            required_science: SCIENCE_INVALID,
            initiate_sound: AudioEventRts::default(),
            initiate_at_location_sound: AudioEventRts::default(),
            academy_classification: AcademyClassificationType::Invalid,
            // Default detection time matches C++ DEFAULT_DEFECTION_DETECTION_PROTECTION_TIME_LIMIT
            // which is LOGICFRAMES_PER_SECOND * 10
            detection_time: LOGICFRAMES_PER_SECOND * 10,
            view_object_duration: 0,
            view_object_range: 0.0,
            radius_cursor_radius: 0.0,
            public_timer: false,
            shared_n_sync: false,
            shortcut_power: false,
        }
    }

    // Getters
    pub fn get_name(&self) -> &str {
        &self.name
    }

    pub fn get_id(&self) -> u32 {
        self.id
    }

    pub fn get_special_power_type(&self) -> SpecialPowerType {
        self.power_type
    }

    pub fn get_reload_time(&self) -> u32 {
        self.reload_time
    }

    pub fn get_required_science(&self) -> ScienceType {
        self.required_science
    }

    pub fn get_initiate_sound(&self) -> &AudioEventRts {
        &self.initiate_sound
    }

    pub fn get_initiate_at_target_sound(&self) -> &AudioEventRts {
        &self.initiate_at_location_sound
    }

    pub fn has_public_timer(&self) -> bool {
        self.public_timer
    }

    pub fn is_shared_n_sync(&self) -> bool {
        self.shared_n_sync
    }

    pub fn get_detection_time(&self) -> u32 {
        self.detection_time
    }

    pub fn get_view_object_duration(&self) -> u32 {
        self.view_object_duration
    }

    pub fn get_view_object_range(&self) -> f32 {
        self.view_object_range
    }

    pub fn get_radius_cursor_radius(&self) -> f32 {
        self.radius_cursor_radius
    }

    pub fn is_shortcut_power(&self) -> bool {
        self.shortcut_power
    }

    pub fn get_academy_classification_type(&self) -> AcademyClassificationType {
        self.academy_classification
    }

    // Setters (builder pattern)
    pub fn with_power_type(mut self, power_type: SpecialPowerType) -> Self {
        self.power_type = power_type;
        self
    }

    pub fn with_reload_time(mut self, reload_time: u32) -> Self {
        self.reload_time = reload_time;
        self
    }

    pub fn with_required_science(mut self, science: ScienceType) -> Self {
        self.required_science = science;
        self
    }

    pub fn with_initiate_sound(mut self, sound: AudioEventRts) -> Self {
        self.initiate_sound = sound;
        self
    }

    pub fn with_initiate_at_location_sound(mut self, sound: AudioEventRts) -> Self {
        self.initiate_at_location_sound = sound;
        self
    }

    pub fn with_academy_classification(
        mut self,
        classification: AcademyClassificationType,
    ) -> Self {
        self.academy_classification = classification;
        self
    }

    pub fn with_detection_time(mut self, time: u32) -> Self {
        self.detection_time = time;
        self
    }

    pub fn with_view_object_duration(mut self, duration: u32) -> Self {
        self.view_object_duration = duration;
        self
    }

    pub fn with_view_object_range(mut self, range: f32) -> Self {
        self.view_object_range = range;
        self
    }

    pub fn with_radius_cursor_radius(mut self, radius: f32) -> Self {
        self.radius_cursor_radius = radius;
        self
    }

    pub fn with_public_timer(mut self, public_timer: bool) -> Self {
        self.public_timer = public_timer;
        self
    }

    pub fn with_shared_n_sync(mut self, shared: bool) -> Self {
        self.shared_n_sync = shared;
        self
    }

    pub fn with_shortcut_power(mut self, shortcut: bool) -> Self {
        self.shortcut_power = shortcut;
        self
    }
}

/// Special power store - manages all special power templates
#[derive(Debug, Default)]
pub struct SpecialPowerStore {
    /// Template IDs in C++ m_specialPowerTemplates insertion order
    template_order: Vec<u32>,

    /// Templates stored by name
    templates_by_name: HashMap<String, SpecialPowerTemplate>,

    /// Templates stored by ID
    templates_by_id: HashMap<u32, SpecialPowerTemplate>,

    /// Next available ID
    next_id: u32,
}

impl SpecialPowerStore {
    /// Create a new special power store
    pub fn new() -> Self {
        Self {
            template_order: Vec::new(),
            templates_by_name: HashMap::new(),
            templates_by_id: HashMap::new(),
            next_id: 1,
        }
    }

    /// Initialize the store
    pub fn init(&mut self) {
        // Initialization logic if needed
    }

    /// Update the store each frame
    pub fn update(&mut self) {
        // Update logic if needed
    }

    /// Reset the store
    pub fn reset(&mut self) {
        self.template_order.clear();
        self.templates_by_name.clear();
        self.templates_by_id.clear();
        self.next_id = 1;
    }

    /// Add a special power template
    pub fn add_template(&mut self, template: SpecialPowerTemplate) {
        let name = template.get_name().to_string();
        let id = template.get_id();

        if !self.templates_by_id.contains_key(&id) {
            self.template_order.push(id);
        }
        self.templates_by_name.insert(name, template.clone());
        self.templates_by_id.insert(id, template);
    }

    /// Find a special power template by name
    pub fn find_special_power_template(&self, name: &str) -> Option<&SpecialPowerTemplate> {
        self.templates_by_name.get(name)
    }

    /// Find a special power template by ID
    pub fn find_special_power_template_by_id(&self, id: u32) -> Option<&SpecialPowerTemplate> {
        self.templates_by_id.get(&id)
    }

    /// Get a special power template by index (for tools/WorldBuilder)
    pub fn get_special_power_template_by_index(
        &self,
        index: usize,
    ) -> Option<&SpecialPowerTemplate> {
        let id = self.template_order.get(index)?;
        self.templates_by_id.get(id)
    }

    /// Get the number of special powers
    pub fn get_num_special_powers(&self) -> usize {
        self.template_order.len()
    }

    /// Get the next available ID
    pub fn get_next_id(&mut self) -> u32 {
        let id = self.next_id;
        self.next_id += 1;
        id
    }

    /// Check if an object can use a special power.
    /// Matches C++ `SpecialPowerStore::canUseSpecialPower`.
    pub fn can_use_special_power(&self, object_id: u32, template: &SpecialPowerTemplate) -> bool {
        use crate::object::registry::OBJECT_REGISTRY;

        let Some(obj_arc) = OBJECT_REGISTRY.get_object(object_id) else {
            return false;
        };
        let Ok(obj_guard) = obj_arc.read() else {
            return false;
        };

        self.can_use_special_power_for_object(&obj_guard, template)
    }

    /// Object-reference variant used when the caller already owns the object.
    pub fn can_use_special_power_for_object(
        &self,
        obj: &crate::object::Object,
        template: &SpecialPowerTemplate,
    ) -> bool {
        if !obj.has_special_power_module_for_power(template) {
            return false;
        }

        let required = template.get_required_science();
        if required == SCIENCE_INVALID {
            return true;
        }

        let Some(player) = obj.get_controlling_player() else {
            return false;
        };
        let Ok(player_guard) = player.read() else {
            return false;
        };

        player_guard.has_science(required)
    }
}

static SPECIAL_POWER_STORE: OnceLock<RwLock<SpecialPowerStore>> = OnceLock::new();

fn special_power_store_cell() -> &'static RwLock<SpecialPowerStore> {
    SPECIAL_POWER_STORE.get_or_init(|| RwLock::new(SpecialPowerStore::new()))
}

pub fn get_special_power_store() -> Option<RwLockReadGuard<'static, SpecialPowerStore>> {
    Some(
        special_power_store_cell()
            .read()
            .expect("SpecialPowerStore poisoned"),
    )
}

pub fn get_special_power_store_mut() -> Option<RwLockWriteGuard<'static, SpecialPowerStore>> {
    Some(
        special_power_store_cell()
            .write()
            .expect("SpecialPowerStore poisoned"),
    )
}

pub fn find_or_create_special_power_template(name: &AsciiString) -> Arc<SpecialPowerTemplate> {
    if let Some(store) = get_special_power_store() {
        if let Some(template) = store.find_special_power_template(name.as_str()) {
            return Arc::new(template.clone());
        }
    }

    let mut store =
        get_special_power_store_mut().expect("SpecialPowerStore missing during template creation");
    let id = store.get_next_id();
    let template = match get_ini_special_power_store()
        .and_then(|ini_store| ini_store.find_template(name).cloned())
    {
        Some(ini_template) => build_from_ini_special_power(name, id, &ini_template),
        None => SpecialPowerTemplate::new(name.to_string(), id),
    };
    store.add_template(template.clone());
    Arc::new(template)
}

fn parse_bool_property(value: &str) -> bool {
    matches!(
        value.trim().to_ascii_lowercase().as_str(),
        "1" | "true" | "yes"
    )
}

fn parse_duration_frames(value: &str) -> Option<u32> {
    INI::parse_duration_unsigned_int(value).ok()
}

fn parse_academy_classification(value: &str) -> AcademyClassificationType {
    match value.trim().to_ascii_uppercase().as_str() {
        "TACTICAL" => AcademyClassificationType::Tactical,
        "STRATEGIC" => AcademyClassificationType::Strategic,
        "SUPERWEAPON" | "SUPERPOWER" | "ACT_SUPERPOWER" => AcademyClassificationType::Superweapon,
        "DEFENSIVE" => AcademyClassificationType::Defensive,
        "ECONOMIC" => AcademyClassificationType::Economic,
        _ => AcademyClassificationType::Invalid,
    }
}

fn parse_special_power_enum(value: &str) -> Option<SpecialPowerType> {
    let token = value.trim();
    SpecialPowerType::from_str(token)
        .or_else(|| SpecialPowerType::from_str(&token.to_ascii_uppercase()))
}

fn map_science_from_names(names: &[AsciiString]) -> ScienceType {
    let Some(first) = names.first() else {
        return SCIENCE_INVALID;
    };
    if let Some(store) = get_science_store() {
        return store.get_science_from_internal_name(first.as_str());
    }
    SCIENCE_INVALID
}

fn build_from_ini_special_power(
    name: &AsciiString,
    id: u32,
    ini_template: &IniSpecialPowerTemplate,
) -> SpecialPowerTemplate {
    let mut template = SpecialPowerTemplate::new(name.to_string(), id);
    let props = &ini_template.properties;

    if let Some(enum_value) = props.get("Enum") {
        if let Some(kind) = parse_special_power_enum(enum_value) {
            template.power_type = kind;
        }
    }

    if let Some(reload) = props
        .get("ReloadTime")
        .and_then(|value| parse_duration_frames(value))
    {
        template.reload_time = reload;
    }

    if let Some(science_value) = props.get("RequiredScience") {
        let sciences: Vec<AsciiString> = science_value
            .split_whitespace()
            .map(AsciiString::from)
            .collect();
        template.required_science = map_science_from_names(&sciences);
    } else if !ini_template.required_science.is_empty() {
        template.required_science = map_science_from_names(&ini_template.required_science);
    }

    if let Some(sound) = props.get("InitiateSound") {
        template.initiate_sound = AudioEventRts::new(sound.as_str());
    } else if !ini_template.sound_effect.is_empty() {
        template.initiate_sound = AudioEventRts::new(ini_template.sound_effect.as_str());
    }

    if let Some(sound) = props.get("InitiateAtLocationSound") {
        template.initiate_at_location_sound = AudioEventRts::new(sound.as_str());
    }

    if let Some(value) = props.get("PublicTimer") {
        template.public_timer = parse_bool_property(value);
    }

    if let Some(value) = props.get("SharedSyncedTimer") {
        template.shared_n_sync = parse_bool_property(value);
    } else if !ini_template.shared_sync_group.is_empty() {
        template.shared_n_sync = true;
    }

    if let Some(value) = props.get("ShortcutPower") {
        template.shortcut_power = parse_bool_property(value);
    }

    if let Some(value) = props.get("DetectionTime") {
        if let Some(frames) = parse_duration_frames(value) {
            template.detection_time = frames;
        }
    }

    if let Some(value) = props.get("ViewObjectDuration") {
        if let Some(frames) = parse_duration_frames(value) {
            template.view_object_duration = frames;
        }
    }

    if let Some(value) = props.get("ViewObjectRange") {
        if let Ok(range) = value.parse::<f32>() {
            template.view_object_range = range;
        }
    }

    if let Some(value) = props.get("RadiusCursorRadius") {
        if let Ok(radius) = value.parse::<f32>() {
            template.radius_cursor_radius = radius;
        }
    }

    if let Some(value) = props.get("AcademyClassify") {
        template.academy_classification = parse_academy_classification(value);
    }

    template
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::object::registry::OBJECT_REGISTRY;
    use crate::object::Object;
    use crate::player::{player_list, Player};
    use std::sync::RwLock;

    #[test]
    fn test_special_power_template_creation() {
        let template = SpecialPowerTemplate::new("TestPower".to_string(), 1)
            .with_power_type(SpecialPowerType::CarpetBomb)
            .with_reload_time(3000)
            .with_public_timer(true);

        assert_eq!(template.get_name(), "TestPower");
        assert_eq!(template.get_id(), 1);
        assert_eq!(
            template.get_special_power_type(),
            SpecialPowerType::CarpetBomb
        );
        assert_eq!(template.get_reload_time(), 3000);
        assert!(template.has_public_timer());
    }

    #[test]
    fn test_special_power_store() {
        let mut store = SpecialPowerStore::new();

        let template = SpecialPowerTemplate::new("TestPower".to_string(), 1)
            .with_power_type(SpecialPowerType::NapalmStrike)
            .with_reload_time(5000);

        store.add_template(template);

        assert_eq!(store.get_num_special_powers(), 1);

        let found = store.find_special_power_template("TestPower");
        assert!(found.is_some());
        assert_eq!(
            found.unwrap().get_special_power_type(),
            SpecialPowerType::NapalmStrike
        );

        let found_by_id = store.find_special_power_template_by_id(1);
        assert!(found_by_id.is_some());
    }

    #[test]
    fn can_use_special_power_requires_matching_object_module() {
        OBJECT_REGISTRY.clear();
        player_list().write().unwrap().clear();

        let store = SpecialPowerStore::new();
        let template = SpecialPowerTemplate::new("MissingModulePower".to_string(), 77);
        let object = Arc::new(RwLock::new(Object::new_test(9001, 100.0)));
        OBJECT_REGISTRY.register_object(9001, &object);

        assert!(!store.can_use_special_power(9001, &template));

        OBJECT_REGISTRY.clear();
    }

    #[test]
    fn special_power_store_indexes_in_insertion_order() {
        let mut store = SpecialPowerStore::new();

        store.add_template(SpecialPowerTemplate::new("FirstPower".to_string(), 20));
        store.add_template(SpecialPowerTemplate::new("SecondPower".to_string(), 10));
        store.add_template(SpecialPowerTemplate::new("ThirdPower".to_string(), 30));

        assert_eq!(
            store
                .get_special_power_template_by_index(0)
                .map(SpecialPowerTemplate::get_name),
            Some("FirstPower")
        );
        assert_eq!(
            store
                .get_special_power_template_by_index(1)
                .map(SpecialPowerTemplate::get_name),
            Some("SecondPower")
        );
        assert_eq!(
            store
                .get_special_power_template_by_index(2)
                .map(SpecialPowerTemplate::get_name),
            Some("ThirdPower")
        );
        assert!(store.get_special_power_template_by_index(3).is_none());
    }

    #[test]
    fn ini_special_power_radius_cursor_defaults_to_zero_when_absent() {
        let mut ini_template = IniSpecialPowerTemplate::new(AsciiString::from("NoCursorRadius"));
        ini_template
            .properties
            .insert("ReloadTime".to_string(), "1000".to_string());
        ini_template.radius = 50.0;

        let template =
            build_from_ini_special_power(&AsciiString::from("NoCursorRadius"), 1, &ini_template);

        assert_eq!(template.get_radius_cursor_radius(), 0.0);
    }

    #[test]
    fn ini_special_power_radius_cursor_uses_explicit_cpp_field() {
        let mut ini_template = IniSpecialPowerTemplate::new(AsciiString::from("HasCursorRadius"));
        ini_template
            .properties
            .insert("RadiusCursorRadius".to_string(), "60".to_string());
        ini_template.radius = 50.0;

        let template =
            build_from_ini_special_power(&AsciiString::from("HasCursorRadius"), 2, &ini_template);

        assert_eq!(template.get_radius_cursor_radius(), 60.0);
    }

    #[test]
    fn ini_special_power_generic_fields_do_not_populate_cpp_template_fields() {
        let mut ini_template = IniSpecialPowerTemplate::new(AsciiString::from("GenericOnly"));
        ini_template.recharge_time = 30.0;
        ini_template.range = 100.0;
        ini_template.view_object_duration = 10.0;

        let template =
            build_from_ini_special_power(&AsciiString::from("GenericOnly"), 3, &ini_template);

        assert_eq!(template.get_reload_time(), 0);
        assert_eq!(template.get_view_object_duration(), 0);
        assert_eq!(template.get_view_object_range(), 0.0);
    }

    #[test]
    fn ini_special_power_cpp_fields_populate_timing_and_view_fields() {
        let mut ini_template = IniSpecialPowerTemplate::new(AsciiString::from("CppFields"));
        ini_template
            .properties
            .insert("ReloadTime".to_string(), "3000".to_string());
        ini_template
            .properties
            .insert("ViewObjectDuration".to_string(), "1.5s".to_string());
        ini_template
            .properties
            .insert("ViewObjectRange".to_string(), "250".to_string());
        ini_template.recharge_time = 30.0;
        ini_template.range = 100.0;

        let template =
            build_from_ini_special_power(&AsciiString::from("CppFields"), 4, &ini_template);

        assert_eq!(template.get_reload_time(), 90);
        assert_eq!(template.get_view_object_duration(), 45);
        assert_eq!(template.get_view_object_range(), 250.0);
    }

    #[test]
    fn test_audio_event() {
        let mut event = AudioEventRts::new("TestSound");
        event.set_object_id(42);
        event.set_position(&(100.0, 200.0, 50.0));
        event.set_player_index(1);

        assert_eq!(event.event_name, "TestSound");
        assert_eq!(event.object_id, 42);
        assert_eq!(event.position, Some((100.0, 200.0, 50.0)));
        assert_eq!(event.player_index, Some(1));
    }

    #[test]
    fn parse_duration_frames_uses_canonical_duration_parser() {
        assert_eq!(parse_duration_frames("1500ms"), Some(45));
        assert_eq!(parse_duration_frames("1.5s"), Some(45));
        assert_eq!(parse_duration_frames(""), None);
    }

    #[test]
    fn parse_academy_classification_accepts_cpp_tokens() {
        assert_eq!(
            parse_academy_classification("ACT_SUPERPOWER"),
            AcademyClassificationType::Superweapon
        );
        assert_eq!(
            parse_academy_classification("SUPERPOWER"),
            AcademyClassificationType::Superweapon
        );
    }
}
