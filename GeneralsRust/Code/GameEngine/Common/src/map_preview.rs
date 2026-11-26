//! Read map selection metadata without admitting the map into a game.

use std::collections::HashMap;
use std::io;

use crate::common::name_key_generator::NameKeyNamespace;
use crate::common::system::skirmish_game_info::SKIRMISH_MAX_SLOTS;
use crate::common::system::{
    DataChunkInfo, DataChunkInput,
    geometry::{Coord3D, Region3D},
};
use crate::map_data::{MapObjectChunk, uncompressed_map_bytes};
use crate::map_object::MAP_XY_FACTOR;
use glam::Vec3;

/// Marks placed on the map preview from the receiving catalog's template traits.
pub enum PreviewMarker {
    Tech,
    Supply,
}

#[derive(Debug, Default)]
pub struct MapPreview {
    pub extent: Region3D,
    pub name_lookup_tag: String,
    pub waypoints: HashMap<String, Vec3>,
    pub supply_positions: Vec<Vec3>,
    pub tech_positions: Vec<Vec3>,
}

impl MapPreview {
    /// Read the dimensions, WorldInfo and Object rows used by C++ MapUtil.
    /// The caller supplies the template classification from its content catalog.
    pub fn read(
        bytes: &[u8],
        mut classify: impl FnMut(&str) -> Option<PreviewMarker>,
    ) -> io::Result<Self> {
        let bytes = uncompressed_map_bytes(bytes)?.into_owned();
        let mut names = NameKeyNamespace::new();
        let mut input = DataChunkInput::with_namespace(bytes, &mut names);
        input.register_parser("HeightMapData", "", read_extent);
        input.register_parser("WorldInfo", "", read_world_info);
        input.register_parser("ObjectsList", "", read_objects);

        let mut fields = PreviewFields::default();
        if !input.parse(&mut fields) || !fields.has_extent {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "Invalid map preview data",
            ));
        }
        for (name, position) in fields.placed_objects {
            match classify(&name) {
                Some(PreviewMarker::Tech) => fields.preview.tech_positions.push(position),
                Some(PreviewMarker::Supply) => fields.preview.supply_positions.push(position),
                None => {}
            }
        }
        Ok(fields.preview)
    }

    /// C++ WaypointMap::update counts contiguous named starts, with a minimum of one.
    pub fn count_start_spots(&self) -> usize {
        (1..=SKIRMISH_MAX_SLOTS)
            .take_while(|slot| self.waypoints.contains_key(&format!("Player_{slot}_Start")))
            .count()
            .max(1)
    }
}

/// Validate the terrain payload for the campaign map availability check.
pub fn map_has_terrain(bytes: &[u8]) -> bool {
    let Ok(bytes) = uncompressed_map_bytes(bytes) else {
        return false;
    };
    let Ok(map) = generals_map::ChunkyMap::decode(Default::default(), bytes.into_owned()) else {
        return false;
    };
    matches!(generals_map::decode_heightmap(&map), Ok(Some(terrain)) if terrain.width > 0 && terrain.height > 0)
}

#[derive(Default)]
struct PreviewFields {
    preview: MapPreview,
    placed_objects: Vec<(String, Vec3)>,
    has_extent: bool,
}

fn read_extent(
    input: &mut DataChunkInput<'_>,
    info: &DataChunkInfo,
    user: &mut dyn std::any::Any,
) -> bool {
    let Some(fields) = user.downcast_mut::<PreviewFields>() else {
        return false;
    };
    let required = match info.version {
        0..=2 => 8,
        3 => 12,
        _ => 16,
    };
    if input.remaining_in_chunk() < required {
        return false;
    }
    let width = input.read_int();
    let height = input.read_int();
    let border = if info.version >= 3 {
        input.read_int()
    } else {
        0
    };
    if info.version >= 4 {
        let count = input.read_int().max(0) as usize;
        if count > input.remaining_in_chunk() / 8 {
            return false;
        }
    }
    if width <= 0 || height <= 0 {
        return false;
    }
    // ParseSizeOnly uses the authored v1 dimensions even when terrain resamples them.
    let width = (i64::from(width) - 2 * i64::from(border)) as f32;
    let height = (i64::from(height) - 2 * i64::from(border)) as f32;
    fields.preview.extent = Region3D::new(
        Coord3D::ZERO,
        Coord3D::new(width * MAP_XY_FACTOR, height * MAP_XY_FACTOR, 0.0),
    );
    fields.has_extent = true;
    true
}

fn read_world_info(
    input: &mut DataChunkInput<'_>,
    _info: &DataChunkInfo,
    user: &mut dyn std::any::Any,
) -> bool {
    let Some(fields) = user.downcast_mut::<PreviewFields>() else {
        return false;
    };
    let dict = input.read_dict();
    let key = input.intern_name("mapName");
    fields.preview.name_lookup_tag = dict.get_ascii_string(key);
    true
}

fn read_objects(
    input: &mut DataChunkInput<'_>,
    info: &DataChunkInfo,
    user: &mut dyn std::any::Any,
) -> bool {
    input.register_parser("Object", &info.label, read_object);
    input.parse(user)
}

fn read_object(
    input: &mut DataChunkInput<'_>,
    info: &DataChunkInfo,
    user: &mut dyn std::any::Any,
) -> bool {
    let Some(fields) = user.downcast_mut::<PreviewFields>() else {
        return false;
    };
    let row = MapObjectChunk::read(input, info.version);
    if let Some((_, name)) = row.waypoint(input) {
        fields.preview.waypoints.insert(name, row.position);
    } else {
        fields.placed_objects.push((row.name, row.position));
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::common_services::{CommonServices, with_common_services};
    use crate::common::dict::Dict;
    use crate::common::name_key_generator::NameKeyGenerator;
    use crate::common::system::DataChunkOutput;
    use std::sync::Arc;

    fn write_object(
        out: &mut DataChunkOutput,
        version: u16,
        name: &str,
        position: [f32; 3],
        properties: &Dict,
    ) {
        out.open_data_chunk("Object", version);
        for coord in position {
            out.write_real(coord);
        }
        out.write_real(0.0);
        out.write_int(0);
        out.write_ascii_string(name);
        if version >= 2 {
            out.write_dict(properties);
        }
        out.close_data_chunk();
    }

    fn preview_fixture(height_fields: &[i32], height_version: u16, samples: &[u8]) -> Vec<u8> {
        with_common_services(&Arc::new(CommonServices::new()), || {
            let mut out = DataChunkOutput::new();
            out.open_data_chunk("HeightMapData", height_version);
            for value in height_fields {
                out.write_int(*value);
            }
            if !samples.is_empty() {
                out.write_int(samples.len() as i32);
                for sample in samples {
                    out.write_byte(*sample);
                }
            }
            out.close_data_chunk();

            let mut world = Dict::new();
            world.set_ascii_string(NameKeyGenerator::name_to_key("mapName"), "MAP:Fixture");
            out.open_data_chunk("WorldInfo", 1);
            out.write_dict(&world);
            out.close_data_chunk();

            let waypoint_id = NameKeyGenerator::name_to_key("waypointID");
            let waypoint_name = NameKeyGenerator::name_to_key("waypointName");
            out.open_data_chunk("ObjectsList", 1);
            for (slot, x) in [(1, 10.0), (1, 20.0), (3, 30.0)] {
                let mut waypoint = Dict::new();
                waypoint.set_int(waypoint_id, slot);
                waypoint.set_ascii_string(waypoint_name, &format!("Player_{slot}_Start"));
                write_object(&mut out, 2, "Marker", [x, 18.0, 99.0], &waypoint);
            }
            write_object(&mut out, 3, "Building", [11.0, 10.0, 45.0], &Dict::new());
            write_object(&mut out, 3, "Supply", [60.0, 90.0, 8.0], &Dict::new());
            out.close_data_chunk();

            // Metadata readers do not dispatch a SidesList or PolygonTriggers parser.
            out.open_data_chunk("SidesList", 7);
            out.write_byte(255);
            out.close_data_chunk();
            out.open_data_chunk("PolygonTriggers", 7);
            out.write_byte(255);
            out.close_data_chunk();
            out.into_ckmp_bytes()
        })
    }

    #[test]
    fn preview_reads_header_only_and_keeps_each_receiving_catalog_separate() {
        let bytes = preview_fixture(&[5, 3], 1, &[]);
        let other = Arc::new(CommonServices::new());
        with_common_services(&other, || {
            let a = MapPreview::read(&bytes, |name| match name {
                "Building" => Some(PreviewMarker::Tech),
                "Supply" => Some(PreviewMarker::Supply),
                _ => panic!("waypoints must not go through template classification"),
            })
            .unwrap();
            let b = MapPreview::read(&bytes, |name| {
                (name == "Building").then_some(PreviewMarker::Supply)
            })
            .unwrap();

            assert_eq!(a.extent.hi, Coord3D::new(50.0, 30.0, 0.0));
            assert_eq!(a.name_lookup_tag, "MAP:Fixture");
            assert_eq!(a.count_start_spots(), 1);
            assert_eq!(a.waypoints["Player_1_Start"], Vec3::new(20.0, 18.0, 0.0));
            assert_eq!(a.waypoints["Player_3_Start"], Vec3::new(30.0, 18.0, 0.0));
            assert_eq!(a.tech_positions, [Vec3::new(11.0, 10.0, 45.0)]);
            assert_eq!(a.supply_positions, [Vec3::new(60.0, 90.0, 8.0)]);
            assert!(b.tech_positions.is_empty());
            assert_eq!(b.supply_positions, [Vec3::new(11.0, 10.0, 45.0)]);
            let names = other.name_key_namespace.lock().unwrap();
            assert!(names.key_for_name("waypointID").is_none());
            assert!(names.key_for_name("mapName").is_none());
        });
        assert!(!map_has_terrain(&bytes));
    }

    #[test]
    fn preview_checks_version_four_header_and_campaign_checks_terrain_samples() {
        let broken = preview_fixture(&[5, 4, 1, 1, 3], 4, &[]);
        assert!(MapPreview::read(&broken, |_| None).is_err());

        let header = [5, 4, 1, 1, 3, 2];
        let header_only = preview_fixture(&header, 4, &[]);
        let full = preview_fixture(&header, 4, &[7; 20]);
        let preview = MapPreview::read(&header_only, |_| None).unwrap();
        assert_eq!(preview.extent.hi, Coord3D::new(30.0, 20.0, 0.0));
        assert!(!map_has_terrain(&header_only));
        assert!(map_has_terrain(&full));
    }
}
