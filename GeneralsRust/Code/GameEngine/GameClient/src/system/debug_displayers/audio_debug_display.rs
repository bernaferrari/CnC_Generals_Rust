//! Audio debug display helpers, matching AudioDebugDisplay.cpp.

use crate::core::subsystems::AudioSubsystem;
use crate::system::debug_display::DebugDisplay;

pub fn audio_debug_display(display: &mut DebugDisplay, audio: &AudioSubsystem) {
    let snapshot = audio.debug_snapshot();
    display.printf(format_args!(
        "Audio events: {} (showing last {})\n",
        snapshot.total_events,
        snapshot.recent_events.len()
    ));

    for record in &snapshot.recent_events {
        match record.position.as_ref() {
            Some(pos) => {
                display.printf(format_args!(
                    "[{:>6} ms] {} @ ({:.1}, {:.1}, {:.1})\n",
                    record.timestamp_ms, record.name, pos.x, pos.y, pos.z
                ));
            }
            None => {
                display.printf(format_args!(
                    "[{:>6} ms] {}\n",
                    record.timestamp_ms, record.name
                ));
            }
        }
    }
}
