use gamelogic::system::engine_stores::{
    active_services, install_services, new_world_services, uninstall_services,
};
use generals_main::game_logic::GameLogic;
use std::sync::Arc;

#[test]
fn initialized_worlds_do_not_publish_over_an_existing_session() {
    let live = new_world_services();
    install_services(Arc::clone(&live));
    let first = GameLogic::initialize();
    assert!(Arc::ptr_eq(&active_services(), &live));
    let peer = GameLogic::initialize();
    assert!(Arc::ptr_eq(&active_services(), &live));
    drop(peer);
    drop(first);
    assert!(Arc::ptr_eq(&active_services(), &live));
    assert!(uninstall_services(&live));
}
