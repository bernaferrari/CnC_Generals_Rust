//! rodio 0.17-shaped types implemented with rodio 0.22.
//!
//! Existing game audio owns `OutputStream` on a dedicated thread and talks to
//! `Sink` / `SpatialSink` through a handle. rodio 0.22 renamed those to
//! `MixerDeviceSink` / `Mixer` / `Player` / `SpatialPlayer`.

pub use rodio::{Decoder, Source};

use core::num::NonZero;

/// rodio 0.17-shaped constructor: channels/sample_rate as integers.
pub fn samples_buffer<D>(channels: u16, sample_rate: u32, data: D) -> rodio::buffer::SamplesBuffer
where
    D: Into<Vec<rodio::Sample>>,
{
    let channels = NonZero::new(channels.max(1)).expect("channels");
    let sample_rate = NonZero::new(sample_rate.max(1)).expect("sample_rate");
    rodio::buffer::SamplesBuffer::new(channels, sample_rate, data)
}

pub type SamplesBuffer = rodio::buffer::SamplesBuffer;

use rodio::mixer::Mixer;
use rodio::{DeviceSinkBuilder, MixerDeviceSink, Player, SpatialPlayer};
use std::io::{Read, Seek};
use std::time::Duration;

pub type StreamError = rodio::DeviceSinkError;

pub struct OutputStream {
    sink: std::sync::Arc<MixerDeviceSink>,
}

#[derive(Clone)]
pub struct OutputStreamHandle {
    sink: std::sync::Arc<MixerDeviceSink>,
}

impl OutputStream {
    pub fn try_default() -> Result<(Self, OutputStreamHandle), StreamError> {
        let mut device = DeviceSinkBuilder::open_default_sink()?;
        device.log_on_drop(false);
        let sink = std::sync::Arc::new(device);
        Ok((
            Self {
                sink: sink.clone(),
            },
            OutputStreamHandle { sink },
        ))
    }
}

impl OutputStreamHandle {
    pub fn mixer(&self) -> &Mixer {
        self.sink.mixer()
    }
}

pub struct Sink {
    player: Player,
}

impl Sink {
    pub fn try_new(handle: &OutputStreamHandle) -> Result<Self, StreamError> {
        Ok(Self {
            player: Player::connect_new(handle.mixer()),
        })
    }

    pub fn append<S>(&self, source: S)
    where
        S: Source + Send + 'static,
    {
        self.player.append(source);
    }

    pub fn stop(&self) {
        self.player.stop();
    }

    pub fn pause(&self) {
        self.player.pause();
    }

    pub fn play(&self) {
        self.player.play();
    }

    pub fn set_volume(&self, value: f32) {
        self.player.set_volume(value);
    }

    pub fn set_speed(&self, value: f32) {
        self.player.set_speed(value);
    }

    pub fn detach(self) {
        self.player.detach();
    }

    pub fn empty(&self) -> bool {
        self.player.empty()
    }

    pub fn is_paused(&self) -> bool {
        self.player.is_paused()
    }

    pub fn sleep_until_end(&self) {
        self.player.sleep_until_end();
    }
}

pub struct SpatialSink {
    player: SpatialPlayer,
}

impl SpatialSink {
    pub fn try_new(
        handle: &OutputStreamHandle,
        emitter_position: [f32; 3],
        left_ear: [f32; 3],
        right_ear: [f32; 3],
    ) -> Result<Self, StreamError> {
        Ok(Self {
            player: SpatialPlayer::connect_new(
                handle.mixer(),
                emitter_position,
                left_ear,
                right_ear,
            ),
        })
    }

    pub fn append<S>(&self, source: S)
    where
        S: Source + Send + 'static,
    {
        self.player.append(source);
    }

    pub fn set_emitter_position(&self, pos: [f32; 3]) {
        self.player.set_emitter_position(pos);
    }

    pub fn set_left_ear_position(&self, pos: [f32; 3]) {
        self.player.set_left_ear_position(pos);
    }

    pub fn set_right_ear_position(&self, pos: [f32; 3]) {
        self.player.set_right_ear_position(pos);
    }

    pub fn set_volume(&self, value: f32) {
        self.player.set_volume(value);
    }

    pub fn set_speed(&self, value: f32) {
        self.player.set_speed(value);
    }

    pub fn play(&self) {
        self.player.play();
    }

    pub fn pause(&self) {
        self.player.pause();
    }

    pub fn empty(&self) -> bool {
        self.player.empty()
    }

    pub fn is_paused(&self) -> bool {
        self.player.is_paused()
    }

    pub fn stop(&self) {
        self.player.stop();
    }

    pub fn detach(self) {
        let _ = self;
    }
}

pub fn play_once<R>(handle: &OutputStreamHandle, input: R) -> Result<Sink, rodio::PlayError>
where
    R: Read + Seek + Send + Sync + 'static,
{
    let player = rodio::play(handle.mixer(), input)?;
    Ok(Sink { player })
}

pub fn sleep(duration: Duration) {
    std::thread::sleep(duration);
}
