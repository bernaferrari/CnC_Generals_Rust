//! C++ `Radar::addObject` color / list / radar-data back-pointer.

use super::{Coord3D, RadarObject, RadarPriorityType, RadarSystem};
use std::sync::{Arc, OnceLock};

/// Appearance used to color a radar blip the way `Radar::addObject` does.
#[derive(Debug, Clone)]
pub struct RadarObjectInsert {
    pub object: RadarObject,
    pub is_disguiser: bool,
    pub disguised: bool,
    pub disguised_player_index: i32,
    pub owner_player_index: i32,
    pub local_player_index: i32,
    pub owner_is_ally_of_local: bool,
    pub local_player_active: bool,
    pub contain_apparent_player_index: Option<i32>,
    pub contain_apparent_color: Option<u32>,
    pub indicator_color: u32,
    pub owner_player_color: u32,
    pub disguised_player_color: u32,
}

/// Live world objects that should appear on the radar each frame.
pub trait RadarObjectProvider: Send + Sync {
    fn collect_objects(&self) -> Vec<RadarObjectInsert>;
}

/// C++ `obj->friend_setRadarData`.
pub trait RadarDataSink: Send + Sync {
    fn set_radar_data(&self, object_id: u32, present: bool);
}

static OBJECT_PROVIDER: OnceLock<Arc<dyn RadarObjectProvider>> = OnceLock::new();
static DATA_SINK: OnceLock<Arc<dyn RadarDataSink>> = OnceLock::new();

pub fn register_radar_object_provider(provider: Arc<dyn RadarObjectProvider>) -> bool {
    OBJECT_PROVIDER.set(provider).is_ok()
}

pub fn register_radar_data_sink(sink: Arc<dyn RadarDataSink>) -> bool {
    DATA_SINK.set(sink).is_ok()
}

fn data_sink() -> Option<&'static dyn RadarDataSink> {
    DATA_SINK.get().map(|s| s.as_ref())
}

/// C++ `Radar::addObject` color selection.
#[must_use]
pub fn resolve_radar_object_color(spec: &RadarObjectInsert) -> u32 {
    let mut player_color = spec.owner_player_color;
    let mut use_indicator = true;

    if spec.is_disguiser && spec.disguised {
        if !spec.owner_is_ally_of_local && spec.local_player_active {
            player_color = spec.disguised_player_color;
            use_indicator = false;
        }
    }

    if let Some(color) = spec.contain_apparent_color {
        return color;
    }
    if spec.contain_apparent_player_index.is_some() {
        use_indicator = false;
        if let Some(idx) = spec.contain_apparent_player_index {
            if idx == spec.local_player_index {
                player_color = spec.owner_player_color;
            }
        }
    }

    if use_indicator {
        spec.indicator_color
    } else {
        player_color
    }
}

impl RadarSystem {
    /// C++ `Radar::addObject` over a fully resolved world object.
    pub fn add_live_object(&mut self, mut spec: RadarObjectInsert) {
        if !spec.object.priority.is_visible() {
            return;
        }
        spec.object.color = resolve_radar_object_color(&spec);
        spec.object.is_disguised = spec.disguised;
        let object_id = spec.object.object_id;
        self.remove_object(object_id);
        self.add_object(spec.object);
        if let Some(sink) = data_sink() {
            sink.set_radar_data(object_id, true);
        }
    }

    /// Rebuild lists from the registered world-object provider.
    pub fn sync_objects_from_provider(&mut self) {
        let Some(provider) = OBJECT_PROVIDER.get() else {
            return;
        };
        let live = provider.collect_objects();
        let live_ids: std::collections::HashSet<u32> =
            live.iter().map(|spec| spec.object.object_id).collect();
        let stale: Vec<u32> = self
            .get_all_objects()
            .filter(|obj| !live_ids.contains(&obj.object_id))
            .map(|obj| obj.object_id)
            .collect();
        for id in stale {
            if self.remove_object(id) {
                if let Some(sink) = data_sink() {
                    sink.set_radar_data(id, false);
                }
            }
        }
        for spec in live {
            self.add_live_object(spec);
        }
    }
}

/// Helper for tests / callers that only have a color + priority.
#[must_use]
pub fn insert_spec_from_object(object: RadarObject) -> RadarObjectInsert {
    let color = object.color;
    RadarObjectInsert {
        object,
        is_disguiser: false,
        disguised: false,
        disguised_player_index: -1,
        owner_player_index: 0,
        local_player_index: 0,
        owner_is_ally_of_local: true,
        local_player_active: true,
        contain_apparent_player_index: None,
        contain_apparent_color: None,
        indicator_color: color,
        owner_player_color: color,
        disguised_player_color: color,
    }
}

impl RadarObject {
    pub fn with_world_pos(mut self, pos: Coord3D) -> Self {
        self.world_pos = pos;
        self
    }

    pub fn with_priority(mut self, priority: RadarPriorityType) -> Self {
        self.priority = priority;
        self
    }
}
