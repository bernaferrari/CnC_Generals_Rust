use std::env;

fn main() {
    println!("cargo:rerun-if-changed=build.rs");

    let target_os = env::var("CARGO_CFG_TARGET_OS").unwrap();

    // Platform-specific device configurations
    match target_os.as_str() {
        "windows" => configure_windows_device(),
        "macos" => configure_macos_device(),
        "linux" => configure_linux_device(),
        _ => eprintln!(
            "Warning: Unsupported target OS for device layer: {}",
            target_os
        ),
    }
}

fn configure_windows_device() {
    // DirectX and Windows multimedia
    println!("cargo:rustc-link-lib=d3d11");
    println!("cargo:rustc-link-lib=dxgi");
    println!("cargo:rustc-link-lib=d3dcompiler");
    println!("cargo:rustc-link-lib=dinput8");
    println!("cargo:rustc-link-lib=xinput");

    // Audio
    println!("cargo:rustc-link-lib=dsound");
    println!("cargo:rustc-link-lib=winmm");

    // Window management
    println!("cargo:rustc-link-lib=user32");
    println!("cargo:rustc-link-lib=gdi32");
}

fn configure_macos_device() {
    // Graphics and window management
    println!("cargo:rustc-link-lib=framework=Metal");
    println!("cargo:rustc-link-lib=framework=MetalKit");
    println!("cargo:rustc-link-lib=framework=QuartzCore");
    println!("cargo:rustc-link-lib=framework=Cocoa");
    println!("cargo:rustc-link-lib=framework=AppKit");

    // Audio
    println!("cargo:rustc-link-lib=framework=CoreAudio");
    println!("cargo:rustc-link-lib=framework=AudioToolbox");
    println!("cargo:rustc-link-lib=framework=AudioUnit");

    // Input
    println!("cargo:rustc-link-lib=framework=IOKit");
    println!("cargo:rustc-link-lib=framework=GameController");
}

fn configure_linux_device() {
    // Graphics
    println!("cargo:rustc-link-lib=vulkan");
    println!("cargo:rustc-link-lib=GL");
    println!("cargo:rustc-link-lib=EGL");

    // Window management
    println!("cargo:rustc-link-lib=X11");
    println!("cargo:rustc-link-lib=Xcursor");
    println!("cargo:rustc-link-lib=Xi");
    println!("cargo:rustc-link-lib=Xrandr");
    println!("cargo:rustc-link-lib=Xxf86vm");

    // Audio
    println!("cargo:rustc-link-lib=asound");
    println!("cargo:rustc-link-lib=pulse");

    // Input
    println!("cargo:rustc-link-lib=udev");
}
