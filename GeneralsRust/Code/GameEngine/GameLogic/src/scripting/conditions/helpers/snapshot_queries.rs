//! Borrowed query algorithms shared by explicit host snapshots and migration adapters.
use super::{
    HostScriptQueryObject, HostScriptQuerySnapshot, HostTriggerWorld, host_xz_to_trigger_point,
};

impl HostScriptQuerySnapshot {
    pub fn named_unit_id(&self, name: &str) -> Option<u32> {
        if name.is_empty() {
            return None;
        }
        self.named.get(name).copied()
    }

    pub fn has_any(&self) -> bool {
        !self.named.is_empty()
            || !self.objects.is_empty()
            || !self.team_ids.is_empty()
            || !self.tech_buildings.is_empty()
    }

    pub fn named_unit_alive(&self, name: &str) -> Option<bool> {
        if name.is_empty() {
            return None;
        }
        self.objects
            .iter()
            .find(|object| object.name == name)
            .map(|object| object.alive)
            .or_else(|| self.named.contains_key(name).then_some(true))
    }

    pub fn object_by_id(&self, id: u32) -> Option<&HostScriptQueryObject> {
        self.objects.iter().find(|object| object.id == id)
    }

    pub fn team_unit_ids(&self, team: u32) -> &[u32] {
        self.team_ids
            .get(&team)
            .map(Vec::as_slice)
            .unwrap_or_default()
    }

    pub fn named_unit_in_area(&self, name: &str, bounds: (f32, f32, f32, f32)) -> bool {
        let (min_x, min_z, max_x, max_z) = bounds;
        self.objects.iter().any(|object| {
            object.alive
                && object.name == name
                && object.x >= min_x
                && object.x <= max_x
                && object.z >= min_z
                && object.z <= max_z
        })
    }

    pub fn named_unit_point_in_trigger(
        &self,
        name: &str,
        trigger: &crate::polygon_trigger::PolygonTrigger,
    ) -> bool {
        self.objects.iter().any(|object| {
            object.name == name
                && trigger.point_in_trigger_int(&host_xz_to_trigger_point(object.x, object.z))
        })
    }

    /// Names are already resolved by the caller's script context. No active world lookup.
    /// Polygon membership uses the original integer XY conversion; AABB pads retain
    /// their inclusive bounds and alive filter. Missing geometry is distinct from outside.
    pub fn named_unit_in_named_area(
        &self,
        unit_name: &str,
        area_name: &str,
        triggers: &HostTriggerWorld,
    ) -> Option<bool> {
        if area_name.is_empty() {
            return None;
        }
        if let Some(trigger) = triggers.trigger_area_by_canonical_name(area_name) {
            return Some(self.named_unit_point_in_trigger(unit_name, &trigger));
        }
        Some(self.named_unit_in_area(unit_name, *self.areas.get(area_name)?))
    }
}
