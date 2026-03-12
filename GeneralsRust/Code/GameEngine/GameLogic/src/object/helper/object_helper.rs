//! Shared helper utilities (ObjectHelper equivalent).

use crate::common::{ObjectStatusTypes, TheGameLogic, UnsignedInt, NEVER};
use crate::modules::UpdateSleepTime;
use crate::object::Object;

const FOREVER: UnsignedInt = u32::MAX;

/// Compute a helper sleep interval until the requested frame.
///
/// Mirrors ObjectHelper::sleepUntil behavior from C++ with guard rails for
/// destroyed objects and NEVER/FOREVER sentinel values.
pub fn sleep_until(object: &Object, when: UnsignedInt) -> UpdateSleepTime {
    if object.get_status_bits().test(ObjectStatusTypes::Destroyed) {
        return UpdateSleepTime::Forever;
    }

    if when == NEVER || when == FOREVER {
        return UpdateSleepTime::Forever;
    }

    let current_frame = TheGameLogic::get_frame();
    if when <= current_frame {
        return UpdateSleepTime::None;
    }

    UpdateSleepTime::Frames(when - current_frame)
}
