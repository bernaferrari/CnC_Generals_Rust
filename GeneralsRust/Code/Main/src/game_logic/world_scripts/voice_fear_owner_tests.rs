//! ActiveBody.cpp:624–637 through the receiving Body/audio boundary.
use crate::game_logic::ThingTemplate;
use crate::game_logic::combat::DamageType;
use crate::game_logic::host_usa_pilot::HostDeathType;
use crate::game_logic::object::{BodyStateFx, DamageHitContext};
use crate::game_logic::{AudioEventRequest, GameLogic, Object, ObjectId, Player, Team};
use glam::Vec3;

fn isolated(name: &str, run: impl FnOnce()) {
    crate::game_logic::game_logic::pose_owner_tests::isolated_at(module_path!(), name, run);
}

struct Roll;
impl Roll {
    fn new(value: i32) -> Self {
        crate::game_logic::host_transition_damage_fx::set_test_voice_fear_roll(Some(value));
        Self
    }
}
impl Drop for Roll {
    fn drop(&mut self) {
        crate::game_logic::host_transition_damage_fx::set_test_voice_fear_roll(None);
    }
}

fn receiving(voice: &str, owner: u32) -> (GameLogic, ObjectId) {
    let mut world = GameLogic::new();
    let ini = format!(
        "Object OwnedFearInfantry\n KindOf = INFANTRY\n VoiceFear = {voice}\n Body = ActiveBody Health\n MaxHealth = 100\n InitialHealth = 100\n End\nEnd\n"
    );
    world
        .admit_receiving_module_content(
            crate::assets::ReceivingObjectContent::from_sources([("fear-owner.ini".into(), ini)])
                .unwrap(),
        )
        .unwrap();
    world.add_player(Player::new(owner, Team::USA, "FearOwner", true));
    let id = world
        .create_object_for_player("OwnedFearInfantry", owner, Vec3::new(120., 4., 80.))
        .unwrap();
    world.host_object_mut(id).unwrap().health.current = 40.;
    (world, id)
}

fn hit(world: &mut GameLogic, id: ObjectId, amount: f32) {
    world
        .apply_owned_damage(
            id,
            amount,
            None,
            DamageType::Unresistable,
            HostDeathType::Normal,
            None,
            &DamageHitContext::default(),
        )
        .unwrap();
}

fn seed_with_successful_fear_roll() -> (u32, i32, [u32; 6]) {
    for seed in 1..=1000 {
        let random = game_engine::common::random_value::SessionRandom::default();
        random.seed_random(seed);
        let roll = random.next_int(0, 99);
        if roll < 25 {
            return (seed, roll, random.seed_words());
        }
    }
    unreachable!("a successful fear roll exists in the seed space")
}
fn assert_fear(requests: &[AudioEventRequest], name: &str, position: Vec3, owner: u32) {
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0].event_type, name);
    assert_eq!(requests[0].position, Some(position));
    assert_eq!(requests[0].player_index, Some(owner as i32));
    assert_eq!(
        requests[0].object_id, None,
        "C++ fear is positional, not ObjectID-attached"
    );
}

#[test]
fn receiving_damage_emits_once_with_owned_definition_position_and_player() {
    isolated(
        "receiving_damage_emits_once_with_owned_definition_position_and_player",
        || {
            let _roll = Roll::new(0);
            let (mut a, aid) = receiving("OwnerAFear", 1);
            let (mut b, bid) = receiving("OwnerBFear", 2);
            assert_eq!(aid, bid);
            b.set_fixture_object_position(bid, Vec3::new(-30., 9., 75.));
            hit(&mut a, aid, 20.);
            assert_fear(
                &a.queued_audio_events,
                "OwnerAFear",
                Vec3::new(120., 4., 80.),
                1,
            );
            assert!(b.queued_audio_events.is_empty());
            assert_eq!(b.host_object(bid).unwrap().health.current, 40.);
            assert!(
                a.host_object_mut(aid)
                    .unwrap()
                    .take_pending_transition_damage_fx()
                    .iter()
                    .all(|event| event.audio_name.as_deref() != Some("OwnerAFear"))
            );
            a.set_fixture_object_position(aid, Vec3::splat(999.));
            a.host_object_mut(aid).unwrap().owner_player_id = Some(8);
            assert_fear(
                &a.queued_audio_events,
                "OwnerAFear",
                Vec3::new(120., 4., 80.),
                1,
            );
            hit(&mut b, bid, 20.);
            assert_fear(
                &b.queued_audio_events,
                "OwnerBFear",
                Vec3::new(-30., 9., 75.),
                2,
            );
            let peer = GameLogic::new();
            drop(peer);
            b.reset();
            drop(b);
            assert_fear(
                &a.queued_audio_events,
                "OwnerAFear",
                Vec3::new(120., 4., 80.),
                1,
            );
            hit(&mut a, aid, 1.);
            assert_eq!(
                a.queued_audio_events.len(),
                1,
                "noncrossing hit cannot replay fear"
            );
            assert!(
                a.apply_owned_damage(
                    ObjectId(u32::MAX),
                    20.,
                    None,
                    DamageType::Unresistable,
                    HostDeathType::Normal,
                    None,
                    &DamageHitContext::default()
                )
                .is_none()
            );
            assert_eq!(a.queued_audio_events.len(), 1);
            a.reset();
            assert!(
                a.queued_audio_events.is_empty(),
                "outgoing match audio cannot survive reset"
            );
        },
    );
}

#[test]
fn receiving_voice_fear_draw_uses_the_interleaved_world_rng() {
    isolated(
        "receiving_voice_fear_draw_uses_the_interleaved_world_rng",
        || {
            let (seed_a, expected_roll, expected_words) = seed_with_successful_fear_roll();
            let (mut control_a, control_id) = receiving("OwnerAFear", 1);
            let (mut a, a_id) = receiving("OwnerAFear", 1);
            let (mut b, b_id) = receiving("OwnerBFear", 2);
            assert_eq!(control_id, a_id);
            assert_eq!(a_id, b_id, "test worlds reuse the same object id");
            for world in [&mut control_a, &mut a] {
                world.set_logic_random_seed(seed_a);
            }
            b.set_logic_random_seed(0xB22D_F002);

            hit(&mut control_a, control_id, 20.);
            assert_fear(
                &control_a.queued_audio_events,
                "OwnerAFear",
                Vec3::new(120., 4., 80.),
                1,
            );
            assert_eq!(
                control_a.logic_random.seed_words(),
                expected_words,
                "eligible fear crossing consumes exactly one receiving draw"
            );

            // Exercise the same production damage boundary in a peer first, with
            // additional peer-only draws around it. A's chance and continuation
            // must still match its isolated control stream.
            for _ in 0..17 {
                b.logic_random.next_int(0, 999);
            }
            hit(&mut b, b_id, 20.);
            b.logic_random.next_int(0, 999);
            hit(&mut a, a_id, 20.);
            assert!(expected_roll < 25);
            assert_fear(
                &a.queued_audio_events,
                "OwnerAFear",
                Vec3::new(120., 4., 80.),
                1,
            );
            assert_eq!(a.logic_random.seed_words(), expected_words);
            assert_eq!(
                a.logic_random.seed_words(),
                control_a.logic_random.seed_words()
            );
        },
    );
}

#[test]
fn source_health_and_chance_boundaries_consume_only_eligible_draws() {
    // Literal source-branch expectations; this checks admission, not RNG sequence.
    let rows = [
        (100., 24., 100., 0, 1, true),
        (100., 24., 100., 24, 1, true),
        (100., 24., 100., 25, 1, false),
        (100., 24., 100., 99, 1, false),
        (25., 24., 100., 0, 0, false),
        (100., 25., 100., 0, 0, false),
        (24., 23., 100., 0, 0, false),
        (100., 0., 100., 0, 0, false),
        (100., -1., 100., 0, 0, false),
        (100., 30., 100., 0, 0, false),
        (10., 24., 100., 0, 0, false),
        (500., 124., 500., 24, 1, true),
        (100., 1., 0., 0, 0, false),
    ];
    for (before, after, max, roll, draws, emits) in rows {
        let mut template = ThingTemplate::new("FearGate");
        template.voice_fear = Some("GateFear".into());
        let mut object = Object::new(template, ObjectId(2), Team::USA);
        object.previous_health = before;
        object.health.current = after;
        object.health.maximum = max;
        let count = std::cell::Cell::new(0);
        let result = object.voice_fear_request(|| {
            count.set(count.get() + 1);
            roll
        });
        assert_eq!(
            count.get(),
            draws,
            "{before}->{after}, max{max}, roll{roll}"
        );
        assert_eq!(result.is_some(), emits);
    }
    let mut object = Object::new(ThingTemplate::new("SilentFear"), ObjectId(3), Team::USA);
    object.previous_health = 100.;
    object.health.current = 24.;
    object.health.maximum = 100.;
    let mut draws = 0;
    assert!(
        object
            .voice_fear_request(|| {
                draws += 1;
                0
            })
            .is_none()
    );
    assert_eq!(
        draws, 1,
        "eligible silent event still consumes the chance draw"
    );
}

#[test]
fn residual_body_route_uses_the_same_receiving_audio_owner() {
    isolated(
        "residual_body_route_uses_the_same_receiving_audio_owner",
        || {
            let _roll = Roll::new(24);
            let (mut world, id) = receiving("ResidualFear", 3);
            assert!(!world.apply_residual_damage_owned(
                id,
                20.,
                None,
                DamageType::Unresistable,
                HostDeathType::Normal,
                None
            ));
            assert_fear(
                &world.queued_audio_events,
                "ResidualFear",
                Vec3::new(120., 4., 80.),
                3,
            );
        },
    );
}

#[test]
fn callback_completion_rereads_late_values_and_is_consumed_once_before_damage_fx() {
    isolated(
        "callback_completion_rereads_late_values_and_is_consumed_once_before_damage_fx",
        || {
            let _roll = Roll::new(0);
            let mut template = ThingTemplate::new("BeforeCallback");
            template
                .set_health(100.)
                .add_kind_of(crate::game_logic::KindOf::Infantry);
            template.voice_fear = Some("BeforeCallbackFear".into());
            let mut object = Object::new(template, ObjectId(77), Team::USA);
            object.health.current = 40.;
            let mut health = crate::game_logic::HostHealthEvents::default();
            let mut application = object.begin_damage_with_context(
                20.,
                None,
                DamageType::Unresistable,
                HostDeathType::Normal,
                None,
                10,
                &DamageHitContext::default(),
                &mut health,
                None,
            );
            // This is the real continuation window used for synchronous onDamage.
            object.set_position(Vec3::new(81., 2., -33.));
            object.owner_player_id = Some(11);
            let mut audio = Vec::new();
            let mut receiver = |object: &mut Object,
                                effect,
                                source: Option<
                &crate::game_logic::host_transition_damage_fx::HostDamageFxVictim,
            >| {
                match effect {
                    BodyStateFx::VoiceFear(request) => {
                        assert_eq!(object.health.current, 20.);
                        assert!(!object.status.on_die_started);
                        audio.push(request);
                    }
                    effect => GameLogic::execute_body_state_fx_without_view(
                        object,
                        effect,
                        source,
                        &mut audio,
                        &game_engine::common::random_value::SessionRandom::default(),
                        &game_engine::common::ini::ini_damage_fx::DamageFXStore::new(),
                    ),
                }
            };
            application.complete_body_state_callbacks_with_body_fx(
                &mut object,
                None,
                &mut receiver,
            );
            application.complete_body_state_callbacks_with_body_fx(
                &mut object,
                None,
                &mut receiver,
            );
            application.finish_standalone_with_body_fx(&mut object, None, &false, &mut receiver);
            assert_fear(&audio, "BeforeCallbackFear", Vec3::new(81., 2., -33.), 11);
        },
    );
}

#[test]
fn actual_radiation_producers_deliver_fear_to_their_receiving_world() {
    isolated(
        "actual_radiation_producers_deliver_fear_to_their_receiving_world",
        || {
            let _roll = Roll::new(0);
            for mode in 0..3 {
                let (mut world, victim) = receiving("RadiationFear", 1);
                let position = Vec3::ZERO;
                world.set_fixture_object_position(victim, position);
                world.host_object_mut(victim).unwrap().health.current =
                    if mode == 2 { 26. } else { 30. };
                // A target classification flag alone must not mask a grounded hit.
                world
                    .host_object_mut(victim)
                    .unwrap()
                    .status
                    .airborne_target = true;
                let high = world
                    .create_object_for_player("OwnedFearInfantry", 1, Vec3::new(0., 1000., 0.))
                    .unwrap();
                world.host_object_mut(high).unwrap().health.current = 26.;
                let source = world
                    .create_object_for_player("OwnedFearInfantry", 1, Vec3::splat(1000.))
                    .unwrap();
                match mode {
                    0 => {
                        world.nuke_cannon_residual.spawn_radiation_zone(
                            source,
                            Team::China,
                            position,
                            world.frame,
                        );
                        world.update_nuke_cannon_radiation_zones();
                    }
                    1 => {
                        world.special_power_strikes.spawn_radiation_field(
                            source,
                            Team::China,
                            position,
                            world.frame,
                            0,
                        );
                        world.update_nuclear_radiation_fields();
                    }
                    _ => {
                        world.nuclear_tanks.spawn_radiation_zone(
                            source,
                            Team::China,
                            position,
                            world.frame,
                        );
                        world.update_nuclear_tanks_radiation_zones();
                    }
                }
                assert!(world.host_object(victim).unwrap().health.current < 25.);
                assert_eq!(
                    world.host_object(high).unwrap().health.current,
                    26.,
                    "height-based airborne filter before Body"
                );
                assert_fear(&world.queued_audio_events, "RadiationFear", position, 1);
            }
        },
    );
}

#[test]
fn encoded_restore_rebinds_fear_definition_and_discards_outgoing_audio() {
    isolated(
        "encoded_restore_rebinds_fear_definition_and_discards_outgoing_audio",
        || {
            let _roll = Roll::new(0);
            let (source, id) = receiving("SavedFear", 1);
            let builder = crate::save_load::SnapshotBuilder::new();
            let snapshot = builder.create_world_snapshot(&source).unwrap();
            let bytes = bincode_legacy::serialize(&snapshot).unwrap();
            let decoded =
                crate::save_load::snapshot::decode_bincode_world_snapshot(&bytes).unwrap();
            let (mut receiver, rid) = receiving("SavedFear", 1);
            assert_eq!(id, rid);
            receiver
                .queued_audio_events
                .push(AudioEventRequest::new("OutgoingAudio"));
            let mut invalid =
                crate::save_load::snapshot::decode_bincode_world_snapshot(&bytes).unwrap();
            invalid.version = u32::MAX;
            assert!(
                builder
                    .restore_from_snapshot(&invalid, &mut receiver)
                    .is_err()
            );
            assert_eq!(
                receiver.queued_audio_events.len(),
                1,
                "failed preflight keeps live requests"
            );
            builder
                .restore_from_snapshot(&decoded, &mut receiver)
                .unwrap();
            assert!(receiver.queued_audio_events.is_empty());
            hit(&mut receiver, id, 20.);
            assert_fear(
                &receiver.queued_audio_events,
                "SavedFear",
                Vec3::new(120., 4., 80.),
                1,
            );
        },
    );
}

#[test]
fn receiving_negative_health_chance_and_nosound_cases_stay_silent() {
    isolated(
        "receiving_negative_health_chance_and_nosound_cases_stay_silent",
        || {
            for (voice, health, damage, roll) in [
                ("NegativeFear", 40., 20., 25),
                ("NegativeFear", 25., 1., 0),
                ("NegativeFear", 40., 15., 0),
                ("NegativeFear", 20., 1., 0),
                ("NegativeFear", 40., 40., 0),
                ("NoSound", 40., 20., 0),
            ] {
                let _roll = Roll::new(roll);
                let (mut world, id) = receiving(voice, 1);
                world.host_object_mut(id).unwrap().health.current = health;
                hit(&mut world, id, damage);
                assert!(
                    world.queued_audio_events.is_empty(),
                    "{voice}, {health}, {damage}, {roll}"
                );
            }
        },
    );
}

#[test]
fn fear_is_already_admitted_when_the_original_post_damage_fx_point_is_observed() {
    isolated(
        "fear_is_already_admitted_when_the_original_post_damage_fx_point_is_observed",
        || {
            let _roll = Roll::new(0);
            let (mut world, id) = receiving("BeforeArmorObserverFear", 1);
            let mut observed = false;
            world
                .apply_owned_damage_with_killer_team_and_after_fx(
                    id,
                    20.,
                    None,
                    DamageType::Unresistable,
                    HostDeathType::Normal,
                    None,
                    &DamageHitContext::default(),
                    None,
                    |world, victim| {
                        assert_eq!(victim, id);
                        assert_fear(
                            &world.queued_audio_events,
                            "BeforeArmorObserverFear",
                            Vec3::new(120., 4., 80.),
                            1,
                        );
                        observed = true;
                    },
                )
                .unwrap();
            assert!(observed);
            assert_eq!(world.queued_audio_events.len(), 1);
        },
    );
}
