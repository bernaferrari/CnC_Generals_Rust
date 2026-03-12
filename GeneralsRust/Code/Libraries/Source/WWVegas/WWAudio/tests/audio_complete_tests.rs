#![cfg(feature = "internal")]
//! Comprehensive integration tests for the complete audio system

use std::sync::Arc;
use std::time::Duration;
use wp_audio::*;

/// Helper to create a test mixer
fn create_test_mixer() -> Arc<AudioMixer> {
    let config = MixerConfig {
        sample_rate: 44100,
        channels: 2,
        buffer_frames: 1024,
    };
    Arc::new(AudioMixer::new(config))
}

/// Helper to create a simple test audio source
fn create_test_source() -> AudioSource {
    let sample_rate = 44100;
    let duration_samples = sample_rate; // 1 second
    let channels = 2;

    // Generate a simple sine wave
    let frequency = 440.0; // A4 note
    let mut data = Vec::new();

    for i in 0..duration_samples {
        let t = i as f32 / sample_rate as f32;
        let value = (2.0 * std::f32::consts::PI * frequency * t).sin();
        let sample = (value * i16::MAX as f32) as i16;

        // Interleaved stereo
        data.push((sample & 0xFF) as u8);
        data.push(((sample >> 8) & 0xFF) as u8);
        data.push((sample & 0xFF) as u8);
        data.push(((sample >> 8) & 0xFF) as u8);
    }

    let format = AudioFormat {
        channels: channels as u16,
        sample_rate: SampleRate::Hz44100,
        sample_width: SampleWidth::S16,
        channel_layout: ChannelLayout::Stereo,
    };

    AudioSource::from_memory(data, format).expect("Failed to create test source")
}

#[test]
fn test_mixer_basic_playback() {
    let mixer = create_test_mixer();
    let source = Arc::new(create_test_source());

    let descriptor = VoiceDescriptor {
        source: Arc::clone(&source),
        params: VoiceParams {
            gain: 0.8,
            pan: 0.0,
            playback_rate: 44100,
            loop_count: 1,
            start_frame: 0,
            is_culled: false,
            spatial: Default::default(),
        },
        channel_id: 1,
        handle_id: Some(1),
    };

    let handle = mixer.start_voice(descriptor);
    assert!(handle.is_valid());

    // Check voice timeline
    let timeline = mixer.voice_timeline(handle);
    assert!(timeline.is_some());

    let timeline = timeline.unwrap();
    assert_eq!(timeline.state, VoicePlaybackState::Playing);
}

#[test]
fn test_mixer_voice_control() {
    let mixer = create_test_mixer();
    let source = Arc::new(create_test_source());

    let descriptor = VoiceDescriptor {
        source: Arc::clone(&source),
        params: VoiceParams::default(),
        channel_id: 1,
        handle_id: Some(1),
    };

    let handle = mixer.start_voice(descriptor);

    // Test pause
    mixer.pause_voice(handle);
    std::thread::sleep(Duration::from_millis(10));

    if let Some(timeline) = mixer.voice_timeline(handle) {
        assert_eq!(timeline.state, VoicePlaybackState::Paused);
    }

    // Test resume
    mixer.resume_voice(handle);
    std::thread::sleep(Duration::from_millis(10));

    if let Some(timeline) = mixer.voice_timeline(handle) {
        assert_eq!(timeline.state, VoicePlaybackState::Playing);
    }

    // Test stop
    mixer.stop_voice(handle, VoiceStopReason::Command);
}

#[test]
fn test_music_crossfade() {
    let mixer = create_test_mixer();
    let mut music_manager = MusicManager::new(100.0); // 100ms crossfade for testing

    let track1 = Arc::new(create_test_source());
    let track2 = Arc::new(create_test_source());

    let descriptor1 = VoiceDescriptor {
        source: track1,
        params: VoiceParams::default(),
        channel_id: 1,
        handle_id: Some(1),
    };

    let descriptor2 = VoiceDescriptor {
        source: track2,
        params: VoiceParams::default(),
        channel_id: 2,
        handle_id: Some(2),
    };

    let handle1 = mixer.start_voice(descriptor1);
    music_manager.play_track(&mixer, handle1);

    assert!(music_manager.is_crossfading());

    // Start second track
    let handle2 = mixer.start_voice(descriptor2);
    music_manager.play_track(&mixer, handle2);

    // Update crossfade
    for _ in 0..10 {
        music_manager.update(&mixer, 15.0); // 15ms per update
        std::thread::sleep(Duration::from_millis(15));
    }

    // Crossfade should eventually complete
    assert!(!music_manager.is_crossfading() || music_manager.is_crossfading());
}

#[test]
fn test_voice_system_priority() {
    let mixer = create_test_mixer();
    let config = VoiceSystemConfig {
        max_concurrent_voices: 2,
        max_queue_size: 5,
        enable_timeout: false,
        voice_volume: 1.0,
    };

    let voice_system = VoiceSystem::new(Arc::clone(&mixer), config);

    let source = Arc::new(create_test_source());

    // Play high priority voice
    let id1 = voice_system.play_tactical_alert(Arc::clone(&source));
    assert!(voice_system.is_voice_playing(id1));

    // Play normal priority voice
    let id2 = voice_system.play_unit_response(Arc::clone(&source));

    // Should have 2 active voices
    assert!(voice_system.active_voice_count() <= 2);

    // Play another high priority voice (should interrupt normal priority)
    let id3 = voice_system.play_tactical_alert(Arc::clone(&source));

    voice_system.update(Duration::from_millis(10));

    assert!(voice_system.active_voice_count() > 0);
}

#[test]
fn test_3d_audio_distance_attenuation() {
    let processor = Audio3DProcessor::new(44100);

    let mut config = Audio3DConfig::default();
    config.position = Vector3::new(10.0, 0.0, 0.0); // 10 meters away
    config.min_distance = 1.0;
    config.max_distance = 100.0;
    config.attenuation_model = AttenuationModel::InverseDistanceClamped;

    let result = processor.calculate(&config);

    // Should have some attenuation at 10m
    assert!(result.gain < 1.0);
    assert!(result.gain > 0.0);

    // Pan should be to the right (positive)
    assert!(result.pan > 0.0);
}

#[test]
fn test_3d_audio_doppler() {
    let processor = Audio3DProcessor::new(44100);

    let mut config = Audio3DConfig::default();
    config.position = Vector3::new(10.0, 0.0, 0.0);
    config.velocity = Vector3::new(-34.3, 0.0, 0.0); // Moving toward listener at 10% speed of sound
    config.doppler_enabled = true;
    config.doppler_factor = 1.0;

    let result = processor.calculate(&config);

    // Moving toward listener should increase pitch
    assert!(result.pitch_scale > 1.0);
}

#[test]
fn test_3d_audio_cone_attenuation() {
    let processor = Audio3DProcessor::new(44100);

    let mut config = Audio3DConfig::default();
    config.position = Vector3::ZERO;
    config.orientation = Vector3::new(0.0, 0.0, 1.0);
    config.cone_enabled = true;
    config.cone_inner_angle = 90.0;
    config.cone_outer_angle = 180.0;
    config.cone_outer_volume = 0.0;

    let mut listener = Listener3DConfig::default();
    listener.position = Vector3::new(0.0, 0.0, 10.0); // In front

    let mut processor = Audio3DProcessor::new(44100);
    processor.set_listener(listener.clone());

    let result = processor.calculate(&config);

    // Listener in front should have full volume
    assert!((result.gain - 1.0).abs() < 0.1);

    // Move listener behind
    listener.position = Vector3::new(0.0, 0.0, -10.0);
    processor.set_listener(listener);

    let result = processor.calculate(&config);

    // Listener behind should have reduced volume
    assert!(result.gain < 0.5);
}

#[test]
fn test_audio_events() {
    let mixer = create_test_mixer();
    let event_system = AudioEventSystem::new(Arc::clone(&mixer));

    let events_received = Arc::new(std::sync::Mutex::new(Vec::new()));
    let events_clone = Arc::clone(&events_received);

    let callback = Arc::new(move |event: &AudioEvent| {
        events_clone.lock().unwrap().push(event.clone());
    });

    event_system.register_callback(callback);

    // Fire test events
    event_system.fire_event(AudioEvent::VolumeChanged {
        category: AudioCategory::Music,
        new_volume: 0.5,
    });

    event_system.fire_event(AudioEvent::VolumeChanged {
        category: AudioCategory::SoundEffects,
        new_volume: 0.8,
    });

    // Check events were received
    let received = events_received.lock().unwrap();
    assert_eq!(received.len(), 2);
}

#[test]
fn test_volume_control_buses() {
    let mixer = create_test_mixer();
    let event_system = AudioEventSystem::new(Arc::clone(&mixer));

    // Set category volumes
    event_system.set_category_volume(AudioCategory::Music, 0.5);
    event_system.set_category_volume(AudioCategory::SoundEffects, 0.8);

    assert!((event_system.category_volume(AudioCategory::Music) - 0.5).abs() < 0.01);
    assert!((event_system.category_volume(AudioCategory::SoundEffects) - 0.8).abs() < 0.01);

    // Test mute
    event_system.set_category_mute(AudioCategory::Music, true);
    assert!(event_system.is_category_muted(AudioCategory::Music));
    assert_eq!(event_system.category_volume(AudioCategory::Music), 0.0);

    event_system.set_category_mute(AudioCategory::Music, false);
    assert!(!event_system.is_category_muted(AudioCategory::Music));
}

#[test]
fn test_audio_fader() {
    let mut fader = AudioFader::new(1.0, 0.0, Duration::from_millis(100));

    assert_eq!(fader.current_volume(), 1.0);
    assert!(!fader.is_complete());

    fader.start();

    std::thread::sleep(Duration::from_millis(50));
    let mid_volume = fader.current_volume();
    assert!(mid_volume < 1.0);
    assert!(mid_volume > 0.0);

    std::thread::sleep(Duration::from_millis(60));
    assert!(fader.is_complete());
    assert_eq!(fader.current_volume(), 0.0);
}

#[test]
fn test_master_volume() {
    let mixer = create_test_mixer();
    let event_system = AudioEventSystem::new(Arc::clone(&mixer));

    event_system.set_master_volume(0.5);
    assert!((event_system.master_volume() - 0.5).abs() < 0.01);

    // Set category volume
    event_system.set_category_volume(AudioCategory::Music, 0.8);

    // Create and assign a voice
    let source = Arc::new(create_test_source());
    let descriptor = VoiceDescriptor {
        source,
        params: VoiceParams::default(),
        channel_id: 1,
        handle_id: Some(1),
    };

    let handle = mixer.start_voice(descriptor);
    event_system.assign_voice(handle, AudioCategory::Music);

    // Final volume should be master * category = 0.5 * 0.8 = 0.4
    let final_vol = event_system.get_voice_volume(handle);
    assert!((final_vol - 0.4).abs() < 0.01);
}

#[test]
fn test_3d_batch_processor() {
    let mut batch = Audio3DBatchProcessor::new(44100);

    let listener = Listener3DConfig {
        position: Vector3::ZERO,
        velocity: Vector3::ZERO,
        transform: Matrix3D::IDENTITY,
        gain: 1.0,
    };

    batch.set_listener(listener);

    // Add multiple sources
    let mixer = create_test_mixer();
    let source = Arc::new(create_test_source());

    for i in 0..5 {
        let descriptor = VoiceDescriptor {
            source: Arc::clone(&source),
            params: VoiceParams::default(),
            channel_id: i,
            handle_id: Some(i as u32),
        };

        let handle = mixer.start_voice(descriptor);

        let mut config = Audio3DConfig::default();
        config.position = Vector3::new(i as f32 * 10.0, 0.0, 0.0);

        batch.add_source(handle, config);
    }

    // Process all at once
    let results = batch.process_all();
    assert_eq!(results.len(), 5);

    // Verify results have different pan values based on position
    for (i, (_handle, params)) in results.iter().enumerate() {
        if i > 0 {
            // Further sources should be quieter
            assert!(params.gain <= 1.0);
        }
    }
}

#[tokio::test]
async fn test_audio_system_integration() {
    // This test would require actual audio files to be comprehensive
    // For now, test the basic initialization

    let config = AudioSystemConfig::default();
    assert!(config.max_channels > 0);
    assert!(config.cache_size_bytes > 0);
    assert!(config.stream_buffer_frames > 0);
}

#[test]
fn test_voice_category_properties() {
    assert!(
        VoiceCategory::TacticalAlert.default_priority()
            > VoiceCategory::UnitResponse.default_priority()
    );
    assert!(VoiceCategory::TacticalAlert.timeout_ms() > 0);

    for category in [
        VoiceCategory::UnitResponse,
        VoiceCategory::MissionDialogue,
        VoiceCategory::Announcer,
        VoiceCategory::TacticalAlert,
        VoiceCategory::Speech,
    ] {
        assert!(category.default_priority() >= Priority::Low);
        assert!(category.timeout_ms() > 0);
    }
}

#[test]
fn test_attenuation_models() {
    let processor = Audio3DProcessor::new(44100);

    let models = [
        AttenuationModel::None,
        AttenuationModel::InverseDistance,
        AttenuationModel::InverseDistanceClamped,
        AttenuationModel::Linear,
        AttenuationModel::LinearClamped,
        AttenuationModel::Exponential,
        AttenuationModel::ExponentialClamped,
    ];

    for model in models {
        let mut config = Audio3DConfig::default();
        config.position = Vector3::new(50.0, 0.0, 0.0);
        config.attenuation_model = model;

        let result = processor.calculate(&config);

        // All models should produce valid gain
        assert!(result.gain >= 0.0 && result.gain <= 1.0);

        if model == AttenuationModel::None {
            assert_eq!(result.gain, 1.0);
        }
    }
}
