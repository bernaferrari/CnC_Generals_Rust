//! CPP AIUpdate.cpp936–952: receiving queue wake, including saved wrapper clock.
use super::*;
use crate::modules::ai_world::AiWorld;
use std::sync::Mutex;

fn installed(
    game: &NativeMatch,
) -> (
    Arc<Mutex<dyn crate::modules::AIUpdateInterface>>,
    game_engine::common::thing::update_module::UpdateModulePtr,
) {
    let owner = game.source.read().unwrap();
    (
        owner.get_ai_update_interface().unwrap(),
        owner
            .installed_update_proxy_named("AIUpdateInterface")
            .unwrap(),
    )
}
fn queue_frame(ai: &Arc<Mutex<dyn crate::modules::AIUpdateInterface>>) -> u32 {
    ai.lock()
        .unwrap()
        .unit_ai_for_test()
        .unwrap()
        .native_waypoint_observation_for_test()
        .queue_for_path_frame
}
fn saved_wake(game: &NativeMatch) -> u32 {
    let handle = game
        .source
        .read()
        .unwrap()
        .find_update_module("AIUpdateInterface")
        .unwrap();
    handle.with_module(|module| module.get_initial_wake_frame().unwrap())
}

#[test]
fn queue_path_shortens_outside_update_before_expiry_and_preserves_peer_and_current() {
    let mut game = NativeMatch::new(150.0);
    let mut peer = NativeMatch::new(150.0);
    let (ai, proxy) = installed(&game);
    let (peer_ai, peer_proxy) = installed(&peer);
    assert_eq!(
        game.source.read().unwrap().get_id(),
        peer.source.read().unwrap().get_id()
    );
    game.scheduler
        .awaken_update_module_with_current(&proxy, None, 70, 200)
        .unwrap();
    peer.scheduler
        .awaken_update_module_with_current(&peer_proxy, None, 70, 300)
        .unwrap();
    game.operate(70, |world| {
        ai.lock()
            .unwrap()
            .set_queue_for_path_time_with_world(30, world)
    })
    .unwrap();
    assert_eq!(
        game.scheduler.sleepy_entry_for(&proxy),
        Some((game.source.read().unwrap().get_id(), 100))
    );
    assert_eq!(saved_wake(&game), 100);
    assert_eq!(queue_frame(&ai), 100);
    assert_eq!(saved_wake(&peer), 300);
    assert_eq!(queue_frame(&peer_ai), 0);
    // Longer requests preserve the existing earlier wake, but update expiry.
    game.operate(70, |world| {
        ai.lock()
            .unwrap()
            .set_queue_for_path_time_with_world(60, world)
    })
    .unwrap();
    assert_eq!(saved_wake(&game), 100);
    assert_eq!(queue_frame(&ai), 130);
    game.operate_with_current(71, Some(proxy.clone()), |world| {
        ai.lock()
            .unwrap()
            .set_queue_for_path_time_with_world(0, world)
    })
    .unwrap();
    assert_eq!(saved_wake(&game), 100);
    assert_eq!(queue_frame(&ai), 0);
    // A distinct same-ID runtime must not mutate the receiving schedule.
    let error = game.operate(72, |world| {
        peer_ai
            .lock()
            .unwrap()
            .set_queue_for_path_time_with_world(1, world)
    });
    assert!(error.is_err());
    assert_eq!(saved_wake(&game), 100);
    assert_eq!(queue_frame(&peer_ai), 0);
}

#[test]
fn queue_path_rejects_foreign_scheduler_before_expiry_and_wraps_valid_deadline() {
    let mut game = NativeMatch::new(150.0);
    let (ai, proxy) = installed(&game);
    let mut wrong_scheduler = crate::system::game_logic::GameLogic::new();
    std::mem::swap(&mut game.scheduler, &mut wrong_scheduler);
    assert!(
        game.operate(70, |world| ai
            .lock()
            .unwrap()
            .set_queue_for_path_time_with_world(30, world))
            .is_err()
    );
    assert_eq!(queue_frame(&ai), 0);
    std::mem::swap(&mut game.scheduler, &mut wrong_scheduler);
    game.scheduler
        .awaken_update_module_with_current(&proxy, None, 70, 200)
        .unwrap();
    game.operate(u32::MAX - 5, |world| {
        ai.lock()
            .unwrap()
            .set_queue_for_path_time_with_world(30, world)
    })
    .unwrap();
    assert_eq!(queue_frame(&ai), 24);
    assert_eq!(game.scheduler.sleepy_entry_for(&proxy).unwrap().1, 200);
    assert_eq!(saved_wake(&game), 200);
}

#[test]
fn held_disabled_actor_queue_wake_uses_actual_receiving_proxy() {
    let mut game = NativeMatch::with_container(50.0, transport_template());
    let (ai, proxy) = installed(&game);
    let source = game.source.read().unwrap().get_id();
    let target = game.target.read().unwrap().get_id();
    game.operate(70, |world| {
        crate::modules::ai_world::complete_enter(world, source, target)
    })
    .unwrap();
    assert!(
        game.source
            .read()
            .unwrap()
            .is_disabled_by_type(game_engine::common::thing::update_module::DisabledType::Held)
    );
    game.scheduler
        .awaken_update_module_with_current(&proxy, None, 70, 200)
        .unwrap();
    game.operate(70, |world| {
        ai.lock()
            .unwrap()
            .set_queue_for_path_time_with_world(30, world)
    })
    .unwrap();
    assert_eq!(queue_frame(&ai), 100);
    assert_eq!(saved_wake(&game), 100);
    assert_eq!(game.source.read().unwrap().get_contained_by(), Some(target));
}
