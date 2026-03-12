use parking_lot::Mutex;
use std::sync::OnceLock;

use super::victory_conditions::AllianceState;

/// Events emitted by gameplay systems that scripts/radar/UI can consume.
#[derive(Debug, Clone)]
pub enum ScriptEvent {
    PlayerDefeated {
        player_id: u32,
    },
    AllianceStateChanged {
        player_id: u32,
        state: AllianceState,
    },
    RevealMapForPlayer {
        player_id: u32,
    },
}

static EVENT_QUEUE: OnceLock<Mutex<Vec<ScriptEvent>>> = OnceLock::new();

fn queue() -> &'static Mutex<Vec<ScriptEvent>> {
    EVENT_QUEUE.get_or_init(|| Mutex::new(Vec::new()))
}

/// Push a new script event into the global queue.
pub fn push_event(event: ScriptEvent) {
    queue().lock().push(event);
}

/// Drain all pending script events (typically once per frame).
pub fn drain_events() -> Vec<ScriptEvent> {
    let mut guard = queue().lock();
    if guard.is_empty() {
        Vec::new()
    } else {
        guard.drain(..).collect()
    }
}
