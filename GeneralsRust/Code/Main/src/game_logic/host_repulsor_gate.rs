//! C++ `TAiData::m_enableRepulsors` residual gate shared with `Object::take_damage`.
//!
//! Host combat mutates objects without borrowing `GameLogic`, so damage-time
//! civilian REPULSOR flagging reads this process-wide enable bit. `GameLogic::
//! set_enable_repulsors` keeps it in sync with the authoritative field.

use std::sync::atomic::{AtomicBool, Ordering};

static ENABLE_REPULSORS: AtomicBool = AtomicBool::new(false);

/// C++ `TheAI->getAiData()->m_enableRepulsors` residual.
#[inline]
pub fn is_enabled() -> bool {
    ENABLE_REPULSORS.load(Ordering::Relaxed)
}

/// Sync from host `GameLogic::enable_repulsors`.
#[inline]
pub fn set_enabled(enabled: bool) {
    ENABLE_REPULSORS.store(enabled, Ordering::Relaxed);
}
