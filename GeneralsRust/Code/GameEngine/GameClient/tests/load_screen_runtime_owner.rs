//! Load-screen hooks + residual live on LoadScreenRuntime.

use game_client_rust::gui::load_screen::LoadScreenRuntime;
use game_client_rust::gui::loading_screen::{
    residual_loading_screen_is_visible, simulate_loading_screen_show,
};

#[test]
fn residual_loading_is_per_runtime() {
    let mut a = LoadScreenRuntime::default();
    let mut b = LoadScreenRuntime::default();
    assert!(simulate_loading_screen_show(&mut a.residual_loading));
    assert!(residual_loading_screen_is_visible(&a.residual_loading));
    assert!(!residual_loading_screen_is_visible(&b.residual_loading));
}
