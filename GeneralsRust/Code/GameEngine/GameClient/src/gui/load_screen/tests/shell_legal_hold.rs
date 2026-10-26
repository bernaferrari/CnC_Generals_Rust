// LoadScreen.cpp shell legal hold fixture cases; included in the original logical test module.

#[test]
fn shell_game_first_load_matches_cpp_title_and_legal_state() {
    if matches!(
        crate::test_process::run_bounded(
            module_path!(),
            "shell_game_first_load_matches_cpp_title_and_legal_state"
        ),
        crate::test_process::TestProcess::ParentVerified
    ) {
        return;
    }
    clear_load_screen_presentation_pump();
    let presentation_calls = Arc::new(AtomicUsize::new(0));
    let hook_presentation_calls = Arc::clone(&presentation_calls);
    register_load_screen_presentation_pump(move || {
        hook_presentation_calls.fetch_add(1, Ordering::SeqCst);
    });
    let mut owner = LoadScreenFixture::new();
    owner.with_runtime(|runtime, pump, _| {
        shell_fixture_windows(runtime);
        initialize_shell_fixture(true, runtime, pump);
        runtime.windows.with_manager_ref(|wm| {
            let root = wm
                .find_window_by_name("ShellGameLoadScreen.wnd:ParentShellGameLoadScreen")
                .expect("root");
            assert_eq!(
                root.borrow()
                    .get_enabled_draw_data(0)
                    .and_then(|draw| draw.image)
                    .map(|image| image.name),
                Some("TitleScreen".to_string())
            );
            let legal = wm
                .find_window_by_name("ShellGameLoadScreen.wnd:StaticTextLegal")
                .expect("legal");
            assert!(!legal.borrow().is_hidden());
            let progress = wm
                .find_window_by_name("ShellGameLoadScreen.wnd:ProgressLoad")
                .expect("progress");
            assert!(!progress.borrow().is_hidden());
        });
        assert!(!*runtime.first_load);
        assert_eq!(presentation_calls.load(Ordering::SeqCst), 1);
        shell_fixture_windows(runtime);
        initialize_shell_fixture(true, runtime, pump);
        runtime.windows.with_manager_ref(|wm| {
            let root = wm
                .find_window_by_name("ShellGameLoadScreen.wnd:ParentShellGameLoadScreen")
                .expect("root");
            assert!(
                root.borrow()
                    .get_enabled_draw_data(0)
                    .and_then(|draw| draw.image)
                    .is_none()
            );
            let legal = wm
                .find_window_by_name("ShellGameLoadScreen.wnd:StaticTextLegal")
                .expect("legal");
            assert!(legal.borrow().is_hidden());
        });
        assert!(!*runtime.first_load);
        let mut peer = LoadScreenFixture::new();
        peer.with_runtime(|runtime, pump, _| {
            shell_fixture_windows(runtime);
            initialize_shell_fixture(true, runtime, pump);
            assert!(!*runtime.first_load);
        });
        assert_eq!(presentation_calls.load(Ordering::SeqCst), 2);
    });
    clear_load_screen_presentation_pump();
}

#[test]
fn shell_game_first_load_skips_legal_intro_when_mem_check_fails_like_cpp() {
    if matches!(
        crate::test_process::run_bounded(
            module_path!(),
            "shell_game_first_load_skips_legal_intro_when_mem_check_fails_like_cpp"
        ),
        crate::test_process::TestProcess::ParentVerified
    ) {
        return;
    }
    let mut owner = LoadScreenFixture::new();
    owner.with_runtime(|runtime, pump, _| {
        shell_fixture_windows(runtime);
        initialize_shell_fixture(false, runtime, pump);
        runtime.windows.with_manager_ref(|wm| {
            let root = wm
                .find_window_by_name("ShellGameLoadScreen.wnd:ParentShellGameLoadScreen")
                .expect("root");
            assert!(
                root.borrow()
                    .get_enabled_draw_data(0)
                    .and_then(|draw| draw.image)
                    .is_none()
            );
            let legal = wm
                .find_window_by_name("ShellGameLoadScreen.wnd:StaticTextLegal")
                .expect("legal");
            assert!(legal.borrow().is_hidden());
        });
        shell_fixture_windows(runtime);
        initialize_shell_fixture(true, runtime, pump);
        runtime.windows.with_manager_ref(|wm| {
            assert_eq!(
                wm.find_window_by_name("ShellGameLoadScreen.wnd:ParentShellGameLoadScreen")
                    .expect("root")
                    .borrow()
                    .get_enabled_draw_data(0)
                    .and_then(|draw| draw.image)
                    .map(|image| image.name),
                Some("TitleScreen".to_string())
            );
            assert!(
                !wm.find_window_by_name("ShellGameLoadScreen.wnd:StaticTextLegal")
                    .expect("legal")
                    .borrow()
                    .is_hidden()
            );
        });
    });
}
