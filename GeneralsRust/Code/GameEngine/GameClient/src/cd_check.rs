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

pub type GameStartCallback =
    Box<dyn for<'a> FnMut(&mut crate::gui::game_window::WindowEventEffects<'a>)>;

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

pub fn check_for_cd_at_game_start(
    mut callback: GameStartCallback,
    effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
) {
    if is_first_cd_present() {
        callback(effects);
        return;
    }

    // C++ checkCDCallback: re-run IsFirstCDPresent; KEEPOPEN if still missing.
    let _ = message_box_ok_cancel(
        &GameText::fetch("GUI:InsertCDPrompt"),
        &GameText::fetch("GUI:InsertCDMessage"),
        Some(Box::new(move |effects| {
            if is_first_cd_present() {
                callback(effects);
            }
        })),
        Some(Box::new(|_| {})),
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
    fn retail_cd_check_finds_discovered_genseczh_big() {
        assert!(
            are_music_files_on_cd(),
            "areMusicFilesOnCD must find genseczh.big via install/CD discovery"
        );
    }
}
