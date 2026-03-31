/***********************************************************************************************
 ***              C O N F I D E N T I A L  ---  W E S T W O O D  S T U D I O S               ***
 ***********************************************************************************************
 *                                                                                             *
 *                 Project Name : WWSaveLoad                                                   *
 *                                                                                             *
 *                     $Archive:: /Commando/Code/wwsaveload/wwsaveload.h                      $*
 *                                                                                             *
 *              Original Author:: Greg Hjelstrom                                               *
 *                                                                                             *
 *                      $Author:: Greg_h                                                      $*
 *                                                                                             *
 *                     $Modtime:: 3/28/00 9:20a                                               $*
 *                                                                                             *
 *                    $Revision:: 2                                                           $*
 *                                                                                             *
 *---------------------------------------------------------------------------------------------*
 * Functions:                                                                                  *
 * - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - */

use std::collections::HashMap;
use std::fmt;
use std::sync::{Mutex, OnceLock, RwLock};

/// Errors that can occur during save/load operations
#[derive(Debug)]
pub enum SaveLoadError {
    NotInitialized,
    DefinitionNotFound(u32),
    NamedDefinitionNotFound(String),
    IoError(std::io::Error),
}

impl fmt::Display for SaveLoadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SaveLoadError::NotInitialized => write!(f, "Definition manager not initialized"),
            SaveLoadError::DefinitionNotFound(id) => {
                write!(f, "Definition with ID {} not found", id)
            }
            SaveLoadError::NamedDefinitionNotFound(name) => {
                write!(f, "Definition with name '{}' not found", name)
            }
            SaveLoadError::IoError(err) => write!(f, "IO error: {}", err),
        }
    }
}

impl std::error::Error for SaveLoadError {}

impl From<std::io::Error> for SaveLoadError {
    fn from(err: std::io::Error) -> Self {
        SaveLoadError::IoError(err)
    }
}

/// Result type for save/load operations
pub type SaveLoadResult<T> = Result<T, SaveLoadError>;

/// Placeholder for Definition class - will be replaced with actual definition implementation
#[derive(Debug, Clone)]
pub struct Definition {
    pub id: u32,
    pub name: String,
    pub class_id: u32,
    // Additional definition fields would go here
}

impl Definition {
    pub fn new(id: u32, name: String, class_id: u32) -> Self {
        Self { id, name, class_id }
    }

    pub fn get_id(&self) -> u32 {
        self.id
    }

    pub fn get_name(&self) -> &str {
        &self.name
    }

    pub fn get_class_id(&self) -> u32 {
        self.class_id
    }
}

/// Definition Manager - Rust equivalent of DefinitionMgrClass
///
/// This manages all definitions used by the save/load system, providing
/// registration, lookup, and cleanup functionality.
#[derive(Debug)]
pub struct DefinitionManager {
    /// Sorted array of definitions for fast binary search lookup
    sorted_definitions: Vec<Definition>,
    /// Hash map for fast name-based lookups
    definition_hash: HashMap<String, Vec<Definition>>,
}

impl Default for DefinitionManager {
    fn default() -> Self {
        Self::new()
    }
}

impl DefinitionManager {
    /// Create a new definition manager
    pub fn new() -> Self {
        Self {
            sorted_definitions: Vec::new(),
            definition_hash: HashMap::new(),
        }
    }

    /// Find a definition by ID using binary search
    pub fn find_definition(&self, id: u32, _twiddle: bool) -> Option<&Definition> {
        self.sorted_definitions
            .binary_search_by_key(&id, |def| def.get_id())
            .ok()
            .map(|index| &self.sorted_definitions[index])
    }

    /// Find a definition by name
    pub fn find_named_definition(&self, name: &str, _twiddle: bool) -> Option<&Definition> {
        self.sorted_definitions
            .iter()
            .find(|def| def.get_name().eq_ignore_ascii_case(name))
    }

    /// Find a typed definition by name and class ID
    pub fn find_typed_definition(
        &self,
        name: &str,
        class_id: u32,
        _twiddle: bool,
    ) -> Option<&Definition> {
        // First check the hash table for fast lookup
        let lower_name = name.to_lowercase();
        if let Some(definitions) = self.definition_hash.get(&lower_name) {
            for def in definitions {
                if def.get_class_id() == class_id {
                    return Some(def);
                }
            }
        }

        // Fall back to linear search if not found in hash
        self.sorted_definitions
            .iter()
            .find(|def| def.get_class_id() == class_id && def.get_name().eq_ignore_ascii_case(name))
    }

    /// Register a new definition
    pub fn register_definition(&mut self, definition: Definition) -> SaveLoadResult<()> {
        // Check if definition with same ID already exists
        if self.find_definition(definition.get_id(), false).is_some() {
            return Err(SaveLoadError::DefinitionNotFound(definition.get_id()));
        }

        // Add to hash table for name-based lookup
        let lower_name = definition.get_name().to_lowercase();
        self.definition_hash
            .entry(lower_name)
            .or_insert_with(Vec::new)
            .push(definition.clone());

        // Insert into sorted array using binary search to find insertion point
        match self
            .sorted_definitions
            .binary_search_by_key(&definition.get_id(), |def| def.get_id())
        {
            Ok(_) => {
                // Should not happen due to the check above, but handle gracefully
                Err(SaveLoadError::DefinitionNotFound(definition.get_id()))
            }
            Err(pos) => {
                self.sorted_definitions.insert(pos, definition);
                Ok(())
            }
        }
    }

    /// Unregister a definition by ID
    pub fn unregister_definition(&mut self, id: u32) -> SaveLoadResult<()> {
        match self
            .sorted_definitions
            .binary_search_by_key(&id, |def| def.get_id())
        {
            Ok(index) => {
                let definition = self.sorted_definitions.remove(index);

                // Remove from hash table
                let lower_name = definition.get_name().to_lowercase();
                if let Some(defs) = self.definition_hash.get_mut(&lower_name) {
                    defs.retain(|def| def.get_id() != id);
                    if defs.is_empty() {
                        self.definition_hash.remove(&lower_name);
                    }
                }

                Ok(())
            }
            Err(_) => Err(SaveLoadError::DefinitionNotFound(id)),
        }
    }

    /// Get the first definition (for enumeration)
    pub fn get_first(&self) -> Option<&Definition> {
        self.sorted_definitions.first()
    }

    /// Get the next definition after the given one (for enumeration)
    pub fn get_next(&self, current_def: &Definition) -> Option<&Definition> {
        match self
            .sorted_definitions
            .binary_search_by_key(&current_def.get_id(), |def| def.get_id())
        {
            Ok(index) if index + 1 < self.sorted_definitions.len() => {
                Some(&self.sorted_definitions[index + 1])
            }
            _ => None,
        }
    }

    /// Free all definitions and clear internal state
    pub fn free_definitions(&mut self) {
        self.sorted_definitions.clear();
        self.definition_hash.clear();
    }

    /// Get the count of registered definitions
    pub fn definition_count(&self) -> usize {
        self.sorted_definitions.len()
    }

    /// List all available definitions (for debugging)
    pub fn list_available_definitions(&self) {
        println!("Available definitions:");
        for def in &self.sorted_definitions {
            println!("  >{}<", def.get_name());
        }
    }
}

/// Global definition manager instance
static THE_DEFINITION_MGR: OnceLock<Mutex<DefinitionManager>> = OnceLock::new();
static DEFINITION_MGR_LOCK: RwLock<()> = RwLock::new(());

/// Get a reference to the global definition manager
fn get_definition_manager() -> std::sync::MutexGuard<'static, DefinitionManager> {
    THE_DEFINITION_MGR
        .get_or_init(|| Mutex::new(DefinitionManager::new()))
        .lock()
        .unwrap()
}

/**
** WWSaveLoad
** The Init and Shutdown functions should be called once by the App.
*/
pub struct WWSaveLoad;

impl WWSaveLoad {
    /// Initialize the WWSaveLoad system
    ///
    /// This function should be called once by the application during startup.
    /// It initializes the global definition manager and prepares the save/load system.
    pub fn init() {
        let _lock = DEFINITION_MGR_LOCK.write();

        // Initialize the definition manager if not already done
        let _ = get_definition_manager();

        // Additional initialization logic would go here
        println!("WWSaveLoad system initialized");
    }

    /// Shutdown the WWSaveLoad system
    ///
    /// This function should be called once by the application during shutdown.
    /// It frees all definitions and cleans up the save/load system resources.
    pub fn shutdown() {
        let _lock = DEFINITION_MGR_LOCK.write();

        // Free all definitions from the global definition manager
        let def_mgr = get_definition_manager();
        def_mgr.free_definitions();

        println!("WWSaveLoad system shutdown complete");
    }

    /// Get a reference to the global definition manager
    ///
    /// This provides access to the definition manager for registration and lookup operations.
    ///
    /// This function should only be called after WWSaveLoad::init() has been called.
    pub fn get_definition_manager() -> std::sync::MutexGuard<'static, DefinitionManager> {
        get_definition_manager()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ww_save_load_init_shutdown() {
        WWSaveLoad::init();
        WWSaveLoad::shutdown();
    }

    #[test]
    fn test_definition_manager() {
        let mut mgr = DefinitionManager::new();

        let def1 = Definition::new(1, "TestDef1".to_string(), 100);
        let def2 = Definition::new(2, "TestDef2".to_string(), 200);

        // Test registration
        assert!(mgr.register_definition(def1.clone()).is_ok());
        assert!(mgr.register_definition(def2.clone()).is_ok());
        assert_eq!(mgr.definition_count(), 2);

        // Test lookup by ID
        let found = mgr.find_definition(1, false);
        assert!(found.is_some());
        assert_eq!(found.unwrap().get_name(), "TestDef1");

        // Test lookup by name
        let found = mgr.find_named_definition("TestDef2", false);
        assert!(found.is_some());
        assert_eq!(found.unwrap().get_id(), 2);

        // Test typed lookup
        let found = mgr.find_typed_definition("TestDef1", 100, false);
        assert!(found.is_some());
        assert_eq!(found.unwrap().get_id(), 1);

        // Test enumeration
        let first = mgr.get_first();
        assert!(first.is_some());
        assert_eq!(first.unwrap().get_id(), 1);

        let next = mgr.get_next(first.unwrap());
        assert!(next.is_some());
        assert_eq!(next.unwrap().get_id(), 2);

        // Test cleanup
        mgr.free_definitions();
        assert_eq!(mgr.definition_count(), 0);
    }

    #[test]
    fn test_definition_unregistration() {
        let mut mgr = DefinitionManager::new();

        let def = Definition::new(42, "TestDef".to_string(), 100);
        assert!(mgr.register_definition(def).is_ok());
        assert_eq!(mgr.definition_count(), 1);

        assert!(mgr.unregister_definition(42).is_ok());
        assert_eq!(mgr.definition_count(), 0);

        // Should fail to unregister non-existent definition
        assert!(mgr.unregister_definition(42).is_err());
    }
}
