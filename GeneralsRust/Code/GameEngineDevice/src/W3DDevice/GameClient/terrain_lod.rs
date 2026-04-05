//! Terrain LOD (Level of Detail) System
//!
//! Corresponds to C++ files:
//! - GameEngineDevice/Source/W3DDevice/GameClient/BaseHeightMap.cpp (adjustTerrainLOD)
//! - GameEngine/Include/Common/GlobalData.h (TerrainLOD enum)
//!
//! Implements adaptive LOD for terrain chunks based on:
//! - Distance from camera
//! - View frustum position
//! - Performance settings
//! - Chunk importance

use cgmath::{InnerSpace, Point3, Vector3};

/// Terrain LOD levels matching C++ `TerrainVisual.h` `_TerrainLOD`
/// Keep the numeric discriminants in sync with `TerrainLODNames[]`.
#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum TerrainLOD {
    /// Invalid / unset terrain LOD
    Invalid = 0,
    /// Minimum quality (half resolution, no clouds, no water)
    Min = 1,
    /// Stretched terrain, no clouds
    StretchNoClouds = 2,
    /// Half resolution with clouds
    HalfClouds = 3,
    /// Full resolution, no clouds
    NoClouds = 4,
    /// Stretched terrain with clouds
    StretchClouds = 5,
    /// Full resolution, no water
    NoWater = 6,
    /// Maximum quality (full resolution, all features)
    Max = 7,
    /// Automatic LOD based on performance
    Automatic = 8,
    /// Terrain rendering disabled
    Disable = 9,
    /// Sentinel matching the C++ `TERRAIN_LOD_NUM_TYPES` terminator
    NumTypes = 10,
}

impl Default for TerrainLOD {
    fn default() -> Self {
        TerrainLOD::Automatic
    }
}

impl TerrainLOD {
    /// Convert from u32
    pub fn from_u32(value: u32) -> Option<Self> {
        match value {
            0 => Some(TerrainLOD::Invalid),
            1 => Some(TerrainLOD::Min),
            2 => Some(TerrainLOD::StretchNoClouds),
            3 => Some(TerrainLOD::HalfClouds),
            4 => Some(TerrainLOD::NoClouds),
            5 => Some(TerrainLOD::StretchClouds),
            6 => Some(TerrainLOD::NoWater),
            7 => Some(TerrainLOD::Max),
            8 => Some(TerrainLOD::Automatic),
            9 => Some(TerrainLOD::Disable),
            10 => Some(TerrainLOD::NumTypes),
            _ => None,
        }
    }

    /// Should use cloud map rendering
    pub fn use_cloud_map(&self) -> bool {
        matches!(
            self,
            TerrainLOD::HalfClouds
                | TerrainLOD::StretchClouds
                | TerrainLOD::NoWater
                | TerrainLOD::Max
        )
    }

    /// Should use light map
    pub fn use_light_map(&self) -> bool {
        matches!(
            self,
            TerrainLOD::HalfClouds
                | TerrainLOD::StretchClouds
                | TerrainLOD::NoWater
                | TerrainLOD::Max
        )
    }

    /// Should use water plane rendering
    pub fn use_water_plane(&self) -> bool {
        *self == TerrainLOD::Max
    }

    /// Should use stretched terrain (lower vertex resolution)
    pub fn use_stretched_terrain(&self) -> bool {
        matches!(
            self,
            TerrainLOD::StretchNoClouds | TerrainLOD::StretchClouds
        )
    }

    /// Should use half height map resolution
    pub fn use_half_height_map(&self) -> bool {
        matches!(self, TerrainLOD::Min | TerrainLOD::HalfClouds)
    }
}

/// LOD distance thresholds (in world units)
pub struct LODDistances {
    /// Distance for LOD level 0 (highest detail)
    pub lod0_distance: f32,
    /// Distance for LOD level 1
    pub lod1_distance: f32,
    /// Distance for LOD level 2
    pub lod2_distance: f32,
    /// Distance for LOD level 3 (lowest detail)
    pub lod3_distance: f32,
}

impl Default for LODDistances {
    fn default() -> Self {
        Self {
            lod0_distance: 200.0,
            lod1_distance: 400.0,
            lod2_distance: 800.0,
            lod3_distance: 1600.0,
        }
    }
}

/// Terrain chunk LOD manager
/// Corresponds to C++ BaseHeightMapRenderObjClass::adjustTerrainLOD logic
pub struct TerrainLODManager {
    /// Global LOD level setting
    global_lod: TerrainLOD,
    /// Distance thresholds for LOD transitions
    distances: LODDistances,
    /// Enable dynamic LOD based on distance
    enable_dynamic_lod: bool,
}

impl TerrainLODManager {
    pub fn new(global_lod: TerrainLOD) -> Self {
        Self {
            global_lod,
            distances: LODDistances::default(),
            enable_dynamic_lod: true,
        }
    }

    /// Set global terrain LOD level
    /// Corresponds to C++ adjustTerrainLOD
    pub fn set_global_lod(&mut self, lod: TerrainLOD) {
        self.global_lod = lod;
    }

    /// Get current global LOD level
    pub fn get_global_lod(&self) -> TerrainLOD {
        self.global_lod
    }

    /// Calculate LOD level for a chunk based on distance from camera
    /// Corresponds to C++ distance-based LOD selection
    pub fn calculate_chunk_lod(&self, chunk_center: Point3<f32>, camera_pos: Point3<f32>) -> u32 {
        if !self.enable_dynamic_lod {
            return 0; // Always use highest detail if dynamic LOD disabled
        }

        // Calculate distance from camera to chunk
        let distance = (chunk_center - camera_pos).magnitude();

        // Select LOD level based on distance
        if distance < self.distances.lod0_distance {
            0
        } else if distance < self.distances.lod1_distance {
            1
        } else if distance < self.distances.lod2_distance {
            2
        } else {
            3
        }
    }

    /// Calculate LOD level with hysteresis to prevent flickering
    pub fn calculate_chunk_lod_with_hysteresis(
        &self,
        chunk_center: Point3<f32>,
        camera_pos: Point3<f32>,
        current_lod: u32,
    ) -> u32 {
        if !self.enable_dynamic_lod {
            return 0;
        }

        let distance = (chunk_center - camera_pos).magnitude();
        let hysteresis = 20.0; // Hysteresis band to prevent flickering

        // Use hysteresis when transitioning LOD levels
        match current_lod {
            0 => {
                if distance > self.distances.lod0_distance + hysteresis {
                    1
                } else {
                    0
                }
            }
            1 => {
                if distance < self.distances.lod0_distance - hysteresis {
                    0
                } else if distance > self.distances.lod1_distance + hysteresis {
                    2
                } else {
                    1
                }
            }
            2 => {
                if distance < self.distances.lod1_distance - hysteresis {
                    1
                } else if distance > self.distances.lod2_distance + hysteresis {
                    3
                } else {
                    2
                }
            }
            _ => {
                if distance < self.distances.lod2_distance - hysteresis {
                    2
                } else {
                    3
                }
            }
        }
    }

    /// Get vertex skip factor for LOD level
    /// Higher LOD = more vertices skipped
    pub fn get_vertex_skip_factor(lod: u32) -> usize {
        match lod {
            0 => 1, // No skip (full detail)
            1 => 2, // Skip every other vertex
            2 => 4, // Skip 3 out of 4 vertices
            _ => 8, // Skip 7 out of 8 vertices
        }
    }

    /// Calculate triangle budget for LOD level
    pub fn get_triangle_budget(lod: u32) -> usize {
        match lod {
            0 => 2048, // Maximum triangles
            1 => 1024, // Half
            2 => 512,  // Quarter
            _ => 256,  // Eighth
        }
    }

    /// Enable or disable dynamic LOD
    pub fn set_dynamic_lod_enabled(&mut self, enabled: bool) {
        self.enable_dynamic_lod = enabled;
    }

    /// Set custom LOD distances
    pub fn set_lod_distances(&mut self, distances: LODDistances) {
        self.distances = distances;
    }

    /// Adjust LOD distances for performance tuning
    pub fn adjust_lod_distances(&mut self, scale: f32) {
        self.distances.lod0_distance *= scale;
        self.distances.lod1_distance *= scale;
        self.distances.lod2_distance *= scale;
        self.distances.lod3_distance *= scale;
    }
}

/// LOD transition information for smooth blending
pub struct LODTransition {
    /// Current LOD level
    pub current_lod: u32,
    /// Target LOD level
    pub target_lod: u32,
    /// Transition progress (0.0 to 1.0)
    pub progress: f32,
    /// Is transitioning
    pub active: bool,
}

impl LODTransition {
    pub fn new() -> Self {
        Self {
            current_lod: 0,
            target_lod: 0,
            progress: 0.0,
            active: false,
        }
    }

    /// Start a transition to a new LOD level
    pub fn start_transition(&mut self, from_lod: u32, to_lod: u32) {
        self.current_lod = from_lod;
        self.target_lod = to_lod;
        self.progress = 0.0;
        self.active = true;
    }

    /// Update transition progress
    pub fn update(&mut self, delta_time: f32) {
        if !self.active {
            return;
        }

        // Transition over 0.5 seconds
        self.progress += delta_time * 2.0;

        if self.progress >= 1.0 {
            self.progress = 1.0;
            self.current_lod = self.target_lod;
            self.active = false;
        }
    }

    /// Get blend factor for smooth LOD transition
    pub fn get_blend_factor(&self) -> f32 {
        if !self.active {
            return 0.0;
        }

        // Smooth step interpolation
        let t = self.progress;
        t * t * (3.0 - 2.0 * t)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_terrain_lod_enum() {
        assert_eq!(TerrainLOD::Invalid as u32, 0);
        assert_eq!(TerrainLOD::Min as u32, 1);
        assert_eq!(TerrainLOD::StretchNoClouds as u32, 2);
        assert_eq!(TerrainLOD::HalfClouds as u32, 3);
        assert_eq!(TerrainLOD::NoClouds as u32, 4);
        assert_eq!(TerrainLOD::StretchClouds as u32, 5);
        assert_eq!(TerrainLOD::NoWater as u32, 6);
        assert_eq!(TerrainLOD::Max as u32, 7);
        assert_eq!(TerrainLOD::Automatic as u32, 8);
        assert_eq!(TerrainLOD::Disable as u32, 9);
        assert_eq!(TerrainLOD::NumTypes as u32, 10);

        assert!(TerrainLOD::Max.use_cloud_map());
        assert!(TerrainLOD::Max.use_water_plane());
        assert!(!TerrainLOD::Min.use_cloud_map());
    }

    #[test]
    fn test_lod_manager() {
        let manager = TerrainLODManager::new(TerrainLOD::Max);

        let chunk_center = Point3::new(0.0, 0.0, 0.0);
        let camera_near = Point3::new(50.0, 50.0, 0.0);
        let camera_far = Point3::new(1000.0, 1000.0, 0.0);

        let lod_near = manager.calculate_chunk_lod(chunk_center, camera_near);
        let lod_far = manager.calculate_chunk_lod(chunk_center, camera_far);

        assert_eq!(lod_near, 0); // Close = high detail
        assert!(lod_far > lod_near); // Far = lower detail
    }

    #[test]
    fn test_lod_hysteresis() {
        let manager = TerrainLODManager::new(TerrainLOD::Max);

        let chunk_center = Point3::new(0.0, 0.0, 0.0);
        let camera = Point3::new(210.0, 0.0, 0.0);

        // Current LOD is 0, distance is just past threshold
        let lod = manager.calculate_chunk_lod_with_hysteresis(chunk_center, camera, 0);
        // Should stay at 0 due to hysteresis
        assert_eq!(lod, 0);

        // Now with current LOD at 1, should transition
        let lod = manager.calculate_chunk_lod_with_hysteresis(chunk_center, camera, 1);
        assert_eq!(lod, 1);
    }

    #[test]
    fn test_vertex_skip_factor() {
        assert_eq!(TerrainLODManager::get_vertex_skip_factor(0), 1);
        assert_eq!(TerrainLODManager::get_vertex_skip_factor(1), 2);
        assert_eq!(TerrainLODManager::get_vertex_skip_factor(2), 4);
        assert_eq!(TerrainLODManager::get_vertex_skip_factor(3), 8);
    }

    #[test]
    fn test_lod_transition() {
        let mut transition = LODTransition::new();
        transition.start_transition(0, 2);

        assert!(transition.active);
        assert_eq!(transition.current_lod, 0);
        assert_eq!(transition.target_lod, 2);

        // Update halfway
        transition.update(0.25);
        assert!(transition.progress > 0.0 && transition.progress < 1.0);

        // Complete transition
        transition.update(0.5);
        assert_eq!(transition.progress, 1.0);
        assert_eq!(transition.current_lod, 2);
        assert!(!transition.active);
    }

    #[test]
    fn test_blend_factor() {
        let mut transition = LODTransition::new();
        transition.start_transition(0, 1);

        let blend_start = transition.get_blend_factor();
        assert_eq!(blend_start, 0.0);

        transition.progress = 0.5;
        let blend_mid = transition.get_blend_factor();
        assert!(blend_mid > 0.0 && blend_mid < 1.0);

        transition.progress = 1.0;
        let blend_end = transition.get_blend_factor();
        assert_eq!(blend_end, 1.0);
    }
}
