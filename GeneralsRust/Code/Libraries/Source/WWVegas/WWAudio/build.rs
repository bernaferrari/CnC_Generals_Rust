fn main() {
    println!("cargo:rerun-if-changed=build.rs");

    // Build scripts run on the host; select libraries for Cargo's target.
    match std::env::var("CARGO_CFG_TARGET_OS").as_deref() {
        Ok("windows") => println!("cargo:rustc-link-lib=winmm"),
        Ok("linux") => println!("cargo:rustc-link-lib=asound"),
        Ok("macos") => {
            println!("cargo:rustc-link-lib=framework=AudioToolbox");
            println!("cargo:rustc-link-lib=framework=CoreAudio");
        }
        _ => {}
    }
}
