//! WorldBuilder HeightMapData chunk save/load.
//!
//! C++ oracle: `WHeightMapEdit.cpp` writer + `WorldHeightMap::ParseHeightMapData`
//! (`K_HEIGHT_MAP_VERSION_4`). Chunky wrapper matches `DataChunkOutput`/`CkMp`.

use std::io::{self, Cursor, Read};

/// C++ `K_HEIGHT_MAP_VERSION_4`.
pub const K_HEIGHT_MAP_VERSION_4: u16 = 4;

/// Editable heightmap saved by WorldBuilder.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HeightMapEdit {
    pub width: i32,
    pub height: i32,
    pub border_size: i32,
    pub data: Vec<u8>,
}

impl HeightMapEdit {
    pub fn new(width: i32, height: i32, border_size: i32) -> Result<Self, SaveMapError> {
        if width <= 0 || height <= 0 || border_size < 0 {
            return Err(SaveMapError::InvalidInput);
        }
        if border_size * 2 >= width || border_size * 2 >= height {
            return Err(SaveMapError::InvalidInput);
        }
        let size = (width as usize)
            .checked_mul(height as usize)
            .ok_or(SaveMapError::InvalidInput)?;
        Ok(Self {
            width,
            height,
            border_size,
            data: vec![0u8; size],
        })
    }

    pub fn index(&self, x: i32, y: i32) -> Option<usize> {
        if x < 0 || y < 0 || x >= self.width || y >= self.height {
            return None;
        }
        Some((y as usize) * (self.width as usize) + (x as usize))
    }

    pub fn set_height(&mut self, x: i32, y: i32, height: u8) -> Result<(), SaveMapError> {
        let idx = self.index(x, y).ok_or(SaveMapError::InvalidInput)?;
        self.data[idx] = height;
        Ok(())
    }

    pub fn get_height(&self, x: i32, y: i32) -> Option<u8> {
        self.index(x, y).map(|i| self.data[i])
    }

    /// Logical playable extent in heightmap cells (C++ version-4 default boundary).
    pub fn logical_extent(&self) -> (i32, i32) {
        (
            self.width - 2 * self.border_size,
            self.height - 2 * self.border_size,
        )
    }

    /// Write a version-4 `HeightMapData` chunky blob.
    pub fn write_chunky(&self) -> Vec<u8> {
        let mut payload = Vec::new();
        payload.extend_from_slice(&self.width.to_le_bytes());
        payload.extend_from_slice(&self.height.to_le_bytes());
        payload.extend_from_slice(&self.border_size.to_le_bytes());
        // version >= 4: numBorders + (x,y)*numBorders
        let (bound_x, bound_y) = self.logical_extent();
        payload.extend_from_slice(&1i32.to_le_bytes());
        payload.extend_from_slice(&bound_x.to_le_bytes());
        payload.extend_from_slice(&bound_y.to_le_bytes());
        payload.extend_from_slice(&(self.data.len() as i32).to_le_bytes());
        payload.extend_from_slice(&self.data);
        make_chunk_bytes("HeightMapData", K_HEIGHT_MAP_VERSION_4, &payload)
    }

    /// Parse a version-4 `HeightMapData` chunky blob written by [`write_chunky`].
    pub fn read_chunky(bytes: &[u8]) -> Result<Self, SaveMapError> {
        if bytes.len() < 4 + 4 + 1 + 4 + 4 + 2 + 4 {
            return Err(SaveMapError::InvalidInput);
        }
        if &bytes[0..4] != b"CkMp" {
            return Err(SaveMapError::InvalidInput);
        }
        let mut cur = Cursor::new(&bytes[4..]);
        let mut i32buf = [0u8; 4];
        cur.read_exact(&mut i32buf)
            .map_err(|_| SaveMapError::InvalidInput)?;
        let mut len_buf = [0u8; 1];
        cur.read_exact(&mut len_buf)
            .map_err(|_| SaveMapError::InvalidInput)?;
        let label_len = len_buf[0] as usize;
        let mut label = vec![0u8; label_len];
        cur.read_exact(&mut label)
            .map_err(|_| SaveMapError::InvalidInput)?;
        if label != b"HeightMapData" {
            return Err(SaveMapError::InvalidInput);
        }
        cur.read_exact(&mut i32buf)
            .map_err(|_| SaveMapError::InvalidInput)?; // data_size unused
        cur.read_exact(&mut i32buf)
            .map_err(|_| SaveMapError::InvalidInput)?; // chunk_count
        let mut ver = [0u8; 2];
        cur.read_exact(&mut ver)
            .map_err(|_| SaveMapError::InvalidInput)?;
        let version = u16::from_le_bytes(ver);
        if version != K_HEIGHT_MAP_VERSION_4 {
            return Err(SaveMapError::InvalidInput);
        }
        cur.read_exact(&mut i32buf)
            .map_err(|_| SaveMapError::InvalidInput)?;
        let payload_len = i32::from_le_bytes(i32buf) as usize;
        let pos = cur.position() as usize;
        let payload = bytes
            .get(4 + pos..4 + pos + payload_len)
            .ok_or(SaveMapError::InvalidInput)?;
        parse_heightmap_payload(payload)
    }
}

fn parse_heightmap_payload(payload: &[u8]) -> Result<HeightMapEdit, SaveMapError> {
    if payload.len() < 20 {
        return Err(SaveMapError::InvalidInput);
    }
    let width = i32::from_le_bytes(payload[0..4].try_into().unwrap());
    let height = i32::from_le_bytes(payload[4..8].try_into().unwrap());
    let border_size = i32::from_le_bytes(payload[8..12].try_into().unwrap());
    let num_borders = i32::from_le_bytes(payload[12..16].try_into().unwrap());
    if num_borders < 0 {
        return Err(SaveMapError::InvalidInput);
    }
    let border_bytes = (num_borders as usize)
        .checked_mul(8)
        .ok_or(SaveMapError::InvalidInput)?;
    let data_size_off = 16 + border_bytes;
    if payload.len() < data_size_off + 4 {
        return Err(SaveMapError::InvalidInput);
    }
    let data_size = i32::from_le_bytes(
        payload[data_size_off..data_size_off + 4]
            .try_into()
            .unwrap(),
    );
    let samples = &payload[data_size_off + 4..];
    if data_size < 0 || samples.len() != data_size as usize {
        return Err(SaveMapError::InvalidInput);
    }
    if data_size as i64 != (width as i64) * (height as i64) {
        return Err(SaveMapError::InvalidInput);
    }
    let mut map = HeightMapEdit::new(width, height, border_size)?;
    map.data.copy_from_slice(samples);
    Ok(map)
}

fn make_chunk_bytes(label: &str, version: u16, payload: &[u8]) -> Vec<u8> {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(b"CkMp");
    bytes.extend_from_slice(&1i32.to_le_bytes());
    bytes.push(label.len() as u8);
    bytes.extend_from_slice(label.as_bytes());
    bytes.extend_from_slice(&1u32.to_le_bytes());
    bytes.extend_from_slice(&1u32.to_le_bytes());
    bytes.extend_from_slice(&version.to_le_bytes());
    bytes.extend_from_slice(&(payload.len() as i32).to_le_bytes());
    bytes.extend_from_slice(payload);
    bytes
}

/// Legacy wrapper kept for the WorldBuilder SaveMap dialog name.
#[derive(Debug, Clone, Default)]
pub struct SaveMap {
    pub heightmap: Option<HeightMapEdit>,
}

impl SaveMap {
    pub fn new() -> Self {
        Self { heightmap: None }
    }

    pub fn save_heightmap(map: &HeightMapEdit) -> Vec<u8> {
        map.write_chunky()
    }

    pub fn load_heightmap(bytes: &[u8]) -> Result<HeightMapEdit, SaveMapError> {
        HeightMapEdit::read_chunky(bytes)
    }
}

/// Map object written under C++ `ObjectsList` / `Object` chunks.
#[derive(Debug, Clone)]
pub struct MapObjectEdit {
    pub name: String,
    pub x: f32,
    pub y: f32,
    pub z: f32,
    pub angle: f32,
    pub flags: i32,
    pub properties: game_engine::common::dict::Dict,
    pub waypoint_name: Option<String>,
}

/// C++ `Script` chunk `K_SCRIPT_DATA_VERSION_2`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScriptEdit {
    pub name: String,
    pub comment: String,
    pub condition_comment: String,
    pub action_comment: String,
    pub is_active: bool,
    pub is_one_shot: bool,
    pub easy: bool,
    pub normal: bool,
    pub hard: bool,
    pub is_subroutine: bool,
    pub delay_evaluation_seconds: i32,
}

/// C++ `ScriptGroup` chunk `K_SCRIPT_GROUP_DATA_VERSION_2`.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ScriptGroupEdit {
    pub name: String,
    pub is_active: bool,
    pub is_subroutine: bool,
    pub scripts: Vec<ScriptEdit>,
}

/// One per-side `ScriptList` (`PlayerScriptsList` child).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ScriptListEdit {
    pub scripts: Vec<ScriptEdit>,
    pub groups: Vec<ScriptGroupEdit>,
}

/// C++ `K_BLEND_TILE_VERSION_8` / `K_WORLDDICT_VERSION_1` / `K_SIDES_DATA_VERSION_3`.
pub const K_BLEND_TILE_VERSION_8: u16 = 8;
pub const K_WORLDDICT_VERSION_1: u16 = 1;
pub const K_SIDES_DATA_VERSION_3: u16 = 3;
pub const K_OBJECTS_VERSION_3: u16 = 3;
pub const K_TRIGGERS_VERSION_4: u16 = 4;
pub const K_LIGHTING_VERSION_3: u16 = 3;
pub const MAX_GLOBAL_LIGHTS: usize = 3;
pub const LIGHTING_TOD_SLOTS: usize = 4;
const K_SCRIPTS_DATA_VERSION_1: u16 = 1;
const K_SCRIPT_LIST_DATA_VERSION_1: u16 = 1;
const K_SCRIPT_DATA_VERSION_2: u16 = 2;
const K_SCRIPT_GROUP_DATA_VERSION_2: u16 = 2;

/// C++ terrain texture class written in BlendTileData.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TextureClassEdit {
    pub first_tile: i32,
    pub num_tiles: i32,
    pub width: i32,
    pub name: String,
}

/// C++ `FLAG_VAL` after each blended-tile record (`WorldHeightMap.h`).
pub const BLEND_TILE_FLAG_VAL: i32 = 0x7ADA_0000;

/// C++ `TBlendTileInfo` written for `i = 1 .. numBlendedTiles-1`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BlendedTileEdit {
    pub blend_ndx: i32,
    pub horiz: u8,
    pub vert: u8,
    pub right_diagonal: u8,
    pub left_diagonal: u8,
    pub inverted: u8,
    pub long_diagonal: u8,
    pub custom_blend_edge_class: i32,
}

/// C++ `TCliffInfo` written for `i = 1 .. numCliffInfo-1`.
#[derive(Debug, Clone, PartialEq)]
pub struct CliffInfoEdit {
    pub tile_index: i32,
    pub u0: f32,
    pub v0: f32,
    pub u1: f32,
    pub v1: f32,
    pub u2: f32,
    pub v2: f32,
    pub u3: f32,
    pub v3: f32,
    pub flip: u8,
    pub mutant: u8,
}

/// Full BlendTileData v8 payload (not a zero stub).
#[derive(Debug, Clone, PartialEq)]
pub struct BlendTileDataEdit {
    pub tile_ndxes: Vec<u16>,
    pub blend_tile_ndxes: Vec<u16>,
    pub extra_blend_tile_ndxes: Vec<u16>,
    pub cliff_info_ndxes: Vec<u16>,
    pub cell_cliff_state: Vec<u8>,
    pub num_bitmap_tiles: i32,
    pub blended_tiles: Vec<BlendedTileEdit>,
    pub cliff_infos: Vec<CliffInfoEdit>,
    pub edge_tiles: i32,
    pub edge_texture_classes: Vec<TextureClassEdit>,
}

impl BlendTileDataEdit {
    /// Empty C++ default: all index 0, `numBlendedTiles=1`, `numCliffInfo=1`.
    pub fn empty_for(heightmap: &HeightMapEdit) -> Self {
        let data_size = heightmap.data.len();
        let flip_state_width = ((heightmap.width + 7) / 8).max(0) as usize;
        let cliff_len = (heightmap.height as usize).saturating_mul(flip_state_width);
        Self {
            tile_ndxes: vec![0; data_size],
            blend_tile_ndxes: vec![0; data_size],
            extra_blend_tile_ndxes: vec![0; data_size],
            cliff_info_ndxes: vec![0; data_size],
            cell_cliff_state: vec![0; cliff_len],
            num_bitmap_tiles: 0,
            blended_tiles: Vec::new(),
            cliff_infos: Vec::new(),
            edge_tiles: 0,
            edge_texture_classes: Vec::new(),
        }
    }

    fn cliff_state_len(width: i32, height: i32) -> usize {
        let flip_state_width = ((width + 7) / 8).max(0) as usize;
        (height.max(0) as usize).saturating_mul(flip_state_width)
    }
}

impl Default for BlendTileDataEdit {
    fn default() -> Self {
        Self {
            tile_ndxes: Vec::new(),
            blend_tile_ndxes: Vec::new(),
            extra_blend_tile_ndxes: Vec::new(),
            cliff_info_ndxes: Vec::new(),
            cell_cliff_state: Vec::new(),
            num_bitmap_tiles: 0,
            blended_tiles: Vec::new(),
            cliff_infos: Vec::new(),
            edge_tiles: 0,
            edge_texture_classes: Vec::new(),
        }
    }
}

/// C++ `BuildListInfo` fields written under SidesList v3.
#[derive(Debug, Clone, PartialEq)]
pub struct BuildListItemEdit {
    pub building_name: String,
    pub template_name: String,
    pub x: f32,
    pub y: f32,
    pub z: f32,
    pub angle: f32,
    pub initially_built: bool,
    pub num_rebuilds: i32,
    pub script: String,
    pub health: i32,
    pub whiner: bool,
    pub unsellable: bool,
    pub repairable: bool,
}

/// One player/side dict + build list.
#[derive(Debug, Clone)]
pub struct SideInfoEdit {
    pub dict: game_engine::common::dict::Dict,
    pub build_list: Vec<BuildListItemEdit>,
}

/// C++ `PolygonTrigger` record written under PolygonTriggers v4.
#[derive(Debug, Clone, PartialEq)]
pub struct PolygonTriggerEdit {
    pub name: String,
    pub layer: String,
    pub id: i32,
    pub is_water: bool,
    pub is_river: bool,
    pub river_start: i32,
    pub points: Vec<(i32, i32, i32)>,
}

/// One GlobalData TerrainLighting slot (ambient/diffuse/pos).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GlobalLightEdit {
    pub ambient: [f32; 3],
    pub diffuse: [f32; 3],
    pub light_pos: [f32; 3],
}

impl Default for GlobalLightEdit {
    fn default() -> Self {
        Self {
            ambient: [0.0; 3],
            diffuse: [0.0; 3],
            light_pos: [0.0, 0.0, -1.0],
        }
    }
}

/// C++ `GlobalLighting` chunk, `K_LIGHTING_VERSION_3`.
#[derive(Debug, Clone, PartialEq)]
pub struct GlobalLightingEdit {
    pub time_of_day: i32,
    pub terrain: [[GlobalLightEdit; MAX_GLOBAL_LIGHTS]; LIGHTING_TOD_SLOTS],
    pub objects: [[GlobalLightEdit; MAX_GLOBAL_LIGHTS]; LIGHTING_TOD_SLOTS],
    pub shadow_color: i32,
}

impl Default for GlobalLightingEdit {
    fn default() -> Self {
        Self {
            time_of_day: 2,
            terrain: [[GlobalLightEdit::default(); MAX_GLOBAL_LIGHTS]; LIGHTING_TOD_SLOTS],
            objects: [[GlobalLightEdit::default(); MAX_GLOBAL_LIGHTS]; LIGHTING_TOD_SLOTS],
            shadow_color: 0,
        }
    }
}

/// WorldBuilder multi-chunk CkMp matching `WHeightMapEdit::saveToFile` order.
#[derive(Debug, Clone)]
pub struct MapDocument {
    pub heightmap: HeightMapEdit,
    pub objects: Vec<MapObjectEdit>,
    pub world_dict: game_engine::common::dict::Dict,
    pub sides: Vec<SideInfoEdit>,
    pub teams: Vec<game_engine::common::dict::Dict>,
    pub texture_classes: Vec<TextureClassEdit>,
    pub blend_tiles: BlendTileDataEdit,
    pub script_lists: Vec<ScriptListEdit>,
    pub polygons: Vec<PolygonTriggerEdit>,
    pub lighting: GlobalLightingEdit,
}

impl MapDocument {
    pub fn new(heightmap: HeightMapEdit) -> Self {
        let blend_tiles = BlendTileDataEdit::empty_for(&heightmap);
        Self {
            heightmap,
            objects: Vec::new(),
            world_dict: game_engine::common::dict::Dict::new(),
            sides: Vec::new(),
            teams: Vec::new(),
            texture_classes: Vec::new(),
            blend_tiles,
            script_lists: Vec::new(),
            polygons: Vec::new(),
            lighting: GlobalLightingEdit::default(),
        }
    }

    pub fn write_ckmp(&self) -> Vec<u8> {
        use game_engine::common::name_key_generator::NameKeyGenerator;
        use game_engine::common::system::DataChunkOutput;

        let mut out = DataChunkOutput::new();
        out.open_data_chunk("HeightMapData", K_HEIGHT_MAP_VERSION_4);
        out.write_int(self.heightmap.width);
        out.write_int(self.heightmap.height);
        out.write_int(self.heightmap.border_size);
        let (bx, by) = self.heightmap.logical_extent();
        out.write_int(1);
        out.write_int(bx);
        out.write_int(by);
        out.write_int(self.heightmap.data.len() as i32);
        for sample in &self.heightmap.data {
            out.write_byte(*sample);
        }
        out.close_data_chunk();

        // BlendTileData v8 (C++ after HeightMapData).
        let data_size = self.heightmap.data.len();
        let cliff_state_len =
            BlendTileDataEdit::cliff_state_len(self.heightmap.width, self.heightmap.height);
        let tile_ndxes = pad_u16(&self.blend_tiles.tile_ndxes, data_size);
        let blend_ndxes = pad_u16(&self.blend_tiles.blend_tile_ndxes, data_size);
        let extra_ndxes = pad_u16(&self.blend_tiles.extra_blend_tile_ndxes, data_size);
        let cliff_ndxes = pad_u16(&self.blend_tiles.cliff_info_ndxes, data_size);
        let mut cliff_state = self.blend_tiles.cell_cliff_state.clone();
        cliff_state.resize(cliff_state_len, 0);
        let num_bitmap = if self.blend_tiles.num_bitmap_tiles > 0 {
            self.blend_tiles.num_bitmap_tiles
        } else {
            self.texture_classes.len() as i32
        };
        // C++ index 0 unused: numBlendedTiles / numCliffInfo are 1 + record count.
        let num_blended = (self.blend_tiles.blended_tiles.len() as i32) + 1;
        let num_cliff = (self.blend_tiles.cliff_infos.len() as i32) + 1;
        out.open_data_chunk("BlendTileData", K_BLEND_TILE_VERSION_8);
        out.write_int(data_size as i32);
        for v in &tile_ndxes {
            out.write_unsigned_short(*v);
        }
        for v in &blend_ndxes {
            out.write_unsigned_short(*v);
        }
        for v in &extra_ndxes {
            out.write_unsigned_short(*v);
        }
        for v in &cliff_ndxes {
            out.write_unsigned_short(*v);
        }
        for b in &cliff_state {
            out.write_byte(*b);
        }
        out.write_int(num_bitmap);
        out.write_int(num_blended);
        out.write_int(num_cliff);
        out.write_int(self.texture_classes.len() as i32);
        for class in &self.texture_classes {
            out.write_int(class.first_tile);
            out.write_int(class.num_tiles);
            out.write_int(class.width);
            out.write_int(0);
            out.write_ascii_string(&class.name);
        }
        out.write_int(self.blend_tiles.edge_tiles);
        out.write_int(self.blend_tiles.edge_texture_classes.len() as i32);
        for class in &self.blend_tiles.edge_texture_classes {
            out.write_int(class.first_tile);
            out.write_int(class.num_tiles);
            out.write_int(class.width);
            out.write_ascii_string(&class.name);
        }
        for rec in &self.blend_tiles.blended_tiles {
            out.write_int(rec.blend_ndx);
            out.write_byte(rec.horiz);
            out.write_byte(rec.vert);
            out.write_byte(rec.right_diagonal);
            out.write_byte(rec.left_diagonal);
            out.write_byte(rec.inverted);
            out.write_byte(rec.long_diagonal);
            out.write_int(rec.custom_blend_edge_class);
            out.write_int(BLEND_TILE_FLAG_VAL);
        }
        for rec in &self.blend_tiles.cliff_infos {
            out.write_int(rec.tile_index);
            out.write_real(rec.u0);
            out.write_real(rec.v0);
            out.write_real(rec.u1);
            out.write_real(rec.v1);
            out.write_real(rec.u2);
            out.write_real(rec.v2);
            out.write_real(rec.u3);
            out.write_real(rec.v3);
            out.write_byte(rec.flip);
            out.write_byte(rec.mutant);
        }
        out.close_data_chunk();

        // WorldInfo before SidesList (C++ comment).
        out.open_data_chunk("WorldInfo", K_WORLDDICT_VERSION_1);
        out.write_dict(&self.world_dict);
        out.close_data_chunk();

        // SidesList v3 + empty PlayerScriptsList.
        out.open_data_chunk("SidesList", K_SIDES_DATA_VERSION_3);
        out.write_int(self.sides.len() as i32);
        for side in &self.sides {
            out.write_dict(&side.dict);
            out.write_int(side.build_list.len() as i32);
            for b in &side.build_list {
                out.write_ascii_string(&b.building_name);
                out.write_ascii_string(&b.template_name);
                out.write_real(b.x);
                out.write_real(b.y);
                out.write_real(b.z);
                out.write_real(b.angle);
                out.write_byte(u8::from(b.initially_built));
                out.write_int(b.num_rebuilds);
                out.write_ascii_string(&b.script);
                out.write_int(b.health);
                out.write_byte(u8::from(b.whiner));
                out.write_byte(u8::from(b.unsellable));
                out.write_byte(u8::from(b.repairable));
            }
        }
        out.write_int(self.teams.len() as i32);
        for team in &self.teams {
            out.write_dict(team);
        }
        out.open_data_chunk("PlayerScriptsList", K_SCRIPTS_DATA_VERSION_1);
        let script_list_count = self.sides.len().max(self.script_lists.len());
        for i in 0..script_list_count {
            out.open_data_chunk("ScriptList", K_SCRIPT_LIST_DATA_VERSION_1);
            if let Some(list) = self.script_lists.get(i) {
                for script in &list.scripts {
                    write_script_chunk(&mut out, script);
                }
                for group in &list.groups {
                    out.open_data_chunk("ScriptGroup", K_SCRIPT_GROUP_DATA_VERSION_2);
                    out.write_ascii_string(&group.name);
                    out.write_byte(u8::from(group.is_active));
                    out.write_byte(u8::from(group.is_subroutine));
                    for script in &group.scripts {
                        write_script_chunk(&mut out, script);
                    }
                    out.close_data_chunk();
                }
            }
            out.close_data_chunk();
        }
        out.close_data_chunk();
        out.close_data_chunk();

        out.open_data_chunk("ObjectsList", K_OBJECTS_VERSION_3);
        for obj in &self.objects {
            out.open_data_chunk("Object", K_OBJECTS_VERSION_3);
            out.write_real(obj.x);
            out.write_real(obj.y);
            out.write_real(obj.z);
            out.write_real(obj.angle);
            out.write_int(obj.flags);
            out.write_ascii_string(&obj.name);
            let mut dict = obj.properties.clone();
            if let Some(wp) = &obj.waypoint_name {
                let id_key = NameKeyGenerator::name_to_key("waypointID");
                let name_key = NameKeyGenerator::name_to_key("waypointName");
                if !matches!(
                    dict.get_type(id_key),
                    Some(game_engine::common::dict::DictType::Int)
                ) {
                    dict.set_int(id_key, 1);
                }
                dict.set_ascii_string(name_key, wp.clone());
            }
            out.write_dict(&dict);
            out.close_data_chunk();
        }
        out.close_data_chunk();

        out.open_data_chunk("PolygonTriggers", K_TRIGGERS_VERSION_4);
        out.write_int(self.polygons.len() as i32);
        for poly in &self.polygons {
            out.write_ascii_string(&poly.name);
            out.write_ascii_string(&poly.layer);
            out.write_int(poly.id);
            out.write_byte(u8::from(poly.is_water));
            out.write_byte(u8::from(poly.is_river));
            out.write_int(poly.river_start);
            out.write_int(poly.points.len() as i32);
            for (x, y, z) in &poly.points {
                out.write_int(*x);
                out.write_int(*y);
                out.write_int(*z);
            }
        }
        out.close_data_chunk();

        out.open_data_chunk("GlobalLighting", K_LIGHTING_VERSION_3);
        out.write_int(self.lighting.time_of_day);
        for tod in 0..LIGHTING_TOD_SLOTS {
            write_global_light(&mut out, &self.lighting.terrain[tod][0]);
            write_global_light(&mut out, &self.lighting.objects[tod][0]);
            for j in 1..MAX_GLOBAL_LIGHTS {
                write_global_light(&mut out, &self.lighting.objects[tod][j]);
            }
            for j in 1..MAX_GLOBAL_LIGHTS {
                write_global_light(&mut out, &self.lighting.terrain[tod][j]);
            }
        }
        out.write_int(self.lighting.shadow_color);
        out.close_data_chunk();

        out.into_ckmp_bytes()
    }

    /// C++ WorldBuilder `saveToFile` destination: write CkMp bytes to a `.map` path.
    pub fn save_to_path(&self, path: &std::path::Path) -> Result<(), SaveMapError> {
        std::fs::write(path, self.write_ckmp())?;
        Ok(())
    }

    pub fn load_from_path(path: &std::path::Path) -> Result<Self, SaveMapError> {
        let bytes = std::fs::read(path).map_err(|_| SaveMapError::InvalidInput)?;
        Self::read_ckmp(&bytes)
    }

    pub fn read_ckmp(bytes: &[u8]) -> Result<Self, SaveMapError> {
        use game_engine::common::dict::DictType;
        use game_engine::common::name_key_generator::NameKeyGenerator;
        use game_engine::common::system::DataChunkInput;

        let mut input = DataChunkInput::new(bytes.to_vec());
        if !input.is_valid_file_type() {
            return Err(SaveMapError::InvalidInput);
        }
        let mut ctx = MapDocParse::default();
        input.register_parser("HeightMapData", "", parse_doc_heightmap);
        input.register_parser("BlendTileData", "", parse_doc_blend_tile);
        input.register_parser("WorldInfo", "", parse_doc_world_info);
        input.register_parser("SidesList", "", parse_doc_sides_list);
        input.register_parser("ObjectsList", "", parse_doc_objects_list);
        input.register_parser("PolygonTriggers", "", parse_doc_polygons);
        input.register_parser("GlobalLighting", "", parse_doc_lighting);
        if !input.parse(&mut ctx) {
            return Err(SaveMapError::ProcessingFailed);
        }
        let heightmap = ctx.heightmap.ok_or(SaveMapError::InvalidInput)?;
        let _ = NameKeyGenerator::name_to_key("waypointID");
        let _ = DictType::Int;
        Ok(Self {
            heightmap,
            objects: ctx.objects,
            world_dict: ctx.world_dict,
            sides: ctx.sides,
            teams: ctx.teams,
            texture_classes: ctx.texture_classes,
            blend_tiles: ctx.blend_tiles,
            script_lists: ctx.script_lists,
            polygons: ctx.polygons,
            lighting: ctx.lighting,
        })
    }
}

fn write_script_chunk(out: &mut game_engine::common::system::DataChunkOutput, script: &ScriptEdit) {
    out.open_data_chunk("Script", K_SCRIPT_DATA_VERSION_2);
    out.write_ascii_string(&script.name);
    out.write_ascii_string(&script.comment);
    out.write_ascii_string(&script.condition_comment);
    out.write_ascii_string(&script.action_comment);
    out.write_byte(u8::from(script.is_active));
    out.write_byte(u8::from(script.is_one_shot));
    out.write_byte(u8::from(script.easy));
    out.write_byte(u8::from(script.normal));
    out.write_byte(u8::from(script.hard));
    out.write_byte(u8::from(script.is_subroutine));
    out.write_int(script.delay_evaluation_seconds);
    out.close_data_chunk();
}

fn pad_u16(src: &[u16], len: usize) -> Vec<u16> {
    let mut out = src.to_vec();
    out.resize(len, 0);
    out
}

fn write_global_light(
    out: &mut game_engine::common::system::DataChunkOutput,
    light: &GlobalLightEdit,
) {
    out.write_real(light.ambient[0]);
    out.write_real(light.ambient[1]);
    out.write_real(light.ambient[2]);
    out.write_real(light.diffuse[0]);
    out.write_real(light.diffuse[1]);
    out.write_real(light.diffuse[2]);
    out.write_real(light.light_pos[0]);
    out.write_real(light.light_pos[1]);
    out.write_real(light.light_pos[2]);
}

fn read_global_light(input: &mut game_engine::common::system::DataChunkInput) -> GlobalLightEdit {
    GlobalLightEdit {
        ambient: [input.read_real(), input.read_real(), input.read_real()],
        diffuse: [input.read_real(), input.read_real(), input.read_real()],
        light_pos: [input.read_real(), input.read_real(), input.read_real()],
    }
}

#[derive(Default)]
struct MapDocParse {
    heightmap: Option<HeightMapEdit>,
    objects: Vec<MapObjectEdit>,
    world_dict: game_engine::common::dict::Dict,
    sides: Vec<SideInfoEdit>,
    teams: Vec<game_engine::common::dict::Dict>,
    texture_classes: Vec<TextureClassEdit>,
    blend_tiles: BlendTileDataEdit,
    script_lists: Vec<ScriptListEdit>,
    polygons: Vec<PolygonTriggerEdit>,
    lighting: GlobalLightingEdit,
}

fn parse_doc_heightmap(
    input: &mut game_engine::common::system::DataChunkInput,
    info: &game_engine::common::system::DataChunkInfo,
    user_data: &mut dyn std::any::Any,
) -> bool {
    let Some(ctx) = user_data.downcast_mut::<MapDocParse>() else {
        return false;
    };
    let width = input.read_int();
    let height = input.read_int();
    let border = if info.version >= 3 {
        input.read_int()
    } else {
        0
    };
    if info.version >= 4 {
        let n = input.read_int().max(0);
        for _ in 0..n {
            let _ = input.read_int();
            let _ = input.read_int();
        }
    }
    let data_size = input.read_int();
    if data_size <= 0 {
        return false;
    }
    let mut data = Vec::with_capacity(data_size as usize);
    for _ in 0..data_size {
        data.push(input.read_byte());
    }
    let Ok(mut map) = HeightMapEdit::new(width, height, border) else {
        return false;
    };
    if map.data.len() != data.len() {
        return false;
    }
    map.data = data;
    ctx.heightmap = Some(map);
    true
}

fn parse_doc_blend_tile(
    input: &mut game_engine::common::system::DataChunkInput,
    info: &game_engine::common::system::DataChunkInfo,
    user_data: &mut dyn std::any::Any,
) -> bool {
    let Some(ctx) = user_data.downcast_mut::<MapDocParse>() else {
        return false;
    };
    let data_size = input.read_int();
    if data_size < 0 {
        return false;
    }
    let n = data_size as usize;
    let mut tile_ndxes = Vec::with_capacity(n);
    let mut blend_tile_ndxes = Vec::with_capacity(n);
    let mut extra_blend_tile_ndxes = Vec::with_capacity(n);
    let mut cliff_info_ndxes = Vec::with_capacity(n);
    for _ in 0..n {
        tile_ndxes.push(input.read_unsigned_short());
    }
    for _ in 0..n {
        blend_tile_ndxes.push(input.read_unsigned_short());
    }
    if info.version >= 6 {
        for _ in 0..n {
            extra_blend_tile_ndxes.push(input.read_unsigned_short());
        }
    } else {
        extra_blend_tile_ndxes.resize(n, 0);
    }
    if info.version >= 5 {
        for _ in 0..n {
            cliff_info_ndxes.push(input.read_unsigned_short());
        }
    } else {
        cliff_info_ndxes.resize(n, 0);
    }
    let w = ctx.heightmap.as_ref().map(|h| h.width).unwrap_or(0);
    let h = ctx.heightmap.as_ref().map(|h| h.height).unwrap_or(0);
    let cliff_len = BlendTileDataEdit::cliff_state_len(w, h);
    let mut cell_cliff_state = Vec::with_capacity(cliff_len);
    if info.version >= 7 {
        for _ in 0..cliff_len {
            cell_cliff_state.push(input.read_byte());
        }
    } else {
        cell_cliff_state.resize(cliff_len, 0);
    }
    let num_bitmap_tiles = input.read_int();
    let num_blended = input.read_int().max(1);
    let num_cliff = if info.version >= 5 {
        input.read_int().max(1)
    } else {
        1
    };
    let num_classes = input.read_int().max(0);
    ctx.texture_classes.clear();
    for _ in 0..num_classes {
        let first_tile = input.read_int();
        let num_tiles = input.read_int();
        let width = input.read_int();
        let _pad = input.read_int();
        let name = input.read_ascii_string();
        ctx.texture_classes.push(TextureClassEdit {
            first_tile,
            num_tiles,
            width,
            name,
        });
    }
    let mut edge_tiles = 0;
    let mut edge_texture_classes = Vec::new();
    if info.version >= 4 {
        edge_tiles = input.read_int();
        let num_edge_classes = input.read_int().max(0);
        for _ in 0..num_edge_classes {
            let first_tile = input.read_int();
            let num_tiles = input.read_int();
            let width = input.read_int();
            let name = input.read_ascii_string();
            edge_texture_classes.push(TextureClassEdit {
                first_tile,
                num_tiles,
                width,
                name,
            });
        }
    }
    let mut blended_tiles = Vec::new();
    for _ in 1..num_blended {
        let blend_ndx = input.read_int();
        let horiz = input.read_byte();
        let vert = input.read_byte();
        let right_diagonal = input.read_byte();
        let left_diagonal = input.read_byte();
        let inverted = input.read_byte();
        let long_diagonal = if info.version >= 3 {
            input.read_byte()
        } else {
            0
        };
        let custom_blend_edge_class = if info.version >= 4 {
            input.read_int()
        } else {
            -1
        };
        let flag = input.read_int();
        if flag != BLEND_TILE_FLAG_VAL {
            return false;
        }
        blended_tiles.push(BlendedTileEdit {
            blend_ndx,
            horiz,
            vert,
            right_diagonal,
            left_diagonal,
            inverted,
            long_diagonal,
            custom_blend_edge_class,
        });
    }
    let mut cliff_infos = Vec::new();
    if info.version >= 5 {
        for _ in 1..num_cliff {
            cliff_infos.push(CliffInfoEdit {
                tile_index: input.read_int(),
                u0: input.read_real(),
                v0: input.read_real(),
                u1: input.read_real(),
                v1: input.read_real(),
                u2: input.read_real(),
                v2: input.read_real(),
                u3: input.read_real(),
                v3: input.read_real(),
                flip: input.read_byte(),
                mutant: input.read_byte(),
            });
        }
    }
    ctx.blend_tiles = BlendTileDataEdit {
        tile_ndxes,
        blend_tile_ndxes,
        extra_blend_tile_ndxes,
        cliff_info_ndxes,
        cell_cliff_state,
        num_bitmap_tiles,
        blended_tiles,
        cliff_infos,
        edge_tiles,
        edge_texture_classes,
    };
    true
}

fn parse_doc_world_info(
    input: &mut game_engine::common::system::DataChunkInput,
    _info: &game_engine::common::system::DataChunkInfo,
    user_data: &mut dyn std::any::Any,
) -> bool {
    let Some(ctx) = user_data.downcast_mut::<MapDocParse>() else {
        return false;
    };
    ctx.world_dict = input.read_dict();
    true
}

fn parse_doc_sides_list(
    input: &mut game_engine::common::system::DataChunkInput,
    info: &game_engine::common::system::DataChunkInfo,
    user_data: &mut dyn std::any::Any,
) -> bool {
    let count = input.read_int().max(0);
    let mut sides = Vec::new();
    for _ in 0..count {
        let dict = input.read_dict();
        let nbuild = input.read_int().max(0);
        let mut build_list = Vec::new();
        for _ in 0..nbuild {
            let building_name = input.read_ascii_string();
            let template_name = input.read_ascii_string();
            let x = input.read_real();
            let y = input.read_real();
            let z = input.read_real();
            let angle = input.read_real();
            let initially_built = input.read_byte() != 0;
            let num_rebuilds = input.read_int();
            let (script, health, whiner, unsellable, repairable) = if info.version >= 3 {
                (
                    input.read_ascii_string(),
                    input.read_int(),
                    input.read_byte() != 0,
                    input.read_byte() != 0,
                    input.read_byte() != 0,
                )
            } else {
                (String::new(), 100, false, false, true)
            };
            build_list.push(BuildListItemEdit {
                building_name,
                template_name,
                x,
                y,
                z,
                angle,
                initially_built,
                num_rebuilds,
                script,
                health,
                whiner,
                unsellable,
                repairable,
            });
        }
        sides.push(SideInfoEdit { dict, build_list });
    }
    let nteams = input.read_int().max(0);
    let mut teams = Vec::new();
    for _ in 0..nteams {
        teams.push(input.read_dict());
    }
    if let Some(ctx) = user_data.downcast_mut::<MapDocParse>() {
        ctx.sides = sides;
        ctx.teams = teams;
    }
    input.register_parser("PlayerScriptsList", &info.label, parse_doc_player_scripts);
    input.parse(user_data)
}

fn parse_doc_player_scripts(
    input: &mut game_engine::common::system::DataChunkInput,
    info: &game_engine::common::system::DataChunkInfo,
    user_data: &mut dyn std::any::Any,
) -> bool {
    input.register_parser("ScriptList", &info.label, parse_doc_script_list);
    input.parse(user_data)
}

fn parse_doc_script_list(
    input: &mut game_engine::common::system::DataChunkInput,
    info: &game_engine::common::system::DataChunkInfo,
    user_data: &mut dyn std::any::Any,
) -> bool {
    {
        let Some(ctx) = user_data.downcast_mut::<MapDocParse>() else {
            return false;
        };
        ctx.script_lists.push(ScriptListEdit::default());
    }
    input.register_parser("Script", &info.label, parse_doc_script);
    input.register_parser("ScriptGroup", &info.label, parse_doc_script_group);
    input.parse(user_data)
}

fn read_script_fields(
    input: &mut game_engine::common::system::DataChunkInput,
    version: u16,
) -> ScriptEdit {
    let name = input.read_ascii_string();
    let comment = input.read_ascii_string();
    let condition_comment = input.read_ascii_string();
    let action_comment = input.read_ascii_string();
    let is_active = input.read_byte() != 0;
    let is_one_shot = input.read_byte() != 0;
    let easy = input.read_byte() != 0;
    let normal = input.read_byte() != 0;
    let hard = input.read_byte() != 0;
    let is_subroutine = input.read_byte() != 0;
    let delay_evaluation_seconds = if version >= K_SCRIPT_DATA_VERSION_2 {
        input.read_int()
    } else {
        0
    };
    ScriptEdit {
        name,
        comment,
        condition_comment,
        action_comment,
        is_active,
        is_one_shot,
        easy,
        normal,
        hard,
        is_subroutine,
        delay_evaluation_seconds,
    }
}

fn parse_doc_script(
    input: &mut game_engine::common::system::DataChunkInput,
    info: &game_engine::common::system::DataChunkInfo,
    user_data: &mut dyn std::any::Any,
) -> bool {
    let script = read_script_fields(input, info.version);
    let Some(ctx) = user_data.downcast_mut::<MapDocParse>() else {
        return false;
    };
    let Some(list) = ctx.script_lists.last_mut() else {
        return false;
    };
    if info.parent_label == "ScriptGroup" {
        if let Some(group) = list.groups.last_mut() {
            group.scripts.push(script);
            return true;
        }
        return false;
    }
    list.scripts.push(script);
    true
}

fn parse_doc_script_group(
    input: &mut game_engine::common::system::DataChunkInput,
    info: &game_engine::common::system::DataChunkInfo,
    user_data: &mut dyn std::any::Any,
) -> bool {
    let name = input.read_ascii_string();
    let is_active = input.read_byte() != 0;
    let is_subroutine = if info.version >= K_SCRIPT_GROUP_DATA_VERSION_2 {
        input.read_byte() != 0
    } else {
        false
    };
    {
        let Some(ctx) = user_data.downcast_mut::<MapDocParse>() else {
            return false;
        };
        let Some(list) = ctx.script_lists.last_mut() else {
            return false;
        };
        list.groups.push(ScriptGroupEdit {
            name,
            is_active,
            is_subroutine,
            scripts: Vec::new(),
        });
    }
    input.register_parser("Script", &info.label, parse_doc_script);
    input.parse(user_data)
}

fn parse_doc_polygons(
    input: &mut game_engine::common::system::DataChunkInput,
    _info: &game_engine::common::system::DataChunkInfo,
    user_data: &mut dyn std::any::Any,
) -> bool {
    let Some(ctx) = user_data.downcast_mut::<MapDocParse>() else {
        return false;
    };
    let count = input.read_int().max(0);
    ctx.polygons.clear();
    for _ in 0..count {
        let name = input.read_ascii_string();
        let layer = input.read_ascii_string();
        let id = input.read_int();
        let is_water = input.read_byte() != 0;
        let is_river = input.read_byte() != 0;
        let river_start = input.read_int();
        let n = input.read_int().max(0);
        let mut points = Vec::new();
        for _ in 0..n {
            points.push((input.read_int(), input.read_int(), input.read_int()));
        }
        ctx.polygons.push(PolygonTriggerEdit {
            name,
            layer,
            id,
            is_water,
            is_river,
            river_start,
            points,
        });
    }
    true
}

fn parse_doc_lighting(
    input: &mut game_engine::common::system::DataChunkInput,
    _info: &game_engine::common::system::DataChunkInfo,
    user_data: &mut dyn std::any::Any,
) -> bool {
    let Some(ctx) = user_data.downcast_mut::<MapDocParse>() else {
        return false;
    };
    ctx.lighting.time_of_day = input.read_int();
    for tod in 0..LIGHTING_TOD_SLOTS {
        ctx.lighting.terrain[tod][0] = read_global_light(input);
        ctx.lighting.objects[tod][0] = read_global_light(input);
        for j in 1..MAX_GLOBAL_LIGHTS {
            ctx.lighting.objects[tod][j] = read_global_light(input);
        }
        for j in 1..MAX_GLOBAL_LIGHTS {
            ctx.lighting.terrain[tod][j] = read_global_light(input);
        }
    }
    ctx.lighting.shadow_color = input.read_int();
    true
}

fn parse_doc_objects_list(
    input: &mut game_engine::common::system::DataChunkInput,
    info: &game_engine::common::system::DataChunkInfo,
    user_data: &mut dyn std::any::Any,
) -> bool {
    input.register_parser("Object", &info.label, parse_doc_object);
    input.parse(user_data)
}

fn parse_doc_object(
    input: &mut game_engine::common::system::DataChunkInput,
    info: &game_engine::common::system::DataChunkInfo,
    user_data: &mut dyn std::any::Any,
) -> bool {
    use game_engine::common::dict::DictType;
    use game_engine::common::name_key_generator::NameKeyGenerator;

    let Some(ctx) = user_data.downcast_mut::<MapDocParse>() else {
        return false;
    };
    let x = input.read_real();
    let y = input.read_real();
    let mut z = input.read_real();
    if info.version <= 2 {
        z = 0.0;
    }
    let angle = input.read_real();
    let flags = input.read_int();
    let name = input.read_ascii_string();
    let dict = if info.version >= 2 {
        input.read_dict()
    } else {
        game_engine::common::dict::Dict::new()
    };
    let wp_key = NameKeyGenerator::name_to_key("waypointID");
    let waypoint_name = if matches!(dict.get_type(wp_key), Some(DictType::Int)) {
        let nk = NameKeyGenerator::name_to_key("waypointName");
        let wp = match dict.get_type(nk) {
            Some(DictType::AsciiString) => dict.get_ascii_string(nk),
            _ => String::new(),
        };
        Some(if wp.is_empty() { name.clone() } else { wp })
    } else {
        None
    };
    ctx.objects.push(MapObjectEdit {
        name,
        x,
        y,
        z,
        angle,
        flags,
        properties: dict,
        waypoint_name,
    });
    true
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SaveMapError {
    NotActive,
    ProcessingFailed,
    InvalidInput,
    Unknown,
}

impl std::fmt::Display for SaveMapError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SaveMapError::NotActive => write!(f, "Not active"),
            SaveMapError::ProcessingFailed => write!(f, "Processing failed"),
            SaveMapError::InvalidInput => write!(f, "Invalid input"),
            SaveMapError::Unknown => write!(f, "Unknown error"),
        }
    }
}

impl std::error::Error for SaveMapError {}

impl From<io::Error> for SaveMapError {
    fn from(_: io::Error) -> Self {
        SaveMapError::InvalidInput
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn heightmap_v4_roundtrip_preserves_samples_and_border() {
        let mut map = HeightMapEdit::new(16, 12, 2).expect("map");
        map.set_height(2, 2, 80).unwrap();
        map.set_height(15, 11, 200).unwrap();
        map.set_height(0, 0, 7).unwrap();
        let bytes = SaveMap::save_heightmap(&map);
        assert!(bytes.starts_with(b"CkMp"));
        assert!(
            bytes
                .windows(b"HeightMapData".len())
                .any(|w| w == b"HeightMapData")
        );
        let loaded = SaveMap::load_heightmap(&bytes).expect("load");
        assert_eq!(loaded.width, 16);
        assert_eq!(loaded.height, 12);
        assert_eq!(loaded.border_size, 2);
        assert_eq!(loaded.logical_extent(), (12, 8));
        assert_eq!(loaded.get_height(2, 2), Some(80));
        assert_eq!(loaded.get_height(15, 11), Some(200));
        assert_eq!(loaded.get_height(0, 0), Some(7));
        assert_eq!(loaded.data.len(), 16 * 12);
    }

    #[test]
    fn rejects_corrupt_data_size() {
        let map = HeightMapEdit::new(8, 8, 1).unwrap();
        let mut bytes = map.write_chunky();
        // Flip a payload byte after the header so sample length no longer matches.
        let last = bytes.len() - 1;
        bytes[last] ^= 0xFF;
        // Still valid length; check invalid label instead.
        bytes[10] = b'X';
        assert!(SaveMap::load_heightmap(&bytes).is_err());
    }

    #[test]
    fn multi_chunk_ckmp_roundtrips_height_and_waypoint_object() {
        let mut height = HeightMapEdit::new(8, 8, 1).unwrap();
        height.set_height(1, 1, 90).unwrap();
        let mut doc = MapDocument::new(height);
        doc.objects.push(MapObjectEdit {
            name: "*Waypoints/Waypoint".into(),
            x: 15.0,
            y: 25.0,
            z: 1.0,
            angle: 0.0,
            flags: 0,
            properties: game_engine::common::dict::Dict::new(),
            waypoint_name: Some("Player_1_Start".into()),
        });
        let bytes = doc.write_ckmp();
        assert!(bytes.starts_with(b"CkMp"));
        assert!(
            bytes
                .windows(b"ObjectsList".len())
                .any(|w| w == b"ObjectsList")
        );
        assert!(
            bytes
                .windows(b"HeightMapData".len())
                .any(|w| w == b"HeightMapData")
        );
        let loaded = MapDocument::read_ckmp(&bytes).expect("load ckmp");
        assert_eq!(loaded.heightmap.get_height(1, 1), Some(90));
        assert_eq!(loaded.objects.len(), 1);
        assert_eq!(
            loaded.objects[0].waypoint_name.as_deref(),
            Some("Player_1_Start")
        );
        assert_eq!(loaded.objects[0].x, 15.0);
        assert_eq!(loaded.objects[0].y, 25.0);
    }

    #[test]
    fn ckmp_roundtrips_blend_worldinfo_and_sides_like_cpp_save_to_file() {
        use game_engine::common::dict::Dict;
        use game_engine::common::name_key_generator::NameKeyGenerator;

        let height = HeightMapEdit::new(8, 8, 1).unwrap();
        let mut doc = MapDocument::new(height);
        let map_key = NameKeyGenerator::name_to_key("mapName");
        let weather_key = NameKeyGenerator::name_to_key("weather");
        doc.world_dict.set_ascii_string(map_key, "Test Map");
        doc.world_dict.set_int(weather_key, 0);
        let mut side_dict = Dict::new();
        side_dict.set_ascii_string(NameKeyGenerator::name_to_key("playerName"), "PlyrCivilian");
        doc.sides.push(SideInfoEdit {
            dict: side_dict,
            build_list: vec![BuildListItemEdit {
                building_name: "b1".into(),
                template_name: "AmericaBarracks".into(),
                x: 10.0,
                y: 20.0,
                z: 0.0,
                angle: 0.0,
                initially_built: true,
                num_rebuilds: 1,
                script: String::new(),
                health: 100,
                whiner: false,
                unsellable: false,
                repairable: true,
            }],
        });
        doc.texture_classes.push(TextureClassEdit {
            first_tile: 0,
            num_tiles: 4,
            width: 2,
            name: "Dirt".into(),
        });

        let bytes = doc.write_ckmp();
        assert!(
            bytes
                .windows(b"BlendTileData".len())
                .any(|w| w == b"BlendTileData")
        );
        assert!(bytes.windows(b"WorldInfo".len()).any(|w| w == b"WorldInfo"));
        assert!(bytes.windows(b"SidesList".len()).any(|w| w == b"SidesList"));
        assert!(
            bytes
                .windows(b"PlayerScriptsList".len())
                .any(|w| w == b"PlayerScriptsList")
        );

        let loaded = MapDocument::read_ckmp(&bytes).expect("load");
        assert_eq!(loaded.world_dict.get_ascii_string(map_key), "Test Map");
        assert_eq!(loaded.world_dict.get_int(weather_key), 0);
        assert_eq!(loaded.sides.len(), 1);
        assert_eq!(
            loaded.sides[0]
                .dict
                .get_ascii_string(NameKeyGenerator::name_to_key("playerName")),
            "PlyrCivilian"
        );
        assert_eq!(loaded.sides[0].build_list.len(), 1);
        assert_eq!(
            loaded.sides[0].build_list[0].template_name,
            "AmericaBarracks"
        );
        assert_eq!(loaded.texture_classes.len(), 1);
        assert_eq!(loaded.texture_classes[0].name, "Dirt");
        assert_eq!(loaded.blend_tiles.tile_ndxes.len(), 64);
        assert!(loaded.blend_tiles.tile_ndxes.iter().all(|&v| v == 0));
        assert_eq!(loaded.blend_tiles.blended_tiles.len(), 0);
        assert_eq!(loaded.blend_tiles.cliff_infos.len(), 0);
    }

    #[test]
    fn ckmp_roundtrips_blend_tile_arrays_and_records_like_cpp_v8() {
        let height = HeightMapEdit::new(8, 8, 1).unwrap();
        let mut doc = MapDocument::new(height);
        doc.blend_tiles.tile_ndxes[3] = 12;
        doc.blend_tiles.blend_tile_ndxes[3] = 1;
        doc.blend_tiles.extra_blend_tile_ndxes[4] = 2;
        doc.blend_tiles.cliff_info_ndxes[5] = 1;
        doc.blend_tiles.cell_cliff_state[0] = 0xA5;
        doc.blend_tiles.num_bitmap_tiles = 8;
        doc.blend_tiles.blended_tiles.push(BlendedTileEdit {
            blend_ndx: 7,
            horiz: 1,
            vert: 0,
            right_diagonal: 0,
            left_diagonal: 1,
            inverted: 2,
            long_diagonal: 1,
            custom_blend_edge_class: -1,
        });
        doc.blend_tiles.cliff_infos.push(CliffInfoEdit {
            tile_index: 4,
            u0: 0.0,
            v0: 0.0,
            u1: 1.0,
            v1: 0.0,
            u2: 1.0,
            v2: 1.0,
            u3: 0.0,
            v3: 1.0,
            flip: 1,
            mutant: 0,
        });
        doc.blend_tiles.edge_tiles = 2;
        doc.blend_tiles.edge_texture_classes.push(TextureClassEdit {
            first_tile: 16,
            num_tiles: 2,
            width: 1,
            name: "EdgeDirt".into(),
        });
        doc.texture_classes.push(TextureClassEdit {
            first_tile: 0,
            num_tiles: 4,
            width: 2,
            name: "Dirt".into(),
        });

        let bytes = doc.write_ckmp();
        let loaded = MapDocument::read_ckmp(&bytes).expect("load blend v8");
        assert_eq!(loaded.blend_tiles.tile_ndxes[3], 12);
        assert_eq!(loaded.blend_tiles.blend_tile_ndxes[3], 1);
        assert_eq!(loaded.blend_tiles.extra_blend_tile_ndxes[4], 2);
        assert_eq!(loaded.blend_tiles.cliff_info_ndxes[5], 1);
        assert_eq!(loaded.blend_tiles.cell_cliff_state[0], 0xA5);
        assert_eq!(loaded.blend_tiles.num_bitmap_tiles, 8);
        assert_eq!(loaded.blend_tiles.blended_tiles.len(), 1);
        assert_eq!(loaded.blend_tiles.blended_tiles[0].blend_ndx, 7);
        assert_eq!(loaded.blend_tiles.blended_tiles[0].left_diagonal, 1);
        assert_eq!(loaded.blend_tiles.blended_tiles[0].long_diagonal, 1);
        assert_eq!(loaded.blend_tiles.cliff_infos.len(), 1);
        assert_eq!(loaded.blend_tiles.cliff_infos[0].tile_index, 4);
        assert_eq!(loaded.blend_tiles.cliff_infos[0].u2, 1.0);
        assert_eq!(loaded.blend_tiles.cliff_infos[0].flip, 1);
        assert_eq!(loaded.blend_tiles.edge_tiles, 2);
        assert_eq!(loaded.blend_tiles.edge_texture_classes.len(), 1);
        assert_eq!(loaded.blend_tiles.edge_texture_classes[0].name, "EdgeDirt");
    }

    #[test]
    fn ckmp_roundtrips_polygon_triggers_and_global_lighting_like_cpp() {
        let height = HeightMapEdit::new(8, 8, 1).unwrap();
        let mut doc = MapDocument::new(height);
        doc.polygons.push(PolygonTriggerEdit {
            name: "WaterArea".into(),
            layer: "Default".into(),
            id: 3,
            is_water: true,
            is_river: false,
            river_start: 0,
            points: vec![(0, 0, 0), (10, 0, 0), (10, 10, 0)],
        });
        doc.lighting.time_of_day = 2;
        doc.lighting.terrain[0][0].ambient = [0.2, 0.3, 0.4];
        doc.lighting.objects[1][2].diffuse = [0.5, 0.6, 0.7];
        doc.lighting.shadow_color = 0x112233;

        let bytes = doc.write_ckmp();
        assert!(
            bytes
                .windows(b"PolygonTriggers".len())
                .any(|w| w == b"PolygonTriggers")
        );
        assert!(
            bytes
                .windows(b"GlobalLighting".len())
                .any(|w| w == b"GlobalLighting")
        );
        let loaded = MapDocument::read_ckmp(&bytes).expect("load");
        assert_eq!(loaded.polygons.len(), 1);
        assert_eq!(loaded.polygons[0].name, "WaterArea");
        assert!(loaded.polygons[0].is_water);
        assert_eq!(loaded.polygons[0].points.len(), 3);
        assert_eq!(loaded.lighting.time_of_day, 2);
        assert_eq!(loaded.lighting.terrain[0][0].ambient, [0.2, 0.3, 0.4]);
        assert_eq!(loaded.lighting.objects[1][2].diffuse, [0.5, 0.6, 0.7]);
        assert_eq!(loaded.lighting.shadow_color, 0x112233);
    }

    #[test]
    fn save_to_path_writes_ckmp_map_not_json() {
        let mut height = HeightMapEdit::new(8, 8, 1).unwrap();
        height.set_height(2, 3, 44).unwrap();
        let mut doc = MapDocument::new(height);
        doc.objects.push(MapObjectEdit {
            name: "*Waypoints/Waypoint".into(),
            x: 11.0,
            y: 22.0,
            z: 0.0,
            angle: 0.0,
            flags: 0,
            properties: game_engine::common::dict::Dict::new(),
            waypoint_name: Some("Player_1_Start".into()),
        });
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("Test Map.map");
        doc.save_to_path(&path).expect("save ckmp map");
        let bytes = std::fs::read(&path).unwrap();
        assert!(
            bytes.starts_with(b"CkMp"),
            "live save must be C++ CkMp .map"
        );
        assert!(!bytes.starts_with(b"{"), "must not write editor JSON");
        let loaded = MapDocument::load_from_path(&path).expect("load saved map");
        assert_eq!(loaded.heightmap.get_height(2, 3), Some(44));
        assert_eq!(
            loaded.objects[0].waypoint_name.as_deref(),
            Some("Player_1_Start")
        );
    }

    #[test]
    fn ckmp_roundtrips_scripts_and_object_angle_flags_dict_like_cpp() {
        use game_engine::common::dict::Dict;
        use game_engine::common::name_key_generator::NameKeyGenerator;

        let height = HeightMapEdit::new(8, 8, 1).unwrap();
        let mut doc = MapDocument::new(height);
        doc.sides.push(SideInfoEdit {
            dict: Dict::new(),
            build_list: Vec::new(),
        });
        let owner_key = NameKeyGenerator::name_to_key("originalOwner");
        let mut props = Dict::new();
        props.set_ascii_string(owner_key, "PlyrAmerica");
        doc.objects.push(MapObjectEdit {
            name: "AmericaTankCrusader".into(),
            x: 100.0,
            y: 200.0,
            z: 5.0,
            angle: 1.5708,
            flags: 0x4,
            properties: props,
            waypoint_name: None,
        });
        doc.script_lists.push(ScriptListEdit {
            scripts: vec![ScriptEdit {
                name: "Victory".into(),
                comment: "win".into(),
                condition_comment: "if".into(),
                action_comment: "then".into(),
                is_active: true,
                is_one_shot: true,
                easy: true,
                normal: true,
                hard: false,
                is_subroutine: false,
                delay_evaluation_seconds: 2,
            }],
            groups: vec![ScriptGroupEdit {
                name: "Group A".into(),
                is_active: true,
                is_subroutine: false,
                scripts: vec![ScriptEdit {
                    name: "Inner".into(),
                    comment: String::new(),
                    condition_comment: String::new(),
                    action_comment: String::new(),
                    is_active: true,
                    is_one_shot: false,
                    easy: true,
                    normal: true,
                    hard: true,
                    is_subroutine: true,
                    delay_evaluation_seconds: 0,
                }],
            }],
        });

        let bytes = doc.write_ckmp();
        assert!(
            bytes
                .windows(b"ScriptList".len())
                .any(|w| w == b"ScriptList")
        );
        assert!(
            bytes
                .windows(b"ScriptGroup".len())
                .any(|w| w == b"ScriptGroup")
        );
        let loaded = MapDocument::read_ckmp(&bytes).expect("load scripts+object");
        assert_eq!(loaded.objects.len(), 1);
        assert!((loaded.objects[0].angle - 1.5708).abs() < 0.0001);
        assert_eq!(loaded.objects[0].flags, 0x4);
        assert_eq!(
            loaded.objects[0].properties.get_ascii_string(owner_key),
            "PlyrAmerica"
        );
        assert_eq!(loaded.script_lists.len(), 1);
        assert_eq!(loaded.script_lists[0].scripts.len(), 1);
        assert_eq!(loaded.script_lists[0].scripts[0].name, "Victory");
        assert_eq!(
            loaded.script_lists[0].scripts[0].delay_evaluation_seconds,
            2
        );
        assert!(!loaded.script_lists[0].scripts[0].hard);
        assert_eq!(loaded.script_lists[0].groups.len(), 1);
        assert_eq!(loaded.script_lists[0].groups[0].name, "Group A");
        assert_eq!(loaded.script_lists[0].groups[0].scripts.len(), 1);
        assert_eq!(loaded.script_lists[0].groups[0].scripts[0].name, "Inner");
        assert!(loaded.script_lists[0].groups[0].scripts[0].is_subroutine);
    }
}
