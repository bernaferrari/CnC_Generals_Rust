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
