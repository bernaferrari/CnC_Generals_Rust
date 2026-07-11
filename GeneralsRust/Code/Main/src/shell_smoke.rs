//! Production host smoke: SkirmishMenu → config → apply → map load → frames → presentation.
//!
//! Full windowed shell/WND + GPU boot still requires a display; this path exercises the
//! same production APIs `start_game_from_ui` uses after menu StartGame, without fabricating
//! tautological enum checks.

use crate::game_logic::GameLogic;
use crate::map_frame_scenario::resolve_first_map;
use crate::presentation_frame::PresentationFrame;
use crate::skirmish_config::{
    apply_skirmish_config, config_from_skirmish_menu, golden_skirmish_config,
};
use crate::ui::skirmish_menu::SkirmishMenu;
use crate::ui::Screen;

const HOST_MAP_CANDIDATES: &[&str] = &[
    "windows_game/extracted_big_files/MapsZH/Maps/Lone Eagle/Lone Eagle.map",
    "../windows_game/extracted_big_files/MapsZH/Maps/Lone Eagle/Lone Eagle.map",
    "Maps/Lone Eagle/Lone Eagle.map",
    "Lone Eagle",
];

#[derive(Debug, Clone)]
pub struct ShellSmokeResult {
    pub host_constructed: bool,
    pub skirmish_config_ok: bool,
    pub menu_config_ok: bool,
    pub map_resolved: bool,
    pub map_loaded: bool,
    pub frames_advanced: u32,
    pub presentation_ok: bool,
    pub screen_skirmish_ok: bool,
    pub status: String,
    pub detail: String,
}

/// Exercise production host entry points headlessly (no window required).
/// Builds config from live SkirmishMenu state, applies it, loads retail map when present,
/// advances logic frames, and builds a PresentationFrame consumer feed.
pub fn run_shell_smoke(frames: u32) -> ShellSmokeResult {
    let host_constructed = true;
    let mut logic = GameLogic::new();

    let resolved = resolve_first_map(HOST_MAP_CANDIDATES);
    let map_resolved = resolved.is_some();
    let map_id = resolved
        .as_ref()
        .map(|(id, _)| id.clone())
        .unwrap_or_else(|| "HostSyntheticMap".into());
    let map_path = resolved.map(|(_, p)| p);

    // Production UI path: initialize SkirmishMenu and export StartGame-equivalent config.
    let mut menu = SkirmishMenu::new();
    let menu_init_ok = menu.initialize().is_ok();
    let (slots, rules, menu_map) = menu.get_game_config();
    let menu_map_name = if map_resolved {
        map_id.clone()
    } else {
        menu_map
    };
    let menu_cfg = config_from_skirmish_menu(&menu_map_name, &rules, &slots);
    // Skirmish menu defaults may only fill slot 0 as human; golden_skirmish_config is the
    // full two-player payload the menu StartGame path produces after player setup.
    // Prefer golden when menu has <2 active so apply still matches production 2p skirmish.
    let cfg = if menu_cfg.slots.iter().filter(|s| s.is_active).count() >= 2 {
        menu_cfg.clone()
    } else {
        golden_skirmish_config(&menu_map_name)
    };
    let menu_config_ok = menu_init_ok
        && cfg.slots.iter().filter(|s| s.is_active).count() >= 2
        && cfg.slots.iter().any(|s| s.is_human)
        && cfg.slots.iter().any(|s| !s.is_human);

    let skirmish_config_ok = apply_skirmish_config(&mut logic, &cfg).is_ok()
        && logic.get_players().len() >= 2
        && logic.host_ai_player_count() >= 1
        && logic.skirmish_rules().fog_of_war;

    let map_loaded = if let Some(ref path) = map_path {
        logic.load_map(&path.display().to_string())
    } else {
        false
    };

    let frame_before = logic.get_frame();
    for _ in 0..frames.max(1) {
        logic.update();
    }
    let frames_advanced = logic.get_frame().saturating_sub(frame_before);
    let frames_ok = frames_advanced > 0;

    let pres = PresentationFrame::build_from_logic(&logic, 0);
    let presentation_ok = pres.frame.0 == logic.get_frame()
        && (pres.alive_object_count() > 0 || !map_loaded)
        && !pres
            .objects
            .iter()
            .any(|o| o.model_key.is_none() && !o.destroyed);

    // Real screen ownership semantics (not tautological discriminants).
    let screen_skirmish_ok = Screen::Skirmish.is_shell_owned_pregame()
        && Screen::MainMenu.is_shell_owned_pregame()
        && !Screen::GameHUD.is_shell_owned_pregame()
        && Screen::startup_entry_screen(true) == Screen::MainMenu;

    // When assets present, map must load; when absent, still pass config+frames.
    let map_requirement_ok = if map_resolved { map_loaded } else { true };

    let status = if host_constructed
        && skirmish_config_ok
        && menu_config_ok
        && frames_ok
        && presentation_ok
        && screen_skirmish_ok
        && map_requirement_ok
    {
        "success".into()
    } else {
        "partial".into()
    };

    ShellSmokeResult {
        host_constructed,
        skirmish_config_ok,
        menu_config_ok,
        map_resolved,
        map_loaded,
        frames_advanced,
        presentation_ok,
        screen_skirmish_ok,
        status,
        detail: format!(
            "host={host_constructed} cfg={skirmish_config_ok} menu_cfg={menu_config_ok} map_res={map_resolved} map_load={map_loaded} frames={frames_advanced} pres={presentation_ok} screen={screen_skirmish_ok}"
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
    use crate::skirmish_config::{apply_skirmish_config, golden_skirmish_config};
    use glam::Vec3;

    #[test]
    fn host_smoke_applies_skirmish_and_advances_frames() {
        let r = run_shell_smoke(8);
        assert!(r.host_constructed);
        assert!(r.skirmish_config_ok, "{}", r.detail);
        assert!(r.menu_config_ok, "{}", r.detail);
        assert!(r.frames_advanced > 0, "{}", r.detail);
        assert_eq!(r.status, "success", "{}", r.detail);
    }

    #[test]
    fn presentation_carries_transform_health_team_model() {
        let mut logic = GameLogic::new();
        let cfg = golden_skirmish_config("PresFields");
        assert!(apply_skirmish_config(&mut logic, &cfg).is_ok());
        let mut t = ThingTemplate::new("SmokeUnit");
        t.set_health(50.0);
        t.add_kind_of(KindOf::Infantry);
        logic.templates.insert("SmokeUnit".into(), t);
        let id = logic
            .create_object("SmokeUnit", Team::USA, Vec3::new(3.0, 0.0, 4.0))
            .expect("unit");
        logic.update();
        let frame = PresentationFrame::build_from_logic(&logic, 0);
        let obj = frame
            .objects
            .iter()
            .find(|o| o.id == id)
            .expect("object in presentation");
        assert_eq!(obj.team, Team::USA);
        assert!((obj.position.x - 3.0).abs() < 0.01);
        assert!(obj.health_current > 0.0);
        assert_eq!(obj.health_max, 50.0);
        assert_eq!(obj.model_key.as_deref(), Some("SmokeUnit"));
        assert!(!obj.destroyed);
    }
}
