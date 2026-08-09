//! The production device layer may use the operating system for input and WASAPI,
//! but rendering must stay behind wgpu rather than linking a direct D3D path.

const MANIFEST: &str = include_str!("../Cargo.toml");
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
