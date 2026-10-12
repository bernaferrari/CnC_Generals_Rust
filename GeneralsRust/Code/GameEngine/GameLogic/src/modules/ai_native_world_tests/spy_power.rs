//! Real installed SpyVision disable edges, vision partition effects and saved clock.
use super::*;
use crate::common::DisabledType;
use crate::modules::ai_world::AiWorld;
use crate::object::update::spy_vision_update::SpyVisionUpdateModuleData;
use crate::player::{OwnedPowerBrownout, PowerBrownoutServices, RadarEdgeServices};
use game_engine::common::system::radar::CellShroudStatus;
use game_engine::common::system::{xfer_load::XferLoad, xfer_save::XferSave};
use game_engine::common::thing::update_module::UpdateModulePtr;
use std::io::Cursor;

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
        panic!("quiet radar")
    }
}
fn spy_template() -> Arc<dyn ThingTemplate> {
    let mut base = DefaultThingTemplate::new("NativeOwnedSpy".into());
    base.add_kind_of(KindOf::Infantry);
    base.add_kind_of(KindOf::Powered);
    base.set_energy_production(-15);
    let mut modules = infantry_template().get_behavior_module_info().to_vec();
    modules.push(TemplateModuleInfo {
        name: "SpyVisionUpdate".into(),
        module_tag: "NativeOwnedSpyVision".into(),
        data: Arc::new(SpyVisionUpdateModuleData::default()),
        interface_mask: ModuleInterfaceType::UPDATE,
    });
    Arc::new(EntryTemplate { base, modules })
}
fn installed_spy(game: &NativeMatch) -> crate::object::BehaviorModuleHandle {
    game.source
        .read()
        .unwrap()
        .module_by_name(&"SpyVisionUpdate".into())
        .unwrap()
}
fn proxy(game: &NativeMatch) -> UpdateModulePtr {
    game.source
        .read()
        .unwrap()
        .named_update_registrations("SpyVisionUpdate")
        .into_iter()
        .next()
        .unwrap()
}
fn register(game: &mut NativeMatch) {
    let source = game.source.read().unwrap().get_id();
    let update = proxy(game);
    game.scheduler
        .register_sleepy_update_module(source, update, 0x3fff_ffff);
}
fn saved(game: &NativeMatch) -> Vec<u8> {
    let mut bytes = Cursor::new(Vec::new());
    installed_spy(game)
        .with_module(|module| module.xfer(&mut XferSave::new(&mut bytes, 1)))
        .unwrap();
    bytes.into_inner()
}
fn power(game: &mut NativeMatch, frame: u32, brownout: bool) -> Result<(), String> {
    power_with_current(game, frame, brownout, None)
}
fn power_with_current(
    game: &mut NativeMatch,
    frame: u32,
    brownout: bool,
    current: Option<UpdateModulePtr>,
) -> Result<(), String> {
    let source = game.source.clone();
    game.operate_with_current(frame, current, |world| {
        let (factory, players, teams) = world.service_owners();
        let mut radar = QuietRadar(frame);
        let mut power = OwnedPowerBrownout {
            factory,
            players,
            teams,
            radar: &mut radar,
            frame,
            special_power_uses_delay: true,
            ai_world: Some(world),
            misc_audio: None,
            audio: None,
        };
        power.apply_brownout_from_handle(0, brownout, &source)
    })
}
fn packed(bytes: &[u8]) -> u32 {
    u32::from_le_bytes(bytes[5..9].try_into().unwrap())
}

#[test]
fn installed_active_spy_power_edges_change_only_driving_vision_and_preserve_saved_continuation() {
    let mut game =
        NativeMatch::with_source_template(150.0, container_template(), true, spy_template());
    let mut peer =
        NativeMatch::with_source_template(150.0, container_template(), true, spy_template());
    register(&mut game);
    register(&mut peer);
    let source = game.source.clone();
    game.operate(20, |world| {
        world.activate_spy_vision_owned(&source, 100).unwrap()
    });
    assert!(game.target.read().unwrap().is_vision_spied_by_player(0));
    assert!(!peer.target.read().unwrap().is_vision_spied_by_player(0));
    let target_position = *game.target.read().unwrap().get_position();
    assert_eq!(
        game.collision
            .partition_manager()
            .get_shroud_status_for_player(0, &target_position),
        CellShroudStatus::Clear
    );
    power(&mut game, 21, true).unwrap();
    assert!(!game.target.read().unwrap().is_vision_spied_by_player(0));
    assert!(
        !peer
            .source
            .read()
            .unwrap()
            .is_disabled_by_type(DisabledType::DisabledUnderpowered)
    );
    assert_ne!(
        game.collision
            .partition_manager()
            .get_shroud_status_for_player(0, &target_position),
        CellShroudStatus::Clear,
        "actual partition lookers must be undone"
    );
    let bytes = saved(&game);
    power(&mut game, 22, true).unwrap();
    assert_eq!(
        saved(&game),
        bytes,
        "repeated disabled mask does not repeat the SpyVision edge"
    );
    assert_eq!(&bytes[..5], &[2, 1, 1, 1, 1]);
    assert_eq!(bytes.len(), 19);
    assert_eq!(packed(&bytes), (0x3fff_ffff << 2) | 2);
    assert_eq!(bytes[13], 0, "disabled spy is inactive");
    assert_eq!(bytes[14], 1, "original resetTimersNextUpdate survives save");
    assert_eq!(
        u32::from_le_bytes(bytes[15..19].try_into().unwrap()),
        u32::MAX
    );
    // Receive the actual module state into a fresh same-ID owner. Its matching
    // Object disabled edge is admitted through the real power producer too.
    power(&mut peer, 22, true).unwrap();
    installed_spy(&peer)
        .with_module(|module| module.xfer(&mut XferLoad::new(Cursor::new(&bytes), 1)))
        .unwrap();
    assert_eq!(saved(&peer), bytes);
    power(&mut game, 42, false).unwrap();
    power(&mut peer, 42, false).unwrap();
    let future = saved(&game);
    assert_eq!(future, saved(&peer));
    assert_eq!(packed(&future), (43 << 2) | 2);
    assert_eq!(future[14], 1);
    assert_eq!(u32::from_le_bytes(future[15..19].try_into().unwrap()), 42);
    assert!(
        !game.target.read().unwrap().is_vision_spied_by_player(0),
        "reenable schedules update, not activation"
    );
}

#[test]
fn absent_scheduler_rejects_before_mask_and_current_proxy_preserves_its_returned_wake() {
    let mut game =
        NativeMatch::with_source_template(150.0, container_template(), true, spy_template());
    let before = saved(&game);
    assert!(power(&mut game, 20, true).is_err());
    assert_eq!(saved(&game), before);
    assert!(
        !game
            .source
            .read()
            .unwrap()
            .is_disabled_by_type(DisabledType::DisabledUnderpowered)
    );
    register(&mut game);
    let entry = installed_spy(&game);
    let before = saved(&game);
    let blocked = entry.with_module(|_| power(&mut game, 20, true));
    assert!(
        blocked.is_err(),
        "reentrant installed module lease must reject before mutation"
    );
    assert_eq!(saved(&game), before);
    assert!(
        !game
            .source
            .read()
            .unwrap()
            .is_disabled_by_type(DisabledType::DisabledUnderpowered)
    );
    let source = game.source.clone();
    let current = proxy(&game);
    let before = saved(&game);
    power_with_current(&mut game, 20, true, Some(current.clone())).unwrap();
    assert!(
        game.source
            .read()
            .unwrap()
            .is_disabled_by_type(DisabledType::DisabledUnderpowered)
    );
    let after = saved(&game);
    assert_eq!(
        packed(&after),
        packed(&before),
        "current update's returned sleep wins over setWakeFrame"
    );
    assert_eq!(
        game.scheduler.sleepy_entry_for(&current).unwrap().1,
        0x3fff_ffff
    );
    assert_eq!(after[14], 1);
    assert_eq!(
        u32::from_le_bytes(after[15..19].try_into().unwrap()),
        u32::MAX
    );
    let mut peer =
        NativeMatch::with_source_template(150.0, container_template(), true, spy_template());
    register(&mut peer);
    let peer_before = saved(&peer);
    peer.operate(21, |world| {
        assert!(world.spy_disabled_edge(&source, false).is_err())
    });
    assert_eq!(saved(&peer), peer_before);
    assert_eq!(saved(&game), after);
}

#[test]
fn original_team_relationship_override_can_spy_the_owners_default_team() {
    let mut game =
        NativeMatch::with_source_template(150.0, container_template(), true, spy_template());
    register(&mut game);
    let team = game.source.read().unwrap().get_team().unwrap();
    game.players
        .get_player(0)
        .unwrap()
        .write()
        .unwrap()
        .set_team_relationship(&team.read().unwrap(), crate::common::Relationship::Enemies);
    let source = game.source.clone();
    game.operate(20, |world| {
        world.activate_spy_vision_owned(&source, 100).unwrap()
    });
    assert!(
        game.source.read().unwrap().is_vision_spied_by_player(0),
        "CPP visits owner row and respects team override before player identity"
    );
    assert!(game.target.read().unwrap().is_vision_spied_by_player(0));
    game.operate(21, |world| world.spy_disabled_edge(&source, true).unwrap());
    assert!(!game.source.read().unwrap().is_vision_spied_by_player(0));
    assert!(!game.target.read().unwrap().is_vision_spied_by_player(0));
}
