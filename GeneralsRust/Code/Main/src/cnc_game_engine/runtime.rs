#![allow(unused_imports, unused_variables, dead_code, non_snake_case)]
//! Runtime-host GPUI control/status/frame bridge extracted from `cnc_game_engine`.
//!
//! Mechanical split: types + I/O live here; `CnCGameEngine` still owns command
//! dispatch and snapshot assembly.

use super::*;
use crate::command_line::CommandLineArgs;
use log::{info, warn};
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

#[derive(Debug, Clone)]
pub(super) struct RuntimeHostSnapshot {
    pub(super) state: String,
    pub(super) ui_screen: String,
    /// Sticky skirmish menu residual (survives InGame ui_screen clear).
    pub(super) skirmish_menu_ok: bool,
    pub(super) paused: bool,
    pub(super) fps: f32,
    pub(super) startup_progress: f32,
    pub(super) startup_phase: String,
    pub(super) map: String,
    pub(super) frame: u32,
    /// Host GameLogic fixed-step frame counter (30 Hz residual).
    pub(super) logic_frame: u32,
    /// Last step_simulation steps_run (catch-up residual).
    pub(super) logic_steps: u32,
    /// Friendly under-construction structures (local player team).
    pub(super) under_construction: u32,
    /// Match damage applied residual (host combat honesty).
    pub(super) match_damage_applied: f32,
    /// Match kill events residual (host combat honesty).
    pub(super) match_kills: u32,
    pub(super) selected_count: u32,
    pub(super) local_mobile_units: u32,
    pub(super) last_gameplay_cmd: String,
    pub(super) match_over: bool,
    pub(super) victory_label: String,
    /// PresentationFrame installed for client/render residual.
    pub(super) presentation_frame_ok: bool,
    /// GameWorld observe-path presentation entity count (after coupled shadow tick).
    pub(super) gameworld_presentation_entities: u32,
    /// PresentationFrame.gameworld_overlay_stamped after last overlay call.
    pub(super) gameworld_overlay_stamped: u32,
    /// PresentationFrame.gameworld_appended after last append_missing_from_gameworld.
    pub(super) gameworld_appended: u32,
    /// PresentationFrame.gameworld_rebuilt after last rebuild_objects_from_gameworld.
    pub(super) gameworld_rebuilt: u32,
    /// PresentationFrame.gameworld_primary_objects residual.
    pub(super) gameworld_primary_objects: bool,
    /// Shell screen stack depth residual (retail WND push honesty).
    pub(super) shell_screen_count: u32,
    /// Top shell layout filename residual (e.g. Menus/MainMenu.wnd).
    pub(super) shell_top_wnd: String,
    /// Shell::is_shell_active residual.
    pub(super) shell_active: bool,
    /// Live GameLogic dual-reads during last presentation-owned collect (must be 0 in-game).
    pub(super) presentation_live_fallback_reads: u32,
    /// Sticky waypoint mode residual.
    pub(super) waypoint_mode: bool,
    /// Live GPU/screenshot frame published (not shell fallback only).
    pub(super) live_frame_ok: bool,
    /// winit window is visible and host is not headless.
    pub(super) window_visible: bool,
    /// Physical WND menu click completed an offline menu-to-match transition.
    pub(super) wnd_widget_tree_nav: bool,
    /// Physical in-game command after a physical WND menu→match transition.
    pub(super) interactive_gameplay: bool,
    /// Physical Control Bar DozerConstruct arm followed by a physical, valid
    /// production queue request in this visible offline session.
    pub(super) physical_build_and_produce: bool,
    /// A tracked carrier from a physical right-click Gather order deposited
    /// positive carried supplies for the local player in a visible offline
    /// match. This is not inferred from generic resource totals.
    pub(super) physical_gather_resources: bool,
    /// Physical confirmed PopupSaveLoad save followed by a physical confirmed
    /// load which both succeeded through Main's snapshot authority.
    pub(super) physical_save_load_continue: bool,
    /// Host requested capture this frame (bridge should force screenshot).
    pub(super) pending_capture: bool,
    /// Last unit-pass collect honesty (presentation residual).
    pub(super) render_alive_objects: u32,
    pub(super) render_fow_filtered: u32,
    pub(super) render_item_count: u32,
    pub(super) render_model_missing: u32,
    pub(super) render_frustum_culled: u32,
    pub(super) camera_pos: String,
    pub(super) camera_target: String,
    pub(super) sample_unit_pos: String,
    /// Physical OS window outer origin X (diagnostic clicker aim). Headless = 0.
    pub(super) window_outer_x: i32,
    /// Physical OS window outer origin Y (diagnostic clicker aim). Headless = 0.
    pub(super) window_outer_y: i32,
    /// Physical OS window outer width (diagnostic clicker aim). Headless = 0.
    pub(super) window_outer_w: u32,
    /// Physical OS window outer height (diagnostic clicker aim). Headless = 0.
    pub(super) window_outer_h: u32,
    /// Hittable named gadget centers as `Name@x,y` (screen space). Empty when none.
    pub(super) gadget_hits: Vec<String>,
}

#[derive(Debug)]
pub(super) struct RuntimeHostBridge {
    control_path: PathBuf,
    status_path: PathBuf,
    frame_path: PathBuf,
    capture_path: PathBuf,
    frame_meta_path: PathBuf,
    fallback_frame_png: Option<Vec<u8>>,
    fallback_frame_luma: f32,
    last_published_frame: u32,
    last_capture_request_at: Option<Instant>,
    capture_request_in_flight: bool,
    capture_request_started_at: Option<Instant>,
    screenshot_enqueue_failed: bool,
    has_published_live_frame: bool,
    created_at: Instant,
    last_capture_health_log_at: Option<Instant>,
}

/// Named WND gadgets an OS clicker may aim at (menu → match). Diagnostic only.
pub(super) const STATUS_GADGET_HIT_NAMES: &[&str] = &[
    "MainMenu.wnd:ButtonSinglePlayer",
    "MainMenu.wnd:ButtonSkirmish",
    "MainMenu.wnd:ButtonUSA",
    "MainMenu.wnd:ButtonGLA",
    "MainMenu.wnd:ButtonChina",
    "MainMenu.wnd:ButtonChallenge",
    "MainMenu.wnd:ButtonSingleBack",
    "MainMenu.wnd:ButtonEasy",
    "MainMenu.wnd:ButtonMedium",
    "MainMenu.wnd:ButtonHard",
    "MainMenu.wnd:ButtonDiffBack",
    "SkirmishGameOptionsMenu.wnd:ButtonStart",
    "SkirmishMapSelectMenu.wnd:ButtonOk",
];

/// Diagnostic clicker-aim fragment for status.txt. Never writes playable_claim
/// or five-flag evidence keys.
pub(super) fn format_clicker_aim_status(
    window_outer_x: i32,
    window_outer_y: i32,
    window_outer_w: u32,
    window_outer_h: u32,
    gadget_hits: &[String],
) -> String {
    let mut out = format!(
        "window_outer_x={window_outer_x}\nwindow_outer_y={window_outer_y}\nwindow_outer_w={window_outer_w}\nwindow_outer_h={window_outer_h}\n"
    );
    for hit in gadget_hits {
        let hit = hit.trim();
        if hit.is_empty() {
            continue;
        }
        out.push_str("gadget_hit=");
        out.push_str(hit);
        out.push('\n');
    }
    out
}

impl RuntimeHostBridge {
    pub(super) const CAPTURE_REQUEST_INTERVAL_LOADING: Duration = Duration::from_millis(120);
    pub(super) const CAPTURE_REQUEST_INTERVAL_INTERACTIVE: Duration = Duration::from_millis(40);
    pub(super) const CAPTURE_REQUEST_TIMEOUT: Duration = Duration::from_secs(3);

    pub(super) fn force_capture_request(&mut self) {
        // Drop interval gate so next publish_frame requests a screenshot immediately.
        self.last_capture_request_at = None;
        self.capture_request_in_flight = false;
        self.capture_request_started_at = None;
        // Best-effort immediate request.
        match ww3d_engine::make_screenshot(&self.capture_path) {
            Ok(()) => {
                self.last_capture_request_at = Some(Instant::now());
                self.capture_request_in_flight = true;
                self.capture_request_started_at = Some(Instant::now());
            }
            Err(err) => {
                log::trace!("force_capture_request screenshot failed: {err:?}");
            }
        }
    }

    /// Latch live_frame_ok after a real windowed wgpu surface present.
    ///
    /// Capture-path promotion still owns the primary residual; this covers the
    /// honest case where the OS surface has presented even if PNG readback is
    /// delayed. Headless must never call this (no OS surface present).
    pub(super) fn note_windowed_surface_presented(&mut self) {
        if !self.has_published_live_frame {
            info!(
                "Runtime host latched live_frame_ok from windowed surface present (frame={})",
                self.last_published_frame
            );
        }
        self.has_published_live_frame = true;
    }

    pub(super) fn capture_interval_for_state(state: &str) -> Duration {
        match state {
            "Menu" | "InGame" | "Paused" => Self::CAPTURE_REQUEST_INTERVAL_INTERACTIVE,
            _ => Self::CAPTURE_REQUEST_INTERVAL_LOADING,
        }
    }

    pub(super) fn is_headless_mode(args: &CommandLineArgs) -> bool {
        args.get_option_value("runtime_host")
            .map(|mode| mode.trim().eq_ignore_ascii_case("headless"))
            .unwrap_or(false)
    }

    /// Any non-empty `--runtime_host` (headless or windowed) with GPUI paths.
    pub(super) fn is_runtime_host_requested(args: &CommandLineArgs) -> bool {
        args.get_option_value("runtime_host")
            .map(|mode| !mode.trim().is_empty())
            .unwrap_or(false)
    }

    pub(super) fn from_command_line(args: &CommandLineArgs) -> Option<Self> {
        if !Self::is_runtime_host_requested(args) {
            return None;
        }
        let control_path = PathBuf::from(args.get_option_value("gpui_control")?);
        let status_path = PathBuf::from(args.get_option_value("gpui_status")?);
        let frame_path = PathBuf::from(args.get_option_value("gpui_frame")?);
        let capture_path = frame_path.with_extension("png.capture");
        let frame_meta_path = frame_path.with_extension("png.meta");

        let _ = fs::remove_file(&control_path);
        let _ = fs::remove_file(&status_path);
        let _ = fs::remove_file(&frame_path);
        let _ = fs::remove_file(&capture_path);
        let _ = fs::remove_file(&frame_meta_path);

        let (fallback_frame_png, fallback_frame_luma) = Self::load_fallback_frame_png();
        Some(Self {
            control_path,
            status_path,
            frame_path,
            capture_path,
            frame_meta_path,
            fallback_frame_png,
            fallback_frame_luma,
            last_published_frame: 0,
            last_capture_request_at: None,
            capture_request_in_flight: false,
            capture_request_started_at: None,
            screenshot_enqueue_failed: false,
            has_published_live_frame: false,
            created_at: Instant::now(),
            last_capture_health_log_at: None,
        })
    }

    pub(super) fn drain_commands(&mut self) -> Vec<String> {
        // Wave 833: read via fs::read_to_string + atomic clear. OpenOptions
        // read/write without an explicit seek(0) can miss peer writes on some
        // platforms; empty drains left smoke stuck at MainMenu forever.
        let payload = match fs::read_to_string(&self.control_path) {
            Ok(text) => text,
            Err(_) => return Vec::new(),
        };
        if payload.trim().is_empty() {
            return Vec::new();
        }
        // Clear after successful read so each command is consumed once.
        if let Err(err) = fs::write(&self.control_path, b"") {
            warn!(
                "Runtime host failed clearing control file {}: {err}",
                self.control_path.display()
            );
        }
        let cmds: Vec<String> = payload
            .lines()
            .map(str::trim)
            .filter(|line| !line.is_empty())
            .map(|line| line.to_string())
            .collect();
        if !cmds.is_empty() {
            info!(
                "Runtime host drained {} control command(s): {:?}",
                cmds.len(),
                cmds.iter().take(4).collect::<Vec<_>>()
            );
        }
        cmds
    }

    pub(super) fn publish_booting(&mut self) {
        self.publish_booting_from_winit_query(true, Some(false));
    }

    /// Boot residual uses the same winit query as the live snapshot.
    /// No window yet → pass `Some(false)`. Never forge `true`.
    pub(super) fn publish_booting_from_winit_query(
        &mut self,
        headless: bool,
        is_visible: Option<bool>,
    ) {
        let window_visible =
            crate::executable_smoke::ExecutableSmokeResult::window_visible_from_winit_query(
                headless, is_visible,
            );
        let snapshot = RuntimeHostSnapshot {
            state: "Booting".to_string(),
            ui_screen: "None".to_string(),
            skirmish_menu_ok: false,
            paused: false,
            fps: 0.0,
            startup_progress: 0.0,
            startup_phase: "Booting runtime".to_string(),
            map: "-".to_string(),
            frame: self.last_published_frame,
            logic_frame: 0,
            logic_steps: 0,
            under_construction: 0,
            match_damage_applied: 0.0,
            match_kills: 0,
            selected_count: 0,
            local_mobile_units: 0,
            last_gameplay_cmd: String::new(),
            match_over: false,
            victory_label: String::new(),
            presentation_frame_ok: false,
            gameworld_presentation_entities: 0,
            gameworld_overlay_stamped: 0,
            gameworld_appended: 0,
            gameworld_rebuilt: 0,
            gameworld_primary_objects: false,
            shell_screen_count: 0,
            shell_top_wnd: String::new(),
            shell_active: false,
            presentation_live_fallback_reads: 0,
            waypoint_mode: false,
            live_frame_ok: false,
            window_visible,
            wnd_widget_tree_nav: false,
            interactive_gameplay: false,
            physical_build_and_produce: false,
            physical_gather_resources: false,
            physical_save_load_continue: false,
            pending_capture: false,
            render_alive_objects: 0,
            render_fow_filtered: 0,
            render_item_count: 0,
            render_model_missing: 0,
            render_frustum_culled: 0,
            camera_pos: String::new(),
            camera_target: String::new(),
            sample_unit_pos: String::new(),
            window_outer_x: 0,
            window_outer_y: 0,
            window_outer_w: 0,
            window_outer_h: 0,
            gadget_hits: Vec::new(),
        };
        self.publish_status(&snapshot);
    }

    pub(super) fn publish_runtime(&mut self, snapshot: &RuntimeHostSnapshot) {
        // Promote capture before status so live_frame_ok reflects this frame.
        self.publish_frame(snapshot.frame, &snapshot.state);
        self.publish_status(snapshot);
    }

    pub(super) fn publish_status(&mut self, snapshot: &RuntimeHostSnapshot) {
        let mut payload = String::new();
        payload.push_str(&format!("state={}\n", snapshot.state));
        payload.push_str(&format!("ui_screen={}\n", snapshot.ui_screen));
        payload.push_str(&format!("skirmish_menu_ok={}\n", snapshot.skirmish_menu_ok));
        payload.push_str(&format!("paused={}\n", snapshot.paused));
        payload.push_str(&format!("fps={:.3}\n", snapshot.fps.max(0.0)));
        payload.push_str(&format!(
            "startup_progress={:.4}\n",
            snapshot.startup_progress.clamp(0.0, 1.0)
        ));
        payload.push_str(&format!("startup_phase={}\n", snapshot.startup_phase));
        payload.push_str(&format!("map={}\n", snapshot.map));
        payload.push_str(&format!("frame={}\n", snapshot.frame));
        payload.push_str(&format!("logic_frame={}\n", snapshot.logic_frame));
        payload.push_str(&format!("logic_steps={}\n", snapshot.logic_steps));
        payload.push_str(&format!(
            "under_construction={}\n",
            snapshot.under_construction
        ));
        payload.push_str(&format!(
            "match_damage_applied={:.3}\n",
            snapshot.match_damage_applied
        ));
        payload.push_str(&format!("match_kills={}\n", snapshot.match_kills));
        payload.push_str(&format!("selected_count={}\n", snapshot.selected_count));
        payload.push_str(&format!(
            "local_mobile_units={}\n",
            snapshot.local_mobile_units
        ));
        payload.push_str(&format!(
            "last_gameplay_cmd={}\n",
            snapshot.last_gameplay_cmd
        ));
        payload.push_str(&format!("match_over={}\n", snapshot.match_over));
        payload.push_str(&format!("victory_label={}\n", snapshot.victory_label));
        payload.push_str(&format!(
            "presentation_frame_ok={}\n",
            snapshot.presentation_frame_ok
        ));
        payload.push_str(&format!(
            "gameworld_presentation_entities={}\n",
            snapshot.gameworld_presentation_entities
        ));
        payload.push_str(&format!(
            "gameworld_overlay_stamped={}\n",
            snapshot.gameworld_overlay_stamped
        ));
        payload.push_str(&format!(
            "gameworld_appended={}\n",
            snapshot.gameworld_appended
        ));
        payload.push_str(&format!(
            "gameworld_rebuilt={}\n",
            snapshot.gameworld_rebuilt
        ));
        payload.push_str(&format!(
            "gameworld_primary_objects={}\n",
            snapshot.gameworld_primary_objects
        ));
        payload.push_str(&format!(
            "shell_screen_count={}\n",
            snapshot.shell_screen_count
        ));
        payload.push_str(&format!("shell_top_wnd={}\n", snapshot.shell_top_wnd));
        payload.push_str(&format!("shell_active={}\n", snapshot.shell_active));
        payload.push_str(&format!(
            "presentation_live_fallback_reads={}\n",
            snapshot.presentation_live_fallback_reads
        ));
        payload.push_str(&format!("waypoint_mode={}\n", snapshot.waypoint_mode));
        // Honest live_frame sources: promoted capture PNG **or** windowed surface
        // present latch (`note_windowed_surface_presented`). Never fallback PNG;
        // never headless present (headless cannot call the present latch).
        payload.push_str(&format!(
            "live_frame_ok={}\n",
            crate::executable_smoke::ExecutableSmokeResult::live_frame_ok_from_windowed_present(
                snapshot.live_frame_ok,
                self.has_published_live_frame,
            )
        ));
        payload.push_str(&format!("window_visible={}\n", snapshot.window_visible));
        payload.push_str(&format!(
            "wnd_widget_tree_nav={}\n",
            snapshot.wnd_widget_tree_nav
        ));
        // Five-flag sit-through diagnostic. Gameplay is physical / inject RMB
        // evidence via handle_mouse_button_input, never a forged host string.
        let live_frame_ok =
            crate::executable_smoke::ExecutableSmokeResult::live_frame_ok_from_windowed_present(
                snapshot.live_frame_ok,
                self.has_published_live_frame,
            );
        let ingame = matches!(snapshot.state.as_str(), "InGame" | "Paused");
        let gameplay = snapshot.interactive_gameplay;
        let sit_through_missing =
            crate::executable_smoke::ExecutableSmokeResult::retail_sit_through_missing_flags(
                snapshot.window_visible,
                snapshot.wnd_widget_tree_nav,
                live_frame_ok,
                ingame,
                gameplay,
            );
        payload.push_str(&format!("ingame={ingame}\n"));
        payload.push_str(&format!("gameplay={gameplay}\n"));
        payload.push_str(&format!(
            "interactive_menu_wnd_match={}\n",
            snapshot.wnd_widget_tree_nav
        ));
        payload.push_str(&format!(
            "physical_build_and_produce={}\n",
            snapshot.physical_build_and_produce
        ));
        payload.push_str(&format!(
            "physical_gather_resources={}\n",
            snapshot.physical_gather_resources
        ));
        payload.push_str(&format!(
            "physical_save_load_continue={}\n",
            snapshot.physical_save_load_continue
        ));
        payload.push_str(&format!(
            "retail_sit_through_missing={sit_through_missing}\n"
        ));
        payload.push_str(&format!(
            "render_alive_objects={}\n",
            snapshot.render_alive_objects
        ));
        payload.push_str(&format!(
            "render_fow_filtered={}\n",
            snapshot.render_fow_filtered
        ));
        payload.push_str(&format!(
            "render_item_count={}\n",
            snapshot.render_item_count
        ));
        payload.push_str(&format!(
            "render_model_missing={}\n",
            snapshot.render_model_missing
        ));
        payload.push_str(&format!(
            "render_frustum_culled={}\n",
            snapshot.render_frustum_culled
        ));
        payload.push_str(&format!("camera_pos={}\n", snapshot.camera_pos));
        payload.push_str(&format!("camera_target={}\n", snapshot.camera_target));
        payload.push_str(&format!("sample_unit_pos={}\n", snapshot.sample_unit_pos));
        payload.push_str(&format_clicker_aim_status(
            snapshot.window_outer_x,
            snapshot.window_outer_y,
            snapshot.window_outer_w,
            snapshot.window_outer_h,
            &snapshot.gadget_hits,
        ));
        payload.push_str(&format!("pending_capture={}\n", snapshot.pending_capture));
        payload.push_str(&format!(
            "frame_path={}\n",
            self.frame_path.to_string_lossy()
        ));
        let _ = fs::write(&self.status_path, payload);
    }

    pub(super) fn publish_frame(&mut self, frame: u32, state: &str) {
        if frame <= self.last_published_frame {
            return;
        }
        self.last_published_frame = frame;

        self.promote_capture_frame_if_ready();

        if self.capture_request_in_flight {
            let timed_out = self
                .capture_request_started_at
                .map(|started| started.elapsed() >= Self::CAPTURE_REQUEST_TIMEOUT)
                .unwrap_or(false);
            if timed_out {
                warn!(
                    "Runtime host capture request timed out after {:?} (frame={}, in_flight={})",
                    Self::CAPTURE_REQUEST_TIMEOUT,
                    frame,
                    self.capture_request_in_flight
                );
                self.capture_request_in_flight = false;
                self.capture_request_started_at = None;
            }
        }

        let capture_interval = Self::capture_interval_for_state(state);
        let should_request_capture = !self.capture_request_in_flight
            && self
                .last_capture_request_at
                .map(|last| last.elapsed() >= capture_interval)
                .unwrap_or(true);
        if should_request_capture {
            let requested_at = Instant::now();
            match ww3d_engine::make_screenshot(&self.capture_path) {
                Ok(()) => {
                    self.last_capture_request_at = Some(requested_at);
                    self.capture_request_in_flight = true;
                    self.capture_request_started_at = Some(requested_at);
                    self.screenshot_enqueue_failed = false;
                }
                Err(err) => {
                    if !self.screenshot_enqueue_failed {
                        warn!(
                            "Runtime host frame capture unavailable ({err:?}); falling back to static frame"
                        );
                        self.screenshot_enqueue_failed = true;
                    }
                }
            }
        }

        self.promote_capture_frame_if_ready();

        if Self::png_file_looks_usable(&self.frame_path) {
            // Keep an already-written PNG (live capture or previous fallback) so
            // we do not clobber it. Do not treat fallback bytes as live_frame_ok;
            // only promote_capture_frame_if_ready latches has_published_live_frame.
            return;
        }
        if self.has_published_live_frame {
            // Keep the most recent live frame while a newer capture is pending.
            return;
        }

        let should_log_capture_health = self
            .last_capture_health_log_at
            .map(|last| last.elapsed() >= Duration::from_secs(2))
            .unwrap_or_else(|| self.created_at.elapsed() >= Duration::from_secs(2));
        if should_log_capture_health {
            warn!(
                "Runtime host awaiting first live frame: frame={} in_flight={} enqueue_failed={} capture_path={}",
                frame,
                self.capture_request_in_flight,
                self.screenshot_enqueue_failed,
                self.capture_path.display()
            );
            self.last_capture_health_log_at = Some(Instant::now());
        }

        let fallback_bytes = if let Some(bytes) = self.fallback_frame_png.as_ref() {
            bytes.clone()
        } else {
            let (generated, generated_luma) = Self::build_procedural_fallback_png();
            self.fallback_frame_luma = generated_luma;
            let generated = generated.unwrap_or_else(|| {
                // 1x1 opaque black PNG
                vec![
                    137, 80, 78, 71, 13, 10, 26, 10, 0, 0, 0, 13, 73, 72, 68, 82, 0, 0, 0, 1, 0, 0,
                    0, 1, 8, 6, 0, 0, 0, 31, 21, 196, 137, 0, 0, 0, 13, 73, 68, 65, 84, 120, 156,
                    99, 96, 96, 96, 248, 15, 0, 1, 4, 1, 0, 95, 161, 122, 86, 0, 0, 0, 0, 73, 69,
                    78, 68, 174, 66, 96, 130,
                ]
            });
            self.fallback_frame_png = Some(generated.clone());
            generated
        };
        if let Err(err) = fs::write(&self.frame_path, &fallback_bytes) {
            warn!(
                "Failed writing GPUI runtime fallback frame {:?}: {err}",
                self.frame_path
            );
        } else {
            let _ = fs::write(
                &self.frame_meta_path,
                format!("luma={:.3}\n", self.fallback_frame_luma),
            );
        }
    }

    pub(super) fn promote_capture_frame_if_ready(&mut self) {
        if !Self::png_file_looks_usable(&self.capture_path) {
            return;
        }
        if let Err(err) = fs::rename(&self.capture_path, &self.frame_path) {
            warn!(
                "Failed to promote GPUI runtime capture {:?} -> {:?}: {err}",
                self.capture_path, self.frame_path
            );
            self.capture_request_in_flight = false;
            self.capture_request_started_at = None;
            return;
        }
        self.capture_request_in_flight = false;
        self.capture_request_started_at = None;
        if !self.has_published_live_frame {
            info!(
                "Runtime host promoted first live frame from capture (frame={})",
                self.last_published_frame
            );
        }
        self.has_published_live_frame = true;
        let _ = fs::write(&self.frame_meta_path, "luma=0.0\n");
    }

    pub(super) fn png_file_looks_usable(path: &Path) -> bool {
        let Ok(metadata) = fs::metadata(path) else {
            return false;
        };
        if metadata.len() < 128 {
            return false;
        }
        let mut signature = [0u8; 8];
        let Ok(mut file) = fs::File::open(path) else {
            return false;
        };
        if file.read_exact(&mut signature).is_err() {
            return false;
        }
        signature == [137, 80, 78, 71, 13, 10, 26, 10]
    }

    pub(super) fn load_fallback_frame_png() -> (Option<Vec<u8>>, f32) {
        let candidates = [
            "Data/English/Art/Textures/loadpageuserinterface.tga",
            "Data/English/Art/Textures/TitleScreenuserinterface.tga",
            "MapsZH/Maps/ShellMapMD/ShellMapMD.tga",
        ];

        // Use the mounted game file system first (C++ W3DFileSystem semantics).
        {
            let fs = game_engine::common::system::file_system::get_file_system();
            let fs_guard_result = fs.lock();
            if let Ok(mut fs_guard) = fs_guard_result {
                for candidate in candidates {
                    if let Some(mut file) = fs_guard.open_file(
                        candidate,
                        game_engine::common::system::file::FileAccess::READ
                            .combine(game_engine::common::system::file::FileAccess::BINARY),
                    ) {
                        let Ok(bytes) = file.read_entire_and_close() else {
                            continue;
                        };
                        let Ok(image) = image::load_from_memory(&bytes) else {
                            continue;
                        };
                        let rgba = image.to_rgba8();
                        let luma = if rgba.is_empty() {
                            0.0
                        } else {
                            let sum = rgba
                                .chunks_exact(4)
                                .map(|px| {
                                    0.2126 * px[0] as f32 / 255.0
                                        + 0.7152 * px[1] as f32 / 255.0
                                        + 0.0722 * px[2] as f32 / 255.0
                                })
                                .sum::<f32>();
                            (sum / (rgba.len() as f32 / 4.0)).clamp(0.0, 1.0) * 255.0
                        };
                        let mut png_bytes = Vec::new();
                        let mut cursor = std::io::Cursor::new(&mut png_bytes);
                        if image.write_to(&mut cursor, image::ImageFormat::Png).is_ok() {
                            return (Some(png_bytes), luma);
                        }
                    }
                }
            }
        }

        // Final local fallback: try plain filesystem copies.
        for candidate in candidates {
            let Ok(bytes) = fs::read(candidate) else {
                continue;
            };
            let Ok(image) = image::load_from_memory(&bytes) else {
                continue;
            };
            let rgba = image.to_rgba8();
            let luma = if rgba.is_empty() {
                0.0
            } else {
                let sum = rgba
                    .chunks_exact(4)
                    .map(|px| {
                        0.2126 * px[0] as f32 / 255.0
                            + 0.7152 * px[1] as f32 / 255.0
                            + 0.0722 * px[2] as f32 / 255.0
                    })
                    .sum::<f32>();
                (sum / (rgba.len() as f32 / 4.0)).clamp(0.0, 1.0) * 255.0
            };
            let mut png_bytes = Vec::new();
            let mut cursor = std::io::Cursor::new(&mut png_bytes);
            if image.write_to(&mut cursor, image::ImageFormat::Png).is_ok() {
                return (Some(png_bytes), luma);
            }
        }
        Self::build_procedural_fallback_png()
    }

    pub(super) fn build_procedural_fallback_png() -> (Option<Vec<u8>>, f32) {
        let width = 1280u32;
        let height = 720u32;
        let mut rgba = image::RgbaImage::new(width, height);
        for y in 0..height {
            let v = y as f32 / (height.saturating_sub(1).max(1)) as f32;
            for x in 0..width {
                let u = x as f32 / (width.saturating_sub(1).max(1)) as f32;
                let r = (22.0 + 26.0 * (1.0 - v) + 12.0 * u).clamp(0.0, 255.0) as u8;
                let g = (34.0 + 38.0 * (1.0 - v)).clamp(0.0, 255.0) as u8;
                let b = (48.0 + 58.0 * v).clamp(0.0, 255.0) as u8;
                rgba.put_pixel(x, y, image::Rgba([r, g, b, 255]));
            }
        }

        let mut png_bytes = Vec::new();
        let mut cursor = std::io::Cursor::new(&mut png_bytes);
        if image::DynamicImage::ImageRgba8(rgba)
            .write_to(&mut cursor, image::ImageFormat::Png)
            .is_ok()
        {
            return (Some(png_bytes), 96.0);
        }
        (None, 0.0)
    }
}

#[cfg(test)]
mod clicker_aim_status_tests {
    use super::format_clicker_aim_status;

    #[test]
    fn format_clicker_aim_status_contains_window_and_gadget_keys() {
        let hits = vec![
            "ButtonSinglePlayer@120,80".to_string(),
            "ButtonSkirmish@120,140".to_string(),
            "ButtonStart@200,400".to_string(),
            "ButtonOk@240,420".to_string(),
        ];
        let status = format_clicker_aim_status(64, 48, 1280, 720, &hits);
        assert!(
            status.contains("window_outer_x=64")
                && status.contains("window_outer_y=48")
                && status.contains("window_outer_w=1280")
                && status.contains("window_outer_h=720"),
            "status must contain window_outer_* keys when provided: {status}"
        );
        assert!(
            status.contains("gadget_hit=ButtonSinglePlayer@120,80")
                && status.contains("gadget_hit=ButtonSkirmish@120,140")
                && status.contains("gadget_hit=ButtonStart@200,400")
                && status.contains("gadget_hit=ButtonOk@240,420"),
            "status must contain gadget_hit=Name@x,y when provided: {status}"
        );
        assert!(
            !status.contains("playable_claim")
                && !status.contains("window_visible=")
                && !status.contains("wnd_widget_tree_nav=")
                && !status.contains("live_frame_ok="),
            "clicker-aim fragment must not write playable_claim or five-flag evidence: {status}"
        );
    }

    #[test]
    fn format_clicker_aim_status_omits_empty_gadget_hits() {
        let status = format_clicker_aim_status(0, 0, 0, 0, &[]);
        assert!(status.contains("window_outer_x=0"));
        assert!(status.contains("window_outer_y=0"));
        assert!(status.contains("window_outer_w=0"));
        assert!(status.contains("window_outer_h=0"));
        assert!(
            !status.contains("gadget_hit="),
            "empty gadget list must omit gadget_hit keys: {status}"
        );
    }
}
