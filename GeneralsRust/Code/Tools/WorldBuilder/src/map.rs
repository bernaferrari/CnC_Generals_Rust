//! Map data structures and management

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use uuid::Uuid;

/// Represents a complete game map with terrain, objects, and metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Map {
    // Basic info
    id: Uuid,
    name: String,
    description: String,
    author: String,
    version: String,

    // Map dimensions and settings
    width: u32,
    height: u32,
    settings: MapSettings,

    // Terrain data
    heightmap: Vec<Vec<f32>>,
    texture_indices: Vec<Vec<u8>>,

    // Game objects
    objects: Vec<MapObject>,

    // Scripting
    scripts: HashMap<String, String>,
    triggers: Vec<Trigger>,

    // Metadata
    created_time: chrono::DateTime<chrono::Utc>,
    modified_time: chrono::DateTime<chrono::Utc>,
    file_path: Option<PathBuf>,
}

impl Map {
    /// Create a new empty map
    pub fn new(settings: MapSettings) -> Result<Self> {
        let width = settings.width;
        let height = settings.height;

        let mut heightmap = Vec::with_capacity(height as usize);
        let mut texture_indices = Vec::with_capacity(height as usize);

        for _ in 0..height {
            heightmap.push(vec![0.0; width as usize]);
            texture_indices.push(vec![0; width as usize]);
        }

        let now = chrono::Utc::now();

        Ok(Self {
            id: Uuid::new_v4(),
            name: settings.name.clone(),
            description: settings.description.clone(),
            author: settings.author.clone(),
            version: "1.0".to_string(),

            width,
            height,
            settings,

            heightmap,
            texture_indices,

            objects: Vec::new(),
            scripts: HashMap::new(),
            triggers: Vec::new(),

            created_time: now,
            modified_time: now,
            file_path: None,
        })
    }

    /// Load a map from file
    pub async fn load(path: &Path) -> Result<Self> {
        let bytes = tokio::fs::read(path).await?;
        if bytes.starts_with(b"CkMp") {
            let doc = world_builder::MapDocument::read_ckmp(&bytes)
                .map_err(|e| anyhow::anyhow!("CkMp load failed: {e}"))?;
            let mut map = Self::from_map_document(doc, path)?;
            map.file_path = Some(path.to_path_buf());
            return Ok(map);
        }
        let content = String::from_utf8(bytes)?;
        let mut map: Self = serde_json::from_str(&content)?;
        map.file_path = Some(path.to_path_buf());
        Ok(map)
    }

    /// Save the map to a C++-shaped `.map` (CkMp HeightMapData + ObjectsList).
    pub async fn save(&mut self, path: &Path) -> Result<()> {
        self.modified_time = chrono::Utc::now();
        self.file_path = Some(path.to_path_buf());
        let doc = self.to_map_document()?;
        doc.save_to_path(path)
            .map_err(|e| anyhow::anyhow!("CkMp save failed: {e}"))?;
        log::info!("Map saved to: {}", path.display());
        Ok(())
    }

    fn to_map_document(&self) -> Result<world_builder::MapDocument> {
        use world_builder::{HeightMapEdit, MapDocument, MapObjectEdit};

        let w = self.width as i32;
        let h = self.height as i32;
        let mut height = HeightMapEdit::new(w, h, 0)?;
        for y in 0..self.height {
            for x in 0..self.width {
                let sample = self.heightmap[y as usize][x as usize].clamp(0.0, 255.0) as u8;
                height.set_height(x as i32, y as i32, sample)?;
            }
        }
        let mut doc = MapDocument::new(height);
        for object in &self.objects {
            let waypoint_name = object
                .properties
                .get("waypointName")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string())
                .or_else(|| {
                    if object.object_type.to_lowercase().contains("waypoint") {
                        Some(object.name.clone())
                    } else {
                        None
                    }
                });
            let name = if object.object_type.is_empty() {
                object.name.clone()
            } else {
                object.object_type.clone()
            };
            doc.objects.push(MapObjectEdit {
                name,
                x: object.position.x,
                y: object.position.z,
                z: object.position.y,
                angle: object.rotation.to_euler(glam::EulerRot::YXZ).0,
                flags: 0,
                properties: game_engine::common::dict::Dict::new(),
                waypoint_name,
            });
        }
        Ok(doc)
    }

    fn from_map_document(doc: world_builder::MapDocument, path: &Path) -> Result<Self> {
        let mut settings = MapSettings::default();
        settings.width = doc.heightmap.width.max(0) as u32;
        settings.height = doc.heightmap.height.max(0) as u32;
        settings.name = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("Map")
            .to_string();
        let mut map = Map::new(settings)?;
        for y in 0..doc.heightmap.height {
            for x in 0..doc.heightmap.width {
                if let Some(h) = doc.heightmap.get_height(x, y) {
                    map.set_height(x as u32, y as u32, h as f32);
                }
            }
        }
        for obj in doc.objects {
            let mut placed = MapObject::new(
                obj.waypoint_name
                    .clone()
                    .unwrap_or_else(|| obj.name.clone()),
                obj.name,
                glam::Vec3::new(obj.x, obj.z, obj.y),
            );
            if let Some(wp) = obj.waypoint_name {
                placed
                    .properties
                    .insert("waypointName".into(), serde_json::Value::String(wp));
            }
            map.add_object(placed);
        }
        Ok(map)
    }

    /// Get map dimensions
    pub fn width(&self) -> u32 {
        self.width
    }
    pub fn height(&self) -> u32 {
        self.height
    }

    /// Get map name
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Set map name
    pub fn set_name(&mut self, name: String) {
        self.name = name;
        self.modified_time = chrono::Utc::now();
    }

    /// Get heightmap data
    pub fn heightmap(&self) -> &Vec<Vec<f32>> {
        &self.heightmap
    }

    /// Get mutable heightmap data
    pub fn heightmap_mut(&mut self) -> &mut Vec<Vec<f32>> {
        self.modified_time = chrono::Utc::now();
        &mut self.heightmap
    }

    /// Get height at specific coordinates
    pub fn get_height(&self, x: u32, y: u32) -> Option<f32> {
        if x < self.width && y < self.height {
            Some(self.heightmap[y as usize][x as usize])
        } else {
            None
        }
    }

    /// Set height at specific coordinates
    pub fn set_height(&mut self, x: u32, y: u32, height: f32) {
        if x < self.width && y < self.height {
            self.heightmap[y as usize][x as usize] = height;
            self.modified_time = chrono::Utc::now();
        }
    }

    /// Get texture indices
    pub fn texture_indices(&self) -> &Vec<Vec<u8>> {
        &self.texture_indices
    }

    /// Get mutable texture indices
    pub fn texture_indices_mut(&mut self) -> &mut Vec<Vec<u8>> {
        self.modified_time = chrono::Utc::now();
        &mut self.texture_indices
    }

    /// Get texture index at coordinates
    pub fn get_texture_index(&self, x: u32, y: u32) -> Option<u8> {
        if x < self.width && y < self.height {
            Some(self.texture_indices[y as usize][x as usize])
        } else {
            None
        }
    }

    /// Set texture index at coordinates
    pub fn set_texture_index(&mut self, x: u32, y: u32, index: u8) {
        if x < self.width && y < self.height {
            self.texture_indices[y as usize][x as usize] = index;
            self.modified_time = chrono::Utc::now();
        }
    }

    /// Add a map object
    pub fn add_object(&mut self, object: MapObject) {
        self.objects.push(object);
        self.modified_time = chrono::Utc::now();
    }

    /// Remove a map object by ID
    pub fn remove_object(&mut self, id: Uuid) -> bool {
        let initial_len = self.objects.len();
        self.objects.retain(|obj| obj.id != id);

        if self.objects.len() != initial_len {
            self.modified_time = chrono::Utc::now();
            true
        } else {
            false
        }
    }

    /// Get all objects
    pub fn objects(&self) -> &[MapObject] {
        &self.objects
    }

    /// Get mutable reference to all objects
    pub fn objects_mut(&mut self) -> &mut Vec<MapObject> {
        self.modified_time = chrono::Utc::now();
        &mut self.objects
    }

    /// Find object by ID
    pub fn find_object(&self, id: Uuid) -> Option<&MapObject> {
        self.objects.iter().find(|obj| obj.id == id)
    }

    /// Find mutable object by ID
    pub fn find_object_mut(&mut self, id: Uuid) -> Option<&mut MapObject> {
        self.modified_time = chrono::Utc::now();
        self.objects.iter_mut().find(|obj| obj.id == id)
    }

    /// Get map settings
    pub fn settings(&self) -> &MapSettings {
        &self.settings
    }

    /// Update map settings
    pub fn set_settings(&mut self, settings: MapSettings) {
        self.settings = settings;
        self.modified_time = chrono::Utc::now();
    }

    /// Add or update a script
    pub fn set_script(&mut self, name: String, content: String) {
        self.scripts.insert(name, content);
        self.modified_time = chrono::Utc::now();
    }

    /// Get a script
    pub fn get_script(&self, name: &str) -> Option<&str> {
        self.scripts.get(name).map(|s| s.as_str())
    }

    /// Remove a script
    pub fn remove_script(&mut self, name: &str) -> bool {
        let removed = self.scripts.remove(name).is_some();
        if removed {
            self.modified_time = chrono::Utc::now();
        }
        removed
    }

    /// Get all script names
    pub fn script_names(&self) -> Vec<&String> {
        self.scripts.keys().collect()
    }

    /// Add a trigger
    pub fn add_trigger(&mut self, trigger: Trigger) {
        self.triggers.push(trigger);
        self.modified_time = chrono::Utc::now();
    }

    /// Get all triggers
    pub fn triggers(&self) -> &[Trigger] {
        &self.triggers
    }

    /// Validate the map for errors
    pub fn validate(&self) -> Vec<ValidationError> {
        let mut errors = Vec::new();

        // Check map dimensions
        if self.width == 0 || self.height == 0 {
            errors.push(ValidationError::InvalidDimensions);
        }

        // Check heightmap consistency
        if self.heightmap.len() != self.height as usize {
            errors.push(ValidationError::HeightmapSizeMismatch);
        }

        // Check for objects outside map bounds
        for object in &self.objects {
            if object.position.x < 0.0
                || object.position.x >= self.width as f32
                || object.position.z < 0.0
                || object.position.z >= self.height as f32
            {
                errors.push(ValidationError::ObjectOutOfBounds {
                    object_id: object.id,
                    position: object.position,
                });
            }
        }

        errors
    }

    /// Get map statistics
    pub fn statistics(&self) -> MapStatistics {
        MapStatistics {
            total_objects: self.objects.len(),
            terrain_area: self.width * self.height,
            min_height: self
                .heightmap
                .iter()
                .flat_map(|row| row.iter())
                .fold(f32::INFINITY, |a, &b| a.min(b)),
            max_height: self
                .heightmap
                .iter()
                .flat_map(|row| row.iter())
                .fold(f32::NEG_INFINITY, |a, &b| a.max(b)),
            script_count: self.scripts.len(),
            trigger_count: self.triggers.len(),
        }
    }
}

/// Map creation and configuration settings
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MapSettings {
    pub name: String,
    pub description: String,
    pub author: String,
    pub width: u32,
    pub height: u32,
    pub max_players: u8,
    pub recommended_players: u8,
    pub game_mode: GameMode,
    pub environment: EnvironmentType,
    pub weather: WeatherType,
    pub time_of_day: TimeOfDay,
}

impl Default for MapSettings {
    fn default() -> Self {
        Self {
            name: "New Map".to_string(),
            description: "A new map for Command & Conquer".to_string(),
            author: "World Builder".to_string(),
            width: 256,
            height: 256,
            max_players: 8,
            recommended_players: 4,
            game_mode: GameMode::Skirmish,
            environment: EnvironmentType::Temperate,
            weather: WeatherType::Clear,
            time_of_day: TimeOfDay::Day,
        }
    }
}

/// Game mode types
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub enum GameMode {
    Skirmish,
    Campaign,
    Multiplayer,
    Custom,
}

/// Environment/biome types
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub enum EnvironmentType {
    Temperate,
    Desert,
    Snow,
    Urban,
    Tropical,
    Wasteland,
}

/// Weather conditions
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub enum WeatherType {
    Clear,
    Rain,
    Snow,
    Fog,
    Storm,
    Sandstorm,
}

/// Time of day
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub enum TimeOfDay {
    Dawn,
    Day,
    Dusk,
    Night,
}

/// Object placed on the map
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MapObject {
    pub id: Uuid,
    pub name: String,
    pub object_type: String,
    pub position: glam::Vec3,
    pub rotation: glam::Quat,
    pub scale: glam::Vec3,
    pub properties: HashMap<String, serde_json::Value>,
    pub player_owner: Option<u8>,
    pub team: Option<u8>,
    pub enabled: bool,
}

impl MapObject {
    pub fn new(name: String, object_type: String, position: glam::Vec3) -> Self {
        Self {
            id: Uuid::new_v4(),
            name,
            object_type,
            position,
            rotation: glam::Quat::IDENTITY,
            scale: glam::Vec3::ONE,
            properties: HashMap::new(),
            player_owner: None,
            team: None,
            enabled: true,
        }
    }
}

/// Script trigger for map events
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Trigger {
    pub id: Uuid,
    pub name: String,
    pub condition: TriggerCondition,
    pub actions: Vec<TriggerAction>,
    pub enabled: bool,
    pub once_only: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TriggerCondition {
    PlayerEntersArea { player: u8, area: Area },
    ObjectDestroyed { object_id: Uuid },
    TimerExpired { seconds: f32 },
    CustomScript { script: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TriggerAction {
    ShowMessage {
        text: String,
        duration: f32,
    },
    SpawnObject {
        object_type: String,
        position: glam::Vec3,
    },
    PlaySound {
        sound_name: String,
    },
    RunScript {
        script_name: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Area {
    pub min: glam::Vec2,
    pub max: glam::Vec2,
}

/// Map validation errors
#[derive(Debug, Clone)]
pub enum ValidationError {
    InvalidDimensions,
    HeightmapSizeMismatch,
    ObjectOutOfBounds {
        object_id: Uuid,
        position: glam::Vec3,
    },
    MissingTextures,
    InvalidScript {
        script_name: String,
        error: String,
    },
}

/// Map statistics
#[derive(Debug, Clone)]
pub struct MapStatistics {
    pub total_objects: usize,
    pub terrain_area: u32,
    pub min_height: f32,
    pub max_height: f32,
    pub script_count: usize,
    pub trigger_count: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn editor_save_writes_ckmp_map_like_cpp_save_to_file() {
        let mut settings = MapSettings::default();
        settings.width = 8;
        settings.height = 8;
        let mut map = Map::new(settings).unwrap();
        map.set_height(1, 2, 55.0);
        map.add_object(MapObject::new(
            "Player_1_Start".into(),
            "*Waypoints/Waypoint".into(),
            glam::Vec3::new(10.0, 0.0, 20.0),
        ));
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("EditorSave.map");
        tokio::runtime::Runtime::new()
            .unwrap()
            .block_on(map.save(&path))
            .unwrap();
        let bytes = std::fs::read(&path).unwrap();
        assert!(bytes.starts_with(b"CkMp"));
        assert!(
            bytes
                .windows(b"HeightMapData".len())
                .any(|w| w == b"HeightMapData")
        );
        assert!(
            bytes
                .windows(b"ObjectsList".len())
                .any(|w| w == b"ObjectsList")
        );
        let loaded = world_builder::MapDocument::load_from_path(&path).unwrap();
        assert_eq!(loaded.heightmap.get_height(1, 2), Some(55));
        assert_eq!(
            loaded.objects[0].waypoint_name.as_deref(),
            Some("Player_1_Start")
        );
    }
}
