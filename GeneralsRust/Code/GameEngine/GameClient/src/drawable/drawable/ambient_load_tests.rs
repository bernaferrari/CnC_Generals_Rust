//! CPP Drawable.cpp:5330-5355 reconstructs inherited audio before saved overrides.
use super::*;
use game_engine::common::audio::audio_event_rts::AudioEventInfo;
use game_engine::common::system::xfer_load::XferLoad;
use game_engine::common::system::xfer_save::XferSave;
use std::io::Cursor;

fn definition() -> AudioEventInfo {
    AudioEventInfo {
        audio_name: "OwnedAmbientBase".into(),
        control: 1,
        loop_count: 0,
        sounds: vec!["OwnedAmbientLoop".into()],
        ..AudioEventInfo::default()
    }
}
fn overrides(base: &AudioEventInfo) -> Vec<u8> {
    let mut saved = DynamicAudioEventInfo::from_base_info(base);
    // CPP custom definitions have already passed mangleCustomAudioName before
    // saving; xferNoName transfers that override bit, but not the name string.
    saved.override_audio_name(&format!(" CUSTOM 77 {}", base.audio_name));
    saved.override_volume(0.75);
    let mut bytes = Vec::new();
    let mut xfer = XferSave::new(Cursor::new(&mut bytes), 1);
    saved.xfer_no_name(&mut xfer).unwrap();
    bytes
}
#[test]
fn base_definition_preserves_unoverridden_permanent_loop_and_custom_name() {
    let base = definition();
    let mut load = XferLoad::new(Cursor::new(overrides(&base)), 1);
    let loaded = load_custom_ambient_info(&mut load, Some(&base), DrawableId(77))
        .unwrap()
        .unwrap();
    let info = loaded.get_audio_event_info();
    assert_eq!(info.audio_name, " CUSTOM 77 OwnedAmbientBase");
    assert_eq!(loaded.get_original_name(), "OwnedAmbientBase");
    assert_eq!((info.control, info.loop_count), (1, 0));
    assert_eq!(info.sounds, base.sounds);
    assert_eq!(info.volume, 0.75);
}
#[test]
fn missing_base_consumes_overrides_and_preserves_following_xfer_field() {
    let mut bytes = overrides(&definition());
    bytes.extend_from_slice(&0xC001_C0DEu32.to_le_bytes());
    let mut load = XferLoad::new(Cursor::new(bytes), 1);
    assert!(
        load_custom_ambient_info(&mut load, None, DrawableId(77))
            .unwrap()
            .is_none()
    );
    let mut following = 0;
    load.xfer_unsigned_int(&mut following).unwrap();
    assert_eq!(following, 0xC001_C0DE);
}
#[test]
fn malformed_override_does_not_produce_custom_definition() {
    let base = definition();
    let mut bytes = overrides(&base);
    bytes.pop();
    let mut load = XferLoad::new(Cursor::new(bytes), 1);
    assert!(load_custom_ambient_info(&mut load, Some(&base), DrawableId(77)).is_err());
    assert_eq!((base.control, base.loop_count), (1, 0));
    assert_eq!(base.audio_name, "OwnedAmbientBase");
}
#[test]
fn real_drawable_snapshot_missing_definition_suppresses_custom_restart() {
    let mut base = definition();
    base.audio_name = "__AbsentDrawableAmbient_hq6m33q".into();
    let mut saved = BasicDrawable::new(DrawableId(881));
    saved.custom_sound_ambient_base_name = Some(base.audio_name.clone());
    saved.custom_sound_ambient_dynamic_info = Some(DynamicAudioEventInfo::from_base_info(&base));
    saved.receives_dynamic_lights = true;
    let mut bytes = Vec::new();
    {
        let mut save = XferSave::new(Cursor::new(&mut bytes), 1);
        saved.xfer_snapshot(&mut save).unwrap();
        let mut following = 0xC001_C0DE;
        save.xfer_unsigned_int(&mut following).unwrap();
    }
    let mut loaded = BasicDrawable::new(DrawableId(0));
    let mut load = XferLoad::new(Cursor::new(bytes), 1);
    loaded.xfer_snapshot(&mut load).unwrap();
    let mut following = 0;
    load.xfer_unsigned_int(&mut following).unwrap();
    assert_eq!(following, 0xC001_C0DE);
    assert_eq!(loaded.id, DrawableId(881));
    assert!(loaded.receives_dynamic_lights);
    assert!(loaded.custom_sound_ambient_dynamic_info.is_none());
    assert!(loaded.custom_sound_ambient_base_name.is_none());
    assert!(!loaded.custom_sound_ambient_off);
    assert_eq!(loaded.ambient_sound_enabled, saved.ambient_sound_enabled);
    assert_eq!(
        loaded.ambient_sound_enabled_from_script,
        saved.ambient_sound_enabled_from_script
    );
    loaded.load_post_process().unwrap();
    assert!(!loaded.ambient_sound_is_active());
}
