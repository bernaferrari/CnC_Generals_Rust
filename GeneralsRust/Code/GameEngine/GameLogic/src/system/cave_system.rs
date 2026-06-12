//! Legacy `CaveSystem` port.
//!
//! Tracks tunnel network indices and provides shared contain lists for cave
//! entrances. Mirrors the C++ vector/index semantics so cave indices remain
//! stable across save/load and gameplay logic.

use crate::common::{GameResult, ObjectID};
use crate::tunnel_tracker::TunnelTracker;
use std::sync::{Arc, Mutex, OnceLock, RwLock};

/// Manages tunnel trackers keyed by "cave index".
#[derive(Debug, Default)]
pub struct CaveSystem {
    trackers: Vec<Option<Arc<RwLock<TunnelTracker>>>>,
}

static THE_CAVE_SYSTEM: OnceLock<Arc<Mutex<CaveSystem>>> = OnceLock::new();

/// Global cave system accessor matching the legacy `TheCaveSystem` singleton.
pub fn get_cave_system() -> Arc<Mutex<CaveSystem>> {
    Arc::clone(THE_CAVE_SYSTEM.get_or_init(|| Arc::new(Mutex::new(CaveSystem::new()))))
}

pub use get_cave_system as TheCaveSystem;

impl CaveSystem {
    /// Construct an empty cave system.
    pub fn new() -> Self {
        Self {
            trackers: Vec::new(),
        }
    }

    /// Initialize the system (no-op for parity).
    pub fn init(&mut self) {}

    /// Reset the system by clearing all trackers.
    pub fn reset(&mut self) {
        self.trackers.clear();
    }

    /// Update tick (no-op for parity).
    pub fn update(&mut self) {}

    /// Register a new cave index. Creates the tracker on demand.
    pub fn register_new_cave(&mut self, index: i32) -> GameResult<()> {
        if index < 0 {
            return Err("Invalid cave index".into());
        }

        let idx = index as usize;
        if self.trackers.len() <= idx {
            self.trackers.resize_with(idx + 1, || None);
        }

        if self.trackers[idx].is_none() {
            self.trackers[idx] = Some(Arc::new(RwLock::new(TunnelTracker::new())));
        }

        Ok(())
    }

    /// Unregister is a no-op to preserve index stability (matches C++).
    pub fn unregister_cave(&mut self, _index: i32) -> GameResult<()> {
        Ok(())
    }

    /// Obtain the tracker for the specified index.
    pub fn get_tunnel_tracker_for_cave_index(
        &self,
        index: i32,
    ) -> GameResult<Arc<RwLock<TunnelTracker>>> {
        if index < 0 {
            return Err("Invalid cave index".into());
        }

        let tracker = self
            .trackers
            .get(index as usize)
            .and_then(|entry| entry.as_ref())
            .cloned()
            .ok_or_else(|| {
                std::io::Error::new(std::io::ErrorKind::NotFound, "Cave index is not registered")
            })?;

        Ok(tracker)
    }

    /// Determine whether a cave can switch indices.  The legacy implementation
    /// prevents switching if either index has contained units.
    pub fn can_switch_index_to_index(&self, old_index: i32, new_index: i32) -> GameResult<bool> {
        if old_index < 0 || new_index < 0 {
            return Err("Invalid cave index".into());
        }

        if let Some(Some(tracker)) = self.trackers.get(old_index as usize) {
            let guard = tracker.read().map_err(|_| "TunnelTracker lock poisoned")?;
            if guard.get_contain_count()? > 0 {
                return Ok(false);
            }
        }

        if let Some(Some(tracker)) = self.trackers.get(new_index as usize) {
            let guard = tracker.read().map_err(|_| "TunnelTracker lock poisoned")?;
            if guard.get_contain_count()? > 0 {
                return Ok(false);
            }
        }

        Ok(true)
    }

    /// Convenience helper exposing the list of registered indices.  Useful for
    /// debugging or future scripting ports.
    pub fn registered_indices(&self) -> Vec<i32> {
        self.trackers
            .iter()
            .enumerate()
            .filter_map(|(index, tracker)| tracker.as_ref().map(|_| index as i32))
            .collect()
    }

    /// Resolve the tracker for a specific container object.  This is a utility
    /// used by some legacy helper functions when dealing with distributed
    /// garrisons.
    pub fn find_tracker_containing_object(
        &self,
        object_id: ObjectID,
    ) -> Option<Arc<RwLock<TunnelTracker>>> {
        for tracker in self.trackers.iter().flatten() {
            if let Ok(guard) = tracker.read() {
                if guard.contains_container(object_id) {
                    return Some(tracker.clone());
                }
            }
        }

        None
    }
}
