//! Path optimization and smoothing algorithms
//!
//! This module contains algorithms for optimizing paths by removing
//! redundant waypoints and smoothing paths for more natural movement.

use super::{PassabilityQuery, Path};
use crate::common::Coord3D;
use crate::path::LocomotorSurfaceTypeMask;

/// Path optimization utilities
pub struct PathOptimizer;

impl PathOptimizer {
    /// Optimize path by removing redundant nodes
    pub fn optimize_path(
        path: &mut Path,
        acceptable_surfaces: LocomotorSurfaceTypeMask,
        blocked: bool,
        passability: Option<&dyn PassabilityQuery>,
    ) {
        path.optimize_internal(acceptable_surfaces, blocked, passability);
        path.mark_optimized();
    }

    /// Optimize ground path for ground units
    pub fn optimize_ground_path(
        path: &mut Path,
        crusher: bool,
        diameter: i32,
        passability: Option<&dyn PassabilityQuery>,
    ) {
        path.optimize_ground_internal(crusher, diameter, passability);
        path.mark_optimized();
    }

    /// Smooth path for more natural movement
    pub fn smooth_path(path: &mut Path, smoothing_factor: f32) {
        path.smooth(smoothing_factor);
    }

    /// Check line of sight between two points
    pub fn has_line_of_sight(
        from: &Coord3D,
        to: &Coord3D,
        _acceptable_surfaces: LocomotorSurfaceTypeMask,
        passability: Option<&dyn PassabilityQuery>,
    ) -> bool {
        if let Some(query) = passability {
            return query.is_line_passable(_acceptable_surfaces, from, to, false);
        }
        let dx = to.x - from.x;
        let dy = to.y - from.y;
        let dz = to.z - from.z;
        let length_sq = dx * dx + dy * dy + dz * dz;
        length_sq > f32::EPSILON
    }
}
