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
fn score_screen_ok_then_main_menu_init_preserves_only_receiving_movie_until_stop() {
    use game_engine::common::name_key_generator::NameKeyGenerator;

    crate::test_support::ensure_ambient_host();
    let root = std::env::temp_dir().join(format!("score-mainmenu-movie-{}", std::process::id()));
    let movie_path = root.join("Data/Movies/CLIP.BIK");
    std::fs::create_dir_all(movie_path.parent().unwrap()).unwrap();
    std::fs::write(&movie_path, b"fixture").unwrap();
    let close_a = Arc::new(AtomicUsize::new(0));
    let close_b = Arc::new(AtomicUsize::new(0));
    let decode_a = Arc::new(AtomicUsize::new(0));
    let decode_b = Arc::new(AtomicUsize::new(0));
    let mut a = GameClient::new().unwrap();
    let mut b = GameClient::new().unwrap();
    for client in [&mut a, &mut b] {
        let campaigns = client.campaign_manager_mut();
        campaigns.new_campaign("movie-campaign".into());
        campaigns.set_campaign("movie-campaign");
        assert!(campaigns.get_current_campaign().is_some());
    }
    a.script_display
        .bind_video_player_slot(player_slot(&root, close_a.clone(), decode_a.clone()));
    b.script_display
        .bind_video_player_slot(player_slot(&root, close_b.clone(), decode_b.clone()));

    assert!(crate::core::script_action_handler::play_script_display_movie(
        a.script_display_receiver(),
        "CampaignEnd"
    ));
    assert!(crate::core::script_action_handler::play_script_display_movie(
        b.script_display_receiver(),
        "CampaignEnd"
    ));
    a.update_display_only().unwrap();
    b.update_display_only().unwrap();
    assert_eq!(decode_a.load(Ordering::SeqCst), 1);
    assert_eq!(decode_b.load(Ordering::SeqCst), 1);

    // Exercise the ordinary ScoreScreen WND OK route, then MainMenuInit on the
    // same receiving client. MainMenuInit resets the break flag; it does not
    // own or stop the asynchronous native movie.
    a.with_window_event_effects(None, |effects| {
        const OK_BUTTON: &str = "ScoreScreen.wnd:ButtonOk";
        let ok_id = NameKeyGenerator::name_to_key(OK_BUTTON) as i32;
        let owner = effects.window_manager_owner.expect("receiving window owner");
        let button = owner.with_manager(|manager| {
            let button = manager
                .test_create_window(None, 10, 40, 80, 24)
                .expect("ScoreScreen OK button");
            {
                let mut window = button.borrow_mut();
                window.set_name(OK_BUTTON);
                window.set_id(ok_id);
                let _ = window.hide_without_manager_side_effects(false);
            }
            button
        });
        assert!(crate::gui::callbacks::score_screen::simulate_score_screen_bind_controls(
            effects
        ));
        let button_ok_id = button.borrow().get_id() as crate::gui::WindowMsgData;
        let handled = crate::gui::callbacks::score_screen::score_screen_system(
            &mut button.borrow_mut(),
            crate::gui::WindowMessage::GadgetSelected,
            button_ok_id,
            0,
            effects,
        );
        assert_eq!(handled, crate::gui::WindowMsgHandled::Handled);
        assert!(
            effects
                .campaign_manager
                .as_deref()
                .unwrap()
                .get_current_campaign()
                .is_none()
        );
        effects.with_main_menu(|menu, effects| {
            menu.init(&(), None, effects).expect("MainMenuInit");
        });
    });

    assert!(b.campaign_manager().get_current_campaign().is_some());
    assert_eq!(close_a.load(Ordering::SeqCst), 0);
    assert_eq!(close_b.load(Ordering::SeqCst), 0);
    assert!(a.script_display.fx().current_movie_frame().is_some());
    assert!(b.script_display.fx().current_movie_frame().is_some());
    assert!(crate::core::script_action_handler::stop_script_display_movie(
        a.script_display_receiver()
    ));
    assert_eq!(close_a.load(Ordering::SeqCst), 1);
    assert!(a.script_display.fx().current_movie_frame().is_none());
    assert!(crate::core::script_action_handler::is_script_display_movie_playing(
        b.script_display_receiver()
    ));
    b.update_display_only().unwrap();
    assert_eq!(decode_b.load(Ordering::SeqCst), 2);
    assert_eq!(close_b.load(Ordering::SeqCst), 0);
    let _ = std::fs::remove_dir_all(root);
}

#[cfg(not(target_arch = "wasm32"))]
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
    let (mixer, mut callback) = rodio::mixer::mixer(
        std::num::NonZeroU16::new(2).unwrap(),
        std::num::NonZeroU32::new(48_000).unwrap(),
    );
    let mut audio = game_engine::common::audio::AudioManager::new();
    audio.init();
    audio.bind_movie_pcm_mixer(mixer);
    client.bind_native_audio_device(Arc::new(Mutex::new(audio)));
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
    let peak = (0..24_000)
        .map(|_| callback.next().unwrap_or(0.0).abs())
        .fold(0.0, f32::max);
    assert!(peak > 1e-4, "native fullscreen PCM reached the receiving output");
    client.reset().unwrap();
    for _ in 0..960 {
        callback.next(); // Rodio samples controls at callback cadence.
    }
    assert!((0..960).all(|_| callback.next().unwrap_or(0.0) == 0.0));
    assert!(
        !crate::core::script_action_handler::is_script_display_movie_playing(
            client.script_display_receiver()
        )
    );
    assert!(client.script_display.fx().current_movie_frame().is_none());
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
#[ignore = "requires the mounted retail WND and movies prepared by mount_score_assets.py"]
fn native_score_post_draw_opens_receiving_movie_and_peer_ok_pops_to_mounted_main_menu() {
    use crate::core::script_action_handler::{
        is_script_display_movie_playing, play_script_display_movie,
    };
    use crate::gui::callbacks::score_screen::ScoreScreenAcademySnapshot;
    use crate::gui::campaign_launch_host_bridge::{MenuWorldFacts, ShellMapSettings};
    use crate::system::SubsystemInterface;
    use crate::video_player::{VideoIniCatalogSources, init_video_player_slot};
    use game_engine::common::{common_services::CommonServicesScope, game_lod, ini};
    use std::rc::Rc;

    crate::test_support::ensure_ambient_host();
    let _common = CommonServicesScope::new();
    let preferences = tempfile::tempdir().expect("isolated score-screen preferences");
    ini::ini_game_data::init_global_data();
    ini::get_global_data()
        .expect("receiving application startup data")
        .write()
        .set_path_user_data(preferences.path().to_string_lossy().into_owned());
    game_lod::reset_static_lod_state_for_tests();
    game_lod::set_mem_passed_override_for_tests(Some(true));
    game_lod::set_ideal_static_lod_from_string("High");
    assert!(!game_lod::prefers_low_res_movies());

    let root = PathBuf::from(
        std::env::var_os("GENERALS_ASSETS_DIR").expect("mounted ScoreScreen and MainMenu assets"),
    );
    for wnd in ["ScoreScreen.wnd", "MainMenu.wnd", "BlankWindow.wnd"] {
        assert!(root.join("Window/Menus").join(wnd).is_file(), "{wnd}");
    }
    let catalog = VideoIniCatalogSources::read_from(|name| std::fs::read(root.join(name)).ok());
    let facts = MenuWorldFacts::default();
    let make_client = |campaign_name: &str, final_movie: Option<&str>| {
        let mut client = GameClient::new_with_input_authority(ClientInputAuthority::NativeHosted)
            .expect("receiving native GameClient");
        client.configure_shell_map(ShellMapSettings {
            enabled: false,
            ..ShellMapSettings::default()
        });

        // Normal receiving window callback setup creates this client's InGameUI.
        client.with_window_event_effects(Some(facts), |_| {});
        let slot = client.ensure_video_player_slot();
        init_video_player_slot(
            &slot,
            &VideoSearchPaths::new(None, root.clone(), "English"),
            &catalog,
        );
        client.script_display.bind_video_player_slot(slot);

        let campaigns = client.campaign_manager_mut();
        let campaign = campaigns.new_campaign(campaign_name.into());
        campaign.first_mission = "terminal".into();
        campaign.new_mission("terminal".into()).map_name = "terminal.map".into();
        campaign.final_movie_name = final_movie.unwrap_or_default().into();
        campaigns.set_campaign(campaign_name);
        campaigns.set_victorious(final_movie.is_some());

        client
            .with_receiving_shell(Some(facts), |shell, effects| {
                shell.init().expect("receiving Shell init");
                shell
                    .push("Menus/MainMenu.wnd", false, effects)
                    .expect("retail MainMenu layout");
            })
            .expect("receiving Shell");
        let menu = client
            .window_manager_owner_ref()
            .with_manager_ref(|manager| {
                manager
                    .find_window_by_name("MainMenu.wnd:MainMenuParent")
                    .expect("materialized retail MainMenu")
            });

        // Starting the match hides MainMenu. Complete the window callback
        // before the later score push checks the layout's visibility.
        client
            .with_receiving_shell(Some(facts), |shell, effects| {
                shell.hide_with_effects(true, effects);
            })
            .expect("receiving Shell");
        client
            .with_receiving_shell(Some(facts), |shell, effects| {
                assert!(shell.top().unwrap().is_hidden());
                shell
                    .push_score_screen(
                        ScoreScreenAcademySnapshot {
                            end_frame: 900,
                            advice: Default::default(),
                            show_advice: false,
                            player_roster: Some(Vec::new()),
                            is_single_player: Some(true),
                            is_skirmish: Some(false),
                            session_difficulty: Some(1),
                            skirmish_outcome: None,
                        },
                        false,
                        effects,
                    )
                    .expect("retail ScoreScreen layout");
                shell
                    .show_shell(false, effects)
                    .expect("receiving score shell");
                assert_eq!(shell.get_screen_count(), 2);
                assert_eq!(shell.top_filename(), Some("Menus/ScoreScreen.wnd"));
            })
            .expect("receiving Shell");
        (client, menu)
    };

    // USACampaignVictory is the stock registered title for USA_end.BIK.
    let (mut victory, _) = make_client("victory-fixture", Some("USACampaignVictory"));
    let (mut peer, peer_menu) = make_client("peer-fixture", None);
    assert!(play_script_display_movie(
        peer.script_display_receiver(),
        "EALogoMovie"
    ));
    peer.update_display_only().unwrap();
    assert!(peer.script_display.fx().current_movie_frame().is_some());

    // Shell owns a real 33 ms Update cadence. Exercise Main's actual entry.
    std::thread::sleep(std::time::Duration::from_millis(40));
    peer.update_post_draw_ui(Some(facts), None)
        .expect("the ordinary no-movie score update finalizes its controls");
    victory
        .update_post_draw_ui(Some(facts), None)
        .expect("the victory score update has its receiving display");
    assert!(is_script_display_movie_playing(
        victory.script_display_receiver()
    ));
    victory.update_display_only().unwrap();
    assert!(victory.script_display.fx().current_movie_frame().is_some());

    const OK: &str = "ScoreScreen.wnd:ButtonOk";
    for (client, hidden) in [(&victory, true), (&peer, false)] {
        client
            .window_manager_owner_ref()
            .with_manager_ref(|manager| {
                let ok = manager
                    .find_window_by_name(OK)
                    .expect("retail ScoreScreen OK");
                assert_eq!(ok.borrow().is_hidden(), hidden);
            });
    }

    assert!(peer.with_window_event_effects(Some(facts), |effects| {
        crate::gui::dispatch_os_click_named_window(OK, effects)
    }));
    assert!(peer.campaign_manager().get_current_campaign().is_none());
    let peer_shell = peer.window_manager_owner_ref().shell_handle();
    assert_eq!(peer_shell.get_screen_count(), 1);
    assert_eq!(
        peer_shell.top_filename().as_deref(),
        Some("Menus/MainMenu.wnd")
    );
    peer.window_manager_owner_ref().with_manager_ref(|manager| {
        assert!(manager.find_window_by_name(OK).is_none());
        let menu = manager
            .find_window_by_name("MainMenu.wnd:MainMenuParent")
            .unwrap();
        assert!(Rc::ptr_eq(&menu, &peer_menu));
        assert!(!menu.borrow().is_hidden());
    });
    assert_eq!(
        victory
            .window_manager_owner_ref()
            .shell_handle()
            .get_screen_count(),
        2
    );
    assert!(victory.campaign_manager().get_current_campaign().is_some());
    assert!(is_script_display_movie_playing(
        victory.script_display_receiver()
    ));
    assert!(is_script_display_movie_playing(
        peer.script_display_receiver()
    ));
}
