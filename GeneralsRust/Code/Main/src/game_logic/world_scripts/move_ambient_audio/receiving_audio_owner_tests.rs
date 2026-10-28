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
    let id = world.create_object(NAME, Team::USA, Vec3::ZERO).unwrap();
    assert!(
        world.host_object(id).unwrap().ambient_audio.is_none(),
        "headless birth must not duplicate native Drawable constructor audio"
    );
    assert!(world.enable_owned_object_sound(id, true));
    id
}
fn assert_cues(world: &mut GameLogic, id: ObjectId, expected: &[(&str, bool)]) {
    let events = world.queued_audio_events.drain_requests();
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

#[test]
fn receiving_retry_borrows_exact_audio_manager_for_same_id_and_event_name() {
    use game_engine::common::audio::{AC_LOOP, AudioEventInfo, AudioManager};
    let mut first = world("SoundAmbient = BorrowedSameName");
    let mut peer = world("SoundAmbient = BorrowedSameName");
    let a = first.create_object(NAME, Team::USA, Vec3::ZERO).unwrap();
    let b = peer.create_object(NAME, Team::USA, Vec3::ZERO).unwrap();
    assert_eq!(a, b);
    // Actual receiving cue admission, without publishing an audio manager or
    // parsing any global audio definitions. Birth remains ambient-inactive.
    assert!(first.host_object(a).unwrap().ambient_audio.is_none());
    first.start_ambient_sound(a);
    peer.start_ambient_sound(b);
    assert_cues(&mut first, a, &[("BorrowedSameName", true)]);
    assert_cues(&mut peer, b, &[("BorrowedSameName", true)]);
    let mut audio = AudioManager::new();
    audio.register_audio_event_info(AudioEventInfo {
        audio_name: "BorrowedSameName".into(),
        control: AC_LOOP,
        loop_count: 0,
        ..Default::default()
    });
    let mut peer_audio = AudioManager::new();
    peer_audio.register_audio_event_info(AudioEventInfo {
        audio_name: "BorrowedSameName".into(),
        loop_count: 1,
        ..Default::default()
    });
    first.frame = 100;
    peer.frame = 100;
    first.retry_dropped_ambient_sounds_with_audio(Some(&audio));
    assert_cues(&mut first, a, &[("BorrowedSameName", true)]);
    peer.retry_dropped_ambient_sounds_with_audio(Some(&peer_audio));
    assert_cues(&mut peer, b, &[]);
    first.frame = 129;
    first.retry_dropped_ambient_sounds_with_audio(Some(&audio));
    assert_cues(&mut first, a, &[]);
    first.frame = 130;
    first.retry_dropped_ambient_sounds_with_audio(Some(&audio));
    assert_cues(&mut first, a, &[("BorrowedSameName", true)]);
    peer.retry_dropped_ambient_sounds_with_audio(Some(&peer_audio));
    assert_cues(&mut peer, b, &[]);
}

#[test]
fn live_retry_uses_the_receiving_audio_observation_for_same_id_peers() {
    struct Observation {
        available: bool,
        playing: bool,
        permanent: bool,
        retry_frames: u32,
    }
    impl crate::game_logic::NativeAudioEffects for Observation {
        fn ambient_retry_frames(&mut self) -> Option<u32> {
            self.available.then_some(self.retry_frames)
        }
        fn ambient_retry_observation(
            &mut self,
            _object_id: u32,
            _name: &str,
        ) -> Option<crate::game_logic::AmbientAudioRetryObservation> {
            self.available
                .then_some(crate::game_logic::AmbientAudioRetryObservation {
                    is_playing: self.playing,
                    is_permanent: (!self.playing).then_some(self.permanent),
                })
        }
        fn add_audio_event(
            &mut self,
            _event: &game_engine::common::audio::AudioEventRts,
            _locality: &game_engine::common::audio::AudioSubmissionLocality,
        ) {
        }
    }

    let mut receiving = world("SoundAmbient = SharedReceivingLoop");
    let mut peer = world("SoundAmbient = SharedReceivingLoop");
    let id = spawn(&mut receiving);
    assert_eq!(id, spawn(&mut peer));
    assert_cues(&mut receiving, id, &[("SharedReceivingLoop", true)]);
    assert_cues(&mut peer, id, &[("SharedReceivingLoop", true)]);
    receiving.frame = 100;
    peer.frame = 100;

    let mut receiving_audio = Observation {
        available: true,
        playing: true,
        permanent: true,
        retry_frames: 30,
    };
    let own_events =
        receiving.process_audio_events_with_receiving_audio(None, Some(&mut receiving_audio));
    assert!(own_events.is_empty(), "its active loop suppresses a retry");

    let mut peer_audio = Observation {
        available: true,
        playing: false,
        permanent: true,
        retry_frames: 30,
    };
    let peer_events = peer.process_audio_events_with_receiving_audio(None, Some(&mut peer_audio));
    assert_eq!(
        peer_events.len(),
        1,
        "the receiving peer retries its culled loop"
    );
    let (event, _) = peer_events.into_iter().next().unwrap().into_parts();
    assert_eq!(event.event_type, "SharedReceivingLoop");
    assert_eq!(event.object_id, Some(id));
    assert!(event.is_looping);

    let mut unavailable = world("SoundAmbient = SharedReceivingLoop");
    let unavailable_id = spawn(&mut unavailable);
    assert_eq!(unavailable_id, id);
    assert_cues(&mut unavailable, id, &[("SharedReceivingLoop", true)]);
    unavailable.frame = 100;
    assert!(
        unavailable
            .process_audio_events_with_receiving_audio(None, None)
            .is_empty(),
        "missing receiver cannot imply a culled loop"
    );
    let mut unavailable_audio = Observation {
        available: false,
        playing: false,
        permanent: true,
        retry_frames: 30,
    };
    assert!(
        unavailable
            .process_audio_events_with_receiving_audio(None, Some(&mut unavailable_audio))
            .is_empty(),
        "unavailable observation cannot imply a culled loop"
    );
}
