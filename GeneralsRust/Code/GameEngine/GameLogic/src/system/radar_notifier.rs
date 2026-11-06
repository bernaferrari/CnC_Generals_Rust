//! Lightweight radar update fan-out.
//!
//! GameLogic pushes radar updates onto a world's queue; UI/network layers drain
//! them later without reaching into the `GameLogic` instance. Matches the C++
//! "game logic produces, client consumes" model without a process-wide queue.

use super::game_logic::RadarUpdate;
use crate::system::engine_stores::{active_services, try_active_services, WorldServices};
use std::sync::Arc;

/// Push a radar update onto `services` for later client/HUD consumption.
pub fn push_on(services: &Arc<WorldServices>, update: &RadarUpdate) {
    services
        .radar_updates()
        .lock()
        .unwrap_or_else(|p| p.into_inner())
        .push_back(update.clone());
}

/// Drain all pending radar updates from `services`.
pub fn drain_on(services: &Arc<WorldServices>) -> Vec<RadarUpdate> {
    services
        .radar_updates()
        .lock()
        .unwrap_or_else(|p| p.into_inner())
        .drain(..)
        .collect()
}

/// Legacy free-fn bridge — prefer [`push_on`] with the driving world.
#[deprecated(note = "pass &Arc<WorldServices> to push_on (hq-spb88j)")]
pub fn push(update: &RadarUpdate) {
    if let Some(services) = try_active_services() {
        push_on(&services, update);
    }
}

/// Legacy free-fn bridge — prefer [`drain_on`] with the driving world.
#[deprecated(note = "pass &Arc<WorldServices> to drain_on (hq-spb88j)")]
pub fn drain() -> Vec<RadarUpdate> {
    match try_active_services() {
        Some(services) => drain_on(&services),
        None => Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::system::engine_stores::new_world_services;
    use crate::system::game_logic::{RadarEventType, RadarUpdate};
    use std::sync::{Arc, Barrier, Mutex};
    use std::thread;

    fn sample(player_id: i32) -> RadarUpdate {
        RadarUpdate {
            player_id,
            position: (player_id as f32, 0.0),
            event_type: RadarEventType::UnitCreated,
        }
    }

    #[test]
    fn parallel_worlds_keep_independent_radar_queues() {
        // Owned path: no with_world_services / TLS binder — each thread holds
        // its Arc<WorldServices> and calls push_on/drain_on directly.
        let barrier = Arc::new(Barrier::new(2));
        let results = Arc::new(Mutex::new(Vec::new()));
        let mut handles = Vec::new();
        for seed in [11i32, 22i32] {
            let barrier = Arc::clone(&barrier);
            let results = Arc::clone(&results);
            handles.push(thread::spawn(move || {
                let services = new_world_services();
                barrier.wait();
                push_on(&services, &sample(seed));
                barrier.wait();
                let drained = drain_on(&services);
                results.lock().unwrap().push((seed, drained));
            }));
        }
        for h in handles {
            h.join().unwrap();
        }
        let mut got = results.lock().unwrap();
        got.sort_by_key(|(seed, _)| *seed);
        assert_eq!(got[0].1.len(), 1);
        assert_eq!(got[0].1[0].player_id, 11);
        assert_eq!(got[1].1.len(), 1);
        assert_eq!(got[1].1[0].player_id, 22);
    }

    #[test]
    fn drain_on_ignores_foreign_active_world_services() {
        use crate::system::engine_stores::with_world_services;
        let owner = new_world_services();
        let foreign = new_world_services();
        push_on(&owner, &sample(7));
        // Foreign world is the TLS active selection; owned drain must still see owner.
        let drained = with_world_services(&foreign, || drain_on(&owner));
        assert_eq!(drained.len(), 1);
        assert_eq!(drained[0].player_id, 7);
        assert!(drain_on(&foreign).is_empty());
        assert!(drain_on(&owner).is_empty());
    }
}
