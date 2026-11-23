//! Decoded movie audio on the receiving application's existing output.

#[cfg(not(target_arch = "wasm32"))]
use rodio::mixer::Mixer;
#[cfg(not(target_arch = "wasm32"))]
use rodio_compat::{OutputStreamHandle, Sink, samples_buffer};

/// One ordered PCM queue. Dropping it cancels only this clip.
pub struct MoviePcmVoice {
    #[cfg(not(target_arch = "wasm32"))]
    mixer: Mixer,
    #[cfg(not(target_arch = "wasm32"))]
    sink: Option<Sink>,
    #[cfg(not(target_arch = "wasm32"))]
    sample_rate: u32,
    #[cfg(not(target_arch = "wasm32"))]
    channels: u16,
    #[cfg(not(target_arch = "wasm32"))]
    gain: f32,
}

impl MoviePcmVoice {
    #[cfg(not(target_arch = "wasm32"))]
    pub(super) fn new(mixer: Mixer, sample_rate: u32, channels: u8, gain: f32) -> Self {
        Self {
            mixer,
            sink: None,
            sample_rate,
            channels: u16::from(channels),
            gain,
        }
    }

    /// Submit complete interleaved sample frames in decode order.
    pub fn enqueue(&mut self, mut samples: Vec<f32>) {
        #[cfg(not(target_arch = "wasm32"))]
        {
            samples
                .truncate(samples.len() / usize::from(self.channels) * usize::from(self.channels));
            if samples.is_empty() {
                return;
            }
            let sink = self.sink.get_or_insert_with(|| {
                let sink = Sink::connect_new(&self.mixer);
                sink.set_volume(self.gain);
                sink
            });
            sink.append(samples_buffer(self.channels, self.sample_rate, samples));
        }
        #[cfg(target_arch = "wasm32")]
        samples.clear();
    }

    pub fn set_gain(&mut self, gain: f32) {
        #[cfg(not(target_arch = "wasm32"))]
        {
            self.gain = gain;
            if let Some(sink) = self.sink.as_ref() {
                sink.set_volume(gain);
            }
        }
        #[cfg(target_arch = "wasm32")]
        let _ = gain;
    }

    pub fn has_pending_samples(&self) -> bool {
        #[cfg(not(target_arch = "wasm32"))]
        return self.sink.as_ref().is_some_and(|sink| !sink.empty());
        #[cfg(target_arch = "wasm32")]
        false
    }

    pub fn discard(&mut self) {
        // A stopped Rodio queue must be drained by its callback before it can
        // accept another append. A seek opens a fresh voice on the same mixer.
        #[cfg(not(target_arch = "wasm32"))]
        drop(self.sink.take());
    }
}

#[cfg(not(target_arch = "wasm32"))]
pub(super) struct MoviePcmOutput {
    pub(super) mixer: Mixer,
    _device: Option<OutputStreamHandle>,
}

#[cfg(not(target_arch = "wasm32"))]
impl MoviePcmOutput {
    pub(super) fn from_device(device: OutputStreamHandle) -> Self {
        Self {
            mixer: device.mixer().clone(),
            _device: Some(device),
        }
    }

    pub(super) fn from_mixer(mixer: Mixer) -> Self {
        Self {
            mixer,
            _device: None,
        }
    }
}
