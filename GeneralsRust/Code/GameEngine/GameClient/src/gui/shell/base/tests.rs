// Split from `gui/shell/base.rs` dump. Included by `base/mod.rs`.
#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell as StdRefCell;
    use std::rc::Rc as StdRc;
    use std::sync::{Mutex, OnceLock};

    fn shell_global_test_lock() -> &'static Mutex<()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(()))
    }

    #[derive(Clone)]
    struct TestLayout {
        filename: String,
        hidden: bool,
        state: LayoutState,
        events: StdRc<StdRefCell<Vec<String>>>,
    }

    struct ReentrantQueueLayout {
        events: StdRc<StdRefCell<Vec<String>>>,
        hidden: bool,
        state: LayoutState,
    }

    impl ReentrantQueueLayout {
        fn new(events: StdRc<StdRefCell<Vec<String>>>) -> Self {
            Self {
                events,
                hidden: false,
                state: LayoutState::Active,
            }
        }
    }

    impl WindowLayout for ReentrantQueueLayout {
        fn get_filename(&self) -> &str {
            "reentrant-queue.wnd"
        }

        fn run_init(&mut self, _data: Option<&dyn std::any::Any>) -> Result<(), ShellError> {
            self.state = LayoutState::Active;
            Ok(())
        }

        fn run_update(&mut self, _data: Option<&dyn std::any::Any>) -> Result<(), ShellError> {
            Ok(())
        }

        fn run_shutdown(&mut self, immediate_pop: &mut bool) -> Result<(), ShellError> {
            self.events.borrow_mut().push("shutdown".to_string());
            let events = self.events.clone();
            queue_shell_operation(move |_| events.borrow_mut().push("queued".to_string()));
            *immediate_pop = true;
            self.hidden = true;
            self.state = LayoutState::ShuttingDown;
            Ok(())
        }

        fn hide(&mut self, hide: bool) {
            self.hidden = hide;
        }

        fn is_hidden(&self) -> bool {
            self.hidden
        }

        fn bring_forward(&mut self) {}

        fn destroy_windows(&mut self) {
            self.events.borrow_mut().push("destroy".to_string());
            self.state = LayoutState::Destroying;
        }

        fn get_state(&self) -> LayoutState {
            self.state
        }

        fn set_state(&mut self, state: LayoutState) {
            self.state = state;
        }
    }

    impl TestLayout {
        fn new(filename: &str, hidden: bool, events: StdRc<StdRefCell<Vec<String>>>) -> Self {
            Self {
                filename: filename.to_string(),
                hidden,
                state: LayoutState::Initializing,
                events,
            }
        }
    }

    impl WindowLayout for TestLayout {
        fn get_filename(&self) -> &str {
            &self.filename
        }

        fn run_init(&mut self, _data: Option<&dyn std::any::Any>) -> Result<(), ShellError> {
            self.hidden = false;
            self.state = LayoutState::Active;
            self.events
                .borrow_mut()
                .push(format!("init:{}", self.filename));
            Ok(())
        }

        fn run_update(&mut self, _data: Option<&dyn std::any::Any>) -> Result<(), ShellError> {
            Ok(())
        }

        fn run_shutdown(&mut self, _immediate_pop: &mut bool) -> Result<(), ShellError> {
            self.hidden = true;
            self.state = LayoutState::ShuttingDown;
            self.events
                .borrow_mut()
                .push(format!("shutdown:{}", self.filename));
            Ok(())
        }

        fn hide(&mut self, hide: bool) {
            self.hidden = hide;
            self.events
                .borrow_mut()
                .push(format!("hide:{}:{}", self.filename, hide));
        }

        fn is_hidden(&self) -> bool {
            self.hidden
        }

        fn bring_forward(&mut self) {
            self.events
                .borrow_mut()
                .push(format!("bring_forward:{}", self.filename));
        }

        fn set_first_window_image(&mut self) {
            self.events
                .borrow_mut()
                .push(format!("set_first_window_image:{}", self.filename));
        }

        fn destroy_windows(&mut self) {
            self.state = LayoutState::Destroying;
            self.events
                .borrow_mut()
                .push(format!("destroy:{}", self.filename));
        }

        fn get_state(&self) -> LayoutState {
            self.state
        }

        fn set_state(&mut self, state: LayoutState) {
            self.state = state;
        }
    }

    #[test]
    fn test_shell_creation() {
        let shell = Shell::new();
        assert_eq!(shell.get_screen_count(), 0);
        assert!(shell.is_shell_active());
        assert!(!shell.shell_map_on);
    }

    #[test]
    fn test_shell_init() {
        let mut shell = Shell::new();
        assert!(shell.init().is_ok());
        assert!(shell.initialized);
    }

    #[test]
    fn test_push_before_init() {
        let mut shell = Shell::new();
        let result = shell.push("test.wnd", false);
        assert!(matches!(result, Err(ShellError::NotInitialized)));
    }

    #[test]
    fn test_push_empty_filename() {
        let mut shell = Shell::new();
        shell.init().unwrap();
        let result = shell.push("", false);
        assert!(matches!(result, Err(ShellError::LayoutNotFound(_))));
    }

    #[test]
    fn test_pop_empty_stack() {
        let mut shell = Shell::new();
        shell.init().unwrap();
        let result = shell.pop();
        assert!(matches!(result, Err(ShellError::EmptyStack)));
    }

    #[test]
    fn test_basic_window_layout() {
        let mut layout = BasicWindowLayout::new("test.wnd".to_string());

        assert_eq!(layout.get_filename(), "test.wnd");
        assert!(layout.is_hidden());
        assert_eq!(layout.get_state(), LayoutState::Initializing);

        // Missing .wnd files should fail to initialize instead of silently succeeding.
        assert!(matches!(
            layout.run_init(None),
            Err(ShellError::LayoutError(_))
        ));
        assert!(layout.is_hidden());
        assert_eq!(layout.get_state(), LayoutState::Initializing);

        let mut immediate = false;
        layout.run_shutdown(&mut immediate).unwrap();
        assert!(layout.is_hidden());
        assert_eq!(layout.get_state(), LayoutState::ShuttingDown);
    }

    #[test]
    fn test_coord2d() {
        let coord = Coord2D::new(10, 20);
        assert_eq!(coord.x, 10);
        assert_eq!(coord.y, 20);

        let zero = Coord2D::zero();
        assert_eq!(zero.x, 0);
        assert_eq!(zero.y, 0);
    }

    #[test]
    fn test_color() {
        let color = Color::new(255, 128, 64, 200);
        assert_eq!(color.r, 255);
        assert_eq!(color.g, 128);
        assert_eq!(color.b, 64);
        assert_eq!(color.a, 200);

        let black = Color::black();
        assert_eq!(black.r, 0);
        assert_eq!(black.a, 255);
    }

    #[test]
    fn test_shell_menu_scheme() {
        let mut scheme = ShellMenuScheme::new("test".to_string());
        assert_eq!(scheme.name, "test");
        assert_eq!(scheme.images.len(), 0);
        assert_eq!(scheme.lines.len(), 0);

        let image = ShellMenuSchemeImage::new(
            "test_image".to_string(),
            Coord2D::new(10, 10),
            Coord2D::new(100, 100),
        );
        scheme.add_image(image);
        assert_eq!(scheme.images.len(), 1);

        let line = ShellMenuSchemeLine::new(
            Coord2D::new(0, 0),
            Coord2D::new(100, 100),
            2,
            Color::white(),
        );
        scheme.add_line(line);
        assert_eq!(scheme.lines.len(), 1);
    }

    #[test]
    fn test_scheme_manager() {
        let mut manager = ShellMenuSchemeManager::new();
        manager.init().unwrap();

        let scheme = manager.new_shell_menu_scheme("test_scheme".to_string());
        assert_eq!(scheme.name, "test_scheme");

        manager.set_shell_menu_scheme("test_scheme");
        // Should not crash when drawing
        manager.draw();
    }

    #[test]
    fn test_scheme_manager_clears_current_scheme_on_empty_name() {
        let mut manager = ShellMenuSchemeManager::new();
        manager.new_shell_menu_scheme("test_scheme".to_string());
        manager.set_shell_menu_scheme("test_scheme");
        assert_eq!(manager.current_scheme.as_deref(), Some("test_scheme"));
        manager.set_shell_menu_scheme("");
        assert!(manager.current_scheme.is_none());
    }

    #[test]
    fn test_scheme_manager_replaces_duplicates_in_cpp_list_order() {
        let mut manager = ShellMenuSchemeManager::new();

        manager.new_shell_menu_scheme("first".to_string());
        manager.new_shell_menu_scheme("second".to_string());
        manager
            .new_shell_menu_scheme("FIRST".to_string())
            .add_line(ShellMenuSchemeLine::new(
                Coord2D::new(1, 2),
                Coord2D::new(3, 4),
                5,
                Color::white(),
            ));

        assert_eq!(manager.scheme_order, vec!["second", "first"]);
        assert_eq!(manager.schemes["first"].lines.len(), 1);
        assert!(manager.schemes["first"].images.is_empty());
    }

    #[test]
    fn test_parse_shell_menu_schemes_replaces_duplicate_blocks() {
        let mut manager = ShellMenuSchemeManager::new();

        manager.parse_shell_menu_schemes(
            r#"
ShellMenuScheme Alpha
  ImagePart
    Position = 1 2
    Size = 3 4
    ImageName = stale
  EndImagePart
End
ShellMenuScheme Beta
End
ShellMenuScheme Alpha
  LinePart
    StartPosition = 5 6
    EndPosition = 7 8
    Color = 4294967295
    Width = 9
  EndLinePart
End
"#,
        );

        assert_eq!(manager.scheme_order, vec!["beta", "alpha"]);
        let alpha = &manager.schemes["alpha"];
        assert!(alpha.images.is_empty());
        assert_eq!(alpha.lines.len(), 1);
        assert_eq!(alpha.lines[0].width, 9);
    }

    #[test]
    fn test_shell_menu_scheme_discovery_uses_deterministic_order() {
        use std::fs;
        use std::time::{SystemTime, UNIX_EPOCH};

        let _guard = shell_global_test_lock()
            .lock()
            .unwrap_or_else(|e| e.into_inner());

        struct CwdGuard(PathBuf);
        impl Drop for CwdGuard {
            fn drop(&mut self) {
                let _ = std::env::set_current_dir(&self.0);
            }
        }

        struct ModDirGuard(Option<String>);
        impl Drop for ModDirGuard {
            fn drop(&mut self) {
                if let Some(global) = get_global_data() {
                    global.write().mod_dir = self.0.take().unwrap_or_default();
                }
            }
        }

        let original_dir = std::env::current_dir().unwrap();
        let _cwd_guard = CwdGuard(original_dir);

        let temp_root = std::env::temp_dir().join(format!(
            "shell_menu_scheme_order_{}_{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(temp_root.join("Data/INI/Default")).unwrap();
        fs::create_dir_all(temp_root.join("Data/INI")).unwrap();
        fs::create_dir_all(
            temp_root.join("windows_game/extracted_big_files/INIZH/Data/INI/Default"),
        )
        .unwrap();
        fs::create_dir_all(temp_root.join("windows_game/extracted_big_files/INIZH/Data/INI"))
            .unwrap();
        fs::create_dir_all(
            temp_root.join("windows_game/extracted_big_files_v2/INIZH/Data/INI/Default"),
        )
        .unwrap();
        fs::create_dir_all(temp_root.join("windows_game/extracted_big_files_v2/INIZH/Data/INI"))
            .unwrap();

        for path in [
            temp_root.join("Data/INI/Default/ShellMenuScheme.ini"),
            temp_root.join("Data/INI/ShellMenuScheme.ini"),
            temp_root.join(
                "windows_game/extracted_big_files/INIZH/Data/INI/Default/ShellMenuScheme.ini",
            ),
            temp_root.join("windows_game/extracted_big_files/INIZH/Data/INI/ShellMenuScheme.ini"),
            temp_root.join(
                "windows_game/extracted_big_files_v2/INIZH/Data/INI/Default/ShellMenuScheme.ini",
            ),
            temp_root
                .join("windows_game/extracted_big_files_v2/INIZH/Data/INI/ShellMenuScheme.ini"),
        ] {
            fs::write(path, b"").unwrap();
        }

        std::env::set_current_dir(&temp_root).unwrap();

        let old_mod_dir = if let Some(global) = get_global_data() {
            let mut global = global.write();
            let old = global.mod_dir.clone();
            global.mod_dir.clear();
            Some(old)
        } else {
            None
        };
        let _mod_dir_guard = ModDirGuard(old_mod_dir);

        let files = discover_shell_menu_scheme_ini_files();
        let expected = vec![
            fs::canonicalize(temp_root.join("Data/INI/Default/ShellMenuScheme.ini")).unwrap(),
            fs::canonicalize(temp_root.join("Data/INI/ShellMenuScheme.ini")).unwrap(),
            fs::canonicalize(temp_root.join(
                "windows_game/extracted_big_files/INIZH/Data/INI/Default/ShellMenuScheme.ini",
            ))
            .unwrap(),
            fs::canonicalize(
                temp_root
                    .join("windows_game/extracted_big_files/INIZH/Data/INI/ShellMenuScheme.ini"),
            )
            .unwrap(),
            fs::canonicalize(temp_root.join(
                "windows_game/extracted_big_files_v2/INIZH/Data/INI/Default/ShellMenuScheme.ini",
            ))
            .unwrap(),
            fs::canonicalize(
                temp_root
                    .join("windows_game/extracted_big_files_v2/INIZH/Data/INI/ShellMenuScheme.ini"),
            )
            .unwrap(),
        ];

        assert!(
            files.ends_with(&expected),
            "expected temp roots at end of discovery order; files={files:?}"
        );
    }

    #[test]
    fn test_animation_manager() {
        let mut manager = AnimateWindowManager::new();
        assert!(manager.is_finished());

        let window = Rc::new(RefCell::new(GameWindow::new()));
        manager.register_window(window, AnimationType::SlideRight, true, 100, 0);
        assert!(!manager.is_finished());

        // Animation should not be finished immediately
        manager.update();
        // Note: In a real test, we'd need to wait for the duration to pass
    }

    #[test]
    fn animation_slide_start_positions_match_cpp_display_width_travel() {
        let mut manager = AnimateWindowManager::new();
        manager.set_screen_size(800, 600);

        let right_window = Rc::new(RefCell::new(GameWindow::new()));
        right_window.borrow_mut().set_position(25, 40).unwrap();
        right_window.borrow_mut().set_size(120, 80).unwrap();
        manager.register_window(
            right_window.clone(),
            AnimationType::SlideRight,
            false,
            100,
            0,
        );
        assert_eq!(right_window.borrow().get_position(), (825, 40));

        let top_window = Rc::new(RefCell::new(GameWindow::new()));
        top_window.borrow_mut().set_position(25, 40).unwrap();
        top_window.borrow_mut().set_size(120, 80).unwrap();
        manager.register_window(top_window.clone(), AnimationType::SlideTop, false, 100, 0);
        assert_eq!(top_window.borrow().get_position(), (25, -760));
    }

    #[test]
    fn shell_register_with_animate_manager_respects_global_animate_windows_flag() {
        let _guard = shell_global_test_lock()
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        let global = game_engine::ini::ini_game_data::ensure_global_data();
        let old_animate_windows = {
            let mut data = global.write();
            let old = data.animate_windows;
            data.animate_windows = false;
            old
        };

        let mut shell = Shell::new();
        let window = Rc::new(RefCell::new(GameWindow::new()));
        window.borrow_mut().set_position(25, 40).unwrap();
        window.borrow_mut().set_size(120, 80).unwrap();

        shell.register_with_animate_manager(window.clone(), AnimationType::SlideRight, true, 0);

        assert_eq!(window.borrow().get_position(), (25, 40));
        assert!(shell.animate_window_manager.is_empty());

        global.write().animate_windows = old_animate_windows;
    }

    #[test]
    fn test_animation_types() {
        let anim = AnimationType::SlideRight;
        assert_eq!(anim, AnimationType::SlideRight);
        assert_ne!(anim, AnimationType::Spiral);
    }

    #[test]
    fn test_window_rect() {
        let rect = WindowRect::new(10, 20, 100, 200);
        assert_eq!(rect.x, 10);
        assert_eq!(rect.y, 20);
        assert_eq!(rect.width, 100);
        assert_eq!(rect.height, 200);

        let zero = WindowRect::zero();
        assert_eq!(zero.x, 0);
        assert_eq!(zero.width, 0);
    }

    #[test]
    fn test_global_shell() {
        let mut shell = get_shell();
        assert!(shell.init().is_ok());
        assert!(shell.is_shell_active());
    }

    #[test]
    fn reentrant_shell_operations_drain_at_lifecycle_boundary_and_survive_unwind() {
        let _guard = shell_global_test_lock()
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        let events = StdRc::new(StdRefCell::new(Vec::new()));

        let pop = with_shell_mut(|shell| {
            shell.screen_stack.clear();
            shell.initialized = true;
            shell
                .screen_stack
                .push(Box::new(ReentrantQueueLayout::new(events.clone())));
            shell.pop_immediate()
        });
        assert!(matches!(pop, Some(Ok(()))));
        assert_eq!(
            events.borrow().as_slice(),
            &["shutdown", "destroy", "queued"],
            "a layout callback queues work, which runs after pop lifecycle work but before the outer shell borrow releases"
        );

        let unwind_events = StdRc::new(StdRefCell::new(Vec::new()));
        let panic_events = unwind_events.clone();
        let trailing_events = unwind_events.clone();
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _ = with_shell_mut(|_| {
                queue_shell_operation(move |_| {
                    panic_events.borrow_mut().push("panicking".to_string());
                    panic!("intentional queued shell operation panic");
                });
                queue_shell_operation(move |_| {
                    trailing_events.borrow_mut().push("trailing".to_string());
                });
            });
        }));
        assert!(result.is_err());
        assert_eq!(unwind_events.borrow().as_slice(), &["panicking"]);

        let _ = with_shell_mut(|_| ());
        assert_eq!(
            unwind_events.borrow().as_slice(),
            &["panicking", "trailing"],
            "unstarted queued work must survive the previous callback unwind"
        );
    }

    #[test]
    fn test_shell_special_layouts() {
        let mut shell = Shell::new();
        shell.init().unwrap();

        // Test save/load menu layout
        let _layout = shell.get_save_load_menu_layout().unwrap();

        // Test popup replay layout
        let _layout = shell.get_popup_replay_layout().unwrap();

        // Test options layout
        let layout = shell.get_options_layout(true);
        assert!(layout.is_some());

        shell.destroy_options_layout();
        let layout = shell.get_options_layout(false);
        assert!(layout.is_none());
    }

    #[test]
    fn test_find_screen_by_filename_is_case_insensitive() {
        let mut shell = Shell::new();
        shell.init().unwrap();
        shell.screen_stack.push(Box::new(BasicWindowLayout::new(
            "Menus/MainMenu.wnd".to_string(),
        )));

        assert!(
            shell
                .find_screen_by_filename("menus/mainmenu.wnd")
                .is_some()
        );
    }

    #[test]
    fn test_shell_show_hide() {
        let _guard = shell_global_test_lock()
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        game_engine::common::ini::ini_game_data::init_global_data();
        if let Some(global) = get_global_data() {
            let mut global = global.write();
            global.initial_file.clear();
            global.shell_map_on = true;
        }
        if let Ok(mut logic) = gamelogic::system::game_logic::get_game_logic().lock() {
            logic.set_game_mode(GAME_NONE);
        }

        let mut shell = Shell::new();
        shell.init().unwrap();

        // Test hide/show functionality
        shell.hide(true);
        shell.hide(false);

        // Test shell map functionality
        shell.show_shell_map(true);
        assert!(shell.shell_map_on);

        shell.show_shell_map(false);
        assert!(!shell.shell_map_on);
    }

    #[test]
    fn test_show_shell_does_not_push_main_menu_when_shell_map_is_on() {
        let _guard = shell_global_test_lock()
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        game_engine::common::ini::ini_game_data::init_global_data();
        if let Some(global) = get_global_data() {
            let mut global = global.write();
            global.initial_file.clear();
            global.shell_map_on = true;
        }
        if let Ok(mut logic) = gamelogic::system::game_logic::get_game_logic().lock() {
            logic.set_game_mode(GAME_NONE);
        }

        let mut shell = Shell::new();
        shell.init().unwrap();
        shell.show_shell_map(true);
        shell.show_shell(false).unwrap();
        assert_eq!(shell.get_screen_count(), 0);
    }

    #[test]
    fn shell_game_start_pushes_main_menu_when_stack_is_empty_like_cpp() {
        let mut shell = Shell::new();
        shell.init().unwrap();

        shell.show_main_menu_after_shell_game_start().unwrap();

        assert_eq!(shell.get_screen_count(), 1);
        assert_eq!(
            shell.top().map(|layout| layout.get_filename().to_string()),
            Some("Menus/MainMenu.wnd".to_string())
        );
    }

    #[test]
    fn shell_game_start_reveals_existing_top_screen_like_cpp() {
        let mut shell = Shell::new();
        shell.init().unwrap();
        let events = StdRc::new(StdRefCell::new(Vec::new()));
        shell.screen_stack.push(Box::new(TestLayout::new(
            "existing.wnd",
            true,
            events.clone(),
        )));

        shell.show_main_menu_after_shell_game_start().unwrap();

        assert_eq!(shell.get_screen_count(), 1);
        assert_eq!(
            events.borrow().as_slice(),
            &[
                "hide:existing.wnd:false".to_string(),
                "bring_forward:existing.wnd".to_string(),
            ]
        );
    }

    #[test]
    fn test_show_shell_map_reapplies_background_image_status() {
        let _guard = shell_global_test_lock()
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        game_engine::common::ini::ini_game_data::init_global_data();
        if let Some(global) = get_global_data() {
            let mut global = global.write();
            global.initial_file.clear();
            global.shell_map_on = false;
        }

        let events = StdRc::new(StdRefCell::new(Vec::new()));
        let mut shell = Shell::new();
        shell.init().unwrap();
        shell.background = Some(Box::new(TestLayout::new(
            "background.wnd",
            false,
            events.clone(),
        )));

        shell.show_shell_map(false);

        let event_log = events.borrow();
        let image_index = event_log
            .iter()
            .position(|event| event == "set_first_window_image:background.wnd")
            .expect("expected background image status to be reapplied");
        let hide_index = event_log
            .iter()
            .position(|event| event == "hide:background.wnd:false")
            .expect("expected background to be shown");
        assert!(image_index < hide_index);
    }

    #[test]
    fn test_shutdown_complete_keeps_clear_background_when_no_background_exists() {
        let mut shell = Shell::new();
        shell.clear_background = true;

        shell.shutdown_complete(None, false).unwrap();

        assert!(shell.clear_background);
    }

    #[test]
    fn test_reset_keeps_special_layouts_like_cpp() {
        let events = StdRc::new(StdRefCell::new(Vec::new()));
        let mut shell = Shell::new();
        shell.init().unwrap();
        shell.screen_stack.push(Box::new(TestLayout::new(
            "stack.wnd",
            false,
            events.clone(),
        )));
        shell.save_load_menu_layout = Some(Box::new(TestLayout::new(
            "save_load.wnd",
            false,
            events.clone(),
        )));
        shell.popup_replay_layout = Some(Box::new(TestLayout::new(
            "popup_replay.wnd",
            false,
            events.clone(),
        )));
        shell.options_layout = Some(Box::new(TestLayout::new(
            "options.wnd",
            false,
            events.clone(),
        )));
        shell.background = Some(Box::new(TestLayout::new(
            "background.wnd",
            false,
            events.clone(),
        )));
        shell.clear_background = true;
        shell.pending_push = true;
        shell.pending_pop = true;
        shell.pending_push_name = "Menus/MainMenu.wnd".to_string();
        shell.last_update = Instant::now() - shell.update_interval;

        shell.reset().unwrap();

        assert_eq!(shell.get_screen_count(), 0);
        assert!(shell.save_load_menu_layout.is_some());
        assert!(shell.popup_replay_layout.is_some());
        assert!(shell.options_layout.is_some());
        assert!(shell.background.is_some());
        assert!(shell.clear_background);
        assert!(shell.pending_push);
        assert!(!shell.pending_pop);
        assert_eq!(shell.pending_push_name, "Menus/MainMenu.wnd");

        let event_log = events.borrow();
        assert!(event_log.iter().any(|event| event == "destroy:stack.wnd"));
        assert!(
            !event_log
                .iter()
                .any(|event| event == "destroy:save_load.wnd")
        );
        assert!(
            !event_log
                .iter()
                .any(|event| event == "destroy:popup_replay.wnd")
        );
        assert!(!event_log.iter().any(|event| event == "destroy:options.wnd"));
        assert!(
            !event_log
                .iter()
                .any(|event| event == "destroy:background.wnd")
        );
    }

    #[test]
    fn test_push_hidden_top_completes_immediately_like_cpp() {
        let events = StdRc::new(StdRefCell::new(Vec::new()));
        let mut shell = Shell::new();
        shell.init().unwrap();
        shell.screen_stack.push(Box::new(TestLayout::new(
            "hidden_top.wnd",
            true,
            events.clone(),
        )));

        shell.push("Menus/MainMenu.wnd", false).unwrap();

        assert_eq!(shell.get_screen_count(), 2);
        assert!(shell.pending_push_name.is_empty());
        assert!(!shell.pending_push);
        let event_log = events.borrow();
        assert!(
            !event_log
                .iter()
                .any(|event| event == "shutdown:hidden_top.wnd"),
            "hidden top should not run shutdown before immediate push completion"
        );
        assert!(
            !event_log
                .iter()
                .any(|event| event == "hide:hidden_top.wnd:true"),
            "C++ Shell::shutdownComplete() does not re-hide the current top during a pending push"
        );
    }

    #[test]
    fn test_pop_immediate_runs_shutdown_before_destroy() {
        let events = StdRc::new(StdRefCell::new(Vec::new()));
        let mut shell = Shell::new();
        shell.init().unwrap();
        shell.screen_stack.push(Box::new(TestLayout::new(
            "first.wnd",
            false,
            events.clone(),
        )));
        shell
            .screen_stack
            .push(Box::new(TestLayout::new("top.wnd", false, events.clone())));

        shell.pop_immediate().unwrap();

        let event_log = events.borrow();
        let shutdown_index = event_log
            .iter()
            .position(|event| event == "shutdown:top.wnd")
            .unwrap();
        let destroy_index = event_log
            .iter()
            .position(|event| event == "destroy:top.wnd")
            .unwrap();
        assert!(shutdown_index < destroy_index);
        assert_eq!(shell.get_screen_count(), 1);
    }
}
