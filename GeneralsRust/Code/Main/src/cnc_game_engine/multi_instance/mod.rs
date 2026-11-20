//! Multi-instance host: N independent game engines in one process.
//!
//! Each instance is a complete `CnCGameEngine` (its own `GameLogic`,
//! `GameClient`, subsystem manager, archive/asset owners and service bags)
//! booted and driven on its own thread. Nothing is shared between instances
//! except the immutable process environment and one host-owned wgpu device:
//! every engine adopts that device through its own `DeviceAuthority`, renders
//! into its own off-screen target, and copies the finished frame into a tile
//! texture the host composites into a labelled grid (one window, or an
//! off-screen grid for screenshot dumps).
//!
//! Thread binding follows `docs/instance_binding.md`: each instance thread
//! publishes its own `MainServices` / `CommonServices` scope for its whole
//! life, so the thread-local stacks never see another instance.

mod compositor;
mod instance;
mod scenario;

pub use instance::{InstancePhase, InstanceStatus, Verdict};
pub use scenario::{e2e_suite, Assertions, InstanceScenario};

use compositor::GridCompositor;
use instance::InstanceHandle;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};
use winit::event::{Event, WindowEvent};
use winit::event_loop::{ControlFlow, EventLoop};
use winit::window::Window;

/// What the host runs and how it shows it.
#[derive(Debug, Clone)]
pub struct MultiInstanceOptions {
    /// Number of game instances.
    pub instances: usize,
    /// Scenario per instance; cycled when shorter than `instances`.
    pub scenarios: Vec<InstanceScenario>,
    /// Render resolution of every instance (C++ default display 800x600).
    pub instance_size: (u32, u32),
    /// Displayed size of one grid cell.
    pub cell_size: (u32, u32),
    /// Grid columns (`None` picks 4x2 for 8, 4x4 for 16, ...).
    pub columns: Option<u32>,
    /// Show the grid in a window; `false` composites off-screen only.
    pub windowed: bool,
    /// Directory for grid / per-tile PNG dumps.
    pub shots_dir: Option<PathBuf>,
    /// Dump once every running match has reached this logic frame.
    pub shot_logic_frame: u32,
    /// Dump anyway after this long.
    pub shot_timeout: Duration,
    /// Stop after this long (`None` = until the window closes, or after the
    /// dump when not windowed).
    pub run_for: Option<Duration>,
    /// Instance present rate.
    pub present_fps: u32,
    /// Host updates per second of every instance (each update steps the
    /// logic once, as the headless runtime host drives it). 60 is the grid's
    /// sim target; 0 runs uncapped to measure capacity.
    pub update_hz: u32,
    /// Instances allowed to boot at the same time.
    pub boot_parallelism: usize,
    /// Private user-data root; instance `i` uses `<root>/instance-<i>`.
    pub userdata_root: PathBuf,
    /// Extra engine command-line switches for every instance.
    pub extra_args: Vec<String>,
    /// Pass/fail rules of host-assessed scenarios.
    pub assertions: Assertions,
    /// Wall budget of one attached retail gate (the `*_gate` binaries
    /// default to 180 s for a single process; a full grid shares the CPU).
    pub gate_timeout: Duration,
    /// Stop once every tile has a verdict (and the dump, if any, is taken).
    pub stop_when_decided: bool,
}

impl Default for MultiInstanceOptions {
    fn default() -> Self {
        Self {
            instances: 8,
            scenarios: vec![InstanceScenario::golden()],
            instance_size: (800, 600),
            cell_size: (400, 300),
            columns: None,
            windowed: true,
            shots_dir: None,
            shot_logic_frame: 300,
            shot_timeout: Duration::from_secs(900),
            run_for: None,
            present_fps: 15,
            update_hz: 60,
            boot_parallelism: 4,
            userdata_root: std::env::temp_dir().join("generals_multi"),
            extra_args: Vec::new(),
            assertions: Assertions::default(),
            gate_timeout: Duration::from_secs(900),
            stop_when_decided: false,
        }
    }
}

/// Grid columns for `n` tiles: 4x2 for 8 and 4x4 for 16.
pub fn grid_columns(n: usize) -> u32 {
    match n {
        0 | 1 => 1,
        2..=4 => 2,
        5 | 6 | 9 => 3,
        7 | 8 | 10..=16 => 4,
        _ => (n as f64).sqrt().ceil() as u32,
    }
}

/// Per-instance result of a run.
#[derive(Debug, Clone)]
pub struct InstanceReport {
    pub index: usize,
    pub scenario: String,
    pub status: InstanceStatus,
}

/// Outcome of a multi-instance run.
#[derive(Debug, Clone, Default)]
pub struct MultiInstanceReport {
    pub instances: Vec<InstanceReport>,
    pub shots: Vec<PathBuf>,
    pub rss_before_boot_bytes: u64,
    pub rss_peak_bytes: u64,
    pub rss_after_boot_bytes: u64,
}

impl MultiInstanceReport {
    /// (pass, fail, pending) tile verdicts.
    pub fn verdict_counts(&self) -> (usize, usize, usize) {
        let mut counts = (0, 0, 0);
        for r in &self.instances {
            match r.status.verdict {
                Verdict::Pass(_) => counts.0 += 1,
                Verdict::Fail(_) => counts.1 += 1,
                Verdict::Pending => counts.2 += 1,
            }
        }
        counts
    }

    /// Human-readable summary (one line per instance).
    pub fn summary(&self) -> String {
        let mut out = String::new();
        for r in &self.instances {
            out.push_str(&format!(
                "#{:02} {:<28} {}\n",
                r.index,
                r.scenario,
                r.status.one_line()
            ));
        }
        let n = self.instances.len().max(1) as u64;
        let mb = |b: u64| b as f64 / (1024.0 * 1024.0);
        out.push_str(&format!(
            "RSS: before boot {:.0} MiB, after boot {:.0} MiB, peak {:.0} MiB, ~{:.0} MiB peak per instance\n",
            mb(self.rss_before_boot_bytes),
            mb(self.rss_after_boot_bytes),
            mb(self.rss_peak_bytes),
            mb(self.rss_peak_bytes.saturating_sub(self.rss_before_boot_bytes) / n),
        ));
        let (pass, fail, pending) = self.verdict_counts();
        out.push_str(&format!(
            "verdicts: {pass} pass, {fail} fail, {pending} pending\n"
        ));
        for shot in &self.shots {
            out.push_str(&format!("shot: {}\n", shot.display()));
        }
        out
    }
}

/// Resident set size of this process (current, peak) in bytes.
pub fn process_rss() -> (u64, u64) {
    #[cfg(target_os = "macos")]
    {
        let mut info = std::mem::MaybeUninit::<libc::mach_task_basic_info>::uninit();
        let info_count = (std::mem::size_of::<libc::mach_task_basic_info>()
            / std::mem::size_of::<libc::natural_t>())
            as libc::mach_msg_type_number_t;
        let mut count = info_count;
        // SAFETY: MACH_TASK_BASIC_INFO accepts a writable buffer of exactly
        // this Mach ABI type and count. The current task port is borrowed.
        // Both fields are read only after Mach reports a fully written buffer.
        unsafe {
            #[allow(deprecated)]
            let task = libc::mach_task_self();
            let kr = libc::task_info(task, libc::MACH_TASK_BASIC_INFO, info.as_mut_ptr().cast(), &mut count);
            if kr == libc::KERN_SUCCESS && count == info_count {
                let info = info.assume_init();
                return (info.resident_size, info.resident_size_max);
            }
        }
        (0, 0)
    }
    #[cfg(target_os = "linux")]
    {
        let read = |key: &str| -> u64 {
            std::fs::read_to_string("/proc/self/status")
                .ok()
                .and_then(|s| {
                    s.lines()
                        .find(|l| l.starts_with(key))
                        .and_then(|l| l.split_whitespace().nth(1))
                        .and_then(|v| v.parse::<u64>().ok())
                })
                .unwrap_or(0)
                * 1024
        };
        (read("VmRSS:"), read("VmHWM:"))
    }
    #[cfg(not(any(target_os = "macos", target_os = "linux")))]
    {
        (0, 0)
    }
}

/// Run `options.instances` engines and composite them until done.
///
/// Must be called on the main thread (winit owns it on macOS). The current
/// tokio runtime handle is entered by every instance thread so engine code
/// that spawns tasks finds a runtime, as it does under `generals`.
pub fn run_multi_instance(
    event_loop: EventLoop<()>,
    options: MultiInstanceOptions,
) -> anyhow::Result<MultiInstanceReport> {
    anyhow::ensure!(options.instances > 0, "need at least one instance");
    anyhow::ensure!(!options.scenarios.is_empty(), "need at least one scenario");
    let runtime = tokio::runtime::Handle::try_current().ok();
    let (iw, ih) = options.instance_size;
    let gpu_config = ww3d_engine::EngineConfig {
        width: iw,
        height: ih,
        ..Default::default()
    };
    let gpu = pollster::block_on(ww3d_engine::HostGpu::request(&gpu_config))
        .map_err(|e| anyhow::anyhow!("host GPU: {e:?}"))?;
    log::info!(
        "multi-instance host: {} instances on {:?}",
        options.instances,
        gpu.adapter.get_info().name
    );

    let columns = options
        .columns
        .unwrap_or_else(|| grid_columns(options.instances))
        .max(1);
    let rows = (options.instances as u32).div_ceil(columns);
    let grid_size = (options.cell_size.0 * columns, options.cell_size.1 * rows);

    let mut report = MultiInstanceReport {
        rss_before_boot_bytes: process_rss().0,
        ..Default::default()
    };
    let boot_gate = Arc::new(instance::BootGate::new(options.boot_parallelism.max(1)));
    let shared_retail = crate::assets::ww3d_asset_manager::SharedRetailContent::default();
    let mut instances: Vec<InstanceHandle> = Vec::new();
    let mut grid_window: Option<Arc<Window>> = None;
    let mut compositor: Option<GridCompositor> = None;
    let started = Instant::now();
    let mut next_composite = Instant::now();
    let mut next_status_log = Instant::now() + Duration::from_secs(5);
    let mut shot_taken = options.shots_dir.is_none();
    let mut stopping = false;
    let mut boot_rss_recorded = false;
    let mut outcome: Result<(), String> = Ok(());
    let composite_interval = Duration::from_millis(33);

    #[allow(deprecated)]
    event_loop.run(|event, elwt| {
        let mut step = || -> Result<(), String> {
            match event {
                Event::Resumed if instances.is_empty() => {
                    if options.windowed {
                        let attrs = Window::default_attributes()
                            .with_title(format!(
                                "Command & Conquer Generals Zero Hour - {} instances",
                                options.instances
                            ))
                            .with_inner_size(winit::dpi::LogicalSize::new(grid_size.0, grid_size.1))
                            .with_resizable(true);
                        let window = Arc::new(
                            elwt.create_window(attrs)
                                .map_err(|e| format!("grid window: {e}"))?,
                        );
                        grid_window = Some(window);
                    }
                    let mut comp = GridCompositor::new(
                        &gpu,
                        grid_window.clone(),
                        grid_size,
                        columns,
                        rows,
                        options.instances,
                        options.instance_size,
                    )?;
                    for index in 0..options.instances {
                        // Every engine still owns a (hidden) native window:
                        // C++ TheDisplay sizes come from it and input attaches
                        // to it. It is never shown or presented to.
                        let attrs = Window::default_attributes()
                            .with_title(format!("generals instance {index}"))
                            .with_inner_size(winit::dpi::LogicalSize::new(iw, ih))
                            .with_visible(false);
                        let window = Arc::new(
                            elwt.create_window(attrs)
                                .map_err(|e| format!("instance window {index}: {e}"))?,
                        );
                        let scenario = options.scenarios[index % options.scenarios.len()].clone();
                        let userdata = options.userdata_root.join(format!("instance-{index}"));
                        let bridge = match scenario.gate() {
                            Some(_) => {
                                let dir = userdata.join("runtime-host");
                                std::fs::create_dir_all(&dir)
                                    .map_err(|e| format!("bridge dir {}: {e}", dir.display()))?;
                                Some(instance::InstanceBridgePaths::in_dir(&dir))
                            }
                            None => None,
                        };
                        let mut handle = InstanceHandle::spawn(instance::InstanceSpawn {
                            index,
                            scenario: scenario.clone(),
                            window,
                            gpu: gpu.clone(),
                            tile: comp.tile_texture(index).clone(),
                            userdata,
                            extra_args: options.extra_args.clone(),
                            present_interval: Duration::from_secs_f64(
                                1.0 / f64::from(options.present_fps.max(1)),
                            ),
                            update_interval: (options.update_hz > 0)
                                .then(|| Duration::from_secs_f64(1.0 / f64::from(options.update_hz))),
                            runtime: runtime.clone(),
                            boot_gate: Arc::clone(&boot_gate),
                            shared_retail: shared_retail.clone(),
                            bridge: bridge.clone(),
                        })
                        .map_err(|e| format!("spawn instance {index}: {e}"))?;
                        if let (Some(gate), Some(bridge)) = (scenario.gate(), bridge) {
                            let host = crate::executable_smoke::AttachedSmokeHost {
                                control_path: bridge.control,
                                status_path: bridge.status,
                                exit: bridge.exit,
                                map: scenario.gate_map_path(instance::retail_assets_dir().as_deref()),
                            };
                            let timeout = options.gate_timeout;
                            handle
                                .attach_driver(move || {
                                    let smoke = crate::executable_smoke::SmokeOptions::from_environment();
                                    let r = crate::executable_smoke::run_attached_smoke(
                                        gate, host, timeout, &smoke,
                                    );
                                    if gate.passed(&r) {
                                        Verdict::Pass(format!("{} {}", gate.name(), r.status))
                                    } else {
                                        Verdict::Fail(format!(
                                            "{} {}: {}",
                                            gate.name(),
                                            r.status,
                                            r.detail.chars().take(120).collect::<String>()
                                        ))
                                    }
                                })
                                .map_err(|e| format!("gate driver {index}: {e}"))?;
                        }
                        instances.push(handle);
                    }
                    compositor = Some(comp);
                }
                Event::WindowEvent {
                    window_id,
                    event: ref window_event,
                } => {
                    let is_grid = grid_window.as_ref().is_some_and(|w| w.id() == window_id);
                    if is_grid {
                        match window_event {
                            WindowEvent::CloseRequested => stopping = true,
                            WindowEvent::Resized(size) => {
                                if let Some(comp) = compositor.as_mut() {
                                    comp.resize_surface(size.width, size.height);
                                }
                            }
                            WindowEvent::RedrawRequested => {
                                if let Some(comp) = compositor.as_mut() {
                                    let statuses: Vec<_> =
                                        instances.iter().map(InstanceHandle::status).collect();
                                    comp.present(&statuses)?;
                                }
                            }
                            _ => {}
                        }
                    }
                }
                Event::AboutToWait => {
                    let now = Instant::now();
                    let statuses: Vec<InstanceStatus> =
                        instances.iter().map(InstanceHandle::status).collect();
                    // Host-assessed verdicts; a crash after a pass still fails.
                    for (handle, status) in instances.iter().zip(statuses.iter()) {
                        if handle.has_driver() {
                            continue;
                        }
                        match &status.verdict {
                            Verdict::Pending => {
                                let verdict = handle.scenario().assess(
                                    status,
                                    started.elapsed(),
                                    &options.assertions,
                                );
                                if verdict.is_decided() {
                                    log::info!(
                                        "instance {} verdict: {} {}",
                                        status.index,
                                        verdict.name(),
                                        verdict.detail()
                                    );
                                    handle.set_verdict(verdict);
                                }
                            }
                            Verdict::Pass(_) => {
                                if let InstancePhase::Failed(reason) = &status.phase {
                                    handle.set_verdict(Verdict::Fail(format!(
                                        "failed after pass: {}",
                                        reason.chars().take(80).collect::<String>()
                                    )));
                                }
                            }
                            Verdict::Fail(_) => {}
                        }
                    }
                    let (rss, peak) = process_rss();
                    report.rss_peak_bytes = report.rss_peak_bytes.max(peak.max(rss));
                    if !boot_rss_recorded
                        && !statuses.is_empty()
                        && statuses
                            .iter()
                            .all(|s| !matches!(s.phase, InstancePhase::Booting))
                    {
                        boot_rss_recorded = true;
                        report.rss_after_boot_bytes = rss;
                        log::info!(
                            "multi-instance: all {} instances booted in {:.1}s, RSS {:.0} MiB",
                            statuses.len(),
                            started.elapsed().as_secs_f32(),
                            rss as f64 / 1048576.0
                        );
                    }
                    if now >= next_status_log {
                        next_status_log = now + Duration::from_secs(5);
                        for (i, s) in statuses.iter().enumerate() {
                            println!("[multi] #{i:02} {}", s.one_line());
                        }
                        println!("[multi] RSS {:.0} MiB", rss as f64 / 1048576.0);
                    }
                    if !shot_taken && !stopping {
                        let all_ready = !statuses.is_empty()
                            && statuses
                                .iter()
                                .zip(instances.iter())
                                .all(|(s, h)| h.scenario().shot_ready(s, options.shot_logic_frame));
                        if all_ready || started.elapsed() >= options.shot_timeout {
                            if let (Some(comp), Some(dir)) =
                                (compositor.as_mut(), options.shots_dir.as_ref())
                            {
                                let tag = format!("x{}", options.instances);
                                let shots = comp.dump(dir, &tag, &statuses)?;
                                for s in &shots {
                                    println!("[multi] wrote {}", s.display());
                                }
                                report.shots.extend(shots);
                            }
                            shot_taken = true;
                            if !options.windowed && options.run_for.is_none() {
                                stopping = true;
                            }
                        }
                    }
                    if options
                        .run_for
                        .is_some_and(|limit| started.elapsed() >= limit)
                    {
                        stopping = true;
                    }
                    if options.stop_when_decided
                        && shot_taken
                        && !statuses.is_empty()
                        && statuses.iter().all(|s| s.verdict.is_decided())
                    {
                        stopping = true;
                    }
                    if !options.windowed
                        && !instances.is_empty()
                        && statuses.iter().all(|s| s.phase.is_terminal())
                    {
                        stopping = true;
                    }
                    if stopping {
                        for h in &instances {
                            h.request_stop();
                        }
                        // Instance threads may still call into their hidden
                        // windows, which macOS services on this thread, so
                        // keep pumping until every one of them has finished.
                        if instances.iter().all(InstanceHandle::is_finished) {
                            elwt.exit();
                            return Ok(());
                        }
                    }
                    if now >= next_composite {
                        next_composite = now + composite_interval;
                        if let Some(window) = grid_window.as_ref() {
                            window.request_redraw();
                        }
                    }
                    elwt.set_control_flow(ControlFlow::WaitUntil(
                        next_composite.min(now + Duration::from_millis(50)),
                    ));
                }
                Event::LoopExiting => {
                    for h in &instances {
                        h.request_stop();
                    }
                }
                _ => {}
            }
            Ok(())
        };
        if let Err(err) = step() {
            log::error!("multi-instance host: {err}");
            if outcome.is_ok() {
                outcome = Err(err);
            }
            stopping = true;
        }
    })?;

    for handle in instances {
        let index = handle.index();
        let scenario = handle.scenario().label();
        let status = handle.join();
        report.instances.push(InstanceReport {
            index,
            scenario,
            status,
        });
    }
    drop(compositor);
    outcome.map_err(anyhow::Error::msg)?;
    Ok(report)
}
