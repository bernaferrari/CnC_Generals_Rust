//! Core audio mixer scaffolding providing a Miles-style mixing surface.
//!
//! This module does not perform real sample mixing yet, but it models the data
//! flow and control surfaces required to reproduce the original Miles behaviour.
//! Higher-level systems enqueue voices through the mixer, which tracks the
//! requested parameters so a future backend can render them deterministically.

use crate::{
    math::{Matrix3D, Vector3},
    AudioSample, AudioSource,
};
use crossbeam_channel::{unbounded, Receiver, Sender, TryRecvError};
use log::{debug, trace, warn};
use std::{
    collections::{hash_map::Entry, HashMap, VecDeque},
    f32::EPSILON,
    fmt,
    sync::{Arc, Mutex},
    time::Duration,
};

const SPEED_OF_SOUND_METERS_PER_SEC: f32 = 343.0;

/// Identifier for mixer voices. Mirrors the legacy Miles handle semantics.
pub type VoiceId = u32;

/// Strongly typed mixer handle that guards against stale references.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct VoiceHandle {
    id: VoiceId,
    generation: u32,
}

impl VoiceHandle {
    /// Construct a new handle.
    pub const fn new(id: VoiceId, generation: u32) -> Self {
        Self { id, generation }
    }

    /// Identifier portion of the handle.
    pub const fn id(self) -> VoiceId {
        self.id
    }

    /// Generation counter ensuring handles cannot be reused accidentally.
    pub const fn generation(self) -> u32 {
        self.generation
    }

    /// Whether this handle refers to an allocated voice.
    pub const fn is_valid(self) -> bool {
        self.id != 0
    }
}

/// Spatialisation mode identifying how a voice should be positioned.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VoiceSpatialMode {
    None,
    Pseudo3D,
    Full3D,
}

/// Parameters describing the spatial relationship between the voice and listener.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct VoiceSpatialParams {
    pub mode: VoiceSpatialMode,
    pub position: Vector3,
    pub velocity: Vector3,
    pub listener_position: Vector3,
    pub listener_velocity: Vector3,
    pub listener_transform: Matrix3D,
    pub min_distance: f32,
    pub max_distance: f32,
}

impl VoiceSpatialParams {
    pub const fn new(mode: VoiceSpatialMode) -> Self {
        Self {
            mode,
            position: Vector3::ZERO,
            velocity: Vector3::ZERO,
            listener_position: Vector3::ZERO,
            listener_velocity: Vector3::ZERO,
            listener_transform: Matrix3D::IDENTITY,
            min_distance: 0.0,
            max_distance: 1.0,
        }
    }
}

impl Default for VoiceSpatialParams {
    fn default() -> Self {
        Self::new(VoiceSpatialMode::None)
    }
}

#[derive(Debug, Clone, Copy)]
struct SpatialMixResult {
    gain: f32,
    pan: f32,
    rate_scale: f32,
}

/// Configuration for the mixer back-end.
#[derive(Debug, Clone, Copy)]
pub struct MixerConfig {
    pub sample_rate: u32,
    pub channels: u16,
    pub buffer_frames: usize,
}

impl Default for MixerConfig {
    fn default() -> Self {
        Self {
            sample_rate: 44_100,
            channels: 2,
            buffer_frames: 1024,
        }
    }
}

/// Snapshot of the mix timeline state for diagnostics or synchronisation.
#[derive(Debug, Clone, Copy)]
pub struct MixerTimelineSnapshot {
    /// Monotonic render buffer sequence number.
    pub sequence: u64,
    /// Absolute output frame the mixer has advanced to.
    pub current_frame: u64,
    /// Output sample rate associated with the timeline.
    pub sample_rate: u32,
}

/// Range of frames processed during a single render invocation.
#[derive(Debug, Clone, Copy)]
struct MixerTimelineSlice {
    sequence: u64,
    start_frame: u64,
    end_frame: u64,
}

/// Deterministic frame timeline that tracks mixer progression.
#[derive(Debug, Clone)]
struct MixerTimeline {
    sample_rate: u32,
    current_frame: u64,
    next_sequence: u64,
    last_span: MixerTimelineSlice,
}

impl MixerTimeline {
    fn new(sample_rate: u32) -> Self {
        Self {
            sample_rate,
            current_frame: 0,
            next_sequence: 0,
            last_span: MixerTimelineSlice {
                sequence: 0,
                start_frame: 0,
                end_frame: 0,
            },
        }
    }

    fn begin_render(&mut self, frames: usize) -> MixerTimelineSlice {
        let span = MixerTimelineSlice {
            sequence: self.next_sequence,
            start_frame: self.current_frame,
            end_frame: self.current_frame.saturating_add(frames as u64),
        };
        self.current_frame = span.end_frame;
        self.next_sequence = self.next_sequence.wrapping_add(1);
        self.last_span = span;
        span
    }

    fn snapshot(&self) -> MixerTimelineSnapshot {
        MixerTimelineSnapshot {
            sequence: self.last_span.sequence,
            current_frame: self.current_frame,
            sample_rate: self.sample_rate,
        }
    }

    fn restore(&mut self, snapshot: MixerTimelineSnapshot) {
        self.sample_rate = snapshot.sample_rate;
        self.current_frame = snapshot.current_frame;
        self.next_sequence = snapshot.sequence.wrapping_add(1);
        self.last_span = MixerTimelineSlice {
            sequence: snapshot.sequence,
            start_frame: snapshot.current_frame,
            end_frame: snapshot.current_frame,
        };
    }

    fn current_frame(&self) -> u64 {
        self.current_frame
    }
}

impl Default for MixerTimeline {
    fn default() -> Self {
        Self::new(44_100)
    }
}

/// Parameters that control how a voice is rendered.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct VoiceParams {
    pub gain: f32,
    pub pan: f32,
    pub playback_rate: u32,
    pub loop_count: u32,
    pub start_frame: u64,
    pub is_culled: bool,
    pub spatial: VoiceSpatialParams,
}

impl Default for VoiceParams {
    fn default() -> Self {
        Self {
            gain: 1.0,
            pan: 0.0,
            playback_rate: 44_100,
            loop_count: 1,
            start_frame: 0,
            is_culled: false,
            spatial: VoiceSpatialParams::default(),
        }
    }
}

/// Description for spawning a new voice.
#[derive(Clone)]
pub struct VoiceDescriptor {
    pub source: Arc<AudioSource>,
    pub params: VoiceParams,
    pub channel_id: u32,
    pub handle_id: Option<u32>,
}

impl fmt::Debug for VoiceDescriptor {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("VoiceDescriptor")
            .field("source", &self.source.identifier())
            .field("params", &self.params)
            .field("channel_id", &self.channel_id)
            .field("handle_id", &self.handle_id)
            .finish()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum VoicePlayState {
    Pending,
    Playing,
    Paused,
    Completed,
}

impl Default for VoicePlayState {
    fn default() -> Self {
        VoicePlayState::Pending
    }
}

/// Public playback state representation for introspection.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VoicePlaybackState {
    Pending,
    Playing,
    Paused,
    Completed,
}

impl From<VoicePlayState> for VoicePlaybackState {
    fn from(value: VoicePlayState) -> Self {
        match value {
            VoicePlayState::Pending => Self::Pending,
            VoicePlayState::Playing => Self::Playing,
            VoicePlayState::Paused => Self::Paused,
            VoicePlayState::Completed => Self::Completed,
        }
    }
}

/// Snapshot of a voice's timeline state for deterministic queries.
#[derive(Debug, Clone, Copy)]
pub struct VoiceTimelineState {
    pub handle: VoiceHandle,
    pub rendered_frames: u64,
    pub position_frames: f64,
    pub source_rate: u32,
    pub timeline_origin: u64,
    pub last_sequence: u64,
    pub state: VoicePlaybackState,
}

#[derive(Debug)]
struct MixerVoice {
    handle: VoiceHandle,
    descriptor: VoiceDescriptor,
    state: VoicePlayState,
    stop_reason: Option<VoiceStopReason>,
    sample: Option<Arc<AudioSample>>,
    position: f64,
    loops_remaining: Option<u32>,
    source_rate: u32,
    timeline_started_at: u64,
    timeline_cursor: u64,
    last_sequence_rendered: u64,
}

impl MixerVoice {
    fn new(handle: VoiceHandle, descriptor: VoiceDescriptor, timeline_origin: u64) -> Self {
        let loops_remaining = if descriptor.params.loop_count == 0 {
            None
        } else {
            Some(descriptor.params.loop_count)
        };
        let start_frame = descriptor.params.start_frame as f64;
        let sample = descriptor.source.sample();
        let source_rate = sample
            .as_ref()
            .and_then(|sample| sample.format.as_ref().map(|fmt| fmt.rate))
            .unwrap_or(0);
        Self {
            handle,
            descriptor,
            state: VoicePlayState::Pending,
            stop_reason: None,
            sample,
            position: start_frame,
            loops_remaining,
            source_rate,
            timeline_started_at: timeline_origin,
            timeline_cursor: timeline_origin,
            last_sequence_rendered: 0,
        }
    }
}

#[derive(Debug)]
struct MixerState {
    next_voice_id: VoiceId,
    voice_generations: HashMap<VoiceId, u32>,
    voices: HashMap<VoiceId, MixerVoice>,
    events: VecDeque<MixerEvent>,
    timeline: MixerTimeline,
}

impl MixerState {
    fn default_with_rate(sample_rate: u32) -> Self {
        Self {
            next_voice_id: 1,
            voice_generations: HashMap::new(),
            voices: HashMap::new(),
            events: VecDeque::new(),
            timeline: MixerTimeline::new(sample_rate),
        }
    }

    fn reserve_voice_handle(&mut self) -> VoiceHandle {
        let id = self.next_voice_id.max(1);
        self.next_voice_id = id.wrapping_add(1).max(1);
        let generation = match self.voice_generations.entry(id) {
            Entry::Occupied(mut entry) => {
                let next = entry.get().wrapping_add(1).max(1);
                entry.insert(next);
                next
            }
            Entry::Vacant(entry) => {
                entry.insert(1);
                1
            }
        };
        VoiceHandle::new(id, generation)
    }

    fn validate_handle(&self, handle: VoiceHandle) -> bool {
        if !handle.is_valid() {
            return false;
        }
        matches!(self.voice_generations.get(&handle.id()), Some(&generation) if generation == handle.generation())
    }

    fn timeline_snapshot(&self) -> MixerTimelineSnapshot {
        self.timeline.snapshot()
    }

    fn begin_render(&mut self, frames: usize) -> MixerTimelineSlice {
        self.timeline.begin_render(frames)
    }

    fn restore_timeline(&mut self, snapshot: MixerTimelineSnapshot) {
        self.timeline.restore(snapshot);
    }

    fn resolve_voice(&self, handle: VoiceHandle) -> Option<&MixerVoice> {
        self.voices
            .get(&handle.id())
            .filter(|voice| voice.handle.generation() == handle.generation())
    }

    fn resolve_voice_mut(&mut self, handle: VoiceHandle) -> Option<&mut MixerVoice> {
        self.voices
            .get_mut(&handle.id())
            .filter(|voice| voice.handle.generation() == handle.generation())
    }

    fn voice_timeline_state(&self, handle: VoiceHandle) -> Option<VoiceTimelineState> {
        self.resolve_voice(handle).map(|voice| VoiceTimelineState {
            handle: voice.handle,
            rendered_frames: voice
                .timeline_cursor
                .saturating_sub(voice.timeline_started_at),
            position_frames: voice.position,
            source_rate: if voice.source_rate == 0 {
                self.timeline.sample_rate
            } else {
                voice.source_rate
            },
            timeline_origin: voice.timeline_started_at,
            last_sequence: voice.last_sequence_rendered,
            state: voice.state.into(),
        })
    }

    fn push_event(&mut self, event: MixerEvent) {
        self.events.push_back(event);
    }

    fn drain_events(&mut self) -> Vec<MixerEvent> {
        self.events.drain(..).collect()
    }

    fn apply_command(&mut self, command: MixerCommand) {
        let mut pending_event: Option<MixerEvent> = None;

        match command {
            MixerCommand::Start { handle, descriptor } => {
                if !self.validate_handle(handle) {
                    trace!(
                        "Mixer: start requested with stale handle {}:{}",
                        handle.id(),
                        handle.generation()
                    );
                    return;
                }
                debug!(
                    "Mixer: starting voice {} (gen {})",
                    handle.id(),
                    handle.generation()
                );
                let timeline_origin = self.timeline.current_frame();
                let voice = MixerVoice::new(handle, descriptor.clone(), timeline_origin);
                self.voices.insert(handle.id(), voice);
                pending_event = Some(MixerEvent::VoiceStarted { handle, descriptor });
            }
            MixerCommand::Stop { handle, reason } => {
                if let Some(voice) = self.resolve_voice_mut(handle) {
                    debug!(
                        "Mixer: stopping voice {} (gen {})",
                        handle.id(),
                        handle.generation()
                    );
                    voice.state = VoicePlayState::Completed;
                    voice.stop_reason = Some(reason);
                } else {
                    trace!(
                        "Mixer: stop requested for missing voice {}:{}",
                        handle.id(),
                        handle.generation()
                    );
                }
            }
            MixerCommand::Pause { handle } => {
                if let Some(voice) = self.resolve_voice_mut(handle) {
                    if voice.state == VoicePlayState::Playing {
                        debug!(
                            "Mixer: pausing voice {} (gen {})",
                            handle.id(),
                            handle.generation()
                        );
                        voice.state = VoicePlayState::Paused;
                    }
                } else {
                    trace!(
                        "Mixer: pause requested for missing voice {}:{}",
                        handle.id(),
                        handle.generation()
                    );
                }
            }
            MixerCommand::Resume { handle } => {
                if let Some(voice) = self.resolve_voice_mut(handle) {
                    if matches!(
                        voice.state,
                        VoicePlayState::Paused | VoicePlayState::Pending
                    ) {
                        debug!(
                            "Mixer: resuming voice {} (gen {})",
                            handle.id(),
                            handle.generation()
                        );
                        voice.state = VoicePlayState::Playing;
                        let descriptor = voice.descriptor.clone();
                        pending_event = Some(MixerEvent::VoiceResumed { handle, descriptor });
                    }
                } else {
                    trace!(
                        "Mixer: resume requested for missing voice {}:{}",
                        handle.id(),
                        handle.generation()
                    );
                }
            }
            MixerCommand::UpdateParams { handle, params } => {
                let current_frame = self.timeline.current_frame();
                if let Some(voice) = self.resolve_voice_mut(handle) {
                    trace!(
                        "Mixer: updating params for voice {} (gen {})",
                        handle.id(),
                        handle.generation()
                    );
                    let previous = voice.descriptor.params;
                    voice.descriptor.params = params;
                    if params.start_frame != previous.start_frame {
                        voice.position = params.start_frame as f64;
                        voice.timeline_started_at = current_frame;
                        voice.timeline_cursor = current_frame;
                    }
                    if params.loop_count != previous.loop_count {
                        voice.loops_remaining = if params.loop_count == 0 {
                            None
                        } else {
                            Some(params.loop_count)
                        };
                    }
                    pending_event = Some(MixerEvent::VoiceUpdated { handle, params });
                } else {
                    trace!(
                        "Mixer: update params requested for missing voice {}:{}",
                        handle.id(),
                        handle.generation()
                    );
                }
            }
            MixerCommand::Seek {
                handle,
                start_frame,
            } => {
                let current_frame = self.timeline.current_frame();
                if let Some(voice) = self.resolve_voice_mut(handle) {
                    trace!(
                        "Mixer: seeking voice {} (gen {}) to {}",
                        handle.id(),
                        handle.generation(),
                        start_frame
                    );
                    voice.descriptor.params.start_frame = start_frame;
                    voice.position = start_frame as f64;
                    voice.timeline_started_at = current_frame;
                    voice.timeline_cursor = current_frame;
                    pending_event = Some(MixerEvent::VoiceSeek {
                        handle,
                        start_frame,
                    });
                } else {
                    trace!(
                        "Mixer: seek requested for missing voice {}:{}",
                        handle.id(),
                        handle.generation()
                    );
                }
            }
        }

        if let Some(event) = pending_event {
            self.push_event(event);
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VoiceStopReason {
    Command,
    Completed,
}

#[derive(Debug, Clone)]
pub enum MixerEvent {
    VoiceStarted {
        handle: VoiceHandle,
        descriptor: VoiceDescriptor,
    },
    VoiceStopped {
        handle: VoiceHandle,
        descriptor: VoiceDescriptor,
        reason: VoiceStopReason,
    },
    VoiceResumed {
        handle: VoiceHandle,
        descriptor: VoiceDescriptor,
    },
    VoiceUpdated {
        handle: VoiceHandle,
        params: VoiceParams,
    },
    VoiceSeek {
        handle: VoiceHandle,
        start_frame: u64,
    },
}

/// Simple PCM mix target used while the render pipeline is being ported.
#[derive(Debug, Clone)]
pub struct MixBuffer {
    pub channels: u16,
    pub frames: usize,
    pub sample_rate: u32,
    pub data: Vec<f32>,
    sequence: u64,
    valid_frames: usize,
}

impl MixBuffer {
    pub fn new(channels: u16, frames: usize, sample_rate: u32) -> Self {
        Self {
            channels,
            frames,
            sample_rate,
            data: vec![0.0; frames.saturating_mul(channels as usize)],
            sequence: 0,
            valid_frames: 0,
        }
    }

    pub fn clear(&mut self) {
        for sample in &mut self.data {
            *sample = 0.0;
        }
        self.valid_frames = 0;
    }

    /// Prepare the buffer for the next render pass and stamp its sequence.
    pub fn prepare(&mut self, sequence: u64) {
        self.clear();
        self.sequence = sequence;
    }

    /// Mark how many frames were written during the render pass.
    pub fn mark_written(&mut self, frames: usize) {
        self.valid_frames = frames.min(self.frames);
    }

    /// Monotonic sequence associated with the current buffer contents.
    pub fn sequence(&self) -> u64 {
        self.sequence
    }

    /// Number of valid frames present in the buffer after mixing.
    pub fn valid_frames(&self) -> usize {
        self.valid_frames
    }

    /// View of the valid interleaved samples produced by the mixer.
    pub fn interleaved_samples(&self) -> &[f32] {
        let len = self
            .valid_frames
            .saturating_mul(self.channels as usize)
            .min(self.data.len());
        &self.data[..len]
    }

    pub fn interleaved_samples_mut(&mut self) -> &mut [f32] {
        let len = self
            .valid_frames
            .saturating_mul(self.channels as usize)
            .min(self.data.len());
        &mut self.data[..len]
    }
}

#[derive(Debug, Default, Clone, Copy)]
pub struct MixRenderStats {
    pub active_voices: usize,
    pub culled_voices: usize,
    pub rendered_frames: u64,
}

impl MixRenderStats {
    pub fn render_latency_ms(&self) -> f64 {
        0.0 // Placeholder until latencies are tracked explicitly.
    }
}

#[derive(Debug)]
enum MixerCommand {
    Start {
        handle: VoiceHandle,
        descriptor: VoiceDescriptor,
    },
    Stop {
        handle: VoiceHandle,
        reason: VoiceStopReason,
    },
    Pause {
        handle: VoiceHandle,
    },
    Resume {
        handle: VoiceHandle,
    },
    UpdateParams {
        handle: VoiceHandle,
        params: VoiceParams,
    },
    Seek {
        handle: VoiceHandle,
        start_frame: u64,
    },
}

/// Thread-safe mixer wrapper shared across subsystems.
#[derive(Debug, Clone)]
pub struct AudioMixer {
    inner: Arc<Mutex<MixerState>>,
    config: MixerConfig,
    command_tx: Sender<MixerCommand>,
    command_rx: Arc<Mutex<Receiver<MixerCommand>>>,
}

impl AudioMixer {
    pub fn new(config: MixerConfig) -> Self {
        let (command_tx, command_rx) = unbounded();
        let state = MixerState::default_with_rate(config.sample_rate);
        Self {
            inner: Arc::new(Mutex::new(state)),
            config,
            command_tx,
            command_rx: Arc::new(Mutex::new(command_rx)),
        }
    }

    pub fn config(&self) -> MixerConfig {
        self.config
    }

    /// Obtain the current mixer timeline snapshot.
    pub fn timeline_snapshot(&self) -> MixerTimelineSnapshot {
        self.inner
            .lock()
            .expect("Mixer mutex poisoned")
            .timeline_snapshot()
    }

    pub fn restore_timeline(&self, snapshot: MixerTimelineSnapshot) {
        self.inner
            .lock()
            .expect("Mixer mutex poisoned")
            .restore_timeline(snapshot);
    }

    /// Query the timeline state for a specific voice handle.
    pub fn voice_timeline(&self, handle: VoiceHandle) -> Option<VoiceTimelineState> {
        self.apply_pending_commands();
        self.inner
            .lock()
            .expect("Mixer mutex poisoned")
            .voice_timeline_state(handle)
    }

    /// Reserve a new voice slot and return its identifier.
    pub fn start_voice(&self, descriptor: VoiceDescriptor) -> VoiceHandle {
        let handle = {
            let mut guard = self.inner.lock().expect("Mixer mutex poisoned");
            guard.reserve_voice_handle()
        };
        if let Err(err) = self
            .command_tx
            .send(MixerCommand::Start { handle, descriptor })
        {
            warn!("Mixer: failed to enqueue start command: {err}");
        }
        self.apply_pending_commands();
        handle
    }

    /// Update mixer-side parameters for an existing voice.
    pub fn update_voice_params(&self, handle: VoiceHandle, params: VoiceParams) {
        if let Err(err) = self
            .command_tx
            .send(MixerCommand::UpdateParams { handle, params })
        {
            warn!("Mixer: failed to enqueue params update: {err}");
        }
        self.apply_pending_commands();
    }

    pub fn stop_voice(&self, handle: VoiceHandle, reason: VoiceStopReason) {
        if let Err(err) = self.command_tx.send(MixerCommand::Stop { handle, reason }) {
            warn!("Mixer: failed to enqueue stop command: {err}");
        }
        self.apply_pending_commands();
    }

    pub fn pause_voice(&self, handle: VoiceHandle) {
        if let Err(err) = self.command_tx.send(MixerCommand::Pause { handle }) {
            warn!("Mixer: failed to enqueue pause command: {err}");
        }
        self.apply_pending_commands();
    }

    pub fn resume_voice(&self, handle: VoiceHandle) {
        if let Err(err) = self.command_tx.send(MixerCommand::Resume { handle }) {
            warn!("Mixer: failed to enqueue resume command: {err}");
        }
        self.apply_pending_commands();
    }

    pub fn seek_voice(&self, handle: VoiceHandle, start_frame: u64) {
        if let Err(err) = self.command_tx.send(MixerCommand::Seek {
            handle,
            start_frame,
        }) {
            warn!("Mixer: failed to enqueue seek command: {err}");
        }
        self.apply_pending_commands();
    }

    /// Advance internal bookkeeping. Real mixing will eventually live here.
    pub fn tick(&self, _delta: Duration) {
        self.apply_pending_commands();
        let mut guard = self.inner.lock().expect("Mixer mutex poisoned");
        let mut completed = Vec::new();

        for voice in guard.voices.values_mut() {
            if voice.state == VoicePlayState::Completed {
                let reason = voice.stop_reason.unwrap_or(VoiceStopReason::Completed);
                completed.push((voice.handle, voice.descriptor.clone(), reason));
            } else if voice.state == VoicePlayState::Pending {
                voice.state = VoicePlayState::Playing;
            }
        }

        for (handle, descriptor, reason) in completed {
            guard.push_event(MixerEvent::VoiceStopped {
                handle,
                descriptor,
                reason,
            });
            guard.voices.remove(&handle.id());
        }
    }

    /// Snapshot the mixer state for debugging or persistence.
    pub fn voice_snapshot(&self) -> HashMap<VoiceHandle, VoiceDescriptor> {
        self.apply_pending_commands();
        self.inner
            .lock()
            .expect("Mixer mutex poisoned")
            .voices
            .values()
            .map(|voice| (voice.handle, voice.descriptor.clone()))
            .collect()
    }

    pub fn drain_events(&self) -> Vec<MixerEvent> {
        self.inner
            .lock()
            .expect("Mixer mutex poisoned")
            .drain_events()
    }

    /// Render the current voice set into the provided mix buffer.
    pub fn render_into(&self, buffer: &mut MixBuffer) -> MixRenderStats {
        self.apply_pending_commands();
        let mut guard = self.inner.lock().expect("Mixer mutex poisoned");
        let frames = buffer.frames;
        let span = guard.begin_render(frames);
        buffer.prepare(span.sequence);
        let mut stats = MixRenderStats::default();

        let out_channels = buffer.channels as usize;
        let output_rate = buffer.sample_rate;

        for voice in guard.voices.values_mut() {
            voice.last_sequence_rendered = span.sequence;

            if voice.descriptor.params.is_culled {
                stats.culled_voices += 1;
                continue;
            }

            match voice.state {
                VoicePlayState::Completed => continue,
                VoicePlayState::Paused => {
                    stats.active_voices += 1;
                    continue;
                }
                VoicePlayState::Pending => {
                    voice.state = VoicePlayState::Playing;
                }
                VoicePlayState::Playing => {}
            }

            let sample = match voice.sample.as_ref() {
                Some(sample) => sample,
                None => continue,
            };
            let sample_ref = sample.as_ref();
            let data = match sample_ref.data.as_ref() {
                Some(data) => data.as_slice(),
                None => continue,
            };
            let format = match sample_ref.format.as_ref() {
                Some(fmt) => fmt,
                None => continue,
            };

            if format.sample_width != 16 {
                continue;
            }

            let sample_channels = format.channels.max(1) as usize;
            if sample_channels == 0 {
                continue;
            }
            let source_rate = format.rate.max(1);
            if source_rate == 0 {
                continue;
            }
            voice.source_rate = source_rate;

            let total_frames = (data.len() / 2) / sample_channels;
            if total_frames == 0 {
                continue;
            }

            let mut position = voice.position.max(0.0);
            let mut loops_remaining = voice.loops_remaining;
            let spatial = compute_spatial_mix(&voice.descriptor.params);
            let step = {
                let playback_rate = voice.descriptor.params.playback_rate.max(1) as f64;
                let base_rate = source_rate as f64;
                ((playback_rate / base_rate) * f64::from(spatial.rate_scale)).max(0.01)
            };

            let gain = spatial.gain;
            let pan = spatial.pan;
            let (pan_left, pan_right) = pan_to_gains(pan);

            let mut frame_index = 0;
            let total_frames_f64 = total_frames as f64;
            'mixing: while frame_index < frames {
                let mut int_pos = position.floor() as usize;
                while int_pos >= total_frames {
                    if let Some(ref mut loops) = loops_remaining {
                        if *loops > 1 {
                            *loops -= 1;
                            position -= total_frames_f64;
                            int_pos = position.floor() as usize;
                            continue;
                        } else {
                            voice.state = VoicePlayState::Completed;
                            voice.stop_reason = Some(VoiceStopReason::Completed);
                            break 'mixing;
                        }
                    } else {
                        position -= total_frames_f64;
                        int_pos = position.floor() as usize;
                    }
                }

                let sample_index = int_pos * sample_channels;
                if (sample_index + sample_channels) * 2 > data.len() {
                    voice.state = VoicePlayState::Completed;
                    voice.stop_reason = Some(VoiceStopReason::Completed);
                    break;
                }

                let mut left = read_sample(data, sample_index);
                let mut right = if sample_channels > 1 {
                    read_sample(data, sample_index + 1)
                } else {
                    left
                };

                left *= gain * pan_left;
                right *= gain * pan_right;

                let base = frame_index * out_channels;
                if out_channels >= 1 {
                    buffer.data[base] += left;
                }
                if out_channels >= 2 {
                    buffer.data[base + 1] += right;
                }
                if out_channels > 2 {
                    let mono = (left + right) * 0.5;
                    for channel in 2..out_channels {
                        buffer.data[base + channel] += mono;
                    }
                }

                position += step * (source_rate as f64 / output_rate as f64);
                frame_index += 1;
            }

            voice.position = position;
            voice.loops_remaining = loops_remaining;
            let processed_frames = frame_index as u64;
            voice.timeline_cursor = span.start_frame.saturating_add(processed_frames);
            voice.last_sequence_rendered = span.sequence;
            if !matches!(voice.state, VoicePlayState::Completed) {
                stats.active_voices += 1;
            }
        }

        buffer.mark_written(frames);
        stats.rendered_frames = stats.rendered_frames.saturating_add(frames as u64);

        for sample in buffer.interleaved_samples_mut() {
            *sample = sample.clamp(-1.0, 1.0);
        }

        stats
    }

    fn apply_pending_commands(&self) {
        let rx = self
            .command_rx
            .lock()
            .expect("Mixer command receiver poisoned");
        loop {
            match rx.try_recv() {
                Ok(command) => {
                    let mut guard = self.inner.lock().expect("Mixer mutex poisoned");
                    guard.apply_command(command);
                }
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => {
                    warn!("Mixer command channel disconnected");
                    break;
                }
            }
        }
    }
}

fn read_sample(data: &[u8], index: usize) -> f32 {
    let byte_index = index * 2;
    if byte_index + 1 >= data.len() {
        return 0.0;
    }
    let sample = i16::from_le_bytes([data[byte_index], data[byte_index + 1]]);
    sample as f32 / i16::MAX as f32
}

fn pan_to_gains(pan: f32) -> (f32, f32) {
    let pan = pan.clamp(-1.0, 1.0);
    let left = if pan <= 0.0 { 1.0 } else { 1.0 - pan };
    let right = if pan >= 0.0 { 1.0 } else { 1.0 + pan };
    (left, right)
}

fn compute_spatial_mix(params: &VoiceParams) -> SpatialMixResult {
    let gain = params.gain.clamp(0.0, 2.0);
    let mut pan = params.pan.clamp(-1.0, 1.0);
    let mut rate_scale = 1.0f32;

    match params.spatial.mode {
        VoiceSpatialMode::None => {}
        VoiceSpatialMode::Pseudo3D | VoiceSpatialMode::Full3D => {
            let relative = params.spatial.position - params.spatial.listener_position;

            let right = params.spatial.listener_transform.right_vector().normalize();
            let pan_scalar = if relative.length_squared() <= EPSILON {
                0.0
            } else {
                relative.normalize().dot(right).clamp(-1.0, 1.0)
            };
            pan = pan_scalar;

            if params.spatial.mode == VoiceSpatialMode::Full3D {
                let dir = if relative.length_squared() <= EPSILON {
                    Vector3::ZERO
                } else {
                    relative.normalize()
                };
                if dir != Vector3::ZERO {
                    let source_speed = params.spatial.velocity.dot(dir);
                    let listener_speed = params.spatial.listener_velocity.dot(dir);
                    let numerator = SPEED_OF_SOUND_METERS_PER_SEC - listener_speed;
                    let denominator = SPEED_OF_SOUND_METERS_PER_SEC - source_speed;
                    if denominator.abs() > EPSILON {
                        let raw_scale = (numerator / denominator).clamp(0.25, 4.0);
                        rate_scale = raw_scale;
                    }
                }
            }
        }
    }

    SpatialMixResult {
        gain,
        pan: pan.clamp(-1.0, 1.0),
        rate_scale,
    }
}

/// Music crossfade state tracking
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum CrossfadeState {
    Idle,
    FadingOut {
        from_handle: VoiceHandle,
        progress: f32,
    },
    FadingIn {
        to_handle: VoiceHandle,
        progress: f32,
    },
    Crossfading {
        from_handle: VoiceHandle,
        to_handle: VoiceHandle,
        progress: f32,
    },
}

impl Default for CrossfadeState {
    fn default() -> Self {
        Self::Idle
    }
}

/// Music manager for crossfading between tracks
#[derive(Debug, Clone)]
pub struct MusicManager {
    current_track: Option<VoiceHandle>,
    next_track: Option<VoiceHandle>,
    crossfade_state: CrossfadeState,
    crossfade_duration_ms: f32,
    music_volume: f32,
}

impl Default for MusicManager {
    fn default() -> Self {
        Self {
            current_track: None,
            next_track: None,
            crossfade_state: CrossfadeState::Idle,
            crossfade_duration_ms: 2000.0, // 2 second default crossfade
            music_volume: 0.8,
        }
    }
}

impl MusicManager {
    pub fn new(crossfade_duration_ms: f32) -> Self {
        Self {
            crossfade_duration_ms,
            ..Default::default()
        }
    }

    pub fn set_music_volume(&mut self, volume: f32) {
        self.music_volume = volume.clamp(0.0, 1.0);
    }

    pub fn music_volume(&self) -> f32 {
        self.music_volume
    }

    /// Start playing a new music track with crossfade from current track
    pub fn play_track(&mut self, mixer: &AudioMixer, track_handle: VoiceHandle) {
        if let Some(current) = self.current_track {
            // Start crossfade from current to new track
            self.crossfade_state = CrossfadeState::Crossfading {
                from_handle: current,
                to_handle: track_handle,
                progress: 0.0,
            };
            self.next_track = Some(track_handle);
        } else {
            // No current track, fade in new track
            self.crossfade_state = CrossfadeState::FadingIn {
                to_handle: track_handle,
                progress: 0.0,
            };
            self.current_track = Some(track_handle);
        }
    }

    /// Stop current music track with fade out
    pub fn stop_track(&mut self) {
        if let Some(current) = self.current_track {
            self.crossfade_state = CrossfadeState::FadingOut {
                from_handle: current,
                progress: 0.0,
            };
        }
    }

    /// Update crossfade state based on elapsed time
    pub fn update(&mut self, mixer: &AudioMixer, delta_ms: f32) -> Vec<VoiceHandle> {
        let mut completed_voices = Vec::new();

        match &mut self.crossfade_state {
            CrossfadeState::Idle => {}
            CrossfadeState::FadingOut {
                from_handle,
                progress,
            } => {
                *progress += delta_ms / self.crossfade_duration_ms;

                if *progress >= 1.0 {
                    // Fade out complete
                    mixer.stop_voice(*from_handle, VoiceStopReason::Command);
                    completed_voices.push(*from_handle);
                    self.current_track = None;
                    self.crossfade_state = CrossfadeState::Idle;
                } else {
                    // Apply fade out gain
                    if let Some(timeline) = mixer.voice_timeline(*from_handle) {
                        let fade_gain = (1.0 - *progress) * self.music_volume;
                        // Update voice params with faded gain
                        // This would be done through the mixer's update_voice_params
                    }
                }
            }
            CrossfadeState::FadingIn {
                to_handle,
                progress,
            } => {
                *progress += delta_ms / self.crossfade_duration_ms;

                if *progress >= 1.0 {
                    // Fade in complete
                    self.crossfade_state = CrossfadeState::Idle;
                } else {
                    // Apply fade in gain
                    let fade_gain = *progress * self.music_volume;
                    // Update voice params with faded gain
                }
            }
            CrossfadeState::Crossfading {
                from_handle,
                to_handle,
                progress,
            } => {
                *progress += delta_ms / self.crossfade_duration_ms;

                if *progress >= 1.0 {
                    // Crossfade complete
                    mixer.stop_voice(*from_handle, VoiceStopReason::Command);
                    completed_voices.push(*from_handle);
                    self.current_track = self.next_track.take();
                    self.crossfade_state = CrossfadeState::Idle;
                } else {
                    // Apply crossfade gains
                    let fade_out_gain = (1.0 - *progress) * self.music_volume;
                    let fade_in_gain = *progress * self.music_volume;
                    // Update both voice params
                }
            }
        }

        completed_voices
    }

    pub fn is_crossfading(&self) -> bool {
        !matches!(self.crossfade_state, CrossfadeState::Idle)
    }
}
