//! Actual cached serializer restores mood policy and links receiving definitions.
use super::*;
use game_engine::common::system::{xfer_load::XferLoad, xfer_save::XferSave};
use std::io::Cursor;
fn ready_idle(game: &mut NativeMatch, frame: u32) {
    game.issue_enter(frame);
    let ai = game
        .source
        .read()
        .unwrap()
        .get_ai_update_interface()
        .unwrap();
    game.operate(frame + 1, |world| {
        ai.lock()
            .unwrap()
            .execute_command_with_world(
                &AiCommandParams::new(AiCommandType::Idle, CommandSourceType::FromScript),
                world,
            )
            .unwrap()
    });
}
#[test]
fn supplied_ai_xfer_restores_all_cpp_attitudes_without_clearing_saved_jitter() {
    for attitude in [
        crate::modules::AIAttitudeType::Normal,
        crate::modules::AIAttitudeType::Defensive,
        crate::modules::AIAttitudeType::Aggressive,
        crate::modules::AIAttitudeType::Passive,
        crate::modules::AIAttitudeType::Sleep,
    ] {
        let mut original = NativeMatch::new(150.0);
        let mut receiving = NativeMatch::new(190.0);
        ready_idle(&mut original, 17);
        ready_idle(&mut receiving, 71);
        let source = original
            .source
            .read()
            .unwrap()
            .get_ai_update_interface()
            .unwrap();
        let target = receiving
            .source
            .read()
            .unwrap()
            .get_ai_update_interface()
            .unwrap();
        let mut catalog = std::collections::BTreeMap::new();
        target.lock().unwrap().with_cur_locomotor(&mut |loco| {
            catalog.insert(
                loco.get_template_name().to_string(),
                Arc::clone(&loco.template),
            );
        });
        assert!(!catalog.is_empty());
        let mut bytes = Cursor::new(Vec::new());
        {
            let mut actor = source.lock().unwrap();
            actor.set_attitude(attitude).unwrap();
            actor.wake_up_and_attempt_to_target_at(217).unwrap();
            actor
                .xfer_ai_update_state(&mut XferSave::new(&mut bytes, 1))
                .unwrap();
        }
        {
            let mut lookup = |name: &str| Ok(catalog.get(name).cloned());
            let mut actor = target.lock().unwrap();
            actor
                .xfer_ai_update_state_with_locomotors(
                    &mut XferLoad::new(Cursor::new(bytes.into_inner()), 1),
                    &mut lookup,
                )
                .unwrap();
            assert_eq!(actor.get_attitude(), attitude);
            assert_eq!(actor.get_next_mood_check_time(), 217);
            assert!(
                actor.take_random_mood_offset(),
                "raw attitude load must preserve the separately saved jitter flag"
            );
            assert_eq!(
                actor.get_current_state_id(),
                Some(crate::ai::states::AIStateType::Idle as u32)
            );
        }
        receiving.operate(221, |world| {
            target
                .lock()
                .unwrap()
                .load_post_process_with_world(world)
                .unwrap()
        });
        assert_eq!(
            target.lock().unwrap().get_current_state_id(),
            Some(crate::ai::states::AIStateType::Idle as u32)
        );
        assert_eq!(receiving.source.read().unwrap().get_position().x, 50.0);
    }
}
