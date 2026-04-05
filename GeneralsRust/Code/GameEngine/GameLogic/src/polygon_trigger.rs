//! Polygon trigger system for script areas
//! Matches C++ PolygonTrigger.h/PolygonTrigger.cpp

use crate::common::xfer::{Xfer, XferExt};
use crate::common::{
    AsciiString, Bool, Coord2D, Coord3D, ICoord2D, ICoord3D, IRegion2D, Int, Real, Snapshot,
    MAP_XY_FACTOR,
};
use crate::helpers::TheTerrainLogic;
use game_engine::common::system::{DataChunkInfo, DataChunkInput, DataChunkOutput};
use std::collections::HashMap;

/// Polygon trigger identifier
pub type PolygonTriggerId = Int;

const K_TRIGGERS_VERSION_1: u16 = 1;
const K_TRIGGERS_VERSION_2: u16 = 2;
const K_TRIGGERS_VERSION_3: u16 = 3;
const K_TRIGGERS_VERSION_4: u16 = 4;

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
    radius: Real,
    bounds_needs_update: Bool,
    export_with_scripts: Bool,
    is_water_area: Bool,
    is_river: Bool,
    river_start: Int,
    should_render: Bool,
    selected: Bool,
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

    pub fn set_trigger_name(&mut self, name: AsciiString) {
        self.trigger_name = name;
    }

    pub fn get_layer_name(&self) -> &AsciiString {
        &self.layer_name
    }

    pub fn set_layer_name(&mut self, name: AsciiString) {
        self.layer_name = name;
    }

    pub fn set_should_render(&mut self, toggle: Bool) {
        self.should_render = toggle;
    }

    pub fn get_should_render(&self) -> Bool {
        self.should_render
    }

    pub fn set_selected(&mut self, toggle: Bool) {
        self.selected = toggle;
    }

    pub fn get_selected(&self) -> Bool {
        self.selected
    }

    pub fn do_export_with_scripts(&self) -> Bool {
        self.export_with_scripts
    }

    pub fn set_do_export_with_scripts(&mut self, val: Bool) {
        self.export_with_scripts = val;
    }

    pub fn is_water_area(&self) -> Bool {
        self.is_water_area
    }

    pub fn set_water_area(&mut self, val: Bool) {
        self.is_water_area = val;
    }

    pub fn is_river(&self) -> Bool {
        self.is_river
    }

    pub fn set_river(&mut self, val: Bool) {
        self.is_river = val;
    }

    pub fn get_river_start(&self) -> Int {
        self.river_start
    }

    pub fn set_river_start(&mut self, val: Int) {
        self.river_start = val;
    }

    pub fn get_num_points(&self) -> Int {
        self.points.len() as Int
    }

    pub fn get_point(&self, ndx: Int) -> Option<&ICoord3D> {
        if self.points.is_empty() {
            return None;
        }
        let mut index = ndx;
        if index < 0 {
            index = 0;
        }
        if index as usize >= self.points.len() {
            index = (self.points.len() - 1) as Int;
        }
        self.points.get(index as usize)
    }

    pub fn add_point(&mut self, point: ICoord3D) {
        self.points.push(point);
        self.update_bounds();
    }

    pub fn set_point(&mut self, point: ICoord3D, ndx: Int) {
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

    pub fn insert_point(&mut self, point: ICoord3D, ndx: Int) {
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

    pub fn delete_point(&mut self, ndx: Int) {
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

    /// Get center point of the polygon
    /// Matches C++ getCenterPoint
    pub fn get_center_point(&self) -> Coord3D {
        let bounds = self.get_bounds();
        let center_x = (bounds.lo.x + bounds.hi.x) as Real / 2.0;
        let center_y = (bounds.lo.y + bounds.hi.y) as Real / 2.0;
        let mut center = Coord3D::new(center_x, center_y, 0.0);
        if let Some(terrain) = TheTerrainLogic::get() {
            center.z = terrain.get_ground_height(center.x, center.y, None);
        }
        center
    }

    pub fn get_radius(&self) -> Real {
        self.get_cached_radius()
    }

    pub fn get_bounds(&self) -> IRegion2D {
        if self.bounds_needs_update {
            return compute_bounds(&self.points).0;
        }
        self.bounds
    }

    pub fn get_bounds_min(&self) -> ICoord2D {
        self.get_bounds().lo
    }

    pub fn get_bounds_max(&self) -> ICoord2D {
        self.get_bounds().hi
    }

    pub fn get_water_handle(&self) -> Option<WaterHandle> {
        if self.is_water_area {
            Some(WaterHandle::new(self.id))
        } else {
            None
        }
    }

    pub fn is_valid(&self) -> Bool {
        !self.points.is_empty()
    }

    /// Check if a point is inside the trigger area
    /// Matches C++ pointInTrigger using ray casting algorithm
    pub fn point_in_trigger(&self, point: &Coord2D) -> bool {
        let point = ICoord3D::new(point.x as Int, point.y as Int, 0);
        self.point_in_trigger_int(&point)
    }

    pub fn point_in_trigger_int(&self, point: &ICoord3D) -> Bool {
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
                pt1.x as Real + (dx as Real * (point.y - pt1.y) as Real) / (dy as Real);
            if intersection_x >= point.x as Real {
                inside = !inside;
            }
        }

        inside
    }

    fn get_cached_radius(&self) -> Real {
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

impl Snapshot for PolygonTrigger {
    fn crc(&self, xfer: &mut dyn Xfer) {
        let mut version: u8 = 1;
        let _ = xfer.xfer_version(&mut version, 1);
        let mut v = self.points.len() as Int;
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
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) {
        let mut version: u8 = 1;
        let _ = xfer.xfer_version(&mut version, 1);

        let mut num_points = self.points.len() as Int;
        let _ = xfer.xfer_int(&mut num_points);

        if xfer.is_loading() {
            self.points.clear();
            self.points.reserve(num_points.max(0) as usize);
        }

        for _ in 0..num_points {
            let mut point = ICoord3D::ZERO;
            let _ = xfer.xfer_int(&mut point.x);
            let _ = xfer.xfer_int(&mut point.y);
            let _ = xfer.xfer_int(&mut point.z);
            if xfer.is_loading() {
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

        if xfer.is_loading() && self.bounds_needs_update {
            self.update_bounds();
        }
    }

    fn load_post_process(&mut self) {}
}

/// Container for polygon triggers
/// Matches C++ ThePolygonTriggerListPtr
pub struct PolygonTriggerList {
    triggers: Vec<PolygonTrigger>,
    /// Maps trigger ID to index in `triggers` for O(1) lookup.
    id_index: HashMap<PolygonTriggerId, usize>,
    current_id: PolygonTriggerId,
}

impl PolygonTriggerList {
    pub fn new() -> Self {
        Self {
            triggers: Vec::new(),
            id_index: HashMap::new(),
            current_id: 1,
        }
    }

    pub fn parse_polygon_triggers_data_chunk(
        input: &mut DataChunkInput,
        info: &DataChunkInfo,
        user_data: &mut dyn std::any::Any,
    ) -> bool {
        let Some(list) = user_data.downcast_mut::<PolygonTriggerList>() else {
            return false;
        };
        list.clear();

        let mut max_trigger_id = 0;
        let mut count = input.read_int();
        while count > 0 {
            count -= 1;
            let trigger_name = AsciiString::from(&input.read_ascii_string());
            let mut layer_name = AsciiString::new();
            if info.version >= K_TRIGGERS_VERSION_4 {
                layer_name = AsciiString::from(&input.read_ascii_string());
            }
            let trigger_id = input.read_int();
            let mut is_water = false;
            if info.version >= K_TRIGGERS_VERSION_2 {
                is_water = input.read_byte() != 0;
            }
            let mut is_river = false;
            let mut river_start = 0;
            if info.version >= K_TRIGGERS_VERSION_3 {
                is_river = input.read_byte() != 0;
                river_start = input.read_int();
            }

            let num_points = input.read_int();
            let mut trigger = PolygonTrigger::new(trigger_id, trigger_name, Vec::new());
            trigger.set_layer_name(layer_name);
            trigger.set_water_area(is_water);
            trigger.set_river(is_river);
            trigger.set_river_start(river_start);

            for _ in 0..num_points {
                let x = input.read_int();
                let y = input.read_int();
                let z = input.read_int();
                trigger.add_point(ICoord3D::new(x, y, z));
            }

            if num_points < 2 {
                continue;
            }

            if trigger_id > max_trigger_id {
                max_trigger_id = trigger_id;
            }

            list.add(trigger);
        }

        if info.version == K_TRIGGERS_VERSION_1 {
            let mut trigger = PolygonTrigger::new(
                max_trigger_id + 1,
                AsciiString::from("AutoAddedWaterAreaTrigger"),
                Vec::new(),
            );
            trigger.set_water_area(true);

            let extent =
                TheTerrainLogic::get().map(|terrain| terrain.get_extent_including_border());
            let water_extent_x = extent.map(|e| e.hi.x as Int).unwrap_or(0);
            let water_extent_y = extent.map(|e| e.hi.y as Int).unwrap_or(0);
            let border = (30.0 * MAP_XY_FACTOR) as Int;

            let mut point = ICoord3D::new(-border, -border, 7);
            trigger.add_point(point);
            point.x = border + water_extent_x;
            trigger.add_point(point);
            point.y = border + water_extent_y;
            trigger.add_point(point);
            point.x = -border;
            trigger.add_point(point);
            list.add(trigger);
            max_trigger_id += 1;
        }

        list.current_id = max_trigger_id + 1;
        input.at_end_of_chunk()
    }

    pub fn write_polygon_triggers_data_chunk(&self, output: &mut DataChunkOutput) {
        output.open_data_chunk("PolygonTriggers", K_TRIGGERS_VERSION_4);

        output.write_int(self.triggers.len() as Int);
        for trigger in &self.triggers {
            output.write_ascii_string(trigger.get_trigger_name().str());
            output.write_ascii_string(trigger.get_layer_name().str());
            output.write_int(trigger.get_id());
            output.write_byte(trigger.is_water_area() as u8);
            output.write_byte(trigger.is_river() as u8);
            output.write_int(trigger.get_river_start());
            output.write_int(trigger.get_num_points());
            for point in &trigger.points {
                output.write_int(point.x);
                output.write_int(point.y);
                output.write_int(point.z);
            }
        }

        output.close_data_chunk();
    }

    /// Add a polygon trigger to the list
    pub fn add(&mut self, trigger: PolygonTrigger) {
        let idx = self.triggers.len();
        self.id_index.insert(trigger.id, idx);
        self.triggers.push(trigger);
    }

    /// Get polygon trigger by ID
    /// Matches C++ getPolygonTriggerByID
    pub fn get_by_id(&self, trigger_id: PolygonTriggerId) -> Option<&PolygonTrigger> {
        self.id_index
            .get(&trigger_id)
            .map(|&idx| &self.triggers[idx])
    }

    /// Get mutable polygon trigger by ID.
    pub fn get_by_id_mut(&mut self, trigger_id: PolygonTriggerId) -> Option<&mut PolygonTrigger> {
        let idx = *self.id_index.get(&trigger_id)?;
        self.triggers.get_mut(idx)
    }

    /// Get polygon trigger by name
    /// Matches C++ getPolygonTriggerByName
    pub fn get_by_name(&self, name: &str) -> Option<&PolygonTrigger> {
        self.triggers.iter().find(|t| t.trigger_name.str() == name)
    }

    pub fn get_by_name_mut(&mut self, name: &str) -> Option<&mut PolygonTrigger> {
        self.triggers
            .iter_mut()
            .find(|t| t.trigger_name.str() == name)
    }

    pub fn get_triggers(&self) -> &[PolygonTrigger] {
        &self.triggers
    }

    /// Get next available trigger ID
    pub fn next_id(&mut self) -> PolygonTriggerId {
        let id = self.current_id;
        self.current_id += 1;
        id
    }

    pub fn len(&self) -> usize {
        self.triggers.len()
    }

    pub fn is_empty(&self) -> bool {
        self.triggers.is_empty()
    }

    pub fn clear(&mut self) {
        self.triggers.clear();
        self.id_index.clear();
        self.current_id = 1;
    }
}

impl Default for PolygonTriggerList {
    fn default() -> Self {
        Self::new()
    }
}

fn compute_bounds(points: &[ICoord3D]) -> (IRegion2D, Real) {
    let big = 0x7ffff0;
    let mut bounds = IRegion2D::new(ICoord2D::new(big, big), ICoord2D::new(-big, -big));
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
    let half_width = (bounds.hi.x - bounds.lo.x) as Real / 2.0;
    let half_height = (bounds.hi.y + bounds.lo.y) as Real / 2.0;
    let radius = (half_height * half_height + half_width * half_width).sqrt();
    (bounds, radius)
}
