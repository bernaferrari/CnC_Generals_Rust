#![cfg(feature = "integration-diagnostics")]

use anyhow::Result;
use game_engine::common::frame_clock::{FrameClock, FrameTiming as ClockFrameTiming};
use integration::{IntegrationConfig, IntegrationSystem};
use log::info;
use std::time::Instant;
use ww3d_engine::FrameTiming;

use crate::cnc_game_engine::CnCGameEngine;

/// Drives the C++-faithful `CnCGameEngine` using telemetry produced by the
/// async `IntegrationSystem` (WW3D renderer/logic pipeline).
///
/// When the `integration-diagnostics` feature is enabled we link against the
/// `integration` crate and mirror the same timings/diagnostics the C++ tools
/// see.  `IntegrationTelemetryBridge` orchestrates this by running the
/// integration loop with the shared `FrameClock`, feeding each frame's
/// `SystemDiagnostics` back into the game engine so the egui debug overlay
/// presents authoritative subsystem health.
pub struct IntegrationTelemetryBridge {
    integration: IntegrationSystem,
    frame_clock: FrameClock,
}

impl IntegrationTelemetryBridge {
    fn to_engine_timing(timing: ClockFrameTiming) -> FrameTiming {
        let sync_time = timing.total_time.as_millis().min(u32::MAX as u128) as u32;
        let delta_ms = timing.delta_time.as_millis().min(u32::MAX as u128) as u32;
        let fps = if timing.delta_time.is_zero() {
            0.0
        } else {
            1.0 / timing.delta_time.as_secs_f32()
        };

        FrameTiming {
            frame_number: timing.frame_number,
            delta_time: timing.delta_time,
            total_time: timing.total_time,
            fps,
            frame_start: Instant::now(),
            sync_time,
            previous_sync_time: sync_time.saturating_sub(delta_ms),
        }
    }

    /// Creates a new bridge, initializes the integration system, and resets
    /// the shared WW3D clock.
    pub async fn new(config: IntegrationConfig) -> Result<Self> {
        let mut integration = IntegrationSystem::with_config(config).await?;
        integration.initialize().await?;
        info!("IntegrationTelemetryBridge: integration system initialized");
        Ok(Self {
            integration,
            frame_clock: FrameClock::new(),
        })
    }

    /// Runs one integration tick using a caller-provided WW3D `FrameTiming`,
    /// pushes any diagnostics sample into the `CnCGameEngine`, and keeps the
    /// telemetry overlay in sync with the integration pipeline.
    pub async fn pump_with_timing(
        &mut self,
        engine: &mut CnCGameEngine,
        timing: FrameTiming,
    ) -> Result<()> {
        self.integration.update(&timing).await?;

        if let Some(diag) = self.integration.latest_diagnostics() {
            engine.set_integration_diagnostics(&diag);
        } else {
            engine.clear_diagnostics_overlay();
        }

        Ok(())
    }

    /// Convenience helper that advances the internal `FrameClock`, then calls
    /// [`Self::pump_with_timing`]. Useful for offline tools or tests that
    /// aren't already driven by WW3D's timing source.
    pub async fn pump(&mut self, engine: &mut CnCGameEngine) -> Result<FrameTiming> {
        let timing = Self::to_engine_timing(self.frame_clock.next_frame());
        self.pump_with_timing(engine, timing).await?;
        Ok(timing)
    }

    /// Shuts down the integration system.
    pub async fn shutdown(mut self) -> Result<()> {
        info!("IntegrationTelemetryBridge: shutting down integration system");
        self.integration.shutdown().await
    }
}
