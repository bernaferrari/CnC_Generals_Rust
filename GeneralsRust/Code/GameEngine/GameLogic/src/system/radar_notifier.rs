//! Lightweight radar update fan-out.
//!
//! This mirrors the beacon manager pattern: GameLogic pushes radar updates here
//! and UI/network layers can drain them later without reaching into the
//! GameLogic instance directly. It is intentionally minimal to match the C++
//! "game logic produces, client consumes" model.

use super::game_logic::{RadarEventType, RadarUpdate};
use std::collections::VecDeque;
use std::sync::{Mutex, OnceLock};

fn global_queue() -> &'static Mutex<VecDeque<RadarUpdate>> {
    static RADAR_QUEUE: OnceLock<Mutex<VecDeque<RadarUpdate>>> = OnceLock::new();
    RADAR_QUEUE.get_or_init(|| Mutex::new(VecDeque::new()))
}

/// Push a radar update for later consumption by the client/HUD.
///
/// C++ `Object::attemptDamage` calls `TheRadar->tryUnderAttackEvent(this)`
/// directly. Crate damage still queues `RadarUpdate{BaseAttacked}`; convert
/// that into a live UnderAttack ping + glow/UI/audio/EVA.
pub fn push(update: &RadarUpdate) {
    if let Ok(mut queue) = global_queue().lock() {
        queue.push_back(update.clone());
    }
    if matches!(update.event_type, RadarEventType::BaseAttacked) {
        let _ = crate::helpers::TheRadar::try_under_attack_event_at(
            update.position.0,
            update.position.1,
        );
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
