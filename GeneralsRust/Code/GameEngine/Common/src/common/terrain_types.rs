//! Terrain type definitions matching C++ TerrainTypes.h / TerrainTypes.cpp
//!
//! TerrainType describes terrain surface properties (name, texture, class, blend edges).
//! TerrainTypeCollection is a linked-list collection of all terrain types.
//! TheTerrainTypes is the global singleton.

use crate::common::rts::AsciiString;
use crate::common::system::{SubsystemInterface, SubsystemResult, SubsystemState};
use std::sync::{Arc, Mutex, OnceLock};

/// Terrain class enumeration matching C++ TerrainClass enum.
/// Must be kept in sync with terrainTypeNames[].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[repr(u32)]
pub enum TerrainClass {
    #[default]
    None = 0,
    Desert1,
    Desert2,
    Desert3,
    EasternEurope1,
    EasternEurope2,
    EasternEurope3,
    Swiss1,
    Swiss2,
    Swiss3,
    Snow1,
    Snow2,
    Snow3,
    Dirt,
    Grass,
    Transition,
    Rock,
    Sand,
    Cliff,
    Wood,
    BlendEdges,
    LiveDesert,
    DryDesert,
    AccentSand,
    TropicalBeach,
    BeachPark,
    RuggedMountain,
    CobblestoneGrass,
    AccentGrass,
    Residential,
    RuggedSnow,
    FlatSnow,
    Field,
    Asphalt,
    Concrete,
    China,
    AccentRock,
    Urban,
    NumClasses,
}

/// Terrain class name table matching C++ terrainTypeNames[].
pub const TERRAIN_TYPE_NAMES: &[&str] = &[
    "NONE",
    "DESERT_1",
    "DESERT_2",
    "DESERT_3",
    "EASTERN_EUROPE_1",
    "EASTERN_EUROPE_2",
    "EASTERN_EUROPE_3",
    "SWISS_1",
    "SWISS_2",
    "SWISS_3",
    "SNOW_1",
    "SNOW_2",
    "SNOW_3",
    "DIRT",
    "GRASS",
    "TRANSITION",
    "ROCK",
    "SAND",
    "CLIFF",
    "WOOD",
    "BLEND_EDGE",
    "DESERT_LIVE",
    "DESERT_DRY",
    "SAND_ACCENT",
    "BEACH_TROPICAL",
    "BEACH_PARK",
    "MOUNTAIN_RUGGED",
    "GRASS_COBBLESTONE",
    "GRASS_ACCENT",
    "RESIDENTIAL",
    "SNOW_RUGGED",
    "SNOW_FLAT",
    "FIELD",
    "ASPHALT",
    "CONCRETE",
    "CHINA",
    "ROCK_ACCENT",
    "URBAN",
];

/// Look up a TerrainClass by name string.
pub fn terrain_class_from_name(name: &str) -> Option<TerrainClass> {
    TERRAIN_TYPE_NAMES
        .iter()
        .position(|&n| name.eq_ignore_ascii_case(n))
        .and_then(|i| {
            if i < TerrainClass::NumClasses as usize {
                Some(unsafe { std::mem::transmute(i as u32) })
            } else {
                None
            }
        })
}

/// Get the name string for a TerrainClass.
pub fn terrain_class_name(class: TerrainClass) -> &'static str {
    let idx = class as usize;
    if idx < TERRAIN_TYPE_NAMES.len() {
        TERRAIN_TYPE_NAMES[idx]
    } else {
        "NONE"
    }
}

/// A single terrain type definition.
/// Matches C++ TerrainType from TerrainTypes.h.
#[derive(Debug, Clone)]
pub struct TerrainType {
    /// Terrain entry name
    name: AsciiString,
    /// Texture.tga file for terrain
    texture: AsciiString,
    /// Whether this terrain contains custom blend edges
    blend_edge_texture: bool,
    /// Type classification
    class: TerrainClass,
    /// Do not allow construction on this terrain tile
    restrict_construction: bool,
}

impl TerrainType {
    pub fn new() -> Self {
        Self {
            name: AsciiString::new(),
            texture: AsciiString::new(),
            blend_edge_texture: false,
            class: TerrainClass::None,
            restrict_construction: false,
        }
    }

    /// Get the name for this terrain.
    pub fn get_name(&self) -> &AsciiString {
        &self.name
    }

    /// Get the texture file for this terrain.
    pub fn get_texture(&self) -> &AsciiString {
        &self.texture
    }

    /// Get whether this terrain is blend edge terrain.
    pub fn is_blend_edge(&self) -> bool {
        self.blend_edge_texture
    }

    /// Get the type classification of this terrain.
    pub fn get_class(&self) -> TerrainClass {
        self.class
    }

    /// Get construction restriction flag.
    pub fn get_restrict_construction(&self) -> bool {
        self.restrict_construction
    }

    /// Set the name (for terrain collection use).
    pub fn set_name(&mut self, name: AsciiString) {
        self.name = name;
    }

    /// Set the texture (for terrain collection use).
    pub fn set_texture(&mut self, texture: AsciiString) {
        self.texture = texture;
    }

    /// Set the blend edge flag (for terrain collection use).
    pub fn set_blend_edge(&mut self, is_blend: bool) {
        self.blend_edge_texture = is_blend;
    }

    /// Set the class (for terrain collection use).
    pub fn set_class(&mut self, class: TerrainClass) {
        self.class = class;
    }

    /// Set the restrict construction flag (for terrain collection use).
    pub fn set_restrict_construction(&mut self, restrict: bool) {
        self.restrict_construction = restrict;
    }
}

impl Default for TerrainType {
    fn default() -> Self {
        Self::new()
    }
}

/// Collection of all terrain types.
/// Matches C++ TerrainTypeCollection from TerrainTypes.h.
#[derive(Debug)]
pub struct TerrainTypeCollection {
    /// All terrain types indexed by name (case-insensitive).
    terrain_map: std::collections::HashMap<String, TerrainType>,
}

impl TerrainTypeCollection {
    pub fn new() -> Self {
        Self {
            terrain_map: std::collections::HashMap::new(),
        }
    }

    /// Initialize the collection (called once at engine startup).
    pub fn init(&mut self) {
        self.terrain_map.clear();
    }

    /// Reset the collection (not used for terrain, kept for SubsystemInterface).
    pub fn reset(&mut self) {
        // Terrain types persist across resets.
    }

    /// Update the collection (not used for terrain, kept for SubsystemInterface).
    pub fn update(&mut self) {
        // No per-frame updates needed.
    }

    /// Find a terrain type by name (case-insensitive).
    pub fn find_terrain(&self, name: &str) -> Option<&TerrainType> {
        self.terrain_map.get(&name.to_ascii_lowercase())
    }

    /// Allocate a new terrain type and register it.
    /// Copies defaults from "DefaultTerrain" if it exists (matching C++ behavior).
    pub fn new_terrain(&mut self, name: AsciiString) -> &mut TerrainType {
        let key = name.as_str().to_ascii_lowercase();

        // Copy defaults from the default terrain entry if present
        if let Some(default) = self.terrain_map.get("defaultterrain") {
            let mut terrain = default.clone();
            terrain.name = name;
            self.terrain_map.insert(key.clone(), terrain);
        } else {
            let mut terrain = TerrainType::new();
            terrain.name = name;
            self.terrain_map.insert(key.clone(), terrain);
        }

        self.terrain_map.get_mut(&key).unwrap()
    }

    /// Iterate over all terrain types.
    pub fn iter(&self) -> impl Iterator<Item = &TerrainType> {
        self.terrain_map.values()
    }

    /// Number of terrain types.
    pub fn len(&self) -> usize {
        self.terrain_map.len()
    }

    /// Check if empty.
    pub fn is_empty(&self) -> bool {
        self.terrain_map.is_empty()
    }
}

impl Default for TerrainTypeCollection {
    fn default() -> Self {
        Self::new()
    }
}

impl SubsystemInterface for TerrainTypeCollection {
    fn name(&self) -> &str {
        "TerrainTypeCollection"
    }

    fn init(&mut self) -> SubsystemResult<()> {
        TerrainTypeCollection::init(self);
        Ok(())
    }

    fn reset(&mut self) -> SubsystemResult<()> {
        TerrainTypeCollection::reset(self);
        Ok(())
    }

    fn update(&mut self, _delta_time: std::time::Duration) -> SubsystemResult<()> {
        TerrainTypeCollection::update(self);
        Ok(())
    }

    fn shutdown(&mut self) -> SubsystemResult<()> {
        self.terrain_map.clear();
        Ok(())
    }

    fn state(&self) -> SubsystemState {
        SubsystemState::Running
    }

    fn as_any(&self) -> &(dyn std::any::Any + Send + Sync) {
        self
    }

    fn as_any_mut(&mut self) -> &mut (dyn std::any::Any + Send + Sync) {
        self
    }
}

/// Global terrain type collection singleton (matches C++ `TheTerrainTypes`).
static THE_TERRAIN_TYPES: OnceLock<Arc<Mutex<TerrainTypeCollection>>> = OnceLock::new();

/// Get the global terrain type collection.
pub fn get_terrain_types() -> Option<Arc<Mutex<TerrainTypeCollection>>> {
    THE_TERRAIN_TYPES.get().cloned()
}

/// Initialize the global terrain type collection.
pub fn init_terrain_types() -> Arc<Mutex<TerrainTypeCollection>> {
    THE_TERRAIN_TYPES
        .get_or_init(|| {
            let collection = TerrainTypeCollection::new();
            Arc::new(Mutex::new(collection))
        })
        .clone()
}

/// Shut down the global terrain type collection.
pub fn shutdown_terrain_types() {
    if let Some(arc) = THE_TERRAIN_TYPES.get() {
        if let Ok(mut guard) = arc.lock() {
            guard.terrain_map.clear();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn terrain_class_round_trips() {
        for (i, &name) in TERRAIN_TYPE_NAMES.iter().enumerate() {
            if name == "NONE" || name == "BLEND_EDGE" {
                continue;
            }
            let class = terrain_class_from_name(name).expect(name);
            assert_eq!(class as usize, i);
            assert_eq!(terrain_class_name(class), name);
        }
    }

    #[test]
    fn find_terrain_matches_name() {
        let mut collection = TerrainTypeCollection::new();
        collection.init();

        {
            let t = collection.new_terrain(AsciiString::from("Grass"));
            t.set_class(TerrainClass::Grass);
            t.set_texture(AsciiString::from("grass.tga"));
        }

        assert!(collection.find_terrain("Grass").is_some());
        assert!(collection.find_terrain("grass").is_some());
        assert!(collection.find_terrain("Sand").is_none());
    }

    #[test]
    fn default_terrain_copies_properties() {
        let mut collection = TerrainTypeCollection::new();
        collection.init();

        {
            let default = collection.new_terrain(AsciiString::from("DefaultTerrain"));
            default.set_class(TerrainClass::Dirt);
            default.set_texture(AsciiString::from("default.tga"));
            default.set_blend_edge(true);
        }

        {
            let grass = collection.new_terrain(AsciiString::from("Grass"));
            assert_eq!(grass.get_class(), TerrainClass::Dirt);
            assert_eq!(grass.get_texture().as_str(), "default.tga");
            assert!(grass.is_blend_edge());
        }
    }

    #[test]
    fn global_collection_init_and_find() {
        let arc = init_terrain_types();
        {
            let mut guard = arc.lock().unwrap();
            let t = guard.new_terrain(AsciiString::from("TestTerrain"));
            t.set_class(TerrainClass::Rock);
        }

        let guard = get_terrain_types().unwrap();
        let collection = guard.lock().unwrap();
        let t = collection.find_terrain("TestTerrain").unwrap();
        assert_eq!(t.get_class(), TerrainClass::Rock);

        shutdown_terrain_types();
    }
}
