//! Per-client W3DDisplay draw schedule latches (was process atomics).
use std::sync::atomic::Ordering;

#[derive(Debug)]
pub struct ClientDrawScheduleState {
    pub sync_time_ms: u32,
    pub last_display_client_frame: u32,
    pub last_cpu_phase_frame: u32,
}

impl Default for ClientDrawScheduleState {
    fn default() -> Self {
        Self {
            sync_time_ms: 0,
            last_display_client_frame: u32::MAX,
            last_cpu_phase_frame: u32::MAX,
        }
    }
}

impl ClientDrawScheduleState {
    pub fn swap_last_display_frame(&mut self, client_frame: u32) -> u32 {
        let last = self.last_display_client_frame;
        self.last_display_client_frame = client_frame;
        last
    }

    pub fn advance_sync(&mut self, freeze_time: bool, frame_ms: u32) -> u32 {
        if !freeze_time {
            self.sync_time_ms = self.sync_time_ms.wrapping_add(frame_ms);
        }
        self.sync_time_ms
    }
}
