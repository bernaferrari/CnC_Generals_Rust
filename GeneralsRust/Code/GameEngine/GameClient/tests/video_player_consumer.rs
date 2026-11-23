//! Exercises independently owned players through the public consumer API.

use game_client_rust::video_player::{Video, VideoPlayer, VideoPlayerInterface, VideoSearchPaths};

#[test]
fn players_open_and_play_independently() {
    let _services = game_engine::common::common_services::CommonServicesScope::new();
    let root = std::env::temp_dir().join(format!("generals_movie_consumer_{}", std::process::id()));
    let mod_root = root.join("mod");
    let path = mod_root.join("Data/Movies/CLIP.BIK");
    std::fs::create_dir_all(path.parent().unwrap()).expect("create movie directory");
    let mut bytes = vec![0u8; 84];
    bytes[..4].copy_from_slice(b"BIKi");
    for (offset, value) in [(4, 84u32), (8, 2), (12, 32), (20, 4), (24, 4), (28, 30), (32, 1), (44, 52), (48, 68)] {
        bytes[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
    }
    for (index, byte) in bytes[52..].iter_mut().enumerate() {
        *byte = index as u8;
    }
    std::fs::write(&path, bytes).expect("write Bink fixture");
    let paths = VideoSearchPaths::new(Some(mod_root), root.join("install"), "English");

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
        first.add_video(Video::new("CLIP".into(), "FixtureMovie".into(), String::new()));
        second.add_video(Video::new("CLIP".into(), "FixtureMovie".into(), String::new()));
        let _first_stream = first.open("FixtureMovie".into()).expect("first movie opens");
        let mut second_stream = second.open("FixtureMovie".into()).expect("second movie opens");
        second_stream.play();
        first.reset();
        drop(first);
        second_stream.frame_next();
        assert_eq!(second_stream.frame_index(), 1);
        assert!(second.open("FixtureMovie".into()).is_some(), "peer can still open a movie");
        second.deinit();
        second.init();
        second.add_video(Video::new("CLIP".into(), "FixtureMovie".into(), String::new()));
        assert!(second.open("FixtureMovie".into()).is_some(), "reinitialized player can open a movie");
    }
    std::fs::remove_dir_all(root).expect("remove Bink fixture tree");
}
