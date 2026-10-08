use super::*;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::Stdio;

const CHILD_MODE: &str = "GENERALS_AUDIO_PATH_SETTINGS_OWNER_CHILD";

struct TempAudioRoot(PathBuf);

impl Drop for TempAudioRoot {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn write_pcm_wav(path: &Path, sample_count: usize) {
    std::fs::create_dir_all(path.parent().expect("fixture parent")).unwrap();
    let spec = hound::WavSpec {
        channels: 1,
        sample_rate: 8_000,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };
    let mut writer = hound::WavWriter::create(path, spec).unwrap();
    for _ in 0..sample_count {
        writer.write_sample(0_i16).unwrap();
    }
    writer.finalize().unwrap();
}

fn event_info(name: &str, sound_type: AudioType, filename: &str) -> AudioEventInfo {
    AudioEventInfo {
        audio_name: name.to_string(),
        sound_type,
        sound_type_field: sound_type,
        type_field: ST_GLOBAL,
        filename: filename.to_string(),
        sounds: Vec::new(),
        attack_sounds: Vec::new(),
        decay_sounds: Vec::new(),
        pitch_shift_min: 1.0,
        pitch_shift_max: 1.0,
        delay_min: 0.0,
        delay_max: 0.0,
        volume_shift: 0.0,
        loop_count: 1,
        ..Default::default()
    }
}

fn configured_manager(
    root: &Path,
    sounds_folder: &str,
    music_folder: &str,
    streaming_folder: &str,
    extension: &str,
) -> AudioManager {
    let source = format!(
        "AudioSettings\nAudioRoot = \"{}\"\nSoundsFolder = {sounds_folder}\nMusicFolder = {music_folder}\nStreamingFolder = {streaming_folder}\nSoundsExtension = {extension}\nEnd\n",
        root.display()
    );
    let mut ini = crate::common::ini::INI::new();
    ini.with_inline_source(&source, |ini| ini.parse_current_file())
        .unwrap();
    let settings = {
        let parsed = crate::common::ini::ini_audio_settings::get_audio_settings_read().unwrap();
        AudioSettings {
            audio_root: parsed.audio_root.clone(),
            sounds_folder: parsed.sounds_folder.clone(),
            music_folder: parsed.music_folder.clone(),
            streaming_folder: parsed.streaming_folder.clone(),
            sounds_extension: parsed.sounds_extension.clone(),
            ..Default::default()
        }
    };

    let mut manager = AudioManager::new();
    manager.audio_settings = settings;
    manager.init();
    manager
}

// CPP adjustForLocalization appends a suffix starting with a backslash to
// a localized prefix already ending with one (AudioEventRTS.cpp781-798).
fn expected_path(root: &Path, folder: &str, filename: &str) -> String {
    format!(
        "{}\\{}\\English\\\\{}",
        root.to_string_lossy(),
        folder,
        filename
    )
}

// Main filenames pass through the existing extracted-asset resolver after
// localization. Absolute fixtures retain the root but use native separators;
// attack/decay filenames keep the C++ form used by the decoder's normalizer.
fn expected_resolved_path(root: &Path, folder: &str, filename: &str) -> String {
    expected_path(root, folder, filename).replace('\\', std::path::MAIN_SEPARATOR_STR)
}

fn queued_event(manager: &AudioManager) -> &AudioEventRts {
    manager
        .audio_requests
        .last()
        .and_then(AudioRequest::get_pending_event)
        .expect("add_audio_event queues its admitted event")
}

#[test]
fn manager_owned_path_settings_drive_add_and_duration_queries() {
    if std::env::var_os(CHILD_MODE).is_none() {
        // The localization setting and filesystem are process-wide. Run the
        // actual manager calls in a fresh child with a fixed language, while
        // keeping both AudioManager instances independent and locally owned.
        let module = module_path!()
            .strip_prefix(concat!(env!("CARGO_CRATE_NAME"), "::"))
            .unwrap_or(module_path!());
        let test = format!("{module}::manager_owned_path_settings_drive_add_and_duration_queries");
        let mut child = std::process::Command::new(std::env::current_exe().unwrap())
            .arg("--exact")
            .arg(&test)
            .arg("--nocapture")
            .env(CHILD_MODE, "1")
            .env("GENERALS_REGISTRY_LANGUAGE", "English")
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        let readers = [
            child
                .stdout
                .take()
                .map(|pipe| std::thread::spawn(move || read_output(pipe))),
            child
                .stderr
                .take()
                .map(|pipe| std::thread::spawn(move || read_output(pipe))),
        ];
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(25);
        let mut timed_out = false;
        let status = loop {
            if let Some(status) = child.try_wait().unwrap() {
                break status;
            }
            if std::time::Instant::now() >= deadline {
                timed_out = true;
                let _ = child.kill();
                break child.wait().unwrap();
            }
            std::thread::sleep(std::time::Duration::from_millis(25));
        };
        let [stdout, stderr] = readers.map(|reader| reader.unwrap().join().unwrap().unwrap());
        let stdout = String::from_utf8_lossy(&stdout);
        let stderr = String::from_utf8_lossy(&stderr);
        assert!(
            !timed_out,
            "audio path child exceeded deadline: {stdout}{stderr}"
        );
        assert!(
            status.success(),
            "audio path child failed: {stdout}{stderr}"
        );
        assert!(
            stdout.contains("1 passed; 0 failed"),
            "exact child must execute one regression test: {stdout}{stderr}"
        );
        return;
    }

    // C++ AudioEventRTS.cpp:746-777 reads the active manager's settings for
    // every prefix/extension. GameAudio.cpp:423 and :923 call filename
    // generation while the owning AudioManager operation is already active.
    let unique = format!(
        "generals_audio_path_owner_{}_{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    );
    let temp = TempAudioRoot(std::env::temp_dir().join(&unique));
    let root_a = temp.0.join("instance_a");
    let root_b = temp.0.join("instance_b");
    let sound_stem = format!("{unique}_main");
    let attack_stem = format!("{unique}_attack");
    let decay_stem = format!("{unique}_decay");
    let extension_a = "wav";
    let extension_b = ".WAV";

    for (root, folder, extension) in [
        (&root_a, "EffectsA", extension_a),
        (&root_b, "EffectsB", extension_b),
    ] {
        write_pcm_wav(
            &root
                .join(folder)
                .join("English")
                .join(format!("{sound_stem}{}", normalized_extension(extension))),
            8_000,
        );
        write_pcm_wav(
            &root
                .join(folder)
                .join("English")
                .join(format!("{attack_stem}{}", normalized_extension(extension))),
            4_000,
        );
        write_pcm_wav(
            &root
                .join(folder)
                .join("English")
                .join(format!("{decay_stem}{}", normalized_extension(extension))),
            2_000,
        );
        write_pcm_wav(
            &root.join("MusicA").join("English").join("track.wav"),
            8_000,
        );
        write_pcm_wav(
            &root.join("MusicB").join("English").join("track.wav"),
            8_000,
        );
        write_pcm_wav(
            &root.join("SpeechA").join("English").join("voice.wav"),
            8_000,
        );
        write_pcm_wav(
            &root.join("SpeechB").join("English").join("voice.wav"),
            8_000,
        );
    }

    let mut manager_a = configured_manager(&root_a, "EffectsA", "MusicA", "SpeechA", extension_a);
    let mut manager_b = configured_manager(&root_b, "EffectsB", "MusicB", "SpeechB", extension_b);
    let mut effect_info_a = event_info(&unique, AudioType::SoundEffect, "");
    effect_info_a.sounds = vec![sound_stem.clone()];
    effect_info_a.attack_sounds = vec![attack_stem.clone()];
    effect_info_a.decay_sounds = vec![decay_stem.clone()];
    let effect_info_b = effect_info_a.clone();
    manager_a.register_audio_event_info(effect_info_a);
    manager_b.register_audio_event_info(effect_info_b);

    let missing = AudioEventRts::with_event_name(&format!("{unique}_missing"));
    assert_eq!(manager_a.add_audio_event(&missing), AHSV_ERROR);
    assert!(manager_a.audio_requests.is_empty());
    assert_eq!(
        manager_a.add_audio_event(&AudioEventRts::with_event_name("NoSound")),
        AHSV_NO_SOUND
    );
    assert!(manager_a.audio_requests.is_empty());

    let event_a = AudioEventRts::with_event_name(&unique);
    let event_b = AudioEventRts::with_event_name(&unique);
    assert!(
        (AHSV_FIRST_HANDLE..AHSV_STOP_THE_MUSIC).contains(&manager_a.add_audio_event(&event_a))
    );
    assert!(
        (AHSV_FIRST_HANDLE..AHSV_STOP_THE_MUSIC).contains(&manager_b.add_audio_event(&event_b))
    );

    for (manager, root, folder, extension) in [
        (&manager_a, &root_a, "EffectsA", extension_a),
        (&manager_b, &root_b, "EffectsB", extension_b),
    ] {
        let queued = queued_event(manager);
        assert_eq!(
            queued.get_filename(),
            expected_resolved_path(
                root,
                folder,
                &format!("{sound_stem}{}", normalized_extension(extension))
            ),
            "main filename must use this manager's root/folder/extension and locale"
        );
        assert_eq!(
            queued.get_attack_filename(),
            expected_path(
                root,
                folder,
                &format!("{attack_stem}{}", normalized_extension(extension))
            ),
            "attack filename must preserve the same settings and locale"
        );
        assert_eq!(
            queued.get_decay_filename(),
            expected_path(
                root,
                folder,
                &format!("{decay_stem}{}", normalized_extension(extension))
            ),
            "decay filename must preserve the same settings and locale"
        );
        assert_eq!(
            manager.get_audio_length_ms(&AudioEventRts::with_event_name(&unique)),
            1_750.0,
            "main + attack + decay durations must come from actual custom PCM files"
        );
    }

    for (manager, root, folder) in [
        (&mut manager_a, &root_a, "MusicA"),
        (&mut manager_b, &root_b, "MusicB"),
    ] {
        let mut music_info = event_info(&format!("{unique}_music"), AudioType::Music, "track.wav");
        music_info.filename = "track.wav".to_string();
        manager.register_audio_event_info(music_info);
        let music = AudioEventRts::with_event_name(&format!("{unique}_music"));
        assert!(
            (AHSV_FIRST_HANDLE..AHSV_STOP_THE_MUSIC).contains(&manager.add_audio_event(&music))
        );
        assert_eq!(
            queued_event(manager).get_filename(),
            expected_resolved_path(root, folder, "track.wav"),
            "music uses the owning manager's music folder and no sound extension"
        );
    }

    for (manager, root, folder) in [
        (&mut manager_a, &root_a, "SpeechA"),
        (&mut manager_b, &root_b, "SpeechB"),
    ] {
        let mut speech_info = event_info(
            &format!("{unique}_speech"),
            AudioType::Streaming,
            "voice.wav",
        );
        speech_info.filename = "voice.wav".to_string();
        manager.register_audio_event_info(speech_info);
        let speech = AudioEventRts::with_event_name(&format!("{unique}_speech"));
        assert!(
            (AHSV_FIRST_HANDLE..AHSV_STOP_THE_MUSIC).contains(&manager.add_audio_event(&speech))
        );
        assert_eq!(
            queued_event(manager).get_filename(),
            expected_resolved_path(root, folder, "voice.wav"),
            "streaming uses the owning manager's speech folder and no sound extension"
        );
    }
}

fn normalized_extension(extension: &str) -> String {
    if extension.starts_with('.') {
        extension.to_string()
    } else {
        format!(".{extension}")
    }
}

fn read_output(mut pipe: impl Read) -> std::io::Result<Vec<u8>> {
    let mut output = Vec::new();
    pipe.read_to_end(&mut output)?;
    Ok(output)
}
