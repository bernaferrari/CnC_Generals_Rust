//! hq-du7d8: real shadow timers must deliver final-death completion only to
//! their driving GameWorld. These controls use ordinary Object begin, sync,
//! native GameWorld frame advancement, and the public post-host session.
//! They do not insert into a completion queue. Retail jet/heli selectors and
//! parsed SlowDeath domain rules are fixture admission, not full asset parity.
use super::*;
use crate::game_logic::game_logic::gameworld_authority::GameWorldAuthority;
use crate::game_logic::game_logic::world_tick::heli_motion_tests::isolated_at;
use crate::gameworld_shadow::{
    GameWorldShadow, shadow_session_after_host_tick, with_gameworld_authority,
};

#[derive(Clone, Copy, Debug)]
enum DeathKind {
    Slow,
    Jet,
    Helicopter,
}

fn fixture(kind: DeathKind, start: bool) -> (GameLogic, GameWorldShadow, ObjectId) {
    let name = match kind {
        DeathKind::Slow => "OwnedDeathInfantry",
        DeathKind::Jet => "AmericaJetRaptor",
        DeathKind::Helicopter => "AmericaHelicopterComanche",
    };
    let mut logic = GameLogic::new();
    logic.set_movement_authority(true);
    let mut template = ThingTemplate::new(name);
    template.set_health(100.0);
    template
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable);
    logic.templates.insert(name.to_string(), template);
    let id = logic
        .create_object(name, Team::USA, Vec3::new(0.0, 10.0, 0.0))
        .unwrap();
    if start {
        let frame = logic.getFrame();
        let object = logic.host_object_mut(id).unwrap();
        match kind {
            DeathKind::Slow => {
                let mut parser = crate::assets::IniParser::new();
                assert_eq!(
                    parser
                        .parse_ini_content(
                            r#"
Object OwnedDeathInfantry
  Behavior = SlowDeathBehavior ModuleTag_Death
    SinkDelay = 0
    SinkRate = 0
    DestructionDelay = 300
    DestructionDelayVariance = 0
  End
End
"#,
                            "owned_death_timer.ini"
                        )
                        .unwrap(),
                    1
                );
                let definition = parser.get_definition(name).unwrap();
                let module = &definition.behavior_modules[0];
                let attrs: Vec<_> = module
                    .attributes
                    .iter()
                    .map(|(k, v)| (k.as_str(), v.as_str()))
                    .collect();
                let ini =
                    crate::game_logic::host_slow_death::slow_death_ini_from_behavior_attrs(&attrs);
                assert_eq!(ini.destruction_delay_ms, 300);
                assert!(object.begin_slow_death_from_ini(frame, &ini));
            }
            DeathKind::Jet => assert!(object.begin_jet_slow_death()),
            DeathKind::Helicopter => assert!(object.begin_helicopter_slow_death()),
        }
    }
    let mut shadow = GameWorldShadow::new(64);
    shadow.sync_from_host(&logic);
    // First sync admits identities; the incremental sync carries timer fields.
    shadow.sync_from_host(&logic);
    (logic, shadow, id)
}

fn is_done(shadow: &GameWorldShadow, id: ObjectId, kind: DeathKind) -> bool {
    let entity = shadow
        .world()
        .entity(shadow.entity_for_host(id).unwrap())
        .unwrap();
    match kind {
        DeathKind::Slow => entity.slow_death_phase == 4,
        DeathKind::Jet => entity.jet_slow_death_done,
        DeathKind::Helicopter => entity.heli_slow_death_done,
    }
}

fn produce(logic: &GameLogic, shadow: &mut GameWorldShadow, id: ObjectId, kind: DeathKind) {
    assert!(!is_done(shadow, id, kind));
    with_gameworld_authority(*logic.gameworld_authority(), || {
        for _ in 0..128 {
            shadow.world_mut().advance_frames(1);
            let frame = u32::try_from(shadow.world().frame()).unwrap();
            shadow.tick_status_timer_expirations(frame);
            if is_done(shadow, id, kind) {
                return;
            }
        }
        panic!("ordinary native-frame timer never completed {kind:?}");
    });
    assert!(
        logic.host_object(id).is_some(),
        "producer has not consumed its completion"
    );
}

fn consume(logic: &mut GameLogic, shadow: &mut GameWorldShadow) {
    with_gameworld_authority(*logic.gameworld_authority(), || {
        let _ = shadow_session_after_host_tick(shadow, logic);
    });
}

fn assert_live(logic: &GameLogic, id: ObjectId, why: &str) {
    let object = logic.host_object(id).expect(why);
    assert!(!object.status.destroyed, "{why}");
    assert!(object.health.current > 0.0, "{why}");
    assert!(
        logic.objects_to_destroy.iter().all(|event| event.id != id),
        "{why}"
    );
}

fn assert_final(logic: &mut GameLogic, id: ObjectId) {
    let object = logic
        .host_object(id)
        .expect("deferred final destruction remains discoverable");
    assert!(object.status.destroyed);
    // This fixture starts only the timer at positive HP. CPP final callbacks
    // delete the object without manufacturing a second body damage operation.
    assert_eq!(object.health.current, 100.0);
    assert!(!object.status.on_die_started);
    assert_eq!(
        logic
            .objects_to_destroy
            .iter()
            .filter(|event| event.id == id)
            .count(),
        1
    );
    logic.process_destroy_list();
    assert!(logic.host_object(id).is_none());
}

fn two_worlds(kind: DeathKind) {
    let (mut a, mut sa, aid) = fixture(kind, true);
    produce(&a, &mut sa, aid, kind);
    // Constructing a second world must neither take nor erase A's completion.
    let (mut b, mut sb, bid) = fixture(kind, false);
    assert_eq!(aid, bid, "allocator-local host IDs intentionally alias");
    consume(&mut b, &mut sb);
    assert_live(&b, bid, "B consumed A's final-death completion");
    consume(&mut a, &mut sa);
    assert_final(&mut a, aid);
    consume(&mut b, &mut sb);
    assert_live(&b, bid, "A's completion was delivered twice");
}

fn normal_completion(kind: DeathKind) {
    let (mut a, mut sa, aid) = fixture(kind, true);
    produce(&a, &mut sa, aid, kind);
    let (b, _sb, bid) = fixture(kind, false);
    assert_eq!(aid, bid);
    consume(&mut a, &mut sa);
    assert_final(&mut a, aid);
    assert_live(&b, bid, "construction or A delivery changed B");
}

fn owner_boundary(kind: DeathKind) {
    for boundary in ["reset", "clear", "drop"] {
        let (a, mut sa, aid) = fixture(kind, true);
        produce(&a, &mut sa, aid, kind);
        let (mut b, _sb, bid) = fixture(kind, false);
        assert_eq!(aid, bid);
        match boundary {
            "reset" => sa.reset_for_world_boundary(),
            "clear" => sa.world_mut().clear_entities(),
            "drop" => {
                drop(sa);
                sa = GameWorldShadow::new(64);
            }
            _ => unreachable!(),
        }
        sa.sync_from_host(&b);
        consume(&mut b, &mut sa);
        assert_live(
            &b,
            bid,
            "old completion survived owner boundary and ID reuse",
        );
    }
}

fn disabled_timer(kind: DeathKind) {
    let (mut a, mut sa, aid) = fixture(kind, true);
    a.set_movement_authority(false);
    with_gameworld_authority(*a.gameworld_authority(), || {
        for _ in 0..128 {
            sa.world_mut().advance_frames(1);
            let frame = u32::try_from(sa.world().frame()).unwrap();
            sa.tick_status_timer_expirations(frame);
        }
    });
    assert!(
        !is_done(&sa, aid, kind),
        "diagnostic timer completed while movement authority was disabled"
    );
    a.set_movement_authority(true);
    consume(&mut a, &mut sa);
    assert_live(
        &a,
        aid,
        "disabled timer left a stale completion for re-enable",
    );
}

fn authority_toggle(kind: DeathKind) {
    let (mut a, mut sa, aid) = fixture(kind, true);
    produce(&a, &mut sa, aid, kind);
    a.set_movement_authority(false);
    consume(&mut a, &mut sa);
    assert_live(&a, aid, "disabled consumer applied a pending completion");
    a.set_movement_authority(true);
    consume(&mut a, &mut sa);
    assert_live(&a, aid, "re-enabled consumer delivered an old completion");
    // A new ordinary timer transition is still deliverable after discard.
    produce(&a, &mut sa, aid, kind);
    consume(&mut a, &mut sa);
    assert_final(&mut a, aid);
}

macro_rules! control {
    ($name:ident, $kind:ident, $body:ident) => {
        #[test]
        fn $name() {
            isolated_at(module_path!(), stringify!($name), || {
                with_gameworld_authority(GameWorldAuthority::DEFAULT_OFF, || {
                    $body(DeathKind::$kind)
                });
            });
        }
    };
}
control!(slow_two_worlds_same_id, Slow, two_worlds);
control!(jet_two_worlds_same_id, Jet, two_worlds);
control!(heli_two_worlds_same_id, Helicopter, two_worlds);
control!(
    slow_normal_completion_and_inert_constructor,
    Slow,
    normal_completion
);
control!(
    jet_normal_completion_and_inert_constructor,
    Jet,
    normal_completion
);
control!(
    heli_normal_completion_and_inert_constructor,
    Helicopter,
    normal_completion
);
control!(slow_reset_clear_drop_reuse_ids, Slow, owner_boundary);
control!(jet_reset_clear_drop_reuse_ids, Jet, owner_boundary);
control!(heli_reset_clear_drop_reuse_ids, Helicopter, owner_boundary);
control!(slow_disabled_timer_then_reenable, Slow, disabled_timer);
control!(jet_disabled_timer_then_reenable, Jet, disabled_timer);
control!(
    heli_disabled_timer_then_reenable,
    Helicopter,
    disabled_timer
);
control!(
    slow_pending_completion_authority_toggle,
    Slow,
    authority_toggle
);
control!(
    jet_pending_completion_authority_toggle,
    Jet,
    authority_toggle
);
control!(
    heli_pending_completion_authority_toggle,
    Helicopter,
    authority_toggle
);
