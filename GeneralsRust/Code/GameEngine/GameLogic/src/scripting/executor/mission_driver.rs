//! Compatibility callbacks for the shared mission action decoder.

use super::{ScriptAction, ScriptActionDispatcher, ScriptActionResult, ScriptError};
use crate::helpers::{TheGameLogic, TheInGameUI};
use generals_sim::GameLogicResult;
use generals_sim::scripting::driver::ScriptExecutionDriver;
use generals_sim::scripting::mission_actions::{
    ScriptMissionWorld, ScriptUiRequest, try_execute_mission_action,
};
use generals_sim::scripting::world::{ScriptOwnerQuery, ScriptWorldRequest};
use std::sync::atomic::Ordering;

impl ScriptActionDispatcher<'_> {
    pub(super) fn try_execute_legacy_mission_action(
        &self,
        action: &ScriptAction,
        driver: &mut dyn ScriptExecutionDriver,
    ) -> Result<Option<ScriptActionResult>, ScriptError> {
        try_execute_mission_action(
            action,
            self.context.borrowed_engine().interpreter(),
            &mut MissionWorld {
                dispatcher: self,
                driver,
            },
        )
    }
}

struct MissionWorld<'operation, 'engine> {
    dispatcher: &'operation ScriptActionDispatcher<'engine>,
    driver: &'operation mut dyn ScriptExecutionDriver,
}

impl ScriptMissionWorld for MissionWorld<'_, '_> {
    fn apply_script_ui(&mut self, request: ScriptUiRequest<'_>) -> GameLogicResult<()> {
        use ScriptUiRequest as Request;
        let world_request = match request {
            Request::Input(enabled) => Some(ScriptWorldRequest::Input { enabled }),
            Request::Freeze(frozen) => Some(ScriptWorldRequest::Freeze(frozen)),
            Request::Weather(visible) => Some(ScriptWorldRequest::Weather(visible)),
            Request::VisualSpeed(multiplier) => Some(ScriptWorldRequest::VisualSpeed(multiplier)),
            Request::AddTimer {
                name,
                text,
                countdown: true,
            } => Some(ScriptWorldRequest::Timer {
                name,
                text,
                countdown: true,
            }),
            _ => None,
        };
        if let Some(request) = world_request
            && let Some(result) = self.driver.world(request)
        {
            return result;
        }
        if let Request::Input(enabled) = request {
            let world = self
                .dispatcher
                .context
                .borrowed_engine()
                .owning_world_services();
            if let Some(world) = world.as_deref() {
                world
                    .session_flags()
                    .input_enabled
                    .store(enabled, Ordering::Relaxed);
            } else {
                TheGameLogic::set_input_enabled(enabled);
            }
            TheInGameUI::request_cinematic_input_lock(world.as_deref(), !enabled);
            return Ok(());
        }
        if let Request::Popup {
            message,
            x,
            y,
            width,
            pause,
        } = request
            && let Some(result) = self
                .driver
                .popup_message(message, x, y, width, pause, false)
        {
            return result;
        }
        let Some(handler) = self.dispatcher.current_script_action_handler() else {
            return Ok(());
        };
        match request {
            Request::Input(_) => unreachable!("input was processed above"),
            Request::Freeze(true) => handler.freeze_time(),
            Request::Freeze(false) => handler.unfreeze_time(),
            Request::Weather(visible) => handler.set_weather_visible(visible),
            Request::VisualSpeed(multiplier) => handler.set_visual_speed_multiplier(multiplier),
            Request::FpsLimit(fps) => handler.set_fps_limit(fps),
            Request::CameoFlash { button, flashes } => handler.cameo_flash(button, flashes),
            Request::AddTimer {
                name,
                text,
                countdown,
            } => handler.add_named_timer(name, text, countdown),
            Request::RemoveTimer(name) => handler.remove_named_timer(name),
            Request::ShowTimers(show) => handler.show_named_timer_display(show),
            Request::ShowSpecialPowers(show) => {
                handler.set_superweapon_display_enabled_by_script(show)
            }
            Request::Popup {
                message,
                x,
                y,
                width,
                pause,
            } => handler.popup_message(message, x, y, width, pause, false),
        }
    }

    fn local_player_is_observer(&self) -> bool {
        match self.driver.local_player_observer() {
            ScriptOwnerQuery::Present(observer) => observer,
            ScriptOwnerQuery::Missing => false,
            ScriptOwnerQuery::Unavailable => self
                .dispatcher
                .context
                .borrowed_engine()
                .players()
                .read()
                .ok()
                .and_then(|players| players.get_local_player().cloned())
                .and_then(|player| player.read().ok().map(|player| player.is_player_observer()))
                .unwrap_or(false),
        }
    }

    fn destroy_win_lose_window(&mut self) -> GameLogicResult<()> {
        self.dispatcher
            .context
            .borrowed_engine()
            .destroy_recorded_window_with_driver(self.driver)
    }

    fn close_game_windows(&mut self) -> GameLogicResult<()> {
        self.dispatcher
            .context
            .borrowed_engine()
            .close_game_windows_with_driver(self.driver)
    }

    fn create_win_lose_window(&mut self, layout: &str) -> GameLogicResult<()> {
        self.dispatcher
            .context
            .borrowed_engine()
            .create_recorded_window_with_driver(layout, self.driver)
    }

    fn set_campaign_victorious(&mut self, victorious: bool) -> GameLogicResult<()> {
        if let Some(result) = self.driver.set_campaign_victorious(victorious) {
            return result;
        }
        self.dispatcher
            .context
            .borrowed_engine()
            .set_campaign_victorious(victorious);
        Ok(())
    }
}
