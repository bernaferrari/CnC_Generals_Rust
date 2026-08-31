// Lifecycle: initialization / asset bootstrap.
//
// Resolves the production `generals` binary and the Lone Eagle map, prepares
// the runtime-host control/status/frame files, and spawns the child. Failure
// paths write `binary_missing` / `spawn_failed` into the result and return
// `None` (mirrors the pre-split early returns of `run_executable_smoke_once`).

/// Spawned child plus its runtime-host file paths for one smoke run.
struct SmokeBootstrap {
    child: Child,
    tmp: PathBuf,
    control_path: PathBuf,
    status_path: PathBuf,
    map: String,
}

/// Resolve binary/assets and spawn the production `generals` child.
fn bootstrap_smoke_child(
    launch: ExecutableSmokeLaunch,
    result: &mut ExecutableSmokeResult,
) -> Option<SmokeBootstrap> {
    let Some(exe) = resolve_runtime_exe() else {
        result.status = "binary_missing".into();
        result.detail =
            "generals binary not found; build with `cargo build -p generals_main --bin generals --release` or set GENERALS_RUNTIME_EXE".into();
        return None;
    };
    // Best-effort: prior flaky runs can leave a hanging runtime_host `generals` holding
    // the GPU/display; that makes the next Booting exit before Menu.
    kill_stale_runtime_host_generals(&exe);
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0);
    let tmp = std::env::temp_dir().join(format!("generals_exec_smoke_{stamp}"));
    let _ = fs::create_dir_all(&tmp);
    let control_path = tmp.join("control.txt");
    let status_path = tmp.join("status.txt");
    let frame_path = tmp.join("frame.png");
    let _ = fs::write(&control_path, b"");
    let _ = fs::write(&status_path, b"");

    let map = resolve_lone_eagle_map();
    result.map_seen = map.clone();
    // Prefer -flag=value so option parsing cannot steal the next token
    // (matches GPUI bridge / verified boot path).
    // Wave 833: run from GeneralsRust workspace root so Data/INI + maps resolve.
    let workspace_cwd = {
        let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR")); // Code/Main
        manifest
            .parent() // Code
            .and_then(|p| p.parent()) // GeneralsRust
            .map(|p| p.to_path_buf())
            .filter(|p| p.join("target").is_dir() || p.join("Code").is_dir())
            .or_else(|| std::env::current_dir().ok())
            .unwrap_or_else(|| PathBuf::from("."))
    };
    let runtime_host_arg = match launch {
        ExecutableSmokeLaunch::HeadlessHost => "-runtime_host=headless",
        ExecutableSmokeLaunch::Windowed => "-runtime_host=windowed",
    };
    let mut child = match Command::new(&exe)
        .current_dir(&workspace_cwd)
        .arg(runtime_host_arg)
        .arg("-windowed")
        .arg("-width=640")
        .arg("-height=480")
        .arg(format!("-gpui_control={}", control_path.display()))
        .arg(format!("-gpui_status={}", status_path.display()))
        .arg(format!("-gpui_frame={}", frame_path.display()))
        .arg("-nologo")
        .arg("-nointro")
        // Default WND=1: retail ButtonStart residual is headless-safe after shell
        // re-borrow + map resolve + InGame world-draw fixes. Override with =0 for soft UI.
        .env(
            "GENERALS_RUNTIME_HOST_WND",
            std::env::var("GENERALS_RUNTIME_HOST_WND").unwrap_or_else(|_| "1".into()),
        )
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        // CRITICAL: do not pipe stderr without a drain thread — Roads.ini warn
        // spam fills the OS pipe and deadlocks the child in Booting.
        // File redirect keeps panic traces for smoke diagnosis without pipe deadlock.
        .stderr(
            std::fs::File::create(tmp.join("child_stderr.txt")).unwrap_or_else(|_| {
                // Fallback if tmp missing — still avoid pipe deadlock.
                std::fs::File::create(std::env::temp_dir().join("generals_smoke_child_stderr.txt"))
                    .expect("smoke stderr file")
            }),
        )
        .spawn()
    {
        Ok(c) => c,
        Err(e) => {
            result.status = "spawn_failed".into();
            result.detail = format!("failed to spawn {}: {e}", exe.display());
            return None;
        }
    };
    result.process_started = true;

    Some(SmokeBootstrap {
        child,
        tmp,
        control_path,
        status_path,
        map,
    })

}
