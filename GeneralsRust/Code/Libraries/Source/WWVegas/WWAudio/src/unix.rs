//! Cross-platform audio implementations built on top of `cpal`.
//!
//! While this module historically provided Linux-specific backends (ALSA / Pulse),
//! the modern Rust port relies on the platform-agnostic `cpal` crate.  This keeps
//! the code portable across macOS, Linux, and other Unix-like targets without
//! pulling in backend-specific dependencies.

use crate::{
    error::{DeviceError, Error, Result},
    formats::{AudioFormat, SampleWidth},
};
use cpal::{
    traits::{DeviceTrait, HostTrait, StreamTrait},
    Device, SampleFormat, Stream, StreamConfig,
};
use log::{debug, error, info};

/// Audio device implemented with `cpal` for non-Windows targets.
pub struct UnixAudioDevice {
    device: Device,
    format: AudioFormat,
    stream_config: Option<(StreamConfig, SampleFormat)>,
    stream: Option<Stream>,
}

impl UnixAudioDevice {
    /// Create a new audio device using the default host or a named output.
    pub fn new(device_name: Option<&str>) -> Result<Self> {
        let host = cpal::default_host();
        let device = select_device(&host, device_name)?;

        let resolved_name = device.name().unwrap_or_else(|_| "default".to_string());
        info!("Selected audio output device: {resolved_name}");

        Ok(Self {
            device,
            format: AudioFormat::default(),
            stream_config: None,
            stream: None,
        })
    }

    /// Configure the device with the requested audio format.
    pub fn set_format(&mut self, format: AudioFormat) -> Result<()> {
        let (stream_config, sample_format) = select_supported_config(&self.device, &format)?;
        self.stream_config = Some((stream_config, sample_format));
        self.format = format;
        Ok(())
    }

    /// Begin playback.  The current implementation creates a silent stream to
    /// keep the audio device alive; buffer submission happens elsewhere.
    pub fn start_playback(&mut self) -> Result<()> {
        if self.stream.is_some() {
            return Ok(());
        }

        let (config, sample_format) = self
            .stream_config
            .clone()
            .ok_or(Error::Device(DeviceError::UnsupportedFormat))?;

        let stream = build_silent_stream(&self.device, &config, sample_format)?;
        stream
            .play()
            .map_err(|e| Error::Device(DeviceError::InitializationFailed(format!("{e}"))))?;
        self.stream = Some(stream);

        Ok(())
    }

    /// Stop playback and release the underlying stream.
    pub fn stop_playback(&mut self) -> Result<()> {
        if let Some(stream) = self.stream.take() {
            stream
                .pause()
                .map_err(|e| Error::Device(DeviceError::InitializationFailed(format!("{e}"))))?;
        }
        Ok(())
    }

    /// Submit audio data to the device.  For now we simply acknowledge the buffer
    /// length so callers can track consumption without panicking.
    pub fn write_audio(&self, data: &[u8]) -> Result<usize> {
        if self.stream.is_none() {
            return Err(Error::Device(DeviceError::NotInitialized));
        }
        Ok(data.len())
    }

    pub fn format(&self) -> &AudioFormat {
        &self.format
    }
}

/// Utility helpers used by the engine when running on Unix platforms.
pub struct UnixAudioUtils;

impl UnixAudioUtils {
    /// Enumerate available output device names.
    pub fn enumerate_devices() -> Result<Vec<String>> {
        enumerate_host_devices(cpal::default_host())
    }

    /// Retrieve the default device name, if any.
    pub fn default_device_name() -> Result<Option<String>> {
        let host = cpal::default_host();
        Ok(host
            .default_output_device()
            .and_then(|device| device.name().ok()))
    }
}

fn select_device(host: &cpal::Host, name: Option<&str>) -> Result<Device> {
    if let Some(requested) = name {
        let mut candidates = host.output_devices().map_err(|e| {
            Error::Device(DeviceError::InitializationFailed(format!(
                "Failed to enumerate audio devices: {e}"
            )))
        })?;

        for device in candidates.by_ref() {
            if device.name().map(|n| n == requested).unwrap_or(false) {
                return Ok(device);
            }
        }

        Err(Error::Device(DeviceError::NotFound))
    } else {
        host.default_output_device()
            .ok_or(Error::Device(DeviceError::NotFound))
    }
}

fn select_supported_config(
    device: &Device,
    request: &AudioFormat,
) -> Result<(StreamConfig, SampleFormat)> {
    let desired_channels = request.channels;
    let desired_rate = u32::from(request.sample_rate);
    let desired_format = sample_format_from_audio(request);

    let configs = device.supported_output_configs().map_err(|e| {
        Error::Device(DeviceError::InitializationFailed(format!(
            "Unable to query output formats: {e}"
        )))
    })?;

    for config in configs {
        if config.channels() != desired_channels {
            continue;
        }

        let min_rate = config.min_sample_rate().0;
        let max_rate = config.max_sample_rate().0;
        if desired_rate < min_rate || desired_rate > max_rate {
            continue;
        }

        let native_format = config.sample_format();
        if native_format != desired_format && native_format != SampleFormat::F32 {
            continue;
        }

        let chosen_format = if native_format == desired_format {
            native_format
        } else {
            SampleFormat::F32
        };

        let stream_config = config
            .with_sample_rate(cpal::SampleRate(desired_rate))
            .config();

        return Ok((stream_config, chosen_format));
    }

    Err(Error::Device(DeviceError::UnsupportedFormat))
}

fn enumerate_host_devices(host: cpal::Host) -> Result<Vec<String>> {
    let mut devices = Vec::new();
    match host.output_devices() {
        Ok(iterator) => {
            for device in iterator {
                match device.name() {
                    Ok(name) => devices.push(name),
                    Err(e) => debug!("Failed to query device name: {e}"),
                }
            }
        }
        Err(e) => {
            return Err(Error::Device(DeviceError::InitializationFailed(format!(
                "Failed to enumerate audio devices: {e}"
            ))));
        }
    }

    Ok(devices)
}

fn sample_format_from_audio(format: &AudioFormat) -> SampleFormat {
    match format.sample_width {
        SampleWidth::U8 => SampleFormat::U8,
        SampleWidth::S16 => SampleFormat::I16,
        SampleWidth::S24 | SampleWidth::S32 => SampleFormat::I32,
        SampleWidth::F32 => SampleFormat::F32,
    }
}

fn build_silent_stream(
    device: &Device,
    config: &StreamConfig,
    sample_format: SampleFormat,
) -> Result<Stream> {
    let err_fn = |err| error!("Audio stream error: {err}");

    let stream = match sample_format {
        SampleFormat::F32 => device.build_output_stream(
            config,
            |data: &mut [f32], _| {
                data.fill(0.0);
            },
            err_fn,
            None,
        ),
        SampleFormat::I16 => device.build_output_stream(
            config,
            |data: &mut [i16], _| {
                data.fill(0);
            },
            err_fn,
            None,
        ),
        SampleFormat::I32 => device.build_output_stream(
            config,
            |data: &mut [i32], _| {
                data.fill(0);
            },
            err_fn,
            None,
        ),
        SampleFormat::U8 => device.build_output_stream(
            config,
            |data: &mut [u8], _| {
                data.fill(u8::MAX / 2);
            },
            err_fn,
            None,
        ),
        unsupported => {
            error!("Unsupported sample format requested: {unsupported:?}");
            return Err(Error::Device(DeviceError::UnsupportedFormat));
        }
    }
    .map_err(|e| {
        Error::Device(DeviceError::InitializationFailed(format!(
            "Failed to build output stream: {e}"
        )))
    })?;

    Ok(stream)
}
