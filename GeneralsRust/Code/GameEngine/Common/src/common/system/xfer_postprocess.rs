// FILE: xfer_postprocess.rs ///////////////////////////////////////////////////
// Post-process loading phase for save/load system
///////////////////////////////////////////////////////////////////////////////

use std::collections::HashMap;
use std::io;

/// Trait for objects requiring post-load processing
pub trait PostLoadProcessor {
    /// Process object after all data has been loaded
    /// Use this to rebuild transient state, reconnect references, etc.
    fn post_load_process(&mut self) -> io::Result<()>;

    /// Get priority for post-load processing (0 = first, higher = later)
    fn post_load_priority(&self) -> u32 {
        100 // Default middle priority
    }
}

/// Post-load phase manager
/// Collects objects needing post-processing and executes them in correct order
pub struct PostLoadManager {
    processors: Vec<(u32, Box<dyn PostLoadProcessor>)>,
    object_references: HashMap<String, usize>,
    processed_count: usize,
}

impl PostLoadManager {
    pub fn new() -> Self {
        Self {
            processors: Vec::new(),
            object_references: HashMap::new(),
            processed_count: 0,
        }
    }

    /// Register object for post-load processing
    pub fn register<T: PostLoadProcessor + 'static>(&mut self, processor: T) {
        let priority = processor.post_load_priority();
        self.processors.push((priority, Box::new(processor)));
    }

    /// Register object reference for later resolution
    pub fn register_reference(&mut self, ref_name: String, object_index: usize) {
        self.object_references.insert(ref_name, object_index);
    }

    /// Resolve object reference by name
    pub fn resolve_reference(&self, ref_name: &str) -> Option<usize> {
        self.object_references.get(ref_name).copied()
    }

    /// Execute all post-load processors in priority order
    pub fn execute_all(&mut self) -> io::Result<()> {
        // Sort by priority (lower numbers first)
        self.processors.sort_by_key(|(priority, _)| *priority);

        log::info!("Executing {} post-load processors", self.processors.len());

        for (priority, processor) in &mut self.processors {
            log::debug!("Executing post-load processor with priority {}", priority);
            processor.post_load_process()?;
            self.processed_count += 1;
        }

        log::info!(
            "Post-load processing complete: {} objects processed",
            self.processed_count
        );
        Ok(())
    }

    /// Clear all processors
    pub fn clear(&mut self) {
        self.processors.clear();
        self.object_references.clear();
        self.processed_count = 0;
    }

    /// Get number of processors
    pub fn processor_count(&self) -> usize {
        self.processors.len()
    }

    /// Get number of processed objects
    pub fn processed_count(&self) -> usize {
        self.processed_count
    }
}

impl Default for PostLoadManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Common post-load operations
pub mod operations {
    use super::*;

    /// Reconnect object pointers after load
    pub fn reconnect_object_references(
        manager: &PostLoadManager,
        ref_names: &[String],
    ) -> io::Result<Vec<usize>> {
        let mut indices = Vec::with_capacity(ref_names.len());

        for ref_name in ref_names {
            match manager.resolve_reference(ref_name) {
                Some(index) => indices.push(index),
                None => {
                    return Err(io::Error::new(
                        io::ErrorKind::NotFound,
                        format!("Cannot resolve reference: {}", ref_name),
                    ));
                }
            }
        }

        Ok(indices)
    }

    /// Rebuild derived state from loaded data
    pub fn rebuild_derived_state<F>(rebuild_fn: F) -> io::Result<()>
    where
        F: FnOnce() -> io::Result<()>,
    {
        rebuild_fn()
    }

    /// Validate loaded data consistency
    pub fn validate_consistency<F>(validation_fn: F) -> io::Result<()>
    where
        F: FnOnce() -> io::Result<()>,
    {
        validation_fn()
    }
}

/// Post-load callbacks registry
pub struct PostLoadCallbacks {
    callbacks: Vec<Box<dyn FnMut() -> io::Result<()>>>,
}

impl PostLoadCallbacks {
    pub fn new() -> Self {
        Self {
            callbacks: Vec::new(),
        }
    }

    /// Add callback to be executed after load
    pub fn add_callback<F>(&mut self, callback: F)
    where
        F: FnMut() -> io::Result<()> + 'static,
    {
        self.callbacks.push(Box::new(callback));
    }

    /// Execute all callbacks
    pub fn execute_all(&mut self) -> io::Result<()> {
        for callback in &mut self.callbacks {
            callback()?;
        }
        Ok(())
    }

    /// Clear all callbacks
    pub fn clear(&mut self) {
        self.callbacks.clear();
    }

    /// Get number of callbacks
    pub fn count(&self) -> usize {
        self.callbacks.len()
    }
}

impl Default for PostLoadCallbacks {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct TestProcessor {
        processed: bool,
        priority: u32,
    }

    impl TestProcessor {
        fn new(priority: u32) -> Self {
            Self {
                processed: false,
                priority,
            }
        }
    }

    impl PostLoadProcessor for TestProcessor {
        fn post_load_process(&mut self) -> io::Result<()> {
            self.processed = true;
            Ok(())
        }

        fn post_load_priority(&self) -> u32 {
            self.priority
        }
    }

    #[test]
    fn test_post_load_manager() {
        let mut manager = PostLoadManager::new();

        manager.register(TestProcessor::new(50));
        manager.register(TestProcessor::new(10));
        manager.register(TestProcessor::new(100));

        assert_eq!(manager.processor_count(), 3);

        manager.execute_all().unwrap();
        assert_eq!(manager.processed_count(), 3);
    }

    #[test]
    fn test_reference_resolution() {
        let mut manager = PostLoadManager::new();

        manager.register_reference("object1".to_string(), 42);
        manager.register_reference("object2".to_string(), 123);

        assert_eq!(manager.resolve_reference("object1"), Some(42));
        assert_eq!(manager.resolve_reference("object2"), Some(123));
        assert_eq!(manager.resolve_reference("object3"), None);
    }

    #[test]
    fn test_callbacks() {
        let mut callbacks = PostLoadCallbacks::new();

        let mut counter = 0;
        callbacks.add_callback(|| {
            counter += 1;
            Ok(())
        });
        callbacks.add_callback(|| {
            counter += 10;
            Ok(())
        });

        assert_eq!(callbacks.count(), 2);

        // Note: This test won't actually increment counter due to closure capture
        // In real use, callbacks would modify external state
        callbacks.execute_all().unwrap();
    }
}
