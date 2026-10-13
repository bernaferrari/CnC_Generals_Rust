//! CPP AIUpdate.cpp936–952 and DozerAIUpdate.cpp2421–2480 inherited ownership.
//! This is inherited AI/Update continuation; full DozerMachine saves reject.
use super::*;
use crate::modules::ai_world::AiWorld;
use crate::object::update::ai_update::dozer_ai_update::{
    DozerAIUpdateModule, DozerAIUpdateModuleData,
};
use game_engine::common::system::{xfer_load::XferLoad, xfer_save::XferSave};
use std::io::Cursor;
use std::sync::Mutex;

fn template() -> Arc<dyn ThingTemplate> {
    let mut base = DefaultThingTemplate::new("OwnedDozerClock".into());
    base.add_kind_of(KindOf::Dozer);
    base.add_kind_of(KindOf::Vehicle);
    let mut data = DozerAIUpdateModuleData::default();
    data.base.set_locomotor_set_entries(
        crate::common::LocomotorSetType::Normal,
        vec!["NativeEnterGround".into()],
    );
    let _ = infantry_template();
    Arc::new(EntryTemplate {
        base,
        modules: vec![TemplateModuleInfo {
            name: "DozerAIUpdate".into(),
            module_tag: "OwnedDozerClockAI".into(),
            data: Arc::new(data),
            interface_mask: ModuleInterfaceType::UPDATE,
        }],
    })
}
fn world() -> NativeMatch {
    NativeMatch::with_source_template(50.0, container_template(), false, template())
}
fn installed(
    game: &NativeMatch,
) -> (
    Arc<Mutex<dyn crate::modules::AIUpdateInterface>>,
    game_engine::common::thing::update_module::UpdateModulePtr,
) {
    let owner = game.source.read().unwrap();
    (
        owner.get_ai_update_interface().unwrap(),
        owner.installed_update_proxy_named("DozerAIUpdate").unwrap(),
    )
}
fn wake(game: &NativeMatch) -> u32 {
    game.source
        .read()
        .unwrap()
        .find_update_module("DozerAIUpdate")
        .unwrap()
        .with_module(|module| module.get_initial_wake_frame().unwrap())
}
fn queue(ai: &Arc<Mutex<dyn crate::modules::AIUpdateInterface>>) -> u32 {
    ai.lock()
        .unwrap()
        .unit_ai_for_test()
        .unwrap()
        .native_waypoint_observation_for_test()
        .queue_for_path_frame
}
fn xfer_inherited(game: &NativeMatch, xfer: &mut dyn crate::common::xfer::Xfer) {
    let mut loco = crate::locomotor::LocomotorTemplate::new("NativeEnterGround".into());
    loco.surfaces = crate::locomotor::SURFACE_GROUND;
    let loco = Arc::new(loco);
    let handle = game
        .source
        .read()
        .unwrap()
        .find_update_module("DozerAIUpdate")
        .unwrap();
    handle.with_module(|module| {
        module
            .as_any_mut()
            .downcast_mut::<DozerAIUpdateModule>()
            .unwrap()
            .xfer_inherited_with_locomotors(xfer, &mut |name| {
                Ok((name == "NativeEnterGround").then(|| loco.clone()))
            })
            .unwrap();
    });
}
#[test]
fn installed_dozer_queue_wake_and_inherited_save_keep_receiving_runtime() {
    let mut game = world();
    let mut peer = world();
    let (ai, proxy) = installed(&game);
    let (peer_ai, peer_proxy) = installed(&peer);
    // CPP AIUpdate's constructor enters its default machine before normal
    // snapshots. Our inert factory needs the actual supplied Idle admission.
    for (owner, runtime) in [(&mut game, &ai), (&mut peer, &peer_ai)] {
        owner
            .operate(70, |world| {
                runtime.lock().unwrap().execute_command_with_world(
                    &crate::ai::AiCommandParams::new(
                        crate::ai::AiCommandType::Idle,
                        crate::common::CommandSourceType::FromAi,
                    ),
                    world,
                )
            })
            .unwrap();
    }
    let handle = game
        .source
        .read()
        .unwrap()
        .find_update_module("DozerAIUpdate")
        .unwrap();
    handle.with_module(|module| {
        let runtime = module
            .as_any()
            .downcast_ref::<DozerAIUpdateModule>()
            .unwrap()
            .runtime_ai_owned()
            .unwrap();
        assert!(
            Arc::ptr_eq(&runtime, &ai),
            "wrapper binds exact admitted cached AI"
        );
        assert!(!Arc::ptr_eq(&runtime, &peer_ai));
    });
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
    assert_eq!(wake(&game), 100);
    assert_eq!(queue(&ai), 100);
    assert_eq!(wake(&peer), 300);
    assert_eq!(queue(&peer_ai), 0);
    let mut saved = Cursor::new(Vec::new());
    xfer_inherited(&game, &mut XferSave::new(&mut saved, 1));
    game.operate_with_current(71, Some(proxy.clone()), |world| {
        ai.lock()
            .unwrap()
            .set_queue_for_path_time_with_world(0, world)
    })
    .unwrap();
    assert_eq!(wake(&game), 100);
    assert_eq!(queue(&ai), 0);
    xfer_inherited(
        &game,
        &mut XferLoad::new(Cursor::new(saved.get_ref().clone()), 1),
    );
    assert_eq!(wake(&game), 100);
    assert_eq!(queue(&ai), 100);
    let mut resaved = Cursor::new(Vec::new());
    xfer_inherited(&game, &mut XferSave::new(&mut resaved, 1));
    assert_eq!(
        saved.into_inner(),
        resaved.into_inner(),
        "inherited runtime and wake fields restore without callbacks"
    );
    game.operate(72, |world| {
        ai.lock()
            .unwrap()
            .set_queue_for_path_time_with_world(5, world)
    })
    .unwrap();
    assert_eq!(wake(&game), 77);
    assert_eq!(queue(&ai), 77);
    assert_eq!(wake(&peer), 300);
    assert!(
        game.operate(72, |world| peer_ai
            .lock()
            .unwrap()
            .set_queue_for_path_time_with_world(1, world))
            .is_err()
    );
    assert_eq!(queue(&peer_ai), 0);
    assert_eq!(wake(&game), 77);
}
#[test]
fn installed_dozer_rejects_full_incomplete_machine_snapshot_before_bytes() {
    let game = world();
    let mut bytes = Cursor::new(Vec::new());
    let error = game
        .source
        .read()
        .unwrap()
        .find_update_module("DozerAIUpdate")
        .unwrap()
        .with_module(|module| module.xfer(&mut XferSave::new(&mut bytes, 1)));
    assert!(error.unwrap_err().contains("state-machine continuation"));
    assert!(bytes.get_ref().is_empty());
}
