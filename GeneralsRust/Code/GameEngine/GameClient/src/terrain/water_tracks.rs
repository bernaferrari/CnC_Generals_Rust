//! CPU-side parity port for C++ `W3DDevice/GameClient/Water/W3DWaterTracks.cpp`.

use glam::Vec2;
use thiserror::Error;

/// C++ `WATER_VB_PAGES`.
pub const WATER_VB_PAGES: usize = 1000;
/// C++ `WATER_STRIP_X`.
pub const WATER_STRIP_X: usize = 2;
/// C++ `WATER_STRIP_Y`.
pub const WATER_STRIP_Y: usize = 2;
/// C++ hard-coded preallocation count in `WaterTracksRenderSystem::init`.
pub const DEFAULT_WATER_TRACK_MODULES: usize = 2000;
/// C++ `WaterTracksRenderSystem::update` fixed frame step.
pub const WATER_TRACK_FRAME_MS: i32 = 1000 / 30;

/// C++ `waveType`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(i32)]
pub enum WaterTrackType {
    /// `WaveTypePond`.
    Pond = 0,
    /// `WaveTypeOcean`.
    Ocean = 1,
    /// `WaveTypeCloseOcean`.
    CloseOcean = 2,
    /// `WaveTypeCloseOceanDouble`.
    CloseOceanDouble = 3,
    /// `WaveTypeRadial`.
    Radial = 4,
    /// `WaveTypeStationary`; C++ has no explicit table initializer for this entry.
    Stationary = 5,
}

impl WaterTrackType {
    /// C++ editor cycles only through `WaveTypeFirst..WaveTypeLast`.
    pub const EDITOR_TYPES: [Self; 5] = [
        Self::Pond,
        Self::Ocean,
        Self::CloseOcean,
        Self::CloseOceanDouble,
        Self::Radial,
    ];

    /// Decode the C++ enum integer stored in `.wak` records.
    pub fn from_i32(value: i32) -> Option<Self> {
        match value {
            0 => Some(Self::Pond),
            1 => Some(Self::Ocean),
            2 => Some(Self::CloseOcean),
            3 => Some(Self::CloseOceanDouble),
            4 => Some(Self::Radial),
            5 => Some(Self::Stationary),
            _ => None,
        }
    }
}

/// C++ `waveInfo`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WaterTrackWaveInfo {
    pub final_width: f32,
    pub final_height: f32,
    pub wave_distance: f32,
    pub initial_velocity: f32,
    pub fade_ms: i32,
    pub initial_width_fraction: f32,
    pub initial_height_width_fraction: f32,
    pub time_to_compress: i32,
    pub second_wave_time_offset: i32,
    pub texture_name: &'static str,
    pub wave_type_name: &'static str,
}

impl WaterTrackWaveInfo {
    const fn zero(name: &'static str) -> Self {
        Self {
            final_width: 0.0,
            final_height: 0.0,
            wave_distance: 0.0,
            initial_velocity: 0.0,
            fade_ms: 0,
            initial_width_fraction: 0.0,
            initial_height_width_fraction: 0.0,
            time_to_compress: 0,
            second_wave_time_offset: 0,
            texture_name: "",
            wave_type_name: name,
        }
    }
}

/// C++ `waveTypeInfo`.
pub const WATER_TRACK_WAVE_INFO: [WaterTrackWaveInfo; 6] = [
    WaterTrackWaveInfo {
        final_width: 28.0,
        final_height: 18.0,
        wave_distance: 25.0,
        initial_velocity: 0.018,
        fade_ms: 900,
        initial_width_fraction: 0.01,
        initial_height_width_fraction: 0.18,
        time_to_compress: 1500,
        second_wave_time_offset: 0,
        texture_name: "wave256.tga",
        wave_type_name: "Pond",
    },
    WaterTrackWaveInfo {
        final_width: 55.0,
        final_height: 36.0,
        wave_distance: 80.0,
        initial_velocity: 0.015,
        fade_ms: 2000,
        initial_width_fraction: 0.5,
        initial_height_width_fraction: 0.18,
        time_to_compress: 1000,
        second_wave_time_offset: 6267,
        texture_name: "wave256.tga",
        wave_type_name: "Ocean",
    },
    WaterTrackWaveInfo {
        final_width: 55.0,
        final_height: 36.0,
        wave_distance: 80.0,
        initial_velocity: 0.015,
        fade_ms: 2000,
        initial_width_fraction: 0.05,
        initial_height_width_fraction: 0.18,
        time_to_compress: 1000,
        second_wave_time_offset: 6267,
        texture_name: "wave256.tga",
        wave_type_name: "Close Ocean",
    },
    WaterTrackWaveInfo {
        final_width: 55.0,
        final_height: 36.0,
        wave_distance: 80.0,
        initial_velocity: 0.015,
        fade_ms: 4000,
        initial_width_fraction: 0.01,
        initial_height_width_fraction: 0.18,
        time_to_compress: 2000,
        second_wave_time_offset: 6267,
        texture_name: "wave256.tga",
        wave_type_name: "Close Ocean Double",
    },
    WaterTrackWaveInfo {
        final_width: 55.0,
        final_height: 27.0,
        wave_distance: 80.0,
        initial_velocity: 0.015,
        fade_ms: 2000,
        initial_width_fraction: 0.01,
        initial_height_width_fraction: 8.0,
        time_to_compress: 2000,
        second_wave_time_offset: 5367,
        texture_name: "wave256.tga",
        wave_type_name: "Radial",
    },
    WaterTrackWaveInfo::zero("Stationary"),
];

/// Water-height provider used by the CPU geometry port.
pub trait WaterTrackHeightProvider {
    /// Return water height for the sampled world position.
    fn water_height(&self, x: f32, y: f32) -> f32;
}

/// C++ `VertexFormatXYZDUV1` subset emitted by water-track rendering.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WaterTrackVertex {
    pub x: f32,
    pub y: f32,
    pub z: f32,
    pub diffuse: u32,
    pub u1: f32,
    pub v1: f32,
}

/// Per-track draw range in a CPU flush.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WaterTrackDrawRange {
    pub handle: usize,
    pub first_vertex: usize,
    pub vertex_count: usize,
    pub first_index: usize,
    pub index_count: usize,
    pub texture_name: String,
    pub wave_type: WaterTrackType,
}

/// CPU-side flush output for all active water tracks.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct WaterTracksFlush {
    pub vertices: Vec<WaterTrackVertex>,
    pub indices: Vec<u16>,
    pub ranges: Vec<WaterTrackDrawRange>,
}

/// Primary `.wak` record written by C++ `saveTracks`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WaterTrackSaveRecord {
    pub start: Vec2,
    pub end: Vec2,
    pub wave_type: WaterTrackType,
}

/// Errors while decoding the C++ `.wak` water-track sidecar format.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum WaterTrackWakError {
    #[error(".wak data is too short for trailing count")]
    MissingCount,
    #[error(".wak trailing count is negative: {0}")]
    NegativeCount(i32),
    #[error(".wak data is too short for {count} records")]
    TruncatedRecords { count: usize },
    #[error(".wak contains unknown wave type {0}")]
    UnknownWaveType(i32),
}

/// C++ `WaterTracksObj`.
#[derive(Debug, Clone)]
pub struct WaterTracksObj {
    wave_type: WaterTrackType,
    bound: bool,
    start_pos: Vec2,
    wave_dir: Vec2,
    perp_dir: Vec2,
    init_start_pos: Vec2,
    init_end_pos: Vec2,
    init_time_offset: i32,
    fade_ms: i32,
    total_ms: f32,
    elapsed_ms: f32,
    wave_initial_width: f32,
    wave_initial_height: f32,
    wave_final_width: f32,
    wave_final_height: f32,
    initial_velocity: f32,
    wave_distance: f32,
    time_to_reach_beach: f32,
    front_slow_down_acc: f32,
    time_to_stop: f32,
    time_to_retreat: f32,
    back_slow_down_acc: f32,
    time_to_compress: f32,
    flip_u: bool,
    texture_name: String,
}

impl Default for WaterTracksObj {
    fn default() -> Self {
        Self {
            wave_type: WaterTrackType::Pond,
            bound: false,
            start_pos: Vec2::ZERO,
            wave_dir: Vec2::ZERO,
            perp_dir: Vec2::X,
            init_start_pos: Vec2::ZERO,
            init_end_pos: Vec2::ZERO,
            init_time_offset: 0,
            fade_ms: 0,
            total_ms: 0.0,
            elapsed_ms: 0.0,
            wave_initial_width: 0.0,
            wave_initial_height: 0.0,
            wave_final_width: 0.0,
            wave_final_height: 0.0,
            initial_velocity: 0.0,
            wave_distance: 0.0,
            time_to_reach_beach: 0.0,
            front_slow_down_acc: 0.0,
            time_to_stop: 0.0,
            time_to_retreat: 0.0,
            back_slow_down_acc: 0.0,
            time_to_compress: 0.0,
            flip_u: false,
            texture_name: String::new(),
        }
    }
}

impl WaterTracksObj {
    pub fn wave_type(&self) -> WaterTrackType {
        self.wave_type
    }

    pub fn bound(&self) -> bool {
        self.bound
    }

    pub fn elapsed_ms(&self) -> f32 {
        self.elapsed_ms
    }

    pub fn init_time_offset(&self) -> i32 {
        self.init_time_offset
    }

    pub fn texture_name(&self) -> &str {
        &self.texture_name
    }

    pub fn flip_u(&self) -> bool {
        self.flip_u
    }

    pub fn set_flip_u(&mut self, flip_u: bool) {
        self.flip_u = flip_u;
    }

    pub fn time_to_reach_beach(&self) -> f32 {
        self.time_to_reach_beach
    }

    pub fn total_ms(&self) -> f32 {
        self.total_ms
    }

    pub fn free_water_tracks_resources(&mut self) {
        self.texture_name.clear();
    }

    /// C++ `WaterTracksObj::init(width, length, start, end, texture, offset)`.
    pub fn init(
        &mut self,
        width: f32,
        length: f32,
        start: Vec2,
        end: Vec2,
        texture_name: impl Into<String>,
        wave_time_offset: i32,
    ) {
        self.free_water_tracks_resources();
        self.init_start_pos = start;
        self.init_end_pos = end;
        self.init_time_offset = wave_time_offset;
        self.elapsed_ms = wave_time_offset as f32;
        self.start_pos = start;

        self.perp_dir = rotate(end - start, -std::f32::consts::FRAC_PI_2).normalize_or_zero();
        self.wave_dir = rotate(self.perp_dir, std::f32::consts::FRAC_PI_2);

        let info = self.wave_info();
        self.wave_distance = info.wave_distance;
        self.wave_dir *= self.wave_distance;
        self.start_pos -= self.wave_dir;

        self.initial_velocity = info.initial_velocity;
        self.fade_ms = info.fade_ms;
        self.wave_initial_width = length * info.initial_width_fraction;
        self.wave_initial_height = self.wave_initial_width * info.initial_height_width_fraction;
        self.wave_final_width = length;
        self.wave_final_height = width;

        self.time_to_reach_beach =
            (self.wave_distance - self.wave_final_height) / self.initial_velocity;
        self.front_slow_down_acc =
            -(self.initial_velocity * self.initial_velocity) / (2.0 * self.wave_final_height);
        self.time_to_stop = -self.initial_velocity / self.front_slow_down_acc;
        self.time_to_retreat = (2.0 * self.wave_final_height / self.front_slow_down_acc)
            .abs()
            .sqrt();
        self.total_ms = self.time_to_reach_beach + self.time_to_stop + self.time_to_retreat;
        self.back_slow_down_acc =
            2.0 * self.wave_initial_height / (self.time_to_stop * self.time_to_stop);
        self.time_to_compress = info.time_to_compress as f32;

        if self.wave_type == WaterTrackType::Stationary {
            self.time_to_retreat = 1000.0;
            self.total_ms = self.time_to_reach_beach
                + self.time_to_stop
                + self.fade_ms as f32
                + self.time_to_retreat;
            self.start_pos = start;
            self.fade_ms = 1000;
        }

        self.texture_name = texture_name.into();
    }

    /// C++ alternate `WaterTracksObj::init(width, start, end, texture)`.
    pub fn init_from_segment(
        &mut self,
        width: f32,
        start: Vec2,
        end: Vec2,
        texture_name: impl Into<String>,
        map_xy_factor: f32,
    ) {
        self.free_water_tracks_resources();
        self.perp_dir = end - start;
        self.start_pos = start + self.perp_dir * 0.5;
        let length = self.perp_dir.length();
        self.perp_dir *= 1.0 / length;
        self.wave_dir = rotate(self.perp_dir, std::f32::consts::FRAC_PI_2);
        self.start_pos -= self.wave_dir * width;
        self.wave_dir *= 1.3 * map_xy_factor;
        self.start_pos -= self.wave_dir;
        self.elapsed_ms = 0.0;
        self.initial_velocity = 0.001 * map_xy_factor;
        self.total_ms = self.wave_dir.length() / self.initial_velocity;
        self.fade_ms = 3000;
        self.wave_final_width = length;
        self.wave_final_height = width;
        self.texture_name = texture_name.into();
    }

    /// C++ `WaterTracksObj::update`.
    pub fn update(&mut self, ms_elapsed: i32) -> bool {
        self.elapsed_ms += ms_elapsed as f32;
        true
    }

    /// Build the four C++ water-track vertices and reset elapsed time when the
    /// C++ render path would wrap the animation.
    pub fn build_vertices<H: WaterTrackHeightProvider>(
        &mut self,
        heights: &H,
    ) -> [WaterTrackVertex; WATER_STRIP_X * WATER_STRIP_Y] {
        let wave_dir_len = self.wave_dir.length();
        let oo_wave_dir_len = 1.0 / wave_dir_len;
        let (wave_tail_origin, wave_front_origin, wave_alpha, width_frac) =
            self.current_origins(oo_wave_dir_len);

        let water_height = heights.water_height(wave_tail_origin.x, wave_tail_origin.y);
        let z = water_height + 1.5;
        let diffuse = ((real_to_int(wave_alpha * 255.0) as u32) << 24) | 0x00ff_ffff;
        let left_u = if self.flip_u { 1.0 } else { 0.0 };
        let right_u = if self.flip_u { 0.0 } else { 1.0 };
        let right_tail = wave_tail_origin + self.perp_dir * self.wave_final_width * width_frac;
        let right_front = wave_front_origin + self.perp_dir * self.wave_final_width * width_frac;

        [
            WaterTrackVertex {
                x: wave_tail_origin.x,
                y: wave_tail_origin.y,
                z,
                diffuse,
                u1: left_u,
                v1: 0.0,
            },
            WaterTrackVertex {
                x: right_tail.x,
                y: right_tail.y,
                z,
                diffuse,
                u1: right_u,
                v1: 0.0,
            },
            WaterTrackVertex {
                x: wave_front_origin.x,
                y: wave_front_origin.y,
                z,
                diffuse,
                u1: left_u,
                v1: 1.0,
            },
            WaterTrackVertex {
                x: right_front.x,
                y: right_front.y,
                z,
                diffuse,
                u1: right_u,
                v1: 1.0,
            },
        ]
    }

    fn wave_info(&self) -> WaterTrackWaveInfo {
        WATER_TRACK_WAVE_INFO[self.wave_type as usize]
    }

    fn current_origins(&mut self, oo_wave_dir_len: f32) -> (Vec2, Vec2, f32, f32) {
        if self.wave_type == WaterTrackType::Stationary {
            let wave_front_origin = self.start_pos - self.perp_dir * self.wave_final_width * 0.5;
            let wave_tail_origin =
                wave_front_origin - self.wave_final_height * oo_wave_dir_len * self.wave_dir;
            let mut wave_alpha = 0.0;

            if self.elapsed_ms >= self.total_ms {
                self.elapsed_ms = 0.0;
            }

            let fade_start = self.time_to_reach_beach + self.time_to_stop - 1000.0;
            let fade_out_start = fade_start + self.fade_ms as f32;
            if self.elapsed_ms > fade_out_start {
                wave_alpha = (self.elapsed_ms - fade_out_start) / self.time_to_retreat;
                wave_alpha = 1.0 - wave_alpha;
                if wave_alpha < 0.0 {
                    wave_alpha = 0.0;
                }
            } else if self.elapsed_ms > fade_start {
                wave_alpha = (self.elapsed_ms - fade_start) / self.fade_ms as f32;
                if wave_alpha > 1.0 {
                    wave_alpha = 1.0;
                }
            }

            return (wave_tail_origin, wave_front_origin, wave_alpha, 1.0);
        }

        if self.elapsed_ms < self.time_to_reach_beach {
            let wave_alpha = self.elapsed_ms / self.time_to_reach_beach;
            let width_frac = (self.wave_initial_width
                + wave_alpha * (self.wave_final_width - self.wave_initial_width))
                / self.wave_final_width;
            let wave_front_origin = self.start_pos
                + self.initial_velocity * self.elapsed_ms * oo_wave_dir_len * self.wave_dir
                - self.perp_dir * self.wave_final_width * 0.5 * width_frac;
            let wave_tail_origin =
                wave_front_origin - self.wave_initial_height * oo_wave_dir_len * self.wave_dir;
            return (wave_tail_origin, wave_front_origin, wave_alpha, width_frac);
        }

        if self.elapsed_ms < self.total_ms {
            let width_frac = 1.0;
            let mut wave_front_origin = self.start_pos
                + self.initial_velocity
                    * self.time_to_reach_beach
                    * oo_wave_dir_len
                    * self.wave_dir;
            let mut wave_tail_origin = wave_front_origin;
            let mut elapsed_ms = self.elapsed_ms - self.time_to_reach_beach;
            wave_front_origin += (self.initial_velocity * elapsed_ms
                + 0.5 * self.front_slow_down_acc * elapsed_ms * elapsed_ms)
                * oo_wave_dir_len
                * self.wave_dir;
            wave_front_origin -= self.perp_dir * self.wave_final_width * 0.5 * width_frac;

            let mut time_since_backtrack =
                self.elapsed_ms - self.time_to_reach_beach - self.time_to_stop;
            if time_since_backtrack < 0.0 {
                time_since_backtrack = 0.0;
            }
            let mut wave_alpha = time_since_backtrack / self.fade_ms as f32;
            if wave_alpha > 1.0 {
                wave_alpha = 1.0;
            }
            wave_alpha = 1.0 - wave_alpha;

            wave_tail_origin -= self.wave_initial_height * oo_wave_dir_len * self.wave_dir;
            if self.elapsed_ms
                > self.time_to_reach_beach + self.time_to_stop + self.time_to_compress
            {
                wave_tail_origin += (0.5
                    * self.back_slow_down_acc
                    * (self.time_to_stop + self.time_to_compress)
                    * (self.time_to_stop + self.time_to_compress))
                    * oo_wave_dir_len
                    * self.wave_dir;
                elapsed_ms = self.elapsed_ms
                    - (self.time_to_reach_beach + self.time_to_stop + self.time_to_compress);
                wave_tail_origin += (0.5 * self.front_slow_down_acc * elapsed_ms * elapsed_ms)
                    * oo_wave_dir_len
                    * self.wave_dir;
            } else {
                wave_tail_origin += (0.5 * self.back_slow_down_acc * elapsed_ms * elapsed_ms)
                    * oo_wave_dir_len
                    * self.wave_dir;
            }
            wave_tail_origin -= self.perp_dir * self.wave_final_width * 0.5 * width_frac;
            return (wave_tail_origin, wave_front_origin, wave_alpha, width_frac);
        }

        self.elapsed_ms = 0.0;
        let wave_alpha = self.elapsed_ms / self.time_to_reach_beach;
        let width_frac = (self.wave_initial_width
            + wave_alpha * (self.wave_final_width - self.wave_initial_width))
            / self.wave_final_width;
        let wave_front_origin = self.start_pos
            + self.initial_velocity * self.elapsed_ms * oo_wave_dir_len * self.wave_dir
            - self.perp_dir * self.wave_final_width * 0.5 * width_frac;
        let wave_tail_origin =
            wave_front_origin - self.wave_initial_height * oo_wave_dir_len * self.wave_dir;
        (wave_tail_origin, wave_front_origin, wave_alpha, width_frac)
    }
}

/// C++ `WaterTracksRenderSystem`.
#[derive(Debug, Clone)]
pub struct WaterTracksRenderSystem {
    tracks: Vec<WaterTracksObj>,
    used: Vec<usize>,
    free: Vec<usize>,
    strip_size_x: usize,
    strip_size_y: usize,
    batch_start: usize,
    show_soft_water_edge: bool,
    transparent_water_depth: f32,
}

impl WaterTracksRenderSystem {
    pub fn new(num_modules: usize) -> Self {
        let tracks = vec![WaterTracksObj::default(); num_modules];
        let free = (0..num_modules).rev().collect();
        Self {
            tracks,
            used: Vec::new(),
            free,
            strip_size_x: WATER_STRIP_X,
            strip_size_y: WATER_STRIP_Y,
            batch_start: 0,
            show_soft_water_edge: true,
            transparent_water_depth: 1.0,
        }
    }

    pub fn used_handles(&self) -> &[usize] {
        &self.used
    }

    pub fn free_count(&self) -> usize {
        self.free.len()
    }

    pub fn track(&self, handle: usize) -> Option<&WaterTracksObj> {
        self.tracks.get(handle)
    }

    pub fn track_mut(&mut self, handle: usize) -> Option<&mut WaterTracksObj> {
        self.tracks.get_mut(handle)
    }

    pub fn set_render_enabled(&mut self, show_soft_water_edge: bool, transparent_water_depth: f32) {
        self.show_soft_water_edge = show_soft_water_edge;
        self.transparent_water_depth = transparent_water_depth;
    }

    /// C++ `bindTrack`, including same-type sorted insertion and `SYNC_WAVES`.
    pub fn bind_track(&mut self, wave_type: WaterTrackType) -> Option<usize> {
        let handle = self.free.pop()?;
        self.tracks[handle].wave_type = wave_type;

        let insert_at = self
            .used
            .iter()
            .position(|&used| self.tracks[used].wave_type == wave_type)
            .unwrap_or(0);
        self.used.insert(insert_at, handle);
        self.tracks[handle].bound = true;

        for &used in &self.used {
            self.tracks[used].elapsed_ms = self.tracks[used].init_time_offset as f32;
        }

        Some(handle)
    }

    /// C++ `unbindTrack`.
    pub fn unbind_track(&mut self, handle: usize) {
        if let Some(track) = self.tracks.get_mut(handle) {
            track.bound = false;
        }
        self.release_track(handle);
    }

    pub fn release_track(&mut self, handle: usize) {
        if handle >= self.tracks.len() || self.tracks[handle].bound {
            return;
        }
        if let Some(pos) = self.used.iter().position(|&used| used == handle) {
            self.used.remove(pos);
            self.tracks[handle].free_water_tracks_resources();
            self.free.push(handle);
        }
    }

    pub fn reset(&mut self) {
        let used = self.used.clone();
        for handle in used {
            self.tracks[handle].bound = false;
            self.release_track(handle);
        }
    }

    pub fn update(&mut self, ms_elapsed: i32) {
        let used = self.used.clone();
        for handle in used {
            let bound = self.tracks[handle].bound;
            let updating = self.tracks[handle].update(ms_elapsed);
            if !bound || (!updating && !self.tracks[handle].bound) {
                self.release_track(handle);
            }
        }
    }

    pub fn update_fixed_frame(&mut self) {
        self.update(WATER_TRACK_FRAME_MS);
    }

    /// Add the primary wave and optional second wave used by C++ `.wak` loading.
    pub fn add_wave_from_info(
        &mut self,
        wave_type: WaterTrackType,
        start: Vec2,
        end: Vec2,
        flip_u: bool,
    ) -> Vec<usize> {
        let info = WATER_TRACK_WAVE_INFO[wave_type as usize];
        let mut handles = Vec::with_capacity(2);
        if let Some(handle) = self.bind_track(wave_type) {
            self.tracks[handle].init(
                info.final_height,
                info.final_width,
                start,
                end,
                info.texture_name,
                0,
            );
            self.tracks[handle].flip_u = flip_u;
            handles.push(handle);
        }

        if info.second_wave_time_offset != 0 {
            if let Some(handle) = self.bind_track(wave_type) {
                self.tracks[handle].init(
                    info.final_height,
                    info.final_width,
                    start,
                    end,
                    info.texture_name,
                    info.second_wave_time_offset,
                );
                self.tracks[handle].flip_u = !flip_u;
                handles.push(handle);
            }
        }

        handles
    }

    pub fn find_track(&self, start: Vec2, end: Vec2, wave_type: WaterTrackType) -> Option<usize> {
        self.used.iter().copied().find(|&handle| {
            let track = &self.tracks[handle];
            track.init_start_pos == start
                && track.init_end_pos == end
                && track.wave_type == wave_type
        })
    }

    /// C++ `saveTracks` record filtering: only primary waves are persisted.
    pub fn save_records(&self) -> Vec<WaterTrackSaveRecord> {
        self.used
            .iter()
            .filter_map(|&handle| {
                let track = &self.tracks[handle];
                (track.init_time_offset == 0).then_some(WaterTrackSaveRecord {
                    start: track.init_start_pos,
                    end: track.init_end_pos,
                    wave_type: track.wave_type,
                })
            })
            .collect()
    }

    /// Load C++ `.wak` records after the trailing count has already been decoded.
    pub fn load_records(&mut self, records: &[WaterTrackSaveRecord]) -> Vec<usize> {
        let mut handles = Vec::new();
        let mut flip_u = false;
        for record in records {
            if self
                .find_track(record.start, record.end, record.wave_type)
                .is_some()
            {
                continue;
            }
            flip_u = !flip_u;
            handles.extend(self.add_wave_from_info(
                record.wave_type,
                record.start,
                record.end,
                flip_u,
            ));
        }
        handles
    }

    /// CPU equivalent of the non-device portion of C++ `flush`.
    pub fn flush<H: WaterTrackHeightProvider>(&mut self, heights: &H) -> WaterTracksFlush {
        if !self.show_soft_water_edge || self.transparent_water_depth == 0.0 {
            return WaterTracksFlush::default();
        }

        self.update_fixed_frame();
        self.batch_start = 0xffff;

        let mut output = WaterTracksFlush::default();
        for &handle in &self.used.clone() {
            if self.batch_start
                >= WATER_VB_PAGES * self.strip_size_x * self.strip_size_y
                    - self.strip_size_x * self.strip_size_y
            {
                self.batch_start = 0;
            }

            let first_vertex = output.vertices.len();
            let first_index = output.indices.len();
            let vertices = self.tracks[handle].build_vertices(heights);
            output.vertices.extend(vertices);
            output.indices.extend(
                water_track_strip_indices(self.strip_size_x, self.strip_size_y)
                    .into_iter()
                    .map(|idx| idx + first_vertex as u16),
            );
            output.ranges.push(WaterTrackDrawRange {
                handle,
                first_vertex,
                vertex_count: self.strip_size_x * self.strip_size_y,
                first_index,
                index_count: (self.strip_size_y - 1) * (self.strip_size_x * 2 + 2) - 2,
                texture_name: self.tracks[handle].texture_name.clone(),
                wave_type: self.tracks[handle].wave_type,
            });
            self.batch_start += self.strip_size_x * self.strip_size_y;
        }

        output
    }
}

/// C++ dynamic triangle-strip index generation from `ReAcquireResources`.
pub fn water_track_strip_indices(strip_size_x: usize, strip_size_y: usize) -> Vec<u16> {
    let idx_count = (strip_size_y - 1) * (strip_size_x * 2 + 2) - 2;
    let mut ib = vec![0u16; idx_count];
    let mut i = 0;
    let mut j = 0;
    let mut k = 0;
    while i < idx_count {
        while k < strip_size_x * (j + 1) {
            ib[i] = (k + strip_size_x) as u16;
            ib[i + 1] = k as u16;
            k += 1;
            i += 2;
        }
        if i < idx_count {
            ib[i] = (k - 1) as u16;
            ib[i + 1] = (k + strip_size_x) as u16;
            i += 2;
        }
        j += 1;
    }
    ib
}

/// Serialize records exactly like C++ `WaterTracksRenderSystem::saveTracks`:
/// raw start/end `Vector2`, raw `waveType`, then a trailing record count.
pub fn encode_wak_records(records: &[WaterTrackSaveRecord]) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(records.len() * 20 + 4);
    for record in records {
        bytes.extend_from_slice(&record.start.x.to_le_bytes());
        bytes.extend_from_slice(&record.start.y.to_le_bytes());
        bytes.extend_from_slice(&record.end.x.to_le_bytes());
        bytes.extend_from_slice(&record.end.y.to_le_bytes());
        bytes.extend_from_slice(&(record.wave_type as i32).to_le_bytes());
    }
    bytes.extend_from_slice(&(records.len() as i32).to_le_bytes());
    bytes
}

/// Decode records from the C++ `.wak` sidecar format. The record count is read
/// from the final four bytes, then records are consumed from the start.
pub fn decode_wak_records(bytes: &[u8]) -> Result<Vec<WaterTrackSaveRecord>, WaterTrackWakError> {
    const RECORD_SIZE: usize = 20;

    if bytes.len() < 4 {
        return Err(WaterTrackWakError::MissingCount);
    }

    let count_offset = bytes.len() - 4;
    let count = i32::from_le_bytes(bytes[count_offset..].try_into().unwrap());
    if count < 0 {
        return Err(WaterTrackWakError::NegativeCount(count));
    }

    let count = count as usize;
    if bytes.len() < count * RECORD_SIZE + 4 {
        return Err(WaterTrackWakError::TruncatedRecords { count });
    }

    let mut records = Vec::with_capacity(count);
    for index in 0..count {
        let offset = index * RECORD_SIZE;
        let start_x = f32::from_le_bytes(bytes[offset..offset + 4].try_into().unwrap());
        let start_y = f32::from_le_bytes(bytes[offset + 4..offset + 8].try_into().unwrap());
        let end_x = f32::from_le_bytes(bytes[offset + 8..offset + 12].try_into().unwrap());
        let end_y = f32::from_le_bytes(bytes[offset + 12..offset + 16].try_into().unwrap());
        let wave_type = i32::from_le_bytes(bytes[offset + 16..offset + 20].try_into().unwrap());
        let wave_type = WaterTrackType::from_i32(wave_type)
            .ok_or(WaterTrackWakError::UnknownWaveType(wave_type))?;

        records.push(WaterTrackSaveRecord {
            start: Vec2::new(start_x, start_y),
            end: Vec2::new(end_x, end_y),
            wave_type,
        });
    }

    Ok(records)
}

/// C++ replaces the final four characters of the map source filename with
/// `.wak`; this helper keeps that exact convention for callers doing I/O.
pub fn water_track_wak_path(source_filename: &str) -> String {
    if source_filename.len() >= 4 {
        format!("{}.wak", &source_filename[..source_filename.len() - 4])
    } else {
        ".wak".to_string()
    }
}

fn rotate(v: Vec2, angle: f32) -> Vec2 {
    let (sin, cos) = angle.sin_cos();
    Vec2::new(v.x * cos - v.y * sin, v.x * sin + v.y * cos)
}

fn real_to_int(value: f32) -> i32 {
    value as i32
}
