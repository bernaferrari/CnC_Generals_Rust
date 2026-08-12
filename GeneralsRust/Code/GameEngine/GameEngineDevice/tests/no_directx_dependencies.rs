//! The production device layer may use the operating system for input and WASAPI,
//! but rendering must stay behind wgpu rather than linking a direct D3D path.

const MANIFEST: &str = include_str!("../Cargo.toml");
const DEVICE_BUILD: &str = include_str!("../build.rs");
const GPU_BUILD: &str =
    include_str!("../../../Libraries/Source/WWVegas/WW3D2/crates/ww3d-gpu/build.rs");
const MAIN_BUILD: &str = include_str!("../../../Main/build.rs");
const MAIN_MANIFEST: &str = include_str!("../../../Main/Cargo.toml");
const PLATFORM_FEATURES: &str = include_str!("../src/platform/mod.rs");
const CAPABILITY_DETECTION: &str = include_str!("../src/platform/device_interface.rs");

#[test]
fn device_manifest_has_no_directx_backend_dependencies() {
    for forbidden in [
        "gpu-allocator = { version = \"0.25\", optional = true, features = [\"vulkan\", \"d3d12\"] }",
        "Win32_Media_Audio_DirectSound",
        "Win32_Graphics_Direct3D11",
        "Win32_Graphics_Dxgi",
    ] {
        assert!(
            !MANIFEST.contains(forbidden),
            "production device manifest must not opt into a direct DirectX backend: {forbidden}"
        );
    }
}

#[test]
fn default_windows_build_scripts_do_not_unconditionally_link_d3d_or_dsound() {
    for (label, src) in [
        ("game_engine_device/build.rs", DEVICE_BUILD),
        ("ww3d-gpu/build.rs", GPU_BUILD),
        ("Main/build.rs", MAIN_BUILD),
    ] {
        for lib in ["d3d11", "d3d12", "dxgi", "d3dcompiler", "dsound"] {
            let needle = format!("cargo:rustc-link-lib={lib}");
            // Allowed only inside an explicit feature/env gate.
            for line in src.lines() {
                let trimmed = line.trim();
                if trimmed.starts_with("println!") && trimmed.contains(&needle) {
                    assert!(
                        src.contains("CARGO_FEATURE_D3D") || src.contains("feature = \"d3d\""),
                        "{label} links {lib} without a d3d feature gate: {trimmed}"
                    );
                }
            }
        }
    }
}

#[test]
fn main_default_features_do_not_enable_d3d_on_ww3d_gpu() {
    assert!(
        !MAIN_MANIFEST.contains("ww3d-gpu") || !MAIN_MANIFEST.contains("features = [\"d3d\"]"),
        "Main must not enable ww3d-gpu/d3d on the default graph"
    );
    assert!(
        MAIN_MANIFEST.contains("wgpu = {"),
        "Main must keep wgpu as the graphics backend"
    );
}

#[test]
fn device_manifest_uses_wgpu_for_video() {
    assert!(MANIFEST.contains("wgpu = {"));
    assert!(MANIFEST.contains("video = [\n    \"wgpu\","));
}

#[test]
fn platform_capability_detection_does_not_claim_legacy_directx_support() {
    assert!(!PLATFORM_FEATURES.contains("PlatformFeature::DirectSound) => true"));
    assert!(!PLATFORM_FEATURES.contains("PlatformFeature::DirectX) => true"));
    assert!(!CAPABILITY_DETECTION.contains("            PlatformFeature::DirectSound,"));
    assert!(!CAPABILITY_DETECTION.contains("            PlatformFeature::DirectX,"));
}
