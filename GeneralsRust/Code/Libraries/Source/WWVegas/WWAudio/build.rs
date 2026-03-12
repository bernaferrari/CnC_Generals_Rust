fn main() {
    println!("cargo:rerun-if-changed=build.rs");

    // Set up platform-specific build flags
    #[cfg(target_os = "windows")]
    {
        println!("cargo:rustc-link-lib=dsound");
        println!("cargo:rustc-link-lib=winmm");
    }

    #[cfg(target_os = "linux")]
    {
        println!("cargo:rustc-link-lib=asound");
    }

    #[cfg(target_os = "macos")]
    {
        println!("cargo:rustc-link-lib=framework=AudioToolbox");
        println!("cargo:rustc-link-lib=framework=CoreAudio");
    }
}
