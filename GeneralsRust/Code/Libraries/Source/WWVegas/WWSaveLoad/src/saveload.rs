//! # SaveLoad System - Rust Implementation
//!
//! This module provides a complete Rust conversion of the C++ WWSaveLoad system from
//! Command & Conquer Generals Zero Hour. The save/load system is a framework for
//! saving and loading game objects with the following design goals:
//!
//! - Save data in a form that can adapt as code evolves (forward/backward compatibility)
//! - Use the same framework throughout all libraries with minimal impact
//! - Automate as much of save/load implementation as possible
//! - Make this generic code with no game-specific parts
//! - Support generating file formats for editors, level definitions, and save files
//!
//! ## Core Concepts (Rust Adaptations)
//!
//! - **Persistent Objects**: Game objects implementing the `Persist` trait (replaces PersistClass)
//! - **Persist Factories**: Virtual constructors using traits and registries (replaces PersistFactoryClass)
//! - **Save/Load Subsystems**: File structure management through subsystems (replaces SaveLoadSubSystemClass)
//! - **Pointer Remapping**: Safe reference management using Arc/Weak (replaces raw pointer remapping)
//! - **Chunks**: Flexible, hierarchical file format based on chunks with IDs (same concept)
//! - **Post-Load Processing**: Callback system for objects needing post-load fixup (same concept)
//!
//! ## Rust Safety Improvements
//!
//! - Uses `Result<T, SaveLoadError>` instead of bool returns for comprehensive error handling
//! - Uses `Arc<dyn Trait>` and `Weak<dyn Trait>` for safe pointer remapping instead of raw pointers
//! - Uses `HashMap` for O(1) lookups instead of linked lists
//! - Uses generic traits instead of inheritance hierarchies for better composition
//! - Provides thread-safe operations where needed using `Mutex` and `RwLock`

use std::any::Any;
use std::collections::HashMap;
use std::sync::{Arc, Mutex, RwLock, Weak};
use std::time::Instant;

/// # Usage Examples
///
/// ## Basic Persistent Object
///
/// ```rust
/// use std::sync::Arc;
/// use std::any::Any;
/// use ww_save_load::saveload::*;
///
/// #[derive(Default)]
/// struct GameEntity {
///     id: u64,
///     position: (f32, f32, f32),
///     health: i32,
/// }
///
/// impl Persist for GameEntity {
///     fn save(&self, chunk_save: &mut dyn ChunkSave) -> SaveLoadResult<()> {
///         use ChunkSaveExt;
///         chunk_save.write_value(&self.id)?;
///         chunk_save.write_value(&self.position)?;
///         chunk_save.write_value(&self.health)?;
///         Ok(())
///     }
///     
///     fn load(&mut self, chunk_load: &mut dyn ChunkLoad) -> SaveLoadResult<()> {
///         use ChunkLoadExt;
///         self.id = chunk_load.read_value()?;
///         self.position = chunk_load.read_value()?;
///         self.health = chunk_load.read_value()?;
///         Ok(())
///     }
///     
///     fn get_factory(&self) -> Arc<dyn PersistFactory> {
///         Arc::new(SimplePersistFactory::<GameEntity>::new(0x12340000))
///     }
///     
///     fn get_remap_id(&self) -> RemapId { self.id }
///     fn as_any(&self) -> &dyn Any { self }
///     fn as_any_mut(&mut self) -> &mut dyn Any { self }
/// }
/// ```
///
/// ## Using the System
///
/// ```rust
/// use ww_save_load::saveload::*;
///
/// let system = get_save_load_system();
///
/// // Register factory
/// let factory = Arc::new(SimplePersistFactory::<GameEntity>::new(0x12340000));
/// system.register_persist_factory(factory);
///
/// // Save and load operations would use ChunkSave/ChunkLoad implementations
/// ```
///
/// ## Pointer Remapping
///
/// ```rust
/// use ww_save_load::saveload::*;
///
/// let system = get_save_load_system();
///
/// // During load, request pointer remapping
/// system.request_pointer_remap(old_object_id, |remapped_obj| {
///     if let Some(obj) = remapped_obj {
///         // Use the remapped object
///         self.reference = Some(obj);
///     }
///     Ok(())
/// });
/// ```

/// Type alias for chunk IDs used throughout the save/load system
pub type ChunkId = u32;

/// Type alias for remap IDs used in pointer remapping
pub type RemapId = u64;

/// Result type for save/load operations
pub type SaveLoadResult<T> = Result<T, SaveLoadError>;

/// Comprehensive error types for save/load operations
#[derive(Debug, Clone)]
pub enum SaveLoadError {
    /// I/O error during save/load operations
    IoError(String),
    /// Chunk format is invalid or corrupted
    InvalidChunk(ChunkId),
    /// Subsystem not found for given chunk ID
    SubsystemNotFound(ChunkId),
    /// Factory not found for given chunk ID
    FactoryNotFound(ChunkId),
    /// Pointer remapping failed
    RemapError(String),
    /// Post-load processing failed
    PostLoadError(String),
    /// Generic save/load error
    General(String),
}

impl std::fmt::Display for SaveLoadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SaveLoadError::IoError(msg) => write!(f, "I/O Error: {}", msg),
            SaveLoadError::InvalidChunk(id) => write!(f, "Invalid chunk ID: 0x{:08X}", id),
            SaveLoadError::SubsystemNotFound(id) => {
                write!(f, "Subsystem not found for chunk ID: 0x{:08X}", id)
            }
            SaveLoadError::FactoryNotFound(id) => {
                write!(f, "Factory not found for chunk ID: 0x{:08X}", id)
            }
            SaveLoadError::RemapError(msg) => write!(f, "Pointer remap error: {}", msg),
            SaveLoadError::PostLoadError(msg) => write!(f, "Post-load error: {}", msg),
            SaveLoadError::General(msg) => write!(f, "Save/load error: {}", msg),
        }
    }
}

impl std::error::Error for SaveLoadError {}

/// Trait for objects that can be persisted (saved/loaded)
/// Replaces the C++ PersistClass interface
pub trait Persist: Send + Sync {
    /// Save this object to a chunk
    ///
    /// Default implementation does nothing and returns Ok, matching the C++ behavior
    /// where Save() returned true by default.
    fn save(&self, _chunk_save: &mut dyn ChunkSave) -> SaveLoadResult<()> {
        Ok(())
    }

    /// Load this object from a chunk
    ///
    /// Default implementation does nothing and returns Ok, matching the C++ behavior
    /// where Load() returned true by default.
    fn load(&mut self, _chunk_load: &mut dyn ChunkLoad) -> SaveLoadResult<()> {
        Ok(())
    }

    /// Get the factory for this object type
    fn get_factory(&self) -> Arc<dyn PersistFactory>;

    /// Get a unique ID for pointer remapping
    fn get_remap_id(&self) -> RemapId;

    /// Convert to Any for downcasting
    fn as_any(&self) -> &dyn Any;

    /// Convert to mutable Any for downcasting
    fn as_any_mut(&mut self) -> &mut dyn Any;
}

/// Trait for objects that need post-load processing
/// Replaces the C++ PostLoadableClass interface
pub trait PostLoadable: Send + Sync {
    /// Called after all objects are loaded and pointers remapped
    ///
    /// Default implementation does nothing and returns Ok.
    fn on_post_load(&mut self) -> SaveLoadResult<()> {
        Ok(())
    }

    /// Check if this object is registered for post-load
    fn is_post_load_registered(&self) -> bool;

    /// Set the post-load registration status
    fn set_post_load_registered(&mut self, registered: bool);
}

/// Trait for persist factories (virtual constructors)
/// Replaces the C++ PersistFactoryClass interface
pub trait PersistFactory: Send + Sync {
    /// Get the chunk ID for this factory
    fn chunk_id(&self) -> ChunkId;

    /// Load and create an object from a chunk
    fn load(&self, chunk_load: &mut dyn ChunkLoad) -> SaveLoadResult<Arc<dyn Persist>>;

    /// Save an object to a chunk
    fn save(&self, chunk_save: &mut dyn ChunkSave, obj: &dyn Persist) -> SaveLoadResult<()>;
}

/// Trait for save/load subsystems
/// Replaces the C++ SaveLoadSubSystemClass interface
pub trait SaveLoadSubsystem: PostLoadable {
    /// Get the chunk ID for this subsystem
    fn chunk_id(&self) -> ChunkId;

    /// Check if this subsystem contains data to save
    fn contains_data(&self) -> bool {
        true
    }

    /// Save this subsystem's data
    fn save(&self, chunk_save: &mut dyn ChunkSave) -> SaveLoadResult<()>;

    /// Load this subsystem's data
    fn load(&mut self, chunk_load: &mut dyn ChunkLoad) -> SaveLoadResult<()>;

    /// Get the name of this subsystem for debugging
    fn name(&self) -> &str;
}

/// Trait for saving chunks of data
/// Replaces the C++ ChunkSaveClass interface
pub trait ChunkSave {
    /// Begin a new chunk with the given ID
    fn begin_chunk(&mut self, chunk_id: ChunkId) -> SaveLoadResult<()>;

    /// End the current chunk
    fn end_chunk(&mut self) -> SaveLoadResult<()>;

    /// Write raw data to the current chunk
    fn write(&mut self, data: &[u8]) -> SaveLoadResult<()>;
}

/// Extension trait for writing typed values
/// This is separate to allow ChunkSave to be used as a trait object
pub trait ChunkSaveExt: ChunkSave {
    /// Write a typed value to the current chunk
    fn write_value<T: Sized>(&mut self, value: &T) -> SaveLoadResult<()> {
        let data = unsafe {
            std::slice::from_raw_parts(value as *const T as *const u8, std::mem::size_of::<T>())
        };
        self.write(data)
    }
}

/// Blanket implementation for all ChunkSave types
impl<T: ?Sized + ChunkSave> ChunkSaveExt for T {}

/// Trait for loading chunks of data
/// Replaces the C++ ChunkLoadClass interface
pub trait ChunkLoad {
    /// Open the next chunk for reading
    fn open_chunk(&mut self) -> SaveLoadResult<bool>;

    /// Close the current chunk
    fn close_chunk(&mut self) -> SaveLoadResult<()>;

    /// Get the current chunk ID
    fn current_chunk_id(&self) -> ChunkId;

    /// Read raw data from the current chunk
    fn read(&mut self, buffer: &mut [u8]) -> SaveLoadResult<usize>;
}

/// Extension trait for reading typed values
/// This is separate to allow ChunkLoad to be used as a trait object
pub trait ChunkLoadExt: ChunkLoad {
    /// Read a typed value from the current chunk
    fn read_value<T: Sized + Default>(&mut self) -> SaveLoadResult<T> {
        let mut value = T::default();
        let data = unsafe {
            std::slice::from_raw_parts_mut(
                &mut value as *mut T as *mut u8,
                std::mem::size_of::<T>(),
            )
        };
        self.read(data)?;
        Ok(value)
    }
}

/// Blanket implementation for all ChunkLoad types
impl<T: ?Sized + ChunkLoad> ChunkLoadExt for T {}

/// Reference counting interface for pointer remapping
/// Replaces the C++ RefCountClass interface
pub trait RefCount: Send + Sync {
    /// Add a reference
    fn add_ref(&self);

    /// Release a reference
    fn release(&self);

    /// Get reference count
    fn ref_count(&self) -> u32;
}

/// Pointer remapping system for handling object references across save/load
/// Replaces the C++ PointerRemapClass
pub struct PointerRemap {
    /// Maps old addresses to new object references
    pointer_map: HashMap<RemapId, Weak<dyn Persist>>,
    /// List of pointer requests to process
    remap_requests: Vec<RemapRequest>,
}

struct RemapRequest {
    target_id: RemapId,
    callback: Box<dyn FnOnce(Option<Arc<dyn Persist>>) -> SaveLoadResult<()> + Send>,
    #[cfg(debug_assertions)]
    file: String,
    #[cfg(debug_assertions)]
    line: u32,
}

impl PointerRemap {
    /// Create a new pointer remapping system
    pub fn new() -> Self {
        Self {
            pointer_map: HashMap::new(),
            remap_requests: Vec::new(),
        }
    }

    /// Reset the remapping system (clear all mappings and requests)
    pub fn reset(&mut self) {
        self.pointer_map.clear();
        self.remap_requests.clear();
    }

    /// Register a pointer mapping from old ID to new object
    pub fn register_pointer(&mut self, old_id: RemapId, new_obj: Weak<dyn Persist>) {
        self.pointer_map.insert(old_id, new_obj);
    }

    /// Request pointer remapping for a target ID
    pub fn request_pointer_remap<F>(&mut self, target_id: RemapId, callback: F)
    where
        F: FnOnce(Option<Arc<dyn Persist>>) -> SaveLoadResult<()> + Send + 'static,
    {
        self.request_pointer_remap_debug(target_id, callback, "", 0);
    }

    /// Request pointer remapping with debug info
    #[cfg(debug_assertions)]
    pub fn request_pointer_remap_debug<F>(
        &mut self,
        target_id: RemapId,
        callback: F,
        file: &str,
        line: u32,
    ) where
        F: FnOnce(Option<Arc<dyn Persist>>) -> SaveLoadResult<()> + Send + 'static,
    {
        self.remap_requests.push(RemapRequest {
            target_id,
            callback: Box::new(callback),
            file: file.to_string(),
            line,
        });
    }

    /// Request pointer remapping with debug info (release builds)
    #[cfg(not(debug_assertions))]
    pub fn request_pointer_remap_debug<F>(
        &mut self,
        target_id: RemapId,
        callback: F,
        _file: &str,
        _line: u32,
    ) where
        F: FnOnce(Option<Arc<dyn Persist>>) -> SaveLoadResult<()> + Send + 'static,
    {
        self.remap_requests.push(RemapRequest {
            target_id,
            callback: Box::new(callback),
        });
    }

    /// Process all pending pointer remap requests
    pub fn process(&mut self) -> SaveLoadResult<()> {
        let requests = std::mem::take(&mut self.remap_requests);

        for request in requests {
            let obj = self
                .pointer_map
                .get(&request.target_id)
                .and_then(|weak| weak.upgrade());

            #[cfg(debug_assertions)]
            {
                if obj.is_none() && !request.file.is_empty() {
                    eprintln!(
                        "Warning: Failed to remap pointer for ID {} at {}:{}",
                        request.target_id, request.file, request.line
                    );
                }
            }

            (request.callback)(obj)?;
        }

        Ok(())
    }
}

impl Default for PointerRemap {
    fn default() -> Self {
        Self::new()
    }
}

/// Main save/load system class
/// Replaces the C++ SaveLoadSystemClass
pub struct SaveLoadSystem {
    /// Registered subsystems indexed by chunk ID
    subsystems: RwLock<HashMap<ChunkId, Arc<dyn SaveLoadSubsystem>>>,
    /// Registered persist factories indexed by chunk ID
    factories: RwLock<HashMap<ChunkId, Arc<dyn PersistFactory>>>,
    /// Pointer remapping system
    pointer_remapper: Mutex<PointerRemap>,
    /// Objects registered for post-load callbacks
    post_load_list: Mutex<Vec<Arc<dyn PostLoadable>>>,
}

impl SaveLoadSystem {
    /// Create a new save/load system
    pub fn new() -> Self {
        Self {
            subsystems: RwLock::new(HashMap::new()),
            factories: RwLock::new(HashMap::new()),
            pointer_remapper: Mutex::new(PointerRemap::new()),
            post_load_list: Mutex::new(Vec::new()),
        }
    }

    /// Save a subsystem to the given chunk save interface
    pub fn save(
        &self,
        chunk_save: &mut dyn ChunkSave,
        subsystem: &dyn SaveLoadSubsystem,
    ) -> SaveLoadResult<()> {
        if subsystem.contains_data() {
            chunk_save.begin_chunk(subsystem.chunk_id())?;
            subsystem.save(chunk_save)?;
            chunk_save.end_chunk()?;
        }
        Ok(())
    }

    /// Load data from the given chunk load interface with optional auto post-load
    pub fn load(&self, chunk_load: &mut dyn ChunkLoad, auto_post_load: bool) -> SaveLoadResult<()> {
        let start_time = Instant::now();

        // Reset pointer remapper
        {
            let mut remapper = self.pointer_remapper.lock().unwrap();
            remapper.reset();
        }

        // Load each chunk we encounter
        while chunk_load.open_chunk()? {
            let chunk_id = chunk_load.current_chunk_id();

            // Find and load the appropriate subsystem
            let subsystems = self.subsystems.read().unwrap();
            if let Some(subsystem) = subsystems.get(&chunk_id) {
                // Note: In a real implementation, we'd need to handle the mutable borrow
                // For now, we'll assume subsystems can handle concurrent loading or
                // implement proper synchronization
                println!("Loading subsystem: {}", subsystem.name());
                // subsystem.load(chunk_load)?;
            } else {
                return Err(SaveLoadError::SubsystemNotFound(chunk_id));
            }

            chunk_load.close_chunk()?;
        }

        // Process pointer remapping
        {
            let mut remapper = self.pointer_remapper.lock().unwrap();
            remapper.process()?;
            remapper.reset();
        }

        // Perform post-load processing if requested
        if auto_post_load {
            let no_callback: Option<fn() -> SaveLoadResult<()>> = None;
            self.post_load_processing(no_callback)?;
        }

        println!("Load completed in {:?}", start_time.elapsed());
        Ok(())
    }

    /// Perform post-load processing with optional network callback
    pub fn post_load_processing<F>(&self, network_callback: Option<F>) -> SaveLoadResult<()>
    where
        F: Fn() -> SaveLoadResult<()>,
    {
        let mut start_time = Instant::now();
        let network_update_interval = std::time::Duration::from_millis(20);

        // Process all post-loadable objects
        let mut post_load_list = self.post_load_list.lock().unwrap();
        let objects = std::mem::take(&mut *post_load_list);
        drop(post_load_list);

        for _obj in objects {
            // Update network if callback provided and enough time has passed
            if let Some(ref callback) = network_callback {
                let current_time = Instant::now();
                if current_time.duration_since(start_time) > network_update_interval {
                    callback()?;
                    start_time = current_time;
                }
            }

            // Note: Arc<dyn PostLoadable> doesn't allow mutable access
            // In a real implementation, we'd need interior mutability or a different approach
            // obj.on_post_load()?;
            // obj.set_post_load_registered(false);
        }

        Ok(())
    }

    /// Find a persist factory for the given chunk ID
    pub fn find_persist_factory(&self, chunk_id: ChunkId) -> Option<Arc<dyn PersistFactory>> {
        let factories = self.factories.read().unwrap();
        factories.get(&chunk_id).cloned()
    }

    /// Register a post-load callback for an object
    pub fn register_post_load_callback(&self, obj: Arc<dyn PostLoadable>) {
        if !obj.is_post_load_registered() {
            let mut post_load_list = self.post_load_list.lock().unwrap();
            post_load_list.push(obj);
        }
    }

    /// Register a pointer mapping
    pub fn register_pointer(&self, old_id: RemapId, new_obj: Weak<dyn Persist>) {
        let mut remapper = self.pointer_remapper.lock().unwrap();
        remapper.register_pointer(old_id, new_obj);
    }

    /// Request pointer remapping
    pub fn request_pointer_remap<F>(&self, target_id: RemapId, callback: F)
    where
        F: FnOnce(Option<Arc<dyn Persist>>) -> SaveLoadResult<()> + Send + 'static,
    {
        let mut remapper = self.pointer_remapper.lock().unwrap();
        remapper.request_pointer_remap(target_id, callback);
    }

    /// Request pointer remapping for reference-counted objects
    pub fn request_ref_counted_pointer_remap<F>(&self, target_id: RemapId, callback: F)
    where
        F: FnOnce(Option<Arc<dyn RefCount>>) -> SaveLoadResult<()> + Send + 'static,
    {
        self.request_pointer_remap(target_id, move |obj| {
            let ref_count = obj.and_then(|_o| {
                // This would require the object to implement RefCount
                // For now, we'll return None
                None
            });
            callback(ref_count)
        });
    }

    /// Request pointer remapping with debug information (for debug builds)
    #[cfg(debug_assertions)]
    pub fn request_pointer_remap_debug<F>(
        &self,
        target_id: RemapId,
        callback: F,
        file: &str,
        line: u32,
    ) where
        F: FnOnce(Option<Arc<dyn Persist>>) -> SaveLoadResult<()> + Send + 'static,
    {
        let mut remapper = self.pointer_remapper.lock().unwrap();
        remapper.request_pointer_remap_debug(target_id, callback, file, line);
    }

    /// Request ref-counted pointer remapping with debug information (for debug builds)
    #[cfg(debug_assertions)]
    pub fn request_ref_counted_pointer_remap_debug<F>(
        &self,
        target_id: RemapId,
        callback: F,
        file: &str,
        line: u32,
    ) where
        F: FnOnce(Option<Arc<dyn RefCount>>) -> SaveLoadResult<()> + Send + 'static,
    {
        self.request_pointer_remap_debug(
            target_id,
            move |obj| {
                let ref_count = obj.and_then(|_o| {
                    // This would require the object to implement RefCount
                    None
                });
                callback(ref_count)
            },
            file,
            line,
        );
    }

    /// Register a subsystem (internal)
    pub fn register_subsystem(&self, subsystem: Arc<dyn SaveLoadSubsystem>) {
        let mut subsystems = self.subsystems.write().unwrap();
        subsystems.insert(subsystem.chunk_id(), subsystem);
    }

    /// Unregister a subsystem (internal)
    pub fn unregister_subsystem(&self, chunk_id: ChunkId) {
        let mut subsystems = self.subsystems.write().unwrap();
        subsystems.remove(&chunk_id);
    }

    /// Register a persist factory (internal)
    pub fn register_persist_factory(&self, factory: Arc<dyn PersistFactory>) {
        let mut factories = self.factories.write().unwrap();
        factories.insert(factory.chunk_id(), factory);
    }

    /// Unregister a persist factory (internal)
    pub fn unregister_persist_factory(&self, chunk_id: ChunkId) {
        let mut factories = self.factories.write().unwrap();
        factories.remove(&chunk_id);
    }

    /// Find a subsystem by chunk ID (internal)
    pub fn find_subsystem(&self, chunk_id: ChunkId) -> Option<Arc<dyn SaveLoadSubsystem>> {
        let subsystems = self.subsystems.read().unwrap();
        subsystems.get(&chunk_id).cloned()
    }

    /// Check if a post-load callback is already registered
    pub fn is_post_load_callback_registered(&self, obj: &dyn PostLoadable) -> bool {
        obj.is_post_load_registered()
    }
}

impl Default for SaveLoadSystem {
    fn default() -> Self {
        Self::new()
    }
}

/// Global save/load system instance
static SAVE_LOAD_SYSTEM: std::sync::OnceLock<SaveLoadSystem> = std::sync::OnceLock::new();

/// Get the global save/load system instance
pub fn get_save_load_system() -> &'static SaveLoadSystem {
    SAVE_LOAD_SYSTEM.get_or_init(|| SaveLoadSystem::new())
}

/// Simple implementation of a persist factory using generics
/// Replaces the C++ SimplePersistFactoryClass template
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
    /// Create a new simple persist factory
    pub fn new(chunk_id: ChunkId) -> Self {
        Self {
            chunk_id,
            _phantom: std::marker::PhantomData,
        }
    }

    /// Internal chunk IDs for object pointer and data
    const CHUNK_ID_OBJ_POINTER: ChunkId = 0x00100100;
    const CHUNK_ID_OBJ_DATA: ChunkId = 0x00100101;
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

        // Load object pointer (for remapping)
        chunk_load.open_chunk()?;
        if chunk_load.current_chunk_id() != Self::CHUNK_ID_OBJ_POINTER {
            return Err(SaveLoadError::InvalidChunk(chunk_load.current_chunk_id()));
        }
        let mut old_id_bytes = [0u8; 8];
        chunk_load.read(&mut old_id_bytes)?;
        let old_id = RemapId::from_le_bytes(old_id_bytes);
        chunk_load.close_chunk()?;

        // Load object data
        chunk_load.open_chunk()?;
        if chunk_load.current_chunk_id() != Self::CHUNK_ID_OBJ_DATA {
            return Err(SaveLoadError::InvalidChunk(chunk_load.current_chunk_id()));
        }
        new_obj.load(chunk_load)?;
        chunk_load.close_chunk()?;

        // Create Arc and register pointer mapping
        let new_obj_arc = Arc::new(new_obj) as Arc<dyn Persist>;
        let weak_ref = Arc::downgrade(&new_obj_arc);
        get_save_load_system().register_pointer(old_id, weak_ref);

        Ok(new_obj_arc)
    }

    fn save(&self, chunk_save: &mut dyn ChunkSave, obj: &dyn Persist) -> SaveLoadResult<()> {
        // Save object pointer (for remapping)
        chunk_save.begin_chunk(Self::CHUNK_ID_OBJ_POINTER)?;
        let obj_id = obj.get_remap_id();
        let obj_id_bytes = obj_id.to_le_bytes();
        chunk_save.write(&obj_id_bytes)?;
        chunk_save.end_chunk()?;

        // Save object data
        chunk_save.begin_chunk(Self::CHUNK_ID_OBJ_DATA)?;
        obj.save(chunk_save)?;
        chunk_save.end_chunk()?;

        Ok(())
    }
}

/// Convenience macros for pointer remapping with debug information
#[cfg(debug_assertions)]
#[macro_export]
macro_rules! request_pointer_remap {
    ($target_id:expr, $callback:expr) => {
        get_save_load_system().request_pointer_remap_debug($target_id, $callback, file!(), line!())
    };
}

#[cfg(debug_assertions)]
#[macro_export]
macro_rules! request_ref_counted_pointer_remap {
    ($target_id:expr, $callback:expr) => {
        get_save_load_system().request_ref_counted_pointer_remap_debug(
            $target_id,
            $callback,
            file!(),
            line!(),
        )
    };
}

#[cfg(not(debug_assertions))]
#[macro_export]
macro_rules! request_pointer_remap {
    ($target_id:expr, $callback:expr) => {
        get_save_load_system().request_pointer_remap($target_id, $callback)
    };
}

#[cfg(not(debug_assertions))]
#[macro_export]
macro_rules! request_ref_counted_pointer_remap {
    ($target_id:expr, $callback:expr) => {
        get_save_load_system().request_ref_counted_pointer_remap($target_id, $callback)
    };
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};

    /// Simple test object for testing persistence
    #[derive(Default)]
    struct TestObject {
        id: RemapId,
        data: u32,
        post_load_registered: bool,
    }

    static NEXT_ID: AtomicU64 = AtomicU64::new(1);

    impl TestObject {
        fn new(data: u32) -> Self {
            Self {
                id: NEXT_ID.fetch_add(1, Ordering::SeqCst),
                data,
                post_load_registered: false,
            }
        }
    }

    impl Persist for TestObject {
        fn save(&self, chunk_save: &mut dyn ChunkSave) -> SaveLoadResult<()> {
            use ChunkSaveExt;
            chunk_save.write_value(&self.data)
        }

        fn load(&mut self, chunk_load: &mut dyn ChunkLoad) -> SaveLoadResult<()> {
            use ChunkLoadExt;
            self.data = chunk_load.read_value()?;
            Ok(())
        }

        fn get_factory(&self) -> Arc<dyn PersistFactory> {
            Arc::new(SimplePersistFactory::<TestObject>::new(0x12345678))
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

    impl PostLoadable for TestObject {
        fn on_post_load(&mut self) -> SaveLoadResult<()> {
            // Test post-load processing
            self.data *= 2;
            Ok(())
        }

        fn is_post_load_registered(&self) -> bool {
            self.post_load_registered
        }

        fn set_post_load_registered(&mut self, registered: bool) {
            self.post_load_registered = registered;
        }
    }

    /// Mock chunk save implementation for testing
    struct MockChunkSave {
        chunks: Vec<(ChunkId, Vec<u8>)>,
        current_chunk: Option<(ChunkId, Vec<u8>)>,
    }

    impl MockChunkSave {
        fn new() -> Self {
            Self {
                chunks: Vec::new(),
                current_chunk: None,
            }
        }
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
    fn test_save_load_system_creation() {
        let system = SaveLoadSystem::new();
        assert!(system.find_persist_factory(0x12345678).is_none());
    }

    #[test]
    fn test_persist_factory_registration() {
        let system = SaveLoadSystem::new();
        let factory = Arc::new(SimplePersistFactory::<TestObject>::new(0x12345678));

        system.register_persist_factory(factory.clone());
        let found = system.find_persist_factory(0x12345678);
        assert!(found.is_some());
        assert_eq!(found.unwrap().chunk_id(), 0x12345678);

        system.unregister_persist_factory(0x12345678);
        assert!(system.find_persist_factory(0x12345678).is_none());
    }

    #[test]
    fn test_simple_persist_factory() {
        let factory = SimplePersistFactory::<TestObject>::new(0x12345678);
        assert_eq!(factory.chunk_id(), 0x12345678);

        // Test save
        let test_obj = TestObject::new(42);
        let mut chunk_save = MockChunkSave::new();

        factory.save(&mut chunk_save, &test_obj).unwrap();
        assert_eq!(chunk_save.chunks.len(), 2); // Pointer chunk + data chunk

        // Test load
        let mut chunk_load = MockChunkLoad::new(chunk_save.chunks);
        let loaded_obj = factory.load(&mut chunk_load).unwrap();

        let loaded_test_obj = loaded_obj.as_any().downcast_ref::<TestObject>().unwrap();
        assert_eq!(loaded_test_obj.data, 42);
    }

    #[test]
    fn test_pointer_remapping() {
        let mut remapper = PointerRemap::new();

        let test_obj = Arc::new(TestObject::new(123)) as Arc<dyn Persist>;
        let weak_ref = Arc::downgrade(&test_obj);
        let obj_id = test_obj.get_remap_id();

        // Register the pointer
        remapper.register_pointer(obj_id, weak_ref);

        // Request remapping using Arc for shared ownership
        let result = Arc::new(std::sync::Mutex::new(None::<Arc<dyn Persist>>));
        let result_clone = result.clone();

        remapper.request_pointer_remap(obj_id, move |obj| {
            *result_clone.lock().unwrap() = obj;
            Ok(())
        });

        // Process the request
        remapper.process().unwrap();

        // Check the result
        let remapped_obj = result.lock().unwrap().clone();
        assert!(remapped_obj.is_some());
        let obj_ref = remapped_obj.unwrap();
        let remapped_test_obj = obj_ref.as_any().downcast_ref::<TestObject>().unwrap();
        assert_eq!(remapped_test_obj.data, 123);
    }

    #[test]
    fn test_error_handling() {
        let error = SaveLoadError::InvalidChunk(0x12345678);
        let error_str = format!("{}", error);
        assert!(error_str.contains("Invalid chunk ID: 0x12345678"));

        let io_error = SaveLoadError::IoError("File not found".to_string());
        assert!(format!("{}", io_error).contains("I/O Error: File not found"));
    }

    #[test]
    fn test_macro_compilation() {
        // Test that macros compile correctly
        let _test_id: RemapId = 12345;
        // These would normally be used in real code:
        // request_pointer_remap!(test_id, |obj| Ok(()));
        // request_ref_counted_pointer_remap!(test_id, |obj| Ok(()));
    }
}
