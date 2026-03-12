use std::time::{Duration, Instant};

use integration::{IntegrationConfig, IntegrationSystem};
use tokio::time::sleep;
use ww3d_engine::FrameTiming;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    let mut system = IntegrationSystem::with_config(IntegrationConfig::default()).await?;
    system.initialize().await?;

    let mut frame_number = 0u64;
    let mut total_time = Duration::ZERO;
    let mut previous_sync_time = 0u32;
    let mut since_last_sample = Duration::ZERO;

    for _ in 0..600 {
        frame_number += 1;
        let delta_time = Duration::from_millis(16);
        total_time += delta_time;
        let sync_time = total_time.as_millis().min(u32::MAX as u128) as u32;
        let timing = FrameTiming {
            frame_number,
            delta_time,
            total_time,
            fps: (1.0 / delta_time.as_secs_f32()).max(0.0),
            frame_start: Instant::now(),
            sync_time,
            previous_sync_time,
        };
        previous_sync_time = sync_time;
        system.update(&timing).await?;
        since_last_sample += timing.delta_time;

        if since_last_sample >= Duration::from_secs(1) {
            since_last_sample = Duration::ZERO;

            if let Some(perf) = system.latest_performance_sample() {
                println!(
                    "Frame {:>5}: {:.1} FPS ({:.2} ms) | Stability {:.1}% | Bottleneck: {}",
                    perf.frame_number,
                    perf.graphics.fps,
                    perf.graphics.frametime_ms,
                    perf.overall.stability,
                    perf.overall.bottleneck.as_deref().unwrap_or("none")
                );
            }

            if let Some(usage) = system.latest_resource_usage() {
                println!(
                    "  Assets: {:>5} | Cache {:.1} MB ({:.0}%)",
                    usage.loaded_assets,
                    usage.cache_memory_mb as f32,
                    (usage.cache_memory_mb as f32 / usage.total_memory_mb.max(1) as f32 * 100.0)
                        .clamp(0.0, 100.0)
                );
            }

            if let Some(diag) = system.latest_diagnostics() {
                println!(
                    "  Health {:.1}% | Engine {:.1}% Graphics {:.1}% | Errors {}",
                    diag.health_score,
                    diag.subsystem_health.engine,
                    diag.subsystem_health.graphics,
                    diag.error_counts.errors
                );
            }
        }

        // Simulate real time passing for readability when running the example.
        sleep(Duration::from_millis(5)).await;
    }

    system.shutdown().await?;
    Ok(())
}
