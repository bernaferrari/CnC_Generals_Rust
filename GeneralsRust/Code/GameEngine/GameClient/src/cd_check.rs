use crate::game_text::GameText;
use crate::gui::callbacks::message_box_ok_cancel;
use game_engine::common::system::copy_protection::{get_protection_manager, ProtectionStatus};

pub type GameStartCallback = fn();

pub fn is_first_cd_present() -> bool {
    get_protection_manager()
        .map(|mut manager| manager.comprehensive_validation().status == ProtectionStatus::Valid)
        .unwrap_or(true)
}

pub fn check_for_cd_at_game_start(callback: GameStartCallback) {
    if is_first_cd_present() {
        callback();
        return;
    }

    // PARITY_NOTE: the original path used a shell-era message box callback chain.
    // Rust routes through the shared message-box helper used by the existing shell menus.
    let _ = message_box_ok_cancel(
        &GameText::fetch("GUI:InsertCDPrompt"),
        &GameText::fetch("GUI:InsertCDMessage"),
        Some(Box::new(callback)),
        Some(Box::new(|| {})),
    );
}
