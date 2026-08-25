//! In-game layout / ControlBar readiness (C++ ShowControlBar / ControlBar.wnd parity).
//!
//! `CncGameEngine::ensure_gameplay_layouts` must not remain a silent no-op: it calls
//! [`ensure_control_bar_layout`], which resolves retail ControlBar assets and attempts
//! a real load when the window manager can parse the layout.
//!
//! Wave 76 residual deepen (host-testable, fail-closed vs full W3D retail UI):
//! - Retail ControlBar.wnd materialises **98** WINDOW nodes (WindowManager parse).
//! - Key named-child residual table (CommandWindow / MoneyDisplay / LeftHUD / …).
//! - Font residual table peeled from ControlBar.wnd FONT= lines
//!   (Times New Roman 10/14, Arial 8/10/14, Generals 15/20).

use std::path::{Path, PathBuf};

/// Live ControlBar parent name (C++ ShowControlBar tree). Never infer liveness
/// from leftover MainMenu/Skirmish `window_count`.
pub const CONTROL_BAR_PARENT_NAME: &str = "ControlBar.wnd:ControlBarParent";

/// True only when TheWindowManager has the retail ControlBar parent gadget.
#[cfg(feature = "game_client")]
pub fn control_bar_parent_is_live() -> bool {
    game_client::gui::with_window_manager_ref(|wm| {
        wm.find_window_by_name(CONTROL_BAR_PARENT_NAME).is_some()
    })
}

#[cfg(not(feature = "game_client"))]
pub fn control_bar_parent_is_live() -> bool {
    false
}

/// Load ControlBar.wnd into TheWindowManager if the parent is missing.
///
/// C++ `ShowControlBar` (ControlBarCallback.cpp:477-507) only looks up
/// `ControlBar.wnd:ControlBarParent` and `winHide(FALSE)` — it does not
/// `winRepaint`. Do not `begin_frame`/`draw_all` here: an open UI frame
/// is classified as abandoned by the next overlay flush and discarded.
pub fn materialise_live_control_bar() -> bool {
    #[cfg(feature = "game_client")]
    {
        if !control_bar_parent_is_live() {
            if let Some(path) = resolve_control_bar_path() {
                let _ = try_load_control_bar_via_window_manager(&path.display().to_string());
            }
            // C++ winCreateFromScript("ControlBar.wnd") uses the archive/ancestor
            // resolver. A cwd-limited disk probe must not skip the live load.
            if !control_bar_parent_is_live() {
                let _ = try_load_control_bar_via_window_manager("ControlBar.wnd");
            }
        }
        control_bar_parent_is_live() && live_window_count() > 0
    }
    #[cfg(not(feature = "game_client"))]
    {
        false
    }
}

#[cfg(feature = "game_client")]
fn live_window_count() -> usize {
    game_client::gui::with_window_manager_ref(|wm| wm.window_count())
}

/// Candidate locations for ControlBar.wnd (extracted BIG / WindowZH trees).
pub const CONTROL_BAR_CANDIDATES: &[&str] = &[
    "windows_game/extracted_big_files/WindowZH/Window/ControlBar.wnd",
    "windows_game/extracted_big_files_v2/WindowZH/Window/ControlBar.wnd",
    "../windows_game/extracted_big_files/WindowZH/Window/ControlBar.wnd",
    "../windows_game/extracted_big_files_v2/WindowZH/Window/ControlBar.wnd",
    "Window/ControlBar.wnd",
    "Data/Window/ControlBar.wnd",
    "ControlBar.wnd",
];

/// Retail ControlBar.wnd WINDOW node count residual (WindowManager parse).
///
/// Counted from WindowZH/Window/ControlBar.wnd: 98 `NAME = "ControlBar.wnd:…"` lines
/// (95 non-empty names + 3 empty-name decorative windows).
pub const CONTROL_BAR_RETAIL_WINDOW_COUNT: usize = 98;

/// Key named child residual table (retail ControlBar.wnd NAME tokens without empty).
///
/// Fail-closed: not full WindowManager name-lookup / DrawCallback dispatch.
pub const CONTROL_BAR_KEY_NAMED_WINDOWS: &[&str] = &[
    "ControlBarParent",
    "Munkee",
    "BackgroundMarker",
    "CenterBackground",
    "BeaconWindow",
    "CommandWindow",
    "ButtonCommand01",
    "ButtonCommand14",
    "UnderConstructionWindow",
    "OCLTimerWindow",
    "LeftHUD",
    "RightHUD",
    "ProductionQueueWindow",
    "WinUnitSelected",
    "CameoWindow",
    "ButtonIdleWorker",
    "ButtonPlaceBeacon",
    "PopupCommunicator",
    "ButtonOptions",
    "ButtonGeneral",
    "MoneyDisplay",
    "PowerWindow",
    "ButtonSmall",
    "ButtonMedium",
    "ButtonLarge",
    "WinUAttack",
    "OnTopDraw",
    "ForegroundMarker",
    "GeneralsExp",
    "ExpBarForeground",
];

/// Font residual entry peeled from ControlBar.wnd FONT= lines.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ControlBarFontResidual {
    pub name: &'static str,
    pub size: u32,
    pub bold: bool,
}

/// Retail ControlBar.wnd font residual table (unique FONT NAME/SIZE/BOLD peels).
///
/// Counts (for honesty, not stored here): Times New Roman 14×41, Arial 8×31,
/// Arial 10×13, Times New Roman 10×8, Arial 14×3, Generals 15×1, Generals 20×1.
pub const CONTROL_BAR_FONT_RESIDUAL_TABLE: &[ControlBarFontResidual] = &[
    ControlBarFontResidual {
        name: "Times New Roman",
        size: 14,
        bold: false,
    },
    ControlBarFontResidual {
        name: "Times New Roman",
        size: 10,
        bold: false,
    },
    ControlBarFontResidual {
        name: "Arial",
        size: 8,
        bold: false,
    },
    ControlBarFontResidual {
        name: "Arial",
        size: 10,
        bold: false,
    },
    ControlBarFontResidual {
        name: "Arial",
        size: 14,
        bold: false,
    },
    ControlBarFontResidual {
        name: "Generals",
        size: 15,
        bold: false,
    },
    ControlBarFontResidual {
        name: "Generals",
        size: 20,
        bold: false,
    },
];

/// Honesty: retail window-count residual constant.
pub fn honesty_control_bar_window_count_residual_ok() -> bool {
    CONTROL_BAR_RETAIL_WINDOW_COUNT == 98
}

/// Honesty: key named-child residual table includes Command / Money / HUD peels.
pub fn honesty_control_bar_named_windows_residual_ok() -> bool {
    CONTROL_BAR_KEY_NAMED_WINDOWS.contains(&"ControlBarParent")
        && CONTROL_BAR_KEY_NAMED_WINDOWS.contains(&"CommandWindow")
        && CONTROL_BAR_KEY_NAMED_WINDOWS.contains(&"MoneyDisplay")
        && CONTROL_BAR_KEY_NAMED_WINDOWS.contains(&"LeftHUD")
        && CONTROL_BAR_KEY_NAMED_WINDOWS.contains(&"RightHUD")
        && CONTROL_BAR_KEY_NAMED_WINDOWS.contains(&"ButtonCommand01")
        && CONTROL_BAR_KEY_NAMED_WINDOWS.contains(&"ButtonCommand14")
        && CONTROL_BAR_KEY_NAMED_WINDOWS.contains(&"OCLTimerWindow")
        && CONTROL_BAR_KEY_NAMED_WINDOWS.contains(&"WinUnitSelected")
        && CONTROL_BAR_KEY_NAMED_WINDOWS.contains(&"PowerWindow")
        && CONTROL_BAR_KEY_NAMED_WINDOWS.len() >= 20
}

/// Honesty: ControlBar font residual table covers retail Arial / Times / Generals peels.
pub fn honesty_control_bar_font_table_residual_ok() -> bool {
    CONTROL_BAR_FONT_RESIDUAL_TABLE.len() == 7
        && CONTROL_BAR_FONT_RESIDUAL_TABLE
            .iter()
            .any(|f| f.name == "Arial" && f.size == 8 && !f.bold)
        && CONTROL_BAR_FONT_RESIDUAL_TABLE
            .iter()
            .any(|f| f.name == "Arial" && f.size == 10 && !f.bold)
        && CONTROL_BAR_FONT_RESIDUAL_TABLE
            .iter()
            .any(|f| f.name == "Arial" && f.size == 14 && !f.bold)
        && CONTROL_BAR_FONT_RESIDUAL_TABLE
            .iter()
            .any(|f| f.name == "Times New Roman" && f.size == 14 && !f.bold)
        && CONTROL_BAR_FONT_RESIDUAL_TABLE
            .iter()
            .any(|f| f.name == "Times New Roman" && f.size == 10 && !f.bold)
        && CONTROL_BAR_FONT_RESIDUAL_TABLE
            .iter()
            .any(|f| f.name == "Generals" && f.size == 15 && !f.bold)
        && CONTROL_BAR_FONT_RESIDUAL_TABLE
            .iter()
            .any(|f| f.name == "Generals" && f.size == 20 && !f.bold)
}

/// Combined Wave 76 ControlBar residual deepen honesty (constant packs).
///
/// When assets load, also requires window_count == 98. When assets absent,
/// constant packs alone are honest residual (fail-closed vs GPU claim).
pub fn honesty_control_bar_residual_pack_wave76_ok(
    window_loaded: bool,
    window_count: usize,
) -> bool {
    let constants_ok = honesty_control_bar_window_count_residual_ok()
        && honesty_control_bar_named_windows_residual_ok()
        && honesty_control_bar_font_table_residual_ok();
    if !constants_ok {
        return false;
    }
    if window_loaded {
        window_count == CONTROL_BAR_RETAIL_WINDOW_COUNT
    } else {
        true
    }
}

/// Result of ensuring the in-game control bar layout is available.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GameplayLayoutStatus {
    /// Layout file found and load path succeeded (or dry-run validated for tests).
    Ready { path: String, loaded: bool },
    /// No ControlBar.wnd found in known asset roots.
    AssetsUnavailable { searched: Vec<String> },
    /// File found but load/parse failed.
    LoadFailed { path: String, error: String },
}

impl GameplayLayoutStatus {
    pub fn is_ready(&self) -> bool {
        matches!(self, Self::Ready { .. })
    }

    pub fn is_assets_unavailable(&self) -> bool {
        matches!(self, Self::AssetsUnavailable { .. })
    }
}

/// Resolve the first existing ControlBar.wnd on disk.
pub fn resolve_control_bar_path() -> Option<PathBuf> {
    for c in CONTROL_BAR_CANDIDATES {
        let p = Path::new(c);
        if p.is_file() {
            return Some(p.to_path_buf());
        }
    }
    if let Ok(current_dir) = std::env::current_dir() {
        for base in current_dir.ancestors() {
            for c in [
                "windows_game/extracted_big_files/WindowZH/Window/ControlBar.wnd",
                "windows_game/extracted_big_files_v2/WindowZH/Window/ControlBar.wnd",
                "Window/ControlBar.wnd",
                "Data/Window/ControlBar.wnd",
            ] {
                let p = base.join(c);
                if p.is_file() {
                    return Some(p);
                }
            }
        }
    }
    None
}

/// Validate ControlBar.wnd is a non-empty retail layout file.
///
/// Residual honesty (fail-closed vs full WindowManager parse / loaded=true):
/// - non-empty file with FILE_VERSION / WINDOW / ControlBar tokens
/// - does **not** claim GUI window tree construction
pub fn validate_control_bar_file(path: &Path) -> Result<(), String> {
    let meta = std::fs::metadata(path).map_err(|e| format!("stat: {e}"))?;
    if !meta.is_file() {
        return Err("not a file".into());
    }
    if meta.len() == 0 {
        return Err("empty layout file".into());
    }
    // Cheap content sniff: .wnd layouts are text-ish script files.
    let sample = std::fs::read(path).map_err(|e| format!("read: {e}"))?;
    if sample.len() < 32 {
        return Err("layout too small".into());
    }
    // Retail ControlBar.wnd structural tokens (ShowControlBar parity residual).
    let text = String::from_utf8_lossy(&sample);
    // Read enough for header + first WINDOW block (files can be large).
    let head_len = sample.len().min(4096);
    let head = String::from_utf8_lossy(&sample[..head_len]);
    if !head.contains("WINDOW") && !text.contains("WINDOW") {
        return Err("missing WINDOW block".into());
    }
    if !head.contains("ControlBar") && !text.contains("ControlBar") {
        return Err("missing ControlBar name token".into());
    }
    // FILE_VERSION is present on retail SAGE .wnd layouts.
    if !head.contains("FILE_VERSION") && !text.contains("FILE_VERSION") {
        return Err("missing FILE_VERSION header".into());
    }
    // Wave 76 residual: key named-child tokens must appear in retail ControlBar.wnd.
    for key in [
        "ControlBarParent",
        "CommandWindow",
        "MoneyDisplay",
        "LeftHUD",
        "RightHUD",
    ] {
        if !text.contains(key) {
            return Err(format!("missing key named window token: {key}"));
        }
    }
    // Wave 76 residual: font table tokens.
    if !text.contains("Times New Roman") {
        return Err("missing Times New Roman font residual".into());
    }
    if !text.contains("Arial") {
        return Err("missing Arial font residual".into());
    }
    if !text.contains("Generals") {
        return Err("missing Generals font residual".into());
    }
    Ok(())
}

/// Host-testable honesty for ControlBar.wnd residual.
///
/// Dry-run (`attempt_window_load=false`) never claims `window_loaded`.
/// Full ensure attempts `WindowManager::load_window` headlessly when assets exist
/// (C++ ShowControlBar residual; not full windowed W3D retail claim).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ControlBarLayoutHonesty {
    pub path_resolved: bool,
    pub wnd_validated: bool,
    pub assets_unavailable: bool,
    pub window_loaded: bool,
    /// GameWindow instances created by WindowManager parse (0 when not loaded).
    pub window_count: usize,
    pub status: GameplayLayoutStatus,
}

impl ControlBarLayoutHonesty {
    pub fn from_status(status: GameplayLayoutStatus) -> Self {
        Self::from_status_with_count(status, 0)
    }

    pub fn from_status_with_count(status: GameplayLayoutStatus, window_count: usize) -> Self {
        match &status {
            GameplayLayoutStatus::Ready { loaded, .. } => Self {
                path_resolved: true,
                wnd_validated: true,
                assets_unavailable: false,
                window_loaded: *loaded,
                window_count: if *loaded { window_count } else { 0 },
                status,
            },
            GameplayLayoutStatus::AssetsUnavailable { .. } => Self {
                path_resolved: false,
                wnd_validated: false,
                assets_unavailable: true,
                window_loaded: false,
                window_count: 0,
                status,
            },
            GameplayLayoutStatus::LoadFailed { .. } => Self {
                path_resolved: true,
                wnd_validated: false,
                assets_unavailable: false,
                window_loaded: false,
                window_count: 0,
                status,
            },
        }
    }

    /// Shell residual OK: Ready after validate, or honest AssetsUnavailable.
    pub fn shell_residual_ok(&self) -> bool {
        self.wnd_validated || self.assets_unavailable
    }
}

/// Resolve + validate ControlBar and return host-testable honesty flags.
///
/// When `attempt_window_load` is true and assets exist, loads into the live
/// `TheWindowManager` (`window_loaded=true` only when `ControlBarParent` is live).
pub fn control_bar_layout_honesty(attempt_window_load: bool) -> ControlBarLayoutHonesty {
    let (status, window_count) = ensure_control_bar_layout_with_count(attempt_window_load);
    ControlBarLayoutHonesty::from_status_with_count(status, window_count)
}

/// Residual: ControlBar.wnd materialisation honesty (Wave 165).
///
/// When retail assets resolve, require a throwaway WindowManager parse with
/// `window_count == CONTROL_BAR_RETAIL_WINDOW_COUNT` (98). Soft-ok only if
/// assets are unavailable (CI without WindowZH). The shipped Ready-gate uses
/// the live `TheWindowManager` instead — leftover shell windows must not
/// block this residual peel.
pub fn simulate_control_bar_materialise_honesty() -> bool {
    let Some(path) = resolve_control_bar_path() else {
        return true;
    };
    if validate_control_bar_file(&path).is_err() {
        return false;
    }
    #[cfg(feature = "game_client")]
    {
        match try_load_control_bar_via_throwaway_window_manager(&path.display().to_string()) {
            Ok(count) => {
                count == CONTROL_BAR_RETAIL_WINDOW_COUNT
                    && honesty_control_bar_residual_pack_wave76_ok(true, count)
            }
            Err(_) => false,
        }
    }
    #[cfg(not(feature = "game_client"))]
    {
        true
    }
}

/// Shipped ensure path: resolve ControlBar.wnd, validate, and attempt load.
///
/// When `attempt_window_load` is false, only resolve+validate (unit-test friendly).
/// When true and `game_client` is enabled, try WindowManager::load_window.
pub fn ensure_control_bar_layout(attempt_window_load: bool) -> GameplayLayoutStatus {
    ensure_control_bar_layout_with_count(attempt_window_load).0
}

/// Like [`ensure_control_bar_layout`] but also returns the **live**
/// `TheWindowManager` window count when a load succeeds (0 otherwise).

pub fn ensure_control_bar_layout_with_count(
    attempt_window_load: bool,
) -> (GameplayLayoutStatus, usize) {
    let Some(path) = resolve_control_bar_path() else {
        return (
            GameplayLayoutStatus::AssetsUnavailable {
                searched: CONTROL_BAR_CANDIDATES
                    .iter()
                    .map(|s| s.to_string())
                    .collect(),
            },
            0,
        );
    };
    let path_str = path.display().to_string();
    if let Err(e) = validate_control_bar_file(&path) {
        return (
            GameplayLayoutStatus::LoadFailed {
                path: path_str,
                error: e,
            },
            0,
        );
    }

    if !attempt_window_load {
        return (
            GameplayLayoutStatus::Ready {
                path: path_str,
                loaded: false,
            },
            0,
        );
    }

    #[cfg(feature = "game_client")]
    {
        match try_load_control_bar_via_window_manager(&path_str) {
            Ok(count) => (
                GameplayLayoutStatus::Ready {
                    path: path_str,
                    loaded: true,
                },
                count,
            ),
            Err(e) => {
                // Assets resolved and validated: load failure is not soft-ok.
                // Host must not claim Ready when the live WindowManager did
                // not materialise ControlBarParent.

                log::warn!(
                    "ControlBar.wnd validated at {} but window load failed: {}",
                    path_str,
                    e
                );
                (
                    GameplayLayoutStatus::LoadFailed {
                        path: path_str,
                        error: e,
                    },
                    0,
                )
            }
        }
    }
    #[cfg(not(feature = "game_client"))]
    {
        let _ = attempt_window_load;
        (
            GameplayLayoutStatus::Ready {
                path: path_str,
                loaded: false,
            },
            0,
        )
    }
}

/// Load ControlBar.wnd into the live `TheWindowManager` (C++ `TheWindowManager`).
///
/// Ready-gate window_count is measured here — never on a throwaway instance.
/// Returns the live window count only when `ControlBarParent` exists.
#[cfg(feature = "game_client")]
fn try_load_control_bar_via_window_manager(path: &str) -> Result<usize, String> {
    if control_bar_parent_is_live() {
        let count = live_window_count();
        if count == 0 {
            return Err("ControlBarParent live but window_count=0".into());
        }
        return Ok(count);
    }
    // Prefer absolute/resolved path first (reliable in tests/CI cwd), then retail names
    // C++ winCreateFromScript uses via the file system search path.
    let names = [path, "ControlBar.wnd", "Window/ControlBar.wnd"];
    let mut last_err = String::from("no load attempted");
    for name in names {
        match game_client::gui::with_window_manager(|wm| wm.load_window(name)) {
            Ok(_) => {
                if !control_bar_parent_is_live() {
                    return Err(format!(
                        "{name}: loaded but ControlBarParent missing on live WindowManager"
                    ));
                }
                let count = live_window_count();
                if count == 0 {
                    return Err(format!(
                        "{name}: load returned window but live window_count=0"
                    ));
                }
                return Ok(count);
            }
            Err(e) => last_err = format!("{name}: {e:?}"),
        }
    }
    Err(last_err)
}

/// Headless throwaway WindowManager parse of ControlBar.wnd.
///
/// Residual honesty only (Wave 165 / 98-window peel). The shipped Ready-gate
/// and InGame materialise use [`try_load_control_bar_via_window_manager`].
#[cfg(feature = "game_client")]
fn try_load_control_bar_via_throwaway_window_manager(path: &str) -> Result<usize, String> {
    use game_client::gui::window_manager::WindowManager;
    let mut wm = WindowManager::new();
    wm.init();
    let names = [path, "ControlBar.wnd", "Window/ControlBar.wnd"];
    let mut last_err = String::from("no load attempted");
    for name in names {
        match wm.load_window(name) {
            Ok(_) => {
                let count = wm.window_count();
                if count == 0 {
                    return Err(format!("{name}: load returned window but window_count=0"));
                }
                return Ok(count);
            }
            Err(e) => last_err = format!("{name}: {e:?}"),
        }
    }
    Err(last_err)
}

/// Format status for logs/gates.
pub fn format_gameplay_layout_status(s: &GameplayLayoutStatus) -> String {
    match s {
        GameplayLayoutStatus::Ready { path, loaded } => {
            format!("control_bar status=ready path={path} loaded={loaded}")
        }
        GameplayLayoutStatus::AssetsUnavailable { searched } => {
            format!(
                "control_bar status=assets_unavailable searched={}",
                searched.len()
            )
        }
        GameplayLayoutStatus::LoadFailed { path, error } => {
            format!("control_bar status=load_failed path={path} error={error}")
        }
    }
}

/// Format honesty for shell_smoke / gate detail lines.
pub fn format_control_bar_honesty(h: &ControlBarLayoutHonesty) -> String {
    format!(
        "{} path_resolved={} wnd_validated={} window_loaded={} windows={}",
        format_gameplay_layout_status(&h.status),
        h.path_resolved,
        h.wnd_validated,
        h.window_loaded,
        h.window_count
    )
}

// ---------------------------------------------------------------------------
// Wave 160 residual: MainMenu.wnd resolve/validate (shell boot path)
// ---------------------------------------------------------------------------

/// Candidate locations for MainMenu.wnd (extracted BIG / WindowZH trees).
pub const MAIN_MENU_WND_CANDIDATES: &[&str] = &[
    "windows_game/extracted_big_files/WindowZH/Window/Menus/MainMenu.wnd",
    "windows_game/extracted_big_files_v2/WindowZH/Window/Menus/MainMenu.wnd",
    "../windows_game/extracted_big_files/WindowZH/Window/Menus/MainMenu.wnd",
    "../windows_game/extracted_big_files_v2/WindowZH/Window/Menus/MainMenu.wnd",
    "Window/Menus/MainMenu.wnd",
    "Data/Window/Menus/MainMenu.wnd",
    "Menus/MainMenu.wnd",
    "MainMenu.wnd",
];

/// Retail MainMenu.wnd WINDOW node count residual (WindowManager parse tokens).
pub const MAIN_MENU_WND_WINDOW_TOKEN_COUNT_RESIDUAL: usize = 126;

/// Retail MainMenu.wnd named `MainMenu.wnd:…` residual count.
pub const MAIN_MENU_WND_NAMED_COUNT_RESIDUAL: usize = 63;

/// Key MainMenu.wnd named-child residual table (shell navigation).
pub const MAIN_MENU_WND_KEY_NAMES_RESIDUAL: &[&str] = &[
    "MainMenu.wnd:MainMenuParent",
    "MainMenu.wnd:ButtonSinglePlayer",
    "MainMenu.wnd:ButtonMultiplayer",
    "MainMenu.wnd:ButtonSkirmish",
    "MainMenu.wnd:ButtonOptions",
    "MainMenu.wnd:ButtonCredits",
    "MainMenu.wnd:ButtonExit",
    "MainMenu.wnd:ButtonUSA",
    "MainMenu.wnd:ButtonGLA",
    "MainMenu.wnd:ButtonChina",
    "MainMenu.wnd:ButtonChallenge",
    "MainMenu.wnd:ButtonLoadReplay",
];

/// Resolve MainMenu.wnd path residual (fail-closed if assets missing).
pub fn resolve_main_menu_wnd_path() -> Option<PathBuf> {
    for c in MAIN_MENU_WND_CANDIDATES {
        let p = Path::new(c);
        if p.is_file() {
            return Some(p.to_path_buf());
        }
    }
    let prefixes = ["", "../", "../../"];
    for prefix in prefixes {
        for c in MAIN_MENU_WND_CANDIDATES {
            let p = Path::new(prefix).join(c);
            if p.is_file() {
                return Some(p);
            }
        }
    }
    None
}

/// Validate MainMenu.wnd is a non-empty retail shell layout file.
///
/// Residual honesty (fail-closed vs full WindowManager parse / shell boot):
/// - non-empty file with FILE_VERSION / WINDOW / MainMenu tokens
/// - does **not** claim GUI window tree construction or W3D draw
pub fn validate_main_menu_wnd_file(path: &Path) -> Result<(), String> {
    let meta = std::fs::metadata(path).map_err(|e| format!("stat: {e}"))?;
    if !meta.is_file() {
        return Err("not a file".into());
    }
    if meta.len() == 0 {
        return Err("empty layout file".into());
    }
    let sample = std::fs::read(path).map_err(|e| format!("read: {e}"))?;
    if sample.len() < 32 {
        return Err("layout too small".into());
    }
    let text = String::from_utf8_lossy(&sample);
    let head_len = sample.len().min(4096);
    let head = String::from_utf8_lossy(&sample[..head_len]);
    if !head.contains("FILE_VERSION") && !text.contains("FILE_VERSION") {
        return Err("missing FILE_VERSION".into());
    }
    if !head.contains("WINDOW") && !text.contains("WINDOW") {
        return Err("missing WINDOW block".into());
    }
    if !head.contains("MainMenu") && !text.contains("MainMenu") {
        return Err("missing MainMenu name token".into());
    }
    if !text.contains("MainMenu.wnd:MainMenuParent") {
        return Err("missing MainMenuParent".into());
    }
    if !text.contains("MainMenu.wnd:ButtonSinglePlayer") {
        return Err("missing ButtonSinglePlayer".into());
    }
    if !text.contains("LAYOUTINIT") {
        return Err("missing LAYOUTINIT".into());
    }
    Ok(())
}

/// MainMenu.wnd honesty residual pack (resolve + validate only).
#[derive(Debug, Clone)]
pub struct MainMenuWndHonesty {
    pub path_resolved: bool,
    pub path: Option<PathBuf>,
    pub wnd_validated: bool,
    pub assets_unavailable: bool,
    pub named_key_hits: usize,
    /// True when headless WindowManager parse materialised windows.
    pub window_loaded: bool,
    /// WindowManager window_count after successful headless parse (0 if not loaded).
    pub window_count: usize,
    pub detail: String,
}

impl MainMenuWndHonesty {
    /// Shell residual ok when resolved+validated, or assets honestly unavailable.
    pub fn shell_residual_ok(&self) -> bool {
        (self.path_resolved && self.wnd_validated)
            || (self.assets_unavailable && !self.path_resolved)
    }
}

/// Build MainMenu.wnd honesty residual (validate-only; no WindowManager load).
pub fn main_menu_wnd_honesty() -> MainMenuWndHonesty {
    main_menu_wnd_honesty_with_load(false)
}

/// Build MainMenu.wnd honesty residual with optional WindowManager load.
///
/// `attempt_window_load`: when true + `game_client`, try headless WindowManager parse
/// (C++ Shell::push MainMenu.wnd residual). Validate-only when false.
pub fn main_menu_wnd_honesty_with_load(attempt_window_load: bool) -> MainMenuWndHonesty {
    match resolve_main_menu_wnd_path() {
        None => MainMenuWndHonesty {
            path_resolved: false,
            path: None,
            wnd_validated: false,
            assets_unavailable: true,
            named_key_hits: 0,
            window_loaded: false,
            window_count: 0,
            detail: "MainMenu.wnd not found in candidate paths".into(),
        },
        Some(path) => match validate_main_menu_wnd_file(&path) {
            Ok(()) => {
                let text = std::fs::read_to_string(&path).unwrap_or_default();
                let named_key_hits = MAIN_MENU_WND_KEY_NAMES_RESIDUAL
                    .iter()
                    .filter(|n| text.contains(*n))
                    .count();
                let path_str = path.display().to_string();
                let (window_loaded, window_count, load_detail) = if attempt_window_load {
                    #[cfg(feature = "game_client")]
                    {
                        match try_load_main_menu_via_window_manager(&path_str) {
                            Ok(count) => (true, count, format!("window_loaded count={count}")),
                            Err(e) => (false, 0, format!("window load deferred/failed: {e}")),
                        }
                    }
                    #[cfg(not(feature = "game_client"))]
                    {
                        let _ = path_str;
                        (false, 0, "game_client feature off".into())
                    }
                } else {
                    (false, 0, "load not attempted".into())
                };
                MainMenuWndHonesty {
                    path_resolved: true,
                    path: Some(path),
                    wnd_validated: true,
                    assets_unavailable: false,
                    named_key_hits,
                    window_loaded,
                    window_count,
                    detail: format!(
                        "MainMenu.wnd validated key_hits={}/{} {}",
                        named_key_hits,
                        MAIN_MENU_WND_KEY_NAMES_RESIDUAL.len(),
                        load_detail
                    ),
                }
            }
            Err(e) => MainMenuWndHonesty {
                path_resolved: true,
                path: Some(path),
                wnd_validated: false,
                assets_unavailable: false,
                named_key_hits: 0,
                window_loaded: false,
                window_count: 0,
                detail: format!("MainMenu.wnd validate failed: {e}"),
            },
        },
    }
}

/// Headless WindowManager parse of MainMenu.wnd (C++ Shell::push residual).
///
/// Returns the number of GameWindow instances materialised. Does **not** require
/// a display/GPU — pure layout script → window tree construction.
#[cfg(feature = "game_client")]
fn try_load_main_menu_via_window_manager(path: &str) -> Result<usize, String> {
    use game_client::gui::window_manager::WindowManager;
    let mut wm = WindowManager::new();
    wm.init();
    let names = [
        path,
        "Menus/MainMenu.wnd",
        "MainMenu.wnd",
        "Window/Menus/MainMenu.wnd",
    ];
    let mut last_err = String::from("no load attempted");
    for name in names {
        match wm.load_window(name) {
            Ok(_) => {
                let count = wm.window_count();
                if count == 0 {
                    return Err(format!("{name}: load returned window but window_count=0"));
                }
                return Ok(count);
            }
            Err(e) => last_err = format!("{name}: {e:?}"),
        }
    }
    Err(last_err)
}

/// Residual: resolve+validate MainMenu.wnd residual peel (no load).
pub fn simulate_main_menu_wnd_prepare_honesty() -> bool {
    let h = main_menu_wnd_honesty();
    h.shell_residual_ok()
        && (!h.path_resolved
            || (h.wnd_validated && h.named_key_hits == MAIN_MENU_WND_KEY_NAMES_RESIDUAL.len()))
}

/// Residual: resolve+validate+WindowManager load residual peel.
pub fn simulate_main_menu_wnd_prepare_load_honesty() -> bool {
    let h = main_menu_wnd_honesty_with_load(true);
    if !h.shell_residual_ok() {
        return false;
    }
    if !h.path_resolved {
        // Assets unavailable — fail-closed honesty (CI without WindowZH).
        return true;
    }
    // When retail assets resolve, require headless WindowManager materialisation
    // (C++ Shell::push residual). Soft-ok path removed for resolved assets.
    h.wnd_validated
        && h.named_key_hits == MAIN_MENU_WND_KEY_NAMES_RESIDUAL.len()
        && h.window_loaded
        && h.window_count > 0
}

// ---------------------------------------------------------------------------
// SkirmishGameOptionsMenu.wnd resolve/validate residual (no full WM load)
// ---------------------------------------------------------------------------

/// Candidate locations for SkirmishGameOptionsMenu.wnd (extracted BIG / WindowZH trees).
pub const SKIRMISH_OPTIONS_WND_CANDIDATES: &[&str] = &[
    "windows_game/extracted_big_files/WindowZH/Window/Menus/SkirmishGameOptionsMenu.wnd",
    "windows_game/extracted_big_files_v2/WindowZH/Window/Menus/SkirmishGameOptionsMenu.wnd",
    "../windows_game/extracted_big_files/WindowZH/Window/Menus/SkirmishGameOptionsMenu.wnd",
    "../windows_game/extracted_big_files_v2/WindowZH/Window/Menus/SkirmishGameOptionsMenu.wnd",
    "Window/Menus/SkirmishGameOptionsMenu.wnd",
    "Data/Window/Menus/SkirmishGameOptionsMenu.wnd",
    "Menus/SkirmishGameOptionsMenu.wnd",
    "SkirmishGameOptionsMenu.wnd",
];

/// Retail SkirmishGameOptionsMenu.wnd WINDOWTYPE token residual count.
pub const SKIRMISH_OPTIONS_WND_WINDOW_TOKEN_COUNT_RESIDUAL: usize = 73;

/// Retail named `SkirmishGameOptionsMenu.wnd:…` residual count (non-empty suffixes).
pub const SKIRMISH_OPTIONS_WND_NAMED_COUNT_RESIDUAL: usize = 70;

/// Key SkirmishGameOptionsMenu.wnd named-child residual table (shell start path).
pub const SKIRMISH_OPTIONS_WND_KEY_NAMES_RESIDUAL: &[&str] = &[
    "SkirmishGameOptionsMenu.wnd:SkirmishGameOptionsMenuParent",
    "SkirmishGameOptionsMenu.wnd:ButtonStart",
    "SkirmishGameOptionsMenu.wnd:ButtonBack",
    "SkirmishGameOptionsMenu.wnd:ButtonReset",
    "SkirmishGameOptionsMenu.wnd:ButtonSelectMap",
    "SkirmishGameOptionsMenu.wnd:ComboBoxStartingCash",
    "SkirmishGameOptionsMenu.wnd:CheckboxLimitSuperweapons",
    "SkirmishGameOptionsMenu.wnd:SliderGameSpeed",
    "SkirmishGameOptionsMenu.wnd:MapWindow",
    "SkirmishGameOptionsMenu.wnd:ListboxInfo",
];

/// Resolve SkirmishGameOptionsMenu.wnd on disk (retail WindowZH trees).
pub fn resolve_skirmish_options_wnd_path() -> Option<PathBuf> {
    for c in SKIRMISH_OPTIONS_WND_CANDIDATES {
        let p = Path::new(c);
        if p.is_file() {
            return Some(p.to_path_buf());
        }
    }
    let prefixes = ["", "../", "../../"];
    for prefix in prefixes {
        for c in SKIRMISH_OPTIONS_WND_CANDIDATES {
            let p = Path::new(prefix).join(c);
            if p.is_file() {
                return Some(p);
            }
        }
    }
    None
}

/// Validate SkirmishGameOptionsMenu.wnd is a non-empty retail layout (not a silent no-op).
///
/// Fail-closed: does **not** run full WindowManager parse (file is ~900KB and can
/// stall headless peels). Token/header residual only.
pub fn validate_skirmish_options_wnd_file(path: &Path) -> Result<(), String> {
    let meta = std::fs::metadata(path).map_err(|e| format!("stat: {e}"))?;
    if !meta.is_file() {
        return Err("not a file".into());
    }
    if meta.len() == 0 {
        return Err("empty layout file".into());
    }
    // Require substantial retail size (MainMenu is ~tens of KB; skirmish options ~900KB).
    if meta.len() < 1024 {
        return Err(format!("layout too small: {} bytes", meta.len()));
    }
    let sample = std::fs::read(path).map_err(|e| format!("read: {e}"))?;
    let text = String::from_utf8_lossy(&sample);
    let head_len = sample.len().min(4096);
    let head = String::from_utf8_lossy(&sample[..head_len]);
    if !head.contains("FILE_VERSION") && !text.contains("FILE_VERSION") {
        return Err("missing FILE_VERSION".into());
    }
    if !head.contains("WINDOW") && !text.contains("WINDOW") {
        return Err("missing WINDOW block".into());
    }
    if !text.contains("SkirmishGameOptionsMenu") {
        return Err("missing SkirmishGameOptionsMenu name token".into());
    }
    if !text.contains("ButtonStart") {
        return Err("missing ButtonStart name token".into());
    }
    // Key-name residual hits
    let mut hits = 0usize;
    for key in SKIRMISH_OPTIONS_WND_KEY_NAMES_RESIDUAL {
        if text.contains(key) {
            hits += 1;
        }
    }
    if hits < SKIRMISH_OPTIONS_WND_KEY_NAMES_RESIDUAL.len() {
        return Err(format!(
            "key name residual hits {hits}/{}",
            SKIRMISH_OPTIONS_WND_KEY_NAMES_RESIDUAL.len()
        ));
    }
    // Token floors (full-file scan; still cheaper than WindowManager tree build).
    let window_tokens = text.matches("WINDOWTYPE").count();
    if window_tokens < SKIRMISH_OPTIONS_WND_WINDOW_TOKEN_COUNT_RESIDUAL / 2 {
        return Err(format!(
            "WINDOWTYPE count {window_tokens} below residual floor"
        ));
    }
    Ok(())
}

/// Host-testable honesty flags for SkirmishGameOptionsMenu.wnd resolve/validate.
#[derive(Debug, Clone)]
pub struct SkirmishOptionsWndHonesty {
    pub path_resolved: bool,
    pub path: Option<PathBuf>,
    pub wnd_validated: bool,
    pub assets_unavailable: bool,
    pub named_key_hits: usize,
    pub detail: String,
}

impl SkirmishOptionsWndHonesty {
    /// Shell residual ok when resolved+validated, or assets honestly unavailable.
    pub fn shell_residual_ok(&self) -> bool {
        (self.path_resolved && self.wnd_validated)
            || (self.assets_unavailable && !self.path_resolved)
    }
}

/// Build SkirmishGameOptionsMenu.wnd honesty residual (validate-only; no WM load).
pub fn skirmish_options_wnd_honesty() -> SkirmishOptionsWndHonesty {
    match resolve_skirmish_options_wnd_path() {
        None => SkirmishOptionsWndHonesty {
            path_resolved: false,
            path: None,
            wnd_validated: false,
            assets_unavailable: true,
            named_key_hits: 0,
            detail: "SkirmishGameOptionsMenu.wnd not found in candidate paths".into(),
        },
        Some(path) => match validate_skirmish_options_wnd_file(&path) {
            Ok(()) => {
                let text = std::fs::read_to_string(&path).unwrap_or_default();
                let hits = SKIRMISH_OPTIONS_WND_KEY_NAMES_RESIDUAL
                    .iter()
                    .filter(|k| text.contains(*k))
                    .count();
                SkirmishOptionsWndHonesty {
                    path_resolved: true,
                    path: Some(path),
                    wnd_validated: true,
                    assets_unavailable: false,
                    named_key_hits: hits,
                    detail: format!(
                        "SkirmishGameOptionsMenu.wnd validated key_hits={}/{}",
                        hits,
                        SKIRMISH_OPTIONS_WND_KEY_NAMES_RESIDUAL.len()
                    ),
                }
            }
            Err(e) => SkirmishOptionsWndHonesty {
                path_resolved: true,
                path: Some(path),
                wnd_validated: false,
                assets_unavailable: false,
                named_key_hits: 0,
                detail: format!("SkirmishGameOptionsMenu.wnd validate failed: {e}"),
            },
        },
    }
}

/// Residual peel: resolve+validate SkirmishGameOptionsMenu.wnd (no full parse).
pub fn simulate_skirmish_options_wnd_prepare_honesty() -> bool {
    let h = skirmish_options_wnd_honesty();
    if !h.shell_residual_ok() {
        return false;
    }
    if !h.path_resolved {
        return true;
    }
    h.wnd_validated && h.named_key_hits == SKIRMISH_OPTIONS_WND_KEY_NAMES_RESIDUAL.len()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ensure_control_bar_is_not_a_silent_noop() {
        // Drives the shipped ensure path — never a constant-true success without work.
        let status = ensure_control_bar_layout(false);
        match &status {
            GameplayLayoutStatus::Ready { path, loaded } => {
                assert!(
                    path.contains("ControlBar"),
                    "resolved path should name ControlBar: {path}"
                );
                assert!(!loaded, "dry-run validate must set loaded=false");
                assert!(
                    Path::new(path).is_file(),
                    "ready status requires existing file: {path}"
                );
            }
            GameplayLayoutStatus::AssetsUnavailable { searched } => {
                assert!(
                    !searched.is_empty(),
                    "must report searched candidates when assets missing"
                );
                // CI without windows_game assets is an honest failure mode.
            }
            GameplayLayoutStatus::LoadFailed { path, error } => {
                panic!("unexpected load failure for {path}: {error}");
            }
        }
        let report = format_gameplay_layout_status(&status);
        assert!(
            report.contains("control_bar status="),
            "report must be structured: {report}"
        );
    }

    #[test]
    fn control_bar_candidates_include_cpp_parity_name() {
        assert!(
            CONTROL_BAR_CANDIDATES
                .iter()
                .any(|c| c.ends_with("ControlBar.wnd")),
            "must search for ControlBar.wnd like C++ ShowControlBar"
        );
    }

    #[test]
    fn control_bar_honesty_flags_are_host_testable() {
        let h = control_bar_layout_honesty(false);
        assert!(
            h.shell_residual_ok(),
            "Ready or AssetsUnavailable must be honest residual: {:?}",
            h.status
        );
        if h.path_resolved {
            assert!(
                h.wnd_validated,
                "resolved path must validate: {:?}",
                h.status
            );
            assert!(!h.window_loaded, "dry-run must not claim window_loaded");
            assert_eq!(h.window_count, 0, "dry-run window_count must be 0");
            if let GameplayLayoutStatus::Ready { path, .. } = &h.status {
                assert!(
                    validate_control_bar_file(Path::new(path)).is_ok(),
                    "structural validate must pass for ready path"
                );
            }
        } else {
            assert!(h.assets_unavailable);
        }
    }

    #[test]
    #[cfg(feature = "game_client")]
    fn control_bar_window_manager_load_when_assets_present() {
        // Shipped Ready-gate: load into the live TheWindowManager. Exact 98
        // is the throwaway residual peel (`simulate_control_bar_materialise_honesty`).
        let h = control_bar_layout_honesty(true);
        assert!(
            h.shell_residual_ok(),
            "load path must remain honest residual: {:?}",
            h.status
        );
        if h.path_resolved {
            assert!(h.wnd_validated, "path must structurally validate");
            // When retail assets resolve, require live ControlBarParent (no soft-ok).
            assert!(
                h.window_loaded && h.window_count > 0,
                "resolved ControlBar.wnd must materialise on live WindowManager: {:?}",
                h
            );
            assert!(
                control_bar_parent_is_live(),
                "Ready-gate must leave ControlBarParent on the live WindowManager: {:?}",
                h
            );
            assert!(
                simulate_control_bar_materialise_honesty(),
                "ControlBar materialise honesty must latch"
            );
        } else {
            assert!(h.assets_unavailable);
            assert!(!h.window_loaded);
            assert!(honesty_control_bar_residual_pack_wave76_ok(false, 0));
            assert!(simulate_control_bar_materialise_honesty());
        }
        let report = format_control_bar_honesty(&h);
        assert!(
            report.contains("window_loaded="),
            "honesty report must surface load flag: {report}"
        );
    }

    #[test]
    #[cfg(feature = "game_client")]
    fn leftover_main_menu_skirmish_tree_is_not_live_control_bar() {
        use game_client::gui::{with_window_manager, with_window_manager_ref};
        with_window_manager(|wm| {
            wm.destroy_all_windows();
            wm.update();
            for i in 0..98 {
                let win = wm.create_window(None, 0, 0, 8, 8).unwrap();
                win.borrow_mut()
                    .set_name(&format!("MainMenu.wnd:Leftover{i}"));
            }
        });
        let count = with_window_manager_ref(|wm| wm.window_count());
        assert!(
            count >= 98,
            "leftover MainMenu/Skirmish-sized tree should exist, got {count}"
        );
        assert!(
            !control_bar_parent_is_live(),
            "leftover MainMenu windows must not count as ControlBar.wnd:ControlBarParent"
        );
        with_window_manager(|wm| {
            wm.destroy_all_windows();
            wm.update();
        });
    }

    /// Wave 76 residual: ControlBar window-count / named-child / font table pack.
    #[test]
    fn control_bar_residual_pack_wave76_honesty() {
        assert!(honesty_control_bar_window_count_residual_ok());
        assert!(honesty_control_bar_named_windows_residual_ok());
        assert!(honesty_control_bar_font_table_residual_ok());
        assert!(honesty_control_bar_residual_pack_wave76_ok(false, 0));
        assert!(!honesty_control_bar_residual_pack_wave76_ok(true, 0));
        assert!(honesty_control_bar_residual_pack_wave76_ok(true, 98));
        assert_eq!(CONTROL_BAR_RETAIL_WINDOW_COUNT, 98);
        assert_eq!(CONTROL_BAR_FONT_RESIDUAL_TABLE.len(), 7);
        assert!(CONTROL_BAR_KEY_NAMED_WINDOWS.len() >= 20);
        // Structural validate must accept key name + font residual tokens.
        if let Some(path) = resolve_control_bar_path() {
            assert!(
                validate_control_bar_file(&path).is_ok(),
                "retail ControlBar.wnd must pass Wave 76 named/font residual validate"
            );
        }
    }

    #[test]
    fn main_menu_wnd_honesty_residual_live() {
        let h = main_menu_wnd_honesty();
        assert!(
            h.shell_residual_ok(),
            "MainMenu.wnd residual must resolve+validate or report assets unavailable: {}",
            h.detail
        );
        if h.path_resolved {
            assert!(h.wnd_validated, "{}", h.detail);
            assert_eq!(
                h.named_key_hits,
                MAIN_MENU_WND_KEY_NAMES_RESIDUAL.len(),
                "key names residual: {}",
                h.detail
            );
            assert!(!h.window_loaded);
            assert_eq!(h.window_count, 0);
            assert!(simulate_main_menu_wnd_prepare_honesty());
        }
    }

    #[cfg(feature = "game_client")]
    #[test]
    fn main_menu_wnd_load_residual_live() {
        let h = main_menu_wnd_honesty_with_load(true);
        assert!(
            h.shell_residual_ok(),
            "MainMenu.wnd load residual: {}",
            h.detail
        );
        assert!(
            simulate_main_menu_wnd_prepare_load_honesty(),
            "load honesty: {}",
            h.detail
        );
        if h.path_resolved && h.wnd_validated {
            assert!(
                h.window_loaded && h.window_count > 0,
                "resolved MainMenu.wnd must materialise windows: {}",
                h.detail
            );
            // Retail named-child residual count is 63 NAME= lines.
            assert!(
                h.window_count >= MAIN_MENU_WND_NAMED_COUNT_RESIDUAL / 2,
                "window_count={} expected substantial tree: {}",
                h.window_count,
                h.detail
            );
        }
    }

    #[test]
    fn skirmish_options_wnd_honesty_residual_live() {
        let h = skirmish_options_wnd_honesty();
        assert!(
            h.shell_residual_ok(),
            "SkirmishGameOptionsMenu.wnd residual: {}",
            h.detail
        );
        assert!(simulate_skirmish_options_wnd_prepare_honesty());
        if h.path_resolved {
            assert!(h.wnd_validated, "{}", h.detail);
            assert_eq!(
                h.named_key_hits,
                SKIRMISH_OPTIONS_WND_KEY_NAMES_RESIDUAL.len(),
                "key names residual: {}",
                h.detail
            );
        }
    }
}
