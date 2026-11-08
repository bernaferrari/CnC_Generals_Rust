//! Core path value types shared by the locomotor (`ActivePath`) and the AI layer.
//!
//! The standalone terrain-cost table / A* engine that used to live here has been
//! removed: path computation belongs to the classic owned pathfinder
//! (`ai/pathfind_complete`, C++ AIPathfind.cpp).

use crate::common::{Coord3D, Real};
use std::f32::INFINITY;

/// Movement layers for different unit types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum PathfindLayerEnum {
    Invalid = 0,
    Ground = 1,
    Top = 2,
    Bridge1 = 3,
    Bridge2 = 4,
    Bridge3 = 5,
    Bridge4 = 6,
    Bridge5 = 7,
    Bridge6 = 8,
    Bridge7 = 9,
    Bridge8 = 10,
    Bridge9 = 11,
    Bridge10 = 12,
    Bridge11 = 13,
    Bridge12 = 14,
    Wall = 15,
    Tunnel = 16,
    Water = 17,
    Air = 18,
}

impl From<crate::common::PathfindLayerEnum> for PathfindLayerEnum {
    fn from(layer: crate::common::PathfindLayerEnum) -> Self {
        PathfindLayerEnum::from_u32(layer as u32)
    }
}

impl From<PathfindLayerEnum> for crate::common::PathfindLayerEnum {
    fn from(layer: PathfindLayerEnum) -> Self {
        if layer == PathfindLayerEnum::Invalid {
            crate::common::PathfindLayerEnum::Ground
        } else {
            crate::common::PathfindLayerEnum::from_u32(layer as u32)
        }
    }
}

impl PathfindLayerEnum {
    fn from_u32(value: u32) -> Self {
        match value {
            0 => Self::Invalid,
            1 => Self::Ground,
            2 => Self::Top,
            3 => Self::Bridge1,
            4 => Self::Bridge2,
            5 => Self::Bridge3,
            6 => Self::Bridge4,
            7 => Self::Bridge5,
            8 => Self::Bridge6,
            9 => Self::Bridge7,
            10 => Self::Bridge8,
            11 => Self::Bridge9,
            12 => Self::Bridge10,
            13 => Self::Bridge11,
            14 => Self::Bridge12,
            15 => Self::Wall,
            16 => Self::Tunnel,
            17 => Self::Water,
            18 => Self::Air,
            _ => Self::Invalid,
        }
    }
}

/// Movement capabilities for pathfinding
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct MovementCapabilities {
    /// Movement layer (ground, air, water, tunnel)
    pub layer: PathfindLayerEnum,
    /// Can cross water
    pub amphibious: bool,
    /// Can crush obstacles
    pub crusher: bool,
    /// Can climb cliffs
    pub climber: bool,
    /// Can fly
    pub flying: bool,
    /// Can tunnel
    pub tunneling: bool,
    /// Surface type mask (bitmask of acceptable terrain)
    pub surface_mask: u32,
}

impl MovementCapabilities {
    /// Create ground unit capabilities
    pub fn ground() -> Self {
        Self {
            layer: PathfindLayerEnum::Ground,
            amphibious: false,
            crusher: false,
            climber: false,
            flying: false,
            tunneling: false,
            surface_mask: 0x01, // Only clear terrain
        }
    }

    /// Create amphibious unit capabilities
    pub fn amphibious() -> Self {
        Self {
            layer: PathfindLayerEnum::Ground,
            amphibious: true,
            crusher: false,
            climber: false,
            flying: false,
            tunneling: false,
            surface_mask: 0x09, // Clear + Water
        }
    }

    /// Create aircraft capabilities
    pub fn air() -> Self {
        Self {
            layer: PathfindLayerEnum::Air,
            amphibious: true,
            crusher: false,
            climber: true,
            flying: true,
            tunneling: false,
            surface_mask: 0xFFFFFFFF, // All terrain
        }
    }

    /// Create naval unit capabilities
    pub fn naval() -> Self {
        Self {
            layer: PathfindLayerEnum::Water,
            amphibious: false,
            crusher: false,
            climber: false,
            flying: false,
            tunneling: false,
            surface_mask: 0x18, // Water + DeepWater
        }
    }
}

// ============================================================================
// PATH STRUCTURES
// ============================================================================

/// Waypoint in a path
#[derive(Debug, Clone)]
pub struct PathWaypoint {
    /// Position in world coordinates
    pub position: Coord3D,
    /// Movement layer at this waypoint
    pub layer: PathfindLayerEnum,
    /// Distance along path from start
    pub distance: f32,
}

/// Complete path result
#[derive(Debug, Clone)]
pub struct Path {
    /// Waypoints making up the path
    pub waypoints: Vec<PathWaypoint>,
    /// Total path cost
    pub total_cost: f32,
    /// Whether path is complete or partial
    pub complete: bool,
    /// Whether path is optimized
    pub optimized: bool,
    /// Frame when path was created
    pub created_frame: u32,
}

impl Path {
    /// Get total path length
    pub fn length(&self) -> f32 {
        if let Some(last) = self.waypoints.last() {
            last.distance
        } else {
            0.0
        }
    }

    /// Get number of waypoints
    pub fn waypoint_count(&self) -> usize {
        self.waypoints.len()
    }

    /// Get first waypoint
    pub fn first_waypoint(&self) -> Option<&PathWaypoint> {
        self.waypoints.first()
    }

    /// Get last waypoint
    pub fn last_waypoint(&self) -> Option<&PathWaypoint> {
        self.waypoints.last()
    }

    /// Get closest waypoint to position
    pub fn closest_waypoint(&self, pos: &Coord3D) -> Option<(usize, f32)> {
        let mut closest_idx = 0;
        let mut closest_dist = INFINITY;

        for (i, waypoint) in self.waypoints.iter().enumerate() {
            let dist = (*pos - waypoint.position).length();
            if dist < closest_dist {
                closest_dist = dist;
                closest_idx = i;
            }
        }

        if closest_dist < INFINITY {
            Some((closest_idx, closest_dist))
        } else {
            None
        }
    }

    /// Remove waypoints up to index
    pub fn advance_to_waypoint(&mut self, idx: usize) {
        if idx < self.waypoints.len() {
            self.waypoints.drain(0..idx);

            // Recalculate distances
            let mut distance = 0.0;
            for i in 0..self.waypoints.len() {
                if i > 0 {
                    distance +=
                        (self.waypoints[i].position - self.waypoints[i - 1].position).length();
                }
                self.waypoints[i].distance = distance;
            }
        }
    }
}
