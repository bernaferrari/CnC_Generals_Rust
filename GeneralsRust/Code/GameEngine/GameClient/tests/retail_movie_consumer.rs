//! Retail Bink consumer smoke. Run with GENERALS_BINK_SMOKE_MOVIE set to a movie path.

use game_client_rust::video_player::{VideoPlayer, VideoPlayerInterface};

#[test]
#[ignore = "requires GENERALS_BINK_SMOKE_MOVIE pointing to a retail Bink movie"]
fn independent_players_open_retail_movie() {
    let _services = game_engine::common::common_services::CommonServicesScope::new();
    let movie = std::env::var("GENERALS_BINK_SMOKE_MOVIE")
        .expect("set GENERALS_BINK_SMOKE_MOVIE to an existing retail Bink movie");
    assert!(std::path::Path::new(&movie).is_file(), "retail Bink movie is missing: {movie}");
    for reverse in [false, true] {
        let mut first = VideoPlayer::new();
        let mut second = VideoPlayer::new();
        if reverse {
            second.init();
            first.init();
        } else {
            first.init();
            second.init();
        }
        let _first_stream = first.open(movie.clone()).expect("first player opens retail movie");
        let mut second_stream = second.open(movie.clone()).expect("second player opens retail movie");
        assert!(second_stream.frame_count() > 0);
        second_stream.play();
        first.reset();
        drop(first);
        second_stream.frame_next();
        assert!(second.open(movie.clone()).is_some(), "second player still opens retail movie");
        second.deinit();
        second.init();
        assert!(second.open(movie.clone()).is_some(), "reinitialized player opens retail movie");
    }
}
