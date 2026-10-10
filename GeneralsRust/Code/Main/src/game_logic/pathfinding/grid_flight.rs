//! Read-only flight layer queries. C++ TerrainLogic.cpp:1701-1735,
//! W3DTerrainLogic.cpp:281-328 and WWMath/tri.h:70-174.
use super::*;

impl PathfindingGrid {
    pub(crate) fn highest_layer_at_or_below(
        &self,
        pos: Vec3,
        raw_ground: f32,
        only_healthy: bool,
    ) -> u8 {
        let mut best = 1; // LAYER_GROUND
        let mut distance = pos.y - raw_ground; // Deliberately signed in C++.
        if distance > self.wall_height * 0.5 && self.is_point_on_wall(pos) {
            let delta = pos.y - self.wall_height;
            if delta >= 0.0 && delta.abs() < distance.abs() {
                best = 15; // LAYER_WALL
                distance = delta;
            }
        }
        for id in &self.flight_bridge_order {
            let bridge = self
                .bridge_layers
                .iter()
                .find(|b| b.id == *id)
                .expect("flight visitation references an admitted bridge");
            if only_healthy && bridge.destroyed {
                continue;
            }
            let corners = [
                bridge.from_left,
                bridge.from_right,
                bridge.to_right,
                bridge.to_left,
            ];
            if flight_point_on_bridge(pos, &corners) {
                let delta = pos.y - bridge_deck_height(&corners, pos.x, pos.z);
                if delta >= 0.0 && delta.abs() < distance.abs() {
                    best = bridge.id;
                    distance = delta;
                }
            }
        }
        best
    }

    /// Initial map admission receives TerrainLogic's already ordered bridge
    /// list. Dynamic admission prepends in alloc_or_find_bridge_layer instead.
    pub(crate) fn admit_flight_bridge_order(&mut self, spans: &[[Vec3; 4]]) {
        let mut order = Vec::with_capacity(self.bridge_layers.len());
        for c in spans {
            let id = self
                .bridge_layers
                .iter()
                .find(|b| {
                    span_xz_eq(b.from_left, c[0])
                        && span_xz_eq(b.from_right, c[1])
                        && span_xz_eq(b.to_right, c[2])
                        && span_xz_eq(b.to_left, c[3])
                })
                .expect("source bridge was stamped before order admission")
                .id;
            if !order.contains(&id) {
                order.push(id);
            }
        }
        // Retain spans absent from the source adapter after its admitted order,
        // preserving their relative order. Full bridge admission is separate.
        for id in &self.flight_bridge_order {
            if !order.contains(id) {
                order.push(*id);
            }
        }
        self.flight_bridge_order = order;
    }

    /// clip=false keeps the selected non-ground layer beyond its footprint.
    /// Bridge health filters selection only, not this selected-layer read.
    pub(crate) fn layer_height_unclipped(&self, pos: Vec3, layer: u8, raw_ground: f32) -> f32 {
        if layer == 15 {
            return self.wall_height;
        }
        if layer != 1 {
            if let Some(bridge) = self.bridge_layers.iter().find(|b| b.id == layer) {
                let corners = [
                    bridge.from_left,
                    bridge.from_right,
                    bridge.to_right,
                    bridge.to_left,
                ];
                let height = bridge_deck_height(&corners, pos.x, pos.z);
                if height > raw_ground {
                    return height;
                }
            }
        }
        raw_ground
    }
}

fn flight_point_on_bridge(pos: Vec3, c: &[Vec3; 4]) -> bool {
    let lo_x = c.iter().map(|p| p.x).fold(f32::INFINITY, f32::min);
    let hi_x = c.iter().map(|p| p.x).fold(f32::NEG_INFINITY, f32::max);
    let lo_z = c.iter().map(|p| p.z).fold(f32::INFINITY, f32::min);
    let hi_z = c.iter().map(|p| p.z).fold(f32::NEG_INFINITY, f32::max);
    if pos.x < lo_x || pos.x > hi_x || pos.z < lo_z || pos.z > hi_z {
        return false;
    }
    flight_point_in_triangle(c[0], c[1], c[3], pos)
        || flight_point_in_triangle(c[1], c[3], c[2], pos)
}

fn flight_point_in_triangle(a: Vec3, b: Vec3, c: Vec3, p: Vec3) -> bool {
    let xz = |v: Vec3| glam::Vec2::new(v.x, v.z);
    let ab = xz(b - a);
    let bc = xz(c - b);
    let ca = xz(a - c);
    let area = ab.perp_dot(xz(c - a));
    if area != 0.0 {
        let side = if area > 0.0 { 1.0 } else { -1.0 };
        return !(ab.perp_dot(xz(p - a)) * side < 0.0
            || bc.perp_dot(xz(p - b)) * side < 0.0
            || ca.perp_dot(xz(p - c)) * side < 0.0);
    }
    let ab2 = ab.length_squared();
    let bc2 = bc.length_squared();
    // Preserve tri.h's original repeated p1p2.Length2(), including degeneracy.
    let ca2 = bc2;
    let (segment, offset, length2) = if ab2 > bc2 && ab2 > ca2 {
        (ab, xz(p - a), ab2)
    } else if ab2 <= bc2 && bc2 > ca2 {
        (bc, xz(p - b), bc2)
    } else {
        (ca, xz(p - c), ca2)
    };
    if length2 != 0.0 {
        segment.perp_dot(offset) == 0.0
            && offset.length_squared() <= length2
            && (offset - segment).length_squared() <= length2
    } else {
        offset.length_squared() == 0.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn span(height: f32, id: u8, destroyed: bool) -> HostBridgeLayer {
        HostBridgeLayer {
            id,
            from_left: Vec3::new(0.0, height, 0.0),
            from_right: Vec3::new(10.0, height, 0.0),
            to_left: Vec3::new(0.0, height, 10.0),
            to_right: Vec3::new(10.0, height, 10.0),
            cells: HashMap::new(),
            destroyed,
            object_id: id as u32,
            ground_connect_cells: Vec::new(),
        }
    }

    #[test]
    fn highest_layer_uses_signed_distance_strict_ties_and_healthy_filter() {
        let mut grid = PathfindingGrid::new(100.0, 100.0, 10.0);
        grid.bridge_layers = vec![span(21.0, 2, false), span(21.0, 3, true)];
        grid.flight_bridge_order = vec![3, 2];
        assert_eq!(
            grid.highest_layer_at_or_below(Vec3::new(5.0, 30.0, 5.0), 0.0, false),
            3
        );
        assert_eq!(
            grid.highest_layer_at_or_below(Vec3::new(5.0, 30.0, 5.0), 0.0, true),
            2
        );
        assert_eq!(
            grid.highest_layer_at_or_below(Vec3::new(5.0, 20.0, 5.0), 0.0, false),
            1
        );
        assert_eq!(
            grid.highest_layer_at_or_below(Vec3::new(5.0, -2.0, 5.0), 0.0, false),
            1
        );
        assert_eq!(
            grid.highest_layer_at_or_below(Vec3::new(5.0, 21.0, 5.0), 21.0, false),
            1
        );
        assert_eq!(
            grid.highest_layer_at_or_below(Vec3::new(0.0, 30.0, 0.0), 0.0, false),
            3
        );
        assert_eq!(
            grid.highest_layer_at_or_below(Vec3::new(-f32::EPSILON, 30.0, 0.0), 0.0, false),
            1
        );
    }

    #[test]
    fn wall_selection_preserves_half_height_gate_and_order_admission() {
        let mut grid = PathfindingGrid::new(100.0, 100.0, 10.0);
        grid.set_wall_height(21.0);
        grid.add_wall_piece(1, Vec3::new(50.0, 0.0, 50.0), 0.0, 40.0, 40.0);
        assert_eq!(
            grid.highest_layer_at_or_below(Vec3::new(50.0, 21.0, 50.0), 0.0, false),
            15
        );
        assert_eq!(
            grid.highest_layer_at_or_below(Vec3::new(50.0, 21.0, 50.0), 18.0, false),
            1
        );
        assert_eq!(
            grid.highest_layer_at_or_below(Vec3::new(50.0, 20.0, 50.0), 0.0, false),
            1
        );
        grid.bridge_layers = vec![span(21.0, 2, false), span(21.0, 3, false)];
        // Distinguish two overlapping spans by one corner while retaining the
        // same plane/covered point and independently supplied source order.
        grid.bridge_layers[1].from_left.x = 1.0;
        grid.flight_bridge_order = vec![3, 2];
        let c = |b: &HostBridgeLayer| [b.from_left, b.from_right, b.to_right, b.to_left];
        let order = [c(&grid.bridge_layers[0]), c(&grid.bridge_layers[1])];
        grid.admit_flight_bridge_order(&order);
        assert_eq!(
            grid.highest_layer_at_or_below(Vec3::new(5.0, 30.0, 5.0), 0.0, false),
            2
        );
        grid.clear_static_blocks();
        assert!(grid.flight_bridge_order.is_empty());
        assert_eq!(
            grid.highest_layer_at_or_below(Vec3::new(5.0, 30.0, 5.0), 0.0, false),
            1
        );
    }

    #[test]
    fn selected_layer_is_unclipped_and_wall_is_not_clamped_to_ground() {
        let mut grid = PathfindingGrid::new(100.0, 100.0, 10.0);
        let mut bridge = span(21.0, 2, true);
        bridge.to_left.y = 31.0;
        bridge.to_right.y = 31.0;
        grid.bridge_layers.push(bridge);
        grid.set_wall_height(55.0);
        assert_eq!(
            grid.layer_height_unclipped(Vec3::new(20.0, 100.0, 20.0), 2, 0.0),
            41.0
        );
        assert_eq!(
            grid.layer_height_unclipped(Vec3::new(20.0, 100.0, 20.0), 2, 50.0),
            50.0
        );
        assert_eq!(grid.layer_height_unclipped(Vec3::ZERO, 15, 80.0), 55.0);
    }
}
