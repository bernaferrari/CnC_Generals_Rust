#![allow(unused_imports, unused_variables, dead_code, non_snake_case)]
use super::*;

#[derive(Debug, Clone, Copy)]
pub(super) enum SoundType {
    Select,
    Command,
    ConstructionComplete,
    UnitReady,
    UpgradeComplete,
    Hit,
    Explosion,
    Build,
}

pub(super) struct NoopWake;

impl Wake for NoopWake {
    fn wake(self: Arc<Self>) {}
}

/// C++ `GameEngine.h:13` `DEFAULT_MAX_FPS` — execute present cap, not logic Hz.
pub(super) const DEFAULT_MAX_FPS: u32 = 45;
/// Windowed GPU present interval: 1/45 s (~22_222 µs).
/// C++ `GameEngine::execute` (`GameEngine.cpp:856-857`) uses
/// `DWORD limit = (1000.0f/m_maxFPS)-1` after `m_maxFPS = DEFAULT_MAX_FPS`
/// (`GameEngine.cpp:271`). This is the windowed present/execute cap only;
/// logic stays 30 Hz (`HEADLESS_LOGIC_INTERVAL` / `LOGICFRAMES_PER_SECOND`).
pub(super) const FRAME_INTERVAL: Duration =
    Duration::from_micros(1_000_000 / DEFAULT_MAX_FPS as u64);
/// Headless logic residual: ~30 Hz fixed step without waiting on GPU present.
pub(super) const HEADLESS_LOGIC_INTERVAL: Duration = Duration::from_nanos(33_333_333);
/// C++ `TheW3DFrameLengthInMsec` / `W3D_FRAME_LENGTH_MS`.
pub(super) const W3D_FRAME_LENGTH_MS: u32 = 33;
/// C++ `W3DDisplay::draw` `minTime = 30` present cap (busy-wait `< minTime-1`).
pub(super) const W3D_DRAW_MIN_TIME_MS: u32 = 30;

fn ww3d_sync_ms() -> &'static std::sync::atomic::AtomicU32 {
    static SYNC: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
    &SYNC
}

fn last_ww3d_client_frame() -> &'static std::sync::atomic::AtomicU32 {
    static LAST: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(u32::MAX);
    &LAST
}

fn time_multiplier_counter() -> &'static std::sync::atomic::AtomicI32 {
    static COUNTER: std::sync::atomic::AtomicI32 = std::sync::atomic::AtomicI32::new(1);
    &COUNTER
}

struct AverageFpsTracker {
    history: [f64; 30],
    offset: usize,
    samples: usize,
    average: f32,
    last: Option<Instant>,
}

impl AverageFpsTracker {
    const fn new() -> Self {
        Self {
            history: [0.0; 30],
            offset: 0,
            samples: 0,
            average: 30.0,
            last: None,
        }
    }

    fn note_frame(&mut self) -> f32 {
        const MAX_FRAME_TIME_CUTOFF: f64 = 0.5;
        let now = Instant::now();
        let elapsed = match self.last {
            Some(prev) => now.saturating_duration_since(prev).as_secs_f64(),
            None => 1.0 / 30.0,
        };
        self.last = Some(now);
        if elapsed > 0.0 && elapsed <= MAX_FRAME_TIME_CUTOFF {
            self.history[self.offset] = 1.0 / elapsed;
            self.offset = (self.offset + 1) % 30;
            self.samples = (self.samples + 1).min(30);
        }
        if self.samples > 0 {
            let sum: f64 = self.history.iter().take(self.samples).sum();
            self.average = (sum / self.samples as f64) as f32;
        }
        self.average
    }
}

fn average_fps_tracker() -> std::sync::MutexGuard<'static, AverageFpsTracker> {
    static TRACKER: std::sync::LazyLock<std::sync::Mutex<AverageFpsTracker>> =
        std::sync::LazyLock::new(|| std::sync::Mutex::new(AverageFpsTracker::new()));
    TRACKER.lock().unwrap_or_else(|e| e.into_inner())
}

/// Run the actual C&C game
pub async fn run_cnc_game(
    event_loop: EventLoop<()>,
    window_attributes: WindowAttributes,
    cmd_args: Arc<CommandLineArgs>,
) -> Result<()> {
    info!("🎮 Starting Command & Conquer Generals Zero Hour - Real Game");

    register_real_game_client_bootstrap();

    let mut pending_window_attributes = Some(window_attributes);
    let mut window: Option<Arc<Window>> = None;
    let mut pending_engine_window: Option<Arc<Window>> = None;
    let mut engine_init_future: Option<Pin<Box<dyn Future<Output = Result<CnCGameEngine>>>>> = None;
    let mut engine_init_started_at: Option<Instant> = None;
    let mut engine_init_last_log_at: Option<Instant> = None;
    let mut engine: Option<CnCGameEngine> = None;
    let mut shutdown_logged = false;
    let mut next_redraw_at = Instant::now();
    let mut last_slow_frame_log = None::<Instant>;
    let mut slow_frame_count = 0u32;
    let mut slow_frame_peak = Duration::ZERO;
    let mut slow_ww3d_peak = Duration::ZERO;
    let mut slow_update_peak = Duration::ZERO;
    let mut slow_render_peak = Duration::ZERO;
    let mut last_render_health_log = Instant::now();
    /// Headless present residual: keep live_frame/screenshot alive, but far below logic rate.
    const HEADLESS_PRESENT_INTERVAL: Duration = Duration::from_millis(250);
    const STARTUP_POLL_INTERVAL: Duration = Duration::from_millis(5);
    const MINIMIZED_POLL_INTERVAL: Duration = Duration::from_millis(5);
    let runtime_headless_mode = RuntimeHostBridge::is_headless_mode(cmd_args.as_ref());
    let mut runtime_host_bridge = RuntimeHostBridge::from_command_line(cmd_args.as_ref());
    if let Some(bridge) = runtime_host_bridge.as_mut() {
        bridge.publish_booting();
    }
    let mut runtime_window_minimized = false;
    let mut next_headless_present_at = Instant::now();

    #[cfg(feature = "integration-diagnostics")]
    let mut integration_bridge: Option<IntegrationTelemetryBridge> = None;
    #[cfg(feature = "integration-diagnostics")]
    let runtime_handle = tokio::runtime::Handle::current();

    #[allow(deprecated)]
    event_loop.run(move |event, elwt| {
        elwt.set_control_flow(ControlFlow::WaitUntil(next_redraw_at));

        let mut drive_frame = |
            engine: &mut CnCGameEngine,
            current_window: &Arc<Window>,
            runtime_host_bridge: &mut Option<RuntimeHostBridge>,
            render_frame: bool,
        | {
            if let Some(bridge) = runtime_host_bridge.as_mut() {
                let mut applied = 0u32;
                for command in bridge.drain_commands() {
                    let cmd_label = command.clone();
                    engine.apply_runtime_host_command(&command);
                    applied = applied.saturating_add(1);
                    log::info!("runtime_host command applied: {cmd_label}");
                    // Publish after *each* command so menu_click + match_started
                    // land in status.txt before a later SIGSEGV.
                    let snapshot = engine.runtime_host_status_snapshot();
                    bridge.publish_status(&snapshot);
                }
                if engine.take_runtime_host_pending_capture() {
                    bridge.force_capture_request();
                }
                let _ = applied;
            }

            let frame_started = Instant::now();
            let mut ww3d_elapsed = Duration::ZERO;
            let frame_timing = if matches!(
                engine.get_state(),
                GameState::Loading | GameState::Menu | GameState::InGame | GameState::Paused
            ) {
                let ww3d_started = Instant::now();
                if matches!(engine.get_state(), GameState::Menu) {
                    log::trace!("drive_frame: pre ww3d_engine::update Menu");
                }
                let timing = match ww3d_engine::update() {
                    Ok(_) => match ww3d_engine::timing() {
                        Ok(timing) => {
                            let sync_ms = engine.advance_ww3d_visual_sync();
                            WW3D::sync(sync_ms);
                            Some(timing)
                        }
                        Err(err) => {
                            error!("Failed to fetch WW3D frame timing: {err:?}");
                            None
                        }
                    },
                    Err(err) => {
                        error!("WW3D engine update failed: {err:?}");
                        None
                    }
                };
                ww3d_elapsed = ww3d_started.elapsed();
                timing
            } else {
                None
            };

            let update_started = Instant::now();
            // C++ GameEngine::update (GameEngine.cpp:732-752):
            // VERIFY_CRC → TheRadar->UPDATE() → TheAudio->UPDATE() →
            // TheGameClient->UPDATE() → propagateMessages → … →
            // TheGameLogic->UPDATE().
            engine.host_update_the_radar();
            engine.host_update_the_audio();
            if let Some(timing) = frame_timing {
                #[cfg(feature = "integration-diagnostics")]
                if let Some(bridge) = integration_bridge.as_mut() {
                    if let Err(err) = runtime_handle.block_on(bridge.pump_with_timing(engine, timing))
                    {
                        error!(
                            "Integration telemetry pump failed: {err:?}. Disabling bridge."
                        );
                        integration_bridge = None;
                    }
                }
                engine.update_with_timing(&timing);
            } else {
                engine.update_with_frame_clock();
            }

            let update_elapsed = update_started.elapsed();
            static DRIVE_FRAME_COUNT: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
            let dfn = DRIVE_FRAME_COUNT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            if dfn < 15 || (dfn < 50 && matches!(engine.get_state(), GameState::Menu)) {
                info!("drive_frame #{} update_done {:?} state={:?} render_frame={}", dfn, update_elapsed, engine.get_state(), render_frame);
            }

            let render_started = Instant::now();
            let render_frame = render_frame && engine.should_present_w3d_frame();
            if render_frame {
                if dfn < 15 || (dfn < 50 && matches!(engine.get_state(), GameState::Menu)) {
                    info!("drive_frame #{} calling render()", dfn);
                }
                match engine.render() {
                    Ok(_) => {
                        // Honest live_frame residual: a successful windowed render
                        // presented a wgpu surface (end_render present_surface_texture).
                        // Headless keeps capture-only promotion so headless smoke
                        // cannot forge playable_claim via live_frame_ok alone.
                        if !runtime_headless_mode {
                            if let Some(bridge) = runtime_host_bridge.as_mut() {
                                bridge.note_windowed_surface_presented();
                            }
                        }
                    }
                    Err(e) => {
                        error!("❌ RENDER ERROR: {:?}", e);
                        if let Some(source_err) = e.source() {
                            if let Some(surface_err) =
                                source_err.downcast_ref::<wgpu::SurfaceError>()
                            {
                                match surface_err {
                                    wgpu::SurfaceError::Lost => {
                                        error!("🔄 SURFACE LOST: Attempting resize");
                                        engine.resize(current_window.inner_size());
                                    }
                                    wgpu::SurfaceError::OutOfMemory => {
                                        error!("💥 OUT OF MEMORY: Exiting");
                                        elwt.exit();
                                    }
                                    _ => {
                                        error!("🚨 Other surface error: {:?}", surface_err);
                                    }
                                }
                            } else {
                                error!("🚨 Non-surface error: {:?}", source_err);
                            }
                        } else {
                            error!("🚨 No source error available");
                        }
                    }
                }
            }
            let render_elapsed = render_started.elapsed();

            let frame_elapsed = frame_started.elapsed();
            if frame_elapsed >= Duration::from_millis(120) {
                slow_frame_count = slow_frame_count.saturating_add(1);
                slow_frame_peak = slow_frame_peak.max(frame_elapsed);
                slow_ww3d_peak = slow_ww3d_peak.max(ww3d_elapsed);
                slow_update_peak = slow_update_peak.max(update_elapsed);
                slow_render_peak = slow_render_peak.max(render_elapsed);
            }
            if frame_elapsed >= Duration::from_millis(300) {
                let should_log = last_slow_frame_log
                    .map(|last| frame_started.duration_since(last) >= Duration::from_secs(1))
                    .unwrap_or(true);
                if should_log {
                    warn!(
                        "Severe slow frame {:?} in {:?} (ww3d={:?}, update={:?}, render={:?}, startup_progress={:.0}%)",
                        frame_elapsed,
                        engine.get_state(),
                        ww3d_elapsed,
                        update_elapsed,
                        render_elapsed,
                        engine.startup_last_reported_progress * 100.0
                    );
                    last_slow_frame_log = Some(frame_started);
                }
            }
            if frame_started.duration_since(last_render_health_log) >= Duration::from_secs(5) {
                if slow_frame_count == 0 {
                    info!(
                        "Render health: ok (state={:?}, render_items={}, no slow frames >120ms in last 5s, startup_progress={:.0}%)",
                        engine.get_state(),
                        engine.render_pipeline.debug_render_item_count(),
                        engine.startup_last_reported_progress * 100.0
                    );
                } else {
                    info!(
                        "Render health: slow_frames={} peak={:?} (ww3d_peak={:?}, update_peak={:?}, render_peak={:?}, state={:?}, render_items={}, startup_progress={:.0}%)",
                        slow_frame_count,
                        slow_frame_peak,
                        slow_ww3d_peak,
                        slow_update_peak,
                        slow_render_peak,
                        engine.get_state(),
                        engine.render_pipeline.debug_render_item_count(),
                        engine.startup_last_reported_progress * 100.0
                    );
                }
                slow_frame_count = 0;
                slow_frame_peak = Duration::ZERO;
                slow_ww3d_peak = Duration::ZERO;
                slow_update_peak = Duration::ZERO;
                slow_render_peak = Duration::ZERO;
                last_render_health_log = frame_started;
            }

            if should_exit_for_smoke_test(
                cmd_args.wants_smoke_test(),
                engine.get_state(),
                engine.startup_last_reported_progress,
                engine.is_state_change_pending(GameState::Exiting),
            ) {
                info!("Smoke test reached main menu; exiting successfully");
                engine.transition_to_state(GameState::Exiting);
                elwt.exit();
                return;
            }

            if let Some(bridge) = runtime_host_bridge.as_mut() {
                let snapshot = engine.runtime_host_status_snapshot();
                bridge.publish_runtime(&snapshot);
            }
        };

        if matches!(event, Event::Resumed) && engine.is_none() {
            let Some(attributes) = pending_window_attributes.take() else {
                error!("Missing window attributes during startup resume");
                elwt.exit();
                return;
            };

            let created_window = match elwt.create_window(attributes) {
                Ok(window) => Arc::new(window),
                Err(err) => {
                    error!("Failed to create window: {err}");
                    elwt.exit();
                    return;
                }
            };

            info!(
                "Window created: {}x{} ({})",
                created_window.inner_size().width,
                created_window.inner_size().height,
                if created_window.fullscreen().is_some() {
                    "Fullscreen"
                } else {
                    "Windowed"
                }
            );

            let window_visible =
                apply_runtime_host_window_visibility(&created_window, runtime_headless_mode);
            if !runtime_headless_mode && !window_visible {
                warn!(
                    "Windowed set_visible(true) but winit is_visible={:?}; window_visible stays false",
                    created_window.is_visible()
                );
            }
            if let Some(bridge) = runtime_host_bridge.as_mut() {
                bridge.publish_booting_from_winit_query(
                    runtime_headless_mode,
                    created_window.is_visible(),
                );
            }
            created_window.request_redraw();
            window = Some(created_window.clone());
            pending_engine_window = Some(created_window);
            return;
        }

        if engine.is_none() {
            match event {
                Event::WindowEvent { ref event, window_id } => {
                    if let Some(current_window) = window.as_ref() {
                        if window_id == current_window.id()
                            && matches!(event, WindowEvent::CloseRequested)
                        {
                            info!("Close requested before engine startup completed");
                            elwt.exit();
                            return;
                        }
                    }
                }
                Event::AboutToWait => {
                    if let Some(bridge) = runtime_host_bridge.as_mut() {
                        let is_visible = match window.as_ref() {
                            Some(current) => current.is_visible(),
                            None => Some(false),
                        };
                        bridge.publish_booting_from_winit_query(
                            runtime_headless_mode,
                            is_visible,
                        );
                        for command in bridge.drain_commands() {
                            if command.trim().eq_ignore_ascii_case("exit") {
                                info!("Runtime host received exit command during startup");
                                elwt.exit();
                                return;
                            }
                        }
                    }

                    if engine_init_future.is_none() {
                        if let Some(created_window) = pending_engine_window.take() {
                            #[cfg(target_os = "windows")]
                            {
                                use raw_window_handle::HasWindowHandle;
                                if let Ok(handle) = created_window.window_handle() {
                                    if let raw_window_handle::RawWindowHandle::Win32(win) =
                                        handle.as_raw()
                                    {
                                        crate::win_main::APPLICATION_WINDOW.store(
                                            win.hwnd.get() as *mut std::ffi::c_void,
                                            std::sync::atomic::Ordering::Relaxed,
                                        );
                                        debug!("Win32 window handle stored");
                                    }
                                }
                            }

                            engine_init_started_at = Some(Instant::now());
                            engine_init_last_log_at = None;
                            created_window
                                .set_title("Command & Conquer Generals Zero Hour - Initializing");
                            engine_init_future = Some(Box::pin(CnCGameEngine::new(
                                created_window.clone(),
                                cmd_args.clone(),
                            )));
                        }
                    }

                    if let Some(init_future) = engine_init_future.as_mut() {
                        let waker: Waker = Waker::from(Arc::new(NoopWake));
                        let mut cx = Context::from_waker(&waker);
                        match init_future.as_mut().poll(&mut cx) {
                            Poll::Ready(Ok(new_engine)) => {
                                if let Some(created_window) = window.as_ref() {
                                    info!("C&C Game engine initialized successfully!");
                                    if runtime_headless_mode {
                                        created_window.set_visible(false);
                                    } else {
                                        created_window.set_visible(true);
                                        created_window.focus_window();
                                        apply_runtime_host_window_placement(created_window);
                                        if !apply_runtime_host_window_visibility(
                                            created_window,
                                            false,
                                        ) {
                                            warn!(
                                                "Windowed engine init complete but winit is_visible={:?}; window_visible stays false",
                                                created_window.is_visible()
                                            );
                                        }
                                    }
                                    created_window.request_redraw();
                                }
                                engine_init_future = None;
                                engine_init_started_at = None;
                                engine_init_last_log_at = None;
                                let mut new_engine = new_engine;
                                if let Some(bridge) = runtime_host_bridge.as_mut() {
                                    let snapshot = new_engine.runtime_host_status_snapshot();
                                    bridge.publish_runtime(&snapshot);
                                }
                                engine = Some(new_engine);
                                // The engine was constructed inside the boxed
                                // boot future; the live-snapshot slot still
                                // holds the pre-move raw address (freed heap
                                // once the box drops). Re-point it at the
                                // final home BEFORE any frame can run —
                                // otherwise the first
                                // `with_live_game_client_mut` (InGame
                                // particle FX pose resolve) faults.
                                if let Some(placed) = engine.as_mut() {
                                    placed.game_client.republish_live_slot_after_engine_move();
                                }
                                #[cfg(feature = "integration-diagnostics")]
                                if cmd_args.wants_integration_diagnostics() {
                                    match pollster::block_on(IntegrationTelemetryBridge::new(
                                        IntegrationConfig::default(),
                                    )) {
                                        Ok(bridge) => {
                                            info!("Integration diagnostics bridge initialized");
                                            integration_bridge = Some(bridge);
                                        }
                                        Err(err) => {
                                            error!(
                                                "Failed to initialize integration diagnostics bridge: {err:?}. Continuing without telemetry overlay."
                                            );
                                        }
                                    }
                                }
                            }
                            Poll::Ready(Err(err)) => {
                                error!("Failed to initialize C&C game engine: {err}");
                                engine_init_future = None;
                                elwt.exit();
                            }
                            Poll::Pending => {
                                if let Some(started_at) = engine_init_started_at {
                                    let should_log = engine_init_last_log_at
                                        .map(|last| {
                                            last.elapsed() >= Duration::from_millis(500)
                                        })
                                        .unwrap_or_else(|| started_at.elapsed() >= Duration::from_millis(500));
                                    if should_log {
                                        info!(
                                            "Engine bootstrap still in progress ({:.2}s elapsed)",
                                            started_at.elapsed().as_secs_f32()
                                        );
                                        engine_init_last_log_at = Some(Instant::now());
                                    }
                                }
                            }
                        }
                    }

                    next_redraw_at = Instant::now() + STARTUP_POLL_INTERVAL;
                    elwt.set_control_flow(ControlFlow::WaitUntil(next_redraw_at));
                }
                _ => {}
            }
            return;
        }

        let Some(current_window) = window.as_ref() else {
            return;
        };
        let Some(engine) = engine.as_mut() else {
            return;
        };

        if engine.is_quitting() {
            if !shutdown_logged {
                info!("Engine shutting down");
                shutdown_logged = true;
            }
            if let Some(bridge) = runtime_host_bridge.as_mut() {
                let snapshot = engine.runtime_host_status_snapshot();
                bridge.publish_runtime(&snapshot);
            }
            elwt.exit();
            return;
        }

        match engine.process_platform_event(&event) {
            Ok(handled) => {
                if handled {
                    return;
                }
            }
            Err(e) => {
                error!("Platform message handling error: {}", e);
            }
        }

        if engine.is_quit_requested() {
            if !engine.is_quitting() && !engine.is_state_change_pending(GameState::Exiting) {
                info!("Platform requested quit");
                engine.request_state_change(GameState::Exiting);
            }
            return;
        }

        match event {
            Event::WindowEvent {
                ref event,
                window_id,
            } if window_id == current_window.id() => {
                if !engine.input(event) {
                    match event {
                        WindowEvent::CloseRequested => {
                            info!("Close requested by window");
                            engine.request_state_change(GameState::Exiting);
                        }
                        WindowEvent::Destroyed => {
                            info!("Window destroyed - forcing exit");
                            engine.request_state_change(GameState::Exiting);
                        }
                        WindowEvent::KeyboardInput {
                            event:
                                KeyEvent {
                                    state: ElementState::Pressed,
                                    logical_key: Key::Named(NamedKey::Escape),
                                    ..
                                },
                            ..
                        } => match engine.get_state() {
                            GameState::InGame => {
                                // C++ MSG_META_OPTIONS always ToggleQuitMenu.
                                info!("Escape pressed in InGame state - pausing");
                                engine.request_state_change(GameState::Paused);
                            }
                            GameState::Paused => {
                                info!("Escape pressed in Paused state - resuming");
                                engine.request_state_change(GameState::InGame);
                            }
                            GameState::Menu | GameState::Loading => {
                                // C++ shell Escape is WindowXlat / menu callbacks, never app-quit.
                            }
                            GameState::Victory | GameState::Defeat => {
                                info!("Escape pressed in endgame - returning to menu");
                                engine.request_state_change(GameState::Menu);
                            }
                            GameState::Exiting | GameState::Initializing => {}
                        },
                        WindowEvent::Resized(physical_size) => {
                            runtime_window_minimized |=
                                physical_size.width == 0 || physical_size.height == 0;
                            update_iconic_state_and_wake_audio(
                                current_window,
                                &mut runtime_window_minimized,
                            );
                            if !runtime_window_minimized {
                                engine.resize(*physical_size);
                            }
                        }
                        WindowEvent::ScaleFactorChanged { .. } => {
                            // Keep UI/layout hit-testing in sync on HiDPI transitions (macOS).
                            update_iconic_state_and_wake_audio(
                                current_window,
                                &mut runtime_window_minimized,
                            );
                            if !runtime_window_minimized {
                                engine.resize(current_window.inner_size());
                            }
                        }
                        WindowEvent::RedrawRequested => {
                            update_iconic_state_and_wake_audio(
                                current_window,
                                &mut runtime_window_minimized,
                            );
                            let runtime_window_suspended = runtime_window_minimized;
                            if runtime_headless_mode {
                                // Keep the headless loop alive: hidden windows may stop
                                // delivering AboutToWait unless redraw is requested.
                                let now = Instant::now();
                                if now >= next_redraw_at {
                                    let present_due = now >= next_headless_present_at;
                                    drive_frame(
                                        engine,
                                        current_window,
                                        &mut runtime_host_bridge,
                                        present_due,
                                    );
                                    if present_due {
                                        next_headless_present_at =
                                            Instant::now() + HEADLESS_PRESENT_INTERVAL;
                                    }
                                    next_redraw_at = Instant::now() + HEADLESS_LOGIC_INTERVAL;
                                }
                                current_window.request_redraw();
                            } else if runtime_window_suspended {
                                if should_keep_logic_running_while_iconic(
                                    // Prefer presentation game_mode residual when installed.
                                    engine.presentation_or_live_game_mode(),
                                ) {
                                    drive_frame(
                                        engine,
                                        current_window,
                                        &mut runtime_host_bridge,
                                        false,
                                    );
                                }
                            } else {
                                let now = Instant::now();
                                if now >= next_redraw_at {
                                    let prev_time = Instant::now();
                                    drive_frame(
                                        engine,
                                        current_window,
                                        &mut runtime_host_bridge,
                                        true,
                                    );
                                    next_redraw_at = execute_wait_deadline(
                                        prev_time,
                                        engine.live_present_interval(),
                                    );
                                }
                            }
                        }
                        _ => {}
                    }
                }
            }
            #[cfg(target_os = "macos")]
            Event::DeviceEvent {
                event: DeviceEvent::Button { button, state },
                ..
            } if !runtime_headless_mode => {
                // Inactive NSApp never gets WindowEvent::MouseInput. winit still
                // emits DeviceEvent::Button from NSApplication.sendEvent for HID
                // that reaches this process (including CGEventPostToPid).
                log::info!("DeviceEvent::Button button={button} state={state:?}");
                if let Some((x, y)) = macos_cursor_client_if_in_window(current_window) {
                    let scale = current_window.scale_factor().max(0.0001) as f32;
                    engine.apply_cursor_position(x * scale, y * scale);
                    log::info!("DeviceEvent in-window client=({x:.0},{y:.0})");
                    #[cfg(feature = "game_client")]
                    if let Some(name) =
                        game_client::gui::os_wnd_widget_under_cursor_name(x as i32, y as i32)
                    {
                        log::info!("DeviceEvent under={name}");
                        if matches!(state, winit::event::ElementState::Pressed)
                            && name.contains("SkirmishGameOptionsMenu.wnd:ButtonStart")
                        {
                            let ok = game_client::gui::simulate_skirmish_start_button_gadget_selected();
                            log::info!("physical Skirmish Start GBM residual ok={ok}");
                            if ok {
                                if let Some(request) = engine.take_pending_new_game_start_request() {
                                    if !matches!(request.mode, GameMode::Shell) {
                                        log::info!(
                                            "DeviceEvent Start drain mode={:?} map={}",
                                            request.mode,
                                            request.map
                                        );
                                        engine.start_game_from_ui(request);
                                        let _ = super::CnCGameEngine::take_new_game_dispatch_from_common_stream();
                                    } else {
                                        super::CnCGameEngine::take_shell_new_game_messages_from_common_stream();
                                    }
                                }
                            }
                        }
                    }
                    game_client::gui::log_named_window_screen_rect("MainMenu.wnd:ButtonSinglePlayer");
                    game_client::gui::log_named_window_screen_rect("MainMenu.wnd:ButtonSkirmish");
                    game_client::gui::log_named_window_screen_rect("MainMenu.wnd:MapBorder");
                    game_client::gui::log_named_window_screen_rect("MainMenu.wnd:EarthMap");
                    game_client::gui::log_named_window_screen_rect("MainMenu.wnd:MapBorder2");
                    game_client::gui::log_named_window_screen_rect(
                        "SkirmishGameOptionsMenu.wnd:ButtonStart",
                    );
                    if let Some(btn) = device_button_to_mouse(button) {
                        let pressed = matches!(state, ElementState::Pressed);
                        engine.handle_mouse_button_input(
                            btn,
                            pressed,
                            super::input::MouseInputOrigin::Physical,
                        );
                    }
                }
            }
            Event::AboutToWait => {
                let now = Instant::now();
                if now >= next_redraw_at {
                    update_iconic_state_and_wake_audio(
                        current_window,
                        &mut runtime_window_minimized,
                    );
                    let runtime_window_suspended = runtime_window_minimized;
                    if runtime_headless_mode {
                        // Split logic vs present: always advance sim at ~30 Hz; only pay for
                        // GPU/screenshot on HEADLESS_PRESENT_INTERVAL so construction/combat
                        // are not gated by mesh draw cost.
                        let present_due = now >= next_headless_present_at;
                        drive_frame(
                            engine,
                            current_window,
                            &mut runtime_host_bridge,
                            present_due,
                        );
                        if present_due {
                            next_headless_present_at = Instant::now() + HEADLESS_PRESENT_INTERVAL;
                        }
                        next_redraw_at = Instant::now() + HEADLESS_LOGIC_INTERVAL;
                        current_window.request_redraw();
                    } else if cmd_args.wants_smoke_test() {
                        drive_frame(engine, current_window, &mut runtime_host_bridge, false);
                        if engine.is_quitting() {
                            elwt.exit();
                            return;
                        }
                        next_redraw_at = now + STARTUP_POLL_INTERVAL;
                    } else if runtime_window_suspended {
                        if should_keep_logic_running_while_iconic(
                            // Prefer presentation game_mode residual when installed.
                            engine.presentation_or_live_game_mode(),
                        ) {
                            drive_frame(engine, current_window, &mut runtime_host_bridge, false);
                        }
                        next_redraw_at = now + MINIMIZED_POLL_INTERVAL;
                    } else {
                        // C++ WinMain pumps GameEngine::update every wait, not
                        // only when macOS delivers RedrawRequested. Waiting on
                        // request_redraw alone froze status.txt after first Menu.
                        // C++ GameEngine::execute (GameEngine.cpp:856-866):
                        // (now - prevTime) includes the just-finished update()+draw.
                        let prev_time = Instant::now();
                        drive_frame(engine, current_window, &mut runtime_host_bridge, true);
                        current_window.request_redraw();
                        next_redraw_at =
                            execute_wait_deadline(prev_time, engine.live_present_interval());
                    }
                }
                if engine.live_present_interval().is_none()
                    && !runtime_headless_mode
                    && !cmd_args.wants_smoke_test()
                    && !runtime_window_minimized
                {
                    elwt.set_control_flow(ControlFlow::Poll);
                } else {
                    elwt.set_control_flow(ControlFlow::WaitUntil(next_redraw_at));
                }
            }
            Event::LoopExiting => {
                #[cfg(feature = "integration-diagnostics")]
                if let Some(bridge) = integration_bridge.take() {
                    if let Err(err) = runtime_handle.block_on(bridge.shutdown()) {
                        error!("Failed to shut down integration telemetry bridge: {err:?}");
                    }
                }
            }
            _ => {}
        }
    })?;

    info!("C&C Game ended successfully");
    Ok(())
}

/// Map HUD structure cameo labels to ThingTemplate residual names.
pub(super) fn resolve_ui_structure_template_name(name: &str) -> String {
    let n = name.trim();
    if n.is_empty() {
        return String::new();
    }
    // Already a template-style name.
    if n.contains("America") || n.contains("China") || n.contains("GLA") || n.contains('_') {
        return n.to_string();
    }
    let key = n.to_ascii_lowercase();
    match key.as_str() {
        "power plant" | "powerplant" => "AmericaPowerPlant".into(),
        "barracks" => "AmericaBarracks".into(),
        "supply center" | "supplycenter" => "AmericaSupplyCenter".into(),
        "war factory" | "warfactory" => "AmericaWarFactory".into(),
        "airfield" => "AmericaAirfield".into(),
        "command center" | "commandcenter" => "AmericaCommandCenter".into(),
        "patriot battery" | "patriot" => "AmericaPatriotBattery".into(),
        "strategy center" => "AmericaStrategyCenter".into(),
        "detention camp" => "AmericaDetentionCamp".into(),
        "particle cannon" => "AmericaParticleCannonUplink".into(),
        _ => {
            // Fallback: strip spaces residual.
            let compact: String = n.chars().filter(|c| !c.is_whitespace()).collect();
            format!("America{compact}")
        }
    }
}

/// C++ `GameEngine::execute` wait (`GameEngine.cpp:856-866`):
/// `(now - prevTime)` includes the just-finished `update()`+draw.
/// Effective period is `max(work, limit)`, not `work+limit`.
fn execute_wait_deadline(prev_time: Instant, interval: Option<Duration>) -> Instant {
    match interval {
        Some(limit) => prev_time + limit,
        None => Instant::now(),
    }
}

/// Map leftover/host `ObjectShroudStatus` onto Common radar discriminants.
fn host_object_shroud_to_radar(
    status: gamelogic::common::ObjectShroudStatus,
) -> game_engine::common::game_common::ObjectShroudStatus {
    use game_engine::common::game_common::ObjectShroudStatus as RadarShroud;
    use gamelogic::common::ObjectShroudStatus as HostShroud;
    match status {
        HostShroud::Invalid => RadarShroud::Invalid,
        HostShroud::Clear => RadarShroud::Clear,
        HostShroud::PartialClear => RadarShroud::PartialClear,
        HostShroud::Fogged => RadarShroud::Fogged,
        HostShroud::Shrouded => RadarShroud::Shrouded,
        HostShroud::InvalidButPreviousValid => RadarShroud::InvalidButPreviousValid,
    }
}

impl CnCGameEngine {
    /// C++ `GameEngine::update` (`GameEngine.cpp:732`) `TheRadar->UPDATE()`.
    pub(super) fn host_update_the_radar(&self) {
        if let Ok(mut shroud) = gamelogic::system::shroud_manager::get_shroud_manager().lock() {
            shroud.refresh_shroud_for_local_player();
        }

        let local_id = self
            .presentation_or_boot_local_player_id()
            .unwrap_or(self.current_player_id);
        let player = self.game_logic.get_player(local_id);
        // C++ `Player::isPlayerActive` = `!observer && !dead`.
        let local_player_active = player.map(|p| p.is_alive && !p.is_observer).unwrap_or(true);
        let local_has_radar = player.map(|p| p.has_radar()).unwrap_or(false);
        let radar_forced = self.game_logic.radar_forced();
        if let Ok(mut radar) = game_engine::common::system::radar::get_radar_system().write() {
            radar.set_local_player_active(local_player_active);
            radar.set_local_has_radar(local_has_radar);
            radar.force_on(radar_forced);
            let frame = self.host_match_logic_frame.unwrap_or(0);
            radar.update(frame);
            // C++ `getShroudedStatus` is queried at overlay render, after
            // `Radar::update` rebuilds the object lists. Stamp after sync so
            // PARTIAL_CLEAR fog-edge blips survive provider rebuild.
            if let Ok(shroud) = gamelogic::system::shroud_manager::get_shroud_manager().lock() {
                radar.apply_object_shrouds(|object_id| {
                    shroud
                        .get_host_object_shroud_status(local_id, object_id)
                        .map(host_object_shroud_to_radar)
                });
            }
        }
    }

    /// C++ W3DDisplay.cpp:1730-1781 freeze-aware virtual clock.
    pub(super) fn advance_ww3d_visual_sync(&self) -> u32 {
        let frame = self.host_match_logic_frame.unwrap_or(0);
        let last = last_ww3d_client_frame().swap(frame, std::sync::atomic::Ordering::SeqCst);
        let same_client_frame = last == frame;
        let freeze = self.presentation_or_boot_time_frozen()
            || self.game_paused
            || matches!(self.current_state, GameState::Paused)
            || same_client_frame;
        if !freeze {
            ww3d_sync_ms().fetch_add(W3D_FRAME_LENGTH_MS, std::sync::atomic::Ordering::SeqCst);
        }
        ww3d_sync_ms().load(std::sync::atomic::Ordering::SeqCst)
    }

    /// C++ `TheScriptEngine->isTimeFast()` analog: visual speed at/above logic Hz.
    pub(super) fn script_time_fast(&self) -> bool {
        self.presentation_or_boot_visual_speed() >= 30.0
    }

    /// C++ W3DDisplay.cpp:1741-1795 + 1852-1855 render-throttle contract.
    pub(super) fn should_present_w3d_frame(&self) -> bool {
        let freeze = self.presentation_or_boot_time_frozen()
            || self.game_paused
            || matches!(self.current_state, GameState::Paused);
        if !freeze && self.script_time_fast() {
            return false;
        }
        let multiplier = self.presentation_or_boot_visual_speed().max(1.0) as i32;
        if multiplier > 1 {
            let prev = time_multiplier_counter().fetch_sub(1, std::sync::atomic::Ordering::SeqCst);
            if prev - 1 > 1 {
                return false;
            }
            time_multiplier_counter().store(multiplier, std::sync::atomic::Ordering::SeqCst);
        }
        let tivo = game_engine::common::global_data::read_safe()
            .map(|data| data.tivo_fast_mode)
            .unwrap_or(false);
        if tivo && self.presentation_or_boot_in_replay_game() && !freeze {
            let frame = self.host_match_logic_frame.unwrap_or(0);
            if frame % 30 != 1 {
                return false;
            }
        }
        true
    }

    /// C++ execute limiter + W3DDisplay 30ms draw limiter.
    /// `None` = unlocked (m_useFpsLimit false) or time-fast / TiVO replay.
    pub(super) fn live_present_interval(&self) -> Option<Duration> {
        let global = game_engine::common::global_data::read_safe().ok()?;
        if !global.writable.use_fps_limit {
            return None;
        }
        let visual_speed = self.presentation_or_boot_visual_speed();
        if visual_speed > 1.0 || self.script_time_fast() {
            return None;
        }
        if global.tivo_fast_mode && self.presentation_or_boot_in_replay_game() {
            return None;
        }
        let max_fps = if global.writable.frames_per_second_limit > 0 {
            global.writable.frames_per_second_limit as u32
        } else {
            DEFAULT_MAX_FPS
        };
        let exec_ms = (1000.0 / max_fps as f32 - 1.0).max(0.0);
        let draw_ms = (W3D_DRAW_MIN_TIME_MS as f32 - 1.0).max(0.0);
        let ms = exec_ms.max(draw_ms);
        Some(Duration::from_millis(ms.round() as u64))
    }

    /// C++ `W3DDisplay::updateAverageFPS` + `findDynamicLODLevel` / force VERY_HIGH.
    pub(super) fn apply_live_draw_dynamic_lod(&self) {
        let average = average_fps_tracker().note_frame();
        game_engine::common::game_engine::GameEngine::apply_draw_dynamic_lod(average);
    }
}

#[cfg(test)]
mod tests {
    use super::{DEFAULT_MAX_FPS, FRAME_INTERVAL, HEADLESS_LOGIC_INTERVAL, execute_wait_deadline};
    use std::time::{Duration, Instant};

    #[test]
    fn windowed_present_cap_is_cpp_default_max_fps_45() {
        // C++ GameEngine.h:13 `#define DEFAULT_MAX_FPS 45`
        // C++ GameEngine.cpp:271 `m_maxFPS = DEFAULT_MAX_FPS`
        // C++ GameEngine.cpp:856-857 execute present cap:
        //   `DWORD limit = (1000.0f/m_maxFPS)-1`
        // Windowed WaitUntil present interval is 1/45 s; logic stays 30 Hz.
        assert_eq!(DEFAULT_MAX_FPS, 45);
        assert_eq!(FRAME_INTERVAL, Duration::from_micros(1_000_000 / 45));
        assert_ne!(
            FRAME_INTERVAL,
            Duration::from_micros(1_000_000 / 60),
            "pre-fix 60 Hz windowed present cap must not remain"
        );
        assert_eq!(
            HEADLESS_LOGIC_INTERVAL,
            Duration::from_nanos(33_333_333),
            "headless 30 Hz logic interval must stay 30 logic frames/sec"
        );
    }

    #[test]
    fn windowed_boot_shows_window_and_latches_present() {
        let src = include_str!("run_loop.rs");
        let live = src
            .split("#[cfg(test)]")
            .next()
            .expect("run_loop live path");
        assert!(
            live.contains("apply_runtime_host_window_visibility")
                && live.contains("publish_booting_from_winit_query")
                && live.contains("created_window.is_visible()"),
            "windowed boot must show the OS window and publish the honest winit query"
        );
        assert!(
            live.contains("note_windowed_surface_presented")
                && live.contains("if !runtime_headless_mode"),
            "live_frame_ok latch only after a successful windowed render"
        );
        let hide = live
            .find("apply_runtime_host_window_visibility(&created_window, runtime_headless_mode)")
            .expect("visibility apply at create");
        assert!(
            live[hide.saturating_sub(80)..hide].contains("runtime_headless_mode")
                || live[hide..hide + 400].contains("runtime_headless_mode"),
            "visibility helper must receive the headless flag"
        );
    }

    #[test]
    fn execute_wait_deadline_includes_work_like_cpp_prev_time() {
        // C++ GameEngine.cpp:856-866: deadline is prevTime+limit, not now+limit.
        let prev = Instant::now();
        let limit = Duration::from_millis(29);
        let deadline = execute_wait_deadline(prev, Some(limit));
        assert_eq!(deadline, prev + limit);
        let unlocked = execute_wait_deadline(prev, None);
        assert!(unlocked >= prev);
    }

    #[test]
    fn live_drive_frame_calls_the_radar_update_before_audio() {
        let src = include_str!("run_loop.rs");
        let live = src
            .split("#[cfg(test)]")
            .next()
            .expect("run_loop live path");
        let radar = live
            .find("engine.host_update_the_radar()")
            .expect("TheRadar->UPDATE on live drive_frame");
        let audio = live
            .find("engine.host_update_the_audio()")
            .expect("TheAudio->UPDATE on live drive_frame");
        assert!(
            radar < audio,
            "C++ GameEngine.cpp:732 TheRadar->UPDATE before TheAudio->UPDATE"
        );
        assert!(
            live.contains("radar.update(frame)"),
            "host_update_the_radar must call RadarSystem::update"
        );
        let update_at = live
            .find("radar.update(frame)")
            .expect("RadarSystem::update");
        let stamp_at = live
            .rfind("apply_object_shrouds")
            .expect("object shroud stamp");
        assert!(
            update_at < stamp_at,
            "C++ getShroudedStatus is queried after Radar::update rebuilds lists"
        );
        assert!(
            live.contains("execute_wait_deadline("),
            "windowed WaitUntil must include work via execute_wait_deadline"
        );
        assert!(
            !live.contains(
                "Instant::now()\n                            + engine.live_present_interval()"
            ),
            "must not add present interval after drive_frame returns"
        );
    }
}
