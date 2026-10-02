//! Typed host delivery for OptionsMenu settings that affect Main-owned input
//! and presentation.
//!
//! GameClient still owns the retail OptionsMenu WND and its standalone
//! `TheInGameUI` compatibility state.  The Main executable owns camera input
//! for an AuthorityOnly match, so a setting that changes camera behaviour must
//! cross that boundary as typed data rather than through the legacy singleton.

use std::{
    cell::RefCell,
    collections::VecDeque,
    sync::{Mutex, OnceLock},
};

/// A user preference selected through the retail OptionsMenu WND whose live
/// effect belongs to Main's input/camera authority.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HostOptionsRequest {
    /// Match C++ `GlobalData::m_useAlternateMouse`: Main owns physical world
    /// input in an AuthorityOnly match, while standalone GameClient retains
    /// its legacy translator/global-data route.
    AlternateMouse { enabled: bool },
    /// Match C++ `MoveRMBScrollAnchor`: during RMB drag scrolling, keep the
    /// camera anchor within half a display width/height of the cursor.
    MoveRmbScrollAnchor { enabled: bool },
    /// Match C++ `DrawRMBScrollAnchor`: render the active RMB drag anchor as
    /// the retail green/black cross in Main's presentation path.
    DrawRmbScrollAnchor { enabled: bool },
}

type DisplaySetGammaFn = fn(f32, f32, f32, bool);
static DISPLAY_SET_GAMMA: OnceLock<DisplaySetGammaFn> = OnceLock::new();

/// Register C++ `TheDisplay->setGamma` implementation (W3DDisplay / Close03).
pub fn set_display_gamma_hook(hook: DisplaySetGammaFn) {
    let _ = DISPLAY_SET_GAMMA.set(hook);
}

/// C++ `TheDisplay->setGamma(gamma, 0.0f, 1.0f, FALSE)`.
pub fn apply_display_gamma(gamma: f32, bright: f32, contrast: f32, calibrate: bool) {
    if let Some(hook) = DISPLAY_SET_GAMMA.get() {
        hook(gamma, bright, contrast, calibrate);
    }
}

#[derive(Default)]
struct HostOptionsBridge {
    enabled: bool,
    requests: VecDeque<HostOptionsRequest>,
}

// THREAD: main thread only — published by GUI-thread OptionsMenu WND callbacks
// and drained by the host on the same thread.
thread_local! {
    static HOST_OPTIONS_BRIDGE: RefCell<HostOptionsBridge> =
        RefCell::new(HostOptionsBridge::default());
}

fn with_host_options_bridge<R>(f: impl FnOnce(&mut HostOptionsBridge) -> R) -> R {
    HOST_OPTIONS_BRIDGE.with_borrow_mut(f)
}

/// Enable or disable Main-host delivery for OptionsMenu camera preferences.
///
/// Standalone GameClient keeps the C++ compatibility route when this is
/// disabled. Changing owners drops only unconsumed preference updates; Main
/// retains the preference it has already applied.
pub fn set_host_options_bridge_enabled(enabled: bool) {
    with_host_options_bridge(|bridge| {
        if bridge.enabled != enabled {
            bridge.enabled = enabled;
            bridge.requests.clear();
        }
    });
}

/// Drain the camera-preference requests published by live OptionsMenu WND
/// callbacks.
pub fn take_host_options_requests() -> Vec<HostOptionsRequest> {
    with_host_options_bridge(|bridge| bridge.requests.drain(..).collect())
}

/// Publish an OptionsMenu update only when Main owns its live effect.
/// Returning `false` preserves the standalone legacy callback behaviour.
pub(crate) fn publish_host_move_rmb_scroll_anchor(enabled: bool) -> bool {
    with_host_options_bridge(|bridge| {
        if !bridge.enabled {
            return false;
        }
        bridge
            .requests
            .push_back(HostOptionsRequest::MoveRmbScrollAnchor { enabled });
        true
    })
}

/// Publish an Alternate Mouse update only when Main owns physical world
/// input.  Returning `false` deliberately leaves the standalone GameClient
/// compatibility path untouched.
pub(crate) fn publish_host_alternate_mouse(enabled: bool) -> bool {
    with_host_options_bridge(|bridge| {
        if !bridge.enabled {
            return false;
        }
        bridge
            .requests
            .push_back(HostOptionsRequest::AlternateMouse { enabled });
        true
    })
}

/// Publish a DrawRMBScrollAnchor update only when Main owns the active
/// camera-drag presentation.  The setting remains process/UI state rather
/// than savegame state, just like its moving-anchor sibling.
pub(crate) fn publish_host_draw_rmb_scroll_anchor(enabled: bool) -> bool {
    with_host_options_bridge(|bridge| {
        if !bridge.enabled {
            return false;
        }
        bridge
            .requests
            .push_back(HostOptionsRequest::DrawRmbScrollAnchor { enabled });
        true
    })
}

#[cfg(test)]
static HOST_OPTIONS_BRIDGE_TEST_LOCK: Mutex<()> = Mutex::new(());

/// Serialize tests that mutate the process-global host-options bridge.
#[cfg(test)]
pub(crate) struct HostOptionsBridgeTestGuard {
    _lock: std::sync::MutexGuard<'static, ()>,
}

#[cfg(test)]
pub(crate) fn acquire_host_options_bridge_test_guard() -> HostOptionsBridgeTestGuard {
    let lock = HOST_OPTIONS_BRIDGE_TEST_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let _ = take_host_options_requests();
    set_host_options_bridge_enabled(false);
    HostOptionsBridgeTestGuard { _lock: lock }
}

#[cfg(test)]
impl Drop for HostOptionsBridgeTestGuard {
    fn drop(&mut self) {
        let _ = take_host_options_requests();
        set_host_options_bridge_enabled(false);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn alternate_mouse_is_published_only_while_main_owns_world_input() {
        let _guard = acquire_host_options_bridge_test_guard();

        set_host_options_bridge_enabled(true);
        assert!(publish_host_alternate_mouse(true));
        assert_eq!(
            take_host_options_requests(),
            vec![HostOptionsRequest::AlternateMouse { enabled: true }]
        );

        set_host_options_bridge_enabled(false);
        assert!(!publish_host_alternate_mouse(false));
        assert!(take_host_options_requests().is_empty());
    }
}
