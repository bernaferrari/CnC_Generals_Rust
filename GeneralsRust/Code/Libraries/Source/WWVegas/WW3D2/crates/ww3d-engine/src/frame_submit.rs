//! Frame-owned GPU submit.
//!
//! The encoder created by [`crate::Engine::begin_render`] is the only in-frame
//! command owner. Subsystems enqueue extra command buffers (uploads before the
//! render encoder, overlays after). [`FrameSubmitState::submit_owned_frame`]
//! issues **one** `queue.submit` at `end_render`.
//!
//! State lives on the owning [`crate::Engine`] (`frame_submit: Arc<Mutex<…>>`).
//! Free-function façades resolve the installed engine arc (or a process
//! standalone arc for out-of-engine screenshot/demo paths).

use parking_lot::Mutex;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, OnceLock};
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

/// Installed engine (or standalone) frame-submit arc for free-function façades.
///
/// `Engine` installs its owned arc at init; shutdown clears this so façades
/// fall back to the standalone process arc (out-of-engine demos/tests).
struct FrameSubmitBridgeRuntime {
    installed: Mutex<Option<Arc<Mutex<FrameSubmitState>>>>,
    standalone: OnceLock<Arc<Mutex<FrameSubmitState>>>,
}

static FRAME_SUBMIT_BRIDGE: FrameSubmitBridgeRuntime = FrameSubmitBridgeRuntime {
    installed: Mutex::new(None),
    standalone: OnceLock::new(),
};

fn standalone_frame_submit() -> &'static Arc<Mutex<FrameSubmitState>> {
    FRAME_SUBMIT_BRIDGE
        .standalone
        .get_or_init(|| Arc::new(Mutex::new(FrameSubmitState::new())))
}

fn active_frame_submit() -> Arc<Mutex<FrameSubmitState>> {
    FRAME_SUBMIT_BRIDGE
        .installed
        .lock()
        .clone()
        .unwrap_or_else(|| Arc::clone(standalone_frame_submit()))
}

/// Install the engine-owned frame-submit state for free-function bridges.
pub fn install_frame_submit(state: Arc<Mutex<FrameSubmitState>>) {
    *FRAME_SUBMIT_BRIDGE.installed.lock() = Some(state);
}

/// Clear the engine install (shutdown). Free functions fall back to standalone.
pub fn uninstall_frame_submit(state: &Arc<Mutex<FrameSubmitState>>) {
    let mut slot = FRAME_SUBMIT_BRIDGE.installed.lock();
    if slot.as_ref().is_some_and(|active| Arc::ptr_eq(active, state)) {
        *slot = None;
    }
}

fn with_active<R>(f: impl FnOnce(&FrameSubmitState) -> R) -> R {
    let arc = active_frame_submit();
    let guard = arc.lock();
    f(&guard)
}

/// Mark the engine frame open. Pending overlays from a previous leaked frame are dropped.
pub fn on_begin_frame() {
    with_active(|s| s.on_begin_frame());
}

/// True between a successful `begin_render` and `submit_owned_frame`.
pub fn frame_is_active() -> bool {
    with_active(|s| s.frame_is_active())
}

/// Record a command buffer into the open frame, or submit immediately if none is open.
pub fn submit_recorded(
    queue: &Queue,
    phase: FrameCommandPhase,
    buffer: CommandBuffer,
    out_of_frame: OutOfFrameReason,
) {
    with_active(|s| s.submit_recorded(queue, phase, buffer, out_of_frame));
}

/// Out-of-frame submit. Each call is one `queue.submit` and is counted separately.
pub fn submit_out_of_frame(
    queue: &Queue,
    buffers: impl IntoIterator<Item = CommandBuffer>,
    reason: OutOfFrameReason,
) {
    with_active(|s| s.submit_out_of_frame(queue, buffers, reason));
}

/// The single in-frame submit: uploads, then the engine encoder, then overlay/post.
pub fn submit_owned_frame(queue: &Queue, frame_encoder: CommandBuffer, frame_index: u64) {
    with_active(|s| s.submit_owned_frame(queue, frame_encoder, frame_index));
}

/// Owned `queue.submit` count for the frame that just ended (0 or 1).
pub fn last_frame_submit_count() -> u64 {
    with_active(|s| s.last_frame_submit_count())
}

/// Out-of-frame submits observed since the last `on_begin_frame`.
pub fn last_out_of_frame_submit_count() -> u64 {
    with_active(|s| s.last_out_of_frame_submit_count())
}

/// Lifetime count of engine-owned frame submits (test hook).
pub fn total_owned_submits() -> u64 {
    with_active(|s| s.total_owned_submits())
}

/// Reset counters (tests).
pub fn reset_submit_debug() {
    with_active(|s| s.reset_submit_debug());
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex as StdMutex;

    static TEST_LOCK: StdMutex<()> = StdMutex::new(());

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
    fn noop_begin_end_bookkeeping_without_gpu() {
        let _guard = TEST_LOCK.lock().unwrap();
        // Ensure install bridge is clear so we exercise standalone.
        *FRAME_SUBMIT_BRIDGE.installed.lock() = None;
        reset_submit_debug();
        assert!(!frame_is_active());
        assert_eq!(last_frame_submit_count(), 0);
        assert_eq!(last_out_of_frame_submit_count(), 0);

        on_begin_frame();
        assert!(frame_is_active());
        assert_eq!(last_frame_submit_count(), 0);

        reset_submit_debug();
        assert!(!frame_is_active());
        assert_eq!(last_frame_submit_count(), 0);
    }

    #[test]
    fn engine_owned_state_is_what_free_functions_see() {
        let _guard = TEST_LOCK.lock().unwrap();
        let owned = Arc::new(Mutex::new(FrameSubmitState::new()));
        install_frame_submit(Arc::clone(&owned));
        owned.lock().on_begin_frame();
        assert!(frame_is_active());
        assert!(owned.lock().frame_is_active());
        uninstall_frame_submit(&owned);
        // Standalone must not inherit the engine frame_active bit.
        assert!(!frame_is_active());
    }
}
