//! Lightweight radar update fan-out.
//!
//! GameLogic pushes radar updates onto the active world's queue; UI/network
//! layers drain them later without reaching into the `GameLogic` instance.
//! Matches the C++ "game logic produces, client consumes" model without a
//! process-wide queue.

use super::game_logic::RadarUpdate;
use crate::system::engine_stores::active_services;

/// Push a radar update for later consumption by the client/HUD.
///
/// C++ `Object::attemptDamage` calls `TheRadar->tryUnderAttackEvent(this)`
/// directly (Object.cpp:1854); the engine damage path does the same through
/// `TheRadar::try_under_attack_event_for_object`, so the under-attack ping and
/// its per-kind message/audio/EVA fire there, gated on the throttled event
/// actually being created (Radar.cpp:1155-1224). This queue stays for the
/// remaining typed radar updates (beacons, unit created/destroyed, ...).
pub fn push(update: &RadarUpdate) {
    active_services()
        .radar_updates()
        .lock()
        .unwrap_or_else(|p| p.into_inner())
        .push_back(update.clone());
}

/// Drain all pending radar updates for the active world.
pub fn drain() -> Vec<RadarUpdate> {
    active_services()
        .radar_updates()
        .lock()
        .unwrap_or_else(|p| p.into_inner())
        .drain(..)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::system::engine_stores::{new_world_services, with_world_services};
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
        let barrier = Arc::new(Barrier::new(2));
        let results = Arc::new(Mutex::new(Vec::new()));
        let mut handles = Vec::new();
        for seed in [11i32, 22i32] {
            let barrier = Arc::clone(&barrier);
            let results = Arc::clone(&results);
            handles.push(thread::spawn(move || {
                let services = new_world_services();
                with_world_services(&services, || {
                    barrier.wait();
                    push(&sample(seed));
                    barrier.wait();
                    let drained = drain();
                    results.lock().unwrap().push((seed, drained));
                });
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
}
