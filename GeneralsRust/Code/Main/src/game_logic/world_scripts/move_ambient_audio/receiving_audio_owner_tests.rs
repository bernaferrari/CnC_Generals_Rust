//! Actual receiving Object factory and movement/ambient playback boundaries.
use super::*;
use crate::game_logic::host_enum_table_residual::HostBodyDamageType;

const NAME: &str = "AmericaVehicleReceivingAudioOwner";
fn source(audio: &str) -> String {
    format!(
        "Object {NAME}\nKindOf = VEHICLE SELECTABLE\n{audio}\nBody = ActiveBody BodyTag\nMaxHealth = 100\nInitialHealth = 100\nEnd\nEnd\n"
    )
}
fn world(audio: &str) -> GameLogic {
    let content = crate::assets::ReceivingObjectContent::from_sources([(
        "receiving-audio.ini".into(),
        source(audio),
    )])
    .unwrap();
    let mut world = GameLogic::new();
    world.admit_receiving_module_content(content).unwrap();
    assert!(world.templates[NAME].receiving_client_template.is_some());
    world
}
fn spawn(world: &mut GameLogic) -> ObjectId {
    world.create_object(NAME, Team::USA, Vec3::ZERO).unwrap()
}
fn assert_cues(world: &mut GameLogic, id: ObjectId, expected: &[(&str, bool)]) {
    let events = std::mem::take(&mut world.queued_audio_events);
    let cues: Vec<_> = events
        .iter()
        .filter(|event| !event.stop)
        .map(|event| {
            assert_eq!(event.object_id, Some(id));
            assert_eq!(event.position, Some(Vec3::ZERO));
            (event.event_type.as_str(), event.is_looping)
        })
        .collect();
    assert_eq!(
        cues, expected,
        "receiving owner determines actual queued playback"
    );
}
fn foreign() {
    assert_eq!(
        game_engine::common::thing::thing_factory::load_templates_from_ini_text(
            &source(
                "SoundMoveStart = ForeignStart\nSoundMoveStartDamaged = ForeignDamagedStart\nSoundMoveLoop = ForeignLoop\nSoundMoveLoopDamaged = ForeignDamagedLoop\nSoundAmbient = ForeignAmbient\nSoundAmbientDamaged = ForeignDamageAmbient\nSoundAmbientReallyDamaged = ForeignReallyAmbient\nSoundAmbientRubble = ForeignRubble"
            ),
            "foreign-audio.ini"
        ),
        1
    );
}

#[test]
fn receiving_silence_and_same_id_peer_never_acquire_foreign_move_or_ambient() {
    crate::game_logic::game_logic::pose_owner_tests::isolated_at(
        module_path!(),
        "receiving_silence_and_same_id_peer_never_acquire_foreign_move_or_ambient",
        || {
            let mut silent = world("SoundMoveStart = NoSound\nSoundAmbient = NoSound");
            let mut peer = world("SoundMoveStart = PeerStart\nSoundAmbient = PeerAmbient");
            foreign();
            let compatibility = ThingTemplate::new(NAME);
            assert_eq!(
                resolve_for_object(&compatibility, TemplateMoveAmbientSlot::SoundAmbient)
                    .as_deref(),
                Some("ForeignAmbient")
            );
            let a = spawn(&mut silent);
            assert_eq!(
                silent.host_object(a).unwrap().ambient_audio,
                None,
                "receiving NoSound must not fall through to foreign template"
            );
            assert_cues(&mut silent, a, &[]);
            let b = spawn(&mut peer);
            assert_eq!(a, b);
            assert_cues(&mut peer, b, &[("PeerAmbient", true)]);
            silent.start_move_sound(a);
            assert_cues(&mut silent, a, &[]);
            peer.start_move_sound(b);
            assert_cues(&mut peer, b, &[("PeerStart", false)]);
            let held = game_engine::common::thing::get_thing_factory().unwrap();
            for state in [
                HostBodyDamageType::Pristine,
                HostBodyDamageType::Damaged,
                HostBodyDamageType::ReallyDamaged,
                HostBodyDamageType::Rubble,
            ] {
                silent.objects.get_mut(&a).unwrap().body_damage_state = state;
                silent.start_ambient_sound(a);
                silent.start_move_sound(a);
                assert_cues(&mut silent, a, &[]);
            }
            drop(held);
            peer.reset();
            assert_eq!(silent.host_object(a).unwrap().ambient_audio, None);
            let c = spawn(&mut peer);
            assert_eq!(c, b);
            assert_cues(&mut peer, c, &[("PeerAmbient", true)]);
            silent.start_ambient_sound(a);
            assert_cues(&mut silent, a, &[]);
        },
    );
}

#[test]
fn receiving_damage_fallbacks_use_effective_owner_not_foreign_slots() {
    crate::game_logic::game_logic::pose_owner_tests::isolated_at(
        module_path!(),
        "receiving_damage_fallbacks_use_effective_owner_not_foreign_slots",
        || {
            let mut world = world(
                "SoundAmbient = OwnAmbient\nSoundMoveStart = NoSound\nSoundMoveLoop = OwnLoop\nSoundMoveStartDamaged = NoSound\nSoundMoveLoopDamaged = OwnDamageLoop",
            );
            foreign();
            let id = spawn(&mut world);
            assert_cues(&mut world, id, &[("OwnAmbient", true)]);
            assert!(world.unit_command_move_free(
                id,
                Vec3::new(80.0, 0.0, 0.0),
                Vec3::new(80.0, 0.0, 0.0)
            ));
            assert_cues(&mut world, id, &[("OwnLoop", true)]);
            assert_eq!(
                world.host_object(id).unwrap().move_loop_audio.as_deref(),
                Some("OwnLoop")
            );
            let held = game_engine::common::thing::get_thing_factory().unwrap();
            world.objects.get_mut(&id).unwrap().body_damage_state =
                HostBodyDamageType::ReallyDamaged;
            world.start_move_sound(id);
            assert_cues(&mut world, id, &[("OwnDamageLoop", true)]);
            // C++ missing damage ambient falls back to pristine, never a foreign
            // damage slot. Stop first so the actual restart emits its event.
            world.stop_ambient_sound(id);
            world.start_ambient_sound(id);
            assert_cues(&mut world, id, &[("OwnAmbient", true)]);
            world.objects.get_mut(&id).unwrap().body_damage_state = HostBodyDamageType::Rubble;
            world.start_ambient_sound(id);
            assert_cues(&mut world, id, &[]);
            assert_eq!(
                world.host_object(id).unwrap().ambient_audio,
                None,
                "C++ rubble never falls back to pristine ambient"
            );
            drop(held);
            world.reset();
            world
                .apply_receiving_module_overrides(&[format!(
                    "Object {NAME}\nSoundAmbient = NoSound\nSoundMoveLoop = NewLoop\nEnd\n"
                )])
                .unwrap();
            let next = spawn(&mut world);
            assert_eq!(next, id);
            assert_cues(&mut world, next, &[]);
            world.start_move_sound(next);
            assert_cues(&mut world, next, &[("NewLoop", true)]);
        },
    );
}
