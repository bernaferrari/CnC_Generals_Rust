//! MenuShellFlags isolation across WindowManagerOwner instances.
use game_client_rust::gui::callbacks::{
    lan_button_pushed, set_lan_button_pushed, set_skirmish_button_pushed, skirmish_button_pushed,
};
use game_client_rust::gui::window_manager::WindowManagerOwner;

#[test]
fn menu_shell_flags_are_instance_owned() {
    let a = WindowManagerOwner::new();
    let b = WindowManagerOwner::new();
    set_lan_button_pushed(&a, true);
    set_skirmish_button_pushed(&a, true);
    assert!(lan_button_pushed(&a));
    assert!(skirmish_button_pushed(&a));
    assert!(!lan_button_pushed(&b));
    assert!(!skirmish_button_pushed(&b));
}
