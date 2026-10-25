//! Architecture-honesty locks for accepted process/backend differences.
//!
//! These are not "fixed by rewrite" beads. They fail if the live host drifts
//! away from the accepted C++ analog.

#[test]
fn live_numeric_stack_is_glam_not_wwmath_crate() {
    // C++ WWMath / Vector3 live in WWMath.lib.
    // Accepted backend: glam is the allowed math analog of wgpu.
    // WWMath crate Vector3 is a glam alias (vector3.rs); Main does not depend on wwmath.

    let main_cargo = include_str!("../../Cargo.toml");
    let main_manifest: toml::Value = toml::from_str(main_cargo).expect("Main Cargo manifest");
    let workspace_manifest: toml::Value =
        toml::from_str(include_str!("../../../../Cargo.toml")).expect("workspace Cargo manifest");
    assert!(
        main_manifest["dependencies"]["glam"]["workspace"].as_bool() == Some(true)
            && workspace_manifest["workspace"]["dependencies"]["glam"]["version"]
                .as_str()
                .is_some_and(|version| !version.is_empty()),
        "live Main host must use the workspace-owned glam dependency"
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

    let main_cargo = include_str!("../../Cargo.toml");
    assert!(
        !main_cargo.contains("wwaudio"),
        "live Main host must not depend on leftover WWAudio crate"
    );
}
