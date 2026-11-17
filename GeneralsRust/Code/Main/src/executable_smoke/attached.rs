// Lifecycle: initialization for an attached (in-process) runtime host.
//
// `generals_multi --suite e2e` runs one engine per thread, each serving the
// runtime-host control/status files a spawned `generals` would. The retail
// gate drivers attach to those files instead of spawning a child, so a grid
// tile runs exactly the gate the `*_gate` binaries run.

/// An in-process runtime host the smoke drivers can attach to.
#[derive(Clone, Debug)]
pub struct AttachedSmokeHost {
    /// `-gpui_control` of the host.
    pub control_path: PathBuf,
    /// `-gpui_status` of the host.
    pub status_path: PathBuf,
    /// Set once the host's frame loop has ended (0 = clean exit).
    pub exit: std::sync::Arc<std::sync::Mutex<Option<i32>>>,
    /// Map the skirmish drivers start; `None` = the gates' Lone Eagle.
    pub map: Option<String>,
}

impl AttachedSmokeHost {
    fn exit_code(&self) -> Option<i32> {
        *self.exit.lock().unwrap_or_else(|e| e.into_inner())
    }
}

/// The retail executable gates, as attachable smoke sessions.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SmokeGate {
    /// `boot_shell_menu_gate`: live MainMenu.wnd.
    BootShellMenu,
    /// `retail_skirmish_map_gate`: menu → skirmish → retail map InGame.
    LoadRetailMap,
    /// `player_orders_gate`: select → move → attack on the live host.
    IssueOrders,
    /// `ai_retail_skirmish_gate`: human orders plus AI activity.
    AiOpponent,
    /// `retail_save_load_gate`: save → load → restore → post-load order.
    SaveLoad,
}

impl SmokeGate {
    pub const ALL: [SmokeGate; 5] = [
        Self::BootShellMenu,
        Self::LoadRetailMap,
        Self::IssueOrders,
        Self::AiOpponent,
        Self::SaveLoad,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Self::BootShellMenu => "menu",
            Self::LoadRetailMap => "map_load",
            Self::IssueOrders => "orders",
            Self::AiOpponent => "ai",
            Self::SaveLoad => "save_load",
        }
    }

    /// Whether the gate starts a skirmish (and so uses a map).
    pub fn starts_skirmish(self) -> bool {
        !matches!(self, Self::BootShellMenu)
    }

    fn driver(self) -> SmokeDriver {
        match self {
            Self::BootShellMenu => SmokeDriver::MenuOnly,
            Self::LoadRetailMap => SmokeDriver::MapLoadOnly,
            Self::IssueOrders => SmokeDriver::OrdersOnly,
            Self::AiOpponent => SmokeDriver::AiActivity,
            Self::SaveLoad => SmokeDriver::SaveLoadOnly,
        }
    }

    /// The pass predicate of the gate binary of the same name.
    pub fn passed(self, r: &ExecutableSmokeResult) -> bool {
        let finished = r.status == "success" || r.status == "success_forced_exit";
        finished
            && r.executable_host_ok
            && match self {
                Self::BootShellMenu => r.reached_menu && r.shell_wnd_ok,
                Self::LoadRetailMap => {
                    r.reached_menu
                        && r.shell_wnd_ok
                        && r.reached_ingame
                        && !ExecutableSmokeResult::map_is_shell_for_claim(&r.map_seen)
                }
                Self::IssueOrders | Self::AiOpponent => r.reached_ingame && r.gameplay_cmd_ok,
                Self::SaveLoad => {
                    let empty_units = r.detail.contains("units=0")
                        || r.detail.contains("unit count is 0")
                        || r.detail.contains("world_restore_fail:unit count");
                    r.reached_ingame
                        && r.gameplay_cmd_ok
                        && r.save_cmd_ok
                        && r.load_cmd_ok
                        && !empty_units
                }
            }
    }
}

impl std::str::FromStr for SmokeGate {
    type Err = String;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        let text = text.trim().to_ascii_lowercase();
        Self::ALL
            .into_iter()
            .find(|gate| gate.name() == text)
            .ok_or_else(|| {
                format!("unknown gate {text:?} (menu | map_load | orders | ai | save_load)")
            })
    }
}

/// Run one retail gate against an attached in-process host. The verdict is
/// [`SmokeGate::passed`] on the returned result.
pub fn run_attached_smoke(
    gate: SmokeGate,
    host: AttachedSmokeHost,
    timeout: Duration,
    options: &SmokeOptions,
) -> ExecutableSmokeResult {
    let started = Instant::now();
    let map = host
        .map
        .clone()
        .unwrap_or_else(|| resolve_lone_eagle_map(options.assets_dir.as_deref()));
    let result = ExecutableSmokeResult {
        playable_claim: false,
        new_game_path: gate.starts_skirmish(),
        process_started: true,
        map_seen: map.clone(),
        ..Default::default()
    };
    let boot = SmokeBootstrap {
        tmp: host
            .control_path
            .parent()
            .map(Path::to_path_buf)
            .unwrap_or_default(),
        owns_tmp: false,
        control_path: host.control_path.clone(),
        status_path: host.status_path.clone(),
        map,
        screenshot_path: options.screenshot_path.clone(),
        child: SmokeHost::Attached(host),
    };
    run_smoke_session(
        boot,
        result,
        timeout,
        started,
        gate.starts_skirmish(),
        ExecutableSmokeLaunch::HeadlessHost,
        gate.driver(),
    )
}
