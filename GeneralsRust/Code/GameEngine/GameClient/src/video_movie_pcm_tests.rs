use super::*;
use game_engine::common::audio::AudioAffect;
use rodio::mixer::{MixerSource, mixer};
use std::num::{NonZeroU16, NonZeroU32};

const CONTROL_QUANTUM: usize = 960; // 10 ms at 48000 Hz, stereo.

fn player_with_mixer(
    root: &Path,
    speech: f32,
) -> (VideoPlayer, Arc<Mutex<AudioManager>>, MixerSource, f32) {
    let (output, callback) = mixer(
        NonZeroU16::new(2).unwrap(),
        NonZeroU32::new(48_000).unwrap(),
    );
    let mut manager = AudioManager::new();
    manager.init();
    manager.set_volume(speech, AudioAffect::Speech);
    let gain = crate::bink::bink_volume_from_speech_slider(manager.get_volume(AudioAffect::Speech));
    manager.bind_movie_pcm_mixer(output);
    let device = Arc::new(Mutex::new(manager));

    let mut player = VideoPlayer::new();
    player.set_search_paths(VideoSearchPaths::new(None, root.to_owned(), "English"));
    player.init_with_video_ini_sources(&VideoIniCatalogSources::default());
    player.add_video(Video::new(
        "EA_LOGO".into(),
        "RetailPcm".into(),
        String::new(),
    ));
    player.bind_movie_audio_device(&device);
    (player, device, callback, gain)
}

#[test]
#[ignore = "requires GENERALS_RETAIL_VIDEO_ROOT and an installed EA_LOGO.BIK"]
fn retail_movie_pcm_drains_last_packet_and_cancels_only_receiving_player() {
    let root = PathBuf::from(std::env::var_os("GENERALS_RETAIL_VIDEO_ROOT").unwrap());
    let (mut first, first_device, mut callback_a, gain_a) = player_with_mixer(&root, 0.31);
    let (mut peer, _peer_device, mut callback_b, gain_b) = player_with_mixer(&root, 0.87);
    let (mut control, _control_device, mut callback_control, _) = player_with_mixer(&root, 0.87);
    let mut first_stream = first.open("RetailPcm".into()).unwrap();
    let mut peer_stream = peer.open("RetailPcm".into()).unwrap();
    let mut control_stream = control.open("RetailPcm".into()).unwrap();

    // Opening validates the first image. Audio begins when the presenter admits it.
    for _ in 0..2 {
        assert_eq!(callback_a.next().unwrap_or(0.0), 0.0);
        assert_eq!(callback_b.next().unwrap_or(0.0), 0.0);
        assert_eq!(callback_control.next().unwrap_or(0.0), 0.0);
    }

    first_device
        .lock()
        .unwrap()
        .set_volume(1.0, AudioAffect::Speech);
    for stream in [&mut first_stream, &mut peer_stream, &mut control_stream] {
        stream.frame_decompress();
        stream.frame_decompress();
        stream.frame_next();
        stream.frame_decompress();
    }
    // The presenter releases the picture at a natural end. Only the queued
    // PCM tail stays on the receiving player.
    peer_stream.finish();
    assert!(peer.active_streams[0].lock().unwrap().inner.is_none());
    assert!(peer.active_streams[0].lock().unwrap().audio_tail.is_some());

    let mut audible = false;
    for _ in 0..24_000 {
        let expected = callback_control.next().unwrap();
        assert_eq!(callback_b.next().unwrap(), expected);
        let actual = callback_a.next().unwrap();
        assert!((actual - expected * gain_a / gain_b).abs() < 1e-6);
        audible |= actual.abs() > 1e-6;
    }
    assert!(
        audible,
        "retail audio reached the real mixer at captured Speech gain"
    );
    first.reset();

    const FIRST_PACKET: usize = 72_960;
    const OUTPUT_SAMPLES: usize = FIRST_PACKET + 3_840;
    let mut heard_second_packet = false;
    for index in 24_000..OUTPUT_SAMPLES + CONTROL_QUANTUM {
        let actual = callback_a.next().unwrap_or(0.0);
        if index >= 24_000 + CONTROL_QUANTUM {
            assert_eq!(actual, 0.0, "reset must retire this voice");
        }
        let expected = callback_control.next().unwrap_or(0.0);
        assert_eq!(callback_b.next().unwrap_or(0.0), expected);
        if index >= FIRST_PACKET && expected.abs() > 1e-6 {
            heard_second_packet = true;
        }
    }
    assert!(
        heard_second_packet,
        "packet one remains ordered behind packet zero"
    );
    peer.update();
    assert!(
        peer.active_streams.is_empty(),
        "natural audio tail has drained"
    );
}
