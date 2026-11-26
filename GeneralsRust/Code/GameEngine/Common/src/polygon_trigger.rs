//! Polygon trigger system for script areas
//! Matches C++ PolygonTrigger.h/PolygonTrigger.cpp

use crate::common::ascii_string::AsciiString;
use crate::common::system::geometry::{ICoord2D as BoundsPoint, IRegion2D};
use crate::common::system::{Snapshotable, Xfer, XferMode};
use generals_game_domain::polygons::TriggerArea;
use glam::{IVec2 as ICoord2D, IVec3 as ICoord3D, Vec2 as Coord2D, Vec3 as Coord3D};

mod map_chunk;
pub use map_chunk::{PolygonTriggerList, decode_polygon_triggers};

/// Polygon trigger identifier.
pub type PolygonTriggerId = i32;

/// Water handle for polygon water areas (matches C++ WaterHandle).
#[derive(Debug, Clone, Copy)]
pub struct WaterHandle {
    polygon_id: PolygonTriggerId,
}

impl WaterHandle {
    pub fn new(polygon_id: PolygonTriggerId) -> Self {
        Self { polygon_id }
    }

    pub fn get_polygon_id(&self) -> PolygonTriggerId {
        self.polygon_id
    }
}

/// Polygon trigger structure
/// Matches C++ PolygonTrigger class from PolygonTrigger.h
#[derive(Debug, Clone)]
pub struct PolygonTrigger {
    id: PolygonTriggerId,
    trigger_name: AsciiString,
    layer_name: AsciiString,
    points: Vec<ICoord3D>,
    bounds: IRegion2D,
    radius: f32,
    bounds_needs_update: bool,
    export_with_scripts: bool,
    is_water_area: bool,
    is_river: bool,
    river_start: i32,
    should_render: bool,
    selected: bool,
}

impl TriggerArea for PolygonTrigger {
    fn trigger_id(&self) -> i32 {
        self.get_id()
    }

    fn trigger_name(&self) -> &str {
        self.get_trigger_name().as_str()
    }

    fn contains_trigger_point(&self, x: i32, y: i32) -> bool {
        self.point_in_trigger_int(&ICoord3D::new(x, y, 0))
    }
}

impl PolygonTrigger {
    pub fn new(id: PolygonTriggerId, name: AsciiString, points: Vec<ICoord3D>) -> Self {
        let mut trigger = Self {
            id,
            trigger_name: name,
            layer_name: AsciiString::new(),
            points,
            bounds: IRegion2D::default(),
            radius: 0.0,
            bounds_needs_update: true,
            export_with_scripts: false,
            is_water_area: false,
            is_river: false,
            river_start: 0,
            should_render: true,
            selected: false,
        };
        trigger.update_bounds();
        trigger
    }

    pub fn get_id(&self) -> PolygonTriggerId {
        self.id
    }

    pub fn get_trigger_name(&self) -> &AsciiString {
        &self.trigger_name
    }

    /// C++ compares `PolygonTrigger*` to the same list node.
    /// Rust stores value clones, so the stable handle is the list id.
    pub fn same_list_node(&self, other: &PolygonTrigger) -> bool {
        if self.id != 0 && other.id != 0 {
            self.id == other.id
        } else {
            self.trigger_name == other.trigger_name
        }
    }

    pub fn set_trigger_name(&mut self, name: AsciiString) {
        self.trigger_name = name;
    }

    pub fn get_layer_name(&self) -> &AsciiString {
        &self.layer_name
    }

    pub fn set_layer_name(&mut self, name: AsciiString) {
        self.layer_name = name;
    }

    pub fn set_should_render(&mut self, toggle: bool) {
        self.should_render = toggle;
    }

    pub fn get_should_render(&self) -> bool {
        self.should_render
    }

    pub fn set_selected(&mut self, toggle: bool) {
        self.selected = toggle;
    }

    pub fn get_selected(&self) -> bool {
        self.selected
    }

    pub fn do_export_with_scripts(&self) -> bool {
        self.export_with_scripts
    }

    pub fn set_do_export_with_scripts(&mut self, val: bool) {
        self.export_with_scripts = val;
    }

    pub fn is_water_area(&self) -> bool {
        self.is_water_area
    }

    pub fn set_water_area(&mut self, val: bool) {
        self.is_water_area = val;
    }

    pub fn is_river(&self) -> bool {
        self.is_river
    }

    pub fn set_river(&mut self, val: bool) {
        self.is_river = val;
    }

    pub fn get_river_start(&self) -> i32 {
        self.river_start
    }

    pub fn set_river_start(&mut self, val: i32) {
        self.river_start = val;
    }

    pub fn get_num_points(&self) -> i32 {
        self.points.len() as i32
    }

    pub fn get_point(&self, ndx: i32) -> Option<&ICoord3D> {
        if self.points.is_empty() {
            return None;
        }
        let mut index = ndx;
        if index < 0 {
            index = 0;
        }
        if index as usize >= self.points.len() {
            index = (self.points.len() - 1) as i32;
        }
        self.points.get(index as usize)
    }

    pub fn add_point(&mut self, point: ICoord3D) {
        self.points.push(point);
        self.update_bounds();
    }

    pub fn set_point(&mut self, point: ICoord3D, ndx: i32) {
        if ndx < 0 {
            return;
        }
        let ndx = ndx as usize;
        if ndx == self.points.len() {
            self.add_point(point);
            return;
        }
        if ndx > self.points.len() {
            return;
        }
        self.points[ndx] = point;
        self.update_bounds();
    }

    pub fn insert_point(&mut self, point: ICoord3D, ndx: i32) {
        if ndx < 0 {
            return;
        }
        let ndx = ndx as usize;
        if ndx == self.points.len() {
            self.add_point(point);
            return;
        }
        if ndx > self.points.len() {
            return;
        }
        self.points.insert(ndx, point);
        self.update_bounds();
    }

    pub fn delete_point(&mut self, ndx: i32) {
        if ndx < 0 {
            return;
        }
        let ndx = ndx as usize;
        if ndx >= self.points.len() {
            return;
        }
        self.points.remove(ndx);
        self.update_bounds();
    }

    /// Return the center with its height sampled from the receiving terrain.
    pub fn get_center_point(&self, ground_height: impl FnOnce(f32, f32) -> f32) -> Coord3D {
        let bounds = self.get_bounds();
        let center_x = (bounds.lo.x + bounds.hi.x) as f32 / 2.0;
        let center_y = (bounds.lo.y + bounds.hi.y) as f32 / 2.0;
        Coord3D::new(center_x, center_y, ground_height(center_x, center_y))
    }

    pub fn get_radius(&self) -> f32 {
        self.get_cached_radius()
    }

    pub fn get_bounds(&self) -> IRegion2D {
        if self.bounds_needs_update {
            return compute_bounds(&self.points).0;
        }
        self.bounds
    }

    pub fn get_bounds_min(&self) -> ICoord2D {
        let point = self.get_bounds().lo;
        ICoord2D::new(point.x, point.y)
    }

    pub fn get_bounds_max(&self) -> ICoord2D {
        let point = self.get_bounds().hi;
        ICoord2D::new(point.x, point.y)
    }

    pub fn get_water_handle(&self) -> Option<WaterHandle> {
        if self.is_water_area {
            Some(WaterHandle::new(self.id))
        } else {
            None
        }
    }

    pub fn is_valid(&self) -> bool {
        !self.points.is_empty()
    }

    /// Check if a point is inside the trigger area
    /// Matches C++ pointInTrigger using ray casting algorithm
    pub fn point_in_trigger(&self, point: &Coord2D) -> bool {
        let point = ICoord3D::new(point.x as i32, point.y as i32, 0);
        self.point_in_trigger_int(&point)
    }

    pub fn point_in_trigger_int(&self, point: &ICoord3D) -> bool {
        let bounds = self.get_bounds();
        if point.x < bounds.lo.x {
            return false;
        }
        if point.y < bounds.lo.y {
            return false;
        }
        if point.x > bounds.hi.x {
            return false;
        }
        if point.y > bounds.hi.y {
            return false;
        }

        let mut inside = false;
        let num_points = self.points.len();
        for i in 0..num_points {
            let pt1 = self.points[i];
            let pt2 = if i == num_points - 1 {
                self.points[0]
            } else {
                self.points[i + 1]
            };

            if pt1.y == pt2.y {
                continue;
            }
            if pt1.y < point.y && pt2.y < point.y {
                continue;
            }
            if pt1.y >= point.y && pt2.y >= point.y {
                continue;
            }
            if pt1.x < point.x && pt2.x < point.x {
                continue;
            }

            let dy = pt2.y - pt1.y;
            let dx = pt2.x - pt1.x;
            let intersection_x =
                pt1.x as f32 + (dx as f32 * (point.y - pt1.y) as f32) / (dy as f32);
            if intersection_x >= point.x as f32 {
                inside = !inside;
            }
        }

        inside
    }

    fn get_cached_radius(&self) -> f32 {
        if self.bounds_needs_update {
            return compute_bounds(&self.points).1;
        }
        self.radius
    }

    fn update_bounds(&mut self) {
        let (bounds, radius) = compute_bounds(&self.points);
        self.bounds = bounds;
        self.radius = radius;
        self.bounds_needs_update = false;
    }
}

impl Snapshotable for PolygonTrigger {
    fn crc(&self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut version: u8 = 1;
        let _ = xfer.xfer_version(&mut version, 1);
        let mut v = self.points.len() as i32;
        let _ = xfer.xfer_int(&mut v);
        for pt in &self.points {
            let mut px = pt.x;
            let _ = xfer.xfer_int(&mut px);
            let mut py = pt.y;
            let _ = xfer.xfer_int(&mut py);
            let mut pz = pt.z;
            let _ = xfer.xfer_int(&mut pz);
        }
        let mut v = self.bounds.lo.x;
        let _ = xfer.xfer_int(&mut v);
        let mut v = self.bounds.lo.y;
        let _ = xfer.xfer_int(&mut v);
        let mut v = self.bounds.hi.x;
        let _ = xfer.xfer_int(&mut v);
        let mut v = self.bounds.hi.y;
        let _ = xfer.xfer_int(&mut v);
        let mut v = self.radius;
        let _ = xfer.xfer_real(&mut v);
        let mut v = self.bounds_needs_update;
        let _ = xfer.xfer_bool(&mut v);
        Ok(())
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut version: u8 = 1;
        let _ = xfer.xfer_version(&mut version, 1);

        let mut num_points = self.points.len() as i32;
        let _ = xfer.xfer_int(&mut num_points);

        if xfer.get_xfer_mode() == XferMode::Load {
            self.points.clear();
            self.points.reserve(num_points.max(0) as usize);
        }

        for _ in 0..num_points {
            let mut point = ICoord3D::ZERO;
            let _ = xfer.xfer_int(&mut point.x);
            let _ = xfer.xfer_int(&mut point.y);
            let _ = xfer.xfer_int(&mut point.z);
            if xfer.get_xfer_mode() == XferMode::Load {
                self.points.push(point);
            }
        }

        let mut bounds = self.bounds;
        let _ = xfer.xfer_int(&mut bounds.lo.x);
        let _ = xfer.xfer_int(&mut bounds.lo.y);
        let _ = xfer.xfer_int(&mut bounds.hi.x);
        let _ = xfer.xfer_int(&mut bounds.hi.y);
        self.bounds = bounds;

        let _ = xfer.xfer_real(&mut self.radius);
        let _ = xfer.xfer_bool(&mut self.bounds_needs_update);

        if xfer.get_xfer_mode() == XferMode::Load && self.bounds_needs_update {
            self.update_bounds();
        }
        Ok(())
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        Ok(())
    }
}

fn compute_bounds(points: &[ICoord3D]) -> (IRegion2D, f32) {
    let big = 0x7ffff0;
    let mut bounds = IRegion2D::new(BoundsPoint::new(big, big), BoundsPoint::new(-big, -big));
    for point in points {
        if point.x < bounds.lo.x {
            bounds.lo.x = point.x;
        }
        if point.y < bounds.lo.y {
            bounds.lo.y = point.y;
        }
        if point.x > bounds.hi.x {
            bounds.hi.x = point.x;
        }
        if point.y > bounds.hi.y {
            bounds.hi.y = point.y;
        }
    }
    let half_width = (bounds.hi.x - bounds.lo.x) as f32 / 2.0;
    let half_height = (bounds.hi.y + bounds.lo.y) as f32 / 2.0;
    let radius = (half_height * half_height + half_width * half_width).sqrt();
    (bounds, radius)
}

#[cfg(test)]
mod trigger_enter_exit_tests {
    use super::*;

    fn square(id: PolygonTriggerId, name: &str) -> PolygonTrigger {
        PolygonTrigger::new(
            id,
            AsciiString::from(name),
            vec![
                ICoord3D::new(0, 0, 0),
                ICoord3D::new(10, 0, 0),
                ICoord3D::new(10, 10, 0),
                ICoord3D::new(0, 10, 0),
            ],
        )
    }

    #[test]
    fn exit_classification_uses_old_ipos_not_new() {
        let trigger = square(1, "Area");
        let old_i_pos = ICoord3D::new(5, 5, 0);
        let new_i_pos = ICoord3D::new(15, 15, 0);
        assert!(
            trigger.point_in_trigger_int(&old_i_pos),
            "old cell is inside; C++ Object.cpp:2599 exits from this cell"
        );
        assert!(
            !trigger.point_in_trigger_int(&new_i_pos),
            "new cell is outside"
        );
    }

    #[test]
    fn never_tracked_polygon_can_still_be_entered() {
        let trigger = square(2, "Untracked");
        let new_i_pos = ICoord3D::new(5, 5, 0);
        assert!(
            trigger.point_in_trigger_int(&new_i_pos),
            "C++ Object.cpp:2615 walks every PolygonTrigger, including never-tracked ones"
        );
    }

    #[test]
    fn same_list_node_matches_cloned_trigger_not_pointer() {
        // C++ Object.cpp:2526 `m_triggerInfo[i].pTrigger == pTrigger`
        let a = square(7, "Zone");
        let b = square(7, "Zone");
        assert!(a.same_list_node(&b));
        assert!(!a.same_list_node(&square(8, "Zone")));
    }
}
