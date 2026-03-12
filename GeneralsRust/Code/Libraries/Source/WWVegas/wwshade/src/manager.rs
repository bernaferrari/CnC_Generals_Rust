//! Shader Definition Manager
//!
//! This module provides the central registry for shader definition factories.
//! It maintains a global registry of all available shader types and handles
//! factory registration, lookup, and shader instance creation.

use once_cell::sync::Lazy;
use std::collections::HashMap;
use std::sync::{Arc, RwLock};

use crate::def::{ShdDefClass, ShdDefFactory};
use crate::error::{ShdError, ShdResult};
// use crate::class_ids::*; // Not used currently

/// Factory registry entry
#[derive(Debug, Clone)]
struct FactoryEntry {
    factory: Arc<dyn ShdDefFactory>,
    name: String,
    class_id: u32,
}

/// Thread-safe shader definition manager
///
/// This is the central registry for all shader definition factories.
/// It provides methods to register factories, find them by ID or name,
/// and create shader definition instances.
#[derive(Debug)]
pub struct ShdDefManager {
    /// Factory registry by class ID
    factories_by_id: HashMap<u32, FactoryEntry>,
    /// Factory registry by name  
    factories_by_name: HashMap<String, FactoryEntry>,
    /// Ordered list of all factories for iteration
    factory_list: Vec<FactoryEntry>,
}

/// Global shader definition manager instance
static GLOBAL_MANAGER: Lazy<RwLock<ShdDefManager>> =
    Lazy::new(|| RwLock::new(ShdDefManager::new()));

impl ShdDefManager {
    /// Create a new shader definition manager
    fn new() -> Self {
        Self {
            factories_by_id: HashMap::new(),
            factories_by_name: HashMap::new(),
            factory_list: Vec::new(),
        }
    }

    /// Register a shader definition factory
    ///
    /// This adds the factory to the global registry, making it available
    /// for shader creation and enumeration.
    pub fn register_factory(factory: Arc<dyn ShdDefFactory>) -> ShdResult<()> {
        let mut manager = GLOBAL_MANAGER
            .write()
            .map_err(|_| ShdError::InvalidConfig("Failed to acquire manager lock".to_string()))?;

        let class_id = factory.get_class_id();
        let name = factory.get_display_name().to_string();

        // Check for duplicate registrations
        if manager.factories_by_id.contains_key(&class_id) {
            return Err(ShdError::InvalidConfig(format!(
                "Factory with class ID {} already registered",
                class_id
            )));
        }

        if manager.factories_by_name.contains_key(&name) {
            return Err(ShdError::InvalidConfig(format!(
                "Factory with name '{}' already registered",
                name
            )));
        }

        let entry = FactoryEntry {
            factory: factory.clone(),
            name: name.clone(),
            class_id,
        };

        manager.factories_by_id.insert(class_id, entry.clone());
        manager.factories_by_name.insert(name, entry.clone());
        manager.factory_list.push(entry);

        Ok(())
    }

    /// Unregister a shader definition factory by class ID
    pub fn unregister_factory(class_id: u32) -> ShdResult<()> {
        let mut manager = GLOBAL_MANAGER
            .write()
            .map_err(|_| ShdError::InvalidConfig("Failed to acquire manager lock".to_string()))?;

        if let Some(entry) = manager.factories_by_id.remove(&class_id) {
            manager.factories_by_name.remove(&entry.name);
            manager.factory_list.retain(|e| e.class_id != class_id);
            Ok(())
        } else {
            Err(ShdError::ResourceNotFound(format!(
                "Factory with class ID {} not found",
                class_id
            )))
        }
    }

    /// Find a factory by class ID
    pub fn find_factory_by_id(class_id: u32) -> ShdResult<Arc<dyn ShdDefFactory>> {
        let manager = GLOBAL_MANAGER
            .read()
            .map_err(|_| ShdError::InvalidConfig("Failed to acquire manager lock".to_string()))?;

        manager
            .factories_by_id
            .get(&class_id)
            .map(|entry| entry.factory.clone())
            .ok_or_else(|| {
                ShdError::ResourceNotFound(format!("Factory with class ID {} not found", class_id))
            })
    }

    /// Find a factory by name (case-insensitive)
    pub fn find_factory_by_name(name: &str) -> ShdResult<Arc<dyn ShdDefFactory>> {
        let manager = GLOBAL_MANAGER
            .read()
            .map_err(|_| ShdError::InvalidConfig("Failed to acquire manager lock".to_string()))?;

        // First try exact match
        if let Some(entry) = manager.factories_by_name.get(name) {
            return Ok(entry.factory.clone());
        }

        // Then try case-insensitive search
        let name_lower = name.to_lowercase();
        for entry in &manager.factory_list {
            if entry.name.to_lowercase() == name_lower {
                return Ok(entry.factory.clone());
            }
        }

        Err(ShdError::ResourceNotFound(format!(
            "Factory with name '{}' not found",
            name
        )))
    }

    /// Get all registered factories
    pub fn get_all_factories() -> ShdResult<Vec<Arc<dyn ShdDefFactory>>> {
        let manager = GLOBAL_MANAGER
            .read()
            .map_err(|_| ShdError::InvalidConfig("Failed to acquire manager lock".to_string()))?;

        Ok(manager
            .factory_list
            .iter()
            .map(|entry| entry.factory.clone())
            .collect())
    }

    /// Get factories filtered by superclass (base class) ID
    ///
    /// This allows enumeration of specific types of shaders (e.g., all bump mapping shaders)
    pub fn get_factories_by_superclass(
        superclass_id: u32,
    ) -> ShdResult<Vec<Arc<dyn ShdDefFactory>>> {
        let manager = GLOBAL_MANAGER
            .read()
            .map_err(|_| ShdError::InvalidConfig("Failed to acquire manager lock".to_string()))?;

        let mut filtered = Vec::new();
        for entry in &manager.factory_list {
            let class_id = entry.class_id;
            // Check if this class ID belongs to the requested superclass
            // This is a simplified check - in a real implementation you might have
            // a more sophisticated class hierarchy system
            if (class_id & 0xFF00) == (superclass_id & 0xFF00) {
                filtered.push(entry.factory.clone());
            }
        }

        Ok(filtered)
    }

    /// Create a shader definition instance by class ID
    pub fn create_shader_definition(class_id: u32) -> ShdResult<Box<dyn ShdDefClass>> {
        let factory = Self::find_factory_by_id(class_id)?;
        factory.create_definition(class_id)
    }

    /// Get the number of registered factories
    pub fn get_factory_count() -> ShdResult<usize> {
        let manager = GLOBAL_MANAGER
            .read()
            .map_err(|_| ShdError::InvalidConfig("Failed to acquire manager lock".to_string()))?;
        Ok(manager.factory_list.len())
    }

    /// Check if a factory with the given class ID is registered
    pub fn has_factory(class_id: u32) -> ShdResult<bool> {
        let manager = GLOBAL_MANAGER
            .read()
            .map_err(|_| ShdError::InvalidConfig("Failed to acquire manager lock".to_string()))?;
        Ok(manager.factories_by_id.contains_key(&class_id))
    }

    /// Clear all registered factories (primarily for testing)
    #[cfg(test)]
    pub fn clear_all_factories() -> ShdResult<()> {
        let mut manager = GLOBAL_MANAGER
            .write()
            .map_err(|_| ShdError::InvalidConfig("Failed to acquire manager lock".to_string()))?;

        manager.factories_by_id.clear();
        manager.factories_by_name.clear();
        manager.factory_list.clear();

        Ok(())
    }
}

/// Shader save/load utilities
///
/// These functions handle serialization of shader definitions to binary format,
/// compatible with the original C++ chunk-based format.
pub struct ShaderSerializer;

impl ShaderSerializer {
    /// Save a shader definition to binary format
    pub fn save_shader(shader_def: &dyn ShdDefClass) -> ShdResult<Vec<u8>> {
        let mut data = Vec::new();

        // Write class ID chunk
        let class_id = shader_def.get_class_id();
        data.extend_from_slice(b"CLID"); // Chunk header
        data.extend_from_slice(&4u32.to_le_bytes()); // Chunk size
        data.extend_from_slice(&class_id.to_le_bytes());

        // Write shader definition data chunk
        let shader_data = shader_def.save()?;
        data.extend_from_slice(b"SHDD"); // Chunk header
        data.extend_from_slice(&(shader_data.len() as u32).to_le_bytes());
        data.extend_from_slice(&shader_data);

        Ok(data)
    }

    /// Load a shader definition from binary format
    pub fn load_shader(data: &[u8]) -> ShdResult<Box<dyn ShdDefClass>> {
        if data.len() < 12 {
            return Err(ShdError::FormatError(
                "Invalid shader data format".to_string(),
            ));
        }

        let mut offset = 0;

        // Read class ID chunk
        let chunk_header = &data[offset..offset + 4];
        if chunk_header != b"CLID" {
            return Err(ShdError::FormatError("Expected CLID chunk".to_string()));
        }
        offset += 4;

        let chunk_size = u32::from_le_bytes([
            data[offset],
            data[offset + 1],
            data[offset + 2],
            data[offset + 3],
        ]);
        offset += 4;

        if chunk_size != 4 {
            return Err(ShdError::FormatError("Invalid CLID chunk size".to_string()));
        }

        let class_id = u32::from_le_bytes([
            data[offset],
            data[offset + 1],
            data[offset + 2],
            data[offset + 3],
        ]);
        offset += 4;

        // Create shader definition instance
        let mut shader_def = ShdDefManager::create_shader_definition(class_id)?;

        // Read shader definition data chunk
        if offset + 8 > data.len() {
            return Err(ShdError::FormatError("Incomplete shader data".to_string()));
        }

        let chunk_header = &data[offset..offset + 4];
        if chunk_header != b"SHDD" {
            return Err(ShdError::FormatError("Expected SHDD chunk".to_string()));
        }
        offset += 4;

        let chunk_size = u32::from_le_bytes([
            data[offset],
            data[offset + 1],
            data[offset + 2],
            data[offset + 3],
        ]) as usize;
        offset += 4;

        if offset + chunk_size > data.len() {
            return Err(ShdError::FormatError(
                "Incomplete shader definition data".to_string(),
            ));
        }

        let shader_data = &data[offset..offset + chunk_size];
        shader_def.load(shader_data)?;

        Ok(shader_def)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::ShdError;

    #[derive(Debug)]
    struct MockFactory {
        class_id: u32,
        name: String,
    }

    impl ShdDefFactory for MockFactory {
        fn create_definition(&self, class_id: u32) -> ShdResult<Box<dyn ShdDefClass>> {
            Ok(Box::new(MockShaderDef {
                class_id,
                name: format!("MockShader_{}", class_id),
                surface_type: 0,
            }))
        }

        fn get_display_name(&self) -> &str {
            &self.name
        }

        fn get_class_id(&self) -> u32 {
            self.class_id
        }
    }

    #[derive(Debug, Clone)]
    struct MockShaderDef {
        class_id: u32,
        name: String,
        surface_type: i32,
    }

    impl ShdDefClass for MockShaderDef {
        fn get_class_id(&self) -> u32 {
            self.class_id
        }

        fn get_name(&self) -> &str {
            &self.name
        }

        fn set_name(&mut self, name: String) {
            self.name = name;
        }

        fn get_surface_type(&self) -> i32 {
            self.surface_type
        }

        fn set_surface_type(&mut self, surface_type: i32) {
            self.surface_type = surface_type;
        }

        fn clone_def(&self) -> Box<dyn ShdDefClass> {
            Box::new(self.clone())
        }

        fn create_shader(&self) -> ShdResult<Box<dyn crate::interface::ShdInterface>> {
            Err(ShdError::InvalidConfig(
                "Mock shader cannot create instances".to_string(),
            ))
        }

        fn is_valid_config(&self) -> ShdResult<()> {
            Ok(())
        }

        fn save(&self) -> ShdResult<Vec<u8>> {
            Ok(self.name.as_bytes().to_vec())
        }

        fn load(&mut self, data: &[u8]) -> ShdResult<()> {
            self.name = String::from_utf8(data.to_vec())
                .map_err(|e| ShdError::FormatError(format!("Invalid UTF-8: {}", e)))?;
            Ok(())
        }

        fn as_any(&self) -> &dyn std::any::Any {
            self
        }

        fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
            self
        }
    }

    fn setup_test() -> std::sync::MutexGuard<'static, ()> {
        static LOCK: std::sync::OnceLock<std::sync::Mutex<()>> = std::sync::OnceLock::new();
        let guard = LOCK
            .get_or_init(|| std::sync::Mutex::new(()))
            .lock()
            .expect("wwshade manager test lock");
        let _ = ShdDefManager::clear_all_factories();
        guard
    }

    #[test]
    fn test_factory_registration() {
        let _lock = setup_test();

        let factory = Arc::new(MockFactory {
            class_id: 100,
            name: "TestFactory".to_string(),
        });

        assert!(ShdDefManager::register_factory(factory.clone()).is_ok());
        assert_eq!(ShdDefManager::get_factory_count().unwrap(), 1);
        assert!(ShdDefManager::has_factory(100).unwrap());

        // Test duplicate registration fails
        assert!(ShdDefManager::register_factory(factory).is_err());
    }

    #[test]
    fn test_factory_lookup() {
        let _lock = setup_test();

        let factory = Arc::new(MockFactory {
            class_id: 200,
            name: "LookupTest".to_string(),
        });

        assert!(ShdDefManager::register_factory(factory).is_ok());

        // Test lookup by ID
        let found_factory = ShdDefManager::find_factory_by_id(200).unwrap();
        assert_eq!(found_factory.get_class_id(), 200);

        // Test lookup by name
        let found_factory = ShdDefManager::find_factory_by_name("LookupTest").unwrap();
        assert_eq!(found_factory.get_display_name(), "LookupTest");

        // Test case-insensitive name lookup
        let found_factory = ShdDefManager::find_factory_by_name("lookuptest").unwrap();
        assert_eq!(found_factory.get_display_name(), "LookupTest");

        // Test non-existent lookups
        assert!(ShdDefManager::find_factory_by_id(999).is_err());
        assert!(ShdDefManager::find_factory_by_name("NonExistent").is_err());
    }

    #[test]
    fn test_shader_creation() {
        let _lock = setup_test();

        let factory = Arc::new(MockFactory {
            class_id: 300,
            name: "CreationTest".to_string(),
        });

        assert!(ShdDefManager::register_factory(factory).is_ok());

        let shader_def = ShdDefManager::create_shader_definition(300).unwrap();
        assert_eq!(shader_def.get_class_id(), 300);
        assert_eq!(shader_def.get_name(), "MockShader_300");
    }

    #[test]
    fn test_factory_unregistration() {
        let _lock = setup_test();

        let factory = Arc::new(MockFactory {
            class_id: 400,
            name: "UnregisterTest".to_string(),
        });

        assert!(ShdDefManager::register_factory(factory).is_ok());
        assert!(ShdDefManager::has_factory(400).unwrap());

        assert!(ShdDefManager::unregister_factory(400).is_ok());
        assert!(!ShdDefManager::has_factory(400).unwrap());

        // Test unregistering non-existent factory
        assert!(ShdDefManager::unregister_factory(999).is_err());
    }

    #[test]
    fn test_serialization() {
        let _lock = setup_test();

        let factory = Arc::new(MockFactory {
            class_id: 500,
            name: "SerializationTest".to_string(),
        });

        assert!(ShdDefManager::register_factory(factory).is_ok());

        let shader_def = ShdDefManager::create_shader_definition(500).unwrap();

        // Test save
        let saved_data = ShaderSerializer::save_shader(shader_def.as_ref()).unwrap();
        assert!(!saved_data.is_empty());

        // Test load
        let loaded_shader = ShaderSerializer::load_shader(&saved_data).unwrap();
        assert_eq!(loaded_shader.get_class_id(), 500);
    }
}
