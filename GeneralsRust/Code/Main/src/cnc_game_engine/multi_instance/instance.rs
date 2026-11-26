//! One game instance on its own thread: boot, scenario, frame loop, tile copy.

use super::super::runtime::RuntimeHostBridge;
use super::super::types::register_real_game_client_bootstrap;
use super::super::{CnCGameEngine, GameState, StartupLoadState};
use super::scenario::InstanceScenario;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Condvar, Mutex};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};
use winit::window::Window;

/// One C++ logic frame (LOGICFRAMES_PER_SECOND = 30).
const LOGIC_FRAME: Duration = Duration::from_nanos(33_333_333);
const STARTUP_POLL_INTERVAL: Duration = Duration::from_millis(5);
/// Debug-depth recursion through map load and AI; matches the harness.
const INSTANCE_THREAD_STACK: usize = 64 * 1024 * 1024;

/// Lifecycle of one instance.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InstancePhase {
    Booting,
    Running,
    Failed(String),
    Finished,
}

impl InstancePhase {
    pub fn is_terminal(&self) -> bool {
        matches!(self, Self::Failed(_) | Self::Finished)
    }
}

/// Outcome of the tile's scenario assertions.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Verdict {
    /// Assertions not decided yet.
    Pending,
    Pass(String),
    Fail(String),
}

impl Verdict {
    pub fn is_decided(&self) -> bool {
        !matches!(self, Self::Pending)
    }

    pub fn name(&self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Pass(_) => "PASS",
            Self::Fail(_) => "FAIL",
        }
    }

    pub fn detail(&self) -> &str {
        match self {
            Self::Pending => "",
            Self::Pass(d) | Self::Fail(d) => d,
        }
    }
}

/// Snapshot an instance publishes for the host (labels, report, shots).
#[derive(Debug, Clone)]
pub struct InstanceStatus {
    pub index: usize,
    pub scenario: String,
    pub map: String,
    pub phase: InstancePhase,
    /// `GameState` of the engine (`Menu`, `Loading`, `InGame`, ...).
    pub state: String,
    pub logic_frame: u32,
    pub rendered_frames: u64,
    /// Presents per wall second over the last sample window.
    pub render_fps: f32,
    /// Logic frames per wall second over the last sample window (30 = real time).
    pub sim_rate: f32,
    /// Mean host frame cost (update + render) in ms over the last window.
    pub frame_ms: f32,
    /// Mean CPU time of the instance thread per host frame in ms over the last
    /// window. Unlike `frame_ms` it does not grow when other processes take
    /// the cores, so it is the figure to compare builds on a busy machine.
    pub cpu_ms: f32,
    pub boot_secs: f32,
    /// Runtime-host commands applied for the scenario.
    pub commands: Vec<String>,
    /// Scenario assertion outcome (set by the host).
    pub verdict: Verdict,
}

impl InstanceStatus {
    pub(super) fn new(index: usize, scenario: &InstanceScenario) -> Self {
        Self {
            index,
            scenario: scenario.label(),
            map: scenario.map_name().to_string(),
            phase: InstancePhase::Booting,
            state: "Booting".into(),
            logic_frame: 0,
            rendered_frames: 0,
            render_fps: 0.0,
            sim_rate: 0.0,
            frame_ms: 0.0,
            cpu_ms: 0.0,
            boot_secs: 0.0,
            commands: Vec::new(),
            verdict: Verdict::Pending,
        }
    }

    /// Phase name for labels.
    pub fn phase_name(&self) -> &str {
        match &self.phase {
            InstancePhase::Booting => "booting",
            InstancePhase::Running => "running",
            InstancePhase::Failed(_) => "FAILED",
            InstancePhase::Finished => "finished",
        }
    }

    pub fn one_line(&self) -> String {
        let mut line = format!(
            "{} state={} frame={} present={:.1}fps sim={:.1}f/s cost={:.1}ms cpu={:.1}ms boot={:.1}s presents={}",
            self.phase_name(),
            self.state,
            self.logic_frame,
            self.render_fps,
            self.sim_rate,
            self.frame_ms,
            self.cpu_ms,
            self.boot_secs,
            self.rendered_frames
        );
        if let InstancePhase::Failed(reason) = &self.phase {
            line.push_str(&format!(" error={reason}"));
        }
        if self.verdict.is_decided() {
            line.push_str(&format!(" verdict={} {}", self.verdict.name(), self.verdict.detail()));
        }
        line
    }
}

/// Limits how many instances run their (I/O and memory heavy) boot at once.
pub struct BootGate {
    free: Mutex<usize>,
    released: Condvar,
}

pub struct BootPermit<'a>(&'a BootGate);

impl BootGate {
    pub fn new(permits: usize) -> Self {
        Self {
            free: Mutex::new(permits),
            released: Condvar::new(),
        }
    }

    fn acquire(&self, stop: &AtomicBool) -> Option<BootPermit<'_>> {
        let mut free = self.free.lock().unwrap_or_else(|e| e.into_inner());
        loop {
            if stop.load(Ordering::Relaxed) {
                return None;
            }
            if *free > 0 {
                *free -= 1;
                return Some(BootPermit(self));
            }
            free = self
                .released
                .wait_timeout(free, Duration::from_millis(100))
                .unwrap_or_else(|e| e.into_inner())
                .0;
        }
    }
}

impl Drop for BootPermit<'_> {
    fn drop(&mut self) {
        *self.0.free.lock().unwrap_or_else(|e| e.into_inner()) += 1;
        self.0.released.notify_one();
    }
}

/// Everything an instance thread needs; moved onto the thread.
pub struct InstanceSpawn {
    pub index: usize,
    pub scenario: InstanceScenario,
    pub window: Arc<Window>,
    pub gpu: ww3d_engine::HostGpu,
    pub tile: wgpu::Texture,
    pub userdata: PathBuf,
    pub extra_args: Vec<String>,
    pub present_interval: Duration,
    /// Host update cadence (one update per interval); `None` runs updates
    /// back to back to measure an instance's capacity.
    pub update_interval: Option<Duration>,
    pub runtime: Option<tokio::runtime::Handle>,
    pub boot_gate: Arc<BootGate>,
    /// Read-only retail content every instance of this host shares.
    pub shared_retail: crate::assets::ww3d_asset_manager::SharedRetailContent,
    /// Runtime-host control/status files this instance serves (the bridge a
    /// spawned `generals -runtime_host=headless` would serve), for attached
    /// smoke drivers. `None` = no bridge.
    pub bridge: Option<InstanceBridgePaths>,
}

/// Where an instance's runtime-host bridge files live.
#[derive(Debug, Clone)]
pub struct InstanceBridgePaths {
    pub control: PathBuf,
    pub status: PathBuf,
    pub frame: PathBuf,
    /// Frame-loop outcome for attached drivers: 0 clean, 1 failed.
    pub exit: Arc<Mutex<Option<i32>>>,
}

impl InstanceBridgePaths {
    pub fn in_dir(dir: &std::path::Path) -> Self {
        Self {
            control: dir.join("control.txt"),
            status: dir.join("status.txt"),
            frame: dir.join("frame.png"),
            exit: Arc::new(Mutex::new(None)),
        }
    }
}

struct Shared {
    status: Mutex<InstanceStatus>,
    stop: AtomicBool,
}

impl Shared {
    fn update(&self, f: impl FnOnce(&mut InstanceStatus)) {
        f(&mut self.status.lock().unwrap_or_else(|e| e.into_inner()));
    }
}

/// Host-side handle of a running instance.
pub struct InstanceHandle {
    index: usize,
    scenario: InstanceScenario,
    shared: Arc<Shared>,
    thread: Option<JoinHandle<()>>,
    boot_admission: Option<Receiver<()>>,
    /// Attached gate driver of this tile, if any.
    driver: Option<JoinHandle<()>>,
}

impl InstanceHandle {
    pub fn spawn(spawn: InstanceSpawn) -> std::io::Result<Self> {
        let index = spawn.index;
        let scenario = spawn.scenario.clone();
        let shared = Arc::new(Shared {
            status: Mutex::new(InstanceStatus::new(index, &scenario)),
            stop: AtomicBool::new(false),
        });
        let thread_shared = Arc::clone(&shared);
        let exit_cell = spawn.bridge.as_ref().map(|b| Arc::clone(&b.exit));
        let (boot_admitted, boot_admission) = mpsc::channel();
        let thread = std::thread::Builder::new()
            .name(format!("generals-instance-{index}"))
            .stack_size(INSTANCE_THREAD_STACK)
            .spawn(move || {
                let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    run_instance(spawn, &thread_shared, boot_admitted)
                }));
                let phase = match outcome {
                    Ok(Ok(())) => InstancePhase::Finished,
                    Ok(Err(reason)) => InstancePhase::Failed(reason),
                    Err(payload) => InstancePhase::Failed(format!(
                        "panic: {}",
                        payload
                            .downcast_ref::<String>()
                            .cloned()
                            .or_else(|| payload.downcast_ref::<&str>().map(|s| (*s).to_string()))
                            .unwrap_or_else(|| "<non-string panic>".into())
                    )),
                };
                if let InstancePhase::Failed(reason) = &phase {
                    log::error!("instance {index} failed: {reason}");
                }
                if let Some(cell) = exit_cell {
                    let code = if matches!(phase, InstancePhase::Finished) { 0 } else { 1 };
                    *cell.lock().unwrap_or_else(|e| e.into_inner()) = Some(code);
                }
                thread_shared.update(|s| s.phase = phase);
            })?;
        Ok(Self {
            index,
            scenario,
            shared,
            thread: Some(thread),
            boot_admission: Some(boot_admission),
            driver: None,
        })
    }

    /// Run `drive` on its own thread after this engine obtains a boot slot.
    /// A queued tile's gate timeout starts when its own boot can begin.
    pub fn attach_driver(
        &mut self,
        drive: impl FnOnce() -> Verdict + Send + 'static,
    ) -> std::io::Result<()> {
        let shared = Arc::clone(&self.shared);
        let index = self.index;
        let boot_admission = self
            .boot_admission
            .take()
            .ok_or_else(|| std::io::Error::other("instance already has an attached driver"))?;
        self.driver = Some(
            std::thread::Builder::new()
                .name(format!("generals-instance-{index}-driver"))
                .spawn(move || {
                    let verdict = if boot_admission.recv().is_ok() {
                        drive()
                    } else {
                        Verdict::Fail("instance stopped before its boot slot was available".into())
                    };
                    log::info!("instance {index} verdict: {} {}", verdict.name(), verdict.detail());
                    shared.update(|s| s.verdict = verdict);
                })?,
        );
        Ok(())
    }

    /// Whether this tile's verdict comes from an attached driver.
    pub fn has_driver(&self) -> bool {
        self.driver.is_some()
    }

    /// Record a host-assessed verdict (scenarios without a driver).
    pub fn set_verdict(&self, verdict: Verdict) {
        self.shared.update(|s| s.verdict = verdict);
    }

    pub fn index(&self) -> usize {
        self.index
    }

    pub fn scenario(&self) -> &InstanceScenario {
        &self.scenario
    }

    pub fn status(&self) -> InstanceStatus {
        self.shared
            .status
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone()
    }

    pub fn request_stop(&self) {
        self.shared.stop.store(true, Ordering::Relaxed);
    }

    pub fn is_finished(&self) -> bool {
        self.thread.as_ref().is_none_or(JoinHandle::is_finished)
            && self.driver.as_ref().is_none_or(JoinHandle::is_finished)
    }

    pub fn join(mut self) -> InstanceStatus {
        self.request_stop();
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
        // The driver sees the instance exit (its exit cell) and returns.
        if let Some(driver) = self.driver.take() {
            let _ = driver.join();
        }
        self.status()
    }
}

pub(super) fn retail_assets_dir() -> Option<PathBuf> {
    ["GENERALS_ASSETS_DIR", "GENERALS_RETAIL_DIR"]
        .into_iter()
        .filter_map(std::env::var_os)
        .map(PathBuf::from)
        .find(|root| root.is_dir())
}

/// Engine command line of one instance: the headless runtime host the
/// golden capture uses, with a private user-data root.
fn instance_argv(spawn: &InstanceSpawn) -> Vec<String> {
    let size = spawn.tile.size();
    let (w, h) = (size.width, size.height);
    let mut argv = vec![
        "generals".to_string(),
        "-runtime_host=headless".to_string(),
        "-runtime_host_private_profile=1".to_string(),
        "-windowed".to_string(),
        format!("-width={w}"),
        format!("-height={h}"),
        "-nologo".to_string(),
        "-nointro".to_string(),
        format!("-userdata={}", spawn.userdata.display()),
    ];
    if let Some(bridge) = &spawn.bridge {
        argv.push(format!("-gpui_control={}", bridge.control.display()));
        argv.push(format!("-gpui_status={}", bridge.status.display()));
        argv.push(format!("-gpui_frame={}", bridge.frame.display()));
    }
    argv.extend(spawn.extra_args.iter().cloned());
    argv
}

fn run_instance(
    spawn: InstanceSpawn,
    shared: &Shared,
    boot_admitted: Sender<()>,
) -> Result<(), String> {
    let _runtime = spawn.runtime.as_ref().map(tokio::runtime::Handle::enter);
    // This thread's own service bags; nothing published here is visible to
    // any other instance (docs/instance_binding.md, rule 2).
    let _main_services = crate::main_services::MainServicesScope::new();
    let _common_services = game_engine::common::common_services::CommonServicesScope::new();
    // Hand this application the host's shared read-only retail content
    // before anything loads (an explicit handle, not process state).
    let shared_retail = spawn.shared_retail.clone();
    game_engine::common::common_services::common_services()
        .application
        .extension(move || shared_retail);
    crate::localization::init("English");
    register_real_game_client_bootstrap();

    std::fs::create_dir_all(&spawn.userdata)
        .map_err(|e| format!("userdata {}: {e}", spawn.userdata.display()))?;
    let mut args = crate::command_line::CommandLineArgs::parse_from_args(instance_argv(&spawn))
        .map_err(|e| format!("command line: {e}"))?;
    // The grid drives the native WND shell on each receiving engine.
    args.enable_runtime_host_wnd();
    *crate::main_services::main_services()
        .command_line_args
        .lock()
        .unwrap_or_else(|e| e.into_inner()) = Some(args.clone());
    let mut bridge = RuntimeHostBridge::from_command_line(&args);
    if let Some(bridge) = bridge.as_mut() {
        bridge.publish_booting();
    }
    let args = Arc::new(args);

    let Some(permit) = spawn.boot_gate.acquire(&shared.stop) else {
        return Ok(());
    };
    let boot_started = Instant::now();
    let _ = boot_admitted.send(());
    drop(boot_admitted);
    let window = Arc::clone(&spawn.window);
    let gpu = spawn.gpu.clone();
    let (mut engine, mut services) = pollster::block_on(async move {
        let startup_sources = CnCGameEngine::load_startup_game_data_texts(None)
            .await
            .map_err(anyhow::Error::msg)?;
        let paths = crate::config::startup_paths::StartupUserData::admit(
            &startup_sources
                .iter()
                .map(|(_, text)| text.clone())
                .collect::<Vec<_>>(),
            &args,
        )?;
        let mut services = crate::subsystem_manager::ApplicationServices::new();
        let engine = CnCGameEngine::new_for_instance(
            window,
            args,
            startup_sources,
            paths,
            &mut services,
            gpu,
        )
        .await?;
        anyhow::Ok((Box::new(engine), services))
    })
    .map_err(|e| format!("engine boot: {e:#}"))?;

    // One logic frame per host update; the update cadence sets the sim rate.
    engine.fixed_logic_step = Some(LOGIC_FRAME);
    let result = drive_instance(&mut engine, &spawn, shared, boot_started, permit, bridge);
    drop(engine);
    if let Err(error) = services.shutdown() {
        log::warn!(
            "instance {}: application services shutdown: {error}",
            spawn.index
        );
    }
    result
}

fn drive_instance(
    engine: &mut CnCGameEngine,
    spawn: &InstanceSpawn,
    shared: &Shared,
    boot_started: Instant,
    permit: BootPermit<'_>,
    mut bridge: Option<RuntimeHostBridge>,
) -> Result<(), String> {
    // Startup asset worker: drained on the owning thread, as the run loop does.
    while matches!(
        engine.startup_load_state,
        StartupLoadState::InProgress { .. }
    ) {
        if shared.stop.load(Ordering::Relaxed) {
            return Ok(());
        }
        engine
            .update_startup_loading()
            .map_err(|e| format!("startup load: {e}"))?;
        if let Some(bridge) = bridge.as_mut() {
            bridge.publish_booting();
        }
        std::thread::sleep(STARTUP_POLL_INTERVAL);
    }
    drop(permit);
    let boot_secs = boot_started.elapsed().as_secs_f32();
    shared.update(|s| {
        s.phase = InstancePhase::Running;
        s.boot_secs = boot_secs;
    });
    log::info!("instance {} booted in {boot_secs:.1}s", spawn.index);

    let assets = retail_assets_dir();
    let mut commands_sent = false;
    let mut last_drive_frame_start = None;
    let mut next_present = Instant::now();
    let mut rendered: u64 = 0;
    let mut window_started = Instant::now();
    let mut window_logic_start = engine.presentation_or_boot_logic_frame();
    let mut window_presents = 0u32;
    let mut window_frames = 0u32;
    let mut window_cost = Duration::ZERO;
    let mut window_cpu = Duration::ZERO;

    while !shared.stop.load(Ordering::Relaxed) {
        let tick_started = Instant::now();
        let tick_cpu_started = thread_cpu_time();
        if !commands_sent && matches!(engine.get_state(), GameState::Menu) {
            commands_sent = true;
            let commands = spawn.scenario.menu_commands(assets.as_deref());
            for command in &commands {
                log::info!("instance {}: {command}", spawn.index);
                engine.apply_runtime_host_command(command)?;
            }
            shared.update(|s| s.commands = commands.clone());
            window_logic_start = engine.presentation_or_boot_logic_frame();
        }

        // Runtime-host bridge, as the run loop serves it: apply queued
        // commands and publish status after each one.
        if let Some(bridge) = bridge.as_mut() {
            for command in bridge.drain_commands() {
                log::info!("instance {}: runtime_host {command}", spawn.index);
                engine.apply_runtime_host_command(&command)?;
                let snapshot = engine.runtime_host_status_snapshot();
                bridge.publish_status(&snapshot);
            }
            if engine.take_runtime_host_pending_capture() {
                bridge.force_capture_request(&engine.ww3d);
            }
        }
        if engine.is_state_change_pending(GameState::Exiting) {
            break;
        }

        // C++ draws once per engine frame; frames the grid will not draw skip
        // the presentation build (bounded, see `PresentationDefer`).
        let present_due = Instant::now() >= next_present;
        engine.set_next_frame_unseen(!present_due);
        engine.drive_frame_update(&mut last_drive_frame_start, &mut |_, _| Ok(()))?;
        let now = Instant::now();
        if present_due && engine.should_present_w3d_frame() {
            next_present = now + spawn.present_interval;
            engine.render().map_err(|e| format!("render: {e:#}"))?;
            copy_frame_to_tile(engine, &spawn.gpu, &spawn.tile);
            rendered += 1;
            window_presents += 1;
        }
        if let Some(bridge) = bridge.as_mut() {
            let snapshot = engine.runtime_host_status_snapshot();
            bridge.publish_runtime(&engine.ww3d, &snapshot);
        }
        window_frames += 1;
        window_cost += tick_started.elapsed();
        window_cpu += thread_cpu_time().saturating_sub(tick_cpu_started);
        if engine.is_quitting() {
            break;
        }

        let elapsed = window_started.elapsed();
        if elapsed >= Duration::from_secs(1) {
            let logic = engine.presentation_or_boot_logic_frame();
            let secs = elapsed.as_secs_f32();
            let state = format!("{:?}", engine.get_state());
            let sim = logic.saturating_sub(window_logic_start) as f32 / secs;
            let fps = window_presents as f32 / secs;
            let cost = window_cost.as_secs_f32() * 1000.0 / window_frames.max(1) as f32;
            let cpu = window_cpu.as_secs_f32() * 1000.0 / window_frames.max(1) as f32;
            shared.update(|s| {
                s.state = state;
                s.logic_frame = logic;
                s.rendered_frames = rendered;
                s.render_fps = fps;
                s.sim_rate = sim;
                s.frame_ms = cost;
                s.cpu_ms = cpu;
            });
            window_started = Instant::now();
            window_logic_start = logic;
            window_presents = 0;
            window_frames = 0;
            window_cost = Duration::ZERO;
            window_cpu = Duration::ZERO;
        }

        // Host cadence: one update per interval (MultiInstanceOptions::update_hz).
        let spent = tick_started.elapsed();
        if let Some(interval) = spawn.update_interval {
            if spent < interval {
                std::thread::sleep(interval - spent);
            }
        }
    }
    Ok(())
}

/// CPU time consumed so far by the calling thread.
fn thread_cpu_time() -> Duration {
    #[cfg(any(target_os = "macos", target_os = "linux"))]
    {
        use rustix::time::{ClockId, DynamicClockId, clock_gettime_dynamic};

        clock_gettime_dynamic(DynamicClockId::Known(ClockId::ThreadCPUTime))
            .map(|ts| Duration::new(ts.tv_sec.max(0) as u64, ts.tv_nsec.clamp(0, 999_999_999) as u32))
            .unwrap_or_default()
    }
    #[cfg(not(any(target_os = "macos", target_os = "linux")))]
    {
        Duration::ZERO
    }
}

/// Copy the engine's finished off-screen frame into its grid tile. Both live
/// on the host device, so this is a GPU-side copy ordered on the shared queue
/// after the frame's own submission.
fn copy_frame_to_tile(engine: &CnCGameEngine, gpu: &ww3d_engine::HostGpu, tile: &wgpu::Texture) {
    let ww3d = engine.ww3d.lock();
    let Some(color) = ww3d.headless_color_texture() else {
        return;
    };
    if color.format() != tile.format() {
        return;
    }
    let extent = wgpu::Extent3d {
        width: color.width().min(tile.width()),
        height: color.height().min(tile.height()),
        depth_or_array_layers: 1,
    };
    let mut encoder = gpu
        .device
        .create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("multi-instance tile copy"),
        });
    encoder.copy_texture_to_texture(color.as_image_copy(), tile.as_image_copy(), extent);
    gpu.queue.submit([encoder.finish()]);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn attached_gate_waits_for_its_receiving_boot_slot() {
        let (admit, boot_admission) = mpsc::channel();
        let (gate_entered, observe_gate) = mpsc::channel();
        let scenario = InstanceScenario::Shell;
        let mut instance = InstanceHandle {
            index: 0,
            shared: Arc::new(Shared {
                status: Mutex::new(InstanceStatus::new(0, &scenario)),
                stop: AtomicBool::new(false),
            }),
            scenario,
            thread: None,
            boot_admission: Some(boot_admission),
            driver: None,
        };
        instance
            .attach_driver(move || {
                gate_entered.send(()).unwrap();
                Verdict::Pass("attached gate ran".into())
            })
            .unwrap();

        assert_eq!(
            observe_gate.recv_timeout(Duration::from_millis(25)),
            Err(mpsc::RecvTimeoutError::Timeout)
        );
        admit.send(()).unwrap();
        assert_eq!(instance.join().verdict, Verdict::Pass("attached gate ran".into()));
        assert_eq!(observe_gate.try_recv(), Ok(()));
    }
}
