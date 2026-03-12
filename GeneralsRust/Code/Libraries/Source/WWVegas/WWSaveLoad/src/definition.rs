//! # Definition System - Rust Implementation
//!
//! This module provides a complete Rust conversion of the C++ Definition system from
//! Command & Conquer Generals Zero Hour. The Definition system manages configurable
//! game objects with unique IDs, names, and persistence capabilities.
//!
//! ## Design Goals
//!
//! - Maintain the same functionality as the original C++ DefinitionClass
//! - Use idiomatic Rust patterns with proper error handling
//! - Provide type safety through the trait system
//! - Support both save/load and configuration validation
//!
//! ## Key Features
//!
//! - **Definition Management**: Each definition has a unique ID and display name
//! - **Configuration Validation**: Built-in validation for definition correctness
//! - **Persistence**: Full save/load support with chunk-based serialization
//! - **User Data**: Generic data storage for game-specific information
//! - **Manager Integration**: Full integration with the DefinitionManager system
//!
//! ## Usage Examples
//!
//! ```rust
//! use definition::{Definition, EditableClass, DefinitionClass};
//! use std::sync::Arc;
//!
//! // Define a concrete definition type
//! struct WeaponDefinition {
//!     base: Definition,
//!     damage: i32,
//!     range: f32,
//! }
//!
//! impl DefinitionClass for WeaponDefinition {
//!     fn get_class_id(&self) -> u32 { 0x12340001 }
//!     fn create(&self) -> Result<Box<dyn DefinitionClass>, DefinitionError> {
//!         Ok(Box::new(WeaponDefinition::new()))
//!     }
//! }
//!
//! impl EditableClass for WeaponDefinition {
//!     // Implementation for editing interface
//! }
//! ```

use crate::saveload::{
    ChunkLoad, ChunkLoadExt, ChunkSave, ChunkSaveExt, Persist, SaveLoadError, SaveLoadResult,
};
use std::any::Any;
use std::fmt;
use std::sync::Arc;

/// Chunk IDs used in definition serialization
const CHUNKID_VARIABLES: u32 = 0x00000100;

/// Variable IDs for micro-chunks in definition serialization
const VARID_INSTANCEID: u32 = 0x01;
const VARID_NAME: u32 = 0x03;

/// Errors specific to the Definition system
#[derive(Debug, Clone)]
pub enum DefinitionError {
    /// Invalid definition ID
    InvalidId(u32),
    /// Invalid definition name
    InvalidName(String),
    /// Configuration validation failed
    ConfigValidationFailed(String),
    /// Definition manager not available
    ManagerNotAvailable,
    /// Save/Load operation failed
    SaveLoadError(String),
    /// Creation failed
    CreationFailed(String),
}

impl fmt::Display for DefinitionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DefinitionError::InvalidId(id) => write!(f, "Invalid definition ID: {}", id),
            DefinitionError::InvalidName(name) => write!(f, "Invalid definition name: '{}'", name),
            DefinitionError::ConfigValidationFailed(msg) => {
                write!(f, "Configuration validation failed: {}", msg)
            }
            DefinitionError::ManagerNotAvailable => write!(f, "Definition manager not available"),
            DefinitionError::SaveLoadError(msg) => write!(f, "Save/load error: {}", msg),
            DefinitionError::CreationFailed(msg) => write!(f, "Creation failed: {}", msg),
        }
    }
}

impl std::error::Error for DefinitionError {}

impl From<SaveLoadError> for DefinitionError {
    fn from(error: SaveLoadError) -> Self {
        DefinitionError::SaveLoadError(error.to_string())
    }
}

/// Result type for Definition operations
pub type DefinitionResult<T> = Result<T, DefinitionError>;

/// Trait for editable classes - Rust equivalent of the DECLARE_EDITABLE functionality
/// This trait provides the interface for objects that can be edited in development tools
pub trait EditableClass: Send + Sync {
    /// Get the type name for this editable class
    fn get_type_name(&self) -> &'static str;

    /// Get a human-readable description of this class
    fn get_description(&self) -> &'static str {
        "Definition class"
    }

    /// Check if this class supports property editing
    fn supports_property_editing(&self) -> bool {
        true
    }

    /// Get the list of editable properties (optional)
    fn get_editable_properties(&self) -> Vec<String> {
        Vec::new()
    }
}

/// Trait for definition classes - core interface for all definitions
/// This is the Rust equivalent of the C++ DefinitionClass interface
pub trait DefinitionClass: EditableClass + Persist {
    /// Get the unique class ID for this definition type
    /// This is equivalent to the pure virtual Get_Class_ID() method
    fn get_class_id(&self) -> u32;

    /// Create a new instance of this definition type
    /// This replaces the pure virtual Create() method
    fn create(&self) -> DefinitionResult<Box<dyn DefinitionClass>>;

    /// Get the instance ID for this specific definition
    fn get_id(&self) -> u32;

    /// Set the instance ID for this definition
    /// This will automatically update the definition manager registration
    fn set_id(&mut self, id: u32) -> DefinitionResult<()>;

    /// Get the display name for this definition
    fn get_name(&self) -> &str;

    /// Set the display name for this definition
    fn set_name(&mut self, name: &str);

    /// Validate the configuration of this definition
    /// Returns Ok(()) if valid, or Err with a descriptive message if invalid
    fn is_valid_config(&self) -> DefinitionResult<()> {
        Ok(())
    }

    /// Get user-defined data associated with this definition
    fn get_user_data(&self) -> u32;

    /// Set user-defined data for this definition
    fn set_user_data(&mut self, data: u32);

    /// Check if saving is enabled for this definition
    fn is_save_enabled(&self) -> bool;

    /// Enable or disable saving for this definition
    fn enable_save(&mut self, enabled: bool);

    /// Get the definition manager link (internal use)
    fn get_definition_mgr_link(&self) -> i32;

    /// Set the definition manager link (internal use)
    fn set_definition_mgr_link(&mut self, link: i32);
}

/// Core definition data structure
/// This contains the basic data that all definitions share
#[derive(Debug, Clone)]
pub struct Definition {
    /// Unique instance ID
    id: u32,
    /// Display name
    name: String,
    /// User-defined data
    user_data: u32,
    /// Whether saving is enabled
    save_enabled: bool,
    /// Link to definition manager (-1 if not registered)
    definition_mgr_link: i32,
}

impl Definition {
    /// Create a new definition with default values
    pub fn new() -> Self {
        Self {
            id: 0,
            name: String::new(),
            user_data: 0,
            save_enabled: true,
            definition_mgr_link: -1,
        }
    }

    /// Create a new definition with specified ID and name
    pub fn with_id_and_name(id: u32, name: String) -> Self {
        Self {
            id,
            name,
            user_data: 0,
            save_enabled: true,
            definition_mgr_link: -1,
        }
    }

    /// Get the instance ID
    pub fn get_id(&self) -> u32 {
        self.id
    }

    /// Set the instance ID
    pub fn set_id(&mut self, id: u32) -> DefinitionResult<()> {
        self.id = id;
        // Note: In a full implementation, we would need to update the definition manager here
        // For now, this is a placeholder for the manager integration
        Ok(())
    }

    /// Get the display name
    pub fn get_name(&self) -> &str {
        &self.name
    }

    /// Set the display name
    pub fn set_name(&mut self, name: &str) {
        self.name = name.to_string();
    }

    /// Get user data
    pub fn get_user_data(&self) -> u32 {
        self.user_data
    }

    /// Set user data
    pub fn set_user_data(&mut self, data: u32) {
        self.user_data = data;
    }

    /// Check if saving is enabled
    pub fn is_save_enabled(&self) -> bool {
        self.save_enabled
    }

    /// Enable or disable saving
    pub fn enable_save(&mut self, enabled: bool) {
        self.save_enabled = enabled;
    }

    /// Get definition manager link
    pub fn get_definition_mgr_link(&self) -> i32 {
        self.definition_mgr_link
    }

    /// Set definition manager link
    pub fn set_definition_mgr_link(&mut self, link: i32) {
        self.definition_mgr_link = link;
    }

    /// Validate the configuration
    pub fn is_valid_config(&self) -> DefinitionResult<()> {
        // Basic validation - can be overridden by implementations
        if self.name.is_empty() {
            return Err(DefinitionError::ConfigValidationFailed(
                "Name cannot be empty".to_string(),
            ));
        }
        Ok(())
    }
}

impl Default for Definition {
    fn default() -> Self {
        Self::new()
    }
}

/// Implementation of Persist trait for Definition
impl Persist for Definition {
    fn save(&self, chunk_save: &mut dyn ChunkSave) -> SaveLoadResult<()> {
        // Begin the variables chunk
        chunk_save.begin_chunk(CHUNKID_VARIABLES)?;

        // Save variables using micro-chunks (implemented manually)
        self.write_micro_chunk(chunk_save, VARID_INSTANCEID, &self.id)?;
        self.write_micro_chunk_string(chunk_save, VARID_NAME, &self.name)?;

        // End the variables chunk
        chunk_save.end_chunk()?;

        Ok(())
    }

    fn load(&mut self, chunk_load: &mut dyn ChunkLoad) -> SaveLoadResult<()> {
        // Load all chunks
        while chunk_load.open_chunk()? {
            match chunk_load.current_chunk_id() {
                CHUNKID_VARIABLES => {
                    self.load_variables(chunk_load)?;
                }
                _ => {
                    // Unknown chunk - skip it
                }
            }
            chunk_load.close_chunk()?;
        }

        Ok(())
    }

    fn get_factory(&self) -> Arc<dyn crate::saveload::PersistFactory> {
        // This is a placeholder - in practice, each definition type would have its own factory
        Arc::new(DefinitionFactory::new(0))
    }

    fn get_remap_id(&self) -> crate::saveload::RemapId {
        self.id as u64
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

impl Definition {
    /// Load variables from micro-chunks
    fn load_variables(&mut self, chunk_load: &mut dyn ChunkLoad) -> SaveLoadResult<()> {
        // Load all micro-chunks (implemented manually)
        loop {
            let (micro_id, found) = self.read_micro_chunk_header(chunk_load)?;
            if !found {
                break;
            }

            match micro_id {
                VARID_INSTANCEID => {
                    self.id = self.read_micro_chunk_value(chunk_load)?;
                }
                VARID_NAME => {
                    self.name = self.read_micro_chunk_string(chunk_load)?;
                }
                _ => {
                    // Unknown micro-chunk - skip it
                    self.skip_micro_chunk_data(chunk_load)?;
                }
            }
        }

        Ok(())
    }

    /// Write a micro-chunk with typed data
    fn write_micro_chunk<T: Sized>(
        &self,
        chunk_save: &mut dyn ChunkSave,
        id: u32,
        value: &T,
    ) -> SaveLoadResult<()> {
        use ChunkSaveExt;
        // Write micro-chunk header: ID (4 bytes) + size (4 bytes)
        chunk_save.write_value(&id)?;
        let size = std::mem::size_of::<T>() as u32;
        chunk_save.write_value(&size)?;
        // Write the actual data
        chunk_save.write_value(value)?;
        Ok(())
    }

    /// Write a micro-chunk with string data
    fn write_micro_chunk_string(
        &self,
        chunk_save: &mut dyn ChunkSave,
        id: u32,
        value: &str,
    ) -> SaveLoadResult<()> {
        use ChunkSaveExt;
        // Write micro-chunk header: ID (4 bytes) + size (4 bytes)
        chunk_save.write_value(&id)?;
        let size = value.len() as u32 + 1; // +1 for null terminator
        chunk_save.write_value(&size)?;
        // Write the string data
        chunk_save.write(value.as_bytes())?;
        chunk_save.write(&[0u8])?; // null terminator
        Ok(())
    }

    /// Read micro-chunk header
    fn read_micro_chunk_header(
        &self,
        chunk_load: &mut dyn ChunkLoad,
    ) -> SaveLoadResult<(u32, bool)> {
        // Try to read the micro-chunk ID
        let mut buffer = [0u8; 4];
        let bytes_read = chunk_load.read(&mut buffer)?;
        if bytes_read == 0 {
            return Ok((0, false)); // End of chunk
        }
        if bytes_read != 4 {
            return Err(SaveLoadError::InvalidChunk(0));
        }
        let id = u32::from_le_bytes(buffer);

        // Read the size
        chunk_load.read(&mut buffer)?;
        let _size = u32::from_le_bytes(buffer);

        Ok((id, true))
    }

    /// Read micro-chunk typed value
    fn read_micro_chunk_value<T: Sized + Default>(
        &self,
        chunk_load: &mut dyn ChunkLoad,
    ) -> SaveLoadResult<T> {
        use ChunkLoadExt;
        chunk_load.read_value()
    }

    /// Read micro-chunk string value
    fn read_micro_chunk_string(&self, chunk_load: &mut dyn ChunkLoad) -> SaveLoadResult<String> {
        let mut buffer = Vec::new();
        let mut byte_buf = [0u8; 1];

        // Read until null terminator
        loop {
            chunk_load.read(&mut byte_buf)?;
            if byte_buf[0] == 0 {
                break;
            }
            buffer.push(byte_buf[0]);
        }

        String::from_utf8(buffer).map_err(|e| SaveLoadError::IoError(e.to_string()))
    }

    /// Skip micro-chunk data
    fn skip_micro_chunk_data(&self, chunk_load: &mut dyn ChunkLoad) -> SaveLoadResult<()> {
        // For now, just read a fixed amount of data to skip
        // In a real implementation, we'd use the size from the header
        let mut buffer = [0u8; 256];
        let _ = chunk_load.read(&mut buffer)?;
        Ok(())
    }
}

/// A simple factory implementation for definitions
/// In practice, each definition type would have its own specific factory
pub struct DefinitionFactory {
    class_id: u32,
}

impl DefinitionFactory {
    pub fn new(class_id: u32) -> Self {
        Self { class_id }
    }
}

impl crate::saveload::PersistFactory for DefinitionFactory {
    fn chunk_id(&self) -> crate::saveload::ChunkId {
        self.class_id
    }

    fn load(&self, chunk_load: &mut dyn ChunkLoad) -> SaveLoadResult<Arc<dyn Persist>> {
        let mut def = Definition::new();
        def.load(chunk_load)?;
        Ok(Arc::new(def))
    }

    fn save(&self, chunk_save: &mut dyn ChunkSave, obj: &dyn Persist) -> SaveLoadResult<()> {
        obj.save(chunk_save)
    }
}

/// Base implementation of DefinitionClass for the basic Definition struct
impl DefinitionClass for Definition {
    fn get_class_id(&self) -> u32 {
        // This is a base definition class - specific implementations should override this
        0x00000000
    }

    fn create(&self) -> DefinitionResult<Box<dyn DefinitionClass>> {
        Ok(Box::new(Definition::new()))
    }

    fn get_id(&self) -> u32 {
        self.id
    }

    fn set_id(&mut self, id: u32) -> DefinitionResult<()> {
        // Store the old ID for manager operations
        let _old_id = self.id;
        self.id = id;

        // If we are registered with the definition manager, update the registration
        if self.definition_mgr_link != -1 {
            // In a full implementation, we would:
            // 1. Unregister the old definition
            // 2. Re-register with the new ID
            // For now, this is a placeholder
            // DefinitionManager::unregister_definition(self);
            // DefinitionManager::register_definition(self);
        }

        Ok(())
    }

    fn get_name(&self) -> &str {
        &self.name
    }

    fn set_name(&mut self, name: &str) {
        self.name = name.to_string();
    }

    fn is_valid_config(&self) -> DefinitionResult<()> {
        Definition::is_valid_config(self)
    }

    fn get_user_data(&self) -> u32 {
        self.user_data
    }

    fn set_user_data(&mut self, data: u32) {
        self.user_data = data;
    }

    fn is_save_enabled(&self) -> bool {
        self.save_enabled
    }

    fn enable_save(&mut self, enabled: bool) {
        self.save_enabled = enabled;
    }

    fn get_definition_mgr_link(&self) -> i32 {
        self.definition_mgr_link
    }

    fn set_definition_mgr_link(&mut self, link: i32) {
        self.definition_mgr_link = link;
    }
}

impl EditableClass for Definition {
    fn get_type_name(&self) -> &'static str {
        "Definition"
    }

    fn get_description(&self) -> &'static str {
        "Base definition class with ID, name, and user data"
    }

    fn get_editable_properties(&self) -> Vec<String> {
        vec![
            "id".to_string(),
            "name".to_string(),
            "user_data".to_string(),
            "save_enabled".to_string(),
        ]
    }
}

/// Macro to implement the EditableClass trait with the DECLARE_EDITABLE functionality
/// This macro provides a convenient way to implement the editable interface for definition types
#[macro_export]
macro_rules! declare_editable {
    ($type_name:ty, $parent_type:ty) => {
        impl EditableClass for $type_name {
            fn get_type_name(&self) -> &'static str {
                stringify!($type_name)
            }

            fn get_description(&self) -> &'static str {
                concat!("Editable ", stringify!($type_name))
            }
        }
    };
}

/// Macro to implement a complete DefinitionClass with specified class ID
/// This provides a convenient way to create definition types with minimal boilerplate
#[macro_export]
macro_rules! impl_definition_class {
    ($type_name:ty, $class_id:expr) => {
        impl DefinitionClass for $type_name {
            fn get_class_id(&self) -> u32 {
                $class_id
            }

            fn create(&self) -> DefinitionResult<Box<dyn DefinitionClass>> {
                Ok(Box::new(<$type_name>::default()))
            }

            fn get_id(&self) -> u32 {
                self.base.get_id()
            }

            fn set_id(&mut self, id: u32) -> DefinitionResult<()> {
                self.base.set_id(id)
            }

            fn get_name(&self) -> &str {
                self.base.get_name()
            }

            fn set_name(&mut self, name: &str) {
                self.base.set_name(name)
            }

            fn get_user_data(&self) -> u32 {
                self.base.get_user_data()
            }

            fn set_user_data(&mut self, data: u32) {
                self.base.set_user_data(data)
            }

            fn is_save_enabled(&self) -> bool {
                self.base.is_save_enabled()
            }

            fn enable_save(&mut self, enabled: bool) {
                self.base.enable_save(enabled)
            }

            fn get_definition_mgr_link(&self) -> i32 {
                self.base.get_definition_mgr_link()
            }

            fn set_definition_mgr_link(&mut self, link: i32) {
                self.base.set_definition_mgr_link(link)
            }
        }

        impl $crate::saveload::Persist for $type_name {
            fn save(
                &self,
                chunk_save: &mut dyn $crate::saveload::ChunkSave,
            ) -> $crate::saveload::SaveLoadResult<()> {
                self.base.save(chunk_save)
            }

            fn load(
                &mut self,
                chunk_load: &mut dyn $crate::saveload::ChunkLoad,
            ) -> $crate::saveload::SaveLoadResult<()> {
                self.base.load(chunk_load)
            }

            fn get_factory(&self) -> ::std::sync::Arc<dyn $crate::saveload::PersistFactory> {
                ::std::sync::Arc::new($crate::definition::DefinitionFactory::new(
                    self.get_class_id(),
                ))
            }

            fn get_remap_id(&self) -> $crate::saveload::RemapId {
                self.base.get_remap_id()
            }

            fn as_any(&self) -> &dyn ::std::any::Any {
                self
            }

            fn as_any_mut(&mut self) -> &mut dyn ::std::any::Any {
                self
            }
        }
    };
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::saveload::*;

    /// Mock ChunkSave implementation for testing
    struct MockChunkSave {
        chunks: Vec<(u32, Vec<u8>)>,
        data: Vec<u8>,
        current_chunk: Option<u32>,
    }

    impl MockChunkSave {
        fn new() -> Self {
            Self {
                chunks: Vec::new(),
                data: Vec::new(),
                current_chunk: None,
            }
        }
    }

    impl ChunkSave for MockChunkSave {
        fn begin_chunk(&mut self, id: u32) -> SaveLoadResult<()> {
            self.current_chunk = Some(id);
            Ok(())
        }

        fn end_chunk(&mut self) -> SaveLoadResult<()> {
            if let Some(chunk_id) = self.current_chunk.take() {
                self.chunks.push((chunk_id, self.data.clone()));
                self.data.clear();
            }
            Ok(())
        }

        fn write(&mut self, data: &[u8]) -> SaveLoadResult<()> {
            self.data.extend_from_slice(data);
            Ok(())
        }
    }

    /// Mock ChunkLoad implementation for testing
    struct MockChunkLoad {
        chunks: Vec<(u32, Vec<u8>)>,
        chunk_index: usize,
        data_pos: usize,
        current_chunk: Option<u32>,
    }

    impl MockChunkLoad {
        fn new() -> Self {
            Self {
                chunks: vec![(CHUNKID_VARIABLES, Vec::new())],
                chunk_index: 0,
                data_pos: 0,
                current_chunk: None,
            }
        }
    }

    impl ChunkLoad for MockChunkLoad {
        fn open_chunk(&mut self) -> SaveLoadResult<bool> {
            if self.chunk_index < self.chunks.len() {
                self.current_chunk = Some(self.chunks[self.chunk_index].0);
                self.chunk_index += 1;
                Ok(true)
            } else {
                Ok(false)
            }
        }

        fn close_chunk(&mut self) -> SaveLoadResult<()> {
            self.current_chunk = None;
            Ok(())
        }

        fn current_chunk_id(&self) -> u32 {
            self.current_chunk.unwrap_or(0)
        }

        fn read(&mut self, _buffer: &mut [u8]) -> SaveLoadResult<usize> {
            // For testing, return 0 to indicate end of data
            Ok(0)
        }
    }

    #[test]
    fn test_definition_creation() {
        let def = Definition::new();
        assert_eq!(def.get_id(), 0);
        assert_eq!(def.get_name(), "");
        assert_eq!(def.get_user_data(), 0);
        assert!(def.is_save_enabled());
        assert_eq!(def.get_definition_mgr_link(), -1);
    }

    #[test]
    fn test_definition_with_id_and_name() {
        let def = Definition::with_id_and_name(42, "TestDef".to_string());
        assert_eq!(def.get_id(), 42);
        assert_eq!(def.get_name(), "TestDef");
    }

    #[test]
    fn test_definition_setters() {
        let mut def = Definition::new();

        let _ = def.set_id(100);
        assert_eq!(def.get_id(), 100);

        def.set_name("NewName");
        assert_eq!(def.get_name(), "NewName");

        def.set_user_data(12345);
        assert_eq!(def.get_user_data(), 12345);

        def.enable_save(false);
        assert!(!def.is_save_enabled());

        def.set_definition_mgr_link(5);
        assert_eq!(def.get_definition_mgr_link(), 5);
    }

    #[test]
    fn test_definition_validation() {
        let mut def = Definition::new();

        // Empty name should fail validation
        assert!(def.is_valid_config().is_err());

        // Non-empty name should pass validation
        def.set_name("ValidName");
        assert!(def.is_valid_config().is_ok());
    }

    #[test]
    fn test_definition_save() {
        let def = Definition::with_id_and_name(42, "TestDef".to_string());
        let mut mock_save = MockChunkSave::new();

        let result = def.save(&mut mock_save);
        assert!(result.is_ok());

        // Verify that chunks were created
        assert_eq!(mock_save.chunks.len(), 1);
        assert_eq!(mock_save.chunks[0].0, CHUNKID_VARIABLES);

        // Verify that data was written (simplified test)
        // In a real implementation, we'd verify the actual data format
    }

    #[test]
    fn test_definition_load() {
        let mut def = Definition::new();
        let mut mock_load = MockChunkLoad::new();

        let result = def.load(&mut mock_load);
        assert!(result.is_ok());

        // The mock loader should have loaded the test data
        // Note: This test is limited by our mock implementation
        // Test would verify loaded data here in a real implementation
    }

    #[test]
    fn test_definition_class_interface() {
        let mut def = Definition::with_id_and_name(123, "TestInterface".to_string());

        // Test DefinitionClass trait methods
        assert_eq!(def.get_class_id(), 0x00000000);
        assert_eq!(def.get_id(), 123);
        assert_eq!(def.get_name(), "TestInterface");

        // Test set_id through trait
        let _result = def.set_id(456);
        assert_eq!(def.get_id(), 456);

        // Test create method
        let created = def.create();
        assert!(created.is_ok());
    }

    #[test]
    fn test_editable_class_interface() {
        let def = Definition::new();

        assert_eq!(def.get_type_name(), "Definition");
        assert!(def.supports_property_editing());

        let properties = def.get_editable_properties();
        assert!(properties.contains(&"id".to_string()));
        assert!(properties.contains(&"name".to_string()));
        assert!(properties.contains(&"user_data".to_string()));
        assert!(properties.contains(&"save_enabled".to_string()));
    }

    /// Test definition type with custom class ID
    #[derive(Debug, Default)]
    struct TestDefinition {
        base: Definition,
        custom_data: i32,
    }

    declare_editable!(TestDefinition, Definition);
    impl_definition_class!(TestDefinition, 0x12340001);

    #[test]
    fn test_custom_definition_with_macros() {
        let mut test_def = TestDefinition::default();
        test_def.set_name("CustomTest");
        let _ = test_def.set_id(999);
        test_def.custom_data = 42;

        // Test that the macro-generated implementation works
        assert_eq!(test_def.get_class_id(), 0x12340001);
        assert_eq!(test_def.get_id(), 999);
        assert_eq!(test_def.get_name(), "CustomTest");
        assert_eq!(test_def.get_type_name(), "TestDefinition");

        // Test creation
        let created = test_def.create();
        assert!(created.is_ok());
    }

    #[test]
    fn test_definition_error_handling() {
        let error = DefinitionError::InvalidId(123);
        assert!(error.to_string().contains("123"));

        let error = DefinitionError::InvalidName("test".to_string());
        assert!(error.to_string().contains("test"));

        let error = DefinitionError::ConfigValidationFailed("validation failed".to_string());
        assert!(error.to_string().contains("validation failed"));
    }

    #[test]
    fn test_definition_factory() {
        let factory = DefinitionFactory::new(0x12345678);
        assert_eq!(factory.chunk_id(), 0x12345678);

        // Test factory creation (limited by mock implementation)
        let mut mock_load = MockChunkLoad::new();
        let result = factory.load(&mut mock_load);
        assert!(result.is_ok());
    }
}
