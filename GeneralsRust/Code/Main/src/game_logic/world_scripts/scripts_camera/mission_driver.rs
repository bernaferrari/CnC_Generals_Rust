//! Mission actions borrow the driving match and its client services.

use super::HostScriptExecutionDriver;
use crate::game_logic::{
    ScriptPopupMessageRequest,
    mission_scripts::{
        CameoFlashRequest, NamedTimerMutation, SetFpsLimitRequest, VisualSpeedMultiplierRequest,
    },
};
use generals_sim::scripting::mission_actions::{ScriptMissionWorld, ScriptUiRequest};
use generals_sim::{GameLogicError, GameLogicResult};

impl HostScriptExecutionDriver<'_> {
    fn set_cinematic_input(&mut self, enabled: bool) -> GameLogicResult<()> {
        if let Some(ui) = self.ui.ui.as_deref_mut() {
            if !ui.has_cinematic_input() {
                return Err(GameLogicError::Configuration(
                    "native presentation lacks receiving cinematic input capability".into(),
                ));
            }
            let was_enabled = self.world.script_input_enabled();
            self.world.set_script_input_enabled(enabled);
            let world = &mut self.world;
            return ui.cinematic_input(enabled, was_enabled, &mut |player| {
                world.select_objects_no_sound(player, Vec::new());
            });
        }
        if self.ui.view.is_some() {
            return Err(GameLogicError::Configuration(
                "native view operation cannot fall back to ambient cinematic input".into(),
            ));
        }
        self.world.set_script_input_enabled(enabled);
        Ok(())
    }
}

impl ScriptMissionWorld for HostScriptExecutionDriver<'_> {
    fn apply_script_ui(&mut self, request: ScriptUiRequest<'_>) -> GameLogicResult<()> {
        use ScriptUiRequest as Request;
        let hooks = &self.world.mission_scripts;
        match request {
            Request::Input(enabled) => return self.set_cinematic_input(enabled),
            Request::Freeze(frozen) => hooks.push_script_freeze_time(frozen),
            Request::Weather(visible) => hooks.push_weather_visible(visible),
            Request::VisualSpeed(multiplier) => {
                hooks.push_visual_speed_multiplier(VisualSpeedMultiplierRequest { multiplier });
            }
            Request::FpsLimit(fps) => hooks.push_set_fps_limit(SetFpsLimitRequest { fps }),
            Request::CameoFlash { button, flashes } => hooks.push_cameo_flash(CameoFlashRequest {
                command_button_name: button.into(),
                flash_count: flashes.max(0),
            }),
            Request::AddTimer {
                name,
                text,
                countdown,
            } => {
                hooks.push_named_timer_mutation(NamedTimerMutation::Add {
                    name: name.into(),
                    text: text.into(),
                    countdown,
                });
            }
            Request::RemoveTimer(name) => {
                hooks.push_named_timer_mutation(NamedTimerMutation::Remove { name: name.into() });
            }
            Request::ShowTimers(show) => hooks.push_named_timer_display(show),
            Request::ShowSpecialPowers(show) => hooks.push_superweapon_display_enabled(show),
            Request::Popup {
                message,
                x,
                y,
                width,
                pause,
            } => {
                hooks.push_popup_message(ScriptPopupMessageRequest {
                    message: message.into(),
                    x_percent: x,
                    y_percent: y,
                    width,
                    pause,
                    pause_music: false,
                    popup_generation: 0,
                });
            }
        }
        Ok(())
    }

    fn local_player_is_observer(&self) -> bool {
        self.world
            .local_player_id()
            .and_then(|id| self.world.get_player(id))
            .is_some_and(|player| player.is_observer)
    }

    fn destroy_win_lose_window(&mut self) -> GameLogicResult<()> {
        self.ui
            .ui
            .as_deref_mut()
            .map_or(Ok(()), |ui| ui.destroy_win_lose_window())
    }

    fn close_game_windows(&mut self) -> GameLogicResult<()> {
        self.ui
            .ui
            .as_deref_mut()
            .map_or(Ok(()), |ui| ui.close_game_windows())
    }

    fn create_win_lose_window(&mut self, layout: &str) -> GameLogicResult<()> {
        self.ui
            .ui
            .as_deref_mut()
            .map_or(Ok(()), |ui| ui.create_win_lose_window(layout))
    }

    fn set_campaign_victorious(&mut self, victorious: bool) -> GameLogicResult<()> {
        match self.ui.ui.as_deref_mut() {
            Some(ui) => ui.set_campaign_victorious(victorious),
            None if self.world.has_campaign_session() => Err(GameLogicError::Configuration(
                "campaign victory requires the receiving application's UI owner".into(),
            )),
            None => Ok(()),
        }
    }
}
