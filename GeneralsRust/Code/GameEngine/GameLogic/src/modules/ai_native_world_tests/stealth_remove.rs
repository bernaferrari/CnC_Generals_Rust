//! Authored StealthUpdate birth and OpenContain reveal against real local owners.
use super::*;
use crate::modules::ai_world::AiWorld;

fn stealth_rider_template() -> Arc<dyn ThingTemplate> {
    let mut base = DefaultThingTemplate::new("NativeStealthRider".into());
    base.add_kind_of(KindOf::Infantry);
    base.add_kind_of(KindOf::StealthGarrison);
    let mut modules = infantry_template().get_behavior_module_info().to_vec();
    modules.push(TemplateModuleInfo {
        name: "StealthUpdate".into(),
        module_tag: "NativeStealth".into(),
        data: Arc::new(
            crate::stealth_update::StealthUpdateModuleData::from_owned_test_fields(&[
                ("StealthDelay", &["1000"]),
                ("OrderIdleEnemiesToAttackMeUponReveal", &["Yes"]),
            ]),
        ),
        interface_mask: ModuleInterfaceType::UPDATE,
    });
    Arc::new(EntryTemplate { base, modules })
}
fn stealth_game() -> NativeMatch {
    NativeMatch::with_source_template(50.0, transport_template(), false, stealth_rider_template())
}
fn board(game: &mut NativeMatch, frame: u32) {
    let source = Arc::clone(&game.source);
    let target = game.target.read().unwrap().get_id();
    let ai = source.read().unwrap().get_ai_update_interface().unwrap();
    game.operate(frame, |world| {
        ai.lock()
            .unwrap()
            .execute_command_with_world(
                &AiCommandParams::new(AiCommandType::Idle, CommandSourceType::FromAI),
                world,
            )
            .unwrap();
        let mut command = AiCommandParams::new(AiCommandType::Enter, CommandSourceType::FromScript);
        command.obj = Some(target);
        ai.lock()
            .unwrap()
            .execute_command_with_world(&command, world)
            .unwrap();
    });
    for phase in frame..frame + 4 {
        game.operate(phase, |world| {
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
        if source.read().unwrap().get_contained_by().is_some() {
            break;
        }
    }
    assert_eq!(source.read().unwrap().get_contained_by(), Some(target));
}
#[test]
fn native_stealth_birth_and_reveal_use_supplied_frame_and_exact_cached_owner() {
    let mut game = stealth_game();
    let mut peer = stealth_game();
    let id = game.source.read().unwrap().get_id();
    let goal = game.target.read().unwrap().get_id();
    let controller = game.source.read().unwrap().get_stealth().unwrap();
    let peer_controller = peer.source.read().unwrap().get_stealth().unwrap();
    assert!(!controller.lock().unwrap().is_native_ready());
    game.operate(17, |world| world.admit_prepared_stealth(id).unwrap());
    peer.operate(71, |world| world.admit_prepared_stealth(id).unwrap());
    assert_eq!(controller.lock().unwrap().birth_deadline(), 47);
    assert_eq!(peer_controller.lock().unwrap().birth_deadline(), 101);
    game.operate(18, |world| {
        assert!(world.admit_prepared_stealth(id).is_err())
    });
    board(&mut game, 41);
    let _foreign_source = peer.source.write().unwrap();
    let _foreign_goal = peer.target.write().unwrap();
    game.operate(207, |world| world.remove_contained(id, goal, true).unwrap());
    assert_eq!(controller.lock().unwrap().detection_deadline(), 237);
    assert_eq!(peer_controller.lock().unwrap().detection_deadline(), 0);
    assert_eq!(game.source.read().unwrap().get_contained_by(), None);
    assert!(
        !game
            .source
            .read()
            .unwrap()
            .test_status(crate::common::ObjectStatusTypes::Detected)
    );
}
