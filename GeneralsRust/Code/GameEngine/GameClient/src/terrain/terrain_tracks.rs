//! CPU-side parity port for C++ `W3DDevice/GameClient/W3DTerrainTracks.cpp`.

use game_engine::map_object::MAP_XY_FACTOR;
use glam::Vec3;

/// C++ `MAX_TRACK_EDGE_COUNT`.
pub const MAX_TRACK_EDGE_COUNT: usize = 100;
/// C++ `MAX_TRACK_OPAQUE_EDGE`.
pub const MAX_TRACK_OPAQUE_EDGE: usize = 25;
/// C++ `FADE_TIME_FRAMES`.
pub const FADE_TIME_FRAMES: i32 = 300_000;
/// Amount C++ raises bridge-layer tracks above bridge geometry.
pub const BRIDGE_OFFSET_FACTOR: f32 = 0.25;
/// C++ default spacing fallback from `computeTrackSpacing`.
pub const DEFAULT_TRACK_SPACING: f32 = MAP_XY_FACTOR * 1.4;
/// C++ `DEFAULT_TRACK_WIDTH`.
pub const DEFAULT_TRACK_WIDTH: f32 = 4.0;

/// C++ `computeTrackSpacing`.
pub fn compute_track_spacing(left_tread_bone: Option<Vec3>, right_tread_bone: Option<Vec3>) -> f32 {
    match (left_tread_bone, right_tread_bone) {
        (Some(left), Some(right)) => (right - left).length() + DEFAULT_TRACK_WIDTH,
        _ => DEFAULT_TRACK_SPACING,
    }
}

/// Height/normal provider used by the terrain track port.
pub trait TerrainTrackHeightProvider {
    /// Ground height and terrain normal.
    fn ground_height_and_normal(&self, x: f32, y: f32) -> (f32, Vec3);

    /// Non-ground layer height and normal.
    fn layer_height_and_normal(&self, x: f32, y: f32) -> (f32, Vec3) {
        self.ground_height_and_normal(x, y)
    }
}

/// Layer used by the track owner.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TerrainTrackLayer {
    /// Ground layer.
    Ground,
    /// Non-ground layer such as bridge.
    Other,
}

/// C++ `VertexFormatXYZDUV1` subset emitted for track rendering.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TerrainTrackVertex {
    /// World X.
    pub x: f32,
    /// World Y.
    pub y: f32,
    /// World Z.
    pub z: f32,
    /// Packed diffuse, including alpha fade.
    pub diffuse: u32,
    /// U coordinate.
    pub u1: f32,
    /// V coordinate.
    pub v1: f32,
}

/// Draw range for one track object inside a flush buffer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerrainTrackDrawRange {
    /// Track handle.
    pub handle: usize,
    /// First vertex in the combined buffer.
    pub first_vertex: usize,
    /// Vertex count.
    pub vertex_count: usize,
    /// First index in the combined index buffer.
    pub first_index: usize,
    /// Index count.
    pub index_count: usize,
    /// Texture name bound to this track object.
    pub texture_name: String,
}

/// CPU flush output for all visible track marks.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct TerrainTracksFlush {
    /// Vertices for all flushed tracks.
    pub vertices: Vec<TerrainTrackVertex>,
    /// Indices for all flushed tracks.
    pub indices: Vec<u16>,
    /// Per-track draw ranges.
    pub ranges: Vec<TerrainTrackDrawRange>,
}

/// C++ `edgeInfo`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TerrainTrackEdge {
    /// Left/right endpoints.
    pub endpoint_pos: [Vec3; 2],
    /// Left/right endpoint UVs.
    pub endpoint_uv: [[f32; 2]; 2],
    /// Sync time when edge was added.
    pub time_added: i32,
    /// Current alpha.
    pub alpha: f32,
}

impl Default for TerrainTrackEdge {
    fn default() -> Self {
        Self {
            endpoint_pos: [Vec3::ZERO; 2],
            endpoint_uv: [[0.0; 2]; 2],
            time_added: 0,
            alpha: 0.0,
        }
    }
}

/// Track mark render object.
#[derive(Debug, Clone)]
pub struct TerrainTracksRenderObjClass {
    edges: Vec<TerrainTrackEdge>,
    last_anchor: Vec3,
    top_index: usize,
    bottom_index: usize,
    active_edge_count: usize,
    total_edges_added: usize,
    have_anchor: bool,
    have_cap: bool,
    bound: bool,
    airborne: bool,
    width: f32,
    length: f32,
    texture_name: String,
    owner_layer: TerrainTrackLayer,
    really_visible: bool,
}

impl TerrainTracksRenderObjClass {
    /// Construct empty track object.
    pub fn new(max_edges: usize) -> Self {
        Self {
            edges: vec![TerrainTrackEdge::default(); max_edges],
            last_anchor: Vec3::new(0.0, 1.0, 2.25),
            top_index: 0,
            bottom_index: 0,
            active_edge_count: 0,
            total_edges_added: 0,
            have_anchor: false,
            have_cap: true,
            bound: false,
            airborne: false,
            width: DEFAULT_TRACK_WIDTH,
            length: DEFAULT_TRACK_SPACING,
            texture_name: String::new(),
            owner_layer: TerrainTrackLayer::Ground,
            really_visible: true,
        }
    }

    /// C++ `freeTerrainTracksResources`.
    pub fn free_terrain_tracks_resources(&mut self) {
        self.have_anchor = false;
        self.have_cap = true;
        self.top_index = 0;
        self.bottom_index = 0;
        self.active_edge_count = 0;
        self.total_edges_added = 0;
        self.bound = false;
        self.airborne = false;
        self.texture_name.clear();
    }

    /// C++ `init`.
    pub fn init(&mut self, width: f32, length: f32, texture_name: impl Into<String>) {
        self.free_terrain_tracks_resources();
        self.width = width;
        self.length = length;
        self.texture_name = texture_name.into();
        self.bound = true;
    }

    /// C++ `setAirborne`.
    pub fn set_airborne(&mut self) {
        self.airborne = true;
    }

    /// Set owner layer used for bridge height selection.
    pub fn set_owner_layer(&mut self, layer: TerrainTrackLayer) {
        self.owner_layer = layer;
    }

    /// Set visibility used by flush.
    pub fn set_really_visible(&mut self, visible: bool) {
        self.really_visible = visible;
    }

    /// C++ `addEdgeToTrack`.
    pub fn add_edge_to_track<M: TerrainTrackHeightProvider>(
        &mut self,
        terrain: &M,
        x: f32,
        y: f32,
        sync_time: i32,
        max_edges: usize,
    ) {
        if !self.have_anchor {
            let (height, _) = self.sample_height_normal(terrain, x, y);
            self.last_anchor = Vec3::new(x, y, height);
            self.have_anchor = true;
            self.airborne = true;
            self.have_cap = true;
            return;
        }

        self.have_cap = false;
        let (height, normal) = self.sample_height_normal(terrain, x, y);
        let pos = Vec3::new(x, y, height);
        let mut dir = pos - self.last_anchor;
        if dir.length_squared() < self.length * self.length {
            return;
        }

        self.reserve_edge_slot(max_edges);
        dir.z = 0.0;
        let dir = dir.normalize_or_zero();
        let side = dir.cross(normal).normalize_or_zero();
        let alpha = if self.airborne || self.active_edge_count <= 1 {
            0.0
        } else {
            1.0
        };
        self.write_top_edge(pos, side, sync_time, alpha);
        self.airborne = false;
        self.last_anchor = pos;
        self.active_edge_count += 1;
        self.total_edges_added += 1;
        self.top_index += 1;
    }

    /// C++ `addCapEdgeToTrack`.
    pub fn add_cap_edge_to_track<M: TerrainTrackHeightProvider>(
        &mut self,
        terrain: &M,
        x: f32,
        y: f32,
        sync_time: i32,
        max_edges: usize,
    ) {
        if self.have_cap {
            return;
        }
        if self.active_edge_count == 1 {
            self.have_cap = true;
            self.have_anchor = false;
            return;
        }

        let (height, normal) = self.sample_height_normal(terrain, x, y);
        let pos = Vec3::new(x, y, height);
        let mut dir = pos - self.last_anchor;
        if dir.length_squared() < self.length * self.length {
            let mut last_added = self.top_index as isize - 1;
            if last_added < 0 {
                last_added = max_edges as isize - 1;
            }
            self.edges[last_added as usize].alpha = 0.0;
            self.have_cap = true;
            self.have_anchor = false;
            return;
        }

        self.reserve_edge_slot(max_edges);
        dir.z = 0.0;
        let dir = dir.normalize_or_zero();
        let side = dir.cross(normal).normalize_or_zero();
        self.write_top_edge(pos, side, sync_time, 0.0);
        self.last_anchor = pos;
        self.active_edge_count += 1;
        self.total_edges_added += 1;
        self.top_index += 1;
        self.have_cap = true;
        self.have_anchor = false;
    }

    /// Active edge count.
    pub fn active_edge_count(&self) -> usize {
        self.active_edge_count
    }

    /// C++ `m_haveAnchor`.
    pub fn have_anchor(&self) -> bool {
        self.have_anchor
    }

    /// C++ `m_haveCap`.
    pub fn have_cap(&self) -> bool {
        self.have_cap
    }

    /// Ordered active edges, oldest first.
    pub fn active_edges(&self, max_edges: usize) -> Vec<TerrainTrackEdge> {
        (0..self.active_edge_count)
            .map(|i| self.edges[(self.bottom_index + i) % max_edges])
            .collect()
    }

    /// Bound flag.
    pub fn bound(&self) -> bool {
        self.bound
    }

    fn sample_height_normal<M: TerrainTrackHeightProvider>(
        &self,
        terrain: &M,
        x: f32,
        y: f32,
    ) -> (f32, Vec3) {
        match self.owner_layer {
            TerrainTrackLayer::Ground => terrain.ground_height_and_normal(x, y),
            TerrainTrackLayer::Other => {
                let (height, normal) = terrain.layer_height_and_normal(x, y);
                (height + BRIDGE_OFFSET_FACTOR, normal)
            }
        }
    }

    fn reserve_edge_slot(&mut self, max_edges: usize) {
        if self.active_edge_count >= max_edges {
            self.bottom_index = (self.bottom_index + 1) % max_edges;
            self.active_edge_count -= 1;
        }
        if self.top_index >= max_edges {
            self.top_index = 0;
        }
    }

    fn write_top_edge(&mut self, pos: Vec3, side: Vec3, sync_time: i32, alpha: f32) {
        let z_lift = 0.2 * MAP_XY_FACTOR;
        let left = pos - side * (self.width * 0.5) + Vec3::new(0.0, 0.0, z_lift);
        let right = pos + side * (self.width * 0.5) + Vec3::new(0.0, 0.0, z_lift);
        let odd = (self.total_edges_added & 1) != 0;
        self.edges[self.top_index] = TerrainTrackEdge {
            endpoint_pos: [left, right],
            endpoint_uv: if odd {
                [[0.0, 0.0], [1.0, 0.0]]
            } else {
                [[0.0, 1.0], [1.0, 1.0]]
            },
            time_added: sync_time,
            alpha,
        };
    }

    fn clear_track_edges(&mut self, max_edges: usize) {
        self.edges.resize(max_edges, TerrainTrackEdge::default());
        self.have_anchor = false;
        self.have_cap = true;
        self.top_index = 0;
        self.bottom_index = 0;
        self.active_edge_count = 0;
        self.total_edges_added = 0;
    }
}

/// Track system configuration matching GlobalData inputs.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TerrainTracksConfig {
    /// Maximum terrain track modules.
    pub max_terrain_tracks: usize,
    /// Maximum edges per track.
    pub max_tank_track_edges: usize,
    /// Number of newest edges that stay opaque before forced distance fade.
    pub max_tank_track_opaque_edges: usize,
    /// Fade delay in sync-time units.
    pub max_tank_track_fade_delay: i32,
    /// Global make-track-marks flag.
    pub make_track_marks: bool,
}

impl Default for TerrainTracksConfig {
    fn default() -> Self {
        Self {
            max_terrain_tracks: 64,
            max_tank_track_edges: MAX_TRACK_EDGE_COUNT,
            max_tank_track_opaque_edges: MAX_TRACK_OPAQUE_EDGE,
            max_tank_track_fade_delay: FADE_TIME_FRAMES,
            make_track_marks: true,
        }
    }
}

/// CPU pool equivalent of C++ `TerrainTracksRenderObjClassSystem`.
#[derive(Debug, Clone)]
pub struct TerrainTracksRenderObjClassSystem {
    tracks: Vec<TerrainTracksRenderObjClass>,
    free_modules: Vec<usize>,
    used_modules: Vec<usize>,
    edges_to_flush: usize,
    config: TerrainTracksConfig,
}

impl TerrainTracksRenderObjClassSystem {
    /// Construct and preallocate track objects.
    pub fn new(config: TerrainTracksConfig) -> Self {
        let max_edges = config.max_tank_track_edges;
        let tracks = (0..config.max_terrain_tracks)
            .map(|_| TerrainTracksRenderObjClass::new(max_edges))
            .collect::<Vec<_>>();
        let free_modules = (0..config.max_terrain_tracks).rev().collect();
        Self {
            tracks,
            free_modules,
            used_modules: Vec::new(),
            edges_to_flush: 0,
            config,
        }
    }

    /// C++ `bindTrack`.
    pub fn bind_track(
        &mut self,
        width: f32,
        length: f32,
        texture_name: impl Into<String>,
    ) -> Option<usize> {
        let handle = self.free_modules.pop()?;
        self.tracks[handle].init(width, length, texture_name);
        self.used_modules.insert(0, handle);
        Some(handle)
    }

    /// C++ `unbindTrack`.
    pub fn unbind_track(&mut self, handle: usize) {
        if let Some(track) = self.tracks.get_mut(handle) {
            track.bound = false;
        }
    }

    /// Access a track.
    pub fn track(&self, handle: usize) -> Option<&TerrainTracksRenderObjClass> {
        self.tracks.get(handle)
    }

    /// Mutable track access.
    pub fn track_mut(&mut self, handle: usize) -> Option<&mut TerrainTracksRenderObjClass> {
        self.tracks.get_mut(handle)
    }

    /// Add edge through system config.
    pub fn add_edge_to_track<M: TerrainTrackHeightProvider>(
        &mut self,
        handle: usize,
        terrain: &M,
        x: f32,
        y: f32,
        sync_time: i32,
    ) {
        if let Some(track) = self.tracks.get_mut(handle) {
            track.add_edge_to_track(terrain, x, y, sync_time, self.config.max_tank_track_edges);
        }
    }

    /// Add cap edge through system config.
    pub fn add_cap_edge_to_track<M: TerrainTrackHeightProvider>(
        &mut self,
        handle: usize,
        terrain: &M,
        x: f32,
        y: f32,
        sync_time: i32,
    ) {
        if let Some(track) = self.tracks.get_mut(handle) {
            track.add_cap_edge_to_track(terrain, x, y, sync_time, self.config.max_tank_track_edges);
        }
    }

    /// C++ `Render` counter behavior.
    pub fn submit_render(&mut self, handle: usize) {
        let Some(track) = self.tracks.get(handle) else {
            return;
        };
        if self.config.make_track_marks && track.active_edge_count >= 2 {
            self.edges_to_flush += track.active_edge_count;
        }
    }

    /// C++ `update`.
    pub fn update(&mut self, sync_time: i32) {
        let max_edges = self.config.max_tank_track_edges;
        let mut release = Vec::new();
        for &handle in &self.used_modules {
            let track = &mut self.tracks[handle];
            if !self.config.make_track_marks {
                track.have_anchor = false;
            }

            let mut i = 0;
            let mut index = track.bottom_index;
            while i < track.active_edge_count {
                if index >= max_edges {
                    index = 0;
                }
                let mut diff = 1.0
                    - (sync_time - track.edges[index].time_added) as f32
                        / self.config.max_tank_track_fade_delay as f32;
                if diff < 0.0 {
                    diff = 0.0;
                }
                if track.edges[index].alpha > 0.0 {
                    track.edges[index].alpha = diff;
                }
                if diff == 0.0 {
                    track.bottom_index = (track.bottom_index + 1) % max_edges;
                    track.active_edge_count -= 1;
                }
                i += 1;
                index += 1;
            }
            if track.active_edge_count == 0 && !track.bound {
                release.push(handle);
            }
        }

        for handle in release {
            self.release_track(handle);
        }
    }

    /// C++ `flush`, CPU vertex/index construction.
    pub fn flush(&mut self, diffuse_light: u32) -> TerrainTracksFlush {
        let mut flush = TerrainTracksFlush::default();
        if self.edges_to_flush < 2 {
            return flush;
        }

        let num_faded_edges = (self.config.max_tank_track_edges
            - self.config.max_tank_track_opaque_edges)
            .max(1) as f32;
        for &handle in &self.used_modules {
            let track = &self.tracks[handle];
            if track.active_edge_count < 2 || !track.really_visible {
                continue;
            }

            let first_vertex = flush.vertices.len();
            let first_index = flush.indices.len();
            for i in 0..track.active_edge_count {
                let index = (track.bottom_index + i) % self.config.max_tank_track_edges;
                let edge = track.edges[index];
                let mut distance_fade = 1.0;
                if (track.active_edge_count - 1 - i) >= self.config.max_tank_track_opaque_edges {
                    distance_fade = 1.0
                        - (track.active_edge_count - i - self.config.max_tank_track_opaque_edges)
                            as f32
                            / num_faded_edges;
                }
                distance_fade *= edge.alpha;
                let alpha = ((distance_fade * 255.0) as u32).min(255);
                let diffuse = (diffuse_light & 0x00ff_ffff) | (alpha << 24);
                for endpoint in 0..2 {
                    let pos = edge.endpoint_pos[endpoint];
                    let uv = edge.endpoint_uv[endpoint];
                    flush.vertices.push(TerrainTrackVertex {
                        x: pos.x,
                        y: pos.y,
                        z: pos.z,
                        diffuse,
                        u1: uv[0],
                        v1: uv[1],
                    });
                }
            }

            for i in 0..(track.active_edge_count - 1) {
                let base = (first_vertex + i * 2) as u16;
                flush.indices.extend_from_slice(&[
                    base,
                    base + 1,
                    base + 3,
                    base,
                    base + 3,
                    base + 2,
                ]);
            }
            flush.ranges.push(TerrainTrackDrawRange {
                handle,
                first_vertex,
                vertex_count: track.active_edge_count * 2,
                first_index,
                index_count: (track.active_edge_count - 1) * 6,
                texture_name: track.texture_name.clone(),
            });
        }

        self.edges_to_flush = 0;
        flush
    }

    /// C++ `Reset`.
    pub fn reset(&mut self) {
        for handle in std::mem::take(&mut self.used_modules) {
            self.tracks[handle].free_terrain_tracks_resources();
            self.free_modules.push(handle);
        }
        self.edges_to_flush = 0;
    }

    /// C++ `clearTracks`.
    pub fn clear_tracks(&mut self) {
        for &handle in &self.used_modules {
            self.tracks[handle].clear_track_edges(self.config.max_tank_track_edges);
        }
        self.edges_to_flush = 0;
    }

    /// C++ `setDetail`.
    pub fn set_detail(&mut self, config: TerrainTracksConfig) {
        self.clear_tracks();
        let old_modules = self.config.max_terrain_tracks;
        self.config = TerrainTracksConfig {
            max_terrain_tracks: old_modules,
            ..config
        };
        for track in &mut self.tracks {
            track.clear_track_edges(self.config.max_tank_track_edges);
        }
        self.edges_to_flush = 0;
    }

    fn release_track(&mut self, handle: usize) {
        if let Some(pos) = self.used_modules.iter().position(|&id| id == handle) {
            self.used_modules.remove(pos);
            self.tracks[handle].free_terrain_tracks_resources();
            self.free_modules.push(handle);
        }
    }
}

impl Default for TerrainTracksRenderObjClassSystem {
    fn default() -> Self {
        Self::new(TerrainTracksConfig::default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct FlatTerrain;

    impl TerrainTrackHeightProvider for FlatTerrain {
        fn ground_height_and_normal(&self, _x: f32, _y: f32) -> (f32, Vec3) {
            (10.0, Vec3::Z)
        }

        fn layer_height_and_normal(&self, _x: f32, _y: f32) -> (f32, Vec3) {
            (20.0, Vec3::Z)
        }
    }

    fn config() -> TerrainTracksConfig {
        TerrainTracksConfig {
            max_terrain_tracks: 2,
            max_tank_track_edges: 4,
            max_tank_track_opaque_edges: 2,
            max_tank_track_fade_delay: 100,
            make_track_marks: true,
        }
    }

    #[test]
    fn first_edge_only_sets_anchor() {
        let terrain = FlatTerrain;
        let mut system = TerrainTracksRenderObjClassSystem::new(config());
        let handle = system.bind_track(4.0, 10.0, "tracks.tga").unwrap();

        system.add_edge_to_track(handle, &terrain, 0.0, 0.0, 0);

        assert_eq!(system.track(handle).unwrap().active_edge_count(), 0);
    }

    #[test]
    fn second_far_edge_adds_transparent_restart_edge() {
        let terrain = FlatTerrain;
        let mut system = TerrainTracksRenderObjClassSystem::new(config());
        let handle = system.bind_track(4.0, 10.0, "tracks.tga").unwrap();
        system.add_edge_to_track(handle, &terrain, 0.0, 0.0, 0);
        system.add_edge_to_track(handle, &terrain, 20.0, 0.0, 1);

        let edge = system.track(handle).unwrap().active_edges(4)[0];
        assert_eq!(edge.alpha, 0.0);
        assert_eq!(edge.endpoint_uv, [[0.0, 1.0], [1.0, 1.0]]);
    }

    #[test]
    fn cap_near_last_anchor_forces_last_edge_transparent() {
        let terrain = FlatTerrain;
        let mut system = TerrainTracksRenderObjClassSystem::new(config());
        let handle = system.bind_track(4.0, 10.0, "tracks.tga").unwrap();
        system.add_edge_to_track(handle, &terrain, 0.0, 0.0, 0);
        system.add_edge_to_track(handle, &terrain, 20.0, 0.0, 1);
        system.add_edge_to_track(handle, &terrain, 40.0, 0.0, 2);

        system.add_cap_edge_to_track(handle, &terrain, 41.0, 0.0, 3);

        let edges = system.track(handle).unwrap().active_edges(4);
        assert_eq!(edges.last().unwrap().alpha, 0.0);
    }

    #[test]
    fn update_fades_and_releases_unbound_tracks() {
        let terrain = FlatTerrain;
        let mut system = TerrainTracksRenderObjClassSystem::new(config());
        let handle = system.bind_track(4.0, 10.0, "tracks.tga").unwrap();
        system.add_edge_to_track(handle, &terrain, 0.0, 0.0, 0);
        system.add_edge_to_track(handle, &terrain, 20.0, 0.0, 1);
        system.add_edge_to_track(handle, &terrain, 40.0, 0.0, 2);
        system.unbind_track(handle);

        system.update(200);

        assert!(!system.track(handle).unwrap().bound());
        assert_eq!(system.track(handle).unwrap().active_edge_count(), 1);

        system.update(201);
        assert_eq!(system.track(handle).unwrap().active_edge_count(), 0);
    }

    #[test]
    fn flush_builds_vertices_indices_and_resets_counter() {
        let terrain = FlatTerrain;
        let mut system = TerrainTracksRenderObjClassSystem::new(config());
        let handle = system.bind_track(4.0, 10.0, "tracks.tga").unwrap();
        system.add_edge_to_track(handle, &terrain, 0.0, 0.0, 0);
        system.add_edge_to_track(handle, &terrain, 20.0, 0.0, 1);
        system.add_edge_to_track(handle, &terrain, 40.0, 0.0, 2);
        system.submit_render(handle);

        let flush = system.flush(0x0012_3456);

        assert_eq!(flush.vertices.len(), 4);
        assert_eq!(flush.indices, vec![0, 1, 3, 0, 3, 2]);
        assert_eq!(flush.ranges[0].texture_name, "tracks.tga");
        assert_eq!(flush.vertices[0].diffuse & 0x00ff_ffff, 0x0012_3456);
        assert!(system.flush(0x0012_3456).vertices.is_empty());
    }

    #[test]
    fn bridge_layer_adds_cpp_bridge_offset() {
        let terrain = FlatTerrain;
        let mut system = TerrainTracksRenderObjClassSystem::new(config());
        let handle = system.bind_track(4.0, 10.0, "tracks.tga").unwrap();
        system
            .track_mut(handle)
            .unwrap()
            .set_owner_layer(TerrainTrackLayer::Other);

        system.add_edge_to_track(handle, &terrain, 0.0, 0.0, 0);
        system.add_edge_to_track(handle, &terrain, 20.0, 0.0, 1);

        let edge = system.track(handle).unwrap().active_edges(4)[0];
        assert_eq!(
            edge.endpoint_pos[0].z,
            20.0 + BRIDGE_OFFSET_FACTOR + 0.2 * MAP_XY_FACTOR
        );
    }
}
