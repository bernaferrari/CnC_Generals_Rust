//! Frame-owned GPU submit.
//!
//! The encoder created by [`crate::Engine::begin_render`] is the only in-frame
//! command owner. Subsystems enqueue extra command buffers (uploads before the
//! render encoder, overlays after). [`FrameSubmitState::submit_owned_frame`]
//! issues **one** `queue.submit` at `end_render`.
//!
//! State lives on the owning [`crate::Engine`] / renderer (`Arc<Mutex<FrameSubmitState>>`).
//! Helpers take an explicit `&FrameSubmitState` — no process-wide bridge.

use parking_lot::Mutex;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
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
    /// Blocking GPU readback (cull/map) that must complete before CPU continues.
    BlockingGpuReadback,
}

struct PendingCommand {
    phase: FrameCommandPhase,
    buffer: CommandBuffer,
}

#[cfg(target_arch = "wasm32")]
// SAFETY: wasm32-only. `buffer: wgpu::CommandBuffer` is an `Rc`-backed handle
// on the web backend and therefore `!Send` by default. wasm32 has no threads,
// so the encoder can never move to or be submitted from another thread.
unsafe impl Send for PendingCommand {}
#[cfg(target_arch = "wasm32")]
// SAFETY: same wasm32-only scope: the target is single-threaded.
unsafe impl Sync for PendingCommand {}

/// Per-engine (or standalone) frame-submit bookkeeping.
///
/// C++ WW3D keeps one in-flight device frame on the engine object; this is the
/// Rust equivalent owned by [`crate::Engine`].
#[derive(Default)]
pub struct FrameSubmitState {
    frame_active: AtomicBool,
    pending: Mutex<Vec<PendingCommand>>,
    last_frame_submit_count: AtomicU64,
    last_out_of_frame_count: AtomicU64,
    total_owned_submits: AtomicU64,
    frame_index_at_submit: AtomicU64,
}

impl std::fmt::Debug for FrameSubmitState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FrameSubmitState")
            .field("frame_active", &self.frame_active.load(Ordering::Relaxed))
            .field("pending_len", &self.pending.lock().len())
            .field(
                "last_frame_submit_count",
                &self.last_frame_submit_count.load(Ordering::Relaxed),
            )
            .field(
                "last_out_of_frame_count",
                &self.last_out_of_frame_count.load(Ordering::Relaxed),
            )
            .field(
                "total_owned_submits",
                &self.total_owned_submits.load(Ordering::Relaxed),
            )
            .finish()
    }
}

impl FrameSubmitState {
    pub fn new() -> Self {
        Self::default()
    }

    /// Mark the engine frame open. Pending overlays from a previous leaked frame are dropped.
    pub fn on_begin_frame(&self) {
        self.frame_active.store(true, Ordering::SeqCst);
        self.last_frame_submit_count.store(0, Ordering::SeqCst);
        self.last_out_of_frame_count.store(0, Ordering::SeqCst);
        self.pending.lock().clear();
    }

    /// True between a successful `begin_render` and `submit_owned_frame`.
    pub fn frame_is_active(&self) -> bool {
        self.frame_active.load(Ordering::SeqCst)
    }

    /// Record a command buffer into the open frame, or submit immediately if none is open.
    pub fn submit_recorded(
        &self,
        queue: &Queue,
        phase: FrameCommandPhase,
        buffer: CommandBuffer,
        out_of_frame: OutOfFrameReason,
    ) {
        if self.frame_active.load(Ordering::SeqCst) {
            self.pending.lock().push(PendingCommand { phase, buffer });
            return;
        }
        self.submit_out_of_frame(queue, std::iter::once(buffer), out_of_frame);
    }

    /// Out-of-frame submit. Each call is one `queue.submit` and is counted separately.
    pub fn submit_out_of_frame(
        &self,
        queue: &Queue,
        buffers: impl IntoIterator<Item = CommandBuffer>,
        reason: OutOfFrameReason,
    ) {
        let buffers: Vec<CommandBuffer> = buffers.into_iter().collect();
        if buffers.is_empty() {
            return;
        }
        self.last_out_of_frame_count.fetch_add(1, Ordering::SeqCst);
        if submit_debug_enabled() {
            eprintln!("w3d-frame-submit out-of-frame reason={reason:?} count=1");
        }
        queue.submit(buffers);
    }

    /// The single in-frame submit: uploads, then the engine encoder, then overlay/post.
    pub fn submit_owned_frame(&self, queue: &Queue, frame_encoder: CommandBuffer, frame_index: u64) {
        let mut pending = std::mem::take(&mut *self.pending.lock());
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
        self.last_frame_submit_count.store(1, Ordering::SeqCst);
        self.total_owned_submits.fetch_add(1, Ordering::SeqCst);
        self.frame_index_at_submit.store(frame_index, Ordering::SeqCst);
        self.frame_active.store(false, Ordering::SeqCst);

        if submit_debug_enabled() {
            eprintln!("w3d-frame-submit frame={frame_index} count=1");
        }
    }

    pub fn last_frame_submit_count(&self) -> u64 {
        self.last_frame_submit_count.load(Ordering::SeqCst)
    }

    pub fn last_out_of_frame_submit_count(&self) -> u64 {
        self.last_out_of_frame_count.load(Ordering::SeqCst)
    }

    pub fn total_owned_submits(&self) -> u64 {
        self.total_owned_submits.load(Ordering::SeqCst)
    }

    pub fn reset_submit_debug(&self) {
        self.last_frame_submit_count.store(0, Ordering::SeqCst);
        self.last_out_of_frame_count.store(0, Ordering::SeqCst);
        self.total_owned_submits.store(0, Ordering::SeqCst);
        self.frame_index_at_submit.store(0, Ordering::SeqCst);
        self.pending.lock().clear();
        self.frame_active.store(false, Ordering::SeqCst);
    }
}

fn submit_debug_enabled() -> bool {
    match std::env::var(SUBMIT_DEBUG_ENV) {
        Ok(value) => matches!(
            value.trim().to_ascii_lowercase().as_str(),
            "1" | "true" | "yes" | "on"
        ),
        Err(_) => false,
    }
}

/// Helpers that operate on an explicitly owned [`FrameSubmitState`].
/// Call sites pass the engine/renderer-owned state — there is no process bridge.

pub fn on_begin_frame(state: &FrameSubmitState) {
    state.on_begin_frame();
}

pub fn frame_is_active(state: &FrameSubmitState) -> bool {
    state.frame_is_active()
}

pub fn submit_recorded(
    state: &FrameSubmitState,
    queue: &Queue,
    phase: FrameCommandPhase,
    buffer: CommandBuffer,
    out_of_frame: OutOfFrameReason,
) {
    state.submit_recorded(queue, phase, buffer, out_of_frame);
}

pub fn submit_out_of_frame(
    state: &FrameSubmitState,
    queue: &Queue,
    buffers: impl IntoIterator<Item = CommandBuffer>,
    reason: OutOfFrameReason,
) {
    state.submit_out_of_frame(queue, buffers, reason);
}

pub fn submit_owned_frame(
    state: &FrameSubmitState,
    queue: &Queue,
    frame_encoder: CommandBuffer,
    frame_index: u64,
) {
    state.submit_owned_frame(queue, frame_encoder, frame_index);
}

pub fn last_frame_submit_count(state: &FrameSubmitState) -> u64 {
    state.last_frame_submit_count()
}

pub fn last_out_of_frame_submit_count(state: &FrameSubmitState) -> u64 {
    state.last_out_of_frame_submit_count()
}

pub fn total_owned_submits(state: &FrameSubmitState) -> u64 {
    state.total_owned_submits()
}

pub fn reset_submit_debug(state: &FrameSubmitState) {
    state.reset_submit_debug();
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    #[test]
    fn debug_env_name_is_stable() {
        assert_eq!(SUBMIT_DEBUG_ENV, "GENERALS_W3D_SUBMIT_DEBUG");
    }

    #[test]
    fn phase_orders_uploads_before_overlays() {
        assert!(FrameCommandPhase::Upload < FrameCommandPhase::Overlay);
        assert!(FrameCommandPhase::Overlay < FrameCommandPhase::Post);
    }

    #[test]
    fn owned_states_on_two_arcs_are_independent() {
        let a = Arc::new(Mutex::new(FrameSubmitState::new()));
        let b = Arc::new(Mutex::new(FrameSubmitState::new()));
        a.lock().on_begin_frame();
        assert!(a.lock().frame_is_active());
        assert!(!b.lock().frame_is_active());
        b.lock().on_begin_frame();
        assert!(b.lock().frame_is_active());
        a.lock().reset_submit_debug();
        assert!(!a.lock().frame_is_active());
        assert!(b.lock().frame_is_active());
    }
}
