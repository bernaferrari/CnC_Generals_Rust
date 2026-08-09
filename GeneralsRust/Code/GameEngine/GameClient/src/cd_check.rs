//! CD presence checks used by MainMenu / Skirmish start.
//!
//! C++ sources:
//! - `GameClient/CDCheck.h` (`IsFirstCDPresent`, `CheckForCDAtGameStart`)
//! - Implementation in `SkirmishGameOptionsMenu.cpp`
//! - Retail body calls `TheFileSystem->areMusicFilesOnCD()` (`FileSystem.cpp`)
//!
//! This is **not** launcher copy-protection (`SAFEMISC` / `ProtectionStatus`).
//! Retail `.big` archives are discovered from cwd/exe/env, not a fixed folder name.

use crate::game_text::GameText;
use crate::gui::callbacks::message_box_ok_cancel;

pub type GameStartCallback = fn();

/// C++ `IsFirstCDPresent`.
///
/// ```c
/// #if !defined(_INTERNAL) && !defined(_DEBUG)
///     return TheFileSystem->areMusicFilesOnCD();
/// #else
///     return TRUE;
/// #endif
/// ```
pub fn is_first_cd_present() -> bool {
    if cfg!(any(debug_assertions, feature = "internal")) {
        return true;
    }
    are_music_files_on_cd()
}

/// C++ `FileSystem::areMusicFilesOnCD` (install dir or CD root holding `genseczh.big`).
pub fn are_music_files_on_cd() -> bool {
    game_engine::common::system::file_system::are_music_files_on_cd()
}

pub fn check_for_cd_at_game_start(callback: GameStartCallback) {
    if is_first_cd_present() {
        callback();
        return;
    }

    // C++ checkCDCallback: re-run IsFirstCDPresent; KEEPOPEN if still missing.
    let _ = message_box_ok_cancel(
        &GameText::fetch("GUI:InsertCDPrompt"),
        &GameText::fetch("GUI:InsertCDMessage"),
        Some(Box::new(move || {
            if is_first_cd_present() {
                callback();
            }
        })),
        Some(Box::new(|| {})),
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn is_first_cd_present_true_in_debug_or_internal_like_cpp() {
        // cargo test is debug; GameClient `--features internal` is _INTERNAL.
        assert!(
            is_first_cd_present(),
            "C++ IsFirstCDPresent is TRUE for _DEBUG/_INTERNAL"
        );
    }

    #[test]
    fn are_music_files_on_cd_does_not_use_copy_protection() {
        let production = include_str!("cd_check.rs")
            .split("#[cfg(test)]")
            .next()
            .unwrap_or("");
        assert!(
            !production.contains("copy_protection"),
            "IsFirstCDPresent must not call launcher copy-protection"
        );
        assert!(
            production.contains("are_music_files_on_cd"),
            "retail path must use FileSystem::areMusicFilesOnCD"
        );
    }

    #[test]
    fn retail_cd_check_finds_discovered_genseczh_big() {
        assert!(
            are_music_files_on_cd(),
            "areMusicFilesOnCD must find genseczh.big via install/CD discovery"
        );
    }
}
