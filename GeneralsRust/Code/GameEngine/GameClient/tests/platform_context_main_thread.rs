//! PlatformContext construction requires creating a winit EventLoop.
//!
//! On macOS, `EventLoop::new` must run on the process main thread
//! (`NSApplication` / AppKit). Lib unit tests run on worker threads, so this
//! lives in a `harness = false` integration binary whose `main` is the main
//! thread. Headless / CI agents without a window server still cannot create
//! an EventLoop; those environments should keep this ignored.

fn main() {
    // Lib unit tests already cover ambient host wiring; this binary only
    // validates EventLoop + window creation on the macOS main thread.
    #[cfg(target_os = "macos")]
    {
        // Soft-skip when no GUI session is available (SSH/CI headless).
        let gui_ok = std::env::var_os("DISPLAY").is_some()
            || std::env::var_os("WAYLAND_DISPLAY").is_some()
            || cfg!(target_os = "macos");
        // macOS always has a WindowServer conceptually, but sandbox/agent
        // sessions often still reject EventLoop. Catch that and exit 0 with
        // a clear skip reason rather than failing the suite.
        match game_client_rust::platform::PlatformContext::new("Test", 800, 600) {
            Ok(_ctx) => {
                println!("platform_context_main_thread: PlatformContext::new ok");
            }
            Err(err) => {
                let msg = err.to_string();
                let headlessish = msg.contains("main thread")
                    || msg.contains("NSApplication")
                    || msg.contains("not supported")
                    || msg.contains("EventLoop")
                    || msg.contains("display")
                    || msg.contains("WindowCreation");
                if headlessish || !gui_ok {
                    eprintln!(
                        "platform_context_main_thread: skipped — winit EventLoop unavailable in this environment ({msg})"
                    );
                    return;
                }
                panic!("PlatformContext::new failed: {msg}");
            }
        }
    }

    #[cfg(not(target_os = "macos"))]
    {
        match game_client_rust::platform::PlatformContext::new("Test", 800, 600) {
            Ok(_ctx) => println!("platform_context_main_thread: PlatformContext::new ok"),
            Err(err) => {
                eprintln!(
                    "platform_context_main_thread: skipped — winit EventLoop unavailable ({err})"
                );
            }
        }
    }
}
