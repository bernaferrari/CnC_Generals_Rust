//! Slow-death timers advance on their owning world's fixed frames. These
//! fixtures start the Object behaviors directly and exercise final retirement,
//! presentation, and same-ID peers.
use super::*;
use crate::game_logic::game_logic::world_tick::heli_motion_tests::isolated_at;
use crate::presentation_frame::PresentationFrame;

#[derive(Clone, Copy, Debug)]
enum DeathKind {
    Slow,
    Jet,
    Helicopter,
}

fn template_name(kind: DeathKind) -> &'static str {
    match kind {
        DeathKind::Slow => "OwnedDeathInfantry",
        DeathKind::Jet => "AmericaJetRaptor",
        DeathKind::Helicopter => "AmericaHelicopterComanche",
    }
}

fn fixture(kind: DeathKind, start: bool) -> (GameLogic, ObjectId) {
    let name = template_name(kind);
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
    let mut owner_idle_entry = false;
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
                    &mut crate::game_logic::ObjectAudio::record(&mut Vec::new())
                ));
            }
            DeathKind::Jet => {
                object.set_ai_state(
                    AIState::Moving,
                    frame,
                    &mut crate::game_logic::ObjectAudio::record(&mut Vec::new()),
                );
                let start = object.begin_jet_slow_death(
                    &mut crate::game_logic::ObjectAudio::record(&mut Vec::new()),
                );
                assert!(start.defer_destruction);
                owner_idle_entry = start.enter_idle;
            }
            DeathKind::Helicopter => {
                object.set_ai_state(
                    AIState::Moving,
                    frame,
                    &mut crate::game_logic::ObjectAudio::record(&mut Vec::new()),
                );
                let start = object.begin_helicopter_slow_death(
                    &mut crate::game_logic::ObjectAudio::record(&mut Vec::new()),
                );
                assert!(start.defer_destruction);
                owner_idle_entry = start.enter_idle;
            }
        }
    }
    if owner_idle_entry {
        assert!(logic
            .set_ai_state_decision_aware_for_ai(id, AIState::Idle)
            .expect("slow-death Idle entry belongs to the receiving world"));
        let actor = logic.host_object(id).unwrap();
        assert!(!actor.idle_mood_reset_pending);
    }
    (logic, id)
}

fn finish_owner(logic: &mut GameLogic, id: ObjectId) {
    let initial = PresentationFrame::try_build_from_logic(logic, 0).unwrap();
    let actor = initial.objects.iter().find(|row| row.id == id).unwrap();
    assert_eq!(actor.health_current, 100.0);
    for _ in 0..512 {
        let Some(object) = logic.host_object(id) else {
            let finished = PresentationFrame::try_build_from_logic(logic, 0).unwrap();
            assert!(finished.objects.iter().all(|row| row.id != id));
            assert!(logic.host_objects().lookup_registered(&id).is_none());
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
        logic.complete_presentation_observations();
    }
    panic!("the death timer did not complete within its fixture bounds");
}

fn assert_live(logic: &GameLogic, id: ObjectId, why: &str) {
    let object = logic.host_object(id).expect(why);
    assert!(object.is_alive(), "{why}");
    assert_eq!(object.health.current, 100.0);
    assert!(!object.status.on_die_started);
    assert!(logic.objects_to_destroy.iter().all(|event| event.id != id));
}
fn two_worlds(kind: DeathKind) {
    let (mut a, aid) = fixture(kind, true);
    let (mut b, bid) = fixture(kind, false);
    assert_eq!(aid, bid);
    for _ in 0..3 {
        let frame = a.getFrame();
        b.update();
        b.complete_presentation_observations();
        assert_eq!(
            a.getFrame(),
            frame,
            "foreign session cannot advance the timer"
        );
        assert_live(&b, bid, "foreign owner remains alive");
    }
    finish_owner(&mut a, aid);
    assert_live(&b, bid, "same-ID peer cannot receive final deletion");
    b.complete_presentation_observations();
    assert_live(
        &b,
        bid,
        "empty later boundary cannot replay another owner's completion",
    );
}
fn normal_completion(kind: DeathKind) {
    let (mut a, aid) = fixture(kind, true);
    let (b, bid) = fixture(kind, false);
    assert_eq!(aid, bid);
    finish_owner(&mut a, aid);
    assert_live(&b, bid, "inert constructor or completion changed a peer");
}
fn owner_boundary(kind: DeathKind) {
    let (mut owner, id) = fixture(kind, true);
    let before = timer_capsule(&owner, id);
    let frame = owner.getFrame();

    let spawn_replacement = |peer: &mut GameLogic| {
        peer.create_object(template_name(kind), Team::USA, Vec3::new(0.0, 10.0, 0.0))
            .unwrap()
    };
    let (mut reset, reset_id) = fixture(kind, true);
    assert_eq!(reset_id, id);
    reset.reset();
    assert!(reset.host_object(id).is_none());
    assert_eq!(spawn_replacement(&mut reset), id);

    let (mut cleared, cleared_id) = fixture(kind, true);
    assert_eq!(cleared_id, id);
    cleared.clear_all_objects();
    assert!(cleared.host_object(id).is_none());
    assert_eq!(spawn_replacement(&mut cleared), id);

    let (retired, retired_id) = fixture(kind, true);
    assert_eq!(retired_id, id);
    drop(retired);
    let (mut rebuilt, rebuilt_id) = fixture(kind, false);
    assert_eq!(rebuilt_id, id);

    assert_eq!(owner.getFrame(), frame);
    assert_eq!(timer_capsule(&owner, id), before);
    finish_owner(&mut owner, id);
    for peer in [&mut reset, &mut cleared, &mut rebuilt] {
        for _ in 0..32 {
            peer.update();
        }
        peer.complete_presentation_observations();
        assert_live(peer, id, "replacement has no retired actor's timer");
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

fn idle_owner_clock(kind: DeathKind) {
    let (mut owner, id) = fixture(kind, true);
    let (mut peer, peer_id) = fixture(kind, false);
    assert_eq!(id, peer_id);
    let frame = owner.getFrame();
    let before = timer_capsule(&owner, id);
    for _ in 0..128 {
        peer.update();
    }
    assert_eq!(peer.getFrame(), 128);
    assert_live(&peer, peer_id, "the stepped peer remains alive");
    assert_eq!(owner.getFrame(), frame);
    assert_eq!(timer_capsule(&owner, id), before);
    finish_owner(&mut owner, id);
}

fn peer_completion(kind: DeathKind) {
    let (mut owner, id) = fixture(kind, true);
    let (mut peer, peer_id) = fixture(kind, true);
    assert_eq!(id, peer_id);
    let before = timer_capsule(&owner, id);
    let frame = owner.getFrame();
    finish_owner(&mut peer, peer_id);
    owner.complete_presentation_observations();
    assert_eq!(owner.getFrame(), frame);
    assert_eq!(timer_capsule(&owner, id), before);
    finish_owner(&mut owner, id);
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
control!(slow_timer_waits_for_its_own_world, Slow, idle_owner_clock);
control!(jet_timer_waits_for_its_own_world, Jet, idle_owner_clock);
control!(
    heli_timer_waits_for_its_own_world,
    Helicopter,
    idle_owner_clock
);
control!(
    slow_peer_completion_cannot_end_owner_timer,
    Slow,
    peer_completion
);
control!(
    jet_peer_completion_cannot_end_owner_timer,
    Jet,
    peer_completion
);
control!(
    heli_peer_completion_cannot_end_owner_timer,
    Helicopter,
    peer_completion
);
