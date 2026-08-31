// Lifecycle: process helpers — stale-host cleanup, binary/map resolution,
// child kill, and the launch/driver enums.

fn kill_stale_runtime_host_generals(exe: &Path) {
    // Fail-soft: prior smoke / cargo runs can leave a hanging `generals` holding
    // GPU/display and cause Booting→exit before Menu (or Tokio shutdown races).
    #[cfg(unix)]
    {
        let exe_s = exe.to_string_lossy().to_string();
        // CLI flag is `-runtime_host=headless` (underscore). Also match basename
        // when the absolute path differs between debug/release invocations.
        // Wave 833: never pkill bare exe path — that races the just-spawned child
        // when paths collide across debug/release. Match runtime_host flag only.
        let patterns = [
            format!("{exe_s}.*runtime_host"),
            "target/.*/generals.*runtime_host=headless".to_string(),
            "generals.*-runtime_host=headless".to_string(),
        ];
        for pat in patterns {
            let _ = std::process::Command::new("pkill")
                .args(["-9", "-f", &pat])
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .status();
        }
        // Allow GPU/window teardown before the next spawn.
        std::thread::sleep(Duration::from_millis(1200));
    }
    let _ = exe;
}

pub(crate) fn resolve_runtime_exe() -> Option<PathBuf> {
    if let Ok(p) = std::env::var("GENERALS_RUNTIME_EXE") {
        let pb = PathBuf::from(p);
        if pb.is_file() {
            return Some(pb);
        }
    }
    // Wave 833: current-source binary. Newest-mtime among debug+release so a
    // freshly built debug tree is not skipped for a stale release. Optional
    // GENERALS_RUNTIME_EXE_PREFER_RELEASE=1 restores release-first. Override
    // with GENERALS_RUNTIME_EXE. GENERALS_RUNTIME_EXE_PREFER_DEBUG=1 still
    // means newest-mtime (same as default).
    let prefer_release_first =
        std::env::var_os("GENERALS_RUNTIME_EXE_PREFER_RELEASE").is_some_and(|v| {
            let s = v.to_string_lossy();
            !(s.is_empty()
                || s == "0"
                || s.eq_ignore_ascii_case("false")
                || s.eq_ignore_ascii_case("no"))
        });
    let candidates = [
        PathBuf::from("target/release/generals"),
        PathBuf::from("GeneralsRust/target/release/generals"),
        PathBuf::from("./target/release/generals"),
        PathBuf::from("target/debug/generals"),
        PathBuf::from("GeneralsRust/target/debug/generals"),
        PathBuf::from("./target/debug/generals"),
    ];
    if let Some(path) = resolve_runtime_exe_from_candidates(&candidates, !prefer_release_first) {
        return Some(path);
    }
    // Try next to current exe
    if let Ok(cur) = std::env::current_exe() {
        if let Some(dir) = cur.parent() {
            let sibling = dir.join("generals");
            if sibling.is_file() {
                return Some(sibling);
            }
        }
    }
    None
}

pub(crate) fn resolve_runtime_exe_from_candidates(
    candidates: &[PathBuf],
    newest_mtime: bool,
) -> Option<PathBuf> {
    if newest_mtime {
        let mut best: Option<(std::time::SystemTime, PathBuf)> = None;
        for c in candidates {
            if !c.is_file() {
                continue;
            }
            let modified = c
                .metadata()
                .and_then(|m| m.modified())
                .unwrap_or(std::time::SystemTime::UNIX_EPOCH);
            match &best {
                Some((t, _)) if modified <= *t => {}
                _ => best = Some((modified, c.clone())),
            }
        }
        if let Some((_, path)) = best {
            return Some(path);
        }
    } else {
        for c in candidates {
            if c.is_file() {
                return Some(c.clone());
            }
        }
    }
    None
}

fn resolve_lone_eagle_map() -> String {
    let mut candidates: Vec<PathBuf> = LONE_EAGLE_CANDIDATES.iter().map(PathBuf::from).collect();
    // Walk from CARGO_MANIFEST_DIR (Code/Main) up to repo root and common extract dirs.
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    for base in [
        manifest.clone(),
        manifest.join(".."),
        manifest.join("../.."),
        manifest.join("../../.."),
        manifest.join("../../../windows_game/extracted_big_files/MapsZH/Maps/Lone Eagle"),
        manifest.join("../../../windows_game/extracted_big_files_v2/MapsZH/Maps/Lone Eagle"),
    ] {
        candidates.push(base.join("Lone Eagle.map"));
        candidates.push(base.join("Maps/Lone Eagle/Lone Eagle.map"));
        candidates.push(
            base.join("windows_game/extracted_big_files/MapsZH/Maps/Lone Eagle/Lone Eagle.map"),
        );
        candidates.push(
            base.join("windows_game/extracted_big_files_v2/MapsZH/Maps/Lone Eagle/Lone Eagle.map"),
        );
    }
    if let Ok(cwd) = std::env::current_dir() {
        for c in LONE_EAGLE_CANDIDATES {
            candidates.push(cwd.join(c));
            candidates.push(cwd.join("..").join(c));
        }
    }
    for c in candidates {
        if c.is_file() {
            // Prefer absolute canonical path so the child process cwd does not matter.
            return c.canonicalize().unwrap_or(c).to_string_lossy().into_owned();
        }
    }
    "Lone Eagle".into()
}

/// True when a Lone Eagle `.map` file exists on one of the smoke search paths.
///
/// The bare `"Lone Eagle"` fallback name (no file on disk) is **not** a hit.
pub fn lone_eagle_map_on_disk() -> bool {
    let resolved = resolve_lone_eagle_map();
    Path::new(&resolved).is_file()
}

fn kill_child(child: &mut Child) {
    let _ = child.kill();
    let _ = child.wait();
}

/// How the production `generals` child is launched.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ExecutableSmokeLaunch {
    /// Existing headless host gate. Uses `-runtime_host=headless`.
    /// `playable_claim` stays false.
    HeadlessHost,
    /// Visible OS window + WGPU. Uses `-runtime_host=windowed` so GPUI
    /// status/control still publish, but `init_headless` is not used.
    Windowed,
}

/// Whether the smoke loop may write gameplay/menu commands into the runtime
/// host's control file.
///
/// The regular headless smoke is deliberately automated. The windowed
/// acceptance gate is deliberately an observer: it must not manufacture the
/// menu, order, production, gather, or save/load evidence that it reports.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum SmokeDriver {
    Automated,
    ManualObserver,
}
