//! TerrainLogic bridge behavior.

use super::*;

impl TerrainLogic {
    fn bridge_pathfinder_bounds(bridge_info: &BridgeInfo) -> (GridCoord, GridCoord) {
        let min_x = bridge_info
            .from_left
            .x
            .min(bridge_info.from_right.x)
            .min(bridge_info.to_left.x)
            .min(bridge_info.to_right.x);
        let max_x = bridge_info
            .from_left
            .x
            .max(bridge_info.from_right.x)
            .max(bridge_info.to_left.x)
            .max(bridge_info.to_right.x);
        let min_y = bridge_info
            .from_left
            .y
            .min(bridge_info.from_right.y)
            .min(bridge_info.to_left.y)
            .min(bridge_info.to_right.y);
        let max_y = bridge_info
            .from_left
            .y
            .max(bridge_info.from_right.y)
            .max(bridge_info.to_left.y)
            .max(bridge_info.to_right.y);

        (
            GridCoord::new(
                (min_x / PATHFIND_CELL_SIZE_F).floor() as i32,
                (min_y / PATHFIND_CELL_SIZE_F).floor() as i32,
            ),
            GridCoord::new(
                (max_x / PATHFIND_CELL_SIZE_F).floor() as i32,
                (max_y / PATHFIND_CELL_SIZE_F).floor() as i32,
            ),
        )
    }

    pub(super) fn bridge_info_from_parts(
        position: Coord3D,
        angle: Real,
        halfsize_x: Real,
        halfsize_y: Real,
        bridge_object_id: ObjectID,
    ) -> BridgeInfo {
        let c = angle.cos();
        let s = angle.sin();

        let from_left = Coord3D::new(
            position.x - halfsize_x * c - halfsize_y * s,
            position.y + halfsize_y * c - halfsize_x * s,
            position.z,
        );
        let to_left = Coord3D::new(
            position.x + halfsize_x * c - halfsize_y * s,
            position.y + halfsize_y * c + halfsize_x * s,
            position.z,
        );
        let from_right = Coord3D::new(
            position.x - halfsize_x * c + halfsize_y * s,
            position.y - halfsize_y * c - halfsize_x * s,
            position.z,
        );
        let to_right = Coord3D::new(
            position.x + halfsize_x * c + halfsize_y * s,
            position.y - halfsize_y * c + halfsize_x * s,
            position.z,
        );

        let mut bridge_info = BridgeInfo::new();
        bridge_info.from_left = from_left;
        bridge_info.from_right = from_right;
        bridge_info.to_left = to_left;
        bridge_info.to_right = to_right;
        bridge_info.from = Coord3D::new(
            (from_left.x + from_right.x) * 0.5,
            (from_left.y + from_right.y) * 0.5,
            (from_left.z + from_right.z) * 0.5,
        );
        bridge_info.to = Coord3D::new(
            (to_left.x + to_right.x) * 0.5,
            (to_left.y + to_right.y) * 0.5,
            (to_left.z + to_right.z) * 0.5,
        );
        bridge_info.bridge_width = halfsize_y * 2.0;
        bridge_info.bridge_object_id = bridge_object_id;
        bridge_info
    }

    pub(crate) fn bridge_info_from_object(bridge_obj: &Object) -> BridgeInfo {
        let position = *bridge_obj.get_position();
        let angle = bridge_obj.get_orientation();
        let geometry = bridge_obj.get_geometry_info();
        Self::bridge_info_from_parts(
            position,
            angle,
            geometry.get_major_radius(),
            geometry.get_minor_radius(),
            bridge_obj.get_id(),
        )
    }

    pub(super) fn register_bridge_with_pathfinder(
        bridge_info: &BridgeInfo,
    ) -> Option<PathfindLayerEnum> {
        use crate::ai::pathfind_complete::{GridCoord, PATHFIND_CELL_SIZE_F};
        let (min_coord, max_coord) = Self::bridge_pathfinder_bounds(bridge_info);
        // C++ PathfindLayer::classifyCells: offset from/to along bridgeDir by
        // 0.7 * PATHFIND_CELL_SIZE before flooring to m_startCell / m_endCell.
        let mut bridge_dir = Coord3D::new(
            bridge_info.to.x - bridge_info.from.x,
            bridge_info.to.y - bridge_info.from.y,
            bridge_info.to.z - bridge_info.from.z,
        );
        let len = (bridge_dir.x * bridge_dir.x
            + bridge_dir.y * bridge_dir.y
            + bridge_dir.z * bridge_dir.z)
            .sqrt();
        if len > 0.0001 {
            bridge_dir.x = bridge_dir.x / len * PATHFIND_CELL_SIZE_F * 0.7;
            bridge_dir.y = bridge_dir.y / len * PATHFIND_CELL_SIZE_F * 0.7;
            bridge_dir.z = bridge_dir.z / len * PATHFIND_CELL_SIZE_F * 0.7;
        }
        let start_world = Coord3D::new(
            bridge_info.from.x - bridge_dir.x,
            bridge_info.from.y - bridge_dir.y,
            bridge_info.from.z - bridge_dir.z,
        );
        let end_world = Coord3D::new(
            bridge_info.to.x + bridge_dir.x,
            bridge_info.to.y + bridge_dir.y,
            bridge_info.to.z + bridge_dir.z,
        );
        let start_cell = GridCoord::from_world(&start_world);
        let end_cell = GridCoord::from_world(&end_world);
        let ai_store = the_ai();let ai_guard = ai_store.read().ok()?;
        let pathfinder = ai_guard.pathfinder()?;
        let mut pathfinder_guard = pathfinder.write().ok()?;
        Some(pathfinder_guard.add_bridge_ex(
            (min_coord, max_coord),
            bridge_info.bridge_object_id,
            start_cell,
            end_cell,
        ))
    }
    pub(super) fn bridge_info_from_map_data(
        bridge: &crate::system::map_loader::BridgeData,
        index: i32,
    ) -> Option<BridgeInfo> {
        if bridge.polygon.len() < 4 {
            return None;
        }
        let dx = bridge.to.x - bridge.from.x;
        let dy = bridge.to.y - bridge.from.y;
        if dx * dx + dy * dy <= f32::EPSILON {
            return None;
        }
        let mut info = BridgeInfo::new();
        info.from = Coord3D::new(bridge.from.x, bridge.from.y, bridge.from.z);
        info.to = Coord3D::new(bridge.to.x, bridge.to.y, bridge.to.z);
        info.bridge_width = bridge.width;
        info.from_left = Coord3D::new(bridge.polygon[0].x, bridge.polygon[0].y, bridge.from.z);
        info.from_right = Coord3D::new(bridge.polygon[1].x, bridge.polygon[1].y, bridge.from.z);
        info.to_right = Coord3D::new(bridge.polygon[2].x, bridge.polygon[2].y, bridge.to.z);
        info.to_left = Coord3D::new(bridge.polygon[3].x, bridge.polygon[3].y, bridge.to.z);
        info.bridge_index = index;
        Some(info)
    }

    pub(super) fn remove_bridge_at(&mut self, location: &Coord3D) -> bool {
        let mut current = &mut self.bridge_list_head;
        loop {
            let should_remove = match current.as_ref() {
                Some(bridge) => bridge.is_point_on_bridge(location),
                None => return false,
            };

            if should_remove {
                let next = current.as_mut().and_then(|bridge| bridge.next.take());
                *current = next;
                self.bridge_damage_states_changed = true;
                return true;
            }

            current = &mut current.as_mut().expect("bridge node exists").next;
        }
    }
}
