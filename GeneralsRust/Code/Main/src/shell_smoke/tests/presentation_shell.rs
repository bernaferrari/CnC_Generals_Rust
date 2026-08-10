//! Presentation shell deepen / bounds / local-team residual tests.

pub use super::*;

#[cfg(test)]
mod presentation_shell_deepen_tests {
    #[test]
    fn presentation_shell_deepens_visual_speed_without_main_draw_ownership() {
        let src = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/../GameEngine/GameClient/src/core/game_client.rs"));
        let idx = src
            .find("fn update_presentation_shell")
            .expect("presentation shell");
        let window = &src[idx..idx + 2800];
        assert!(
            window.contains("get_script_visual_speed_multiplier"),
            "shell must scale visual delta by script visual speed"
        );
        assert!(
            window.contains("should_freeze_visual_time"),
            "shell must honor visual freeze residual"
        );
        assert!(
            window.contains("update_display_string_manager"),
            "shell must tick DisplayStringManager residual"
        );
        assert!(
            window.contains("update_display_only"),
            "shell must run display UPDATE residual (not DRAW)"
        );
        assert!(
            window.contains("draw_drawable_icon_ui"),
            "shell must run drawable icon UI residual"
        );
        assert!(
            !window.contains("self.update_input")
                && !window.contains("self.update_audio")
                && !window.contains("self.draw_display"),
            "presentation shell must not take Main input/audio/draw ownership"
        );
        assert!(
            window.contains("update_drawables_local"),
            "shell keeps local drawable path (no OBJECT_REGISTRY shroud bind)"
        );
    }
}

#[cfg(test)]
mod presentation_mouse_bounds_tests {
    #[test]
    fn mouse_world_position_prefers_presentation_bounds() {
        let eng = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/cnc_game_engine.rs"));
        let idx = eng
            .find("fn update_mouse_world_position")
            .expect("update_mouse_world_position");
        let window = &eng[idx..idx + 900];
        assert!(
            window.contains("last_presentation_frame") && window.contains("world_bounds_vec3"),
            "mouse map must prefer presentation world_env bounds"
        );
        assert!(
            window.contains("game_logic.world_bounds()"),
            "boot residual without frame may still use host bounds"
        );
    }
}

#[cfg(test)]
mod presentation_camera_bounds_tests {
    #[test]
    fn clamp_to_world_bounds_prefers_presentation() {
        let eng = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/cnc_game_engine.rs"));
        let idx = eng
            .find("fn clamp_to_world_bounds")
            .expect("clamp_to_world_bounds");
        let window = &eng[idx..idx + 700];
        assert!(
            window.contains("last_presentation_frame") && window.contains("world_bounds_vec3"),
            "camera clamp must prefer presentation world_env bounds"
        );
        assert!(
            window.contains("game_logic.world_bounds()"),
            "boot residual without frame may still use host bounds"
        );
    }
}

#[cfg(test)]
mod presentation_minimap_bounds_tests {
    #[test]
    fn minimap_viewport_prefers_presentation_bounds() {
        let eng = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/cnc_game_engine.rs"));
        let idx = eng
            .find("fn update_minimap_viewport")
            .expect("update_minimap_viewport");
        let window = &eng[idx..idx + 700];
        assert!(
            window.contains("last_presentation_frame") && window.contains("world_bounds_vec3"),
            "minimap viewport must prefer presentation world_env bounds"
        );
        assert!(
            window.contains("game_logic.world_bounds()"),
            "boot residual without frame may still use host bounds"
        );
        // Radar pings also prefer presentation bounds near the UI overlay path.
        let radar_idx = eng.find("update_radar_pings").expect("update_radar_pings");
        let radar_window = &eng[radar_idx.saturating_sub(350)..radar_idx + 80];
        assert!(
            radar_window.contains("last_presentation_frame")
                && radar_window.contains("world_bounds_vec3"),
            "radar pings must prefer presentation world_env bounds"
        );
    }
}

#[cfg(test)]
mod presentation_local_team_tests {
    #[test]
    fn selection_hotkeys_prefer_presentation_local_team() {
        let eng = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/cnc_game_engine.rs"));
        // Selection hotkeys / pick residual prefer presentation local_team when dual-scanning.
        // Right-click context path is command-system residual via current_player_id.
        for needle in [
            "Retail SELECT_ALL (KEY_Q) / Ctrl+A residual",
            "fn select_all_friendly_units",
            "fn find_object_at_position",
        ] {
            let idx = eng
                .find(needle)
                .unwrap_or_else(|| panic!("missing {needle}"));
            let window = &eng[idx..eng.len().min(idx + 2500)];
            assert!(
                window.contains("local_team")
                    || window.contains("local_team()")
                    || window.contains("pick_object_id_at_world_from_presentation")
                    || window.contains("Boot residual")
                    || window.contains("Presentation-only"),
                "{needle} must prefer presentation local_team / presentation pick"
            );
            if needle == "fn find_object_at_position" {
                // Bound to this method only (next fn starts pathfollowing stub).
                let end = window
                    .find("fn update_unit_pathfinding")
                    .unwrap_or(window.len());
                let body = &window[..end];
                assert!(
                    body.contains("Presentation-only")
                        && body.contains("pick_object_id_at_world_from_presentation")
                        && !body.contains("game_logic.get_objects()"),
                    "engine find_object_at_position must be presentation-only"
                );
            }
        }
        assert!(
            eng.contains("fn handle_right_click")
                && eng.contains("process_mouse_input")
                && eng.contains("current_player_id"),
            "right-click must route context commands via current_player selection residual"
        );
        let pf = crate::presentation_frame::PRESENTATION_FRAME_SRC;
        assert!(
            pf.contains("pub local_team: Team"),
            "PresentationFrame must freeze local_team"
        );
    }
}

#[cfg(test)]
mod presentation_select_similar_tests {
    #[test]
    fn select_similar_units_prefers_presentation_local_team() {
        let eng = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/cnc_game_engine.rs"));
        let idx = eng
            .find("fn select_similar_units")
            .expect("select_similar_units");
        let window = &eng[idx..idx + 900];
        assert!(
            window.contains("local_team") || window.contains("local_team()"),
            "select_similar_units must prefer presentation local_team"
        );
        assert!(
            window.contains("similar_unit_ids"),
            "select_similar_units must use presentation similar_unit_ids when frame set"
        );
        // Presentation-only: no live get_player dual-read in this path.
        assert!(
            window.contains("last_presentation_frame") || window.contains("Presentation-only"),
            "select_similar_units must be presentation-frame gated"
        );
        assert!(
            !window.contains("game_logic.get_player"),
            "select_similar_units must not dual-read live get_player"
        );
    }
}

#[cfg(test)]
mod presentation_player_roster_tests {
    #[test]
    fn defeat_ui_prefers_presentation_player_roster() {
        let eng = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/cnc_game_engine.rs"));
        let idx = eng
            .find("Broadcast defeat notifications")
            .expect("defeat notifications");
        let window = &eng[idx..idx + 1600];
        assert!(
            window.contains("player_info(player_id)") || window.contains("player_info("),
            "defeat UI must prefer presentation player roster"
        );
        // Live get_player only as residual after presentation roster miss.
        assert!(
            window.contains("get_player") || window.contains("player_info"),
            "defeat UI must use presentation roster and/or residual get_player"
        );
        let alliance_idx = eng
            .find("Prefer presentation roster team when installed")
            .expect("alliance roster prefer");
        let alliance_window = &eng[alliance_idx..alliance_idx + 500];
        assert!(
            alliance_window.contains("player_team("),
            "alliance radar must prefer presentation player_team"
        );
        let pf = crate::presentation_frame::PRESENTATION_FRAME_SRC;
        assert!(
            pf.contains("pub struct PresentationPlayerInfo")
                && pf.contains("pub players: Vec<PresentationPlayerInfo>"),
            "PresentationFrame must freeze players roster"
        );
    }
}
