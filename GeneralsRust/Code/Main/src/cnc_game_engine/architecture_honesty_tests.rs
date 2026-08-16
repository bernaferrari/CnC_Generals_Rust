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
    let vector3 = include_str!(
        "../../../Libraries/Source/WWVegas/WWMath/src/vector3.rs"
    );
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
    let game_audio = include_str!(
        "../../../GameEngine/Common/src/common/audio/game_audio.rs"
    );
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
