//! Retail Bink consumer smoke. Run with GENERALS_BINK_SMOKE_MOVIE set to a movie path.

use game_client_rust::video_player::{Video, VideoPlayer, VideoPlayerInterface, VideoSearchPaths};

#[test]
#[ignore = "requires GENERALS_BINK_SMOKE_MOVIE pointing to a retail Bink movie"]
fn independent_players_open_retail_movie() {
    let _services = game_engine::common::common_services::CommonServicesScope::new();
    let movie = std::env::var("GENERALS_BINK_SMOKE_MOVIE")
        .expect("set GENERALS_BINK_SMOKE_MOVIE to an existing retail Bink movie");
    assert!(std::path::Path::new(&movie).is_file(), "retail Bink movie is missing: {movie}");
    let root = std::env::temp_dir().join(format!("retail_movie_consumer_{}", std::process::id()));
    let game_root = root.join("install");
    let mounted = game_root.join("Data/English/Movies/RETAIL.BIK");
    std::fs::create_dir_all(mounted.parent().unwrap()).expect("create retail movie fixture directory");
    std::fs::copy(&movie, &mounted).expect("copy mounted retail Bink fixture");
    let paths = VideoSearchPaths::new(None, game_root, "English");
    for reverse in [false, true] {
        let mut first = VideoPlayer::new();
        let mut second = VideoPlayer::new();
        for player in [&mut first, &mut second] {
            player.set_search_paths(paths.clone());
        }
        if reverse {
            second.init();
            first.init();
        } else {
            first.init();
            second.init();
        }
        first.add_video(Video::new("RETAIL".into(), "RetailMovie".into(), String::new()));
        second.add_video(Video::new("RETAIL".into(), "RetailMovie".into(), String::new()));
        let _first_stream = first.open("RetailMovie".into()).expect("first player opens retail movie");
        let mut second_stream = second.open("RetailMovie".into()).expect("second player opens retail movie");
        assert!(second_stream.frame_count() > 0);
        second_stream.play();
        first.reset();
        drop(first);
        second_stream.frame_next();
        assert!(second.open("RetailMovie".into()).is_some(), "second player still opens retail movie");
        second.deinit();
        second.init();
        second.add_video(Video::new("RETAIL".into(), "RetailMovie".into(), String::new()));
        assert!(second.open("RetailMovie".into()).is_some(), "reinitialized player opens retail movie");
    }
    std::fs::remove_dir_all(root).expect("remove retail movie fixture tree");
}

#[test]
#[ignore = "requires GENERALS_BINK_SMOKE_MOVIE pointing to retail EA_LOGO.BIK"]
fn retail_ea_logo_decoded_audio_length_uses_pcm_bytes() {
    use game_client_rust::bink::BinkDecoder;
    use game_client_rust::bink_audio::{
        BinkAudioDecoder, parse_audio_layout, split_frame_audio_and_video,
    };

    let path = std::env::var_os("GENERALS_BINK_SMOKE_MOVIE")
        .expect("set GENERALS_BINK_SMOKE_MOVIE to the retail EA_LOGO.BIK");
    let bytes = std::fs::read(path).expect("read the retail EA logo");
    let layout = parse_audio_layout(&bytes, 1, false);
    let track = &layout.tracks[0];
    assert_eq!((track.sample_rate, track.channels), (48_000, 2));
    let mut audio = BinkAudioDecoder::new(track.clone());
    let decoder = BinkDecoder::from_bytes(bytes).expect("open the real Bink container");
    assert_eq!(decoder.header().audio_track_count, 1);

    // Stock waveform witnesses from FFmpeg's Bink DCT decoder. Both
    // packets include independent left and right values after overlap.
    let expected = [
        (
            0,
            145_920,
            [
                (26_870, 0.253_387_81),
                (26_871, 0.100_617_826),
                (41_321, -0.067_972_109),
                (56_510, 0.019_652_925),
                (56_511, 0.104_742_85),
                (72_959, 0.144_397_5),
            ],
        ),
        (
            1,
            7_680,
            [
                (0, 0.149_031_03),
                (1, 0.158_363_33),
                (47, 0.174_890_07),
                (48, -0.002_807_607_4),
                (216, -0.096_859_68),
                (217, -0.140_356_65),
            ],
        ),
    ];
    for (frame, declared_bytes, samples) in expected {
        let (packets, _) = split_frame_audio_and_video(decoder.packet(frame), layout.tracks.len());
        let packet = packets[0];
        let pcm_bytes = u32::from_le_bytes(packet[..4].try_into().unwrap()) as usize;
        assert_eq!(pcm_bytes, declared_bytes);
        let started = std::time::Instant::now();
        let pcm = audio.decode_packet(packet);
        eprintln!("EA_LOGO audio packet {frame} decode {:?}", started.elapsed());
        assert_eq!(
            pcm.len(),
            pcm_bytes / std::mem::size_of::<i16>(),
            "retail EA logo audio packet {frame}",
        );
        for (sample, expected) in samples {
            assert!(
                (pcm[sample] - expected).abs() < 2e-6,
                "EA_LOGO packet {frame} sample {sample}: {} vs {expected}",
                pcm[sample],
            );
        }
    }

    let impossible_size = [0xfc, 0xff, 0xff, 0xff, 0x00];
    assert!(audio.decode_packet(&impossible_size).is_empty());
}
