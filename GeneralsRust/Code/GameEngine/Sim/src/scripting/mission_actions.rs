//! ScriptActions.cpp mission endings, cinematic input, and HUD actions.

use super::interpreter::ScriptInterpreter;
use super::program::{ScriptAction, ScriptActionType};
use super::{ScriptActionResult, ScriptError};
use crate::{GameLogicResult, time::LOGIC_HZ};

pub enum ScriptUiRequest<'a> {
    Input(bool),
    Freeze(bool),
    Weather(bool),
    VisualSpeed(i32),
    FpsLimit(i32),
    CameoFlash {
        button: &'a str,
        flashes: i32,
    },
    AddTimer {
        name: &'a str,
        text: &'a str,
        countdown: bool,
    },
    RemoveTimer(&'a str),
    ShowTimers(bool),
    ShowSpecialPowers(bool),
    Popup {
        message: &'a str,
        x: i32,
        y: i32,
        width: i32,
        pause: bool,
    },
}

pub trait ScriptMissionWorld {
    fn apply_script_ui(&mut self, request: ScriptUiRequest<'_>) -> GameLogicResult<()>;
    fn local_player_is_observer(&self) -> bool;
    fn destroy_win_lose_window(&mut self) -> GameLogicResult<()>;
    fn close_game_windows(&mut self) -> GameLogicResult<()>;
    fn create_win_lose_window(&mut self, layout: &str) -> GameLogicResult<()>;
    fn set_campaign_victorious(&mut self, victorious: bool) -> GameLogicResult<()>;
}

#[derive(Clone, Copy)]
enum MissionOutcome {
    Victory,
    QuickVictory,
    Defeat,
    LocalDefeat,
}

impl MissionOutcome {
    fn apply(
        self,
        interpreter: &ScriptInterpreter,
        world: &mut (impl ScriptMissionWorld + ?Sized),
    ) -> GameLogicResult<()> {
        let presentation = interpreter.presentation();
        let local_defeat = matches!(self, Self::LocalDefeat);
        if local_defeat {
            presentation.set_shown_mp_local_defeat_window(true);
        }
        // ScriptActions.cpp:156–252 closes both window owners before changing
        // input or selecting the new layout. No state loan spans a callback.
        presentation.record_window_close(false);
        world.destroy_win_lose_window()?;
        world.close_game_windows()?;
        if !local_defeat {
            world.apply_script_ui(ScriptUiRequest::Input(false))?;
        }

        let layout = match self {
            Self::QuickVictory => None,
            Self::LocalDefeat => {
                (!world.local_player_is_observer()).then_some("Menus/LocalDefeat.wnd")
            }
            Self::Victory | Self::Defeat => {
                let observer = world.local_player_is_observer()
                    || presentation.has_shown_mp_local_defeat_window();
                Some(if observer {
                    "Menus/ObserverQuit.wnd"
                } else if matches!(self, Self::Victory) {
                    "Menus/Victorious.wnd"
                } else {
                    "Menus/Defeat.wnd"
                })
            }
        };
        if let Some(layout) = layout
            && presentation.record_window_open(layout)
        {
            world.create_win_lose_window(layout)?;
        }
        world.set_campaign_victorious(matches!(self, Self::Victory | Self::QuickVictory))?;

        let timers = &mut interpreter.frame_mut().mission_timers;
        match self {
            Self::QuickVictory => timers.start_quick_end_game(),
            Self::LocalDefeat => timers.start_close_window(),
            Self::Victory | Self::Defeat => timers.start_end_game(),
        }
        Ok(())
    }
}

pub fn try_execute_mission_action(
    action: &ScriptAction,
    interpreter: &ScriptInterpreter,
    world: &mut (impl ScriptMissionWorld + ?Sized),
) -> Result<Option<ScriptActionResult>, ScriptError> {
    use ScriptActionType::*;
    use ScriptUiRequest as Request;

    let kind = action.get_action_type();
    let outcome = match kind {
        Victory => Some(MissionOutcome::Victory),
        Quickvictory => Some(MissionOutcome::QuickVictory),
        Defeat => Some(MissionOutcome::Defeat),
        Localdefeat => Some(MissionOutcome::LocalDefeat),
        _ => None,
    };
    if let Some(outcome) = outcome {
        outcome
            .apply(interpreter, world)
            .map_err(|error| ScriptError::ExecutionFailed(error.to_string()))?;
        return Ok(Some(ScriptActionResult::Success));
    }

    let parameter = |slot| {
        action
            .get_parameter(slot)
            .ok_or_else(|| ScriptError::missing_parameter(slot))
    };
    let name = |slot| parameter(slot).map(|value| value.get_string());
    let int = |slot| parameter(slot).map(|value| value.get_int());
    let default_int = |slot, default| {
        action
            .get_parameter(slot)
            .map_or(default, |value| value.get_int())
    };

    let request = match kind {
        DisableInput | EnableInput => Request::Input(kind == EnableInput),
        FreezeTime | UnfreezeTime => {
            let frozen = kind == FreezeTime;
            interpreter.frame_mut().freeze_by_script = frozen;
            Request::Freeze(frozen)
        }
        ShowWeather => Request::Weather(default_int(0, 1) != 0),
        SetVisualSpeedMultiplier => Request::VisualSpeed(int(0)?),
        SetFpsLimit => Request::FpsLimit(int(0)?),
        CameoFlash => {
            let button = name(0)?;
            let frames = LOGIC_HZ as i32 * int(1)?;
            let flashes = frames / (LOGIC_HZ as i32 / 2).max(1);
            Request::CameoFlash {
                button,
                flashes: flashes + i32::from(flashes % 2 == 1),
            }
        }
        DisplayCountdownTimer | DisplayCounter => Request::AddTimer {
            name: name(0)?,
            text: name(1)?,
            countdown: kind == DisplayCountdownTimer,
        },
        HideCountdownTimer | HideCounter => Request::RemoveTimer(name(0)?),
        EnableCountdownTimerDisplay | DisableCountdownTimerDisplay => {
            Request::ShowTimers(kind == EnableCountdownTimerDisplay)
        }
        EnableSpecialPowerDisplay | DisableSpecialPowerDisplay => {
            Request::ShowSpecialPowers(kind == EnableSpecialPowerDisplay)
        }
        IngamePopupMessage => Request::Popup {
            message: name(0)?,
            x: default_int(1, 50),
            y: default_int(2, 50),
            width: default_int(3, 400),
            pause: default_int(4, 0) != 0,
        },
        _ => return Ok(None),
    };
    if let Err(error) = world.apply_script_ui(request) {
        if matches!(kind, EnableInput | DisableInput | IngamePopupMessage) {
            return Err(ScriptError::ExecutionFailed(error.to_string()));
        }
        log::warn!("Script UI action {kind:?} failed: {error}");
    }
    Ok(Some(ScriptActionResult::Success))
}
