//! Architecture-honesty locks for accepted process/backend differences.
//!
//! These are not "fixed by rewrite" beads. They fail if the live host drifts
//! away from the accepted C++ analog.

#[test]
fn live_host_maps_cpp_gamemain_init_execute_to_cnc_new_and_loop() {
    // C++ GameMain.cpp:16-20:
    //   TheGameEngine = CreateGameEngine();
    //   TheGameEngine->init(argc, argv);
    //   TheGameEngine->execute();
    //   delete TheGameEngine;
    // C++ WinMain.cpp is the OS entry (CreateWindow / message pump).
    // Rust OS entry is winit; behavioral analog is host tick, not WinMain text.
    let main_src = include_str!("../../src/main.rs");
    assert!(
        main_src.contains("async fn game_main(")
            && main_src.contains("generals_main::cnc_game_engine::run_cnc_game("),
        "live GameMain must dispatch run_cnc_game, not Common leftover GameEngine::execute"
    );

    let run_loop = include_str!("run_loop.rs");
    let boot = include_str!("boot.rs");
    assert!(
        boot.contains("pub async fn new(window: Arc<Window>, command_line: Arc<CommandLineArgs>)"),
        "CnCGameEngine::new is the live GameEngine::init analog"
    );
    assert!(
        run_loop.contains("engine_init_future = Some(Box::pin(CnCGameEngine::new("),
        "run_cnc_game must construct CnCGameEngine (init) before the host tick"
    );
    assert!(
        run_loop.contains("engine.update_with_timing(&timing)")
            && run_loop.contains("engine.update_with_frame_clock()")
            && run_loop.contains("pub async fn run_cnc_game("),
        "run_cnc_game drive_frame is the live GameEngine::execute analog"
    );
}

#[test]
fn live_numeric_stack_is_glam_not_wwmath_crate() {
    // C++ WWMath / Vector3 live in WWMath.lib.
    // Accepted backend: glam is the allowed math analog of wgpu.
    // WWMath crate Vector3 is a glam alias (vector3.rs); Main does not depend on wwmath.
    let vector3 = include_str!("../../../Libraries/Source/WWVegas/WWMath/src/vector3.rs");
    assert!(
        vector3.contains("use glam::Vec3") && vector3.contains("pub type Vector3 = Vec3"),
        "WWMath Vector3 must remain a glam::Vec3 alias, not a second numeric stack"
    );

    let main_cargo = include_str!("../../Cargo.toml");
    assert!(
        main_cargo.contains("glam = { version = \"0.28\""),
        "live Main host must depend on glam"
    );
    assert!(
        !main_cargo.contains("wwmath") && !main_cargo.contains("math_utilities"),
        "live Main host must not depend on the leftover WWMath crate"
    );
}

#[test]
fn live_the_audio_is_common_audio_manager_rodio_not_wwaudio() {
    // C++ TheAudio is AudioManager (Miles backend).
    // Live Rust TheAudio is Common game_audio::THE_AUDIO + register_rodio_playback_hook.
    // WWAudio crate is a library leftover (not a Main/Common/GameClient/GameLogic dep).
    let game_audio = include_str!("../../../GameEngine/Common/src/common/audio/game_audio.rs");
    assert!(
        game_audio.contains("static THE_AUDIO: OnceLock<Arc<Mutex<AudioManager>>>")
            && game_audio.contains("pub fn initialize_global_audio_manager()")
            && game_audio.contains("register_rodio_playback_hook()"),
        "live TheAudio must be Common AudioManager initialized with the rodio hook"
    );

    let main_cargo = include_str!("../../Cargo.toml");
    assert!(
        !main_cargo.contains("wwaudio"),
        "live Main host must not depend on leftover WWAudio crate"
    );
}

#[test]
fn live_run_loop_updates_common_the_audio_each_frame() {
    // C++ GameEngine::update TheAudio->UPDATE() (GameEngine.cpp:736).
    // Pre-fix leftover AudioManagerSubsystem ticked host rodio only;
    // Common THE_AUDIO never processed AR_Play on the live frame.
    let run_loop = include_str!("run_loop.rs");
    let audio = include_str!("audio.rs");
    let boot = include_str!("boot.rs");
    assert!(
        run_loop.contains("engine.host_update_the_audio()"),
        "live run_loop drive_frame must call TheAudio update before engine.update"
    );
    assert!(
        audio.contains("TheAudio::get()")
            && audio.contains("audio.update()")
            && audio.contains("GameEngine.cpp:736"),
        "host_update_the_audio must call Common TheAudio::update"
    );
    assert!(
        audio.contains("sync_audio_listener_from_main_camera")
            && audio.contains("camera_position")
            && audio.contains("camera_target")
            && audio.contains("GameAudio.cpp:281"),
        "host_update_the_audio must sync 3D listener/mic from Main camera"
    );
    assert!(
        audio.contains("set_eva_host_frame")
            && audio.contains("set_eva_host_sufficient_power")
            && audio.contains("update_eva_system")
            && audio.contains("Eva.cpp:264"),
        "live audio tick must publish host frame/energy and run Eva::update"
    );
    let subsystem = include_str!("../subsystem_manager.rs");
    assert!(
        subsystem.contains("play_sound_through_the_audio_at")
            && subsystem.contains("event.position.map")
            && subsystem.contains("event.object_id.map")
            && subsystem.contains("leftover_the_audio_is_live")
            && subsystem.contains("playSample3D"),
        "live SFX drain must keep pose+object_id and not rodio leftover-muted"
    );
    assert!(
        boot.contains("initialize_global_audio_manager()") && boot.contains("createAudioManager"),
        "live boot must construct Common THE_AUDIO like GameEngine::init"
    );
    assert!(
        boot.contains("initialize_eva_system()") && boot.contains("Eva.cpp:43-57"),
        "live boot must load Eva.ini into TheEva like GameClient::init"
    );

    let game_audio = include_str!("../../../GameEngine/Common/src/common/audio/game_audio.rs");
    assert!(
        game_audio.contains("fn load_audio_event_inis()")
            && game_audio.contains("Data/INI/SoundEffects.ini")
            && game_audio.contains("Data/INI/Speech.ini")
            && game_audio.contains("Data/INI/Voice.ini")
            && game_audio.contains("Data/INI/Music.ini"),
        "THE_AUDIO init must load Music/SoundEffects/Speech/Voice INIs"
    );
}

#[test]
fn live_world_clicks_follow_cpp_place_gui_selection_command_precedence() {
    // C++ GameClient.cpp:273-282 attach order (lower number first):
    //   WindowTranslator 10, PlaceEventTranslator 30, GUICommandTranslator 40,
    //   SelectionTranslator 50, LookAtTranslator 60, CommandTranslator 70.
    // C++ CommandXlat.cpp:3656-3744 / SelectionXlat.cpp:1007-1023:
    //   Place/GUI keep LMB in both mouse layouts; classic LMB is context and
    //   RMB is cancel/deselect; alternate LMB is select and RMB is context.
    // Live host is the sole command authority (no dual-own through
    // THE_MESSAGE_STREAM). These functions are the live translator equivalent.
    let input = include_str!("input.rs");
    let mouse = super::ENGINE_SRC;

    let roles = input
        .find("fn world_mouse_action(")
        .expect("world_mouse_action");
    let roles_body = &input[roles..roles + 800];
    assert!(
        roles_body.contains("MouseButton::Left if targeting_active => Some(WorldMouseAction::Targeting)")
            && roles_body.contains("MouseButton::Left if use_alternate_mouse => Some(WorldMouseAction::Selection)")
            && roles_body.contains("MouseButton::Left => Some(WorldMouseAction::ContextCommand)")
            && roles_body.contains("MouseButton::Right if use_alternate_mouse => Some(WorldMouseAction::ContextCommand)")
            && roles_body.contains("MouseButton::Right => Some(WorldMouseAction::CancelOrDeselect)"),
        "live world_mouse_action must keep C++ Place/GUI LMB + classic/alternate roles"
    );

    let left = mouse
        .find("fn handle_left_click(")
        .expect("handle_left_click");
    let left_end = mouse[left + 1..]
        .find("\n    fn ")
        .map(|i| left + 1 + i)
        .unwrap_or(left + 2500);
    let left_body = &mouse[left..left_end];
    let place = left_body
        .find("pending_structure_placement")
        .expect("PlaceEvent analog first");
    let gui = left_body
        .find("pending_map_command")
        .expect("GUICommand analog second");
    assert!(
        place < gui,
        "handle_left_click must be Place then GUI, not GUI-first"
    );
    assert!(
        !left_body.contains("select_left_click_target"),
        "RAW LMB down must not commit SelectionXlat (C++ SelectionXlat.cpp:890-898)"
    );
    assert!(
        left_body.contains("classic_left_context_action_allowed"),
        "classic LMB must still probe CommandXlat context after Place/GUI"
    );

    let release = mouse
        .find("fn handle_left_release(")
        .expect("handle_left_release");
    let release_end = mouse[release..]
        .find("fn handle_right_click(")
        .map(|i| release + i)
        .unwrap_or(release + 12_000);
    let release_body = &mouse[release..release_end];
    assert!(
        release_body.contains("select_left_click_target")
            && release_body.contains("is_point_click_drag"),
        "MSG_MOUSE_LEFT_CLICK analog must commit selection on non-drag release"
    );
    assert!(
        !release_body.contains("force-select local object"),
        "empty LMB must not force-select CanSelectDrawable rejects"
    );

    let cancel = mouse
        .find("fn cancel_world_mouse_targeting(")
        .expect("cancel_world_mouse_targeting");
    let cancel_end = mouse[cancel + 1..]
        .find("\n    fn ")
        .map(|i| cancel + 1 + i)
        .unwrap_or(cancel + 800);
    let cancel_body = &mouse[cancel..cancel_end];
    let gui_cancel = cancel_body
        .find("pending_map_command")
        .expect("RMB cancels GUI command first");
    let place_cancel = cancel_body
        .find("pending_structure_placement")
        .expect("RMB cancels Place after GUI");
    assert!(
        gui_cancel < place_cancel,
        "SelectionXlat.cpp:1007-1023 cancels GUI without deselect before Place"
    );
    assert!(
        cancel_body.contains("deselect_world_selection_from_right_click"),
        "pending Place still deselects the builder (place source != 0)"
    );

    let handler = input
        .find("fn handle_mouse_button_input")
        .expect("handle_mouse_button_input");
    let handler_end = input[handler..]
        .find("fn inject_winit_equivalent_cursor_at")
        .map(|i| handler + i)
        .unwrap_or(handler + 4000);
    let handler_body = &input[handler..handler_end];
    let rmb = handler_body
        .find("(MouseButton::Right, false)")
        .expect("RMB release arm");
    let rmb_body = &handler_body[rmb..];
    let cancel_call = rmb_body
        .find("cancel_world_mouse_targeting")
        .expect("RMB must cancel targeting first");
    let context = rmb_body
        .find("WorldMouseAction::ContextCommand")
        .expect("alternate RMB context");
    let deselect = rmb_body
        .find("WorldMouseAction::CancelOrDeselect")
        .expect("classic RMB deselect");
    assert!(
        cancel_call < context && context < deselect,
        "RMB must cancel Place/GUI before Command/Selection roles"
    );

    assert!(
        !handler_body.contains("THE_MESSAGE_STREAM")
            && !handler_body.contains("RawMouseLeftButtonDown")
            && !handler_body.contains("append_message"),
        "live OS clicks must not dual-own through crate MessageStream"
    );
}
