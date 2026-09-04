//! Offline ownership bridge for the live C++-parity QuitMenu WND.
//!
//! `QuitMenu.cpp` owns layout creation, button callbacks, and its small client
//! pause flag.  Main owns the only offline simulation, so it adopts the live
//! WND visibility here rather than replacing it with the old Rust PauseMenu.

#![allow(unused_imports, unused_variables, dead_code, non_snake_case)]

use super::*;

impl CnCGameEngine {
    /// Run the retail `MSG_META_OPTIONS -> ToggleQuitMenu()` path for an
    /// offline match.  `true` means a real QuitMenu layout handled the key;
    /// callers may retain the legacy pause fallback only when WND creation was
    /// unavailable.
    pub(super) fn host_toggle_retail_quit_menu(&mut self) -> bool {
        if !matches!(self.current_state, GameState::InGame)
            || !self.host_quit_menu_bridge_is_offline()
        {
            return false;
        }

        match game_client::gui::callbacks::toggle_quit_menu_with_result() {
            // C++ checks the Options and PopupSaveLoad layouts before it
            // toggles QuitMenu.  They did handle Escape, but they must not
            // seize (or release) Main's pause ownership.
            game_client::gui::callbacks::QuitMenuToggleResult::ClosedInterveningLayout => true,
            // C++ discards the meta-options command while these states are
            // active. Do not substitute Main's old PauseMenu there.
            game_client::gui::callbacks::QuitMenuToggleResult::Suppressed => true,
            // The old pause surface is only a damaged-install fallback after
            // C++ actually attempted, but failed to materialise, its WND.
            game_client::gui::callbacks::QuitMenuToggleResult::LayoutUnavailable => false,
            game_client::gui::callbacks::QuitMenuToggleResult::ToggledQuitMenu => {
                // Do NOT re-derive quit_menu_host_active from WND visibility
                // here: on a host toggle that HIDES the menu it would clear
                // ownership before host_tick_quit_menu_bridge can resume the
                // pause this QuitMenu created (same contract as ButtonReturn /
                // Escape hiding the WND). The tick below adopts on show and
                // resumes on hide.
                self.host_tick_quit_menu_bridge();
                true
            }
        }
    }

    /// Keep Main's offline frame gate synchronized with the real WND callback.
    /// This runs after other UI bridges have drained: PopupSaveLoad remains its
    /// own authority, while QuitMenu owns only pause/resume/restart/exit.
    pub(crate) fn host_tick_quit_menu_bridge(&mut self) {
        if !matches!(self.current_state, GameState::InGame)
            || !self.host_quit_menu_bridge_is_offline()
        {
            self.quit_menu_host_active = false;
            return;
        }

        // The QuitMenu can also be opened by its retail ControlBar callback.
        // Adopting any live WND here keeps Main authoritative without making
        // that callback depend on an out-of-band pause command.
        if game_client::gui::callbacks::is_quit_menu_visible() {
            self.quit_menu_host_active = true;
            self.host_set_paused(true);
            return;
        }

        // No live WND and no prior QuitMenu-owned pause: leave independent
        // pause reasons alone (legacy PauseMenu, load/restart transitions).
        if !self.quit_menu_host_active {
            return;
        }

        self.quit_menu_host_active = false;

        // C++ `restartMissionMenu()` destroys QuitMenu and appends MSG_NEW_GAME
        // with gameMode / difficulty / rankPoints (QuitMenu.cpp:211-216).
        // Drain that payload into the host restart so Challenge general and
        // selected difficulty survive.
        if let Some(dispatch) = Self::take_new_game_dispatch_from_common_stream() {
            self.host_set_paused(false);
            self.host_restart_mission_from_dispatch(dispatch);
            return;
        }

        // C++ `exitQuitMenu()` appends MSG_CLEAR_GAME_DATA.  Consume exactly
        // that control message at Main's single-authority boundary, reset the
        // Rust match, then return to the retail shell.  Other stream messages
        // are retained unchanged. Scripted VICTORY/DEFEAT use the same
        // consumer via host_consume_clear_game_data.
        if self.host_consume_clear_game_data() {
            return;
        }

        // ButtonReturn / Escape hiding the WND: resume the same host world
        // without entering Main's separate PauseMenu screen.
        self.host_set_paused(false);
    }

    fn host_quit_menu_bridge_is_offline(&self) -> bool {
        !matches!(
            self.host_match_game_mode,
            Some(
                crate::game_logic::GameMode::Multiplayer
                    | crate::game_logic::GameMode::Internet
                    | crate::game_logic::GameMode::Lan
                    | crate::game_logic::GameMode::Replay
            )
        )
    }

    /// Remove only the ClearGameData request produced by QuitMenu's confirmed
    /// Exit button.  Delegates to the shared InGame consumer used by scripted
    /// end-game timers (ScriptEngine.cpp:5514-5518 MSG_CLEAR_GAME_DATA).
    fn take_quit_menu_clear_game_data_from_common_stream() -> bool {
        Self::take_clear_game_data_from_common_stream()
    }
}
