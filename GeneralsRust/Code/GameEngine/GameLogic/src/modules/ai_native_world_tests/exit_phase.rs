//! Actual cached AIExit and original-point Transport callback continuation.
use super::*;
fn boarded() -> NativeMatch {
    let mut game = NativeMatch::with_container(50.0, transport_template());
    game.issue_enter(31);
    let ai = game
        .source
        .read()
        .unwrap()
        .get_ai_update_interface()
        .unwrap();
    for frame in 31..35 {
        game.operate(frame, |world| {
            world.process_enter_path_queue(8).unwrap();
            if ai.lock().unwrap().get_current_state_id()
                == Some(crate::ai::states::AIStateType::Enter as u32)
            {
                ai.lock()
                    .unwrap()
                    .update_enter_phase_with_world(world)
                    .unwrap();
            }
        });
        if game.source.read().unwrap().get_contained_by().is_some() {
            break;
        }
    }
    assert_eq!(
        game.source.read().unwrap().get_contained_by(),
        Some(game.target.read().unwrap().get_id())
    );
    game
}
#[test]
fn owned_regular_exit_releases_ai_before_removal_then_completes_same_phase() {
    let mut game = boarded();
    let peer = boarded();
    let id = game.source.read().unwrap().get_id();
    let ai = game
        .source
        .read()
        .unwrap()
        .get_ai_update_interface()
        .unwrap();
    let _peer_source = peer.source.write().unwrap();
    let _peer_target = peer.target.write().unwrap();
    game.operate(41, |world| {
        world
            .execute_exit_command(id, false, CommandSourceType::FromScript)
            .unwrap()
    });
    assert!(game.source.read().unwrap().get_contained_by().is_some());
    assert_eq!(
        ai.lock().unwrap().get_current_state_id(),
        Some(crate::ai::states::AIStateType::Exit as u32)
    );
    {
        let mut actor = ai.lock().unwrap();
        actor
            .unit_ai_for_test()
            .unwrap()
            .set_queue_deadline_for_test(1234);
    }
    game.operate(42, |world| world.update_exit_phase(id).unwrap());
    assert_eq!(
        ai.lock()
            .unwrap()
            .unit_ai_for_test()
            .unwrap()
            .queue_deadline_for_test(),
        0,
        "the terminal state-change callback clears the actor's saved queue deadline"
    );
    assert_eq!(game.source.read().unwrap().get_contained_by(), None);
    assert!(
        !game
            .source
            .read()
            .unwrap()
            .is_disabled_by_type(crate::common::DisabledType::Held)
    );
    assert_eq!(
        game.target
            .read()
            .unwrap()
            .get_contain()
            .unwrap()
            .lock()
            .unwrap()
            .get_contained_count(),
        0
    );
    assert_eq!(
        ai.lock().unwrap().get_current_state_id(),
        Some(crate::ai::states::AIStateType::Idle as u32)
    );
}
#[test]
fn owned_instant_exit_removes_before_command_return_and_completes_next_update() {
    let mut game = boarded();
    let id = game.source.read().unwrap().get_id();
    let ai = game
        .source
        .read()
        .unwrap()
        .get_ai_update_interface()
        .unwrap();
    game.operate(51, |world| {
        world
            .execute_exit_command(id, true, CommandSourceType::FromScript)
            .unwrap()
    });
    assert_eq!(game.source.read().unwrap().get_contained_by(), None);
    assert_eq!(
        ai.lock().unwrap().get_current_state_id(),
        Some(crate::ai::states::AIStateType::ExitInstantly as u32)
    );
    game.operate(52, |world| world.update_exit_phase(id).unwrap());
    assert_eq!(
        ai.lock().unwrap().get_current_state_id(),
        Some(crate::ai::states::AIStateType::Idle as u32)
    );
}
