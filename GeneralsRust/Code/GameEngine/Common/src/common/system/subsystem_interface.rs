////////////////////////////////////////////////////////////////////////////////
//                                                                            //
//  (c) 2001-2003 Electronic Arts Inc.                                       //
//                                                                            //
////////////////////////////////////////////////////////////////////////////////

// FILE: subsystem_interface.rs ///////////////////////////////////////////////
// Base interface for game subsystems
///////////////////////////////////////////////////////////////////////////////

use std::any::Any;
use std::collections::HashMap;
use std::time::{Duration, Instant};

/// Subsystem initialization result
pub type SubsystemResult<T> = Result<T, SubsystemError>;

/// Subsystem error types
#[derive(Debug, thiserror::Error)]
pub enum SubsystemError {
    #[error("Initialization failed: {0}")]
    InitializationFailed(String),
    #[error("Subsystem not initialized")]
    NotInitialized,
    #[error("Invalid configuration: {0}")]
    InvalidConfiguration(String),
    #[error("Resource error: {0}")]
    ResourceError(String),
    #[error("Operation failed: {0}")]
    OperationFailed(String),
    #[error("Update failed: {0}")]
    UpdateFailed(String),
    #[error("Shutdown failed: {0}")]
    ShutdownFailed(String),
}

/// Subsystem state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SubsystemState {
    Uninitialized,
    Initializing,
    Running,
    Paused,
    ShuttingDown,
    Shutdown,
    Error,
}

/// Base trait for all subsystems
pub trait SubsystemInterface: Send + Sync {
    /// Get the subsystem name
    fn name(&self) -> &str;

    /// Initialize the subsystem
    fn init(&mut self) -> SubsystemResult<()>;

    /// Update the subsystem (called each frame)
    fn update(&mut self, delta_time: Duration) -> SubsystemResult<()>;

    /// Shutdown the subsystem
    fn shutdown(&mut self) -> SubsystemResult<()>;

    /// Get current state
    fn state(&self) -> SubsystemState;

    /// Reset the subsystem to initial state
    fn reset(&mut self) -> SubsystemResult<()> {
        Ok(())
    }

    /// Post-process loading operations
    fn post_process_load(&mut self) -> SubsystemResult<()> {
        Ok(())
    }

    /// Pause the subsystem
    fn pause(&mut self) -> SubsystemResult<()> {
        Ok(())
    }

    /// Resume the subsystem
    fn resume(&mut self) -> SubsystemResult<()> {
        Ok(())
    }

    /// Get subsystem-specific data (for communication between subsystems)
    fn get_data(&self, _key: &str) -> Option<&(dyn Any + Send + Sync)> {
        None
    }

    /// Set subsystem-specific data
    fn set_data(
        &mut self,
        _key: String,
        _value: Box<dyn Any + Send + Sync>,
    ) -> SubsystemResult<()> {
        Ok(())
    }

    /// Accessor used for downcasting to concrete subsystem types.
    fn as_any(&self) -> &(dyn Any + Send + Sync);

    /// Mutable accessor used for downcasting to concrete subsystem types.
    fn as_any_mut(&mut self) -> &mut (dyn Any + Send + Sync);
}

/// Descriptor used when registering subsystems with the manager.
pub struct SubsystemDescriptor {
    subsystem: Box<dyn SubsystemInterface>,
    depends_on: Vec<String>,
}

impl SubsystemDescriptor {
    pub fn new(subsystem: Box<dyn SubsystemInterface>) -> Self {
        Self {
            subsystem,
            depends_on: Vec::new(),
        }
    }

    pub fn depends_on<I, S>(mut self, deps: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.depends_on = deps.into_iter().map(Into::into).collect();
        self
    }

    fn into_parts(self) -> (Box<dyn SubsystemInterface>, Vec<String>) {
        (self.subsystem, self.depends_on)
    }
}

struct SubsystemEntry {
    name: String,
    subsystem: Box<dyn SubsystemInterface>,
    depends_on: Vec<String>,
}

/// Subsystem manager
pub struct SubsystemManager {
    entries: Vec<SubsystemEntry>,
    index_map: HashMap<String, usize>,
    initialization_order: Vec<String>,
    is_running: bool,
    last_update: Instant,
}

impl SubsystemManager {
    /// Create a new subsystem manager
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
            index_map: HashMap::new(),
            initialization_order: Vec::new(),
            is_running: false,
            last_update: Instant::now(),
        }
    }

    /// Register a subsystem
    pub fn register_subsystem(&mut self, descriptor: SubsystemDescriptor) -> SubsystemResult<()> {
        let (subsystem, depends_on) = descriptor.into_parts();
        let name = subsystem.name().to_string();

        if self.index_map.contains_key(&name) {
            return Err(SubsystemError::InvalidConfiguration(format!(
                "Subsystem '{}' is already registered",
                name
            )));
        }

        let index = self.entries.len();
        self.index_map.insert(name.clone(), index);
        self.entries.push(SubsystemEntry {
            name: name.clone(),
            subsystem,
            depends_on,
        });
        self.initialization_order.clear();

        Ok(())
    }

    fn compute_initialization_order(&self) -> SubsystemResult<Vec<String>> {
        let mut indegree: HashMap<String, usize> = HashMap::new();
        let mut graph: HashMap<String, Vec<String>> = HashMap::new();

        for entry in &self.entries {
            indegree.entry(entry.name.clone()).or_insert(0);
        }

        for entry in &self.entries {
            for dep in &entry.depends_on {
                if !self.index_map.contains_key(dep) {
                    return Err(SubsystemError::InvalidConfiguration(format!(
                        "Subsystem '{}' depends on unknown subsystem '{}'",
                        entry.name, dep
                    )));
                }
                if dep == &entry.name {
                    return Err(SubsystemError::InvalidConfiguration(format!(
                        "Subsystem '{}' cannot depend on itself",
                        entry.name
                    )));
                }
                *indegree
                    .get_mut(&entry.name)
                    .expect("indegree must contain entry") += 1;
                graph
                    .entry(dep.clone())
                    .or_default()
                    .push(entry.name.clone());
            }
        }

        let mut ready: Vec<String> = indegree
            .iter()
            .filter(|(_, &deg)| deg == 0)
            .map(|(name, _)| name.clone())
            .collect();
        ready.sort();

        let mut queue: std::collections::VecDeque<String> = ready.into();
        let mut order = Vec::new();
        let mut indegree_mut = indegree;

        while let Some(name) = queue.pop_front() {
            order.push(name.clone());
            if let Some(children) = graph.get(&name) {
                let mut newly_ready = Vec::new();
                for child in children {
                    if let Some(deg) = indegree_mut.get_mut(child) {
                        *deg = deg.saturating_sub(1);
                        if *deg == 0 {
                            newly_ready.push(child.clone());
                        }
                    }
                }
                newly_ready.sort();
                for child in newly_ready {
                    queue.push_back(child);
                }
            }
        }

        if order.len() != self.entries.len() {
            return Err(SubsystemError::InvalidConfiguration(
                "Subsystem dependency graph contains a cycle".to_string(),
            ));
        }

        Ok(order)
    }

    /// Initialize all subsystems
    pub fn init_all(&mut self) -> SubsystemResult<()> {
        let order = self.compute_initialization_order()?;

        for name in &order {
            let index = self
                .index_map
                .get(name)
                .expect("initialization order references unknown subsystem");
            let subsystem = &mut self.entries[*index].subsystem;
            subsystem.init().map_err(|e| {
                SubsystemError::InitializationFailed(format!(
                    "Failed to initialize '{}': {}",
                    name, e
                ))
            })?;
        }

        self.initialization_order = order;
        self.is_running = true;
        self.last_update = Instant::now();

        Ok(())
    }

    /// Async-friendly wrapper around [`init_all`].
    pub async fn init_all_async(&mut self) -> SubsystemResult<()> {
        self.init_all()?;
        // C++ parity: After all subsystems initialize, call postProcessLoadAll()
        // to allow subsystems to perform second-pass initialization (resolve
        // cross-references, prime shared resources, etc.).
        self.post_process_load_all()?;
        Ok(())
    }

    /// Call post_process_load on all registered subsystems in order.
    /// This matches C++'s TheSubsystemList->postProcessLoadAll().
    pub fn post_process_load_all(&mut self) -> SubsystemResult<()> {
        for name in &self.initialization_order {
            let index = self
                .index_map
                .get(name)
                .expect("initialization order references unknown subsystem");
            let subsystem = &mut self.entries[*index].subsystem;
            subsystem
                .post_process_load()
                .map_err(|e| SubsystemError::OperationFailed(format!("postProcessLoad failed for '{}': {}", name, e)))?;
        }
        Ok(())
    }

    /// Update all subsystems
    pub fn update_all(&mut self) -> SubsystemResult<()> {
        if !self.is_running {
            return Err(SubsystemError::NotInitialized);
        }

        let now = Instant::now();
        let delta_time = now.duration_since(self.last_update);
        self.last_update = now;

        for name in &self.initialization_order {
            let index = self
                .index_map
                .get(name)
                .expect("initialization order references unknown subsystem");
            let subsystem = &mut self.entries[*index].subsystem;
            if subsystem.state() == SubsystemState::Running {
                subsystem.update(delta_time)?;
            }
        }

        Ok(())
    }

    /// Async-friendly wrapper around [`update_all`].
    pub async fn update_all_async(&mut self) -> SubsystemResult<()> {
        self.update_all()
    }

    /// Reset all subsystems in initialization order.
    pub fn reset_all(&mut self) -> SubsystemResult<()> {
        if !self.is_running {
            return Err(SubsystemError::NotInitialized);
        }

        for name in &self.initialization_order {
            let index = self
                .index_map
                .get(name)
                .expect("initialization order references unknown subsystem");
            let subsystem = &mut self.entries[*index].subsystem;
            subsystem.reset().map_err(|e| {
                SubsystemError::OperationFailed(format!("Failed to reset '{}': {}", name, e))
            })?;
        }

        self.last_update = Instant::now();
        Ok(())
    }

    /// Async-friendly wrapper around [`reset_all`].
    pub async fn reset_all_async(&mut self) -> SubsystemResult<()> {
        self.reset_all()
    }

    /// Shutdown all subsystems
    pub fn shutdown_all(&mut self) -> SubsystemResult<()> {
        // Shutdown in reverse order
        for name in self.initialization_order.iter().rev() {
            if let Some(&index) = self.index_map.get(name) {
                let subsystem = &mut self.entries[index].subsystem;
                if let Err(e) = subsystem.shutdown() {
                    eprintln!("Warning: Failed to shutdown '{}': {}", name, e);
                }
            }
        }

        self.is_running = false;
        Ok(())
    }

    /// Async-friendly wrapper around [`shutdown_all`].
    pub async fn shutdown_all_async(&mut self) -> SubsystemResult<()> {
        self.shutdown_all()
    }

    /// Get a subsystem by name
    pub fn get_subsystem(&self, name: &str) -> Option<&(dyn SubsystemInterface + '_)> {
        self.index_map
            .get(name)
            .and_then(|&index| self.entries.get(index))
            .map(|entry| entry.subsystem.as_ref())
    }

    /// Returns true if any subsystem reports an error state.
    pub fn has_error(&self) -> bool {
        self.entries
            .iter()
            .any(|entry| entry.subsystem.state() == SubsystemState::Error)
    }

    /// Get a mutable reference to a subsystem by name
    pub fn get_subsystem_mut(&mut self, name: &str) -> Option<&mut dyn SubsystemInterface> {
        let index = *self.index_map.get(name)?;
        if let Some(entry) = self.entries.get_mut(index) {
            Some(entry.subsystem.as_mut())
        } else {
            None
        }
    }

    /// Pause a specific subsystem
    pub fn pause_subsystem(&mut self, name: &str) -> SubsystemResult<()> {
        if let Some(&index) = self.index_map.get(name) {
            self.entries[index].subsystem.pause()
        } else {
            Err(SubsystemError::InvalidConfiguration(format!(
                "Subsystem '{}' not found",
                name
            )))
        }
    }

    /// Resume a specific subsystem
    pub fn resume_subsystem(&mut self, name: &str) -> SubsystemResult<()> {
        if let Some(&index) = self.index_map.get(name) {
            self.entries[index].subsystem.resume()
        } else {
            Err(SubsystemError::InvalidConfiguration(format!(
                "Subsystem '{}' not found",
                name
            )))
        }
    }

    /// Get subsystem states
    pub fn get_subsystem_states(&self) -> HashMap<String, SubsystemState> {
        self.initialization_order
            .iter()
            .filter_map(|name| {
                self.index_map
                    .get(name)
                    .and_then(|&index| self.entries.get(index))
                    .map(|entry| (name.clone(), entry.subsystem.state()))
            })
            .collect()
    }

    /// Check if all subsystems are running
    pub fn all_running(&self) -> bool {
        self.entries
            .iter()
            .all(|entry| entry.subsystem.state() == SubsystemState::Running)
    }

    /// Get number of registered subsystems
    pub fn subsystem_count(&self) -> usize {
        self.entries.len()
    }

    /// Return the cached initialization plan.
    pub fn initialization_plan(&self) -> &[String] {
        &self.initialization_order
    }
}

impl Default for SubsystemManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Example subsystem implementation
pub struct ExampleSubsystem {
    name: String,
    state: SubsystemState,
    data: HashMap<String, Box<dyn Any + Send + Sync>>,
}

impl ExampleSubsystem {
    pub fn new(name: String) -> Self {
        Self {
            name,
            state: SubsystemState::Uninitialized,
            data: HashMap::new(),
        }
    }
}

impl SubsystemInterface for ExampleSubsystem {
    fn name(&self) -> &str {
        &self.name
    }

    fn init(&mut self) -> SubsystemResult<()> {
        self.state = SubsystemState::Initializing;
        // Perform initialization
        self.state = SubsystemState::Running;
        Ok(())
    }

    fn update(&mut self, _delta_time: Duration) -> SubsystemResult<()> {
        // Perform per-frame updates
        Ok(())
    }

    fn shutdown(&mut self) -> SubsystemResult<()> {
        self.state = SubsystemState::ShuttingDown;
        // Perform cleanup
        self.data.clear();
        self.state = SubsystemState::Shutdown;
        Ok(())
    }

    fn state(&self) -> SubsystemState {
        self.state
    }

    fn pause(&mut self) -> SubsystemResult<()> {
        if self.state == SubsystemState::Running {
            self.state = SubsystemState::Paused;
        }
        Ok(())
    }

    fn resume(&mut self) -> SubsystemResult<()> {
        if self.state == SubsystemState::Paused {
            self.state = SubsystemState::Running;
        }
        Ok(())
    }

    fn get_data(&self, key: &str) -> Option<&(dyn Any + Send + Sync)> {
        self.data.get(key).map(|v| v.as_ref())
    }

    fn set_data(&mut self, key: String, value: Box<dyn Any + Send + Sync>) -> SubsystemResult<()> {
        self.data.insert(key, value);
        Ok(())
    }

    fn as_any(&self) -> &(dyn Any + Send + Sync) {
        self
    }

    fn as_any_mut(&mut self) -> &mut (dyn Any + Send + Sync) {
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    };

    struct ResetTrackingSubsystem {
        state: SubsystemState,
        reset_counter: Arc<AtomicUsize>,
    }

    impl ResetTrackingSubsystem {
        fn new(reset_counter: Arc<AtomicUsize>) -> Self {
            Self {
                state: SubsystemState::Uninitialized,
                reset_counter,
            }
        }
    }

    impl SubsystemInterface for ResetTrackingSubsystem {
        fn name(&self) -> &str {
            "ResetTracking"
        }

        fn init(&mut self) -> SubsystemResult<()> {
            self.state = SubsystemState::Running;
            Ok(())
        }

        fn update(&mut self, _delta_time: Duration) -> SubsystemResult<()> {
            Ok(())
        }

        fn shutdown(&mut self) -> SubsystemResult<()> {
            self.state = SubsystemState::Shutdown;
            Ok(())
        }

        fn state(&self) -> SubsystemState {
            self.state
        }

        fn reset(&mut self) -> SubsystemResult<()> {
            self.reset_counter.fetch_add(1, Ordering::Relaxed);
            self.state = SubsystemState::Running;
            Ok(())
        }

        fn as_any(&self) -> &(dyn Any + Send + Sync) {
            self
        }

        fn as_any_mut(&mut self) -> &mut (dyn Any + Send + Sync) {
            self
        }
    }

    #[test]
    fn test_subsystem_manager() {
        let mut manager = SubsystemManager::new();

        let descriptor =
            SubsystemDescriptor::new(Box::new(ExampleSubsystem::new("TestSubsystem".to_string())));
        manager.register_subsystem(descriptor).unwrap();

        assert_eq!(manager.subsystem_count(), 1);

        manager.init_all().unwrap();
        assert!(manager.is_running);

        // Test getting subsystem
        let subsystem = manager.get_subsystem("TestSubsystem");
        assert!(subsystem.is_some());
        assert_eq!(subsystem.unwrap().name(), "TestSubsystem");

        manager.shutdown_all().unwrap();
        assert!(!manager.is_running);
    }

    #[test]
    fn test_subsystem_states() {
        let mut manager = SubsystemManager::new();
        let descriptor =
            SubsystemDescriptor::new(Box::new(ExampleSubsystem::new("StateTest".to_string())));
        manager.register_subsystem(descriptor).unwrap();

        manager.init_all().unwrap();

        let states = manager.get_subsystem_states();
        assert_eq!(states.get("StateTest"), Some(&SubsystemState::Running));

        manager.pause_subsystem("StateTest").unwrap();
        let states = manager.get_subsystem_states();
        assert_eq!(states.get("StateTest"), Some(&SubsystemState::Paused));

        manager.resume_subsystem("StateTest").unwrap();
        let states = manager.get_subsystem_states();
        assert_eq!(states.get("StateTest"), Some(&SubsystemState::Running));
    }

    #[test]
    fn test_reset_all_requires_init_and_invokes_reset() {
        let mut manager = SubsystemManager::new();
        let reset_counter = Arc::new(AtomicUsize::new(0));
        manager
            .register_subsystem(SubsystemDescriptor::new(Box::new(
                ResetTrackingSubsystem::new(reset_counter.clone()),
            )))
            .unwrap();

        let err = manager
            .reset_all()
            .expect_err("reset before init should fail");
        assert!(matches!(err, SubsystemError::NotInitialized));

        manager.init_all().unwrap();
        manager.reset_all().unwrap();
        assert_eq!(reset_counter.load(Ordering::Relaxed), 1);
    }
}
