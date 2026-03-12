use std::io::Cursor;

use wp_audio::{
    logical::list::LogicalSoundRegistry,
    logical_listener::LogicalListener,
    math::Vector3,
    save_load::{
        DynamicAudioSaveLoad, MemorySourceRecord, SavedLogicalRecord, SavedMixerVoiceRecord,
        SavedSoundRecord,
    },
    sound_scene::SoundScene,
    SoundClassId, VoiceHandle, VoicePlaybackState, VoiceSpatialMode,
};

fn legacy_dynamic_sample_bytes() -> Vec<u8> {
    const CHUNKID_DYNAMIC: u32 = 0x5741_4430; // 'WAD0'
    const CHUNKID_DYNAMIC_VARIABLES: u32 = 0x1029_1222;
    const CHUNKID_DYNAMIC_SCENE: u32 = 0x1029_1221;
    const CHUNKID_DYNAMIC_MIXER_STATE: u32 = 0x0003_0200;
    const CHUNKID_DYNAMIC_SOUND_ENTRY: u32 = 0x0003_0201;
    const CHUNKID_DYNAMIC_LOGICAL_ENTRY: u32 = 0x0003_0202;

    const VARID_LOGICAL_LISTENER_GLOBAL_SCALE: u8 = 0x04;
    const VARID_MIXER_SEQUENCE: u8 = 0x01;
    const VARID_MIXER_FRAME: u8 = 0x02;
    const VARID_MIXER_SAMPLE_RATE: u8 = 0x03;
    const VARID_SOUND_ID: u8 = 0x01;
    const VARID_SOUND_CLASS: u8 = 0x02;
    const VARID_SOUND_POSITION: u8 = 0x03;
    const VARID_SOUND_PRIORITY: u8 = 0x04;
    const VARID_SOUND_DROPOFF: u8 = 0x05;
    const VARID_SOUND_VOLUME: u8 = 0x06;
    const VARID_SOUND_PAN: u8 = 0x07;
    const VARID_SOUND_LOOP_COUNT: u8 = 0x08;
    const VARID_SOUND_PLAYBACK_RATE: u8 = 0x09;
    const VARID_SOUND_START_FRAME: u8 = 0x0A;
    const VARID_LOGICAL_ID: u8 = 0x01;
    const VARID_LOGICAL_TYPE_MASK: u8 = 0x02;
    const VARID_LOGICAL_POSITION: u8 = 0x03;
    const VARID_LOGICAL_DROPOFF: u8 = 0x04;
    const VARID_LOGICAL_SINGLE_SHOT: u8 = 0x05;
    const VARID_LOGICAL_MAX_LISTENERS: u8 = 0x06;
    const VARID_LOGICAL_NOTIFY_DELAY: u8 = 0x07;
    const VARID_LOGICAL_LAST_NOTIFY: u8 = 0x08;
    const VARID_LOGICAL_LISTENER_TIMESTAMP: u8 = 0x09;
    const VARID_LOGICAL_DISPLAY_NAME: u8 = 0x0A;

    fn micro(id: u8, payload: &[u8]) -> Vec<u8> {
        let mut out = Vec::with_capacity(2 + payload.len());
        out.push(id);
        out.push(payload.len() as u8);
        out.extend_from_slice(payload);
        out
    }

    fn chunk(id: u32, payload: &[u8], has_children: bool) -> Vec<u8> {
        let mut out = Vec::with_capacity(8 + payload.len());
        out.extend_from_slice(&id.to_le_bytes());
        let mut size = payload.len() as u32;
        if has_children {
            size |= 0x8000_0000;
        }
        out.extend_from_slice(&size.to_le_bytes());
        out.extend_from_slice(payload);
        out
    }

    let variables_payload = micro(VARID_LOGICAL_LISTENER_GLOBAL_SCALE, &1.0_f32.to_le_bytes());
    let variables_chunk = chunk(CHUNKID_DYNAMIC_VARIABLES, &variables_payload, false);

    let mut mixer_payload = Vec::new();
    mixer_payload.extend(micro(VARID_MIXER_SEQUENCE, &5_u32.to_le_bytes()));
    mixer_payload.extend(micro(VARID_MIXER_FRAME, &123_456_789_u64.to_le_bytes()));
    mixer_payload.extend(micro(VARID_MIXER_SAMPLE_RATE, &44_100_u32.to_le_bytes()));
    let mixer_chunk = chunk(CHUNKID_DYNAMIC_MIXER_STATE, &mixer_payload, false);

    let mut sound_payload = Vec::new();
    sound_payload.extend(micro(VARID_SOUND_ID, &42_u32.to_le_bytes()));
    sound_payload.extend(micro(
        VARID_SOUND_CLASS,
        &(SoundClassId::ThreeD as u32).to_le_bytes(),
    ));
    let mut position_bytes = Vec::new();
    position_bytes.extend_from_slice(&1.0_f32.to_le_bytes());
    position_bytes.extend_from_slice(&2.0_f32.to_le_bytes());
    position_bytes.extend_from_slice(&3.0_f32.to_le_bytes());
    sound_payload.extend(micro(VARID_SOUND_POSITION, &position_bytes));
    sound_payload.extend(micro(VARID_SOUND_PRIORITY, &0.75_f32.to_le_bytes()));
    sound_payload.extend(micro(VARID_SOUND_DROPOFF, &10.0_f32.to_le_bytes()));
    sound_payload.extend(micro(VARID_SOUND_VOLUME, &0.5_f32.to_le_bytes()));
    sound_payload.extend(micro(VARID_SOUND_PAN, &100_i32.to_le_bytes()));
    sound_payload.extend(micro(VARID_SOUND_LOOP_COUNT, &3_u32.to_le_bytes()));
    sound_payload.extend(micro(VARID_SOUND_PLAYBACK_RATE, &44_100_u32.to_le_bytes()));
    sound_payload.extend(micro(VARID_SOUND_START_FRAME, &3_456_u64.to_le_bytes()));
    let sound_chunk = chunk(CHUNKID_DYNAMIC_SOUND_ENTRY, &sound_payload, false);

    let mut logical_payload = Vec::new();
    logical_payload.extend(micro(VARID_LOGICAL_ID, &7_u32.to_le_bytes()));
    logical_payload.extend(micro(VARID_LOGICAL_TYPE_MASK, &0x5_u32.to_le_bytes()));
    let mut logical_position = Vec::new();
    logical_position.extend_from_slice(&4.0_f32.to_le_bytes());
    logical_position.extend_from_slice(&5.0_f32.to_le_bytes());
    logical_position.extend_from_slice(&6.0_f32.to_le_bytes());
    logical_payload.extend(micro(VARID_LOGICAL_POSITION, &logical_position));
    logical_payload.extend(micro(VARID_LOGICAL_DROPOFF, &20.0_f32.to_le_bytes()));
    logical_payload.extend(micro(VARID_LOGICAL_SINGLE_SHOT, &[1]));
    logical_payload.extend(micro(VARID_LOGICAL_MAX_LISTENERS, &2_u32.to_le_bytes()));
    logical_payload.extend(micro(VARID_LOGICAL_NOTIFY_DELAY, &500_u32.to_le_bytes()));
    logical_payload.extend(micro(VARID_LOGICAL_LAST_NOTIFY, &1_000_u32.to_le_bytes()));
    logical_payload.extend(micro(
        VARID_LOGICAL_LISTENER_TIMESTAMP,
        &123_u32.to_le_bytes(),
    ));
    logical_payload.extend(micro(VARID_LOGICAL_DISPLAY_NAME, b"Alert"));
    let logical_chunk = chunk(CHUNKID_DYNAMIC_LOGICAL_ENTRY, &logical_payload, false);

    let mut scene_payload = Vec::new();
    scene_payload.extend_from_slice(&mixer_chunk);
    scene_payload.extend_from_slice(&sound_chunk);
    scene_payload.extend_from_slice(&logical_chunk);
    let scene_chunk = chunk(CHUNKID_DYNAMIC_SCENE, &scene_payload, true);

    let mut payload = Vec::new();
    payload.extend_from_slice(&variables_chunk);
    payload.extend_from_slice(&scene_chunk);

    let mut top = Vec::new();
    top.extend_from_slice(&CHUNKID_DYNAMIC.to_le_bytes());
    top.extend_from_slice(&(payload.len() as u32).to_le_bytes());
    top.extend_from_slice(&payload);
    top
}

fn approx_eq(left: f32, right: f32) {
    const EPS: f32 = 1e-6;
    assert!((left - right).abs() <= EPS, "{left} != {right}");
}

#[test]
fn dynamic_audio_round_trips_legacy_sample() {
    let sample = legacy_dynamic_sample_bytes();
    let mut loader = DynamicAudioSaveLoad::default();
    loader
        .load(Cursor::new(sample.as_slice()))
        .expect("legacy sample should load");

    approx_eq(loader.logical_listener_global_scale(), 1.0);

    let snapshot = loader.mixer_snapshot().expect("snapshot should be present");
    assert_eq!(snapshot.sequence, 5);
    assert_eq!(snapshot.current_frame, 123_456_789);
    assert_eq!(snapshot.sample_rate, 44_100);

    assert_eq!(loader.loaded_dynamic_sounds().len(), 1);
    let sound: &SavedSoundRecord = &loader.loaded_dynamic_sounds()[0];
    assert_eq!(sound.id, 42);
    assert_eq!(sound.class_id, SoundClassId::ThreeD);
    approx_eq(sound.position.x, 1.0);
    approx_eq(sound.position.y, 2.0);
    approx_eq(sound.position.z, 3.0);
    approx_eq(sound.priority, 0.75);
    approx_eq(sound.dropoff_radius, 10.0);
    approx_eq(sound.volume, 0.5);
    assert_eq!(sound.pan, 100);
    assert_eq!(sound.loop_count, 3);
    assert_eq!(sound.playback_rate, 44_100);
    assert_eq!(sound.start_frame, 3_456);

    assert_eq!(loader.loaded_logical_sounds().len(), 1);
    let logical: &SavedLogicalRecord = &loader.loaded_logical_sounds()[0];
    assert_eq!(logical.id, 7);
    assert_eq!(logical.type_mask, 0x5);
    approx_eq(logical.position.x, 4.0);
    approx_eq(logical.position.y, 5.0);
    approx_eq(logical.position.z, 6.0);
    approx_eq(logical.dropoff_radius, 20.0);
    assert!(logical.is_single_shot);
    assert_eq!(logical.max_listeners, 2);
    assert_eq!(logical.notify_delay_ms, 500);
    assert_eq!(logical.last_notification_ms, 1_000);
    assert_eq!(logical.listener_timestamp, 123);
    assert_eq!(logical.display_name.as_deref(), Some("Alert"));

    LogicalListener::set_global_scale(loader.logical_listener_global_scale());

    let mut scene = SoundScene::new();
    for record in loader.loaded_dynamic_sounds() {
        scene.dynamic_sounds.push(record.instantiate());
    }

    let mut registry = LogicalSoundRegistry::new();
    for record in loader.loaded_logical_sounds() {
        scene.logical_sounds.push(record.instantiate());
        registry.register(record.id, record.type_mask, record.display_name.clone());
    }

    let mut saver = DynamicAudioSaveLoad::default();
    if let Some(snapshot) = loader.mixer_snapshot() {
        saver.set_mixer_snapshot(snapshot);
    }

    let mut cursor = Cursor::new(Vec::new());
    saver
        .save(&scene, &registry, &[], &mut cursor)
        .expect("save succeeds");
    let round_trip = cursor.into_inner();
    assert_eq!(
        round_trip, sample,
        "serialized bytes should match legacy sample"
    );
}

#[test]
fn dynamic_audio_serializes_voice_records() {
    let scene = SoundScene::new();
    let registry = LogicalSoundRegistry::new();
    let mut saver = DynamicAudioSaveLoad::default();

    let mut voice = SavedMixerVoiceRecord::default();
    voice.handle = VoiceHandle::new(99, 4);
    voice.channel_id = 7;
    voice.miles_handle_id = Some(12345);
    voice.sound_id = Some(42);
    voice.source_identifier = Some("Data/Sounds/test.wav".to_string());
    voice.params.gain = 0.65;
    voice.params.pan = -0.4;
    voice.params.playback_rate = 22_050;
    voice.params.loop_count = 2;
    voice.params.start_frame = 512;
    voice.params.is_culled = true;
    voice.params.spatial.mode = VoiceSpatialMode::Pseudo3D;
    voice.params.spatial.position = Vector3::new(1.0, 2.0, 3.0);
    voice.params.spatial.velocity = Vector3::new(0.1, 0.2, 0.3);
    voice.params.spatial.listener_position = Vector3::new(4.0, 5.0, 6.0);
    voice.params.spatial.listener_velocity = Vector3::new(0.4, 0.5, 0.6);
    voice.params.spatial.listener_right = Vector3::new(0.0, 1.0, 0.0);
    voice.params.spatial.listener_up = Vector3::new(0.0, 0.0, 1.0);
    voice.params.spatial.listener_forward = Vector3::new(1.0, 0.0, 0.0);
    voice.params.spatial.min_distance = 3.5;
    voice.params.spatial.max_distance = 12.0;
    voice.timeline.position_frames = 2_048.0;
    voice.timeline.rendered_frames = 1_024;
    voice.timeline.timeline_origin = 256;
    voice.timeline.last_sequence = 42;
    voice.timeline.source_rate = 44_100;
    voice.playback_state = VoicePlaybackState::Playing;

    let mut cursor = Cursor::new(Vec::new());
    saver
        .save(&scene, &registry, &[voice.clone()], &mut cursor)
        .expect("voice save");
    let bytes = cursor.into_inner();

    let mut loader = DynamicAudioSaveLoad::default();
    loader
        .load(Cursor::new(bytes.as_slice()))
        .expect("voice load");

    assert_eq!(loader.loaded_mixer_voices().len(), 1);
    let loaded = &loader.loaded_mixer_voices()[0];
    assert_eq!(loaded.handle, voice.handle);
    assert_eq!(loaded.channel_id, voice.channel_id);
    assert_eq!(loaded.miles_handle_id, voice.miles_handle_id);
    assert_eq!(loaded.sound_id, voice.sound_id);
    assert_eq!(loaded.source_identifier, voice.source_identifier);
    approx_eq(loaded.params.gain, voice.params.gain);
    approx_eq(loaded.params.pan, voice.params.pan);
    assert_eq!(loaded.params.playback_rate, voice.params.playback_rate);
    assert_eq!(loaded.params.loop_count, voice.params.loop_count);
    assert_eq!(loaded.params.start_frame, voice.params.start_frame);
    assert_eq!(loaded.params.is_culled, voice.params.is_culled);
    assert_eq!(loaded.params.spatial.mode, voice.params.spatial.mode);
    approx_eq(
        loaded.params.spatial.position.x,
        voice.params.spatial.position.x,
    );
    approx_eq(
        loaded.params.spatial.position.y,
        voice.params.spatial.position.y,
    );
    approx_eq(
        loaded.params.spatial.position.z,
        voice.params.spatial.position.z,
    );
    approx_eq(
        loaded.params.spatial.velocity.x,
        voice.params.spatial.velocity.x,
    );
    approx_eq(
        loaded.params.spatial.velocity.y,
        voice.params.spatial.velocity.y,
    );
    approx_eq(
        loaded.params.spatial.velocity.z,
        voice.params.spatial.velocity.z,
    );
    approx_eq(
        loaded.params.spatial.listener_position.x,
        voice.params.spatial.listener_position.x,
    );
    approx_eq(
        loaded.params.spatial.listener_position.y,
        voice.params.spatial.listener_position.y,
    );
    approx_eq(
        loaded.params.spatial.listener_position.z,
        voice.params.spatial.listener_position.z,
    );
    approx_eq(
        loaded.params.spatial.listener_velocity.x,
        voice.params.spatial.listener_velocity.x,
    );
    approx_eq(
        loaded.params.spatial.listener_velocity.y,
        voice.params.spatial.listener_velocity.y,
    );
    approx_eq(
        loaded.params.spatial.listener_velocity.z,
        voice.params.spatial.listener_velocity.z,
    );
    approx_eq(
        loaded.params.spatial.listener_right.x,
        voice.params.spatial.listener_right.x,
    );
    approx_eq(
        loaded.params.spatial.listener_right.y,
        voice.params.spatial.listener_right.y,
    );
    approx_eq(
        loaded.params.spatial.listener_right.z,
        voice.params.spatial.listener_right.z,
    );
    approx_eq(
        loaded.params.spatial.listener_up.x,
        voice.params.spatial.listener_up.x,
    );
    approx_eq(
        loaded.params.spatial.listener_up.y,
        voice.params.spatial.listener_up.y,
    );
    approx_eq(
        loaded.params.spatial.listener_up.z,
        voice.params.spatial.listener_up.z,
    );
    approx_eq(
        loaded.params.spatial.listener_forward.x,
        voice.params.spatial.listener_forward.x,
    );
    approx_eq(
        loaded.params.spatial.listener_forward.y,
        voice.params.spatial.listener_forward.y,
    );
    approx_eq(
        loaded.params.spatial.listener_forward.z,
        voice.params.spatial.listener_forward.z,
    );
    approx_eq(
        loaded.params.spatial.min_distance,
        voice.params.spatial.min_distance,
    );
    approx_eq(
        loaded.params.spatial.max_distance,
        voice.params.spatial.max_distance,
    );
    assert_eq!(
        loaded.timeline.rendered_frames,
        voice.timeline.rendered_frames
    );
    assert_eq!(
        loaded.timeline.timeline_origin,
        voice.timeline.timeline_origin
    );
    assert_eq!(loaded.timeline.last_sequence, voice.timeline.last_sequence);
    assert_eq!(loaded.timeline.source_rate, voice.timeline.source_rate);
    approx_eq(
        loaded.timeline.position_frames as f32,
        voice.timeline.position_frames as f32,
    );
    assert_eq!(loaded.playback_state, voice.playback_state);
}

#[test]
fn dynamic_audio_serializes_memory_sources() {
    let scene = SoundScene::new();
    let registry = LogicalSoundRegistry::new();
    let mut saver = DynamicAudioSaveLoad::default();

    let mut voice = SavedMixerVoiceRecord::default();
    voice.handle = VoiceHandle::new(11, 2);
    voice.channel_id = 5;
    voice.source_identifier = Some("<memory>".to_string());
    voice.memory_source = Some(MemorySourceRecord {
        data: (0..300).map(|v| v as u8).collect(),
        channels: 1,
        sample_rate: 22_050,
        sample_width: 16,
    });

    let mut cursor = Cursor::new(Vec::new());
    saver
        .save(&scene, &registry, &[voice.clone()], &mut cursor)
        .expect("memory voice save");

    let mut loader = DynamicAudioSaveLoad::default();
    loader
        .load(Cursor::new(cursor.into_inner().as_slice()))
        .expect("memory voice load");

    assert_eq!(loader.loaded_mixer_voices().len(), 1);
    let loaded = &loader.loaded_mixer_voices()[0];
    let memory = loaded
        .memory_source
        .as_ref()
        .expect("memory source present");
    assert_eq!(memory.channels, 1);
    assert_eq!(memory.sample_rate, 22_050);
    assert_eq!(memory.sample_width, 16);
    assert_eq!(memory.data.len(), 300);
    assert_eq!(
        &memory.data[..],
        &voice.memory_source.as_ref().unwrap().data[..]
    );
}
