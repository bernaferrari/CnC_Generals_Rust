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
