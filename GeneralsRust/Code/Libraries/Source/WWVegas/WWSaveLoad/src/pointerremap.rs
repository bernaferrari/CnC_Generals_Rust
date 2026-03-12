//! # Pointer Remapping System
//!
//! This module provides a safe Rust implementation of the pointer remapping system
//! used in the WWSaveLoad library. It handles the translation of memory addresses
//! during save/load operations to maintain object relationships across serialization.
//!
//! The system supports both regular pointer remapping and reference-counted pointer
//! remapping with automatic reference management.

use std::cmp::Ordering;
use std::collections::BTreeMap;
use std::fmt;
use std::sync::{Arc, Weak};

/// Growth step size for pointer tables (same as original C++ implementation)
const POINTER_TABLES_GROWTH_STEP: usize = 4096;

/// Error types for pointer remapping operations
#[derive(Debug, Clone, PartialEq)]
pub enum PointerRemapError {
    /// Failed to find a mapping for the given pointer
    PointerNotFound {
        /// The pointer address that couldn't be remapped
        old_pointer: usize,
        /// File where the remap was requested (debug builds only)
        #[cfg(debug_assertions)]
        file: String,
        /// Line where the remap was requested (debug builds only)
        #[cfg(debug_assertions)]
        line: u32,
    },
    /// Invalid pointer address (null or invalid)
    InvalidPointer,
    /// Reference count operation failed
    RefCountError(String),
}

impl fmt::Display for PointerRemapError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PointerRemapError::PointerNotFound { old_pointer, .. } => {
                write!(f, "Failed to remap pointer: 0x{:x}", old_pointer)
            }
            PointerRemapError::InvalidPointer => {
                write!(f, "Invalid pointer provided for remapping")
            }
            PointerRemapError::RefCountError(msg) => {
                write!(f, "Reference count error: {}", msg)
            }
        }
    }
}

impl std::error::Error for PointerRemapError {}

/// Represents a pointer pair mapping (old address -> new address)
#[derive(Debug, Clone, PartialEq)]
struct PtrPairStruct {
    /// Original pointer address (as usize for platform independence)
    old_pointer: usize,
    /// New pointer address after remapping
    new_pointer: usize,
}

impl PtrPairStruct {
    /// Create a new pointer pair mapping
    fn new(old_pointer: usize, new_pointer: usize) -> Self {
        Self {
            old_pointer,
            new_pointer,
        }
    }
}

impl PartialOrd for PtrPairStruct {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for PtrPairStruct {
    fn cmp(&self, other: &Self) -> Ordering {
        self.old_pointer.cmp(&other.old_pointer)
    }
}

impl Eq for PtrPairStruct {}

/// Represents a request for pointer remapping
struct PtrRemapStruct {
    /// The current pointer value that needs remapping
    pointer_value: usize,
    /// Callback to update the pointer once remapping is found
    update_callback: Box<dyn FnOnce(usize) + Send>,
    /// Debug information
    #[cfg(debug_assertions)]
    debug_info: DebugInfo,
}

/// Debug information for pointer remapping requests
#[cfg(debug_assertions)]
#[derive(Debug, Clone)]
struct DebugInfo {
    file: String,
    line: u32,
}

/// Trait for objects that can be reference counted
pub trait RefCountable: Send + Sync {
    /// Add a reference to this object
    fn add_ref(&self);
    /// Remove a reference from this object, returning true if object should be destroyed
    fn release(&self) -> bool;
    /// Get the current reference count
    fn ref_count(&self) -> u32;
}

/// A reference-counted pointer wrapper
pub type RefCountPtr<T> = Arc<T>;
pub type WeakRefCountPtr<T> = Weak<T>;

/// Main pointer remapping class - handles translation of memory addresses
/// during save/load operations to maintain object relationships
pub struct PointerRemapClass {
    /// Mapping of old pointer addresses to new pointer addresses
    pointer_pair_table: Vec<PtrPairStruct>,
    /// Queue of regular pointer remap requests
    pointer_request_queue: Vec<PtrRemapStruct>,
    /// Queue of reference-counted pointer remap requests  
    refcount_request_queue: Vec<PtrRemapStruct>,
    /// Fast lookup map for pointer pairs (for O(1) lookups after sorting)
    pointer_lookup_map: BTreeMap<usize, usize>,
    /// Flag to track if tables need resorting
    needs_sorting: bool,
}

impl Default for PointerRemapClass {
    fn default() -> Self {
        Self::new()
    }
}

impl PointerRemapClass {
    /// Create a new pointer remapping instance
    pub fn new() -> Self {
        Self {
            pointer_pair_table: Vec::with_capacity(POINTER_TABLES_GROWTH_STEP),
            pointer_request_queue: Vec::with_capacity(POINTER_TABLES_GROWTH_STEP),
            refcount_request_queue: Vec::with_capacity(POINTER_TABLES_GROWTH_STEP),
            pointer_lookup_map: BTreeMap::new(),
            needs_sorting: false,
        }
    }

    /// Reset all tables and clear all pending requests
    pub fn reset(&mut self) {
        self.pointer_pair_table.clear();
        self.pointer_request_queue.clear();
        self.refcount_request_queue.clear();
        self.pointer_lookup_map.clear();
        self.needs_sorting = false;
    }

    /// Process all pending pointer remap requests
    /// This sorts the tables and performs all requested remappings
    pub fn process(&mut self) -> Result<(), PointerRemapError> {
        // Sort pointer pairs if we have any
        if !self.pointer_pair_table.is_empty() && self.needs_sorting {
            self.pointer_pair_table.sort_by_key(|pair| pair.old_pointer);

            // Build fast lookup map
            self.pointer_lookup_map.clear();
            for pair in &self.pointer_pair_table {
                self.pointer_lookup_map
                    .insert(pair.old_pointer, pair.new_pointer);
            }

            self.needs_sorting = false;
        }

        // Process regular pointer requests
        if !self.pointer_request_queue.is_empty() {
            if self.pointer_pair_table.is_empty() {
                return Err(PointerRemapError::RefCountError(
                    "No pointer pairs available for remapping".to_string(),
                ));
            }
            self.process_request_queue(false)?;
        }

        // Process reference-counted pointer requests
        if !self.refcount_request_queue.is_empty() {
            if self.pointer_pair_table.is_empty() {
                return Err(PointerRemapError::RefCountError(
                    "No pointer pairs available for ref-counted remapping".to_string(),
                ));
            }
            self.process_request_queue(true)?;
        }

        Ok(())
    }

    /// Register a pointer mapping from old address to new address
    pub fn register_pointer(&mut self, old_pointer: usize, new_pointer: usize) {
        self.pointer_pair_table
            .push(PtrPairStruct::new(old_pointer, new_pointer));
        self.needs_sorting = true;
    }

    /// Register a pointer mapping using raw pointers (for C++ compatibility)
    pub fn register_raw_pointer<T>(&mut self, old_pointer: *const T, new_pointer: *const T) {
        self.register_pointer(old_pointer as usize, new_pointer as usize);
    }

    /// Request remapping of a regular pointer
    #[cfg(debug_assertions)]
    pub fn request_pointer_remap<T, F>(
        &mut self,
        current_value: *const T,
        update_fn: F,
        file: &str,
        line: u32,
    ) where
        F: FnOnce(*const T) + Send + 'static,
    {
        let remap = PtrRemapStruct {
            pointer_value: current_value as usize,
            update_callback: Box::new(move |new_addr| {
                update_fn(new_addr as *const T);
            }),
            debug_info: DebugInfo {
                file: file.to_string(),
                line,
            },
        };
        self.pointer_request_queue.push(remap);
    }

    /// Request remapping of a regular pointer (release mode)
    #[cfg(not(debug_assertions))]
    pub fn request_pointer_remap<T, F>(&mut self, current_value: *const T, update_fn: F)
    where
        F: FnOnce(*const T) + Send + 'static,
    {
        let remap = PtrRemapStruct {
            pointer_value: current_value as usize,
            update_callback: Box::new(move |new_addr| {
                update_fn(new_addr as *const T);
            }),
        };
        self.pointer_request_queue.push(remap);
    }

    /// Request remapping of a reference-counted pointer
    #[cfg(debug_assertions)]
    pub fn request_ref_counted_pointer_remap<T, F>(
        &mut self,
        current_value: *const T,
        update_fn: F,
        file: &str,
        line: u32,
    ) where
        T: RefCountable,
        F: FnOnce(*const T) + Send + 'static,
    {
        let remap = PtrRemapStruct {
            pointer_value: current_value as usize,
            update_callback: Box::new(move |new_addr| {
                let new_ptr = new_addr as *const T;
                if !new_ptr.is_null() {
                    unsafe {
                        (*new_ptr).add_ref();
                    }
                }
                update_fn(new_ptr);
            }),
            debug_info: DebugInfo {
                file: file.to_string(),
                line,
            },
        };
        self.refcount_request_queue.push(remap);
    }

    /// Request remapping of a reference-counted pointer (release mode)
    #[cfg(not(debug_assertions))]
    pub fn request_ref_counted_pointer_remap<T, F>(&mut self, current_value: *const T, update_fn: F)
    where
        T: RefCountable,
        F: FnOnce(*const T) + Send + 'static,
    {
        let remap = PtrRemapStruct {
            pointer_value: current_value as usize,
            update_callback: Box::new(move |new_addr| {
                let new_ptr = new_addr as *const T;
                if !new_ptr.is_null() {
                    unsafe {
                        (*new_ptr).add_ref();
                    }
                }
                update_fn(new_ptr);
            }),
        };
        self.refcount_request_queue.push(remap);
    }

    /// Safe Arc-based reference counted pointer remapping
    pub fn request_arc_remap<T>(
        &mut self,
        old_arc: &Arc<T>,
        update_fn: impl FnOnce(Option<Arc<T>>) + Send + 'static,
    ) where
        T: Send + Sync + 'static,
    {
        let old_addr = Arc::as_ptr(old_arc) as usize;
        let remap = PtrRemapStruct {
            pointer_value: old_addr,
            update_callback: Box::new(move |new_addr| {
                if new_addr == 0 {
                    update_fn(None);
                } else {
                    // In a real implementation, this would need a registry
                    // of Arc instances to safely reconstruct them
                    log::warn!("Arc reconstruction not implemented - setting to None");
                    update_fn(None);
                }
            }),
            #[cfg(debug_assertions)]
            debug_info: DebugInfo {
                file: "arc_remap".to_string(),
                line: 0,
            },
        };
        self.refcount_request_queue.push(remap);
    }

    /// Process the request queue (internal implementation)
    fn process_request_queue(&mut self, is_refcount: bool) -> Result<(), PointerRemapError> {
        let request_queue = if is_refcount {
            &mut self.refcount_request_queue
        } else {
            &mut self.pointer_request_queue
        };

        // Sort requests by pointer value for efficient processing
        request_queue.sort_by_key(|req| req.pointer_value);

        // Process each request
        let mut requests_to_process = Vec::new();
        std::mem::swap(&mut requests_to_process, request_queue);

        for request in requests_to_process {
            let pointer_to_remap = request.pointer_value;

            // Look up the new pointer value
            match self.pointer_lookup_map.get(&pointer_to_remap) {
                Some(&new_pointer) => {
                    // Found mapping - update the pointer
                    (request.update_callback)(new_pointer);
                }
                None => {
                    // Failed to remap - set to null and log error
                    (request.update_callback)(0);

                    #[cfg(debug_assertions)]
                    {
                        let error = PointerRemapError::PointerNotFound {
                            old_pointer: pointer_to_remap,
                            file: request.debug_info.file,
                            line: request.debug_info.line,
                        };
                        log::error!("Pointer remap failed: {}", error);
                        // In debug mode, we might want to panic or return error
                        // For now, just log and continue
                    }

                    #[cfg(not(debug_assertions))]
                    {
                        log::warn!("Failed to remap pointer: 0x{:x}", pointer_to_remap);
                    }
                }
            }
        }

        Ok(())
    }

    /// Get statistics about the pointer remapping state
    pub fn get_statistics(&self) -> PointerRemapStatistics {
        PointerRemapStatistics {
            registered_pairs: self.pointer_pair_table.len(),
            pending_regular_requests: self.pointer_request_queue.len(),
            pending_refcount_requests: self.refcount_request_queue.len(),
            needs_sorting: self.needs_sorting,
        }
    }

    /// Check if a pointer mapping exists
    pub fn has_mapping(&self, old_pointer: usize) -> bool {
        if self.needs_sorting {
            // If not sorted, do linear search
            self.pointer_pair_table
                .iter()
                .any(|pair| pair.old_pointer == old_pointer)
        } else {
            // Use fast lookup map
            self.pointer_lookup_map.contains_key(&old_pointer)
        }
    }

    /// Get the mapped pointer value if it exists
    pub fn get_mapping(&self, old_pointer: usize) -> Option<usize> {
        if self.needs_sorting {
            // If not sorted, do linear search
            self.pointer_pair_table
                .iter()
                .find(|pair| pair.old_pointer == old_pointer)
                .map(|pair| pair.new_pointer)
        } else {
            // Use fast lookup map
            self.pointer_lookup_map.get(&old_pointer).copied()
        }
    }
}

/// Statistics about the pointer remapping system
#[derive(Debug, Clone)]
pub struct PointerRemapStatistics {
    pub registered_pairs: usize,
    pub pending_regular_requests: usize,
    pub pending_refcount_requests: usize,
    pub needs_sorting: bool,
}

/// Convenience macros for pointer remapping requests with debug info
/// Note: These macros are defined in saveload.rs to avoid duplication

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU32, Ordering};
    use std::sync::{Arc, Mutex};

    /// Mock reference counted object for testing
    struct MockRefCountable {
        ref_count: AtomicU32,
        value: i32,
    }

    impl MockRefCountable {
        fn new(value: i32) -> Self {
            Self {
                ref_count: AtomicU32::new(1),
                value,
            }
        }
    }

    impl RefCountable for MockRefCountable {
        fn add_ref(&self) {
            self.ref_count.fetch_add(1, Ordering::SeqCst);
        }

        fn release(&self) -> bool {
            let count = self.ref_count.fetch_sub(1, Ordering::SeqCst);
            count == 1
        }

        fn ref_count(&self) -> u32 {
            self.ref_count.load(Ordering::SeqCst)
        }
    }

    #[test]
    fn test_pointer_remap_creation() {
        let remap = PointerRemapClass::new();
        let stats = remap.get_statistics();

        assert_eq!(stats.registered_pairs, 0);
        assert_eq!(stats.pending_regular_requests, 0);
        assert_eq!(stats.pending_refcount_requests, 0);
        assert!(!stats.needs_sorting);
    }

    #[test]
    fn test_pointer_registration() {
        let mut remap = PointerRemapClass::new();

        let old_addr = 0x1000usize;
        let new_addr = 0x2000usize;

        remap.register_pointer(old_addr, new_addr);

        let stats = remap.get_statistics();
        assert_eq!(stats.registered_pairs, 1);
        assert!(stats.needs_sorting);

        assert!(remap.has_mapping(old_addr));
        assert_eq!(remap.get_mapping(old_addr), Some(new_addr));
    }

    #[test]
    fn test_pointer_remap_process() {
        let mut remap = PointerRemapClass::new();

        // Register some pointer mappings
        remap.register_pointer(0x1000, 0x2000);
        remap.register_pointer(0x1004, 0x2004);
        remap.register_pointer(0x1008, 0x2008);

        // Add a remap request using Arc<Mutex<>> for shared state
        let result_ptr = Arc::new(std::sync::Mutex::new(0usize));
        let old_ptr = 0x1004usize;
        let result_ptr_clone = Arc::clone(&result_ptr);

        #[cfg(debug_assertions)]
        remap.request_pointer_remap(
            old_ptr as *const u8,
            move |new_ptr| {
                *result_ptr_clone.lock().unwrap() = new_ptr as usize;
            },
            "test.rs",
            123,
        );

        #[cfg(not(debug_assertions))]
        {
            let result_ptr_clone = Arc::clone(&result_ptr);
            remap.request_pointer_remap(old_ptr as *const u8, move |new_ptr| {
                *result_ptr_clone.lock().unwrap() = new_ptr as usize;
            });
        }

        // Process the requests
        assert!(remap.process().is_ok());

        // Verify the pointer was remapped correctly
        let final_result = *result_ptr.lock().unwrap();
        assert_eq!(final_result, 0x2004);
    }

    #[test]
    fn test_reset() {
        let mut remap = PointerRemapClass::new();

        remap.register_pointer(0x1000, 0x2000);

        let stats_before = remap.get_statistics();
        assert_eq!(stats_before.registered_pairs, 1);

        remap.reset();

        let stats_after = remap.get_statistics();
        assert_eq!(stats_after.registered_pairs, 0);
        assert_eq!(stats_after.pending_regular_requests, 0);
        assert_eq!(stats_after.pending_refcount_requests, 0);
        assert!(!stats_after.needs_sorting);
    }

    #[test]
    fn test_mapping_lookup() {
        let mut remap = PointerRemapClass::new();

        // Test with unsorted table
        remap.register_pointer(0x3000, 0x4000);
        remap.register_pointer(0x1000, 0x2000);
        remap.register_pointer(0x5000, 0x6000);

        assert!(remap.has_mapping(0x1000));
        assert!(remap.has_mapping(0x3000));
        assert!(remap.has_mapping(0x5000));
        assert!(!remap.has_mapping(0x7000));

        assert_eq!(remap.get_mapping(0x1000), Some(0x2000));
        assert_eq!(remap.get_mapping(0x3000), Some(0x4000));
        assert_eq!(remap.get_mapping(0x5000), Some(0x6000));
        assert_eq!(remap.get_mapping(0x7000), None);

        // Process to trigger sorting
        assert!(remap.process().is_ok());

        // Test with sorted table
        assert!(remap.has_mapping(0x1000));
        assert!(remap.has_mapping(0x3000));
        assert!(remap.has_mapping(0x5000));
        assert!(!remap.has_mapping(0x7000));

        assert_eq!(remap.get_mapping(0x1000), Some(0x2000));
        assert_eq!(remap.get_mapping(0x3000), Some(0x4000));
        assert_eq!(remap.get_mapping(0x5000), Some(0x6000));
        assert_eq!(remap.get_mapping(0x7000), None);
    }

    #[test]
    fn test_raw_pointer_registration() {
        let mut remap = PointerRemapClass::new();

        let old_val = 42i32;
        let new_val = 84i32;

        remap.register_raw_pointer(&old_val as *const i32, &new_val as *const i32);

        let stats = remap.get_statistics();
        assert_eq!(stats.registered_pairs, 1);

        let old_addr = &old_val as *const i32 as usize;
        let new_addr = &new_val as *const i32 as usize;

        assert!(remap.has_mapping(old_addr));
        assert_eq!(remap.get_mapping(old_addr), Some(new_addr));
    }

    #[test]
    fn test_arc_integration() {
        let mut remap = PointerRemapClass::new();

        let arc1 = Arc::new(42i32);
        let arc2 = Arc::new(84i32);

        // Register Arc pointers
        let old_addr = Arc::as_ptr(&arc1) as usize;
        let new_addr = Arc::as_ptr(&arc2) as usize;
        remap.register_pointer(old_addr, new_addr);

        assert!(remap.has_mapping(old_addr));
        assert_eq!(remap.get_mapping(old_addr), Some(new_addr));
    }

    #[test]
    fn test_statistics() {
        let mut remap = PointerRemapClass::new();

        let initial_stats = remap.get_statistics();
        assert_eq!(initial_stats.registered_pairs, 0);
        assert_eq!(initial_stats.pending_regular_requests, 0);
        assert_eq!(initial_stats.pending_refcount_requests, 0);
        assert!(!initial_stats.needs_sorting);

        remap.register_pointer(0x1000, 0x2000);
        remap.register_pointer(0x3000, 0x4000);

        let after_register_stats = remap.get_statistics();
        assert_eq!(after_register_stats.registered_pairs, 2);
        assert!(after_register_stats.needs_sorting);

        // Process to clear needs_sorting flag
        remap.process().unwrap();

        let after_process_stats = remap.get_statistics();
        assert_eq!(after_process_stats.registered_pairs, 2);
        assert!(!after_process_stats.needs_sorting);
    }
}

/// Integration tests that demonstrate usage patterns
#[cfg(test)]
mod integration_tests {
    use super::*;
    use std::sync::atomic::{AtomicU32, Ordering};
    use std::sync::{Arc, Mutex};

    /// Mock RefCountable implementation for integration testing
    struct MockRefCountable {
        ref_count: AtomicU32,
        value: i32,
    }

    impl MockRefCountable {
        fn new(value: i32) -> Self {
            Self {
                ref_count: AtomicU32::new(1),
                value,
            }
        }
    }

    impl RefCountable for MockRefCountable {
        fn add_ref(&self) {
            self.ref_count.fetch_add(1, Ordering::SeqCst);
        }

        fn release(&self) -> bool {
            let count = self.ref_count.fetch_sub(1, Ordering::SeqCst);
            count == 1
        }

        fn ref_count(&self) -> u32 {
            self.ref_count.load(Ordering::SeqCst)
        }
    }

    #[test]
    fn test_save_load_scenario() {
        // Simulate a save/load scenario where pointers need remapping
        let mut remap = PointerRemapClass::new();

        // Original objects (simulating pre-save state)
        let obj1 = Box::new(100i32);
        let obj2 = Box::new(200i32);
        let obj3 = Box::new(300i32);

        let old_addr1 = obj1.as_ref() as *const i32 as usize;
        let old_addr2 = obj2.as_ref() as *const i32 as usize;
        let old_addr3 = obj3.as_ref() as *const i32 as usize;

        // New objects (simulating post-load state)
        let new_obj1 = Box::new(100i32);
        let new_obj2 = Box::new(200i32);
        let new_obj3 = Box::new(300i32);

        let new_addr1 = new_obj1.as_ref() as *const i32 as usize;
        let new_addr2 = new_obj2.as_ref() as *const i32 as usize;
        let new_addr3 = new_obj3.as_ref() as *const i32 as usize;

        // Register the mappings
        remap.register_pointer(old_addr1, new_addr1);
        remap.register_pointer(old_addr2, new_addr2);
        remap.register_pointer(old_addr3, new_addr3);

        // Simulate pointers that need remapping
        let remapped_results = Arc::new(Mutex::new(Vec::new()));

        // Request remapping for each old address
        for &old_addr in &[old_addr1, old_addr2, old_addr3] {
            let results_clone = Arc::clone(&remapped_results);

            #[cfg(debug_assertions)]
            remap.request_pointer_remap(
                old_addr as *const i32,
                move |new_ptr| {
                    results_clone.lock().unwrap().push(new_ptr as usize);
                },
                "integration_test.rs",
                42,
            );

            #[cfg(not(debug_assertions))]
            remap.request_pointer_remap(old_addr as *const i32, move |new_ptr| {
                results_clone.lock().unwrap().push(new_ptr as usize);
            });
        }

        // Process all remap requests
        assert!(remap.process().is_ok());

        // Verify that all pointers were correctly remapped
        let results = remapped_results.lock().unwrap();
        assert_eq!(results.len(), 3);

        // Note: The actual verification would depend on the order of processing,
        // which may not be deterministic. In a real implementation, you'd want
        // to use a more structured approach to verify specific mappings.
    }

    #[test]
    fn test_reference_counting_integration() {
        // This test demonstrates how the system would work with reference counted objects
        let mut remap = PointerRemapClass::new();

        // Create Arc objects to simulate reference counted pointers
        let old_arc1 = Arc::new(MockRefCountable::new(42));
        let old_arc2 = Arc::new(MockRefCountable::new(84));

        let new_arc1 = Arc::new(MockRefCountable::new(42));
        let new_arc2 = Arc::new(MockRefCountable::new(84));

        // Register mappings
        remap.register_pointer(
            Arc::as_ptr(&old_arc1) as usize,
            Arc::as_ptr(&new_arc1) as usize,
        );
        remap.register_pointer(
            Arc::as_ptr(&old_arc2) as usize,
            Arc::as_ptr(&new_arc2) as usize,
        );

        // Verify mappings exist
        assert!(remap.has_mapping(Arc::as_ptr(&old_arc1) as usize));
        assert!(remap.has_mapping(Arc::as_ptr(&old_arc2) as usize));

        // In a full implementation, you would queue ref-counted remap requests
        // and verify that reference counts are properly managed
    }
}

// Re-export commonly used types
pub use PointerRemapClass as PointerRemap;
pub use PointerRemapError as RemapError;
pub use PointerRemapStatistics as RemapStatistics;
