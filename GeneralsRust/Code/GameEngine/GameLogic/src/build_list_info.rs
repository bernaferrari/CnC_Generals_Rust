//! Build list info system - port of C++ BuildListInfo in SidesList.h

use crate::common::xfer::{Xfer, XferExt};
use crate::common::{
    AsciiString, Bool, Coord2D, Coord3D, CoordOrigin, INVALID_ID, Int, ObjectID, Real, Snapshot,
    UnsignedInt,
};

#[derive(Debug, Clone)]
pub struct BuildListInfo {
    building_name: AsciiString,
    template_name: AsciiString,
    location: Coord3D,
    rally_point_offset: Coord2D,
    angle: Real,
    initially_built: Bool,
    num_rebuilds: UnsignedInt,
    next_build_list: Option<Box<BuildListInfo>>,
    script: AsciiString,
    health: Int,
    whiner: Bool,
    unsellable: Bool,
    repairable: Bool,
    automatically_build: Bool,
    object_id: ObjectID,
    object_timestamp: UnsignedInt,
    under_construction: Bool,
    resource_gatherers: [ObjectID; BuildListInfo::MAX_RESOURCE_GATHERERS],
    is_supply_building: Bool,
    desired_gatherers: Int,
    current_gatherers: Int,
    priority_build: Bool,
}

impl Default for BuildListInfo {
    fn default() -> Self {
        Self::new()
    }
}

impl BuildListInfo {
    pub const UNLIMITED_REBUILDS: UnsignedInt = 0xFFFF_FFFF;
    pub const MAX_RESOURCE_GATHERERS: usize = 10;

    pub fn new() -> Self {
        Self {
            building_name: AsciiString::new(),
            template_name: AsciiString::new(),
            location: Coord3D::origin(),
            rally_point_offset: Coord2D::origin(),
            angle: 0.0,
            initially_built: false,
            num_rebuilds: 0,
            next_build_list: None,
            script: AsciiString::new(),
            health: 100,
            whiner: true,
            unsellable: false,
            repairable: true,
            automatically_build: true,
            object_id: INVALID_ID,
            object_timestamp: 0,
            under_construction: false,
            resource_gatherers: [INVALID_ID; BuildListInfo::MAX_RESOURCE_GATHERERS],
            is_supply_building: false,
            desired_gatherers: 0,
            current_gatherers: 0,
            priority_build: false,
        }
    }

    pub fn get_next(&self) -> Option<&BuildListInfo> {
        self.next_build_list.as_deref()
    }

    pub fn get_next_mut(&mut self) -> Option<&mut BuildListInfo> {
        self.next_build_list.as_deref_mut()
    }

    /// Visit this entry and every following linked build-list entry in the
    /// original list order.
    ///
    /// This deliberately uses a scoped callback instead of exposing a mutable
    /// iterator: a linked node cannot safely yield `&mut self` while retaining
    /// a mutable borrow of its `next` field for the iterator's next step.
    pub(crate) fn for_each_mut(&mut self, mut visit: impl FnMut(&mut BuildListInfo)) {
        self.visit_chain_mut(&mut visit);
    }

    fn visit_chain_mut(&mut self, visit: &mut impl FnMut(&mut BuildListInfo)) {
        visit(self);
        if let Some(next) = self.next_build_list.as_deref_mut() {
            next.visit_chain_mut(visit);
        }
    }

    pub fn set_next_build_list(&mut self, next: Option<BuildListInfo>) {
        self.next_build_list = next.map(Box::new);
    }

    pub fn set_next_build_list_boxed(&mut self, next: Option<Box<BuildListInfo>>) {
        self.next_build_list = next;
    }

    pub fn take_next_build_list(&mut self) -> Option<Box<BuildListInfo>> {
        self.next_build_list.take()
    }

    pub fn get_building_name(&self) -> AsciiString {
        self.building_name.clone()
    }

    pub fn set_building_name(&mut self, name: AsciiString) {
        self.building_name = name;
    }

    pub fn get_template_name(&self) -> AsciiString {
        self.template_name.clone()
    }

    pub fn set_template_name(&mut self, name: AsciiString) {
        self.template_name = name;
    }

    pub fn get_location(&self) -> &Coord3D {
        &self.location
    }

    pub fn set_location(&mut self, loc: Coord3D) {
        self.location = loc;
    }

    pub fn get_rally_offset(&self) -> &Coord2D {
        &self.rally_point_offset
    }

    pub fn set_rally_offset(&mut self, offset: Coord2D) {
        self.rally_point_offset = offset;
    }

    pub fn get_angle(&self) -> Real {
        self.angle
    }

    pub fn set_angle(&mut self, angle: Real) {
        self.angle = angle;
    }

    pub fn is_initially_built(&self) -> Bool {
        self.initially_built
    }

    pub fn set_initially_built(&mut self, built: Bool) {
        self.initially_built = built;
    }

    pub fn get_num_rebuilds(&self) -> UnsignedInt {
        self.num_rebuilds
    }

    pub fn set_num_rebuilds(&mut self, num_rebuilds: UnsignedInt) {
        self.num_rebuilds = num_rebuilds;
    }

    pub fn decrement_num_rebuilds(&mut self) {
        if self.num_rebuilds > 0 && self.num_rebuilds != Self::UNLIMITED_REBUILDS {
            self.num_rebuilds -= 1;
        }
    }

    pub fn increment_num_rebuilds(&mut self) {
        if self.num_rebuilds != Self::UNLIMITED_REBUILDS {
            self.num_rebuilds = self.num_rebuilds.saturating_add(1);
        }
    }

    pub fn is_buildable(&self) -> Bool {
        self.num_rebuilds > 0 || self.num_rebuilds == Self::UNLIMITED_REBUILDS
    }

    pub fn get_script(&self) -> AsciiString {
        self.script.clone()
    }

    pub fn set_script(&mut self, script: AsciiString) {
        self.script = script;
    }

    pub fn get_health(&self) -> Int {
        self.health
    }

    pub fn set_health(&mut self, health: Int) {
        self.health = health;
    }

    pub fn get_whiner(&self) -> Bool {
        self.whiner
    }

    pub fn set_whiner(&mut self, whiner: Bool) {
        self.whiner = whiner;
    }

    pub fn get_unsellable(&self) -> Bool {
        self.unsellable
    }

    pub fn set_unsellable(&mut self, unsellable: Bool) {
        self.unsellable = unsellable;
    }

    pub fn get_repairable(&self) -> Bool {
        self.repairable
    }

    pub fn set_repairable(&mut self, repairable: Bool) {
        self.repairable = repairable;
    }

    pub fn is_automatic_build(&self) -> Bool {
        self.automatically_build
    }

    pub fn set_automatic_build(&mut self, enabled: Bool) {
        self.automatically_build = enabled;
    }

    pub fn set_object_id(&mut self, obj_id: ObjectID) {
        self.object_id = obj_id;
    }

    pub fn get_object_id(&self) -> ObjectID {
        self.object_id
    }

    pub fn set_object_timestamp(&mut self, frame: UnsignedInt) {
        self.object_timestamp = frame;
    }

    pub fn get_object_timestamp(&self) -> UnsignedInt {
        self.object_timestamp
    }

    pub fn is_under_construction(&self) -> Bool {
        self.under_construction
    }

    pub fn set_under_construction(&mut self, construction: Bool) {
        self.under_construction = construction;
    }

    pub fn mark_priority_build(&mut self) {
        self.priority_build = true;
    }

    pub fn is_priority_build(&self) -> Bool {
        self.priority_build
    }

    pub fn is_supply_building(&self) -> Bool {
        self.is_supply_building
    }

    pub fn set_supply_building(&mut self, is_supply: Bool) {
        self.is_supply_building = is_supply;
    }

    pub fn get_gatherer_id(&self, idx: Int) -> ObjectID {
        if idx >= 0 && (idx as usize) < Self::MAX_RESOURCE_GATHERERS {
            self.resource_gatherers[idx as usize]
        } else {
            INVALID_ID
        }
    }

    pub fn set_gatherer_id(&mut self, idx: Int, id: ObjectID) {
        if idx >= 0 && (idx as usize) < Self::MAX_RESOURCE_GATHERERS {
            self.resource_gatherers[idx as usize] = id;
        }
    }

    pub fn get_desired_gatherers(&self) -> Int {
        self.desired_gatherers
    }

    pub fn set_desired_gatherers(&mut self, desired: Int) {
        self.desired_gatherers = desired;
    }

    pub fn get_current_gatherers(&self) -> Int {
        self.current_gatherers
    }

    pub fn set_current_gatherers(&mut self, current: Int) {
        self.current_gatherers = current;
    }

    pub fn duplicate(&self) -> BuildListInfo {
        self.clone()
    }
}

impl Snapshot for BuildListInfo {
    fn crc(&self, xfer: &mut dyn Xfer) {
        let mut version: u8 = 2;
        let _ = xfer.xfer_version(&mut version, 2);
        let mut v = self.building_name.as_str().to_string();
        let _ = xfer.xfer_ascii_string(&mut v);
        let mut v = self.template_name.as_str().to_string();
        let _ = xfer.xfer_ascii_string(&mut v);
        let mut v = self.location;
        xfer.xfer_coord3d(&mut v);
        let mut v = self.rally_point_offset;
        xfer.xfer_coord2d(&mut v);
        let mut v = self.angle;
        let _ = xfer.xfer_real(&mut v);
        let mut v = self.initially_built;
        let _ = xfer.xfer_bool(&mut v);
        let mut v = self.num_rebuilds;
        let _ = xfer.xfer_unsigned_int(&mut v);
        let mut v = self.script.as_str().to_string();
        let _ = xfer.xfer_ascii_string(&mut v);
        let mut v = self.health;
        let _ = xfer.xfer_int(&mut v);
        let mut v = self.whiner;
        let _ = xfer.xfer_bool(&mut v);
        let mut v = self.unsellable;
        let _ = xfer.xfer_bool(&mut v);
        let mut v = self.repairable;
        let _ = xfer.xfer_bool(&mut v);
        let mut v = self.automatically_build;
        let _ = xfer.xfer_bool(&mut v);
        let mut v = self.object_id;
        let _ = xfer.xfer_object_id(&mut v);
        let mut v = self.object_timestamp;
        let _ = xfer.xfer_unsigned_int(&mut v);
        let mut v = self.under_construction;
        let _ = xfer.xfer_bool(&mut v);
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) {
        let mut version: u8 = 2;
        let _ = xfer.xfer_version(&mut version, 2);

        let mut building_name = self.building_name.as_str().to_string();
        let _ = xfer.xfer_ascii_string(&mut building_name);
        if xfer.is_reading() {
            self.building_name = AsciiString::from(&building_name);
        }

        let mut template_name = self.template_name.as_str().to_string();
        let _ = xfer.xfer_ascii_string(&mut template_name);
        if xfer.is_reading() {
            self.template_name = AsciiString::from(&template_name);
        }

        let mut location = self.location;
        xfer.xfer_coord3d(&mut location);
        self.location = location;

        let mut rally_point_offset = self.rally_point_offset;
        xfer.xfer_coord2d(&mut rally_point_offset);
        self.rally_point_offset = rally_point_offset;

        let _ = xfer.xfer_real(&mut self.angle);
        let _ = xfer.xfer_bool(&mut self.initially_built);
        let _ = xfer.xfer_unsigned_int(&mut self.num_rebuilds);

        let mut script = self.script.as_str().to_string();
        let _ = xfer.xfer_ascii_string(&mut script);
        if xfer.is_reading() {
            self.script = AsciiString::from(&script);
        }

        let _ = xfer.xfer_int(&mut self.health);
        let _ = xfer.xfer_bool(&mut self.whiner);
        let _ = xfer.xfer_bool(&mut self.unsellable);
        let _ = xfer.xfer_bool(&mut self.repairable);
        let _ = xfer.xfer_bool(&mut self.automatically_build);

        let _ = xfer.xfer_object_id(&mut self.object_id);
        let _ = xfer.xfer_unsigned_int(&mut self.object_timestamp);
        let _ = xfer.xfer_bool(&mut self.under_construction);

        for entry in &mut self.resource_gatherers {
            let _ = xfer.xfer_object_id(entry);
        }

        let _ = xfer.xfer_bool(&mut self.is_supply_building);
        let _ = xfer.xfer_int(&mut self.desired_gatherers);
        let _ = xfer.xfer_bool(&mut self.priority_build);

        if version >= 2 {
            let _ = xfer.xfer_int(&mut self.current_gatherers);
        } else if xfer.is_reading() {
            self.current_gatherers = 0;
        }
    }

    fn load_post_process(&mut self) {}
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn for_each_mut_visits_and_mutates_the_entire_linked_list_in_order() {
        let mut third = BuildListInfo::new();
        third.set_building_name(AsciiString::from("third"));

        let mut second = BuildListInfo::new();
        second.set_building_name(AsciiString::from("second"));
        second.set_next_build_list(Some(third));

        let mut first = BuildListInfo::new();
        first.set_building_name(AsciiString::from("first"));
        first.set_next_build_list(Some(second));

        let mut names = Vec::new();
        let mut index = 0;
        first.for_each_mut(|entry| {
            names.push(entry.get_building_name().as_str().to_owned());
            entry.set_angle(index as Real);
            index += 1;
        });

        assert_eq!(names, ["first", "second", "third"]);
        assert_eq!(first.get_angle(), 0.0);
        assert_eq!(first.get_next().map(BuildListInfo::get_angle), Some(1.0));
        assert_eq!(
            first
                .get_next()
                .and_then(BuildListInfo::get_next)
                .map(BuildListInfo::get_angle),
            Some(2.0)
        );
    }
}
