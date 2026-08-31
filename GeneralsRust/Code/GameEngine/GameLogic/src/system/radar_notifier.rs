//! Lightweight radar update fan-out.
//!
//! This mirrors the beacon manager pattern: GameLogic pushes radar updates here
//! and UI/network layers can drain them later without reaching into the
//! GameLogic instance directly. It is intentionally minimal to match the C++
//! "game logic produces, client consumes" model.

use super::game_logic::RadarUpdate;
use std::collections::VecDeque;
use std::sync::{Mutex, OnceLock};

fn global_queue() -> &'static Mutex<VecDeque<RadarUpdate>> {
    static RADAR_QUEUE: OnceLock<Mutex<VecDeque<RadarUpdate>>> = OnceLock::new();
    RADAR_QUEUE.get_or_init(|| Mutex::new(VecDeque::new()))
}

/// Push a radar update for later consumption by the client/HUD.
///
/// C++ `Object::attemptDamage` calls `TheRadar->tryUnderAttackEvent(this)`
/// directly (Object.cpp:1854); the engine damage path does the same through
/// `TheRadar::try_under_attack_event_for_object`, so the under-attack ping and
/// its per-kind message/audio/EVA fire there, gated on the throttled event
/// actually being created (Radar.cpp:1155-1224). This queue stays for the
/// remaining typed radar updates (beacons, unit created/destroyed, ...).
pub fn push(update: &RadarUpdate) {
    if let Ok(mut queue) = global_queue().lock() {
        queue.push_back(update.clone());
    }
}

/// Drain all pending radar updates.
pub fn drain() -> Vec<RadarUpdate> {
    if let Ok(mut queue) = global_queue().lock() {
        if queue.is_empty() {
            Vec::new()
        } else {
            queue.drain(..).collect()
        }
    } else {
        Vec::new()
    }
}
