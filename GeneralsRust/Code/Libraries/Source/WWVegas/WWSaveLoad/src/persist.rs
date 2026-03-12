//! # Persist System - Core Persistence Interface
//!
//! This module provides the core persistence traits and implementations that form the foundation
//! of the WWSaveLoad system in Rust. It converts the C++ PersistClass hierarchy to idiomatic
//! Rust traits that leverage the type system for safety and performance.
//!
//! ## Original C++ Design
//!
//! The original C++ system used:
//! ```cpp
//! class PersistClass : public PostLoadableClass
//! {
//! public:
//!     virtual const PersistFactoryClass &	Get_Factory (void) const = 0;
//!     virtual bool Save (ChunkSaveClass &csave) { return true; }
//!     virtual bool Load (ChunkLoadClass &cload) { return true; }
//! };
//! ```
//!
//! ## Rust Adaptation
//!
//! The Rust version uses traits for composition instead of inheritance:
//! - `Persist` trait replaces `PersistClass`
//! - `PostLoadable` trait replaces `PostLoadableClass`
//! - `PersistFactory` trait replaces `PersistFactoryClass`
//! - Result types replace boolean returns for comprehensive error handling
//! - Arc/Weak references replace raw pointers for memory safety
//!
//! ## Core Concepts
//!
//! ### Persistent Objects
//! Objects that can be saved and loaded must implement the `Persist` trait:
//!
//! ```rust
//! use std::sync::Arc;
//! use std::any::Any;
//!
//! #[derive(Default)]
//! struct GameEntity {
//!     id: u64,
//!     health: i32,
//! }
//!
//! impl Persist for GameEntity {
//!     fn save(&self, chunk_save: &mut dyn ChunkSave) -> SaveLoadResult<()> {
//!         // Save implementation
//!         Ok(())
//!     }
//!     
//!     fn load(&mut self, chunk_load: &mut dyn ChunkLoad) -> SaveLoadResult<()> {
//!         // Load implementation
//!         Ok(())
//!     }
//!     
//!     fn get_factory(&self) -> Arc<dyn PersistFactory> {
//!         Arc::new(SimplePersistFactory::<GameEntity>::new(0x12340000))
//!     }
//!     
//!     fn get_remap_id(&self) -> RemapId { self.id }
//!     fn as_any(&self) -> &dyn Any { self }
//!     fn as_any_mut(&mut self) -> &mut dyn Any { self }
//! }
//! ```
//!
//! ### Post-Load Processing
//! Objects that need post-load processing implement `PostLoadable`:
//!
//! ```rust
//! impl PostLoadable for GameEntity {
//!     fn on_post_load(&mut self) -> SaveLoadResult<()> {
//!         // Initialize references, validate state, etc.
//!         Ok(())
//!     }
//!     
//!     fn is_post_load_registered(&self) -> bool { false }
//!     fn set_post_load_registered(&mut self, registered: bool) { }
//! }
//! ```
//!
//! ### Factory Registration
//! Factories are registered with the save/load system:
//!
//! ```rust
//! let system = get_save_load_system();
//! let factory = Arc::new(SimplePersistFactory::<GameEntity>::new(0x12340000));
//! system.register_persist_factory(factory);
//! ```

use std::collections::HashMap;
use std::fmt;
use std::sync::{Arc, Mutex};

use crate::saveload::{
    get_save_load_system, ChunkId, ChunkLoad, ChunkSave, Persist, PersistFactory, RemapId,
    SaveLoadError, SaveLoadResult,
};

/// Simple generic persist factory implementation
///
/// This struct provides a simple implementation of the PersistFactory trait
/// for types that implement Persist + Default. It automates the common pattern
/// of creating an object, loading its data, and handling pointer remapping.
///
/// This replaces the C++ SimplePersistFactoryClass template and provides
/// the same functionality with Rust's type system safety.
///
/// # Type Parameters
/// * `T` - The type of object this factory creates. Must implement Persist + Default + 'static
///
/// # Usage
/// ```rust
/// // Create and register a factory for MyObject
/// let factory = Arc::new(SimplePersistFactory::<MyObject>::new(0x12340000));
/// get_save_load_system().register_persist_factory(factory);
/// ```
///
/// # Chunk Structure
/// The SimplePersistFactory uses a two-chunk structure:
/// 1. Object Pointer Chunk (0x00100100) - Contains the original object ID for remapping
/// 2. Object Data Chunk (0x00100101) - Contains the actual object data
///
/// This matches the C++ implementation's chunk structure for compatibility.
pub struct SimplePersistFactory<T>
where
    T: Persist + Default + 'static,
{
    chunk_id: ChunkId,
    _phantom: std::marker::PhantomData<T>,
}

impl<T> SimplePersistFactory<T>
where
    T: Persist + Default + 'static,
{
    /// Create a new simple persist factory with the given chunk ID
    ///
    /// # Arguments
    /// * `chunk_id` - The chunk ID that identifies objects created by this factory
    ///
    /// # Returns
    /// A new SimplePersistFactory instance
    pub fn new(chunk_id: ChunkId) -> Self {
        Self {
            chunk_id,
            _phantom: std::marker::PhantomData,
        }
    }

    /// Internal chunk IDs used by the simple persist factory
    /// These match the C++ implementation for compatibility
    pub const CHUNK_ID_OBJ_POINTER: ChunkId = 0x00100100;
    pub const CHUNK_ID_OBJ_DATA: ChunkId = 0x00100101;
}

impl<T> PersistFactory for SimplePersistFactory<T>
where
    T: Persist + Default + 'static,
{
    fn chunk_id(&self) -> ChunkId {
        self.chunk_id
    }

    fn load(&self, chunk_load: &mut dyn ChunkLoad) -> SaveLoadResult<Arc<dyn Persist>> {
        let mut new_obj = T::default();

        // Load object pointer chunk for remapping
        if !chunk_load.open_chunk()? {
            return Err(SaveLoadError::General(
                "Expected object pointer chunk".to_string(),
            ));
        }

        if chunk_load.current_chunk_id() != Self::CHUNK_ID_OBJ_POINTER {
            return Err(SaveLoadError::InvalidChunk(chunk_load.current_chunk_id()));
        }

        // Read the old object ID for pointer remapping
        let mut old_id_bytes = [0u8; 8];
        chunk_load.read(&mut old_id_bytes)?;
        let old_id = RemapId::from_le_bytes(old_id_bytes);
        chunk_load.close_chunk()?;

        // Load object data chunk
        if !chunk_load.open_chunk()? {
            return Err(SaveLoadError::General(
                "Expected object data chunk".to_string(),
            ));
        }

        if chunk_load.current_chunk_id() != Self::CHUNK_ID_OBJ_DATA {
            return Err(SaveLoadError::InvalidChunk(chunk_load.current_chunk_id()));
        }

        // Load the actual object data
        new_obj.load(chunk_load)?;
        chunk_load.close_chunk()?;

        // Create Arc and register pointer mapping
        let new_obj_arc = Arc::new(new_obj) as Arc<dyn Persist>;
        let weak_ref = Arc::downgrade(&new_obj_arc);
        get_save_load_system().register_pointer(old_id, weak_ref);

        Ok(new_obj_arc)
    }

    fn save(&self, chunk_save: &mut dyn ChunkSave, obj: &dyn Persist) -> SaveLoadResult<()> {
        // Save object pointer chunk for remapping
        chunk_save.begin_chunk(Self::CHUNK_ID_OBJ_POINTER)?;
        let obj_id = obj.get_remap_id();
        let obj_id_bytes = obj_id.to_le_bytes();
        chunk_save.write(&obj_id_bytes)?;
        chunk_save.end_chunk()?;

        // Save object data chunk
        chunk_save.begin_chunk(Self::CHUNK_ID_OBJ_DATA)?;
        obj.save(chunk_save)?;
        chunk_save.end_chunk()?;

        Ok(())
    }
}

impl<T> fmt::Debug for SimplePersistFactory<T>
where
    T: Persist + Default + 'static,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SimplePersistFactory")
            .field("chunk_id", &format!("0x{:08X}", self.chunk_id))
            .field("type", &std::any::type_name::<T>())
            .finish()
    }
}

/// Registry for managing persist factories
///
/// This struct provides a centralized registry for persist factories, allowing
/// factories to be registered and looked up by chunk ID. This replaces the
/// C++ factory registration system with a type-safe Rust implementation.
///
/// # Thread Safety
/// The registry uses Arc and Mutex internally to provide thread-safe access
/// to the factory collection.
#[derive(Default)]
pub struct PersistFactoryRegistry {
    factories: Mutex<HashMap<ChunkId, Arc<dyn PersistFactory>>>,
}

impl PersistFactoryRegistry {
    /// Create a new empty persist factory registry
    pub fn new() -> Self {
        Self {
            factories: Mutex::new(HashMap::new()),
        }
    }

    /// Register a persist factory with the registry
    ///
    /// # Arguments
    /// * `factory` - The factory to register
    ///
    /// # Returns
    /// * `Ok(())` if registration was successful
    /// * `Err(SaveLoadError)` if a factory with the same chunk ID is already registered
    pub fn register_factory(&self, factory: Arc<dyn PersistFactory>) -> SaveLoadResult<()> {
        let mut factories = self.factories.lock().unwrap();
        let chunk_id = factory.chunk_id();

        if factories.contains_key(&chunk_id) {
            return Err(SaveLoadError::General(format!(
                "Factory with chunk ID 0x{:08X} is already registered",
                chunk_id
            )));
        }

        factories.insert(chunk_id, factory);
        Ok(())
    }

    /// Unregister a persist factory from the registry
    ///
    /// # Arguments
    /// * `chunk_id` - The chunk ID of the factory to unregister
    ///
    /// # Returns
    /// `true` if a factory was unregistered, `false` if no factory was found
    pub fn unregister_factory(&self, chunk_id: ChunkId) -> bool {
        let mut factories = self.factories.lock().unwrap();
        factories.remove(&chunk_id).is_some()
    }

    /// Find a persist factory by chunk ID
    ///
    /// # Arguments
    /// * `chunk_id` - The chunk ID to search for
    ///
    /// # Returns
    /// * `Some(Arc<dyn PersistFactory>)` if a factory was found
    /// * `None` if no factory was found for the given chunk ID
    pub fn find_factory(&self, chunk_id: ChunkId) -> Option<Arc<dyn PersistFactory>> {
        let factories = self.factories.lock().unwrap();
        factories.get(&chunk_id).cloned()
    }

    /// Get the number of registered factories
    pub fn factory_count(&self) -> usize {
        let factories = self.factories.lock().unwrap();
        factories.len()
    }

    /// Clear all registered factories
    pub fn clear(&self) {
        let mut factories = self.factories.lock().unwrap();
        factories.clear();
    }

    /// Get all registered chunk IDs
    ///
    /// This is useful for debugging and diagnostics.
    ///
    /// # Returns
    /// A vector containing all registered chunk IDs
    pub fn registered_chunk_ids(&self) -> Vec<ChunkId> {
        let factories = self.factories.lock().unwrap();
        factories.keys().copied().collect()
    }
}

impl fmt::Debug for PersistFactoryRegistry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let factories = self.factories.lock().unwrap();
        f.debug_struct("PersistFactoryRegistry")
            .field("factory_count", &factories.len())
            .field(
                "registered_chunk_ids",
                &factories.keys().collect::<Vec<_>>(),
            )
            .finish()
    }
}

/// Convenience macro for implementing default PostLoadable behavior
///
/// Many objects don't need custom post-load processing, so this macro
/// provides a standard implementation.
///
/// # Usage
/// ```rust
/// use ww_save_load::impl_default_post_loadable;
///
/// #[derive(Default)]
/// struct MyObject {
///     data: i32,
///     post_load_registered: bool,
/// }
///
/// impl_default_post_loadable!(MyObject, post_load_registered);
/// ```
#[macro_export]
macro_rules! impl_default_post_loadable {
    ($type:ty, $field:ident) => {
        impl $crate::saveload::PostLoadable for $type {
            fn on_post_load(&mut self) -> $crate::saveload::SaveLoadResult<()> {
                Ok(())
            }

            fn is_post_load_registered(&self) -> bool {
                self.$field
            }

            fn set_post_load_registered(&mut self, registered: bool) {
                self.$field = registered;
            }
        }
    };
}

/// Convenience macro for registering a simple persist factory
///
/// This macro simplifies the common pattern of creating and registering
/// a SimplePersistFactory for a type.
///
/// # Usage
/// ```rust
/// use ww_save_load::register_persist_factory;
///
/// // Register a factory for MyObject with chunk ID 0x12340000
/// register_persist_factory!(MyObject, 0x12340000);
/// ```
#[macro_export]
macro_rules! register_persist_factory {
    ($type:ty, $chunk_id:expr) => {{
        let factory = std::sync::Arc::new($crate::persist::SimplePersistFactory::<$type>::new(
            $chunk_id,
        ));
        $crate::saveload::get_save_load_system().register_persist_factory(factory);
    }};
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::saveload::{ChunkLoadExt, ChunkSaveExt, PostLoadable};
    use std::any::Any;
    use std::sync::atomic::{AtomicU64, Ordering};

    /// Test object for persistence testing
    #[derive(Debug, Default, PartialEq)]
    struct TestPersistObject {
        id: RemapId,
        value: i32,
        name: String,
        post_load_registered: bool,
        post_load_called: bool,
    }

    static NEXT_ID: AtomicU64 = AtomicU64::new(1);

    impl TestPersistObject {
        fn new(value: i32, name: String) -> Self {
            Self {
                id: NEXT_ID.fetch_add(1, Ordering::SeqCst),
                value,
                name,
                post_load_registered: false,
                post_load_called: false,
            }
        }
    }

    impl PostLoadable for TestPersistObject {
        fn on_post_load(&mut self) -> SaveLoadResult<()> {
            self.post_load_called = true;
            self.value *= 2; // Double the value as a test
            Ok(())
        }

        fn is_post_load_registered(&self) -> bool {
            self.post_load_registered
        }

        fn set_post_load_registered(&mut self, registered: bool) {
            self.post_load_registered = registered;
        }
    }

    impl Persist for TestPersistObject {
        fn save(&self, chunk_save: &mut dyn ChunkSave) -> SaveLoadResult<()> {
            chunk_save.write_value(&self.value)?;

            // Save string length and data
            let name_bytes = self.name.as_bytes();
            let name_len = name_bytes.len() as u32;
            chunk_save.write_value(&name_len)?;
            chunk_save.write(name_bytes)?;

            Ok(())
        }

        fn load(&mut self, chunk_load: &mut dyn ChunkLoad) -> SaveLoadResult<()> {
            self.value = chunk_load.read_value()?;

            // Load string length and data
            let name_len: u32 = chunk_load.read_value()?;
            let mut name_bytes = vec![0u8; name_len as usize];
            chunk_load.read(&mut name_bytes)?;
            self.name = String::from_utf8(name_bytes)
                .map_err(|e| SaveLoadError::General(format!("Invalid UTF-8 in name: {}", e)))?;

            Ok(())
        }

        fn get_factory(&self) -> Arc<dyn PersistFactory> {
            Arc::new(SimplePersistFactory::<TestPersistObject>::new(0x12345678))
        }

        fn get_remap_id(&self) -> RemapId {
            self.id
        }

        fn as_any(&self) -> &dyn Any {
            self
        }

        fn as_any_mut(&mut self) -> &mut dyn Any {
            self
        }
    }

    /// Mock chunk save implementation for testing
    #[derive(Default)]
    struct MockChunkSave {
        chunks: Vec<(ChunkId, Vec<u8>)>,
        current_chunk: Option<(ChunkId, Vec<u8>)>,
    }

    impl ChunkSave for MockChunkSave {
        fn begin_chunk(&mut self, chunk_id: ChunkId) -> SaveLoadResult<()> {
            if self.current_chunk.is_some() {
                return Err(SaveLoadError::General("Chunk already open".to_string()));
            }
            self.current_chunk = Some((chunk_id, Vec::new()));
            Ok(())
        }

        fn end_chunk(&mut self) -> SaveLoadResult<()> {
            if let Some(chunk) = self.current_chunk.take() {
                self.chunks.push(chunk);
                Ok(())
            } else {
                Err(SaveLoadError::General("No chunk open".to_string()))
            }
        }

        fn write(&mut self, data: &[u8]) -> SaveLoadResult<()> {
            if let Some((_, ref mut chunk_data)) = self.current_chunk {
                chunk_data.extend_from_slice(data);
                Ok(())
            } else {
                Err(SaveLoadError::General("No chunk open".to_string()))
            }
        }
    }

    /// Mock chunk load implementation for testing
    struct MockChunkLoad {
        chunks: Vec<(ChunkId, Vec<u8>)>,
        current_index: usize,
        current_chunk: Option<(ChunkId, Vec<u8>, usize)>, // ID, data, read position
    }

    impl MockChunkLoad {
        fn new(chunks: Vec<(ChunkId, Vec<u8>)>) -> Self {
            Self {
                chunks,
                current_index: 0,
                current_chunk: None,
            }
        }
    }

    impl ChunkLoad for MockChunkLoad {
        fn open_chunk(&mut self) -> SaveLoadResult<bool> {
            if self.current_chunk.is_some() {
                return Err(SaveLoadError::General("Chunk already open".to_string()));
            }

            if self.current_index < self.chunks.len() {
                let (id, data) = self.chunks[self.current_index].clone();
                self.current_chunk = Some((id, data, 0));
                self.current_index += 1;
                Ok(true)
            } else {
                Ok(false)
            }
        }

        fn close_chunk(&mut self) -> SaveLoadResult<()> {
            if self.current_chunk.is_some() {
                self.current_chunk = None;
                Ok(())
            } else {
                Err(SaveLoadError::General("No chunk open".to_string()))
            }
        }

        fn current_chunk_id(&self) -> ChunkId {
            self.current_chunk.as_ref().map_or(0, |(id, _, _)| *id)
        }

        fn read(&mut self, buffer: &mut [u8]) -> SaveLoadResult<usize> {
            if let Some((_, ref data, ref mut pos)) = self.current_chunk {
                let available = data.len().saturating_sub(*pos);
                let to_read = buffer.len().min(available);

                if to_read > 0 {
                    buffer[..to_read].copy_from_slice(&data[*pos..*pos + to_read]);
                    *pos += to_read;
                }

                Ok(to_read)
            } else {
                Err(SaveLoadError::General("No chunk open".to_string()))
            }
        }
    }

    #[test]
    fn test_persist_trait_default_implementations() {
        // Create a simple object that uses default implementations
        struct DefaultPersistObject {
            id: RemapId,
            post_load_registered: bool,
        }

        impl PostLoadable for DefaultPersistObject {
            fn is_post_load_registered(&self) -> bool {
                self.post_load_registered
            }

            fn set_post_load_registered(&mut self, registered: bool) {
                self.post_load_registered = registered;
            }
        }

        impl Persist for DefaultPersistObject {
            // Uses default save/load implementations

            fn get_factory(&self) -> Arc<dyn PersistFactory> {
                Arc::new(SimplePersistFactory::<TestPersistObject>::new(0x12345678))
            }

            fn get_remap_id(&self) -> RemapId {
                self.id
            }

            fn as_any(&self) -> &dyn Any {
                self
            }

            fn as_any_mut(&mut self) -> &mut dyn Any {
                self
            }
        }

        let obj = DefaultPersistObject {
            id: 1,
            post_load_registered: false,
        };

        // Test default save (should do nothing and return Ok)
        let mut chunk_save = MockChunkSave::default();
        let result = obj.save(&mut chunk_save);
        assert!(result.is_ok());

        // Test default load (should do nothing and return Ok)
        let mut obj_for_load = DefaultPersistObject {
            id: 2,
            post_load_registered: false,
        };
        let chunks = vec![];
        let mut chunk_load = MockChunkLoad::new(chunks);
        let result = obj_for_load.load(&mut chunk_load);
        assert!(result.is_ok());
    }

    #[test]
    fn test_post_loadable_trait() {
        let mut obj = TestPersistObject::new(10, "test".to_string());

        // Test initial state
        assert!(!obj.is_post_load_registered());
        assert!(!obj.post_load_called);
        assert_eq!(obj.value, 10);

        // Test registration
        obj.set_post_load_registered(true);
        assert!(obj.is_post_load_registered());

        // Test post-load processing
        obj.on_post_load().unwrap();
        assert!(obj.post_load_called);
        assert_eq!(obj.value, 20); // Should be doubled
    }

    #[test]
    fn test_simple_persist_factory() {
        let factory = SimplePersistFactory::<TestPersistObject>::new(0x12345678);
        assert_eq!(factory.chunk_id(), 0x12345678);

        // Create and save an object
        let original_obj = TestPersistObject::new(42, "Hello World".to_string());
        let original_id = original_obj.get_remap_id();

        let mut chunk_save = MockChunkSave::default();
        factory.save(&mut chunk_save, &original_obj).unwrap();

        // Should have exactly 2 chunks (pointer + data)
        assert_eq!(chunk_save.chunks.len(), 2);

        // Verify chunk IDs
        assert_eq!(
            chunk_save.chunks[0].0,
            SimplePersistFactory::<TestPersistObject>::CHUNK_ID_OBJ_POINTER
        );
        assert_eq!(
            chunk_save.chunks[1].0,
            SimplePersistFactory::<TestPersistObject>::CHUNK_ID_OBJ_DATA
        );

        // Load the object back
        let mut chunk_load = MockChunkLoad::new(chunk_save.chunks);
        let loaded_obj_arc = factory.load(&mut chunk_load).unwrap();
        let loaded_obj = loaded_obj_arc
            .as_any()
            .downcast_ref::<TestPersistObject>()
            .unwrap();

        // Verify loaded data
        assert_eq!(loaded_obj.value, 42);
        assert_eq!(loaded_obj.name, "Hello World");

        // Verify that new ID is different (new object instance)
        assert_ne!(loaded_obj.id, original_id);
    }

    #[test]
    fn test_persist_factory_registry() {
        let registry = PersistFactoryRegistry::new();

        // Test empty registry
        assert_eq!(registry.factory_count(), 0);
        assert!(registry.find_factory(0x12345678).is_none());

        // Register a factory
        let factory = Arc::new(SimplePersistFactory::<TestPersistObject>::new(0x12345678));
        registry.register_factory(factory.clone()).unwrap();

        // Test registry state
        assert_eq!(registry.factory_count(), 1);
        let found_factory = registry.find_factory(0x12345678);
        assert!(found_factory.is_some());
        assert_eq!(found_factory.unwrap().chunk_id(), 0x12345678);

        // Test duplicate registration fails
        let duplicate_factory =
            Arc::new(SimplePersistFactory::<TestPersistObject>::new(0x12345678));
        let result = registry.register_factory(duplicate_factory);
        assert!(result.is_err());

        // Test unregistration
        assert!(registry.unregister_factory(0x12345678));
        assert!(!registry.unregister_factory(0x12345678)); // Should return false the second time
        assert_eq!(registry.factory_count(), 0);

        // Test clear
        registry.register_factory(factory).unwrap();
        registry.clear();
        assert_eq!(registry.factory_count(), 0);
    }

    #[test]
    fn test_registered_chunk_ids() {
        let registry = PersistFactoryRegistry::new();

        let factory1 = Arc::new(SimplePersistFactory::<TestPersistObject>::new(0x11111111));
        let factory2 = Arc::new(SimplePersistFactory::<TestPersistObject>::new(0x22222222));

        registry.register_factory(factory1).unwrap();
        registry.register_factory(factory2).unwrap();

        let mut chunk_ids = registry.registered_chunk_ids();
        chunk_ids.sort(); // HashMap order is not guaranteed

        assert_eq!(chunk_ids, vec![0x11111111, 0x22222222]);
    }

    #[test]
    fn test_simple_persist_factory_debug() {
        let factory = SimplePersistFactory::<TestPersistObject>::new(0x12345678);
        let debug_str = format!("{:?}", factory);

        assert!(debug_str.contains("SimplePersistFactory"));
        assert!(debug_str.contains("0x12345678"));
        assert!(debug_str.contains("TestPersistObject"));
    }

    #[test]
    fn test_macro_compilation() {
        // Test that the macros compile correctly

        // Test impl_default_post_loadable macro
        struct TestMacroObject {
            post_load_registered: bool,
        }

        impl_default_post_loadable!(TestMacroObject, post_load_registered);

        let mut obj = TestMacroObject {
            post_load_registered: false,
        };
        assert!(!obj.is_post_load_registered());
        obj.set_post_load_registered(true);
        assert!(obj.is_post_load_registered());

        // The register_persist_factory macro would be tested in integration tests
        // since it depends on the global save/load system
    }

    #[test]
    fn test_error_conditions() {
        let factory = SimplePersistFactory::<TestPersistObject>::new(0x12345678);

        // Test loading with invalid chunk structure
        let invalid_chunks = vec![
            (0xBEEFCAFE, vec![1, 2, 3, 4]), // Wrong chunk ID
        ];

        let mut chunk_load = MockChunkLoad::new(invalid_chunks);
        let result = factory.load(&mut chunk_load);
        assert!(result.is_err());

        if let Err(SaveLoadError::InvalidChunk(chunk_id)) = result {
            assert_eq!(chunk_id, 0xBEEFCAFE);
        } else {
            panic!("Expected InvalidChunk error");
        }
    }
}
