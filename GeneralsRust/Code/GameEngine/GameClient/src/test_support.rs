//! Owned fixtures for unit tests. No process-wide UI / window manager state.

use crate::core::subsystems::InGameUISubsystem;
use crate::message_stream::ReceivingUi;

/// Run `f` with a stack-owned InGameUISubsystem exposed as ReceivingUi.
pub fn with_receiving_ui<R>(f: impl FnOnce(&mut ReceivingUi<'_>) -> R) -> R {
    let mut state = InGameUISubsystem::default();
    let mut ui = ReceivingUi::new(&mut state, 0, 0);
    f(&mut ui)
}

/// Same as [`with_receiving_ui`], with explicit logic/client frames.
pub fn with_receiving_ui_at<R>(
    logic_frame: u32,
    client_frame: u32,
    f: impl FnOnce(&mut ReceivingUi<'_>) -> R,
) -> R {
    let mut state = InGameUISubsystem::default();
    let mut ui = ReceivingUi::new(&mut state, logic_frame, client_frame);
    f(&mut ui)
}
