use std::collections::HashMap;

use super::{AsciiString, PolygonTrigger, PolygonTriggerId};
use crate::common::system::{DataChunkInput, DataChunkOutput};
use crate::map_object::MAP_XY_FACTOR;
use generals_map::BinaryReader;
use glam::IVec3;

const VERSION_WITHOUT_WATER: u16 = 1;
const VERSION_WITH_WATER: u16 = 2;
const VERSION_WITH_RIVER: u16 = 3;
const VERSION_WITH_LAYER: u16 = 4;

/// Decode the rows in a bounded PolygonTriggers payload.
/// A truncated record ends the list after the last complete record.
pub fn decode_polygon_triggers(
    version: u16,
    payload: &[u8],
    water_extent: [f32; 2],
) -> Vec<PolygonTrigger> {
    let mut triggers = Vec::new();
    let mut reader = BinaryReader::new(payload);
    if let Some(max_id) = read_authored_rows(version, &mut reader, &mut triggers) {
        if version == VERSION_WITHOUT_WATER {
            triggers.push(default_water(max_id, water_extent));
        }
    }
    triggers
}

/// The streaming reader retains its zero-filled scalar defaults.
/// The bounded runtime reader stops on incomplete fields.
trait PolygonChunkReader {
    fn int(&mut self) -> Option<i32>;
    fn byte(&mut self) -> Option<u8>;
    fn name(&mut self) -> Option<String>;
}

impl PolygonChunkReader for BinaryReader<'_> {
    fn int(&mut self) -> Option<i32> {
        self.read_i32().ok()
    }

    fn byte(&mut self) -> Option<u8> {
        self.read_u8().ok()
    }

    fn name(&mut self) -> Option<String> {
        self.read_ascii_string().ok()
    }
}

impl PolygonChunkReader for DataChunkInput<'_> {
    fn int(&mut self) -> Option<i32> {
        Some(self.read_int())
    }

    fn byte(&mut self) -> Option<u8> {
        Some(self.read_byte())
    }

    fn name(&mut self) -> Option<String> {
        Some(self.read_ascii_string())
    }
}

fn read_authored_rows(
    version: u16,
    reader: &mut impl PolygonChunkReader,
    triggers: &mut Vec<PolygonTrigger>,
) -> Option<PolygonTriggerId> {
    let count = reader.int()?;
    let mut max_id = 0;
    for _ in 0..count {
        let Some((id, trigger)) = read_authored_row(version, reader) else {
            break;
        };
        max_id = max_id.max(id);
        triggers.extend(trigger);
    }
    Some(max_id)
}

fn read_authored_row(
    version: u16,
    reader: &mut impl PolygonChunkReader,
) -> Option<(PolygonTriggerId, Option<PolygonTrigger>)> {
    let name = reader.name()?;
    let layer = if version >= VERSION_WITH_LAYER {
        AsciiString::from(&reader.name()?)
    } else {
        AsciiString::new()
    };
    let id = reader.int()?;
    let is_water = version >= VERSION_WITH_WATER && reader.byte()? != 0;
    let (is_river, river_start) = if version >= VERSION_WITH_RIVER {
        (reader.byte()? != 0, reader.int()?)
    } else {
        (false, 0)
    };
    let point_count = reader.int()?;
    let mut points = Vec::new();
    for _ in 0..point_count {
        points.push(IVec3::new(reader.int()?, reader.int()?, reader.int()?));
    }
    if points.len() < 2 {
        return Some((id, None));
    }
    let mut trigger = PolygonTrigger::new(id, AsciiString::from(&name), points);
    trigger.set_layer_name(layer);
    trigger.set_water_area(is_water);
    trigger.set_river(is_river);
    trigger.set_river_start(river_start);
    Some((id, Some(trigger)))
}

fn default_water(id: PolygonTriggerId, [extent_x, extent_y]: [f32; 2]) -> PolygonTrigger {
    let border = 30.0 * MAP_XY_FACTOR;
    let min = -border as i32;
    let max_x = (border + extent_x) as i32;
    let max_y = (border + extent_y) as i32;
    let water_height = 7;
    let mut trigger = PolygonTrigger::new(
        id,
        AsciiString::from("AutoAddedWaterAreaTrigger"),
        vec![
            IVec3::new(min, min, water_height),
            IVec3::new(max_x, min, water_height),
            IVec3::new(max_x, max_y, water_height),
            IVec3::new(min, max_y, water_height),
        ],
    );
    trigger.set_water_area(true);
    trigger
}

/// Polygon list with first-match ID and name lookups in authored order.
pub struct PolygonTriggerList {
    triggers: Vec<PolygonTrigger>,
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

    /// Replace the list with the current chunk. Only version 1 reads water extents.
    pub fn read_polygon_triggers_data_chunk(
        &mut self,
        input: &mut DataChunkInput<'_>,
        version: u16,
        water_extent: impl FnOnce() -> [f32; 2],
    ) {
        self.clear();
        let mut rows = Vec::new();
        let mut max_id = read_authored_rows(version, input, &mut rows).unwrap_or_default();
        if version == VERSION_WITHOUT_WATER {
            // PolygonTrigger.cpp uses the current max, then increments it.
            rows.push(default_water(max_id, water_extent()));
            max_id += 1;
        }
        for trigger in rows {
            self.add(trigger);
        }
        self.current_id = max_id + 1;
        if !input.at_end_of_chunk() {
            log::debug!("PolygonTriggers chunk has trailing data; ignoring remainder");
        }
    }

    pub fn write_polygon_triggers_data_chunk(&self, output: &mut DataChunkOutput) {
        output.open_data_chunk("PolygonTriggers", VERSION_WITH_LAYER);
        output.write_int(self.triggers.len() as i32);
        for trigger in &self.triggers {
            output.write_ascii_string(trigger.get_trigger_name().str());
            output.write_ascii_string(trigger.get_layer_name().str());
            output.write_int(trigger.get_id());
            output.write_byte(u8::from(trigger.is_water_area()));
            output.write_byte(u8::from(trigger.is_river()));
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

    pub fn add(&mut self, trigger: PolygonTrigger) {
        let index = self.triggers.len();
        self.id_index.entry(trigger.get_id()).or_insert(index);
        self.triggers.push(trigger);
    }

    pub fn get_by_id(&self, trigger_id: PolygonTriggerId) -> Option<&PolygonTrigger> {
        self.id_index
            .get(&trigger_id)
            .map(|&index| &self.triggers[index])
    }

    pub fn get_by_id_mut(&mut self, trigger_id: PolygonTriggerId) -> Option<&mut PolygonTrigger> {
        let index = *self.id_index.get(&trigger_id)?;
        self.triggers.get_mut(index)
    }

    pub fn get_by_name(&self, name: &str) -> Option<&PolygonTrigger> {
        self.triggers
            .iter()
            .find(|trigger| trigger.get_trigger_name().str() == name)
    }

    pub fn get_by_name_mut(&mut self, name: &str) -> Option<&mut PolygonTrigger> {
        self.triggers
            .iter_mut()
            .find(|trigger| trigger.get_trigger_name().str() == name)
    }

    pub fn get_triggers(&self) -> &[PolygonTrigger] {
        &self.triggers
    }

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_one_water_uses_last_complete_id_and_receiving_extents() {
        let mut payload = Vec::new();
        payload.extend_from_slice(&2i32.to_le_bytes());
        payload.extend_from_slice(&0u16.to_le_bytes());
        payload.extend_from_slice(&15i32.to_le_bytes());
        payload.extend_from_slice(&1i32.to_le_bytes());
        for coordinate in [1i32, 2, 3] {
            payload.extend_from_slice(&coordinate.to_le_bytes());
        }
        // The next record has a higher ID, but its point is truncated.
        payload.extend_from_slice(&0u16.to_le_bytes());
        payload.extend_from_slice(&90i32.to_le_bytes());
        payload.extend_from_slice(&2i32.to_le_bytes());
        payload.extend_from_slice(&5i32.to_le_bytes());

        for ([x, y], max) in [([25.0, 80.0], [325, 380]), ([51.0, 4.0], [351, 304])] {
            let rows = decode_polygon_triggers(1, &payload, [x, y]);
            assert_eq!(rows.len(), 1);
            let water = &rows[0];
            assert_eq!(water.get_id(), 15);
            assert!(water.is_water_area());
            assert_eq!(water.get_point(2), Some(&IVec3::new(max[0], max[1], 7)));
        }
        assert!(decode_polygon_triggers(1, &payload[..3], [25.0, 80.0]).is_empty());
    }
}
