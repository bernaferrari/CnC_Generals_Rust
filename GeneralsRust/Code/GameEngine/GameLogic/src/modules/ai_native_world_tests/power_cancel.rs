//! Actual installed Dozer request admission and Player power-disable continuation.
use super::*;
use crate::common::DisabledType;
use crate::object::update::ai_update::dozer_ai_update::{DozerAIUpdateModuleData, DozerTask};
use crate::player::{OwnedPowerBrownout, PowerBrownoutServices, RadarEdgeServices};

struct QuietRadar(u32);
impl RadarEdgeServices for QuietRadar {
    fn has_single_player_been_defeated(&self, _: &Player) -> bool {
        false
    }
    fn client_quiet(&self) -> bool {
        true
    }
    fn in_logic_update(&self) -> bool {
        true
    }
    fn frame(&self) -> u32 {
        self.0
    }
    fn play_radar_edge(&mut self, _: i32, _: bool) {
        panic!("quiet native test has no radar audio")
    }
}
fn powered_dozer() -> Arc<dyn ThingTemplate> {
    let mut base = DefaultThingTemplate::new("NativePowerDozer".into());
    for kind in [KindOf::Dozer, KindOf::Vehicle, KindOf::Powered] {
        base.add_kind_of(kind);
    }
    base.set_energy_production(-15);
    let mut data = DozerAIUpdateModuleData::default();
    data.base.set_locomotor_set_entries(
        crate::common::LocomotorSetType::Normal,
        vec!["NativeEnterGround".into()],
    );
    // The parent's admitted ground locomotor definition is immutable test content.
    let _ = infantry_template();
    Arc::new(EntryTemplate {
        base,
        modules: vec![TemplateModuleInfo {
            name: "DozerAIUpdate".into(),
            module_tag: "NativePowerDozerAI".into(),
            data: Arc::new(data),
            interface_mask: ModuleInterfaceType::UPDATE,
        }],
    })
}
fn current_task(game: &NativeMatch) -> DozerTask {
    let ai = game
        .source
        .read()
        .unwrap()
        .get_ai_update_interface()
        .unwrap();
    let mut ai = ai.lock().unwrap();
    ai.get_dozer_ai_update_interface_mut()
        .unwrap()
        .get_current_task()
}
fn power_off(game: &mut NativeMatch, frame: u32) {
    let source = game.source.clone();
    game.operate(frame, |world| {
        let (players, teams) = world.player_service_owners();
        let mut radar = QuietRadar(frame);
        let mut power = OwnedPowerBrownout {
            objects: crate::player::PowerObjectOwner::World(world),
            players,
            teams,
            radar: &mut radar,
            frame,
            special_power_uses_delay: true,

            misc_audio: None,
            audio: None,
        };
        power.apply_brownout_from_handle(0, true, &source).unwrap();
    });
}
fn idle(game: &mut NativeMatch, frame: u32) {
    let ai = game
        .source
        .read()
        .unwrap()
        .get_ai_update_interface()
        .unwrap();
    game.operate(frame, |world| {
        ai.lock()
            .unwrap()
            .execute_command_with_world(
                &AiCommandParams::new(AiCommandType::Idle, CommandSourceType::FromAi),
                world,
            )
            .unwrap()
    });
}

#[test]
fn real_fortify_request_activates_only_in_primary_phase_then_power_cancels_it() {
    let mut game =
        NativeMatch::with_source_template(58.0, container_template(), false, powered_dozer());
    let mut peer =
        NativeMatch::with_source_template(58.0, container_template(), false, powered_dozer());
    assert_eq!(
        game.source.read().unwrap().get_id(),
        peer.source.read().unwrap().get_id()
    );
    idle(&mut game, 17);
    idle(&mut peer, 17);
    let ai = game
        .source
        .read()
        .unwrap()
        .get_ai_update_interface()
        .unwrap();
    let target = game.target.read().unwrap().get_id();
    game.operate(17, |world| {
        ai.lock()
            .unwrap()
            .request_dozer_fortify_with_world(target, world)
            .unwrap()
    });
    assert_eq!(
        current_task(&game),
        DozerTask::Invalid,
        "newTask admits a request, not an active action"
    );
    game.operate(17, |world| {
        ai.lock()
            .unwrap()
            .update_dozer_primary_with_world(world)
            .unwrap()
    });
    assert_eq!(current_task(&game), DozerTask::Fortify);
    let source = game.source.read().unwrap().get_id();
    assert_eq!(
        game.idle_workers.take(),
        (vec![(source, 0)], vec![(source, 0)]),
        "Idle body runs before condition exit"
    );
    power_off(&mut game, 18);
    assert_eq!(current_task(&game), DozerTask::Invalid);
    assert!(
        game.source
            .read()
            .unwrap()
            .is_disabled_by_type(DisabledType::DisabledUnderpowered)
    );
    let mut ai = ai.lock().unwrap();
    let dozer = ai.get_dozer_ai_update_interface_mut().unwrap();
    assert!(!dozer.is_any_task_pending());
    drop(ai);
    assert_eq!(current_task(&peer), DozerTask::Invalid);
    assert!(
        !peer
            .source
            .read()
            .unwrap()
            .is_disabled_by_type(DisabledType::DisabledUnderpowered)
    );
    assert_eq!(peer.idle_workers.take(), (vec![], vec![]));
}

#[test]
fn supplied_request_rejects_foreign_owner_and_missing_target_before_admission() {
    let mut game =
        NativeMatch::with_source_template(58.0, container_template(), false, powered_dozer());
    let mut peer =
        NativeMatch::with_source_template(58.0, container_template(), false, powered_dozer());
    let ai = game
        .source
        .read()
        .unwrap()
        .get_ai_update_interface()
        .unwrap();
    let target = game.target.read().unwrap().get_id();
    peer.operate(17, |world| {
        assert!(
            ai.lock()
                .unwrap()
                .request_dozer_fortify_with_world(target, world)
                .is_err()
        )
    });
    game.operate(17, |world| {
        assert!(
            ai.lock()
                .unwrap()
                .request_dozer_fortify_with_world(crate::common::INVALID_ID, world)
                .is_err()
        )
    });
    assert_eq!(current_task(&game), DozerTask::Invalid);
    assert!(
        !ai.lock()
            .unwrap()
            .get_dozer_ai_update_interface_mut()
            .unwrap()
            .is_any_task_pending()
    );
    assert_eq!(game.idle_workers.take(), (vec![], vec![]));
}

#[test]
fn frame_zero_request_and_no_active_task_dozer_do_not_invent_an_action() {
    let mut game =
        NativeMatch::with_source_template(58.0, container_template(), false, powered_dozer());
    idle(&mut game, 0);
    let ai = game
        .source
        .read()
        .unwrap()
        .get_ai_update_interface()
        .unwrap();
    let target = game.target.read().unwrap().get_id();
    game.operate(0, |world| {
        let mut ai = ai.lock().unwrap();
        ai.request_dozer_fortify_with_world(target, world).unwrap();
        ai.update_dozer_primary_with_world(world).unwrap();
    });
    assert_eq!(
        current_task(&game),
        DozerTask::Invalid,
        "CPP strict orderFrame > 0 selection"
    );
    assert!(
        ai.lock()
            .unwrap()
            .get_dozer_ai_update_interface_mut()
            .unwrap()
            .is_task_pending(DozerTask::Fortify)
    );
    power_off(&mut game, 1);
    assert_eq!(current_task(&game), DozerTask::Invalid);
    assert!(
        ai.lock()
            .unwrap()
            .get_dozer_ai_update_interface_mut()
            .unwrap()
            .is_task_pending(DozerTask::Fortify),
        "CPP onDisabledEdge cancels only active currentTask"
    );
}
