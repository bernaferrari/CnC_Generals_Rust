// path_optimization.rs
// Path Smoothing and Optimization
// Reference: /GeneralsMD/Code/GameEngine/Source/GameLogic/AI/AIPathfind.cpp:450-696

use super::pathfind_astar::{GridCoord, PathfindLayerEnum, PATHFIND_CELL_SIZE_F};
use crate::common::{Coord2D, Coord3D};

/// Path optimizer that removes redundant waypoints
/// Matches C++ Path::optimize() and Path::optimizeGroundPath()
pub struct PathOptimizer {
    /// Maximum steps allowed when optimizing bridge transitions
    /// Matches C++ ALLOWED_STEPS at AIPathfind.cpp:473
    allowed_bridge_steps: usize,
}

impl PathOptimizer {
    pub fn new() -> Self {
        Self {
            allowed_bridge_steps: 3,
        }
    }

    /// Optimize path by removing unnecessary waypoints using line-of-sight checks
    /// Matches C++ Path::optimize() at AIPathfind.cpp:450-573
    pub fn optimize(
        &self,
        waypoints: &[Coord3D],
        layers: &[PathfindLayerEnum],
        passability_checker: impl Fn(&Coord3D, &Coord3D, PathfindLayerEnum) -> bool,
    ) -> (Vec<Coord3D>, Vec<PathfindLayerEnum>) {
        if waypoints.len() <= 2 {
            return (waypoints.to_vec(), layers.to_vec());
        }

        let mut optimized_waypoints = Vec::new();
        let mut optimized_layers = Vec::new();

        let mut anchor_idx = 0;
        let mut first_node = true;
        let first_layer = layers[anchor_idx];

        optimized_waypoints.push(waypoints[anchor_idx]);
        optimized_layers.push(layers[anchor_idx]);

        while anchor_idx < waypoints.len() - 1 {
            let mut layer = layers[anchor_idx];
            let mut cur_layer = layers[anchor_idx];
            let mut count = 0;

            let mut node_idx = anchor_idx + 1;
            while node_idx + 1 < waypoints.len() {
                count += 1;
                if cur_layer == PathfindLayerEnum::Ground {
                    if layers[node_idx] != cur_layer {
                        layer = layers[node_idx];
                        cur_layer = layer;
                        if count > self.allowed_bridge_steps {
                            break;
                        }
                    }
                } else if layers[node_idx + 1] != cur_layer && count > self.allowed_bridge_steps {
                    break;
                }
                cur_layer = layers[node_idx];
                node_idx += 1;
            }

            if first_node {
                layer = first_layer;
                first_node = false;
            }

            let mut optimized_segment = false;
            let mut test_idx = node_idx;
            while test_idx > anchor_idx {
                let anchor_pos = waypoints[anchor_idx];
                let test_pos = waypoints[test_idx];

                let mut is_passable = passability_checker(&anchor_pos, &test_pos, layer);
                if !is_passable {
                    is_passable = self.is_simple_path_segment(&waypoints[anchor_idx..=test_idx]);
                }

                if is_passable {
                    optimized_waypoints.push(waypoints[test_idx]);
                    optimized_layers.push(layers[test_idx]);
                    anchor_idx = test_idx;
                    optimized_segment = true;
                    break;
                }
                test_idx -= 1;
            }

            if !optimized_segment {
                optimized_waypoints.push(waypoints[anchor_idx + 1]);
                optimized_layers.push(layers[anchor_idx + 1]);
                anchor_idx += 1;
            }
        }

        (optimized_waypoints, optimized_layers)
    }

    /// Optimize ground paths specifically (removes jig-jogs)
    /// Matches C++ Path::optimizeGroundPath() at AIPathfind.cpp:578-696
    pub fn optimize_ground_path(
        &self,
        waypoints: &[Coord3D],
        layers: &[PathfindLayerEnum],
        is_crusher: bool,
        path_diameter: i32,
        passability_checker: impl Fn(&Coord3D, &Coord3D, i32) -> bool,
    ) -> (Vec<Coord3D>, Vec<PathfindLayerEnum>) {
        // First do basic optimization
        let basic_passability = |from: &Coord3D, to: &Coord3D, _layer: PathfindLayerEnum| {
            passability_checker(from, to, path_diameter)
        };

        let (mut optimized, mut opt_layers) = self.optimize(waypoints, layers, basic_passability);

        // Remove jig-jogs (small detours)
        // Matches C++ at AIPathfind.cpp:681-692
        let mut i = 0;
        while i + 2 < optimized.len() {
            let dx = optimized[i + 1].x - optimized[i].x;
            let dy = optimized[i + 1].y - optimized[i].y;
            let dist_sq = dx * dx + dy * dy;

            // If segment is very short (less than 2 cells), remove it
            // Matches C++ threshold at AIPathfind.cpp:688
            let threshold = PATHFIND_CELL_SIZE_F * PATHFIND_CELL_SIZE_F * 3.9;
            if dist_sq < threshold {
                optimized.remove(i + 1);
                opt_layers.remove(i + 1);
            } else {
                i += 1;
            }
        }

        (optimized, opt_layers)
    }

    /// Check if path segment is simple (horizontal, vertical, or diagonal)
    /// Matches C++ logic at AIPathfind.cpp:511-552
    fn is_simple_path_segment(&self, waypoints: &[Coord3D]) -> bool {
        if waypoints.len() < 2 {
            return true;
        }

        let first = &waypoints[0];
        let last = &waypoints[waypoints.len() - 1];

        let dx = last.x - first.x;
        let dy = last.y - first.y;
        let eps = 0.1;
        let cell = PATHFIND_CELL_SIZE_F;

        if (dx.abs() - cell).abs() < eps && (dy.abs() - cell).abs() < eps {
            return true;
        }

        // Check for horizontal path
        if dx.abs() < eps {
            return waypoints.iter().all(|wp| (wp.x - first.x).abs() < eps);
        }

        // Check for vertical path
        if dy.abs() < eps {
            return waypoints.iter().all(|wp| (wp.y - first.y).abs() < eps);
        }

        // Check for diagonal path (dx == dy or dx == -dy)
        if (dx - dy).abs() < eps {
            // Positive diagonal
            return waypoints.iter().all(|wp| {
                let wx = wp.x - first.x;
                let wy = wp.y - first.y;
                (wx - wy).abs() < eps
            });
        }

        if (dx + dy).abs() < eps {
            // Negative diagonal
            return waypoints.iter().all(|wp| {
                let wx = wp.x - first.x;
                let wy = wp.y - first.y;
                (wx + wy).abs() < eps
            });
        }

        false
    }

    /// Smooth path using Catmull-Rom spline or similar
    /// This provides additional smoothing beyond optimization
    pub fn smooth_path(&self, waypoints: &[Coord3D], smoothness: f32) -> Vec<Coord3D> {
        if waypoints.len() <= 2 {
            return waypoints.to_vec();
        }

        let mut smoothed = Vec::new();
        smoothed.push(waypoints[0]); // Keep first point

        for i in 1..waypoints.len() - 1 {
            let prev = waypoints[i - 1];
            let curr = waypoints[i];
            let next = waypoints[i + 1];

            // Simple smoothing: move point towards average of neighbors
            let smooth_x = curr.x + (prev.x + next.x - 2.0 * curr.x) * smoothness * 0.5;
            let smooth_y = curr.y + (prev.y + next.y - 2.0 * curr.y) * smoothness * 0.5;
            let smooth_z = curr.z + (prev.z + next.z - 2.0 * curr.z) * smoothness * 0.5;

            smoothed.push(Coord3D::new(smooth_x, smooth_y, smooth_z));
        }

        smoothed.push(waypoints[waypoints.len() - 1]); // Keep last point

        smoothed
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn always_passable(_from: &Coord3D, _to: &Coord3D, _layer: PathfindLayerEnum) -> bool {
        true
    }

    fn ground_passable(_from: &Coord3D, _to: &Coord3D, _diameter: i32) -> bool {
        true
    }

    #[test]
    fn test_path_optimization_straight_line() {
        let optimizer = PathOptimizer::new();

        // Create zigzag path that should optimize to straight line
        let waypoints = vec![
            Coord3D::new(0.0, 0.0, 0.0),
            Coord3D::new(10.0, 10.0, 0.0),
            Coord3D::new(20.0, 20.0, 0.0),
            Coord3D::new(30.0, 30.0, 0.0),
        ];
        let layers = vec![
            PathfindLayerEnum::Ground,
            PathfindLayerEnum::Ground,
            PathfindLayerEnum::Ground,
            PathfindLayerEnum::Ground,
        ];

        let (optimized, _) = optimizer.optimize(&waypoints, &layers, always_passable);

        // Should reduce to just start and end
        assert_eq!(optimized.len(), 2);
        assert_eq!(optimized[0], waypoints[0]);
        assert_eq!(optimized[1], waypoints[waypoints.len() - 1]);
    }

    #[test]
    fn test_simple_path_detection() {
        let optimizer = PathOptimizer::new();

        // Horizontal path
        let horizontal = vec![
            Coord3D::new(0.0, 5.0, 0.0),
            Coord3D::new(10.0, 5.0, 0.0),
            Coord3D::new(20.0, 5.0, 0.0),
        ];
        assert!(optimizer.is_simple_path_segment(&horizontal));

        // Vertical path
        let vertical = vec![
            Coord3D::new(5.0, 0.0, 0.0),
            Coord3D::new(5.0, 10.0, 0.0),
            Coord3D::new(5.0, 20.0, 0.0),
        ];
        assert!(optimizer.is_simple_path_segment(&vertical));

        // Diagonal path
        let diagonal = vec![
            Coord3D::new(0.0, 0.0, 0.0),
            Coord3D::new(10.0, 10.0, 0.0),
            Coord3D::new(20.0, 20.0, 0.0),
        ];
        assert!(optimizer.is_simple_path_segment(&diagonal));

        // Complex path (should not be simple)
        let complex = vec![
            Coord3D::new(0.0, 0.0, 0.0),
            Coord3D::new(10.0, 5.0, 0.0),
            Coord3D::new(20.0, 0.0, 0.0),
        ];
        assert!(!optimizer.is_simple_path_segment(&complex));
    }

    #[test]
    fn test_jig_jog_removal() {
        let optimizer = PathOptimizer::new();

        // Create path with small jig-jogs
        let waypoints = vec![
            Coord3D::new(0.0, 0.0, 0.0),
            Coord3D::new(10.0, 0.0, 0.0),
            Coord3D::new(11.0, 1.0, 0.0), // Small deviation
            Coord3D::new(20.0, 0.0, 0.0),
        ];
        let layers = vec![
            PathfindLayerEnum::Ground,
            PathfindLayerEnum::Ground,
            PathfindLayerEnum::Ground,
            PathfindLayerEnum::Ground,
        ];

        let (optimized, _) =
            optimizer.optimize_ground_path(&waypoints, &layers, false, 10, ground_passable);

        // Should remove the small deviation
        assert!(optimized.len() < waypoints.len());
    }

    #[test]
    fn test_path_smoothing() {
        let optimizer = PathOptimizer::new();

        let waypoints = vec![
            Coord3D::new(0.0, 0.0, 0.0),
            Coord3D::new(10.0, 5.0, 0.0),
            Coord3D::new(20.0, 0.0, 0.0),
        ];

        let smoothed = optimizer.smooth_path(&waypoints, 0.5);

        assert_eq!(smoothed.len(), waypoints.len());
        assert_eq!(smoothed[0], waypoints[0]); // First unchanged
        assert_eq!(smoothed[2], waypoints[2]); // Last unchanged

        // Middle point should be smoothed
        assert_ne!(smoothed[1], waypoints[1]);
    }
}
