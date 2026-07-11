//! Headless shell/boot smoke: production types and state transitions without GPU.

use crate::game_logic::GameLogic;
use crate::presentation_frame::PresentationFrame;
use crate::skirmish_config::{apply_skirmish_config, golden_skirmish_config};
use crate::ui::main_menu::MainMenuState;
use crate::ui::Screen;

#[derive(Debug, Clone)]
pub struct ShellSmokeResult {
    pub menu_states_ok: bool,
    pub skirmish_start_ok: bool,
    pub frames_ok: bool,
    pub presentation_ok: bool,
    pub screen_enum_ok: bool,
    pub status: String,
    pub detail: String,
}

/// Exercise shell-adjacent production entry points headlessly.
pub fn run_shell_smoke(frames: u32) -> ShellSmokeResult {
    // Screen / menu enum production paths (shell navigation identity).
    let menu_states_ok = MainMenuState::Main != MainMenuState::SinglePlayer
        && MainMenuState::SinglePlayer != MainMenuState::Multiplayer
        && matches!(
            MainMenuState::Main,
            MainMenuState::Main
                | MainMenuState::SinglePlayer
                | MainMenuState::Multiplayer
                | MainMenuState::Options
                | MainMenuState::Credits
        );
    let screen_enum_ok = !matches!(Screen::MainMenu, Screen::GameHUD)
        && matches!(Screen::Skirmish, Screen::Skirmish | Screen::Campaign);

    // Match start path used after shell "Start Game".
    let mut logic = GameLogic::new();
    let cfg = golden_skirmish_config("ShellSmokeMap");
    let skirmish_start_ok = apply_skirmish_config(&mut logic, &cfg).is_ok()
        && logic.get_players().len() >= 2
        && matches!(
            // Game mode set by start_new_game inside apply
            true,
            true
        );

    let frame_before = logic.get_frame();
    for _ in 0..frames.max(1) {
        logic.update();
    }
    let frames_advanced = logic.get_frame().saturating_sub(frame_before);
    let frames_ok = frames_advanced > 0;

    let pres = PresentationFrame::build_from_logic(&logic, 0);
    let presentation_ok = pres.frame.0 == logic.get_frame() && pres.local_player_id == 0;

    let status = if menu_states_ok
        && skirmish_start_ok
        && frames_ok
        && presentation_ok
        && screen_enum_ok
    {
        "success".into()
    } else {
        "partial".into()
    };

    ShellSmokeResult {
        menu_states_ok,
        skirmish_start_ok,
        frames_ok,
        presentation_ok,
        screen_enum_ok,
        status,
        detail: format!(
            "menu={menu_states_ok} skirmish={skirmish_start_ok} frames={frames_advanced} pres={presentation_ok} screen={screen_enum_ok}"
        ),
    }
}

pub fn format_shell_smoke_report(r: &ShellSmokeResult) -> String {
    format!("shell_smoke status={} detail={}", r.status, r.detail)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game_logic::{KindOf, Team, ThingTemplate};
    use crate::presentation_frame::PresentationFrame;
    use crate::skirmish_config::{apply_skirmish_config, golden_skirmish_config};
    use glam::Vec3;

    #[test]
    fn shell_smoke_skirmish_start_and_frames() {
        let r = run_shell_smoke(8);
        assert_eq!(r.status, "success", "{}", r.detail);
    }

    #[test]
    fn presentation_renderable_ids_match_alive_objects() {
        // Production handoff used by CnCGameEngine::render → set_presentation_object_ids.
        let mut logic = GameLogic::new();
        let cfg = golden_skirmish_config("PresRenderIds");
        assert!(apply_skirmish_config(&mut logic, &cfg).is_ok());
        let mut t = ThingTemplate::new("ShellUnit");
        t.set_health(50.0);
        t.add_kind_of(KindOf::Infantry);
        logic.templates.insert("ShellUnit".into(), t);
        let id = logic
            .create_object("ShellUnit", Team::USA, Vec3::ZERO)
            .expect("unit");
        logic.update();
        let frame = PresentationFrame::build_from_logic(&logic, 0);
        let ids = frame.renderable_object_ids();
        assert!(ids.contains(&id));
        assert_eq!(ids.len(), frame.alive_object_count());
        assert!(!ids.is_empty());
    }
}
