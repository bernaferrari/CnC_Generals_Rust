use std::env;

fn main() {
    println!("cargo:rerun-if-changed=build.rs");

    let target_os = env::var("CARGO_CFG_TARGET_OS").unwrap();

    // GPU-specific configurations per platform
    match target_os.as_str() {
        "windows" => configure_windows_gpu(),
        "macos" => configure_macos_gpu(),
        "linux" => configure_linux_gpu(),
        _ => eprintln!("Warning: Unsupported target OS for GPU: {}", target_os),
    }
}

fn configure_windows_gpu() {
    // WGPU-only: do not link D3D11/12/DXGI/D3DCompiler. Optional `d3d`
    // feature re-enables the native DirectX loader for diagnostics only.
    if env::var_os("CARGO_FEATURE_D3D").is_some() {
        println!("cargo:rustc-link-lib=d3d11");
        println!("cargo:rustc-link-lib=d3d12");
        println!("cargo:rustc-link-lib=dxgi");
        println!("cargo:rustc-link-lib=d3dcompiler");
        println!("cargo:rustc-link-lib=dxguid");
    }
}

fn configure_macos_gpu() {
    // Metal framework
    println!("cargo:rustc-link-lib=framework=Metal");
    println!("cargo:rustc-link-lib=framework=MetalKit");
    println!("cargo:rustc-link-lib=framework=MetalPerformanceShaders");
    println!("cargo:rustc-link-lib=framework=QuartzCore");
}

fn configure_linux_gpu() {
    // Vulkan
    println!("cargo:rustc-link-lib=vulkan");

    // OpenGL fallback
    println!("cargo:rustc-link-lib=GL");
    println!("cargo:rustc-link-lib=GLX");
    println!("cargo:rustc-link-lib=EGL");
}
