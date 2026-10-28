//! Real receiving Drawable and AudioManager operations; no global audio fixture.
use super::*;
use game_engine::common::audio::audio_event_rts::{AC_LOOP, ST_GLOBAL};
use game_engine::common::audio::game_audio::AudioLocalityRelationship;
use game_engine::common::audio::{AudioEventInfo, AudioLocalityResolver, SoundPlaybackHook};
use game_engine::common::ini::INILoadType;
use game_engine::common::name_key_generator::NameKeyNamespace;
use game_engine::common::thing::module_factory::ModuleFactory;
use game_engine::common::thing::receiving_module_content::ReceivingModuleContent;
use game_engine::common::thing::thing_template::ThingTemplateAudioType;

struct Players;
impl AudioLocalityResolver for Players {
    fn get_local_player_index(&self) -> Option<i32> {
        Some(1)
    }
    fn is_player_active(&self, _: i32) -> bool {
        true
    }
    fn player_exists(&self, _: i32) -> bool {
        true
    }
    fn has_default_team(&self, _: i32) -> bool {
        true
    }
    fn get_relationship_to_local_team(&self, _: i32, _: i32) -> AudioLocalityRelationship {
        AudioLocalityRelationship::Allies
    }
}
fn make_actor(id: u32) -> BasicDrawable {
    // Ambient slots are scalar Object content, not module-only admission data.
    let mut content =
        ReceivingModuleContent::new(ModuleFactory::new_receiving(NameKeyNamespace::new()));
    content
        .load_ini_text(
            "Object OwnedAmbient\nSoundAmbient = OwnedPristine\nSoundAmbientDamaged = OwnedDamaged\nSoundAmbientRubble = OwnedRubble\nEnd\n",
            INILoadType::Overwrite,
        )
        .unwrap();
    let template = content.admitted_template("OwnedAmbient").unwrap();
    assert_eq!(
        template
            .audio_event(ThingTemplateAudioType::SoundAmbient)
            .unwrap()
            .get_event_name(),
        "OwnedPristine"
    );
    let mut actor = BasicDrawable::new(DrawableId(id));
    assert!(
        actor.initialize_native_constructor_template(Some("OwnedAmbient".into()), Some(template))
    );
    actor
}
fn make_audio() -> AudioManager {
    let mut audio = AudioManager::new();
    audio.init();
    for name in ["OwnedPristine", "OwnedDamaged", "OwnedRubble"] {
        audio.register_audio_event_info(AudioEventInfo {
            audio_name: name.into(),
            sounds: vec!["owned_sample".into()],
            type_field: ST_GLOBAL,
            control: AC_LOOP,
            loop_count: 0,
            ..Default::default()
        });
    }
    audio
}

#[derive(Clone)]
struct RecordingPlayback(std::sync::Arc<std::sync::Mutex<Vec<(char, u32)>>>);
impl SoundPlaybackHook for RecordingPlayback {
    fn play(&self, event: &AudioEventRts) -> Result<(), String> {
        self.0
            .lock()
            .unwrap()
            .push(('P', event.get_playing_handle()));
        Ok(())
    }
    fn stop(&self, handle: u32) {
        self.0.lock().unwrap().push(('S', handle));
    }
    fn pause(&self, _: u32) {}
    fn resume(&self, _: u32) {}
    fn is_playing(&self, _: u32) -> bool {
        true
    }
}

fn make_recording_audio(
    events: std::sync::Arc<std::sync::Mutex<Vec<(char, u32)>>>,
) -> AudioManager {
    let mut audio = AudioManager::with_playback_hook(Box::new(RecordingPlayback(events)));
    audio.init();
    audio.register_audio_event_info(AudioEventInfo {
        audio_name: "OwnedPristine".into(),
        sounds: vec!["owned_sample".into()],
        type_field: ST_GLOBAL,
        control: AC_LOOP,
        loop_count: 0,
        ..Default::default()
    });
    audio
}

#[test]
fn native_retirement_queues_stop_on_the_bound_application_audio_owner() {
    let events_a = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
    let events_b = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
    let device_a = std::sync::Arc::new(std::sync::Mutex::new(make_recording_audio(
        std::sync::Arc::clone(&events_a),
    )));
    let device_b = std::sync::Arc::new(std::sync::Mutex::new(make_recording_audio(
        std::sync::Arc::clone(&events_b),
    )));
    let locality = AudioSubmissionLocality::from_resolver(1, &Players);
    let mut actor_a = make_actor(17);
    let mut actor_b = make_actor(17);
    for (actor, device, tod) in [
        (&mut actor_a, &device_a, TimeOfDay::Night),
        (&mut actor_b, &device_b, TimeOfDay::Morning),
    ] {
        let mut audio = device.lock().unwrap();
        actor
            .finish_native_constructor_ambient_with_audio(false, false, tod, &mut audio, &locality)
            .unwrap();
        audio.process_request_list();
    }
    let handle_a = actor_a
        .ambient_sound_event
        .as_ref()
        .unwrap()
        .get_playing_handle();
    let handle_b = actor_b
        .ambient_sound_event
        .as_ref()
        .unwrap()
        .get_playing_handle();
    assert_eq!(
        handle_a, handle_b,
        "peer audio managers reuse local handles"
    );

    actor_a
        .finish_native_retirement_with_device(Some(&device_a))
        .unwrap();
    assert_eq!(events_a.lock().unwrap().as_slice(), &[('P', handle_a)]);
    device_a.lock().unwrap().process_request_list();

    assert_eq!(
        events_a.lock().unwrap().as_slice(),
        &[('P', handle_a), ('S', handle_a)],
        "Drawable retirement must enqueue and deliver its stop through its bound owner"
    );
    assert_eq!(
        events_b.lock().unwrap().as_slice(),
        &[('P', handle_b)],
        "same-ID/same-handle peer audio stays alive"
    );
    assert!(actor_b.ambient_sound_event.is_some());
}
#[test]
fn receiving_body_transition_and_snapshot_restamps_have_distinct_audio_effects() {
    let mut actor = make_actor(17);
    let mut peer = make_actor(17);
    let mut audio = make_audio();
    let mut peer_audio = make_audio();
    let locality = AudioSubmissionLocality::from_resolver(1, &Players);
    actor
        .finish_native_constructor_ambient_with_audio(
            false,
            false,
            TimeOfDay::Night,
            &mut audio,
            &locality,
        )
        .unwrap();
    peer.finish_native_constructor_ambient_with_audio(
        false,
        false,
        TimeOfDay::Morning,
        &mut peer_audio,
        &locality,
    )
    .unwrap();
    assert_eq!(audio.pending_play_request_count_for("OwnedPristine"), 1);
    assert_eq!(
        actor
            .ambient_sound_event
            .as_ref()
            .unwrap()
            .get_time_of_day(),
        game_engine::common::audio::audio_event_rts::TimeOfDay::Night
    );
    assert_eq!(
        peer.ambient_sound_event.as_ref().unwrap().get_time_of_day(),
        game_engine::common::audio::audio_event_rts::TimeOfDay::Morning
    );
    let original = actor
        .ambient_sound_event
        .as_ref()
        .unwrap()
        .get_playing_handle();
    let peer_handle = peer
        .ambient_sound_event
        .as_ref()
        .unwrap()
        .get_playing_handle();
    for _ in 0..120 {
        actor.react_to_body_damage_state_change(BodyDamageType::Damaged);
    }
    assert_eq!(
        actor
            .ambient_sound_event
            .as_ref()
            .unwrap()
            .get_playing_handle(),
        original
    );
    assert_eq!(audio.pending_play_request_count_for("OwnedDamaged"), 0);
    actor
        .react_to_body_damage_state_change_with_audio(
            BodyDamageType::Damaged,
            false,
            TimeOfDay::Night,
            &mut audio,
            &locality,
        )
        .unwrap();
    assert_eq!(
        actor.ambient_sound_event.as_ref().unwrap().get_event_name(),
        "OwnedDamaged"
    );
    assert_eq!(audio.pending_play_request_count_for("OwnedDamaged"), 1);
    assert_eq!(
        peer.ambient_sound_event
            .as_ref()
            .unwrap()
            .get_playing_handle(),
        peer_handle
    );
    assert_eq!(peer_audio.pending_play_request_count_for("OwnedDamaged"), 0);
    let before = actor
        .ambient_sound_event
        .as_ref()
        .unwrap()
        .get_playing_handle();
    actor
        .react_to_body_damage_state_change_with_audio(
            BodyDamageType::Rubble,
            true,
            TimeOfDay::Night,
            &mut audio,
            &locality,
        )
        .unwrap();
    assert_eq!(
        actor
            .ambient_sound_event
            .as_ref()
            .unwrap()
            .get_playing_handle(),
        before
    );
    assert_eq!(audio.pending_play_request_count_for("OwnedRubble"), 0);
    actor.stop_ambient_sound_with_audio(&mut audio);
    assert!(actor.ambient_sound_event.is_none());
}
#[test]
fn explicit_constructor_load_gates_and_missing_resources_do_not_invent_audio() {
    let locality = AudioSubmissionLocality::from_resolver(1, &Players);
    for (loading, saved) in [(true, false), (false, true), (true, true)] {
        let mut actor = make_actor(19);
        let mut audio = make_audio();
        actor
            .finish_native_constructor_ambient_with_audio(
                loading,
                saved,
                TimeOfDay::Afternoon,
                &mut audio,
                &locality,
            )
            .unwrap();
        assert!(actor.ambient_sound_event.is_none());
        assert_eq!(audio.pending_play_request_count(), 0);
    }
    let mut actor = make_actor(20);
    let mut unavailable = AudioManager::new();
    actor
        .react_to_body_damage_state_change_with_audio(
            BodyDamageType::Damaged,
            false,
            TimeOfDay::Night,
            &mut unavailable,
            &locality,
        )
        .unwrap();
    assert!(
        actor.ambient_sound_event.is_none(),
        "missing resource is not owner failure"
    );
}

#[test]
fn explicit_constructor_audio_preserves_ambient_enable_gates() {
    let locality = AudioSubmissionLocality::from_resolver(1, &Players);
    let mut actor = make_actor(21);
    actor.ambient_sound_enabled_from_script = false;
    let mut audio = make_audio();
    actor
        .finish_native_constructor_ambient_with_audio(
            false,
            false,
            TimeOfDay::Night,
            &mut audio,
            &locality,
        )
        .unwrap();
    assert!(actor.ambient_sound_event.is_none());
    assert_eq!(audio.pending_play_request_count(), 0);
}
#[test]
fn receiving_state_fallback_rubble_and_custom_off_follow_original_selection() {
    let mut actor = make_actor(21);
    let mut audio = make_audio();
    let locality = AudioSubmissionLocality::from_resolver(1, &Players);
    actor
        .start_ambient_sound_for_state_with_audio(
            BodyDamageType::ReallyDamaged,
            TimeOfDay::Evening,
            false,
            &mut audio,
            &locality,
        )
        .unwrap();
    assert_eq!(
        actor.ambient_sound_event.as_ref().unwrap().get_event_name(),
        "OwnedPristine"
    );
    actor.custom_sound_ambient_off = true;
    actor
        .start_ambient_sound_for_state_with_audio(
            BodyDamageType::Damaged,
            TimeOfDay::Evening,
            false,
            &mut audio,
            &locality,
        )
        .unwrap();
    assert!(actor.ambient_sound_event.is_none());
    actor
        .start_ambient_sound_for_state_with_audio(
            BodyDamageType::Rubble,
            TimeOfDay::Evening,
            false,
            &mut audio,
            &locality,
        )
        .unwrap();
    assert_eq!(
        actor.ambient_sound_event.as_ref().unwrap().get_event_name(),
        "OwnedRubble"
    );
}
#[test]
fn valid_nonpermanent_or_out_of_range_events_remain_owned_without_play() {
    let locality = AudioSubmissionLocality::from_resolver(1, &Players);
    let mut actor = make_actor(22);
    let mut audio = AudioManager::new();
    audio.init();
    audio.register_audio_event_info(AudioEventInfo {
        audio_name: "OwnedPristine".into(),
        sounds: vec!["one_shot".into()],
        type_field: ST_GLOBAL,
        ..Default::default()
    });
    actor
        .start_ambient_sound_for_state_with_audio(
            BodyDamageType::Pristine,
            TimeOfDay::Afternoon,
            true,
            &mut audio,
            &locality,
        )
        .unwrap();
    assert!(
        actor.ambient_sound_event.is_some(),
        "C++ retains nonpermanent event when onlyIfPermanent skips play"
    );
    assert_eq!(audio.pending_play_request_count(), 0);
    audio.register_audio_event_info(AudioEventInfo {
        audio_name: "OwnedPristine".into(),
        sounds: vec!["distant".into()],
        type_field: 0,
        max_distance: 0.0,
        ..Default::default()
    });
    actor
        .start_ambient_sound_for_state_with_audio(
            BodyDamageType::Pristine,
            TimeOfDay::Afternoon,
            false,
            &mut audio,
            &locality,
        )
        .unwrap();
    assert!(
        actor.ambient_sound_event.is_some(),
        "range boundary skips play, not event ownership"
    );
    assert_eq!(audio.pending_play_request_count(), 0);
}
