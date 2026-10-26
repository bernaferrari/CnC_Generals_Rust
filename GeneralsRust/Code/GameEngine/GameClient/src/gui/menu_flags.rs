//! Menu handoff flags owned by one receiving client's menu manager.
use crate::gui::game_window::WindowEventEffects;

#[derive(Default)]
pub struct MenuFlags {
    dont_show_main_menu: bool,
    replay_was_pressed: bool,
}
pub fn get_dont_show_main_menu(effects: &WindowEventEffects<'_>) -> bool {
    effects.menu_flags().dont_show_main_menu
}
pub fn set_dont_show_main_menu(effects: &mut WindowEventEffects<'_>, value: bool) {
    effects.menu_flags_mut().dont_show_main_menu = value;
}
pub fn get_replay_was_pressed(effects: &WindowEventEffects<'_>) -> bool {
    effects.menu_flags().replay_was_pressed
}
pub fn set_replay_was_pressed(effects: &mut WindowEventEffects<'_>, value: bool) {
    effects.menu_flags_mut().replay_was_pressed = value;
}
