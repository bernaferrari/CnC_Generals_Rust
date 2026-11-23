use super::*;
use crate::video_player::{
    Video, VideoPlayer, VideoPlayerHandle, VideoPlayerInterface, VideoSearchPaths,
    VideoStreamProvider,
};
use crate::video_stream::VideoStreamInterface;
use std::path::Path;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

struct ProbeStream {
    frame: i32,
    closes: Arc<AtomicUsize>,
    decoded: Arc<AtomicUsize>,
}
impl VideoStreamInterface for ProbeStream {
    fn next(&self) -> Option<&dyn VideoStreamInterface> {
        None
    }
    fn next_mut(&mut self) -> Option<&mut dyn VideoStreamInterface> {
        None
    }
    fn update(&mut self) {}
    fn close(self: Box<Self>) {
        self.closes.fetch_add(1, Ordering::SeqCst);
    }
    fn is_frame_ready(&self) -> bool {
        true
    }
    fn frame_decompress(&mut self) {
        self.decoded.fetch_add(1, Ordering::SeqCst);
    }
    fn frame_render(&mut self, buffer: &mut dyn crate::video_buffer::VideoBuffer) {
        if let Some(pixels) = buffer.lock() {
            pixels.fill(0x7f);
        }
        buffer.unlock();
    }
    fn frame_next(&mut self) {
        self.frame += 1;
    }
    fn frame_index(&self) -> i32 {
        self.frame
    }
    fn frame_count(&self) -> i32 {
        3
    }
    fn frame_goto(&mut self, index: i32) {
        self.frame = index.clamp(0, 2);
    }
    fn height(&self) -> i32 {
        2
    }
    fn width(&self) -> i32 {
        2
    }
}
struct ProbeProvider {
    closes: Arc<AtomicUsize>,
    decoded: Arc<AtomicUsize>,
}
impl VideoStreamProvider for ProbeProvider {
    fn file_extension(&self) -> &str {
        "bik"
    }
    fn open(&self, _title: &str, _path: &Path) -> Option<Box<dyn VideoStreamInterface>> {
        Some(Box::new(ProbeStream {
            frame: 0,
            closes: self.closes.clone(),
            decoded: self.decoded.clone(),
        }))
    }
}
fn player_slot(
    root: &Path,
    closes: Arc<AtomicUsize>,
    decoded: Arc<AtomicUsize>,
) -> VideoPlayerHandle {
    let mut player = VideoPlayer::new();
    player.set_search_paths(VideoSearchPaths::new(None, root.to_path_buf(), "English"));
    player.add_video(Video::new(
        "CLIP".into(),
        "CampaignEnd".into(),
        String::new(),
    ));
    player.register_video_stream_provider(Box::new(ProbeProvider { closes, decoded }));
    Arc::new(Mutex::new(Some(player)))
}
#[test]
fn native_receiving_movie_is_client_owned_and_advances_through_main_fx() {
    crate::test_support::ensure_ambient_host();
    let root = std::env::temp_dir().join(format!("native-script-movie-{}", std::process::id()));
    let movie_path = root.join("Data/Movies/CLIP.BIK");
    std::fs::create_dir_all(movie_path.parent().unwrap()).unwrap();
    std::fs::write(&movie_path, b"fixture").unwrap();
    let close_a = Arc::new(AtomicUsize::new(0));
    let close_b = Arc::new(AtomicUsize::new(0));
    let decode_a = Arc::new(AtomicUsize::new(0));
    let decode_b = Arc::new(AtomicUsize::new(0));
    let mut a = GameClient::new().unwrap();
    let mut b = GameClient::new().unwrap();
    assert!(a.subsystem_manager.display.is_none());
    assert!(b.subsystem_manager.display.is_none());
    a.script_display
        .bind_video_player_slot(player_slot(&root, close_a.clone(), decode_a.clone()));
    b.script_display
        .bind_video_player_slot(player_slot(&root, close_b.clone(), decode_b.clone()));
    assert!(
        crate::core::script_action_handler::play_script_display_movie(
            a.script_display_receiver(),
            "CampaignEnd"
        )
    );
    assert!(
        crate::core::script_action_handler::play_script_display_movie(
            b.script_display_receiver(),
            "CampaignEnd"
        )
    );
    a.update_display_only().unwrap();
    a.update_display_only().unwrap();
    assert_eq!(decode_a.load(Ordering::SeqCst), 2);
    assert_eq!(decode_b.load(Ordering::SeqCst), 0);
    assert!(a.script_display.fx().current_movie_frame().is_some());
    assert!(
        crate::core::script_action_handler::is_script_display_movie_playing(
            b.script_display_receiver()
        )
    );
    assert!(
        crate::core::script_action_handler::stop_script_display_movie(a.script_display_receiver())
    );
    assert_eq!(close_a.load(Ordering::SeqCst), 1);
    assert!(a.script_display.fx().current_movie_frame().is_none());
    b.update_display_only().unwrap();
    b.update_display_only().unwrap();
    b.update_display_only().unwrap();
    assert_eq!(decode_b.load(Ordering::SeqCst), 3);
    assert_eq!(close_b.load(Ordering::SeqCst), 1);
    assert!(
        !crate::core::script_action_handler::is_script_display_movie_playing(
            b.script_display_receiver()
        )
    );
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn native_fullscreen_receives_pixels_from_the_retail_provider() {
    use crate::video_player::{VideoIniCatalogSources, init_video_player_slot};

    crate::test_support::ensure_ambient_host();
    let Some(root) = std::env::var_os("GENERALS_RETAIL_VIDEO_ROOT").map(PathBuf::from) else {
        return;
    };
    let catalog = VideoIniCatalogSources::read_from(|path| {
        (path == "Data/INI/Video.ini")
            .then(|| b"Video EALogoMovie\n  Filename = EA_LOGO\nEnd\n".to_vec())
    });
    let mut client = GameClient::new().unwrap();
    let slot = client.ensure_video_player_slot();
    init_video_player_slot(
        &slot,
        &VideoSearchPaths::new(None, root, "English"),
        &catalog,
    );
    client.script_display.bind_video_player_slot(slot);
    assert!(client.subsystem_manager.display.is_none());
    assert!(
        crate::core::script_action_handler::play_script_display_movie(
            client.script_display_receiver(),
            "EALogoMovie",
        )
    );

    client.update_display_only().unwrap();
    let (width, height, rgba) = client.script_display.fx().current_movie_frame().unwrap();
    assert_eq!((width, height), (720, 486));
    assert_eq!(rgba.len(), width as usize * height as usize * 4);
    assert!(rgba.chunks_exact(4).any(|pixel| pixel != &rgba[..4]));
    client.reset().unwrap();
    assert!(
        !crate::core::script_action_handler::is_script_display_movie_playing(
            client.script_display_receiver()
        )
    );
    assert!(client.script_display.fx().current_movie_frame().is_none());
}
