// Lifecycle: owned child cleanup, binary/map resolution, and launch modes.

/// Own only the process spawned by this smoke run. Cleanup on unwinding or an
/// early return must never target another worktree's runtime host by name.
struct SmokeChild(Child);

impl std::ops::Deref for SmokeChild {
    type Target = Child;
    fn deref(&self) -> &Child {
        &self.0
    }
}

impl std::ops::DerefMut for SmokeChild {
    fn deref_mut(&mut self) -> &mut Child {
        &mut self.0
    }
}

impl Drop for SmokeChild {
    fn drop(&mut self) {
        kill_child(&mut self.0);
    }
}

fn configure_smoke_child_assets(command: &mut Command, options: &SmokeOptions) {
    if let Some(assets_dir) = &options.assets_dir {
        command.env("GENERALS_ASSETS_DIR", assets_dir);
    } else {
        command.env_remove("GENERALS_ASSETS_DIR");
    }
}

pub(crate) fn resolve_runtime_exe() -> Option<PathBuf> {
    if let Ok(p) = std::env::var("GENERALS_RUNTIME_EXE") {
        let pb = PathBuf::from(p);
        if pb.is_file() {
            return Some(pb.canonicalize().unwrap_or(pb));
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
    // Gates run with cwd = Code/Main, so cwd-relative candidates alone never
    // resolve there: also anchor at the workspace target dir (CARGO_TARGET_DIR
    // or `<workspace>/target`) and at the running test binary's profile dir
    // (`target/<profile>/deps/<test>` → `target/<profile>/generals`).
    let mut target_dirs: Vec<PathBuf> = vec![
        PathBuf::from("target"),
        PathBuf::from("GeneralsRust/target"),
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target"),
    ];
    if let Some(dir) = std::env::var_os("CARGO_TARGET_DIR") {
        target_dirs.push(PathBuf::from(dir));
    }
    let mut candidates: Vec<PathBuf> = ["release", "debug"]
        .iter()
        .flat_map(|profile| target_dirs.iter().map(move |t| t.join(profile).join("generals")))
        .collect();
    if let Ok(cur) = std::env::current_exe() {
        candidates.extend(cur.ancestors().skip(1).take(2).map(|dir| dir.join("generals")));
    }
    if let Some(path) = resolve_runtime_exe_from_candidates(&candidates, !prefer_release_first) {
        // The child runs in a temporary working directory. Keep the chosen
        // binary valid after Command::current_dir changes that directory.
        return Some(path.canonicalize().unwrap_or(path));
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

fn resolve_lone_eagle_map(assets_dir: Option<&Path>) -> String {
    if let Some(path) = assets_dir.and_then(retail_map::lone_eagle_from_assets_root)
    {
        return path.to_string_lossy().into_owned();
    }
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
pub fn lone_eagle_map_on_disk(options: &SmokeOptions) -> bool {
    let resolved = resolve_lone_eagle_map(options.assets_dir.as_deref());
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
    /// Stop once the live shell reaches MainMenu.wnd, then exit cleanly.
    /// Used by the P0 boot-to-shell-menu gate (hq-x1j4u.9.1).
    MenuOnly,
    /// Menu → skirmish start → first InGame with a non-shell retail map, then exit.
    /// Used by the P0 load-retail-map gate (hq-x1j4u.9.2).
    MapLoadOnly,
    /// MapLoadOnly path, then select → move → attack on the live host, then exit.
    /// Used by the P0 issue-orders gate (hq-x1j4u.9.3).
    OrdersOnly,
    /// After retail InGame: player select/move, then require AI activity residual.
    /// Used by the P0 AI-opponent gate (hq-x1j4u.9.5).
    AiActivity,
    /// After retail InGame: orders, N ticks, save, load into fresh candidate,
    /// assert world restore, then post-load order. P0 hq-x1j4u.9.7.
    SaveLoadOnly,
    /// Windowed Lone Eagle sit-through ≥60s with a selected unit and stable
    /// non-zero render items; captures a screenshot. P0 hq-x1j4u.9.6.
    PresentFrameOnly,
}
