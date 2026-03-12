use std::env;

fn main() {
    println!("cargo:rerun-if-changed=build.rs");

    let target_os = env::var("CARGO_CFG_TARGET_OS").unwrap();
    let target_arch = env::var("CARGO_CFG_TARGET_ARCH").unwrap();

    println!("cargo:rustc-env=TARGET_OS={}", target_os);
    println!("cargo:rustc-env=TARGET_ARCH={}", target_arch);

    // Platform-specific configurations
    match target_os.as_str() {
        "windows" => configure_windows(),
        "macos" => configure_macos(),
        "linux" => configure_linux(),
        _ => eprintln!("Warning: Unsupported target OS: {}", target_os),
    }

    // Architecture-specific optimizations
    match target_arch.as_str() {
        "x86_64" => {
            println!("cargo:rustc-env=TARGET_ARCH_FEATURES=sse2,sse3,sse4.1,sse4.2");
        }
        "aarch64" => {
            println!("cargo:rustc-env=TARGET_ARCH_FEATURES=neon");
        }
        _ => {}
    }
}

fn configure_windows() {
    println!("cargo:rustc-link-lib=user32");
    println!("cargo:rustc-link-lib=gdi32");
    println!("cargo:rustc-link-lib=shell32");
    println!("cargo:rustc-link-lib=ole32");
    println!("cargo:rustc-link-lib=winmm");
    println!("cargo:rustc-link-lib=dsound");

    // Enable Windows subsystem for GUI application
    println!("cargo:rustc-link-arg=/SUBSYSTEM:WINDOWS");
    println!("cargo:rustc-link-arg=/ENTRY:mainCRTStartup");

    // Windows-specific defines
    println!("cargo:rustc-cfg=platform=\"windows\"");
}

fn configure_macos() {
    println!("cargo:rustc-link-lib=framework=Cocoa");
    println!("cargo:rustc-link-lib=framework=IOKit");
    println!("cargo:rustc-link-lib=framework=CoreFoundation");
    println!("cargo:rustc-link-lib=framework=CoreGraphics");
    println!("cargo:rustc-link-lib=framework=Metal");
    println!("cargo:rustc-link-lib=framework=QuartzCore");
    println!("cargo:rustc-link-lib=framework=AudioToolbox");
    println!("cargo:rustc-link-lib=framework=CoreAudio");

    // macOS-specific defines
    println!("cargo:rustc-cfg=platform=\"macos\"");
}

fn configure_linux() {
    println!("cargo:rustc-link-lib=X11");
    println!("cargo:rustc-link-lib=Xi");
    println!("cargo:rustc-link-lib=Xrandr");
    println!("cargo:rustc-link-lib=asound");

    // Linux-specific defines
    println!("cargo:rustc-cfg=platform=\"linux\"");
}
