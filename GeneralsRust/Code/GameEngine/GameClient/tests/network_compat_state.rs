//! NetworkCompatState isolation across WindowManagerOwner instances.
#![cfg(not(feature = "network"))]
use game_client_rust::gui::window_manager::WindowManagerOwner;
use game_client_rust::{get_network, gamespy};

#[test]
fn network_compat_state_is_instance_owned() {
    let a = WindowManagerOwner::new();
    let b = WindowManagerOwner::new();
    let na = get_network(a.network_compat()).expect("a");
    let nb = get_network(b.network_compat()).expect("b");
    assert!(std::sync::Arc::ptr_eq(&na, &a.network_compat().facade));
    assert!(!std::sync::Arc::ptr_eq(&na, &nb));
    na.toggle_network_on();
    assert!(!na.is_network_on());
    assert!(nb.is_network_on());
    let ga = gamespy::peer_defs::get_gamespy_info(a.network_compat()).unwrap();
    let gb = gamespy::peer_defs::get_gamespy_info(b.network_compat()).unwrap();
    assert!(!std::sync::Arc::ptr_eq(&ga, &gb));
}
