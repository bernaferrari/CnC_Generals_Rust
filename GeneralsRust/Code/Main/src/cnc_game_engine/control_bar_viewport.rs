//! Native ControlBar operations borrow the receiving bar and camera directly.
use super::*;

#[cfg(feature = "game_client")]
impl CnCGameEngine {
    fn control_bar_parent_handle(
        &self,
    ) -> Option<std::rc::Rc<std::cell::RefCell<game_client::gui::GameWindow>>> {
        game_client::gui::with_window_manager(|manager| {
            manager.find_window_by_name(crate::gameplay_layout::CONTROL_BAR_PARENT_NAME)
        })
    }

    pub(super) fn update_owned_control_bar_animations(&mut self) {
        // Source Mouse/WindowXlat observes stationary hover each admitted UI
        // frame before ControlBar consumes and clears its show-layout flag.
        let replay = matches!(
            self.game_logic.game_mode(),
            crate::game_logic::GameMode::Replay
        );
        let game_ending = self.game_logic.owned_script_game_ending();
        let tooltips_disabled = self
            .game_client
            .in_game_ui_subsystem()
            .and_then(|ui| {
                ui.lock()
                    .ok()
                    .map(|ui| ui.tooltips_disabled_at(self.game_logic.frame))
            })
            .unwrap_or(true);
        let cursor_mouse = self.game_client.input_mouse_handle();
        let (x, y) = self.mouse_position;
        let mut context = game_client::gui::window_manager::ControlBarInputContext {
            camera_options: None,
            control_bar: &mut self.control_bar,
            viewport: &mut self.game_logic.session_view.camera,
            replay,
            game_ending,
            tooltips_disabled,
            cursor_mouse: cursor_mouse.as_ref(),
        };
        game_client::gui::with_window_manager(|manager| {
            manager.refresh_control_bar_tooltip(x as i32, y as i32, &mut context);
        });
        self.control_bar
            .update_build_tooltip_layout_owned(game_ending);
        let parent = self.control_bar_parent_handle();
        self.control_bar_callbacks.update_window_animations(parent);
    }

    pub(super) fn set_owned_control_bar_scheme(&mut self, side: &str) {
        let parent = self.control_bar_parent_handle();
        let replay = matches!(
            self.game_logic.game_mode(),
            crate::game_logic::GameMode::Replay
        );
        let side = if replay { "Observer" } else { side };
        self.control_bar
            .set_control_bar_scheme_by_player_side_owned(
                side,
                &mut self.game_logic.session_view.camera,
                replay,
                parent,
            );
    }

    pub(super) fn show_owned_control_bar(&mut self, immediate: bool) {
        let replay = matches!(
            self.game_logic.game_mode(),
            crate::game_logic::GameMode::Replay
        );
        let parent = game_client::gui::with_window_manager(|manager| {
            game_client::gui::callbacks::replay_controls::show_replay_controls_owned(
                manager, replay,
            );
            manager.find_window_by_name(crate::gameplay_layout::CONTROL_BAR_PARENT_NAME)
        });
        self.control_bar_callbacks.show_control_bar_owned(
            &mut self.control_bar,
            &mut self.game_logic.session_view.camera,
            parent,
            replay,
            immediate,
        );
    }

    pub(super) fn hide_owned_control_bar(&mut self, immediate: bool) {
        let replay = matches!(
            self.game_logic.game_mode(),
            crate::game_logic::GameMode::Replay
        );
        let parent = game_client::gui::with_window_manager(|manager| {
            game_client::gui::callbacks::replay_controls::hide_replay_controls_owned(
                manager, replay,
            );
            manager.find_window_by_name(crate::gameplay_layout::CONTROL_BAR_PARENT_NAME)
        });
        self.control_bar_callbacks.hide_control_bar_owned(
            &mut self.control_bar,
            &mut self.game_logic.session_view.camera,
            parent,
            true,
            immediate,
        );
    }

    pub(super) fn toggle_owned_control_bar(&mut self, immediate: bool) {
        let replay = matches!(
            self.game_logic.game_mode(),
            crate::game_logic::GameMode::Replay
        );
        let parent = game_client::gui::with_window_manager(|manager| {
            game_client::gui::callbacks::replay_controls::toggle_replay_controls_owned(
                manager, replay,
            );
            manager.find_window_by_name(crate::gameplay_layout::CONTROL_BAR_PARENT_NAME)
        });
        self.control_bar_callbacks.toggle_control_bar_owned(
            &mut self.control_bar,
            &mut self.game_logic.session_view.camera,
            parent,
            replay,
            immediate,
        );
    }
}

/// Native script actions borrow UI for one synchronous interpreter operation.
/// GameLogic retains only its camera; no UI service is published into a world.
#[cfg(feature = "game_client")]
pub(super) struct NativeScriptUi<'a> {
    pub(super) input: Option<super::mouse::CinematicInputLoan<'a>>,
    snow: &'a mut game_client::snow::SnowManager,
    bar: &'a mut game_client::gui::control_bar::ControlBar,
    callbacks: &'a mut game_client::gui::callbacks::control_bar_callbacks::ControlBarCallbacks,
}

#[cfg(feature = "game_client")]
impl<'a> NativeScriptUi<'a> {
    pub(super) fn new(
        snow: &'a mut game_client::snow::SnowManager,
        bar: &'a mut game_client::gui::control_bar::ControlBar,
        callbacks: &'a mut game_client::gui::callbacks::control_bar_callbacks::ControlBarCallbacks,
    ) -> Self {
        Self {
            input: None,
            snow,
            bar,
            callbacks,
        }
    }

    pub(super) fn with_cinematic_input(
        mut self,
        input: super::mouse::CinematicInputLoan<'a>,
    ) -> Self {
        self.input = Some(input);
        self
    }

    fn letterbox_with_parent(
        &mut self,
        camera: &mut crate::presentation_frame::camera::SessionCamera,
        enabled: bool,
        replay: bool,
        parent: Option<std::rc::Rc<std::cell::RefCell<game_client::gui::GameWindow>>>,
    ) {
        // ScriptActions.cpp:3749-3760: HideControlBar(TRUE) precedes enabling
        // letterbox; ShowControlBar(FALSE) precedes disabling it. Every authored
        // call applies even if the display's letterbox flag is unchanged.
        if enabled {
            self.callbacks
                .hide_control_bar_owned(self.bar, camera, parent, true, true);
        } else {
            self.callbacks
                .show_control_bar_owned(self.bar, camera, parent, replay, false);
        }
    }
}

#[cfg(feature = "game_client")]
impl crate::game_logic::ScriptUiEffects for NativeScriptUi<'_> {
    fn has_cinematic_input(&self) -> bool {
        self.input.as_ref().is_some_and(|input| input.available())
    }
    fn cinematic_input(
        &mut self,
        enabled: bool,
        was_enabled: bool,
        deselect: &mut dyn FnMut(u32),
    ) -> gamelogic::GameLogicResult<()> {
        self.input
            .as_mut()
            .ok_or_else(|| {
                gamelogic::GameLogicError::Configuration(
                    "native cinematic input loan is absent".into(),
                )
            })?
            .apply(enabled, was_enabled, deselect, self.bar)
    }

    fn weather_visible(&mut self, visible: bool) {
        self.snow.set_visible(visible);
    }

    fn letterbox(
        &mut self,
        camera: &mut crate::presentation_frame::camera::SessionCamera,
        enabled: bool,
        replay: bool,
    ) {
        let parent = game_client::gui::with_window_manager(|manager| {
            if enabled {
                game_client::gui::callbacks::replay_controls::hide_replay_controls_owned(
                    manager, replay,
                );
            } else {
                game_client::gui::callbacks::replay_controls::show_replay_controls_owned(
                    manager, replay,
                );
            }
            manager.find_window_by_name(crate::gameplay_layout::CONTROL_BAR_PARENT_NAME)
        });
        self.letterbox_with_parent(camera, enabled, replay, parent);
    }
}

#[cfg(all(test, feature = "game_client"))]
#[path = "control_bar_viewport_tests.rs"]
mod script_ui_tests;
