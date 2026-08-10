// Sleep conversion, dummy module, SleepyUpdatePhase
//
// Split from `modules.rs` for module-size parity.
// Observable behavior is unchanged.

/// Update sleep time type - re-export from object::helper
pub use crate::object::helper::UpdateSleepTime;

/// Convert up to four candidate wake frames into an UpdateSleepTime relative to now.
/// Mirrors C++ UpdateModule::frameToSleepTime behavior.
pub fn frame_to_sleep_time(
    mut frame1: UnsignedInt,
    frame2: UnsignedInt,
    frame3: UnsignedInt,
    frame4: UnsignedInt,
) -> UpdateSleepTime {
    if frame1 > frame2 {
        frame1 = frame2;
    }
    if frame1 > frame3 {
        frame1 = frame3;
    }
    if frame1 > frame4 {
        frame1 = frame4;
    }

    let now = TheGameLogic::get_frame();
    if frame1 > now {
        UpdateSleepTime::frames(frame1 - now)
    } else if frame1 == now {
        UpdateSleepTime::None
    } else {
        log::warn!("frame_to_sleep_time: frame is in the past ({frame1} < {now})");
        UpdateSleepTime::None
    }
}

/// Update module pointer type
pub type UpdateModulePtr = Arc<RwLock<dyn UpdateModuleInterface>>;

/// Minimal no-op update module used in scaffolding and tests.
#[derive(Debug, Default)]
pub struct UpdateModuleDummy;

impl UpdateModuleInterface for UpdateModuleDummy {}

/// Phase ordering for sleepy updates (mirrors C++ SleepyUpdatePhase).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[repr(u8)]
pub enum SleepyUpdatePhase {
    Initial = 0,
    Physics = 1,
    Normal = 2,
    Final = 3,
}

impl Default for SleepyUpdatePhase {
    fn default() -> Self {
        SleepyUpdatePhase::Normal
    }
}

