//! HUD / multi-consumer selection residual honesty.

#![allow(unused_imports)]

use super::host::HostSession;
use super::imports::*;

pub(super) fn evaluate_selection(host: &HostSession, logic: &GameLogic) -> (bool, bool) {
    let select_id = host.select_id;
    let hud = &host.hud;
    let pres = &host.pres;
    let ui_state = &host.ui_state;
    let rts = &host.rts;
    let command_panel = &host.command_panel;
    let map_loaded = host.map_loaded;
    let skirmish_config_ok = host.skirmish_config_ok;
    // HUD + multi-consumer selection panel health from presentation after dual-tick.
    let (hud_selection_ok, selection_consumers_ok) = if let Some(id) = select_id {
        let infos = hud.selected_unit_infos();
        let snap_infos = pres.selected_unit_display_infos();
        let hud_hit = infos.iter().any(|u| {
            u.object_id == id && u.health_current > 0.0 && u.health_maximum >= u.health_current
        });
        let snap_hit = snap_infos
            .iter()
            .any(|u| u.object_id == id && u.health_current > 0.0);
        let ids_ok = hud.selected_unit_ids().contains(&id);
        let minimap_ok = !pres.hud_minimap_units().is_empty() || !map_loaded;
        let panel = hud.selection_panel();
        let panel_ok =
            panel.visible && panel.has_positive_health() && panel.primary_object_id == Some(id);
        // Optional ControlBar path (headless selection health; not full WND claim).
        #[cfg(feature = "game_client")]
        let control_bar_ok = {
            use game_client::core::game_client::ClientInputAuthority;
            use game_client::gui::callbacks::control_bar_observation::{
                ControlBarCallbackObservation, ControlBarPlayerObservation,
            };

            let id = pres.local_player_id;
            let local_player = logic.get_player(id).map(|player| ControlBarPlayerObservation {
                id,
                active: player.is_alive && !player.is_observer,
                selection_count: pres.selected.len(),
            });
            let local_player_side = logic.player_template_identity(id).and_then(|identity| {
                let template = identity.resolve_in(logic.player_template_definitions.as_ref()?)?;
                Some(template.get_side().to_owned())
            });
            let mut bar = game_client::gui::control_bar::ControlBar::new();
            let mut client = game_client::core::GameClient::new_with_input_authority(
                ClientInputAuthority::NativeHosted,
            )
            .expect("owned headless ControlBar client");
            client.set_in_game_ui_subsystem(
                game_client::core::subsystems::InGameUISubsystem::default(),
            );
            client.with_window_event_effects(None, |effects| {
                effects.admit_control_bar_callback_observation(ControlBarCallbackObservation {
                    game_ending: logic.owned_script_game_ending(),
                    multiplayer: false,
                    local_player,
                    local_player_side,
                    mouse: None,
                });
                effects.with_manager(|manager, effects| {
                    pres.apply_to_control_bar(&mut bar, manager, effects)
                })
            });
            bar.selection_panel_health()
                .map(|(hp, max)| hp > 0.0 && max >= hp)
                .unwrap_or(false)
        };
        // Fail-closed: no ControlBar without game_client — do not tautology-pass.
        #[cfg(not(feature = "game_client"))]
        let control_bar_ok = false;
        let ui_ok = ui_state.selection_panel.has_positive_health()
            && ui_state.selection_panel.primary_object_id == Some(id);
        let rts_ok =
            rts.selection_panel().has_positive_health() && rts.selected_ids().contains(&id);
        let cmd_ok = command_panel.is_visible()
            && command_panel.selection_panel().has_positive_health()
            && command_panel.selected_ids().contains(&id);
        let consumers_ok = ui_ok && rts_ok && cmd_ok && control_bar_ok;
        (
            hud_hit && snap_hit && ids_ok && minimap_ok && panel_ok && control_bar_ok,
            consumers_ok,
        )
    } else {
        // No objects (absent-map synthetic host): still require resource apply path.
        let empty_ok = hud.selected_unit_ids().is_empty()
            && !hud.selection_panel().visible
            && (pres.local_supplies > 0 || skirmish_config_ok);
        let consumers_empty = !ui_state.selection_panel.visible
            && rts.selected_ids().is_empty()
            && !command_panel.is_visible();
        (empty_ok, empty_ok && consumers_empty)
    };
    (hud_selection_ok, selection_consumers_ok)
}
