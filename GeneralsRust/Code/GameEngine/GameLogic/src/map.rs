//! Map System - Rust Implementation
//!
//! Map object management and height map functionality.
//! Based on MapObject.h and related C++ implementations.
//!
//! This module provides:
//! - MapObject management for static map elements
//! - Height map interface for terrain height queries
//! - Map loading and coordinate transformation
//! - Strategic point and landmark management

use crate::common::*;
use crate::object::*;
use bitflags::bitflags;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

pub mod map_loader;
pub mod object_placer;
pub mod polygon_trigger;
pub mod sides_list;
pub mod terrain_loader;
pub mod terrain_logic;

pub use map_loader::*;
pub use object_placer::*;
pub use polygon_trigger::*;
pub use sides_list::*;
pub use terrain_loader::*;

// Map object flags (matching C++ definitions)
bitflags! {
    /// Map object flags
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct MapObjectFlags: u32 {
        const DRAWS_IN_MIRROR = 0x00000001;          // If set, draws in water mirror
        const ROAD_POINT1 = 0x00000002;              // If set, is the first point in a road segment
        const ROAD_POINT2 = 0x00000004;              // If set, is the second point in a road segment
        const ROAD_CORNER_ANGLED = 0x00000008;       // If set, the road corner is angled rather than curved
        const BRIDGE_POINT1 = 0x00000010;            // If set, is the first point in a bridge
        const BRIDGE_POINT2 = 0x00000020;            // If set, is the second point in a bridge
        const ROAD_CORNER_TIGHT = 0x00000040;        // Tight road corner
        const ROAD_JOIN = 0x00000080;                // If set, this road end does a generic alpha join
        const DONT_RENDER = 0x00000100;              // If set, do not render this object
    }
}

impl MapObjectFlags {
    pub const ROAD_FLAGS: MapObjectFlags =
        MapObjectFlags::ROAD_POINT1.union(MapObjectFlags::ROAD_POINT2);
    pub const BRIDGE_FLAGS: MapObjectFlags =
        MapObjectFlags::BRIDGE_POINT1.union(MapObjectFlags::BRIDGE_POINT2);
}

/// Runtime flags for map objects (used by World Builder)
bitflags! {
    /// Map object runtime flags
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct MapObjectRuntimeFlags: u32 {
        const SELECTED = 0x01;
        const LIGHT = 0x02;
        const WAYPOINT = 0x04;
        const SCORCH = 0x08;
    }
}

/// Map object class encapsulating static map elements
#[derive(Debug)]
pub struct MapObject {
    /// Location of the center of the object
    location: Coord3D,
    /// The object name
    object_name: AsciiString,
    /// Thing template for map object
    thing_template: Option<Arc<dyn ThingTemplate>>,
    /// Angle (positive x is 0 degrees, counterclockwise)
    angle: f32,
    /// Bit flags
    flags: MapObjectFlags,
    /// General property sheet
    properties: HashMap<String, String>,
    /// Display color (runtime data)
    color: u32,
    /// Runtime flags (not saved in map file)
    runtime_flags: MapObjectRuntimeFlags,
}

impl MapObject {
    pub fn new(
        location: Coord3D,
        name: AsciiString,
        angle: f32,
        flags: MapObjectFlags,
        properties: Option<HashMap<String, String>>,
        thing_template: Option<Arc<dyn ThingTemplate>>,
    ) -> Self {
        Self {
            location,
            object_name: name,
            thing_template,
            angle: normalize_angle(angle),
            flags,
            properties: properties.unwrap_or_default(),
            color: 0xFFFFFFFF, // White by default
            runtime_flags: MapObjectRuntimeFlags::empty(),
        }
    }

    /// Get the object's property sheet
    pub fn get_properties(&self) -> &HashMap<String, String> {
        &self.properties
    }

    /// Get mutable properties
    pub fn get_properties_mut(&mut self) -> &mut HashMap<String, String> {
        &mut self.properties
    }

    /// Get the center point
    pub fn get_location(&self) -> &Coord3D {
        &self.location
    }

    /// Get the angle
    pub fn get_angle(&self) -> f32 {
        self.angle
    }

    /// Get the UI color
    pub fn get_color(&self) -> u32 {
        self.color
    }

    /// Set the UI color
    pub fn set_color(&mut self, color: u32) {
        self.color = color;
    }

    /// Get the object name
    pub fn get_name(&self) -> &AsciiString {
        &self.object_name
    }

    /// Set the object name
    pub fn set_name(&mut self, name: AsciiString) {
        self.object_name = name;
    }

    /// Set thing template
    pub fn set_thing_template(&mut self, thing: Option<Arc<dyn ThingTemplate>>) {
        self.thing_template = thing;
    }

    /// Get thing template
    pub fn get_thing_template(&self) -> Option<&Arc<dyn ThingTemplate>> {
        self.thing_template.as_ref()
    }

    /// Duplicate this map object
    pub fn duplicate(&self) -> Box<MapObject> {
        Box::new(MapObject {
            location: self.location,
            object_name: self.object_name.clone(),
            thing_template: self.thing_template.clone(),
            angle: self.angle,
            flags: self.flags,
            properties: self.properties.clone(),
            color: self.color,
            runtime_flags: self.runtime_flags,
        })
    }

    /// Set angle
    pub fn set_angle(&mut self, angle: f32) {
        self.angle = normalize_angle(angle);
    }

    /// Set location
    pub fn set_location(&mut self, location: &Coord3D) {
        self.location = *location;
    }

    /// Set flag
    pub fn set_flag(&mut self, flag: MapObjectFlags) {
        self.flags |= flag;
    }

    /// Clear flag
    pub fn clear_flag(&mut self, flag: MapObjectFlags) {
        self.flags &= !flag;
    }

    /// Get flag
    pub fn get_flag(&self, flag: MapObjectFlags) -> bool {
        self.flags.contains(flag)
    }

    /// Get all flags
    pub fn get_flags(&self) -> MapObjectFlags {
        self.flags
    }

    /// Check if selected
    pub fn is_selected(&self) -> bool {
        self.runtime_flags.contains(MapObjectRuntimeFlags::SELECTED)
    }

    /// Set selected state
    pub fn set_selected(&mut self, selected: bool) {
        if selected {
            self.runtime_flags |= MapObjectRuntimeFlags::SELECTED;
        } else {
            self.runtime_flags &= !MapObjectRuntimeFlags::SELECTED;
        }
    }

    /// Check if light
    pub fn is_light(&self) -> bool {
        self.runtime_flags.contains(MapObjectRuntimeFlags::LIGHT)
    }

    /// Check if waypoint
    pub fn is_waypoint(&self) -> bool {
        self.runtime_flags.contains(MapObjectRuntimeFlags::WAYPOINT)
    }

    /// Check if scorch mark
    pub fn is_scorch(&self) -> bool {
        self.runtime_flags.contains(MapObjectRuntimeFlags::SCORCH)
    }

    /// Set as light
    pub fn set_is_light(&mut self) {
        self.runtime_flags |= MapObjectRuntimeFlags::LIGHT;
    }

    /// Set as waypoint
    pub fn set_is_waypoint(&mut self) {
        self.runtime_flags |= MapObjectRuntimeFlags::WAYPOINT;
    }

    /// Set as scorch mark
    pub fn set_is_scorch(&mut self) {
        self.runtime_flags |= MapObjectRuntimeFlags::SCORCH;
    }

    /// Get waypoint ID
    pub fn get_waypoint_id(&self) -> WaypointID {
        self.properties
            .get("waypointID")
            .and_then(|s| s.parse().ok())
            .unwrap_or(INVALID_WAYPOINT_ID)
    }

    /// Get waypoint name
    pub fn get_waypoint_name(&self) -> AsciiString {
        self.properties
            .get("waypointName")
            .cloned()
            .unwrap_or_default()
            .into()
    }

    /// Set waypoint ID
    pub fn set_waypoint_id(&mut self, id: WaypointID) {
        self.properties
            .insert("waypointID".to_string(), id.to_string());
    }

    /// Set waypoint name
    pub fn set_waypoint_name(&mut self, name: AsciiString) {
        self.properties
            .insert("waypointName".to_string(), name.to_string());
    }

    /// Validate the map object
    pub fn validate(&mut self) {
        self.verify_valid_team();
        self.verify_valid_unique_id();
    }

    /// Verify valid team assignment
    pub fn verify_valid_team(&mut self) {
        let owner_key = "owner";
        let original_owner_key = "originalOwner";

        let owner = self
            .properties
            .get(owner_key)
            .map(|value| value.trim())
            .filter(|value| !value.is_empty())
            .map(|value| value.to_string());

        let original_owner = self
            .properties
            .get(original_owner_key)
            .map(|value| value.trim())
            .filter(|value| !value.is_empty())
            .map(|value| value.to_string());

        match (owner, original_owner) {
            (Some(owner_value), None) => {
                self.properties
                    .insert(original_owner_key.to_string(), owner_value);
            }
            (None, Some(original_value)) => {
                self.properties
                    .insert(owner_key.to_string(), original_value);
            }
            (None, None) => {
                let default_owner = "PlyrCivilian".to_string();
                self.properties
                    .insert(owner_key.to_string(), default_owner.clone());
                self.properties
                    .insert(original_owner_key.to_string(), default_owner);
            }
            _ => {}
        }
    }

    /// Verify unique ID
    pub fn verify_valid_unique_id(&mut self) {
        let unique_id_key = "uniqueID";
        let valid = self
            .properties
            .get(unique_id_key)
            .and_then(|value| value.parse::<u32>().ok())
            .filter(|value| *value != 0)
            .is_some();

        if !valid {
            self.properties
                .insert(unique_id_key.to_string(), "0".to_string());
        }
    }
}

/// World height map interface for terrain height queries
pub trait WorldHeightMapInterface {
    /// Get border size
    fn get_border_size(&self) -> i32;

    /// Get seismic Z velocity at grid position
    fn get_seismic_z_velocity(&self, x_index: i32, y_index: i32) -> f32;

    /// Set seismic Z velocity at grid position
    fn set_seismic_z_velocity(&mut self, x_index: i32, y_index: i32, value: f32);

    /// Get bilinear sampled seismic Z velocity
    fn get_bilinear_sample_seismic_z_velocity(&self, x: i32, y: i32) -> f32;
}

/// Map management system
pub struct MapSystem {
    /// Collection of map objects
    map_objects: Vec<Box<MapObject>>,
    /// World dictionary for global properties
    world_dict: HashMap<String, String>,
    /// Map object counter for unique IDs
    next_object_id: u32,
}

impl MapSystem {
    pub fn new() -> Self {
        Self {
            map_objects: Vec::new(),
            world_dict: HashMap::new(),
            next_object_id: 1,
        }
    }

    /// Get first map object in list
    pub fn get_first_map_object(&self) -> Option<&MapObject> {
        self.map_objects.first().map(|obj| obj.as_ref())
    }

    /// Get mutable first map object
    pub fn get_first_map_object_mut(&mut self) -> Option<&mut MapObject> {
        self.map_objects.first_mut().map(|obj| obj.as_mut())
    }

    /// Add map object to the list
    pub fn add_map_object(&mut self, map_object: Box<MapObject>) {
        self.map_objects.insert(0, map_object);
    }

    /// Remove map object by name
    pub fn remove_map_object(&mut self, name: &str) -> Option<Box<MapObject>> {
        if let Some(index) = self
            .map_objects
            .iter()
            .position(|obj| obj.get_name() == name)
        {
            return Some(self.map_objects.remove(index));
        }
        None
    }

    /// Find map object by name
    pub fn find_map_object(&self, name: &str) -> Option<&MapObject> {
        self.map_objects
            .iter()
            .find(|obj| obj.get_name() == name)
            .map(|obj| obj.as_ref())
    }

    /// Find map object by name (mutable)
    pub fn find_map_object_mut(&mut self, name: &str) -> Option<&mut MapObject> {
        self.map_objects
            .iter_mut()
            .find(|obj| obj.get_name() == name)
            .map(|obj| obj.as_mut())
    }

    /// Get world dictionary
    pub fn get_world_dict(&self) -> &HashMap<String, String> {
        &self.world_dict
    }

    /// Get mutable world dictionary
    pub fn get_world_dict_mut(&mut self) -> &mut HashMap<String, String> {
        &mut self.world_dict
    }

    /// Count map objects with specific owner
    pub fn count_map_objects_with_owner(&self, owner: &str) -> i32 {
        let mut count = 0;
        for obj in &self.map_objects {
            if let Some(obj_owner) = obj.get_properties().get("owner") {
                if obj_owner == owner {
                    count += 1;
                }
            }
        }
        count
    }

    /// Clear all map objects
    pub fn clear(&mut self) {
        self.map_objects.clear();
        self.world_dict.clear();
        self.next_object_id = 1;
    }

    /// Assign unique IDs to all map objects (fast version)
    pub fn fast_assign_all_unique_ids(&mut self) {
        self.next_object_id = 1;
        for obj in &mut self.map_objects {
            obj.get_properties_mut()
                .insert("uniqueID".to_string(), self.next_object_id.to_string());
            self.next_object_id += 1;
        }
    }

    /// Validate all map objects
    pub fn validate_all_map_objects(&mut self) {
        for obj in &mut self.map_objects {
            obj.validate();
        }

        self.ensure_unique_ids();
    }

    fn ensure_unique_ids(&mut self) {
        use std::collections::HashSet;

        let mut seen = HashSet::new();
        let mut invalid_indices = Vec::new();
        let mut max_id = 0u32;

        for (index, obj) in self.map_objects.iter().enumerate() {
            let id = obj
                .get_properties()
                .get("uniqueID")
                .and_then(|value| value.parse::<u32>().ok())
                .unwrap_or(0);

            if id == 0 || !seen.insert(id) {
                invalid_indices.push(index);
                continue;
            }

            if id > max_id {
                max_id = id;
            }
        }

        let mut next_id = max_id.saturating_add(1).max(self.next_object_id);
        for index in invalid_indices {
            if let Some(obj) = self.map_objects.get_mut(index) {
                obj.get_properties_mut()
                    .insert("uniqueID".to_string(), next_id.to_string());
                next_id = next_id.saturating_add(1);
            }
        }

        self.next_object_id = next_id;
    }
}

/// Game state map system for save games
pub struct GameStateMap {
    /// Map data for save games
    map_data: Vec<u8>,
    /// Pristine map filename
    original_filename: AsciiString,
}

impl GameStateMap {
    pub fn new() -> Self {
        Self {
            map_data: Vec::new(),
            original_filename: String::new().into(),
        }
    }

    /// Initialize
    pub fn init(&mut self) {
        // Initialize game state map
    }

    /// Reset
    pub fn reset(&mut self) {
        self.map_data.clear();
        self.original_filename.clear();
    }

    /// Update
    pub fn update(&mut self) {
        // Update game state map
    }

    /// Clear scratch pad maps from save directory
    pub fn clear_scratch_pad_maps(&mut self) {
        // Would clear temporary map files
    }

    /// Set original filename
    pub fn set_original_filename(&mut self, filename: AsciiString) {
        self.original_filename = filename;
    }

    /// Get original filename
    pub fn get_original_filename(&self) -> &AsciiString {
        &self.original_filename
    }
}

// Utility functions

/// Normalize angle to [0, 2π) range
pub fn normalize_angle(angle: f32) -> f32 {
    let two_pi = 2.0 * PI;
    let mut normalized = angle % two_pi;
    if normalized < 0.0 {
        normalized += two_pi;
    }
    normalized
}

use lazy_static::lazy_static;
/// Global map system instance
use std::sync::RwLock;

lazy_static! {
    pub static ref THE_MAP_SYSTEM: Arc<RwLock<MapSystem>> = Arc::new(RwLock::new(MapSystem::new()));
    pub static ref THE_GAME_STATE_MAP: Arc<RwLock<GameStateMap>> =
        Arc::new(RwLock::new(GameStateMap::new()));
}

/// Get global map system instance
pub fn get_map_system() -> &'static Arc<RwLock<MapSystem>> {
    &THE_MAP_SYSTEM
}

/// Get global game state map instance  
pub fn get_game_state_map() -> &'static Arc<RwLock<GameStateMap>> {
    &THE_GAME_STATE_MAP
}

// Aliases to match C++ naming
pub use THE_GAME_STATE_MAP as TheGameStateMap;
pub use THE_MAP_SYSTEM as TheMapSystem;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_normalize_angle() {
        const TOLERANCE: f32 = 1.0e-5;
        assert!((normalize_angle(0.0) - 0.0).abs() < TOLERANCE);
        assert!((normalize_angle(2.0 * PI) - 0.0).abs() < TOLERANCE);
        assert!((normalize_angle(-PI) - PI).abs() < TOLERANCE);
        assert!((normalize_angle(3.0 * PI) - PI).abs() < TOLERANCE);
    }

    #[test]
    fn test_map_object_creation() {
        let location = Coord3D::new(100.0, 200.0, 0.0);
        let name = "TestObject".to_string();
        let angle = PI / 4.0;
        let flags = MapObjectFlags::ROAD_POINT1;

        let obj = MapObject::new(location, name.clone().into(), angle, flags, None, None);

        assert_eq!(*obj.get_location(), location);
        assert_eq!(obj.get_name(), &name);
        assert!((obj.get_angle() - angle).abs() < f32::EPSILON);
        assert!(obj.get_flag(MapObjectFlags::ROAD_POINT1));
        assert!(!obj.get_flag(MapObjectFlags::ROAD_POINT2));
    }

    #[test]
    fn test_map_system() {
        let mut map_system = MapSystem::new();

        let obj1 = MapObject::new(
            Coord3D::new(0.0, 0.0, 0.0),
            "Object1".to_string().into(),
            0.0,
            MapObjectFlags::empty(),
            None,
            None,
        );

        map_system.add_map_object(Box::new(obj1));

        assert!(map_system.find_map_object("Object1").is_some());
        assert!(map_system.find_map_object("NonExistent").is_none());

        let removed = map_system.remove_map_object("Object1");
        assert!(removed.is_some());
        assert!(map_system.find_map_object("Object1").is_none());
    }
}
