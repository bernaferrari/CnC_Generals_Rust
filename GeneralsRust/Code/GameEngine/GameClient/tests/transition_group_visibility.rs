use game_client_rust::core::game_client::GameClient;
use game_engine::common::name_key_generator::NameKeyGenerator;
use std::path::PathBuf;

struct RestoreDirectory(PathBuf);
impl Drop for RestoreDirectory {
    fn drop(&mut self) {
        std::env::set_current_dir(&self.0).unwrap();
    }
}

#[test]
fn transition_styles_keep_visibility_and_duration_with_receiving_owners() {
    let temp = tempfile::tempdir().unwrap();
    let ini_dir = temp.path().join("Data/INI");
    std::fs::create_dir_all(&ini_dir).unwrap();
    std::fs::write(
        ini_dir.join("WindowTransitions.ini"),
        r#"
WindowTransition SoundOnly
  Window
    WinName = Test:Sound
    Style = REVERSESOUND
    FrameDelay = 0
  END
END
WindowTransition FlashOnly
  Window
    WinName = Test:Sound
    Style = FLASH
    FrameDelay = 0
  END
END
WindowTransition ScreenFadeOnly
  Window
    Style = SCREENFADE
    FrameDelay = 0
  END
END
WindowTransition CountOnly
  Window
    WinName = Test:Sound
    Style = COUNTUP
    FrameDelay = 0
  END
END
"#,
    )
    .unwrap();
    // This integration test has its own process; the current directory is
    // restored before the temporary content is deleted.
    let _restore = RestoreDirectory(std::env::current_dir().unwrap());
    std::env::set_current_dir(temp.path()).unwrap();
    let mut first = GameClient::new().unwrap();
    let mut second = GameClient::new().unwrap();
    let id = NameKeyGenerator::name_to_key("Test:Sound") as i32;
    let setup = |client: &mut GameClient, hidden| {
        client.with_window_event_effects(None, |effects| {
            effects.with_manager(|manager, effects| {
                manager.init();
                let window = manager
                    .create_window_with_id(None, 0, 0, 20, 20, id, effects)
                    .unwrap();
                let _ = window.borrow_mut().hide(hidden, manager);
                window
            })
        })
    };
    let visible = setup(&mut first, false);
    let hidden = setup(&mut second, true);
    first.with_window_event_effects(None, |effects| {
        effects.with_manager(|manager, effects| {
            manager.transition_set_group("SoundOnly", true, None);
            assert!(
                !visible.borrow().is_hidden(),
                "sound init must not hide its window"
            );
            manager.update(effects);
            assert!(
                !visible.borrow().is_hidden(),
                "sound update must not hide its window"
            );
            manager.transition_remove("SoundOnly", true, None);
            assert!(
                !visible.borrow().is_hidden(),
                "sound skip must not change visibility"
            );
            manager.transition_set_group("FlashOnly", true, None);
            assert!(
                visible.borrow().is_hidden(),
                "the flash style hides on init"
            );
            manager.transition_set_group("SoundOnly", true, None);
            assert!(
                !visible.borrow().is_hidden(),
                "skipping flash reveals its window; overlapping sound must leave it revealed"
            );
            manager.transition_set_group("ScreenFadeOnly", true, None);
            assert!(!manager.transitions_finished(), "fade starts unfinished");
            for frame in 1..30 {
                manager.update(effects);
                assert!(
                    !manager.transitions_finished(),
                    "fade ended at frame {frame}"
                );
            }
            manager.update(effects);
            assert!(manager.transitions_finished(), "fade completes at frame 30");
            manager.transition_reverse("ScreenFadeOnly", None);
            assert!(
                !manager.transitions_finished(),
                "reversed fade starts unfinished"
            );
            for frame in (1..30).rev() {
                manager.update(effects);
                assert!(
                    !manager.transitions_finished(),
                    "reversed fade ended at frame {frame}"
                );
            }
            manager.update(effects);
            assert!(
                manager.transitions_finished(),
                "reversed fade completes at frame 0"
            );
            visible.borrow_mut().set_text(" 15 units ").unwrap();
            manager.transition_set_group("CountOnly", true, None);
            assert_eq!(visible.borrow().get_text(), "0");
            manager.update(effects);
            assert_eq!(visible.borrow().get_text(), "1");
            assert!(hidden.borrow().is_hidden());
            for _ in 1..15 {
                manager.update(effects);
            }
            assert_eq!(visible.borrow().get_text(), " 15 units ");
            assert!(manager.transitions_finished());
        });
    });
    second.with_window_event_effects(None, |effects| {
        effects.with_manager(|manager, effects| {
            manager.transition_set_group("SoundOnly", true, None);
            assert!(
                hidden.borrow().is_hidden(),
                "sound init must not reveal a peer window"
            );
            manager.update(effects);
            manager.transition_reverse("SoundOnly", None);
            manager.transition_remove("SoundOnly", true, None);
            assert!(
                hidden.borrow().is_hidden(),
                "sound must not reveal a hidden peer window"
            );
        });
    });
}
