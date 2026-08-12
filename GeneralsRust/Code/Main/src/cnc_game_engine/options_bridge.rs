//! Delivery of camera-affecting OptionsMenu preferences into Main.
//!
//! The retail OptionsMenu WND remains in GameClient.  Main is the sole camera
//! input authority for the offline world, so it consumes only the typed
//! preference that changes C++ LookAtXlat's RMB drag-anchor behaviour.

#![allow(unused_imports, unused_variables, dead_code, non_snake_case)]

use super::*;
#[cfg(feature = "game_client")]
use game_client::gui::options_host_bridge::{take_host_options_requests, HostOptionsRequest};

/// C++ `LookAtXlat.cpp` moves an enabled RMB drag anchor only once the cursor
/// exceeds half the display extent from it.  Keeping this pure makes the
/// camera rule independently testable without a window/GPU fixture.
#[inline]
pub(super) fn clamp_move_rmb_scroll_anchor(
    anchor: &mut (f32, f32),
    current: (f32, f32),
    display_size: (f32, f32),
    move_anchor: bool,
) {
    if !move_anchor {
        return;
    }

    let max_x = (display_size.0.max(1.0)) * 0.5;
    let max_y = (display_size.1.max(1.0)) * 0.5;
    if current.0 + max_x < anchor.0 {
        anchor.0 = current.0 + max_x;
    } else if current.0 - max_x > anchor.0 {
        anchor.0 = current.0 - max_x;
    }
    if current.1 + max_y < anchor.1 {
        anchor.1 = current.1 + max_y;
    } else if current.1 - max_y > anchor.1 {
        anchor.1 = current.1 - max_y;
    }
}

#[cfg(feature = "game_client")]
impl CnCGameEngine {
    /// Apply OptionsMenu updates without routing them through legacy GameLogic.
    ///
    /// Preferences are intentionally accepted in Menu as well as an active
    /// match: a player may configure them before starting a game, and they
    /// remain process/UI preferences rather than savegame state.
    pub(crate) fn host_tick_options_bridge(&mut self) {
        for request in take_host_options_requests() {
            match request {
                HostOptionsRequest::MoveRmbScrollAnchor { enabled } => {
                    self.move_rmb_scroll_anchor = enabled;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn disabled_move_rmb_scroll_anchor_keeps_the_press_anchor_fixed() {
        let mut anchor = (10.0, 990.0);
        clamp_move_rmb_scroll_anchor(&mut anchor, (1000.0, 0.0), (400.0, 300.0), false);
        assert_eq!(anchor, (10.0, 990.0));
    }

    #[test]
    fn enabled_move_rmb_scroll_anchor_clamps_both_axes_like_look_at_xlat() {
        let mut anchor = (10.0, 990.0);
        clamp_move_rmb_scroll_anchor(&mut anchor, (1000.0, 0.0), (400.0, 300.0), true);
        assert_eq!(anchor, (800.0, 150.0));
    }
}
