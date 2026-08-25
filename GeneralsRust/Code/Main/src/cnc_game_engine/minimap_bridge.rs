//! Delivery of the live `ControlBar.wnd:LeftHUD` minimap into Main's world.
//!
//! The retail WND callback belongs to GameClient, while the executable owns
//! the Rust simulation and WGPU minimap/FOW state.  This bridge carries only
//! the typed click geometry and publication-time input provenance across that
//! boundary; Main remains the sole place that converts it to a world action.

#![allow(unused_imports, unused_variables, dead_code, non_snake_case)]

use super::*;
use game_client::gui::control_bar::{
    HostMinimapInteraction, HostMinimapMouseButton, take_host_minimap_interactions,
};

/// Mirror C++ `LeftHUDInput`'s action choice before Main does any world work.
///
/// With the normal mouse mapping, a selected unit uses LMB for the minimap
/// order and RMB for camera movement.  Alternate mouse reverses those two.
/// An empty selection always pans.  `MinimapActionKind` describes the action
/// consumed by Main's existing handler, not the original physical button.
fn host_minimap_action_kind(
    button: HostMinimapMouseButton,
    alternate_mouse: bool,
    selection_empty: bool,
) -> MinimapActionKind {
    let camera_pan = selection_empty
        || (!alternate_mouse && matches!(button, HostMinimapMouseButton::Right))
        || (alternate_mouse && matches!(button, HostMinimapMouseButton::Left));
    if camera_pan {
        MinimapActionKind::LeftClick
    } else {
        MinimapActionKind::RightClick
    }
}

/// Revalidate WND geometry at the authority boundary before it is passed to
/// the WGPU minimap mapping.  The WND callback already performs this check,
/// but the queued request is deliberately treated as untrusted input here.
fn host_minimap_interaction_is_inside_window(interaction: &HostMinimapInteraction) -> bool {
    let [mouse_x, mouse_y] = interaction.screen_position.map(i64::from);
    let [left, top] = interaction.screen_top_left.map(i64::from);
    let [width, height] = interaction.screen_size.map(i64::from);
    width > 0
        && height > 0
        && mouse_x >= left
        && mouse_y >= top
        && mouse_x < left + width
        && mouse_y < top + height
}

impl CnCGameEngine {
    /// Drain real ControlBar LeftHUD interactions into the authoritative Rust
    /// world after synchronous WND dispatch has unwound.
    ///
    /// This intentionally permits injected inputs to follow the same gameplay
    /// path as physical inputs, exactly like ordinary engine input.  Their
    /// explicit `InjectedOrUnknown` provenance is retained and no playable
    /// evidence latch is derived from this bridge.
    pub(crate) fn host_tick_minimap_bridge(&mut self) {
        let interactions = take_host_minimap_interactions();
        if interactions.is_empty() {
            return;
        }

        if !matches!(self.current_state, GameState::InGame)
            || !matches!(
                self.host_match_game_mode,
                Some(
                    crate::game_logic::GameMode::SinglePlayer
                        | crate::game_logic::GameMode::Skirmish
                )
            )
        {
            debug!(
                "discarded {} LeftHUD minimap interactions outside an in-game offline world",
                interactions.len()
            );
            return;
        }

        // Do not revive GameClient's separate radar/GameLogic state here. The
        // frozen presentation frame is the source for both the HUD radar
        // availability and the WGPU minimap that validates FOW below.
        let radar_enabled = self
            .render_pipeline
            .presentation_frame()
            .or(self.last_presentation_frame.as_ref())
            .is_some_and(|frame| frame.radar_ui_enabled);
        if !radar_enabled {
            debug!(
                "discarded {} LeftHUD minimap interactions while radar is unavailable",
                interactions.len()
            );
            return;
        }

        for interaction in interactions {
            self.host_apply_minimap_interaction(interaction);
        }
    }

    fn host_apply_minimap_interaction(&mut self, interaction: HostMinimapInteraction) {
        if !host_minimap_interaction_is_inside_window(&interaction) {
            debug!("discarded LeftHUD minimap interaction outside its WND rectangle");
            return;
        }

        let [left, top] = interaction.screen_top_left;
        let [width, height] = interaction.screen_size;
        self.render_pipeline.update_minimap_screen_rect(
            Vec2::new(left as f32, top as f32),
            Vec2::new(width as f32, height as f32),
        );

        let selection_empty = self.ui_selected_ids(self.current_player_id).is_empty();
        let kind = host_minimap_action_kind(
            interaction.button,
            interaction.alternate_mouse,
            selection_empty,
        );
        let [mouse_x, mouse_y] = interaction.screen_position;
        let physical_os_input = interaction
            .input_provenance
            .is_physical_window_mouse_input();
        debug!(
            "applying LeftHUD minimap interaction through Main (physical_window_input={physical_os_input})"
        );

        // `handle_minimap_interaction` uses RenderPipeline::handle_minimap_click,
        // which rejects coordinates outside the stamped rectangle and FOW cells
        // that are neither explored nor visible before any camera/order action.
        self.handle_minimap_interaction(MinimapInteraction {
            screen_position: crate::ui::UiPos2::new(mouse_x as f32, mouse_y as f32),
            kind,
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn left_hud_mouse_policy_matches_retail_normal_and_alternate_modes() {
        assert_eq!(
            host_minimap_action_kind(HostMinimapMouseButton::Left, false, false),
            MinimapActionKind::RightClick,
            "normal selected LMB issues the minimap order"
        );
        assert_eq!(
            host_minimap_action_kind(HostMinimapMouseButton::Right, false, false),
            MinimapActionKind::LeftClick,
            "normal selected RMB pans the camera"
        );
        assert_eq!(
            host_minimap_action_kind(HostMinimapMouseButton::Left, true, false),
            MinimapActionKind::LeftClick,
            "alternate selected LMB pans the camera"
        );
        assert_eq!(
            host_minimap_action_kind(HostMinimapMouseButton::Right, true, false),
            MinimapActionKind::RightClick,
            "alternate selected RMB issues the minimap order"
        );
        assert_eq!(
            host_minimap_action_kind(HostMinimapMouseButton::Left, false, true),
            MinimapActionKind::LeftClick,
            "an empty selection always pans"
        );
    }

    #[test]
    fn minimap_bridge_rejects_geometry_outside_the_live_left_hud() {
        let interaction = HostMinimapInteraction {
            screen_position: [173, 594],
            screen_top_left: [7, 443],
            screen_size: [167, 152],
            button: HostMinimapMouseButton::Left,
            alternate_mouse: false,
            input_provenance:
                game_client::gui::control_bar::HostControlBarInputProvenance::InjectedOrUnknown,
        };
        assert!(host_minimap_interaction_is_inside_window(&interaction));

        let outside = HostMinimapInteraction {
            screen_position: [174, 595],
            ..interaction
        };
        assert!(
            !host_minimap_interaction_is_inside_window(&outside),
            "the right/bottom WND edges are exclusive"
        );
    }
}
