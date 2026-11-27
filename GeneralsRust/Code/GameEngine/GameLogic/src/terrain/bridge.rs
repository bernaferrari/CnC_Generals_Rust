//! TerrainLogic bridge behavior.

use super::*;

impl TerrainLogic {
    pub(crate) fn bridge_info_from_object(bridge_obj: &Object) -> BridgeInfo {
        let position = *bridge_obj.get_position();
        let angle = bridge_obj.get_orientation();
        let geometry = bridge_obj.get_geometry_info();
        BridgeInfo::from_landmark(
            position,
            angle,
            geometry.get_major_radius(),
            geometry.get_minor_radius(),
            bridge_obj.get_id(),
        )
    }

    pub(super) fn bridge_info_from_map_data(
        bridge: &crate::system::map_loader::BridgeData,
        index: i32,
    ) -> Option<BridgeInfo> {
        let [from_left, from_right, to_right, to_left, ..] = bridge.polygon.as_slice() else {
            return None;
        };
        let polygon =
            [from_left, from_right, to_right, to_left].map(|point| Coord2D::new(point.x, point.y));
        BridgeInfo::from_authored(bridge.from, bridge.to, bridge.width, polygon, index)
    }
}
