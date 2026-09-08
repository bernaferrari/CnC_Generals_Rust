//! Presentation shell deepen / bounds / local-team residual tests.

pub use super::*;

#[cfg(test)]
mod presentation_shell_deepen_tests {
    #[test]
    fn presentation_shell_deepens_visual_speed_without_main_draw_ownership() {
        let src = game_client::core::game_client::GAME_CLIENT_SRC;
        let idx = src
            .find("fn update_presentation_shell")
            .expect("presentation shell");
        // Bound to this function's body: the shell tick deepened past the
        // former 2800-char window (visual-speed / freeze / DisplayString
        // residuals moved deeper into the tick), so a fixed window broke.
        let end = src[idx..]
            .find("\n    pub fn ")
            .map(|rel| idx + rel)
            .unwrap_or(src.len());
        let window = &src[idx..end];
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
        let eng = crate::cnc_game_engine::ENGINE_SRC;
        let idx = eng
            .find("fn update_mouse_world_position")
            .expect("update_mouse_world_position");
        // Wave 461 refactor: the inline freeze-first bounds read moved into the
        // shared `presentation_world_bounds` probe; the mouse path now calls it.
        let window = &eng[idx..idx + 1100];
        assert!(
            window.contains("presentation_world_bounds()"),
            "mouse map must prefer presentation world_env bounds via shared probe"
        );
        // Shared probe: freeze-first (pipeline or last frame) world_env bounds.
        let probe_idx = eng
            .find("fn presentation_world_bounds")
            .expect("presentation_world_bounds");
        let probe = &eng[probe_idx..probe_idx + 560];
        assert!(
            probe.contains("last_presentation_frame") && probe.contains("world_bounds_vec3"),
            "shared probe must prefer presentation world_env bounds"
        );
        assert!(
            probe.contains("self.host_world_bounds()")
                && eng.contains("game_logic.world_bounds()"),
            "boot residual without frame may still use host bounds"
        );
    }
}
#[cfg(test)]
mod presentation_camera_bounds_tests {
    #[test]
    fn clamp_to_world_bounds_prefers_presentation() {
        let eng = crate::cnc_game_engine::ENGINE_SRC;
        let idx = eng
            .find("fn clamp_to_world_bounds")
            .expect("clamp_to_world_bounds");
        // Wave 461 refactor: clamp prefers bounds via the shared
        // `presentation_world_bounds` probe (freeze-first, host residual else).
        let window = &eng[idx..idx + 420];
        assert!(
            window.contains("presentation_world_bounds()"),
            "camera clamp must prefer presentation world_env bounds via shared probe"
        );
        let probe_idx = eng
            .find("fn presentation_world_bounds")
            .expect("presentation_world_bounds");
        let probe = &eng[probe_idx..probe_idx + 560];
        assert!(
            probe.contains("last_presentation_frame") && probe.contains("world_bounds_vec3"),
            "shared probe must prefer presentation world_env bounds"
        );
        assert!(
            probe.contains("self.host_world_bounds()")
                && eng.contains("game_logic.world_bounds()"),
            "boot residual without frame may still use host bounds"
        );
    }
}

#[cfg(test)]
mod presentation_minimap_bounds_tests {
    #[test]
    fn minimap_viewport_prefers_presentation_bounds() {
        let eng = crate::cnc_game_engine::ENGINE_SRC;
        let idx = eng
            .find("fn update_minimap_viewport")
            .expect("update_minimap_viewport");
        // Wave 461 refactor: viewport prefers bounds via the shared
        // `presentation_world_bounds` probe (freeze-first, host residual else).
        let window = &eng[idx..idx + 420];
        assert!(
            window.contains("presentation_world_bounds()"),
            "minimap viewport must prefer presentation world_env bounds via shared probe"
        );
        let probe_idx = eng
            .find("fn presentation_world_bounds")
            .expect("presentation_world_bounds");
        let probe = &eng[probe_idx..probe_idx + 560];
        assert!(
            probe.contains("last_presentation_frame") && probe.contains("world_bounds_vec3"),
            "shared probe must prefer presentation world_env bounds"
        );
        assert!(
            probe.contains("self.host_world_bounds()")
                && eng.contains("game_logic.world_bounds()"),
            "boot residual without frame may still use host bounds"
        );
        // Radar pings also prefer presentation bounds near the UI overlay path;
        // host_finalize_render_ui_state probes shared bounds before forwarding.
        let radar_idx = eng
            .find(".update_radar_pings(&ui_state.radar_pings")
            .expect("update_radar_pings");
        let radar_window = &eng[radar_idx.saturating_sub(420)..radar_idx + 80];
        assert!(
            radar_window.contains("presentation_world_bounds")
                && radar_window.contains("Prefer presentation world_env for radar/minimap"),
            "radar pings must prefer presentation world_env bounds"
        );
    }
}

#[cfg(test)]
mod presentation_local_team_tests {
    #[test]
    fn selection_hotkeys_prefer_presentation_local_team() {
        let eng = crate::cnc_game_engine::ENGINE_SRC;
        // Selection hotkeys / pick residual prefer presentation local_team when dual-scanning.
        // Right-click context path is command-system residual via current_player_id.
        for needle in [
            "Retail SELECT_ALL (KEY_Q) residual",
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
                // The pick chain moved: find_object_at_position is now a thin
                // presentation-only delegate to host_find_object_at_position
                // (selection.rs), which picks from the frozen frame via
                // pick_object_id_at_world_from_presentation. Verify both hops.
                let end = window
                    .find("fn find_object_at_cursor")
                    .unwrap_or(window.len());
                let body = &window[..end];
                assert!(
                    body.contains("presentation-only pick")
                        && body.contains("self.host_find_object_at_position(position, command_context)")
                        && !body.contains("game_logic.get_objects()"),
                    "engine find_object_at_position must stay a presentation-only delegate"
                );
                let host_idx = eng
                    .find("fn host_find_object_at_position")
                    .expect("host_find_object_at_position");
                let host_window = &eng[host_idx..host_idx + 1200];
                assert!(
                    host_window.contains("last_presentation_frame")
                        && host_window.contains("local_team()")
                        && host_window
                            .contains("pick_object_id_at_world_from_presentation")
                        && !host_window.contains("game_logic.get_objects()"),
                    "host pick must be presentation-frame-driven with frozen local_team"
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
        let eng = crate::cnc_game_engine::ENGINE_SRC;
        let idx = eng
            .find("fn select_similar_units")
            .expect("select_similar_units");
        // Bound to this method's body: the presentation similar_unit_ids scan
        // sits deeper than the former 900-char fixed window.
        let end = eng[idx..]
            .find("fn select_similar_units_for_double_click")
            .map(|rel| idx + rel)
            .unwrap_or(idx + 2800);
        let window = &eng[idx..end];
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
        let eng = crate::cnc_game_engine::ENGINE_SRC;
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
        // The alliance/radar team residual moved: team now resolves via
        // ui_player_team → ui_player_info, which prefers the presentation
        // roster freeze (frame.player_info) before boot/diplomacy residual.
        let team_idx = eng
            .find("fn host_ui_player_team")
            .expect("host_ui_player_team");
        let team_window = &eng[team_idx..team_idx + 300];
        assert!(
            team_window.contains("ui_player_info("),
            "alliance radar team must resolve via presentation roster probe"
        );
        let roster_idx = eng
            .find("fn host_ui_player_info")
            .expect("host_ui_player_info");
        let roster_window = &eng[roster_idx..roster_idx + 620];
        assert!(
            roster_window.contains("last_presentation_frame")
                && roster_window.contains("frame.player_info("),
            "alliance radar must prefer presentation player_team roster"
        );
        assert!(
            !roster_window.contains("game_logic.get_player"),
            "roster probe must not dual-read live get_player mid-frame"
        );
        let pf = crate::presentation_frame::PRESENTATION_FRAME_SRC;
        assert!(
            pf.contains("pub struct PresentationPlayerInfo")
                && pf.contains("pub players: Vec<PresentationPlayerInfo>"),
            "PresentationFrame must freeze players roster"
        );
    }
}
