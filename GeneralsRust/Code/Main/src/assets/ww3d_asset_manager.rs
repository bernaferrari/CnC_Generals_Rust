////////////////////////////////////////////////////////////////////////////////
//                                                                            //
//  (c) 2001-2003 Electronic Arts Inc.                                       //
//                                                                            //
////////////////////////////////////////////////////////////////////////////////

// WW3D Asset Manager - Port of C++ WW3DAssetManager
// Handles texture and model loading from INI-defined object templates
// Reference: /GeneralsMD/Code/Libraries/Source/WWVegas/WW3D2/assetmgr.cpp

use crate::assets::archive::ArchiveFileSystem;
use crate::assets::ini_parser::{IniParser, ObjectDefinition};
use anyhow::Result;
use log::{debug, info, warn};
use std::collections::{HashMap, HashSet};

/// WW3D Asset Manager - Manages object definitions and their associated assets
/// Equivalent to C++ WW3DAssetManager::Get_Texture() and object template system
pub struct WW3DAssetManager {
    /// Object definitions loaded from INI files
    object_definitions: HashMap<String, ObjectDefinition>,
    /// Lowercase -> canonical name lookup for case-insensitive matches
    normalized_name_lookup: HashMap<String, String>,
    /// Normalized model name -> object names that reference that model
    model_lookup: HashMap<String, Vec<String>>,

    /// INI parser instance
    ini_parser: IniParser,

    /// Texture cache: object_name -> texture_filename
    texture_cache: HashMap<String, String>,

    /// Model cache: object_name -> model_filename
    model_cache: HashMap<String, String>,

    /// Whether INI files have been loaded
    initialized: bool,
}

impl WW3DAssetManager {
    /// Create a new WW3D Asset Manager
    pub fn new() -> Self {
        Self {
            object_definitions: HashMap::new(),
            normalized_name_lookup: HashMap::new(),
            model_lookup: HashMap::new(),
            ini_parser: IniParser::new(),
            texture_cache: HashMap::new(),
            model_cache: HashMap::new(),
            initialized: false,
        }
    }

    /// Initialize by loading all object INI files from INIZH.big
    /// Matches C++ behavior of loading object templates at startup
    pub async fn initialize(&mut self, archive_system: &mut ArchiveFileSystem) -> Result<()> {
        if self.initialized {
            warn!("WW3DAssetManager already initialized");
            return Ok(());
        }

        info!("Initializing WW3DAssetManager - Loading INI object definitions from INIZH.big");

        // Discover available object INIs directly from mounted archives.
        // This avoids stale hardcoded names and keeps parity with mods/expansions.
        let object_ini_files = Self::discover_object_ini_files(archive_system);

        let mut total_objects_loaded = 0;
        let mut files_processed = 0;
        let total_files = object_ini_files.len();

        for (idx, ini_file) in object_ini_files.iter().enumerate() {
            debug!(
                "📄 Loading INI file {}/{}: {}",
                idx + 1,
                total_files,
                ini_file
            );

            match archive_system.open_file(ini_file).await {
                Ok(data) => {
                    // Try to parse as UTF-8
                    match String::from_utf8(data) {
                        Ok(content) => {
                            // Parse the INI content
                            match self.ini_parser.parse_ini_content(&content, ini_file) {
                                Ok(count) => {
                                    debug!("✅ Loaded {} objects from {}", count, ini_file);
                                    total_objects_loaded += count;
                                    files_processed += 1;
                                }
                                Err(e) => {
                                    warn!("⚠️ Failed to parse {}: {}", ini_file, e);
                                }
                            }
                        }
                        Err(_) => {
                            warn!("⚠️ Failed to decode {} as UTF-8", ini_file);
                        }
                    }
                }
                Err(e) => {
                    // File not found or other error - continue with next
                    debug!("File not found or not accessible: {}: {}", ini_file, e);
                }
            }
        }

        // Copy parsed definitions to internal cache
        let raw_definitions: HashMap<String, ObjectDefinition> = self
            .ini_parser
            .get_all_definitions()
            .iter()
            .map(|(name, def)| (name.clone(), def.clone()))
            .collect();
        let mut resolved_definitions = HashMap::with_capacity(raw_definitions.len());
        for name in raw_definitions.keys() {
            let mut stack = HashSet::new();
            if let Some(definition) = Self::resolve_inherited_definition(
                name,
                &raw_definitions,
                &mut resolved_definitions,
                &mut stack,
            ) {
                resolved_definitions.insert(name.clone(), definition);
            }
        }

        let mut definitions: Vec<(String, ObjectDefinition)> =
            resolved_definitions.into_iter().collect();
        definitions.sort_by(|a, b| a.0.cmp(&b.0));

        for (name, def) in definitions {
            self.register_definition_indices(&name, &def);
            self.object_definitions.insert(name.clone(), def.clone());

            // Build texture and model caches for quick lookup
            if let Some(model) = &def.model_name {
                self.model_cache.insert(name.clone(), model.clone());
            }

            if let Some(texture) = def.get_primary_texture() {
                self.texture_cache.insert(name.clone(), texture.to_string());
            }
        }

        info!(
            "✅ WW3DAssetManager initialized: Loaded {} objects from {} INI files",
            total_objects_loaded, files_processed
        );

        self.initialized = true;
        Ok(())
    }

    fn resolve_inherited_definition(
        name: &str,
        raw_definitions: &HashMap<String, ObjectDefinition>,
        resolved_definitions: &mut HashMap<String, ObjectDefinition>,
        stack: &mut HashSet<String>,
    ) -> Option<ObjectDefinition> {
        if let Some(existing) = resolved_definitions.get(name) {
            return Some(existing.clone());
        }

        let raw = raw_definitions.get(name)?.clone();
        if !stack.insert(name.to_string()) {
            warn!(
                "Detected cyclic object inheritance while resolving '{}'",
                name
            );
            return Some(raw);
        }

        let resolved = if let Some(parent_name) = raw.parent_name.as_deref() {
            if let Some(parent) = Self::resolve_inherited_definition(
                parent_name,
                raw_definitions,
                resolved_definitions,
                stack,
            ) {
                Self::merge_definition_inheritance(parent, raw)
            } else {
                raw
            }
        } else {
            raw
        };

        stack.remove(name);
        Some(resolved)
    }

    fn merge_definition_inheritance(
        mut parent: ObjectDefinition,
        child: ObjectDefinition,
    ) -> ObjectDefinition {
        parent.name = child.name.clone();
        parent.parent_name = child.parent_name.clone();

        if !child.object_type.is_empty() {
            parent.object_type = child.object_type;
        }
        if !child.display_name.is_empty() {
            parent.display_name = child.display_name;
        }
        if child.model_name.is_some() {
            parent.model_name = child.model_name;
        }
        if child.draw_module.is_some() {
            parent.draw_module = child.draw_module;
        }
        if child.armor_type.is_some() {
            parent.armor_type = child.armor_type;
        }
        if child.hit_points.is_some() {
            parent.hit_points = child.hit_points;
        }
        if (child.scale - 1.0).abs() > f32::EPSILON {
            parent.scale = child.scale;
        }
        if child.owner.is_some() {
            parent.owner = child.owner;
        }

        for (slot, texture) in child.textures {
            parent.textures.insert(slot, texture);
        }
        for (key, value) in child.attributes {
            parent.attributes.insert(key, value);
        }

        parent
    }

    fn discover_object_ini_files(archive_system: &ArchiveFileSystem) -> Vec<String> {
        let mut discovered: Vec<String> = archive_system
            .list_all_files()
            .into_iter()
            .map(|path| path.replace('\\', "/"))
            .filter(|path| {
                let normalized = path.to_ascii_lowercase();
                (normalized.starts_with("data/ini/object/") && normalized.ends_with(".ini"))
                    || normalized == "data/ini/crate.ini"
            })
            .collect();

        discovered.sort_by_key(|path| path.to_ascii_lowercase());
        discovered.dedup_by(|a, b| a.eq_ignore_ascii_case(b));

        discovered
    }

    /// Get the texture filename for an object
    /// Matches C++ WW3DAssetManager::Get_Texture() behavior
    pub fn get_texture_for_object(&self, object_name: &str) -> Option<String> {
        self.get_texture_for_object_with_model(object_name, None)
    }

    /// Get the model filename for an object
    pub fn get_model_for_object(&self, object_name: &str) -> Option<String> {
        if let Some(model) = self.model_cache.get(object_name) {
            return Some(model.clone());
        }

        self.resolve_object_definition(object_name, None)
            .and_then(|def| def.model_name.clone())
    }

    /// Get the full object definition
    pub fn get_object_definition(&self, object_name: &str) -> Option<&ObjectDefinition> {
        self.object_definitions.get(object_name)
    }

    /// Resolve an object definition by name with optional model hint fallback.
    pub fn resolve_object_definition(
        &self,
        object_name: &str,
        model_hint: Option<&str>,
    ) -> Option<&ObjectDefinition> {
        if let Some(def) = self.object_definitions.get(object_name) {
            return Some(def);
        }

        let normalized_key = Self::normalize_object_key(object_name);
        if let Some(canonical) = self.normalized_name_lookup.get(&normalized_key) {
            if let Some(def) = self.object_definitions.get(canonical) {
                return Some(def);
            }
        }

        if let Some(model) = model_hint {
            return self.find_definition_by_model(Some(object_name), model);
        }

        None
    }

    /// Get all loaded object definitions
    pub fn get_all_objects(&self) -> &HashMap<String, ObjectDefinition> {
        &self.object_definitions
    }

    /// Check if an object is defined
    pub fn has_object(&self, object_name: &str) -> bool {
        self.resolve_object_definition(object_name, None).is_some()
    }

    /// Get total number of objects loaded
    pub fn object_count(&self) -> usize {
        self.object_definitions.len()
    }

    /// Check if manager is initialized
    pub fn is_initialized(&self) -> bool {
        self.initialized
    }

    /// Get all texture filenames that should be preloaded
    /// Returns a Vec of unique texture filenames defined in INI object definitions
    pub fn get_all_texture_filenames(&self) -> Vec<String> {
        let mut textures = std::collections::HashSet::new();

        // Collect all texture filenames from all object definitions
        for (_, def) in self.object_definitions.iter() {
            if let Some(texture) = def.get_primary_texture() {
                textures.insert(texture.to_string());
            }
        }

        // Convert to Vec and sort for consistent ordering
        let mut texture_vec: Vec<String> = textures.into_iter().collect();
        texture_vec.sort();
        texture_vec
    }
}

impl Default for WW3DAssetManager {
    fn default() -> Self {
        Self::new()
    }
}

impl WW3DAssetManager {
    fn register_definition_indices(&mut self, name: &str, def: &ObjectDefinition) {
        self.normalized_name_lookup
            .entry(Self::normalize_object_key(name))
            .or_insert_with(|| name.to_string());

        if let Some(model_name) = &def.model_name {
            for key in Self::normalized_model_keys(model_name) {
                self.model_lookup
                    .entry(key)
                    .or_default()
                    .push(name.to_string());
            }
        }
    }

    fn get_texture_for_object_with_model(
        &self,
        object_name: &str,
        model_hint: Option<&str>,
    ) -> Option<String> {
        if let Some(texture) = self.texture_cache.get(object_name) {
            return Some(texture.clone());
        }

        if let Some(def) = self.resolve_object_definition(object_name, model_hint) {
            if let Some(texture) = def.get_primary_texture() {
                return Some(texture.to_string());
            }
        }

        // Fallback: try using the model hint directly (matches behavior observed in
        // GeneralsMD/Code/GameEngineDevice/Source/W3DDevice/GameClient/W3DAssetManager.cpp)
        if let Some(model) = model_hint {
            if let Some(def) = self.find_definition_by_model(Some(object_name), model) {
                if let Some(texture) = def.get_primary_texture() {
                    return Some(texture.to_string());
                }
            }
        }

        None
    }

    #[inline]
    fn normalize_object_key(name: &str) -> String {
        name.trim().to_ascii_lowercase()
    }

    fn normalized_model_keys(model_name: &str) -> Vec<String> {
        let mut key = model_name.trim().to_ascii_lowercase();
        if let Some(stripped) = key.strip_suffix(".w3d") {
            key = stripped.to_string();
        }
        vec![key]
    }

    fn find_definition_by_model(
        &self,
        object_name: Option<&str>,
        model_hint: &str,
    ) -> Option<&ObjectDefinition> {
        let _ = object_name;
        for key in Self::normalized_model_keys(model_hint) {
            if let Some(entries) = self.model_lookup.get(&key) {
                if let Some(candidate) = entries.first() {
                    return self.object_definitions.get(candidate);
                }
            }
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_asset_manager_creation() {
        let manager = WW3DAssetManager::new();
        assert!(!manager.is_initialized());
        assert_eq!(manager.object_count(), 0);
    }

    #[test]
    fn test_texture_lookup() {
        let mut manager = WW3DAssetManager::new();

        // Manually add a test definition
        let mut def = ObjectDefinition::new("TestUnit".to_string());
        def.textures
            .insert("0".to_string(), "test_texture.tga".to_string());
        manager
            .object_definitions
            .insert("TestUnit".to_string(), def);
        manager
            .texture_cache
            .insert("TestUnit".to_string(), "test_texture.tga".to_string());

        let texture = manager.get_texture_for_object("TestUnit");
        assert_eq!(texture, Some("test_texture.tga".to_string()));
    }

    #[test]
    fn test_model_hint_resolution() {
        let mut manager = WW3DAssetManager::new();
        let mut def = ObjectDefinition::new("AmericaVehicleHumvee".to_string());
        def.model_name = Some("AVHUMMER".to_string());
        def.textures
            .insert("0".to_string(), "avhummer.tga".to_string());

        manager
            .object_definitions
            .insert("AmericaVehicleHumvee".to_string(), def.clone());
        manager.register_definition_indices("AmericaVehicleHumvee", &def);

        let texture =
            manager.get_texture_for_object_with_model("NotInDefinitions", Some("avhummer"));
        assert_eq!(texture, Some("avhummer.tga".to_string()));
    }
}
