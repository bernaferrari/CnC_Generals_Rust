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
pub use game_engine::common::thing::update_module::UpdateModulePtr;

/// Minimal no-op update module used in scaffolding and tests.
#[derive(Debug, Default)]
pub struct UpdateModuleDummy;

impl UpdateModuleInterface for UpdateModuleDummy {}

/// Phase ordering for sleepy updates (mirrors C++ SleepyUpdatePhase).
///
/// Hoisted into `Common` (re-exported here) so the Common `Module` trait can
/// expose the typed `get_update_module_interface()` accessor.
pub use game_engine::common::thing::update_module::SleepyUpdatePhase;
