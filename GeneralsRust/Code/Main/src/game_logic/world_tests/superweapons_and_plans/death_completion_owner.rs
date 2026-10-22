//! Death timers belong to their actual Main owner. These controls use real
//! Object begin and shipped Main fixed frames, with same-ID peers and poisoned
//! read-only observations that cannot request deletion.
//! They do not insert into a completion queue. Retail jet/heli selectors and
//! parsed SlowDeath domain rules are fixture admission, not full asset parity.
use super::*;
use crate::gameworld_shadow::GameWorldShadow;

use crate::game_logic::game_logic::world_tick::heli_motion_tests::isolated_at;

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
                assert!(object.begin_slow_death_from_ini(
                    frame,
                    &ini,
                    &mut crate::game_logic::MovementAudio::queue(&mut Vec::new())
                ));
            }
            DeathKind::Jet => assert!(object.begin_jet_slow_death(
                &mut crate::game_logic::MovementAudio::queue(&mut Vec::new())
            )),
            DeathKind::Helicopter => assert!(object.begin_helicopter_slow_death(
                &mut crate::game_logic::MovementAudio::queue(&mut Vec::new())
            )),
        }
    }
    let mut shadow = GameWorldShadow::new(64);
    shadow.sync_from_host(&logic);
    // First sync admits identities; the incremental sync carries timer fields.
    shadow.sync_from_host(&logic);
    (logic, shadow, id)
}

// Run the shipped authoritative step. The projection never advances a death timer.
fn finish_owner(logic: &mut GameLogic, shadow: &mut GameWorldShadow, id: ObjectId) {
    for _ in 0..512 {
        let Some(object) = logic.host_object(id) else {
            assert!(shadow.entity_for_host(id).is_none());
            assert!(
                logic.health_events.snapshot_damage().is_empty(),
                "final destroy must not fabricate another body hit"
            );
            return;
        };
        assert_eq!(object.health.current, 100.0);
        assert!(
            !object.status.on_die_started,
            "finishing a directly begun timer must not re-enter onDie"
        );
        let before = logic.getFrame();
        logic.update();
        assert_eq!(logic.getFrame(), before + 1);
        crate::gameworld_shadow::run_owned_host_boundary(Some(shadow), logic);
    }
    panic!("actual owner death timer did not complete within its admitted fixture bounds");
}
fn assert_live(logic: &GameLogic, id: ObjectId, why: &str) {
    let object = logic.host_object(id).expect(why);
    assert!(object.is_alive(), "{why}");
    assert_eq!(object.health.current, 100.0);
    assert!(!object.status.on_die_started);
    assert!(logic.objects_to_destroy.iter().all(|event| event.id != id));
}
fn two_worlds(kind: DeathKind) {
    let (mut a, mut sa, aid) = fixture(kind, true);
    let (mut b, mut sb, bid) = fixture(kind, false);
    assert_eq!(aid, bid);
    for _ in 0..3 {
        let frame = a.getFrame();
        b.update();
        crate::gameworld_shadow::run_owned_host_boundary(Some(&mut sb), &mut b);
        assert_eq!(
            a.getFrame(),
            frame,
            "foreign session cannot advance the timer"
        );
        assert_live(&b, bid, "foreign owner remains alive");
    }
    finish_owner(&mut a, &mut sa, aid);
    assert_live(&b, bid, "same-ID peer cannot receive final deletion");
    crate::gameworld_shadow::run_owned_host_boundary(Some(&mut sb), &mut b);
    assert_live(
        &b,
        bid,
        "empty later boundary cannot replay another owner's completion",
    );
}
fn normal_completion(kind: DeathKind) {
    let (mut a, mut sa, aid) = fixture(kind, true);
    let (b, _sb, bid) = fixture(kind, false);
    assert_eq!(aid, bid);
    finish_owner(&mut a, &mut sa, aid);
    assert_live(&b, bid, "inert constructor or completion changed a peer");
}
fn owner_boundary(kind: DeathKind) {
    for boundary in ["reset", "clear", "drop"] {
        let (mut a, mut sa, aid) = fixture(kind, true);
        let (mut b, _sb, bid) = fixture(kind, false);
        assert_eq!(aid, bid);
        let frame = a.getFrame();
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
        crate::gameworld_shadow::run_owned_host_boundary(Some(&mut sa), &mut b);
        assert_live(&b, bid, "observer reuse cannot deliver a foreign deletion");
        assert_eq!(a.getFrame(), frame);
        finish_owner(&mut a, &mut sa, aid);
        assert_live(&b, bid, "observer lifecycle cannot change same-ID peer");
    }
}
fn timer_capsule(logic: &GameLogic, id: ObjectId) -> serde_json::Value {
    let object = logic.host_object(id).expect("installed timer owner");
    assert_eq!(object.health.current, 100.0);
    assert!(!object.status.on_die_started);
    assert!(logic.objects_to_destroy.iter().all(|event| event.id != id));
    serde_json::json!({
        "slow": object.slow_death,
        "jet": object.jet_slow_death,
        "helicopter": object.helicopter_slow_death,
        "effectively_dead": object.status.effectively_dead,
        "destroyed": object.status.destroyed,
        "position": object.get_position().to_array(),
    })
}

fn disabled_timer(kind: DeathKind) {
    let (mut a, mut sa, aid) = fixture(kind, true);
    let frame = a.getFrame();
    let before = timer_capsule(&a, aid);
    // Independent oracle frames have no authority over Main timer execution.
    sa.world_mut().advance_frames(128);
    assert_eq!(a.getFrame(), frame);
    assert_eq!(
        timer_capsule(&a, aid),
        before,
        "observer clocks cannot advance or remove the installed owner timer"
    );
    sa.sync_from_host(&a);
    finish_owner(&mut a, &mut sa, aid);
}
fn authority_toggle(kind: DeathKind) {
    let (mut a, mut sa, aid) = fixture(kind, true);
    let before = timer_capsule(&a, aid);
    let eid = sa.entity_for_host(aid).unwrap();
    let entity = sa.world_mut().world_mut().entity_mut(eid).unwrap();
    entity.slow_death_phase = 4;
    entity.jet_slow_death_done = true;
    entity.heli_slow_death_done = true;
    crate::gameworld_shadow::run_owned_host_boundary(Some(&mut sa), &mut a);
    assert_eq!(
        timer_capsule(&a, aid),
        before,
        "poisoned observer completion cannot mutate Main"
    );
    finish_owner(&mut a, &mut sa, aid);
}

macro_rules! control {
    ($name:ident, $kind:ident, $body:ident) => {
        #[test]
        fn $name() {
            isolated_at(module_path!(), stringify!($name), || {
                $body(DeathKind::$kind);
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
