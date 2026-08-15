//! Frame-owned GPU submit.
//!
//! The encoder created by [`crate::Engine::begin_render`] is the only in-frame
//! command owner. Subsystems enqueue extra command buffers (uploads before the
//! render encoder, overlays after). [`submit_owned_frame`] issues **one**
//! `queue.submit` at `end_render`. This is not a second FrameQueue.

use parking_lot::Mutex;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use wgpu::{CommandBuffer, Queue};

/// Env var: when `1`/`true`/`yes`, log per-frame owned submit count.
pub const SUBMIT_DEBUG_ENV: &str = "GENERALS_W3D_SUBMIT_DEBUG";

/// Ordering relative to the engine's render encoder inside the single submit.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
#[repr(u8)]
pub enum FrameCommandPhase {
    /// Buffer/texture uploads that must land before draws.
    Upload = 0,
    /// Extra recorded work sharing the frame submit (video, shade, W3D C-API).
    Overlay = 1,
    /// After the main render encoder (post-process recorded off the main pass).
    Post = 2,
}

/// Documented reasons a subsystem may `queue.submit` outside `begin_render`/`end_render`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OutOfFrameReason {
    /// Screenshot/movie readback copy after the frame encoder was already submitted.
    ScreenshotReadback,
    /// Blocking debug capture (`capture_texture_to_file`).
    BlockingScreenshotCapture,
    /// W3D C-API / unit tests running without an engine frame.
    StandaloneW3dRenderer,
    /// Video tool/demo path without WW3D `begin_render`.
    StandaloneVideo,
    /// wwshade examples / tests without WW3D `begin_render`.
    StandaloneShade,
}

struct PendingCommand {
    phase: FrameCommandPhase,
    buffer: CommandBuffer,
}

static FRAME_ACTIVE: AtomicBool = AtomicBool::new(false);
static PENDING: Mutex<Vec<PendingCommand>> = Mutex::new(Vec::new());
static LAST_FRAME_SUBMIT_COUNT: AtomicU64 = AtomicU64::new(0);
static LAST_OUT_OF_FRAME_COUNT: AtomicU64 = AtomicU64::new(0);
static TOTAL_OWNED_SUBMITS: AtomicU64 = AtomicU64::new(0);
static FRAME_INDEX_AT_SUBMIT: AtomicU64 = AtomicU64::new(0);

fn submit_debug_enabled() -> bool {
    match std::env::var(SUBMIT_DEBUG_ENV) {
        Ok(value) => matches!(
            value.trim().to_ascii_lowercase().as_str(),
            "1" | "true" | "yes" | "on"
        ),
        Err(_) => false,
    }
}

/// Mark the engine frame open. Pending overlays from a previous leaked frame are dropped.
pub fn on_begin_frame() {
    FRAME_ACTIVE.store(true, Ordering::SeqCst);
    LAST_FRAME_SUBMIT_COUNT.store(0, Ordering::SeqCst);
    LAST_OUT_OF_FRAME_COUNT.store(0, Ordering::SeqCst);
    PENDING.lock().clear();
}

/// True between a successful `begin_render` and `submit_owned_frame`.
pub fn frame_is_active() -> bool {
    FRAME_ACTIVE.load(Ordering::SeqCst)
}

/// Record a command buffer into the open frame, or submit immediately if none is open.
pub fn submit_recorded(
    queue: &Queue,
    phase: FrameCommandPhase,
    buffer: CommandBuffer,
    out_of_frame: OutOfFrameReason,
) {
    if FRAME_ACTIVE.load(Ordering::SeqCst) {
        PENDING.lock().push(PendingCommand { phase, buffer });
        return;
    }
    submit_out_of_frame(queue, std::iter::once(buffer), out_of_frame);
}

/// Out-of-frame submit. Each call is one `queue.submit` and is counted separately.
pub fn submit_out_of_frame(
    queue: &Queue,
    buffers: impl IntoIterator<Item = CommandBuffer>,
    reason: OutOfFrameReason,
) {
    let buffers: Vec<CommandBuffer> = buffers.into_iter().collect();
    if buffers.is_empty() {
        return;
    }
    LAST_OUT_OF_FRAME_COUNT.fetch_add(1, Ordering::SeqCst);
    if submit_debug_enabled() {
        eprintln!("w3d-frame-submit out-of-frame reason={reason:?} count=1");
    }
    queue.submit(buffers);
}

/// The single in-frame submit: uploads, then the engine encoder, then overlay/post.
pub fn submit_owned_frame(queue: &Queue, frame_encoder: CommandBuffer, frame_index: u64) {
    let mut pending = std::mem::take(&mut *PENDING.lock());
    pending.sort_by_key(|pending| pending.phase as u8);

    let mut uploads = Vec::new();
    let mut after = Vec::new();
    for command in pending {
        match command.phase {
            FrameCommandPhase::Upload => uploads.push(command.buffer),
            FrameCommandPhase::Overlay | FrameCommandPhase::Post => after.push(command.buffer),
        }
    }

    let mut buffers = uploads;
    buffers.push(frame_encoder);
    buffers.extend(after);

    queue.submit(buffers);
    LAST_FRAME_SUBMIT_COUNT.store(1, Ordering::SeqCst);
    TOTAL_OWNED_SUBMITS.fetch_add(1, Ordering::SeqCst);
    FRAME_INDEX_AT_SUBMIT.store(frame_index, Ordering::SeqCst);
    FRAME_ACTIVE.store(false, Ordering::SeqCst);

    if submit_debug_enabled() {
        eprintln!("w3d-frame-submit frame={frame_index} count=1");
    }
}

/// Owned `queue.submit` count for the frame that just ended (0 or 1).
pub fn last_frame_submit_count() -> u64 {
    LAST_FRAME_SUBMIT_COUNT.load(Ordering::SeqCst)
}

/// Out-of-frame submits observed since the last `on_begin_frame`.
pub fn last_out_of_frame_submit_count() -> u64 {
    LAST_OUT_OF_FRAME_COUNT.load(Ordering::SeqCst)
}

/// Lifetime count of engine-owned frame submits (test hook).
pub fn total_owned_submits() -> u64 {
    TOTAL_OWNED_SUBMITS.load(Ordering::SeqCst)
}

/// Reset counters (tests).
pub fn reset_submit_debug() {
    LAST_FRAME_SUBMIT_COUNT.store(0, Ordering::SeqCst);
    LAST_OUT_OF_FRAME_COUNT.store(0, Ordering::SeqCst);
    TOTAL_OWNED_SUBMITS.store(0, Ordering::SeqCst);
    FRAME_INDEX_AT_SUBMIT.store(0, Ordering::SeqCst);
    PENDING.lock().clear();
    FRAME_ACTIVE.store(false, Ordering::SeqCst);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn debug_env_name_is_stable() {
        assert_eq!(SUBMIT_DEBUG_ENV, "GENERALS_W3D_SUBMIT_DEBUG");
    }

    #[test]
    fn phase_orders_uploads_before_overlays() {
        assert!(FrameCommandPhase::Upload < FrameCommandPhase::Overlay);
        assert!(FrameCommandPhase::Overlay < FrameCommandPhase::Post);
    }
}
