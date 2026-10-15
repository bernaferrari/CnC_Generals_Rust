//! One display owner joins completed Client update, camera, CPU Draw and GPU admission.
use super::display_clock::{DisplayAdmission, DisplayInputs, VisualClock};
use super::*;

#[cfg(feature = "game_client")]
use game_client::core::game_client::{
    NativeDrawPhase, NativeMaterialObservation, NativeParticleFrames, PreparedPresentationShell,
};

#[cfg(feature = "game_client")]
struct PendingShell {
    step: PreparedPresentationShell,
    facts: Vec<NativeMaterialObservation>,
    epoch: u64,
    client_frame: u32,
}

pub(super) struct NativeDisplayState {
    clock: VisualClock,
    completed: Option<DisplayAdmission>,
    guardband: [f32; 2],
    epoch: u64,
    #[cfg(feature = "game_client")]
    pending: Option<PendingShell>,
    failed_shell: Option<String>,
}
impl NativeDisplayState {
    pub(super) const fn new() -> Self {
        Self {
            clock: VisualClock::new(),
            completed: None,
            guardband: [0.; 2],
            epoch: 1,
            #[cfg(feature = "game_client")]
            pending: None,
            failed_shell: None,
        }
    }
    pub(super) fn admit(&mut self, input: DisplayInputs) -> DisplayAdmission {
        let decision = self.clock.admit(input);
        self.completed = Some(decision);
        decision
    }
    pub(super) fn should_present(&self) -> bool {
        self.completed.is_some_and(|completed| completed.draw_gpu)
    }
    pub(super) fn reset_world(&mut self, epoch: u64) {
        #[cfg(feature = "game_client")]
        if self.pending.take().is_some() {
            log::warn!(
                "World replacement explicitly cancels the preceding world's pending Client display continuation"
            );
        }
        // W3DView::reset clears guardband, while W3DDisplay's static clock lives on.
        self.epoch = epoch;
        self.guardband = [0.; 2];
        self.completed = None;
        self.failed_shell = None;
    }
    pub(super) fn fail_shell(&mut self, error: impl Into<String>) {
        self.completed = None;
        self.failed_shell.get_or_insert_with(|| error.into());
    }
    fn check_shell_before_display(&mut self) -> Result<(), String> {
        if let Some(error) = self.failed_shell.take() {
            Err(error)
        } else {
            Ok(())
        }
    }
    #[cfg(feature = "game_client")]
    pub(super) fn require_shell_slot(&self) -> Result<(), String> {
        if self.pending.is_some() || self.failed_shell.is_some() {
            Err("uncompleted native display shell cannot be overwritten".into())
        } else {
            Ok(())
        }
    }
    #[cfg(feature = "game_client")]
    pub(super) fn queue_shell(
        &mut self,
        step: PreparedPresentationShell,
        facts: Vec<NativeMaterialObservation>,
        epoch: u64,
        client_frame: u32,
    ) -> Result<(), String> {
        self.require_shell_slot()?;
        self.pending = Some(PendingShell {
            step,
            facts,
            epoch,
            client_frame,
        });
        Ok(())
    }
    #[cfg(feature = "game_client")]
    fn receive_guardband(&mut self, epoch: u64, bias: [f32; 2]) -> Result<(), String> {
        if epoch != self.epoch {
            return Err("native guardband belongs to a different world".into());
        }
        if !bias.iter().all(|value| value.is_finite()) {
            return Err("native view guardband is nonfinite".into());
        }
        self.guardband = bias;
        Ok(())
    }
    #[cfg(feature = "game_client")]
    fn cpu_phase(decision: DisplayAdmission) -> NativeDrawPhase {
        NativeDrawPhase {
            draw_drawables: decision.draw_drawables,
            update_particles: decision.update_particles,
        }
    }
    fn suppress_failed_gpu(&mut self) {
        if let Some(completed) = &mut self.completed {
            completed.draw_gpu = false;
        }
    }
}

#[cfg(feature = "game_client")]
#[path = "native_display_view.rs"]
mod view;

impl CnCGameEngine {
    /// Invoked once after update and before the optional GPU scheduling decision.
    pub(super) fn complete_native_display_step(&mut self) -> Result<(), String> {
        self.native_display.completed = None;
        self.native_display.check_shell_before_display()?;
        #[cfg(feature = "game_client")]
        if self
            .native_display
            .pending
            .as_ref()
            .is_some_and(|pending| !pending.step.display_requested())
        {
            let pending = self
                .native_display
                .pending
                .take()
                .expect("checked no-Draw continuation");
            if pending.epoch != self.host_direct_visual_world_epoch
                || pending.client_frame != self.game_client.get_frame()
            {
                return Err("cancelled no-Draw continuation from a different world/frame".into());
            }
            // GameClient returned before Display::draw. Do not offer the
            // clock, Sync, device poll, camera update or GPU attempt.
            return self
                .game_client
                .finish_presentation_shell_with_native_views(
                    pending.step,
                    &pending.facts,
                    &[],
                    NativeDrawPhase {
                        draw_drawables: false,
                        update_particles: false,
                    },
                )
                .map_err(|error| error.to_string());
        }
        let frozen = self.presentation_or_boot_time_frozen()
            || self.game_paused
            || matches!(self.current_state, GameState::Paused);
        let tivo = game_engine::common::global_data::read_safe()
            .map_err(|error| format!("display configuration unavailable: {error}"))?
            .tivo_fast_mode;
        #[cfg(feature = "game_client")]
        let client_frame = self.game_client.get_frame();
        #[cfg(not(feature = "game_client"))]
        let client_frame = self.host_match_logic_frame.unwrap_or(0);
        // WGPU owns a valid device from GraphicsSystem construction. Poll is
        // nonblocking and does not acquire/present a surface; headless GPU
        // throttling is independent from this CPU service eligibility check.
        let device_ready = self
            .graphics_system
            .device()
            .poll(wgpu::PollType::Poll)
            .is_ok();
        let decision = self.native_display.admit(DisplayInputs {
            client_frame,
            frozen,
            time_fast: self.script_time_fast(),
            multiplier: self.presentation_or_boot_visual_speed().max(1.) as i32,
            device_ready,
            tivo_replay: tivo
                && self.presentation_or_boot_in_replay_game()
                && !self.game_paused
                && !matches!(self.current_state, GameState::Paused),
            replay_frame: self.game_logic.frame,
        });
        if decision.sync {
            WW3D::sync(decision.sync_time_ms);
        }
        let result = self.finish_native_display_after_admission(decision);
        if result.is_err() {
            self.native_display.suppress_failed_gpu();
            #[cfg(feature = "game_client")]
            if self.native_display.pending.take().is_some() {
                log::error!(
                    "Failed display explicitly cancels its uncompleted Client continuation; no fallback Draw is executed"
                );
            }
        }
        result
    }

    fn finish_native_display_after_admission(
        &mut self,
        decision: DisplayAdmission,
    ) -> Result<(), String> {
        // Script camera requests and movement already ran in the owning update.
        // Commit matrices without advancing those effects for a second time.
        if decision.update_views || decision.fast_camera_only {
            self.finalize_native_view_camera();
        }
        #[cfg(feature = "game_client")]
        {
            let current_frame = self.game_client.get_frame();
            if self.native_display.pending.as_ref().is_some_and(|pending| {
                pending.epoch != self.host_direct_visual_world_epoch
                    || pending.client_frame != current_frame
            }) {
                return Err("pending Client display belongs to a different world/frame".into());
            }
            if self.native_display.pending.is_none() {
                self.host_sync_presentation_direct_drawables(
                    self.presentation_or_boot_time_frozen() || self.game_paused,
                )?;
            }
            let views = if decision.draw_drawables {
                let frame = self
                    .last_presentation_frame
                    .as_ref()
                    .ok_or("native Draw lacks a completed presentation frame")?;
                let (x, y) = frame
                    .view_guardband
                    .ok_or("native Draw lacks owning view guardband")?;
                self.native_display
                    .receive_guardband(self.host_direct_visual_world_epoch, [x, y])?;
                let terrain = self
                    .game_logic
                    .world_services
                    .terrain()
                    .read()
                    .map_err(|_| "owning terrain extent is poisoned")?;
                let extent = terrain.get_extent();
                let (width, height) = self.tactical_viewport_size();
                vec![view::native_view_from_camera(
                    self.view_matrix,
                    self.projection_matrix,
                    width,
                    height,
                    self.native_display.guardband,
                    [extent.lo.z, extent.hi.z],
                )?]
            } else {
                Vec::new()
            };
            let phase = NativeDisplayState::cpu_phase(decision);
            let pending = self.native_display.pending.take();
            if let Some(pending) = pending {
                self.game_client
                    .finish_presentation_shell_with_native_views(
                        pending.step,
                        &pending.facts,
                        &views,
                        phase,
                    )
                    .map_err(|error| error.to_string())?;
            } else {
                let facts = self.host_prepare_presentation_materials()?;
                self.game_client
                    .draw_native_views_with_frames(
                        NativeParticleFrames {
                            logic_frame: self.game_logic.frame,
                            client_frame: current_frame,
                        },
                        &facts,
                        &views,
                        phase,
                    )
                    .map_err(|error| error.to_string())?;
            }
        }
        Ok(())
    }

    /// Shared final camera commitment. Rendering consumes these exact matrices.
    fn finalize_native_view_camera(&mut self) {
        if matches!(
            self.current_state,
            GameState::InGame | GameState::Paused | GameState::Menu
        ) {
            self.apply_camera_orbit_transform();
        }
        self.host_ensure_presentation_frame_for_render();
        if matches!(self.current_state, GameState::InGame | GameState::Paused) {
            let target = self.camera_target;
            if target.x * target.x + target.z * target.z < 80. * 80. {
                self.snap_camera_to_local_units_if_needed();
            }
        }
        let (width, height) = super::types::render_surface_extent(&self.window);
        self.render_pipeline.set_tactical_3d_viewport(
            width as f32,
            height as f32,
            self.tactical_view_height_frac(),
        );
        self.rebuild_tactical_projection();
    }
}

#[cfg(all(test, feature = "game_client"))]
#[path = "native_display_tests.rs"]
mod tests;
