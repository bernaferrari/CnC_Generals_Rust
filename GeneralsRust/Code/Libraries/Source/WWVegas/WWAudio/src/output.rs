//! CPAL output glue that mixes audio directly from the `AudioMixer`.
//!
//! Miles mixes on a dedicated render thread that the OS audio stack drives. To emulate that
//! behaviour we spawn a lightweight helper thread that owns the CPAL stream and keeps feeding the
//! mixer whenever the host requests more audio. Render metrics are pushed back to the game thread
//! so timing and diagnostics mirror the original engine.

use crate::{
    error::{DeviceError, Error, Result},
    formats::AudioFormat,
    mixer::{AudioMixer, MixBuffer, MixRenderStats, MixerTimelineSnapshot},
};
use cpal::{
    traits::{DeviceTrait, HostTrait, StreamTrait},
    Device, SampleFormat, Stream, StreamConfig,
};
use crossbeam_channel::{bounded, Receiver, Sender};
use log::{debug, error};
use std::{
    sync::{
        mpsc::{self, Sender as ShutdownSender},
        Arc,
    },
    thread::{self, JoinHandle},
};

/// Render metrics captured on the audio callback thread.
#[derive(Debug, Clone, Copy)]
pub struct AudioRenderMetrics {
    pub stats: MixRenderStats,
    pub snapshot: MixerTimelineSnapshot,
}

/// CPAL output stream controller.
pub struct CpalOutput {
    metrics_rx: Receiver<AudioRenderMetrics>,
    shutdown: Option<ShutdownSender<()>>,
    thread: Option<JoinHandle<()>>,
}

impl CpalOutput {
    pub fn new(format: &AudioFormat, buffer_frames: usize, mixer: Arc<AudioMixer>) -> Result<Self> {
        let format = format.clone();
        let (metrics_tx, metrics_rx) = bounded::<AudioRenderMetrics>(buffer_frames.max(4));
        let (shutdown_tx, shutdown_rx) = mpsc::channel();
        let (init_tx, init_rx) = mpsc::channel();

        let handle = thread::Builder::new()
            .name("wp-audio-cpal".to_string())
            .spawn(move || {
                let init_result: Result<Stream> = (|| {
                    let host = cpal::default_host();
                    let device = host
                        .default_output_device()
                        .ok_or(Error::Device(DeviceError::NotFound))?;
                    let (stream_config, sample_format) = select_stream_config(&device, &format)?;
                    build_stream(
                        device,
                        stream_config,
                        sample_format,
                        mixer,
                        buffer_frames,
                        metrics_tx,
                    )
                })();

                match init_result {
                    Ok(stream) => {
                        if let Err(err) = stream.play() {
                            let _ = init_tx.send(Err(Error::Device(
                                DeviceError::InitializationFailed(format!(
                                    "Failed to start CPAL stream: {err}"
                                )),
                            )));
                            return;
                        }
                        let _ = init_tx.send(Ok(()));
                        let _ = shutdown_rx.recv();
                        drop(stream);
                    }
                    Err(err) => {
                        let _ = init_tx.send(Err(err));
                    }
                }
            })
            .map_err(|e| {
                Error::Device(DeviceError::InitializationFailed(format!(
                    "Failed to spawn CPAL output thread: {e}"
                )))
            })?;

        match init_rx.recv() {
            Ok(Ok(())) => Ok(Self {
                metrics_rx,
                shutdown: Some(shutdown_tx),
                thread: Some(handle),
            }),
            Ok(Err(err)) => {
                let _ = shutdown_tx.send(());
                if let Err(join_err) = handle.join() {
                    error!("CPAL init thread panicked: {join_err:?}");
                }
                Err(err)
            }
            Err(recv_err) => Err(Error::Device(DeviceError::InitializationFailed(format!(
                "Failed to initialise CPAL stream: {recv_err}"
            )))),
        }
    }

    /// Drain the most recent render metrics produced by the audio callback.
    pub fn drain_metrics(&self) -> Option<AudioRenderMetrics> {
        let mut latest = None;
        while let Ok(metrics) = self.metrics_rx.try_recv() {
            latest = Some(metrics);
        }
        latest
    }
}

impl Drop for CpalOutput {
    fn drop(&mut self) {
        if let Some(tx) = self.shutdown.take() {
            let _ = tx.send(());
        }
        if let Some(handle) = self.thread.take() {
            if let Err(err) = handle.join() {
                error!("CPAL thread join failed: {err:?}");
            }
        }
    }
}

fn select_stream_config(
    device: &Device,
    format: &AudioFormat,
) -> Result<(StreamConfig, SampleFormat)> {
    let desired_channels = format.channels;
    let desired_rate = u32::from(format.sample_rate);

    let supported_configs = device.supported_output_configs().map_err(|e| {
        Error::Device(DeviceError::InitializationFailed(format!(
            "Unable to query output formats: {e}"
        )))
    })?;

    for config in supported_configs {
        if config.channels() != desired_channels {
            continue;
        }
        let min_rate = config.min_sample_rate().0;
        let max_rate = config.max_sample_rate().0;
        if desired_rate < min_rate || desired_rate > max_rate {
            continue;
        }

        let mut sample_format = config.sample_format();
        if sample_format != SampleFormat::F32 {
            sample_format = SampleFormat::F32;
        }

        let mut stream_config = config
            .with_sample_rate(cpal::SampleRate(desired_rate))
            .config();
        stream_config.channels = desired_channels;
        return Ok((stream_config, sample_format));
    }

    let default_config = device.default_output_config().map_err(|e| {
        Error::Device(DeviceError::InitializationFailed(format!(
            "Failed to obtain default output format: {e}"
        )))
    })?;

    let mut stream_config = default_config.config();
    stream_config.channels = desired_channels;
    stream_config.sample_rate = cpal::SampleRate(desired_rate);
    let sample_format = SampleFormat::F32;
    Ok((stream_config, sample_format))
}

fn build_stream(
    device: Device,
    config: StreamConfig,
    sample_format: SampleFormat,
    mixer: Arc<AudioMixer>,
    buffer_frames: usize,
    metrics_tx: Sender<AudioRenderMetrics>,
) -> Result<Stream> {
    let err_fn = |err| error!("CPAL stream error: {err}");
    let channels = config.channels as usize;
    let sample_rate = config.sample_rate.0;

    match sample_format {
        SampleFormat::F32 => {
            let mut mix_buffer = MixBuffer::new(config.channels, buffer_frames, sample_rate);
            device
                .build_output_stream(
                    &config,
                    move |output: &mut [f32], _| {
                        let frames = if channels == 0 {
                            0
                        } else {
                            output.len() / channels
                        };
                        if frames == 0 {
                            output.fill(0.0);
                            return;
                        }

                        if mix_buffer.frames != frames {
                            mix_buffer.frames = frames;
                            mix_buffer.data.resize(frames.saturating_mul(channels), 0.0);
                        }
                        if mix_buffer.channels != config.channels {
                            mix_buffer.channels = config.channels;
                        }
                        if mix_buffer.sample_rate != sample_rate {
                            mix_buffer.sample_rate = sample_rate;
                        }

                        let stats = mixer.render_into(&mut mix_buffer);
                        let samples = mix_buffer.interleaved_samples();
                        let to_copy = samples.len().min(output.len());
                        output[..to_copy].copy_from_slice(&samples[..to_copy]);
                        if to_copy < output.len() {
                            output[to_copy..].fill(0.0);
                        }

                        let metrics = AudioRenderMetrics {
                            stats,
                            snapshot: mixer.timeline_snapshot(),
                        };
                        let _ = metrics_tx.try_send(metrics);
                    },
                    err_fn,
                    None,
                )
                .map_err(|e| Error::Device(DeviceError::InitializationFailed(format!("{e}"))))
        }
        other => {
            debug!(
                "Falling back to f32 mixing for unsupported format {:?}",
                other
            );
            build_stream(
                device,
                config,
                SampleFormat::F32,
                mixer,
                buffer_frames,
                metrics_tx,
            )
        }
    }
}
