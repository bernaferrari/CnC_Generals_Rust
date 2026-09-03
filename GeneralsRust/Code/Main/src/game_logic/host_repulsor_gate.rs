//! C++ `TAiData::m_enableRepulsors` residual gate shared with `Object::take_damage`.
//!
//! Host combat mutates objects without borrowing `GameLogic`, so damage-time
//! civilian REPULSOR flagging reads this process-wide enable bit. `GameLogic::
//! set_enable_repulsors` keeps it in sync with the authoritative field.

use std::sync::atomic::{AtomicBool, Ordering};

static ENABLE_REPULSORS: AtomicBool = AtomicBool::new(false);
static AIDATA_INI_APPLIED: AtomicBool = AtomicBool::new(false);

/// Retail `Default/AIData.ini` `EnableRepulsors = Yes`.
/// C++ `TAiData` ctor default is false; INI overwrites at TheAI init.
pub const RETAIL_ENABLE_REPULSORS: bool = true;

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

/// Mark that AIData.ini was parsed into the leftover store this process.
#[inline]
pub fn mark_aidata_ini_applied() {
    AIDATA_INI_APPLIED.store(true, Ordering::Relaxed);
}

#[inline]
pub fn aidata_ini_applied() -> bool {
    AIDATA_INI_APPLIED.load(Ordering::Relaxed)
}

#[cfg(test)]
pub fn clear_aidata_ini_applied_for_test() {
    AIDATA_INI_APPLIED.store(false, Ordering::Relaxed);
}

/// Resolve EnableRepulsors for the live player path.
///
/// When AIData.ini has been loaded, honor the parsed field (C++ TheAI).
/// Otherwise use retail Yes so civilian panic is not dead without extracted INI.
pub fn from_aidata() -> bool {
    if aidata_ini_applied() {
        if let Some(data) = game_engine::common::ini::get_ai_data_store().get_active() {
            return data.enable_repulsors;
        }
    }
    RETAIL_ENABLE_REPULSORS
}

/// Apply leftover `the_ai` + process-wide gate from the resolved AIData value.
pub fn apply_resolved_to_leftover_and_gate() -> bool {
    let enabled = from_aidata();
    set_enabled(enabled);
    let ai_store = gamelogic::ai::the_ai(); if let Ok(ai) = ai_store.write() {
        if let Ok(mut data) = ai.get_ai_data().write() {
            data.enable_repulsors = enabled;
        }
    }
    enabled
}
