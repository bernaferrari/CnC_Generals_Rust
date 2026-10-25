//! Synchronous GUI services lent to one standalone Core script operation.
//! The native Main interpreter uses the same driver methods through its own
//! ScriptUiEffects adapter. No window owner is stored in ScriptEngine.
use super::*;
use crate::gui::game_window::WindowEventEffects;
use gamelogic::scripting::engine::{ScriptEngine, ScriptExecutionDriver};

/// C++ GameLogic::closeWindows order, applied to the same receiving tree/bar.
pub fn close_receiving_game_windows(
    manager: &mut crate::gui::window_manager::WindowManager,
    bar: &mut crate::gui::control_bar::ControlBar,
    effects: &mut WindowEventEffects<'_>,
) {
    let _ = crate::gui::callbacks::diplomacy::hide_diplomacy(false, manager, effects);
    let _ = crate::gui::callbacks::diplomacy::reset_diplomacy(manager);
    let _ = crate::gui::callbacks::ingame_callbacks::hide_in_game_chat(false, manager, effects);
    let _ = crate::gui::callbacks::ingame_callbacks::reset_in_game_chat(manager, effects);
    bar.hide_purchase_science(manager);
    bar.hide_special_power_shortcut(manager);
    crate::gui::callbacks::quit_menu::hide_quit_menu(manager, None);
}
struct ReceivingClientScriptDriver<'a, 'effects> {
    effects: WindowEventEffects<'effects>,
    world: &'a mut gamelogic::system::game_logic::GameLogic,
    bar: &'a mut crate::gui::control_bar::ControlBar,
    stream: &'a mut crate::message_stream::message_stream::MessageStream,
    military_ui: Option<&'a Arc<Mutex<InGameUISubsystem>>>,
}
impl ScriptExecutionDriver for ReceivingClientScriptDriver<'_, '_> {
    fn display(
        &mut self,
        request: gamelogic::scripting::engine::ScriptDisplayRequest<'_>,
    ) -> Option<gamelogic::GameLogicResult<()>> {
        match request {
            gamelogic::scripting::engine::ScriptDisplayRequest::MilitaryCaption {
                text,
                duration_ms,
            } => {
                let Some(ui) = self.military_ui else {
                    return Some(Err(gamelogic::GameLogicError::Configuration(
                        "military caption requires receiving InGameUI".into(),
                    )));
                };
                let mut ui = ui.lock().unwrap_or_else(|error| error.into_inner());
                self.effects.with_manager(|manager, _| {
                    ui.push_military_subtitle(text, duration_ms, manager)
                });
                Some(Ok(()))
            }
            _ => None,
        }
    }
    fn speech_play(
        &mut self,
        name: &str,
        allow_overlap: bool,
    ) -> Option<gamelogic::GameLogicResult<()>> {
        let Some(ui) = self.military_ui else {
            return Some(Err(gamelogic::GameLogicError::Configuration(
                "speech subtitles require receiving InGameUI".into(),
            )));
        };
        let mut ui = ui.lock().unwrap_or_else(|error| error.into_inner());
        Some(self.effects.with_manager(|manager, _| {
            crate::core::script_action_handler::GameClientScriptActionHandler::speech_play_receiving(name, allow_overlap, &mut ui, manager)
        }))
    }

    fn popup_message(
        &mut self,
        message: &str,
        x: i32,
        y: i32,
        width: i32,
        pause: bool,
        pause_music: bool,
    ) -> Option<gamelogic::GameLogicResult<()>> {
        let old = crate::gui::callbacks::in_game_popup_message::clear_receiving_popup_message(
            &mut self.effects,
        );
        // clearPopupMessageData destroys the prior WND before resuming logic.
        if let Err(error) = self.after_action() {
            return Some(Err(error));
        }
        if let Some(old) = old.filter(|data| data.pause) {
            self.world.set_game_paused(false, old.pause_music);
        }
        // C++ pauses before WND construction and init callbacks can observe it.
        if pause {
            self.world.set_game_paused(true, pause_music);
        }
        crate::gui::callbacks::in_game_popup_message::show_receiving_popup_message(
            message,
            x,
            y,
            width,
            0xffffffff,
            pause,
            pause_music,
            None,
            &mut self.effects,
        );
        Some(self.after_action())
    }

    fn create_win_lose_window(&mut self, layout: &str) -> Option<gamelogic::GameLogicResult<()>> {
        self.effects.with_manager(|manager, effects| {
            manager.create_script_message_window_with_effects(layout, effects)
        });
        Some(Ok(()))
    }
    fn destroy_win_lose_window(&mut self) -> Option<gamelogic::GameLogicResult<()>> {
        self.effects.with_manager(|manager, effects| {
            manager.destroy_script_message_window_with_effects(effects)
        });
        Some(Ok(()))
    }
    fn close_game_windows(&mut self) -> Option<gamelogic::GameLogicResult<()>> {
        let bar = &mut *self.bar;
        self.effects
            .with_manager(|manager, effects| close_receiving_game_windows(manager, bar, effects));
        Some(Ok(()))
    }
    fn set_campaign_victorious(
        &mut self,
        victorious: bool,
    ) -> Option<gamelogic::GameLogicResult<()>> {
        Some(match self.effects.campaign_manager.as_deref_mut() {
            Some(manager) => {
                manager.set_victorious(victorious);
                Ok(())
            }
            None => Err(gamelogic::GameLogicError::Configuration(
                "script victory requires the receiving CampaignManager".into(),
            )),
        })
    }
    fn after_action(&mut self) -> gamelogic::GameLogicResult<()> {
        if let Some(owner) = self.effects.window_manager_owner {
            owner.complete_event_callbacks(&mut self.effects);
        }
        for message in self.effects.emitted_messages.drain(..) {
            self.stream.append_message_value(message);
        }
        Ok(())
    }
}
impl GameClient {
    /// Paired standalone Core input/callback boundary. Native Main owns its
    /// tagged popup acknowledgements through MenuHostMailbox instead.
    pub fn with_core_window_event_effects<R>(
        &mut self,
        world: &mut gamelogic::system::game_logic::GameLogic,
        operation: impl FnOnce(&mut WindowEventEffects<'_>) -> R,
    ) -> R {
        let (result, old, current) = self.with_window_event_effects(None, |effects| {
            let old =
                crate::gui::callbacks::in_game_popup_message::receiving_popup_message(effects);
            let result = operation(effects);
            if let Some(owner) = effects.window_manager_owner {
                owner.complete_event_callbacks(effects);
            }
            let current =
                crate::gui::callbacks::in_game_popup_message::receiving_popup_message(effects);
            (result, old, current)
        });
        if current.is_none() {
            if let Some(old) = old.filter(|data| data.pause) {
                world.set_game_paused(false, old.pause_music);
            }
        }
        result
    }
    fn with_receiving_core_script_driver<R>(
        &mut self,
        bar: &mut crate::gui::control_bar::ControlBar,
        world: &mut gamelogic::system::game_logic::GameLogic,
        operation: impl FnOnce(&mut dyn ScriptExecutionDriver) -> gamelogic::GameLogicResult<R>,
    ) -> gamelogic::GameLogicResult<R> {
        self.script_display
            .bind(self.subsystem_manager.display.as_ref());
        let mut messages = Vec::new();
        let mut driver = ReceivingClientScriptDriver {
            world,
            effects: WindowEventEffects {
                menus: Some(self.menu_manager.receiving_menus()),
                manager: None,
                ime: Some(&mut self.ime),
                window_manager_owner: Some(&self.window_manager_owner),
                selected_window: None,
                emitted_messages: &mut messages,
                pending_control_bar_clicks: None,
                campaign_manager: Some(&mut self.campaign_manager),
                main_menu: Some(&mut self.main_menu),
                menu_world: None,
                menu_host: Some(&mut self.menu_host),
                script_display: Some(&mut self.script_display),
                window_videos: Some(&mut self.window_videos.challenge),
                pending_main_menu_actions: Vec::new(),
            },
            bar,
            stream: &mut self.message_stream,
            military_ui: self.subsystem_manager.in_game_ui.as_ref(),
        };
        let result = operation(&mut driver);
        let completed = driver.after_action();
        match result {
            Ok(value) => {
                completed?;
                Ok(value)
            }
            Err(error) => Err(error),
        }
    }
    /// The paired Core session root: script actions run in GameLogic's existing
    /// early-script phase and borrow this client's actual presentation services.
    pub fn update_core_logic_with_scripts(
        &mut self,
        logic: &mut gamelogic::system::game_logic::GameLogic,
        scripts: &ScriptEngine,
        bar: &mut crate::gui::control_bar::ControlBar,
        frame: u32,
        observe_camera: impl FnOnce() -> Option<gamelogic::helpers::CameraTimeObservation>,
    ) -> gamelogic::GameLogicResult<()> {
        logic
            .update_with_script_phase(
                frame,
                &mut |world| {
                    self.with_receiving_core_script_driver(bar, world, |driver| {
                        scripts.update_with_driver(
                            gamelogic::scripting::executor::ScriptContext::new(),
                            driver,
                        )
                    })
                    .map_err(|error| {
                        gamelogic::system::game_logic::GameLogicError::ScriptError(
                            error.to_string(),
                        )
                    })
                },
                observe_camera,
            )
            .map_err(|error| gamelogic::GameLogicError::Configuration(error.to_string()))
    }

    pub fn execute_core_script_subroutine(
        &mut self,
        scripts: &ScriptEngine,
        bar: &mut crate::gui::control_bar::ControlBar,
        world: &mut gamelogic::system::game_logic::GameLogic,
        name: &str,
        context: gamelogic::scripting::executor::ScriptContext,
    ) -> gamelogic::GameLogicResult<bool> {
        self.with_receiving_core_script_driver(bar, world, |driver| {
            scripts.execute_subroutine_by_name_with_driver(name, context, driver)
        })
    }
}
