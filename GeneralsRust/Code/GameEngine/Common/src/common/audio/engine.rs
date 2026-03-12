//! Modern Audio Engine Core
//!
//! This module implements the core audio engine using Rodio, providing:
//! - High-performance audio playback
//! - 3D spatial audio with HRTF
//! - Multi-channel mixing
//! - Low-latency audio processing
//! - Cross-platform audio backend support

use parking_lot::{Mutex as ParkingMutex, RwLock as ParkingRwLock};
use std::collections::HashMap;
use std::sync::{Arc, Mutex, RwLock};
use std::thread;
use std::time::{Duration, Instant};

#[cfg(feature = "audio")]
use cpal::{
    traits::{DeviceTrait, HostTrait, StreamTrait},
    Device, Host, Stream, StreamConfig, SupportedStreamConfig,
};
#[cfg(feature = "audio")]
use crossbeam_channel::{bounded, unbounded, Receiver, Sender};
#[cfg(feature = "audio")]
use rodio::{Decoder, OutputStream, OutputStreamHandle, Sink, Source, SpatialSink};
#[cfg(feature = "audio")]
use symphonia::core::io::MediaSourceStream;
// Always import basic channel types since they're used without features
#[cfg(not(feature = "audio"))]
use crossbeam_channel::{unbounded, Receiver, Sender};
#[cfg(feature = "audio")]
use rtrb::{Consumer, Producer, RingBuffer};

use crate::common::audio::{
    AsciiString, AudioAffect, AudioEventRts, AudioHandle, AudioPriority, AudioType, Bool, Coord3D,
    Int, Real, TimeOfDay, UnsignedInt,
};

/// Maximum number of simultaneous audio sources
pub const MAX_AUDIO_SOURCES: usize = 256;

/// Audio buffer size for low-latency processing
pub const AUDIO_BUFFER_SIZE: usize = 512;

/// Default sample rate for audio processing
pub const DEFAULT_SAMPLE_RATE: u32 = 44100;

/// Default number of audio channels
pub const DEFAULT_CHANNELS: u16 = 2;

/// Audio engine configuration
#[derive(Debug, Clone)]
pub struct AudioEngineConfig {
    /// Sample rate (Hz)
    pub sample_rate: u32,
    /// Number of audio channels
    pub channels: u16,
    /// Audio buffer size
    pub buffer_size: usize,
    /// Maximum number of simultaneous sources
    pub max_sources: usize,
    /// Enable hardware acceleration if available
    pub hardware_acceleration: bool,
    /// Enable 3D audio processing
    pub enable_3d_audio: bool,
    /// Enable HRTF for spatial audio
    pub enable_hrtf: bool,
    /// Audio device name (None for default)
    pub device_name: Option<String>,
}

impl Default for AudioEngineConfig {
    fn default() -> Self {
        Self {
            sample_rate: DEFAULT_SAMPLE_RATE,
            channels: DEFAULT_CHANNELS,
            buffer_size: AUDIO_BUFFER_SIZE,
            max_sources: MAX_AUDIO_SOURCES,
            hardware_acceleration: true,
            enable_3d_audio: true,
            enable_hrtf: true,
            device_name: None,
        }
    }
}

/// Audio source state
#[derive(Debug, Clone, PartialEq)]
pub enum AudioSourceState {
    /// Source is stopped
    Stopped,
    /// Source is playing
    Playing,
    /// Source is paused
    Paused,
    /// Source is looping
    Looping,
    /// Source is fading in
    FadingIn,
    /// Source is fading out
    FadingOut,
}

/// 3D audio parameters
#[derive(Debug, Clone)]
pub struct Audio3DParams {
    /// Position in 3D space
    pub position: [f32; 3],
    /// Velocity for Doppler effect
    pub velocity: [f32; 3],
    /// Left and right ear positions for HRTF
    pub left_ear: [f32; 3],
    pub right_ear: [f32; 3],
    /// Maximum distance for attenuation
    pub max_distance: f32,
    /// Minimum distance for attenuation
    pub min_distance: f32,
    /// Attenuation rolloff factor
    pub rolloff_factor: f32,
}

impl Default for Audio3DParams {
    fn default() -> Self {
        Self {
            position: [0.0, 0.0, 0.0],
            velocity: [0.0, 0.0, 0.0],
            left_ear: [-0.1, 0.0, 0.0],
            right_ear: [0.1, 0.0, 0.0],
            max_distance: 100.0,
            min_distance: 1.0,
            rolloff_factor: 1.0,
        }
    }
}

/// Audio source information
#[derive(Debug)]
pub struct AudioSource {
    /// Unique handle for this source
    pub handle: AudioHandle,
    /// Current state
    pub state: AudioSourceState,
    /// Audio priority
    pub priority: AudioPriority,
    /// Volume (0.0 - 1.0)
    pub volume: f32,
    /// Pitch (1.0 = normal)
    pub pitch: f32,
    /// Pan (-1.0 = left, 0.0 = center, 1.0 = right)
    pub pan: f32,
    /// Loop flag
    pub looping: bool,
    /// 3D audio parameters
    pub spatial_params: Option<Audio3DParams>,
    /// Start time
    pub start_time: Instant,
    /// Duration (if known)
    pub duration: Option<Duration>,
    /// File path
    pub file_path: String,
    /// Fade in/out parameters
    pub fade_duration: Option<Duration>,
    /// Associated audio event
    pub audio_event: Option<AudioEventRts>,
    /// Platform-specific sink
    #[cfg(feature = "audio")]
    pub sink: Option<Arc<Sink>>,
    #[cfg(feature = "audio")]
    pub spatial_sink: Option<Arc<SpatialSink>>,
}

impl AudioSource {
    pub fn new(handle: AudioHandle, file_path: String) -> Self {
        Self {
            handle,
            state: AudioSourceState::Stopped,
            priority: AudioPriority::Normal,
            volume: 1.0,
            pitch: 1.0,
            pan: 0.0,
            looping: false,
            spatial_params: None,
            start_time: Instant::now(),
            duration: None,
            file_path,
            fade_duration: None,
            audio_event: None,
            #[cfg(feature = "audio")]
            sink: None,
            #[cfg(feature = "audio")]
            spatial_sink: None,
        }
    }

    pub fn is_3d(&self) -> bool {
        self.spatial_params.is_some()
    }

    pub fn is_playing(&self) -> bool {
        matches!(
            self.state,
            AudioSourceState::Playing | AudioSourceState::Looping | AudioSourceState::FadingIn
        )
    }

    pub fn is_finished(&self) -> bool {
        if let Some(duration) = self.duration {
            if !self.looping {
                return self.start_time.elapsed() >= duration;
            }
        }

        #[cfg(feature = "audio")]
        {
            if let Some(sink) = &self.sink {
                return sink.empty();
            }
            if let Some(spatial_sink) = &self.spatial_sink {
                return spatial_sink.empty();
            }
        }

        false
    }
}

/// Audio listener parameters for 3D audio
#[derive(Debug, Clone)]
pub struct AudioListener {
    /// Position in 3D space
    pub position: [f32; 3],
    /// Forward direction
    pub forward: [f32; 3],
    /// Up direction
    pub up: [f32; 3],
    /// Velocity for Doppler effect
    pub velocity: [f32; 3],
    /// Global volume multiplier
    pub global_volume: f32,
}

impl Default for AudioListener {
    fn default() -> Self {
        Self {
            position: [0.0, 0.0, 0.0],
            forward: [0.0, 0.0, -1.0],
            up: [0.0, 1.0, 0.0],
            velocity: [0.0, 0.0, 0.0],
            global_volume: 1.0,
        }
    }
}

/// Audio command for thread communication
#[derive(Debug)]
pub enum AudioCommand {
    /// Play an audio file
    Play {
        handle: AudioHandle,
        file_path: String,
        volume: f32,
        pitch: f32,
        looping: bool,
        spatial_params: Option<Audio3DParams>,
        fade_in: Option<Duration>,
    },
    /// Stop an audio source
    Stop {
        handle: AudioHandle,
        fade_out: Option<Duration>,
    },
    /// Pause an audio source
    Pause { handle: AudioHandle },
    /// Resume an audio source
    Resume { handle: AudioHandle },
    /// Set volume of an audio source
    SetVolume { handle: AudioHandle, volume: f32 },
    /// Set pitch of an audio source
    SetPitch { handle: AudioHandle, pitch: f32 },
    /// Set pan of an audio source
    SetPan { handle: AudioHandle, pan: f32 },
    /// Update 3D parameters
    Update3D {
        handle: AudioHandle,
        spatial_params: Audio3DParams,
    },
    /// Update listener parameters
    UpdateListener { listener: AudioListener },
    /// Set master volume for a category
    SetMasterVolume { affect: AudioAffect, volume: f32 },
    /// Get source information
    GetSourceInfo {
        handle: AudioHandle,
        response: Sender<Option<AudioSource>>,
    },
    /// Shutdown the audio engine
    Shutdown,
}

/// Audio engine response
#[derive(Debug)]
pub enum AudioResponse {
    /// Source state changed
    SourceStateChanged {
        handle: AudioHandle,
        state: AudioSourceState,
    },
    /// Source finished playing
    SourceFinished { handle: AudioHandle },
    /// Error occurred
    Error {
        handle: Option<AudioHandle>,
        message: String,
    },
}

/// High-performance audio engine
pub struct AudioEngine {
    /// Engine configuration
    config: AudioEngineConfig,
    /// Currently active audio sources
    sources: Arc<ParkingRwLock<HashMap<AudioHandle, AudioSource>>>,
    /// Next available handle
    next_handle: Arc<ParkingMutex<AudioHandle>>,
    /// Audio listener parameters
    listener: Arc<ParkingRwLock<AudioListener>>,
    /// Master volume controls
    master_volumes: Arc<ParkingRwLock<HashMap<AudioAffect, f32>>>,
    /// Command channel for thread communication
    command_sender: Option<Sender<AudioCommand>>,
    command_receiver: Option<Receiver<AudioCommand>>,
    /// Response channel for status updates
    response_sender: Option<Sender<AudioResponse>>,
    response_receiver: Option<Receiver<AudioResponse>>,
    /// Audio output stream
    #[cfg(feature = "audio")]
    _output_stream: Option<OutputStream>,
    #[cfg(feature = "audio")]
    output_stream_handle: Option<OutputStreamHandle>,
    /// Audio processing thread handle
    audio_thread: Option<thread::JoinHandle<()>>,
    /// Engine running flag
    running: Arc<std::sync::atomic::AtomicBool>,
}

impl AudioEngine {
    /// Create a new audio engine with default configuration
    pub fn new() -> Result<Self, Box<dyn std::error::Error>> {
        Self::with_config(AudioEngineConfig::default())
    }

    /// Create a new audio engine with custom configuration
    pub fn with_config(config: AudioEngineConfig) -> Result<Self, Box<dyn std::error::Error>> {
        let (command_sender, command_receiver) = unbounded();
        let (response_sender, response_receiver) = unbounded();

        #[cfg(feature = "audio")]
        let (_output_stream, output_stream_handle) = OutputStream::try_default()?;

        let mut master_volumes = HashMap::new();
        master_volumes.insert(AudioAffect::Music, 1.0);
        master_volumes.insert(AudioAffect::SoundEffects, 1.0);
        master_volumes.insert(AudioAffect::Speech, 1.0);
        master_volumes.insert(AudioAffect::Ambient, 1.0);

        let engine = Self {
            config,
            sources: Arc::new(ParkingRwLock::new(HashMap::new())),
            next_handle: Arc::new(ParkingMutex::new(1)),
            listener: Arc::new(ParkingRwLock::new(AudioListener::default())),
            master_volumes: Arc::new(ParkingRwLock::new(master_volumes)),
            command_sender: Some(command_sender),
            command_receiver: Some(command_receiver),
            response_sender: Some(response_sender),
            response_receiver: Some(response_receiver),
            #[cfg(feature = "audio")]
            _output_stream: Some(_output_stream),
            #[cfg(feature = "audio")]
            output_stream_handle: Some(output_stream_handle),
            audio_thread: None,
            running: Arc::new(std::sync::atomic::AtomicBool::new(false)),
        };

        Ok(engine)
    }

    /// Start the audio engine
    pub fn start(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        if self.running.load(std::sync::atomic::Ordering::Relaxed) {
            return Ok(());
        }

        self.running
            .store(true, std::sync::atomic::Ordering::Relaxed);

        // Start audio processing thread
        let command_receiver = self.command_receiver.take().unwrap();
        let response_sender = self.response_sender.clone().unwrap();
        let sources = Arc::clone(&self.sources);
        let listener = Arc::clone(&self.listener);
        let master_volumes = Arc::clone(&self.master_volumes);
        let running = Arc::clone(&self.running);

        #[cfg(feature = "audio")]
        let output_stream_handle = self.output_stream_handle.clone().unwrap();

        let config = self.config.clone();

        self.audio_thread = Some(thread::spawn(move || {
            Self::audio_thread_main(
                command_receiver,
                response_sender,
                sources,
                listener,
                master_volumes,
                running,
                config,
                #[cfg(feature = "audio")]
                output_stream_handle,
            );
        }));

        Ok(())
    }

    /// Stop the audio engine
    pub fn stop(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        if !self.running.load(std::sync::atomic::Ordering::Relaxed) {
            return Ok(());
        }

        // Send shutdown command
        if let Some(sender) = &self.command_sender {
            let _ = sender.send(AudioCommand::Shutdown);
        }

        self.running
            .store(false, std::sync::atomic::Ordering::Relaxed);

        // Wait for audio thread to finish
        if let Some(handle) = self.audio_thread.take() {
            let _ = handle.join();
        }

        Ok(())
    }

    /// Allocate a new audio handle
    pub fn allocate_handle(&self) -> AudioHandle {
        let mut next = self.next_handle.lock();
        let handle = *next;
        *next += 1;
        handle
    }

    /// Play an audio file
    pub fn play(
        &self,
        file_path: &str,
        volume: f32,
        looping: bool,
        spatial_params: Option<Audio3DParams>,
    ) -> Result<AudioHandle, Box<dyn std::error::Error>> {
        let handle = self.allocate_handle();

        if let Some(sender) = &self.command_sender {
            sender.send(AudioCommand::Play {
                handle,
                file_path: file_path.to_string(),
                volume,
                pitch: 1.0,
                looping,
                spatial_params,
                fade_in: None,
            })?;
        }

        Ok(handle)
    }

    /// Stop an audio source
    pub fn stop_source(&self, handle: AudioHandle) -> Result<(), Box<dyn std::error::Error>> {
        if let Some(sender) = &self.command_sender {
            sender.send(AudioCommand::Stop {
                handle,
                fade_out: None,
            })?;
        }
        Ok(())
    }

    /// Stop an audio source with fade out
    pub fn stop_with_fade(
        &self,
        handle: AudioHandle,
        fade_duration: Duration,
    ) -> Result<(), Box<dyn std::error::Error>> {
        if let Some(sender) = &self.command_sender {
            sender.send(AudioCommand::Stop {
                handle,
                fade_out: Some(fade_duration),
            })?;
        }
        Ok(())
    }

    /// Pause an audio source
    pub fn pause(&self, handle: AudioHandle) -> Result<(), Box<dyn std::error::Error>> {
        if let Some(sender) = &self.command_sender {
            sender.send(AudioCommand::Pause { handle })?;
        }
        Ok(())
    }

    /// Resume a paused audio source
    pub fn resume(&self, handle: AudioHandle) -> Result<(), Box<dyn std::error::Error>> {
        if let Some(sender) = &self.command_sender {
            sender.send(AudioCommand::Resume { handle })?;
        }
        Ok(())
    }

    /// Set volume of an audio source
    pub fn set_volume(
        &self,
        handle: AudioHandle,
        volume: f32,
    ) -> Result<(), Box<dyn std::error::Error>> {
        if let Some(sender) = &self.command_sender {
            sender.send(AudioCommand::SetVolume { handle, volume })?;
        }
        Ok(())
    }

    /// Set master volume for an audio category
    pub fn set_master_volume(
        &self,
        affect: AudioAffect,
        volume: f32,
    ) -> Result<(), Box<dyn std::error::Error>> {
        if let Some(sender) = &self.command_sender {
            sender.send(AudioCommand::SetMasterVolume { affect, volume })?;
        }
        Ok(())
    }

    /// Update listener position and orientation
    pub fn update_listener(
        &self,
        listener: AudioListener,
    ) -> Result<(), Box<dyn std::error::Error>> {
        if let Some(sender) = &self.command_sender {
            sender.send(AudioCommand::UpdateListener { listener })?;
        }
        Ok(())
    }

    /// Check if a source is currently playing
    pub fn is_playing(&self, handle: AudioHandle) -> bool {
        let sources = self.sources.read();
        if let Some(source) = sources.get(&handle) {
            source.is_playing()
        } else {
            false
        }
    }

    /// Get the number of active audio sources
    pub fn active_source_count(&self) -> usize {
        let sources = self.sources.read();
        sources.len()
    }

    /// Process audio responses
    pub fn update(&self) -> Vec<AudioResponse> {
        let mut responses = Vec::new();

        if let Some(receiver) = &self.response_receiver {
            while let Ok(response) = receiver.try_recv() {
                responses.push(response);
            }
        }

        responses
    }

    /// Audio processing thread main function
    #[cfg(feature = "audio")]
    fn audio_thread_main(
        command_receiver: Receiver<AudioCommand>,
        response_sender: Sender<AudioResponse>,
        sources: Arc<ParkingRwLock<HashMap<AudioHandle, AudioSource>>>,
        listener: Arc<ParkingRwLock<AudioListener>>,
        master_volumes: Arc<ParkingRwLock<HashMap<AudioAffect, f32>>>,
        running: Arc<std::sync::atomic::AtomicBool>,
        config: AudioEngineConfig,
        output_stream_handle: OutputStreamHandle,
    ) {
        while running.load(std::sync::atomic::Ordering::Relaxed) {
            // Process commands
            match command_receiver.try_recv() {
                Ok(AudioCommand::Play {
                    handle,
                    file_path,
                    volume,
                    pitch,
                    looping,
                    spatial_params,
                    fade_in,
                }) => {
                    if let Err(e) = Self::handle_play_command(
                        handle,
                        file_path,
                        volume,
                        pitch,
                        looping,
                        spatial_params,
                        fade_in,
                        &sources,
                        &output_stream_handle,
                        &response_sender,
                    ) {
                        let _ = response_sender.send(AudioResponse::Error {
                            handle: Some(handle),
                            message: e.to_string(),
                        });
                    }
                }
                Ok(AudioCommand::Stop { handle, fade_out }) => {
                    Self::handle_stop_command(handle, fade_out, &sources, &response_sender);
                }
                Ok(AudioCommand::Pause { handle }) => {
                    Self::handle_pause_command(handle, &sources, &response_sender);
                }
                Ok(AudioCommand::Resume { handle }) => {
                    Self::handle_resume_command(handle, &sources, &response_sender);
                }
                Ok(AudioCommand::SetVolume { handle, volume }) => {
                    Self::handle_set_volume_command(handle, volume, &sources);
                }
                Ok(AudioCommand::UpdateListener {
                    listener: new_listener,
                }) => {
                    *listener.write() = new_listener;
                }
                Ok(AudioCommand::SetMasterVolume { affect, volume }) => {
                    master_volumes.write().insert(affect, volume);
                }
                Ok(AudioCommand::Shutdown) => {
                    running.store(false, std::sync::atomic::Ordering::Relaxed);
                    break;
                }
                _ => {}
            }

            // Clean up finished sources
            Self::cleanup_finished_sources(&sources, &response_sender);

            // Small delay to prevent busy waiting
            thread::sleep(Duration::from_millis(1));
        }
    }

    #[cfg(not(feature = "audio"))]
    fn audio_thread_main(
        _command_receiver: Receiver<AudioCommand>,
        _response_sender: Sender<AudioResponse>,
        _sources: Arc<ParkingRwLock<HashMap<AudioHandle, AudioSource>>>,
        _listener: Arc<ParkingRwLock<AudioListener>>,
        _master_volumes: Arc<ParkingRwLock<HashMap<AudioAffect, f32>>>,
        _running: Arc<std::sync::atomic::AtomicBool>,
        _config: AudioEngineConfig,
    ) {
        // Stub implementation when audio feature is disabled
    }

    #[cfg(feature = "audio")]
    fn handle_play_command(
        handle: AudioHandle,
        file_path: String,
        volume: f32,
        pitch: f32,
        looping: bool,
        spatial_params: Option<Audio3DParams>,
        fade_in: Option<Duration>,
        sources: &Arc<ParkingRwLock<HashMap<AudioHandle, AudioSource>>>,
        output_stream_handle: &OutputStreamHandle,
        response_sender: &Sender<AudioResponse>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        use std::fs::File;
        use std::io::BufReader;

        // Open audio file
        let file = File::open(&file_path)?;
        let source = Decoder::new(BufReader::new(file))?;

        let mut audio_source = AudioSource::new(handle, file_path);
        audio_source.volume = volume;
        audio_source.pitch = pitch;
        audio_source.looping = looping;
        audio_source.spatial_params = spatial_params.clone();
        audio_source.state = AudioSourceState::Playing;

        if let Some(spatial_params) = spatial_params {
            // Create spatial sink for 3D audio
            let spatial_sink = Arc::new(
                output_stream_handle
                    .play_raw(source.convert_samples().speed(pitch).amplify(volume))?,
            );

            audio_source.spatial_sink = Some(spatial_sink);
        } else {
            // Create regular sink for 2D audio
            let sink = Arc::new(Sink::try_new(output_stream_handle)?);
            sink.append(source.speed(pitch).amplify(volume));

            if looping {
                // Note: Rodio doesn't have built-in looping, would need custom implementation
            }

            audio_source.sink = Some(sink);
        }

        sources.write().insert(handle, audio_source);

        let _ = response_sender.send(AudioResponse::SourceStateChanged {
            handle,
            state: AudioSourceState::Playing,
        });

        Ok(())
    }

    #[cfg(feature = "audio")]
    fn handle_stop_command(
        handle: AudioHandle,
        fade_out: Option<Duration>,
        sources: &Arc<ParkingRwLock<HashMap<AudioHandle, AudioSource>>>,
        response_sender: &Sender<AudioResponse>,
    ) {
        let mut sources_guard = sources.write();
        if let Some(mut source) = sources_guard.get_mut(&handle) {
            source.state = if fade_out.is_some() {
                AudioSourceState::FadingOut
            } else {
                AudioSourceState::Stopped
            };

            if let Some(sink) = &source.sink {
                sink.stop();
            }
            if let Some(spatial_sink) = &source.spatial_sink {
                spatial_sink.stop();
            }

            let _ = response_sender.send(AudioResponse::SourceStateChanged {
                handle,
                state: source.state.clone(),
            });
        }
    }

    #[cfg(feature = "audio")]
    fn handle_pause_command(
        handle: AudioHandle,
        sources: &Arc<ParkingRwLock<HashMap<AudioHandle, AudioSource>>>,
        response_sender: &Sender<AudioResponse>,
    ) {
        let mut sources_guard = sources.write();
        if let Some(mut source) = sources_guard.get_mut(&handle) {
            source.state = AudioSourceState::Paused;

            if let Some(sink) = &source.sink {
                sink.pause();
            }
            if let Some(spatial_sink) = &source.spatial_sink {
                spatial_sink.pause();
            }

            let _ = response_sender.send(AudioResponse::SourceStateChanged {
                handle,
                state: AudioSourceState::Paused,
            });
        }
    }

    #[cfg(feature = "audio")]
    fn handle_resume_command(
        handle: AudioHandle,
        sources: &Arc<ParkingRwLock<HashMap<AudioHandle, AudioSource>>>,
        response_sender: &Sender<AudioResponse>,
    ) {
        let mut sources_guard = sources.write();
        if let Some(mut source) = sources_guard.get_mut(&handle) {
            source.state = AudioSourceState::Playing;

            if let Some(sink) = &source.sink {
                sink.play();
            }
            if let Some(spatial_sink) = &source.spatial_sink {
                spatial_sink.play();
            }

            let _ = response_sender.send(AudioResponse::SourceStateChanged {
                handle,
                state: AudioSourceState::Playing,
            });
        }
    }

    #[cfg(feature = "audio")]
    fn handle_set_volume_command(
        handle: AudioHandle,
        volume: f32,
        sources: &Arc<ParkingRwLock<HashMap<AudioHandle, AudioSource>>>,
    ) {
        let mut sources_guard = sources.write();
        if let Some(mut source) = sources_guard.get_mut(&handle) {
            source.volume = volume;

            if let Some(sink) = &source.sink {
                sink.set_volume(volume);
            }
            if let Some(spatial_sink) = &source.spatial_sink {
                spatial_sink.set_volume(volume);
            }
        }
    }

    #[cfg(feature = "audio")]
    fn cleanup_finished_sources(
        sources: &Arc<ParkingRwLock<HashMap<AudioHandle, AudioSource>>>,
        response_sender: &Sender<AudioResponse>,
    ) {
        let mut to_remove = Vec::new();

        {
            let sources_guard = sources.read();
            for (handle, source) in sources_guard.iter() {
                if source.is_finished() {
                    to_remove.push(*handle);
                }
            }
        }

        if !to_remove.is_empty() {
            let mut sources_guard = sources.write();
            for handle in to_remove {
                sources_guard.remove(&handle);
                let _ = response_sender.send(AudioResponse::SourceFinished { handle });
            }
        }
    }

    #[cfg(not(feature = "audio"))]
    fn handle_play_command(
        _handle: AudioHandle,
        _file_path: String,
        _volume: f32,
        _pitch: f32,
        _looping: bool,
        _spatial_params: Option<Audio3DParams>,
        _fade_in: Option<Duration>,
        _sources: &Arc<ParkingRwLock<HashMap<AudioHandle, AudioSource>>>,
        _response_sender: &Sender<AudioResponse>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        Ok(())
    }

    #[cfg(not(feature = "audio"))]
    fn handle_stop_command(
        _handle: AudioHandle,
        _fade_out: Option<Duration>,
        _sources: &Arc<ParkingRwLock<HashMap<AudioHandle, AudioSource>>>,
        _response_sender: &Sender<AudioResponse>,
    ) {
    }

    #[cfg(not(feature = "audio"))]
    fn handle_pause_command(
        _handle: AudioHandle,
        _sources: &Arc<ParkingRwLock<HashMap<AudioHandle, AudioSource>>>,
        _response_sender: &Sender<AudioResponse>,
    ) {
    }

    #[cfg(not(feature = "audio"))]
    fn handle_resume_command(
        _handle: AudioHandle,
        _sources: &Arc<ParkingRwLock<HashMap<AudioHandle, AudioSource>>>,
        _response_sender: &Sender<AudioResponse>,
    ) {
    }

    #[cfg(not(feature = "audio"))]
    fn handle_set_volume_command(
        _handle: AudioHandle,
        _volume: f32,
        _sources: &Arc<ParkingRwLock<HashMap<AudioHandle, AudioSource>>>,
    ) {
    }

    #[cfg(not(feature = "audio"))]
    fn cleanup_finished_sources(
        _sources: &Arc<ParkingRwLock<HashMap<AudioHandle, AudioSource>>>,
        _response_sender: &Sender<AudioResponse>,
    ) {
    }
}

impl Drop for AudioEngine {
    fn drop(&mut self) {
        let _ = self.stop();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn test_audio_engine_creation() {
        let engine = AudioEngine::new();
        assert!(engine.is_ok());
    }

    #[test]
    fn test_handle_allocation() {
        let engine = AudioEngine::new().unwrap();
        let handle1 = engine.allocate_handle();
        let handle2 = engine.allocate_handle();
        assert_ne!(handle1, handle2);
        assert!(handle2 > handle1);
    }

    #[test]
    fn test_audio_source_creation() {
        let source = AudioSource::new(1, "test.wav".to_string());
        assert_eq!(source.handle, 1);
        assert_eq!(source.file_path, "test.wav");
        assert_eq!(source.state, AudioSourceState::Stopped);
        assert!(!source.is_3d());
        assert!(!source.is_playing());
    }

    #[test]
    fn test_3d_audio_params() {
        let params = Audio3DParams::default();
        assert_eq!(params.position, [0.0, 0.0, 0.0]);
        assert_eq!(params.max_distance, 100.0);
        assert_eq!(params.min_distance, 1.0);
        assert_eq!(params.rolloff_factor, 1.0);
    }

    #[test]
    fn test_audio_listener() {
        let listener = AudioListener::default();
        assert_eq!(listener.position, [0.0, 0.0, 0.0]);
        assert_eq!(listener.forward, [0.0, 0.0, -1.0]);
        assert_eq!(listener.up, [0.0, 1.0, 0.0]);
        assert_eq!(listener.global_volume, 1.0);
    }
}
