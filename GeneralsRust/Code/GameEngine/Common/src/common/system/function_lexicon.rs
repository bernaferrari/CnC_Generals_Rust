//! Function lexicon for callback management
//!
//! This module provides a function pointer registry system for managing callbacks
//! and function lookups by name, similar to the C++ FunctionLexicon system.

use std::any::Any;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use crate::common::{
    name_key_generator::NameKeyGenerator,
    rts::NameKeyType,
    system::subsystem_interface::{SubsystemInterface, SubsystemResult, SubsystemState},
};

/// Invalid name key constant
const INVALID_NAME_KEY: NameKeyType = 0;

/// Function pointer type for various callback functions
///
/// Since raw pointers don't implement Send + Sync, we need to wrap them
/// in a safe wrapper that asserts thread safety for function pointers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct FunctionPtr(pub usize);

unsafe impl Send for FunctionPtr {}
unsafe impl Sync for FunctionPtr {}

impl FunctionPtr {
    /// Create a new function pointer from a raw function pointer
    pub fn new(func: *const ()) -> Self {
        Self(func as usize)
    }

    /// Convert back to a raw function pointer
    pub fn as_ptr(self) -> *const () {
        self.0 as *const ()
    }

    /// Create a null function pointer
    pub fn null() -> Self {
        Self(0)
    }

    /// Check if the function pointer is null
    pub fn is_null(self) -> bool {
        self.0 == 0
    }
}

/// Table entry for function lookups
#[derive(Debug, Clone)]
pub struct TableEntry {
    pub key: NameKeyType,
    pub name: String,
    pub func: Option<FunctionPtr>,
}

impl TableEntry {
    /// Create a new table entry
    pub fn new(name: &str, func: Option<FunctionPtr>) -> Self {
        Self {
            key: INVALID_NAME_KEY, // Will be set when loaded into lexicon
            name: name.to_string(),
            func,
        }
    }

    /// Create a new table entry from a raw function pointer
    pub fn with_raw_ptr(name: &str, func: Option<*const ()>) -> Self {
        Self {
            key: INVALID_NAME_KEY,
            name: name.to_string(),
            func: func.map(FunctionPtr::new),
        }
    }
}

/// Table indices for different function types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TableIndex {
    Any = -1,
    GameWinSystem = 0,
    GameWinInput,
    GameWinTooltip,
    GameWinDeviceDraw,
    GameWinDraw,
    WinLayoutInit,
    WinLayoutDeviceInit,
    WinLayoutUpdate,
    WinLayoutShutdown,
    MaxFunctionTables,
}

impl From<i32> for TableIndex {
    fn from(value: i32) -> Self {
        match value {
            -1 => TableIndex::Any,
            0 => TableIndex::GameWinSystem,
            1 => TableIndex::GameWinInput,
            2 => TableIndex::GameWinTooltip,
            3 => TableIndex::GameWinDeviceDraw,
            4 => TableIndex::GameWinDraw,
            5 => TableIndex::WinLayoutInit,
            6 => TableIndex::WinLayoutDeviceInit,
            7 => TableIndex::WinLayoutUpdate,
            8 => TableIndex::WinLayoutShutdown,
            _ => TableIndex::MaxFunctionTables,
        }
    }
}

/// Function lexicon for managing function pointer lookups
///
/// This system allows registration and lookup of function pointers by name,
/// organized into different tables for different types of callbacks.
pub struct FunctionLexicon {
    /// Function tables indexed by TableIndex
    tables: HashMap<TableIndex, Vec<TableEntry>>,
    /// Current subsystem state
    state: SubsystemState,
}

impl FunctionLexicon {
    /// Create a new function lexicon
    pub fn new() -> Self {
        Self {
            tables: HashMap::new(),
            state: SubsystemState::Uninitialized,
        }
    }

    /// Load a table of function entries
    ///
    /// This processes the table entries, generates keys for names, and stores
    /// the table for later lookup.
    pub fn load_table(&mut self, mut table: Vec<TableEntry>, table_index: TableIndex) {
        for entry in &mut table {
            if !entry.name.is_empty() {
                entry.key = NameKeyGenerator::name_to_key(&entry.name);
            }
        }

        // Store the table
        self.tables.insert(table_index, table);
    }

    /// Recompute name keys for all loaded tables.
    ///
    /// C++ `FunctionLexicon::init()` always re-runs `loadTable(...)` over static
    /// entries, which regenerates keys. Preserve that behavior for already-loaded
    /// Rust tables during init/reset.
    fn rekey_loaded_tables(&mut self) {
        for table in self.tables.values_mut() {
            for entry in table {
                if !entry.name.is_empty() {
                    entry.key = NameKeyGenerator::name_to_key(&entry.name);
                }
            }
        }
    }

    /// Find a function by key in a specific table
    pub fn key_to_func(&self, key: NameKeyType, table_index: TableIndex) -> Option<FunctionPtr> {
        if key == INVALID_NAME_KEY {
            return None;
        }

        if let Some(table) = self.tables.get(&table_index) {
            for entry in table {
                if entry.key == key {
                    return entry.func;
                }
            }
        }

        None
    }

    /// Find a function by key, searching all tables if table_index is Any
    pub fn find_function(&self, key: NameKeyType, table_index: TableIndex) -> Option<FunctionPtr> {
        if key == INVALID_NAME_KEY {
            return None;
        }

        match table_index {
            TableIndex::Any => {
                // Search all tables
                for table in self.tables.values() {
                    for entry in table {
                        if entry.key == key {
                            return entry.func;
                        }
                    }
                }
                None
            }
            _ => self.key_to_func(key, table_index),
        }
    }

    /// Find a function by name
    pub fn find_function_by_name(
        &self,
        name: &str,
        table_index: TableIndex,
    ) -> Option<FunctionPtr> {
        let key = NameKeyGenerator::name_to_key(name);
        self.find_function(key, table_index)
    }

    /// Get a table by index
    pub fn get_table(&self, table_index: TableIndex) -> Option<&Vec<TableEntry>> {
        self.tables.get(&table_index)
    }

    /// Validate all tables for duplicate function addresses
    ///
    /// This helps catch cases where multiple function names resolve to the
    /// same address (common in optimized builds with empty functions).
    pub fn validate(&self) -> bool {
        let mut all_functions = HashMap::new();
        let mut valid = true;

        for (table_idx, table) in &self.tables {
            for entry in table {
                if let Some(func_ptr) = entry.func {
                    if let Some((existing_table, existing_name)) = all_functions.get(&func_ptr) {
                        eprintln!(
                            "WARNING! Function lexicon entries match same address! '{:?}:{}' and '{:?}:{}'",
                            existing_table, existing_name, table_idx, entry.name
                        );
                        valid = false;
                    } else {
                        all_functions.insert(func_ptr, (table_idx, &entry.name));
                    }
                }
            }
        }

        valid
    }

    /// Get the number of tables
    pub fn table_count(&self) -> usize {
        self.tables.len()
    }

    /// Get the total number of function entries across all tables
    pub fn function_count(&self) -> usize {
        self.tables.values().map(|table| table.len()).sum()
    }
}

impl SubsystemInterface for FunctionLexicon {
    fn name(&self) -> &str {
        "FunctionLexicon"
    }

    fn init(&mut self) -> SubsystemResult<()> {
        self.state = SubsystemState::Initializing;

        // C++ init reloads all static tables and recomputes NameKey values.
        // Rust keeps callback tables externally loaded, so rekey whatever is present.
        self.rekey_loaded_tables();

        // Validate the loaded tables
        if !self.validate() {
            // C++ only logs duplicate-address warnings and continues startup.
            eprintln!(
                "Function lexicon validation detected duplicate function addresses; continuing for C++ parity"
            );
        }

        self.state = SubsystemState::Running;
        Ok(())
    }

    fn update(&mut self, _delta_time: Duration) -> SubsystemResult<()> {
        // No ongoing updates needed for function lexicon
        Ok(())
    }

    fn shutdown(&mut self) -> SubsystemResult<()> {
        self.state = SubsystemState::ShuttingDown;
        self.tables.clear();
        self.state = SubsystemState::Shutdown;
        Ok(())
    }

    fn state(&self) -> SubsystemState {
        self.state
    }

    fn reset(&mut self) -> SubsystemResult<()> {
        // C++ reset() calls init() and keeps static callback tables alive.
        self.state = SubsystemState::Uninitialized;
        self.init()
    }

    fn as_any(&self) -> &(dyn Any + Send + Sync) {
        self
    }

    fn as_any_mut(&mut self) -> &mut (dyn Any + Send + Sync) {
        self
    }
}

impl Default for FunctionLexicon {
    fn default() -> Self {
        Self::new()
    }
}

// Global function lexicon instance (mirrors TheFunctionLexicon singleton)
lazy_static::lazy_static! {
    pub static ref THE_FUNCTION_LEXICON: Arc<Mutex<FunctionLexicon>> =
        Arc::new(Mutex::new(FunctionLexicon::new()));
}

/// Convenience function to access the global function lexicon
pub fn get_function_lexicon() -> Arc<Mutex<FunctionLexicon>> {
    THE_FUNCTION_LEXICON.clone()
}

/// Macro for creating function table entries
///
/// This helps create table entries with proper typing for function pointers.
#[macro_export]
macro_rules! function_table_entry {
    ($name:expr, $func:expr) => {
        TableEntry::new($name, Some(FunctionPtr::new($func as *const ())))
    };
    ($name:expr) => {
        TableEntry::new($name, None)
    };
}

/// Specific function type aliases for different callback types
///
/// These correspond to the C++ function pointer typedefs
pub type GameWinSystemFunc = fn();
pub type GameWinInputFunc = fn();
pub type GameWinTooltipFunc = fn();
pub type GameWinDrawFunc = fn();
pub type WindowLayoutInitFunc = fn();
pub type WindowLayoutUpdateFunc = fn();
pub type WindowLayoutShutdownFunc = fn();

/// Helper functions for type-safe function retrieval
impl FunctionLexicon {
    /// Get a game window system function
    pub fn game_win_system_func(&self, key: NameKeyType) -> Option<GameWinSystemFunc> {
        self.find_function(key, TableIndex::GameWinSystem)
            .map(|ptr| unsafe { std::mem::transmute(ptr.as_ptr()) })
    }

    /// Get a game window input function
    pub fn game_win_input_func(&self, key: NameKeyType) -> Option<GameWinInputFunc> {
        self.find_function(key, TableIndex::GameWinInput)
            .map(|ptr| unsafe { std::mem::transmute(ptr.as_ptr()) })
    }

    /// Get a game window draw function (searches both device-specific and general tables)
    pub fn game_win_draw_func(&self, key: NameKeyType) -> Option<GameWinDrawFunc> {
        // First try device-specific draw table
        if let Some(func) = self.find_function(key, TableIndex::GameWinDeviceDraw) {
            return Some(unsafe { std::mem::transmute(func.as_ptr()) });
        }

        // Fall back to general draw table
        self.find_function(key, TableIndex::GameWinDraw)
            .map(|ptr| unsafe { std::mem::transmute(ptr.as_ptr()) })
    }

    /// Get a window layout init function (searches both device-specific and general tables)
    pub fn win_layout_init_func(&self, key: NameKeyType) -> Option<WindowLayoutInitFunc> {
        // First try device-specific init table
        if let Some(func) = self.find_function(key, TableIndex::WinLayoutDeviceInit) {
            return Some(unsafe { std::mem::transmute(func.as_ptr()) });
        }

        // Fall back to general init table
        self.find_function(key, TableIndex::WinLayoutInit)
            .map(|ptr| unsafe { std::mem::transmute(ptr.as_ptr()) })
    }

    /// Get a window layout update function
    pub fn win_layout_update_func(&self, key: NameKeyType) -> Option<WindowLayoutUpdateFunc> {
        self.find_function(key, TableIndex::WinLayoutUpdate)
            .map(|ptr| unsafe { std::mem::transmute(ptr.as_ptr()) })
    }

    /// Get a window layout shutdown function
    pub fn win_layout_shutdown_func(&self, key: NameKeyType) -> Option<WindowLayoutShutdownFunc> {
        self.find_function(key, TableIndex::WinLayoutShutdown)
            .map(|ptr| unsafe { std::mem::transmute(ptr.as_ptr()) })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Mock functions for testing
    fn test_function_1() {}
    fn test_function_2() {}

    #[test]
    fn test_function_lexicon_creation() {
        NameKeyGenerator::reset();
        let lexicon = FunctionLexicon::new();
        assert_eq!(lexicon.table_count(), 0);
        assert_eq!(lexicon.function_count(), 0);
    }

    #[test]
    fn test_table_loading() {
        NameKeyGenerator::reset();
        let mut lexicon = FunctionLexicon::new();

        let table = vec![
            function_table_entry!("TestFunction1", test_function_1),
            function_table_entry!("TestFunction2", test_function_2),
        ];

        lexicon.load_table(table, TableIndex::GameWinSystem);

        assert_eq!(lexicon.table_count(), 1);
        assert_eq!(lexicon.function_count(), 2);
    }

    #[test]
    fn test_function_lookup() {
        NameKeyGenerator::reset();
        let mut lexicon = FunctionLexicon::new();

        let table = vec![
            function_table_entry!("TestFunction1", test_function_1),
            function_table_entry!("TestFunction2", test_function_2),
        ];

        lexicon.load_table(table, TableIndex::GameWinSystem);

        // Test lookup by name
        let func1 = lexicon.find_function_by_name("TestFunction1", TableIndex::GameWinSystem);
        assert!(func1.is_some());

        let func2 = lexicon.find_function_by_name("TestFunction2", TableIndex::GameWinSystem);
        assert!(func2.is_some());

        let nonexistent = lexicon.find_function_by_name("NonExistent", TableIndex::GameWinSystem);
        assert!(nonexistent.is_none());
    }

    #[test]
    fn test_table_entry_creation() {
        NameKeyGenerator::reset();
        let entry = function_table_entry!("TestFunction", test_function_1);
        assert_eq!(entry.name, "TestFunction");
        assert!(entry.func.is_some());

        let empty_entry = function_table_entry!("EmptyFunction");
        assert_eq!(empty_entry.name, "EmptyFunction");
        assert!(empty_entry.func.is_none());
    }

    #[test]
    fn test_validation() {
        NameKeyGenerator::reset();
        let mut lexicon = FunctionLexicon::new();

        // Test with unique functions
        let table1 = vec![
            function_table_entry!("TestFunction1", test_function_1),
            function_table_entry!("TestFunction2", test_function_2),
        ];

        lexicon.load_table(table1, TableIndex::GameWinSystem);
        assert!(lexicon.validate());

        // Test with duplicate functions (this would normally fail in real usage)
        let table2 = vec![
            function_table_entry!("DuplicateFunction", test_function_1), // Same function as in table1
        ];

        lexicon.load_table(table2, TableIndex::GameWinInput);
        // This should detect the duplicate
        assert!(!lexicon.validate());
    }

    #[test]
    fn test_reset_preserves_loaded_tables() {
        NameKeyGenerator::reset();
        let mut lexicon = FunctionLexicon::new();
        lexicon.load_table(
            vec![function_table_entry!("TestFunction1", test_function_1)],
            TableIndex::GameWinSystem,
        );
        assert_eq!(lexicon.function_count(), 1);

        let reset_result = <FunctionLexicon as SubsystemInterface>::reset(&mut lexicon);
        assert!(reset_result.is_ok());
        assert_eq!(lexicon.function_count(), 1);
        assert!(lexicon
            .find_function_by_name("TestFunction1", TableIndex::GameWinSystem)
            .is_some());
    }

    #[test]
    fn test_init_does_not_fail_on_duplicate_addresses() {
        NameKeyGenerator::reset();
        let mut lexicon = FunctionLexicon::new();
        lexicon.load_table(
            vec![function_table_entry!("A", test_function_1)],
            TableIndex::GameWinSystem,
        );
        lexicon.load_table(
            vec![function_table_entry!("B", test_function_1)],
            TableIndex::GameWinInput,
        );
        let result = <FunctionLexicon as SubsystemInterface>::init(&mut lexicon);
        assert!(result.is_ok());
        assert_eq!(lexicon.state(), SubsystemState::Running);
    }
}
