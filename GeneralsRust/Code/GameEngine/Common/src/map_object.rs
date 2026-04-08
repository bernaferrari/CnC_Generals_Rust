// FILE: map_object.rs
// Ported from: GeneralsMD/Code/GameEngine/Include/Common/MapObject.h
// Author: John Ahlquist, April 2001
//
// PARITY_NOTE: MapObject in C++ is a MemoryPoolObject with location, name,
// angle, flags, linked list, properties dict, render/shadow objects, and
// bridge tower arrays.  This is a world-builder editor concept, not a
// runtime gameplay object.  We define the data structure here matching
// the C++ fields.

use std::cell::Cell;

pub const MAP_XY_FACTOR: f32 = 10.0;
pub const MAP_HEIGHT_SCALE: f32 = MAP_XY_FACTOR / 16.0;

pub const FLAG_DRAWS_IN_MIRROR: u32 = 0x00000001;
pub const FLAG_ROAD_POINT1: u32 = 0x00000002;
pub const FLAG_ROAD_POINT2: u32 = 0x00000004;
pub const FLAG_ROAD_FLAGS: u32 = FLAG_ROAD_POINT1 | FLAG_ROAD_POINT2;
pub const FLAG_ROAD_CORNER_ANGLED: u32 = 0x00000008;
pub const FLAG_BRIDGE_POINT1: u32 = 0x00000010;
pub const FLAG_BRIDGE_POINT2: u32 = 0x00000020;
pub const FLAG_BRIDGE_FLAGS: u32 = FLAG_BRIDGE_POINT1 | FLAG_BRIDGE_POINT2;
pub const FLAG_ROAD_CORNER_TIGHT: u32 = 0x00000040;
pub const FLAG_ROAD_JOIN: u32 = 0x00000080;
pub const FLAG_DONT_RENDER: u32 = 0x00000100;

const MO_SELECTED: u32 = 0x01;
const MO_LIGHT: u32 = 0x02;
const MO_WAYPOINT: u32 = 0x04;
const MO_SCORCH: u32 = 0x08;

#[derive(Debug, Clone)]
pub struct Coord3D {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

impl Default for Coord3D {
    fn default() -> Self {
        Self {
            x: 0.0,
            y: 0.0,
            z: 0.0,
        }
    }
}

impl Coord3D {
    pub fn new(x: f32, y: f32, z: f32) -> Self {
        Self { x, y, z }
    }
}

#[derive(Debug, Clone)]
pub struct MapObject {
    pub location: Coord3D,
    pub object_name: String,
    pub angle: f32,
    pub flags: u32,
    pub properties: std::collections::HashMap<String, String>,
    pub color: i32,
    pub runtime_flags: u32,
}

impl MapObject {
    pub fn new(location: Coord3D, name: &str, angle: f32, flags: u32) -> Self {
        Self {
            location,
            object_name: name.to_string(),
            angle,
            flags,
            properties: std::collections::HashMap::new(),
            color: 0,
            runtime_flags: 0,
        }
    }

    pub fn set_angle(&mut self, angle: f32) {
        self.angle = normalize_angle(angle);
    }

    pub fn set_flag(&mut self, flag: u32) {
        self.flags |= flag;
    }

    pub fn clear_flag(&mut self, flag: u32) {
        self.flags &= !flag;
    }

    pub fn get_flag(&self, flag: u32) -> bool {
        (self.flags & flag) != 0
    }

    pub fn is_selected(&self) -> bool {
        (self.runtime_flags & MO_SELECTED) != 0
    }

    pub fn set_selected(&mut self, sel: bool) {
        if sel {
            self.runtime_flags |= MO_SELECTED;
        } else {
            self.runtime_flags &= !MO_SELECTED;
        }
    }

    pub fn is_light(&self) -> bool {
        (self.runtime_flags & MO_LIGHT) != 0
    }

    pub fn is_waypoint(&self) -> bool {
        (self.runtime_flags & MO_WAYPOINT) != 0
    }

    pub fn is_scorch(&self) -> bool {
        (self.runtime_flags & MO_SCORCH) != 0
    }

    pub fn get_properties(&self) -> &std::collections::HashMap<String, String> {
        &self.properties
    }

    pub fn get_properties_mut(&mut self) -> &mut std::collections::HashMap<String, String> {
        &mut self.properties
    }

    pub fn get_location(&self) -> &Coord3D {
        &self.location
    }

    pub fn get_angle(&self) -> f32 {
        self.angle
    }

    pub fn get_name(&self) -> &str {
        &self.object_name
    }

    pub fn set_name(&mut self, name: &str) {
        self.object_name = name.to_string();
    }
}

fn normalize_angle(angle: f32) -> f32 {
    let mut a = angle % 360.0;
    if a < 0.0 {
        a += 360.0;
    }
    a
}
