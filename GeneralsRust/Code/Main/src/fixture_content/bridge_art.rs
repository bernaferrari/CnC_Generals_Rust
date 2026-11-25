//! Synthetic W3D art used by native authored bridge fixtures.

use std::path::Path;
use std::sync::{Arc, Mutex};

use game_engine::common::ini::ini_road::TerrainRoadCollection;
use ww3d_core::W3DChunkType::{self, *};

use crate::game_logic::GameLogic;

pub(crate) const MODEL: &str = "BridgeFixture";
const TREE: &str = "FixtureTree";
const SECTION: &str = "BRIDGE_LEFT";

pub(crate) fn attach(
    world: &mut GameLogic,
    root: &Path,
    names: impl IntoIterator<Item = String>,
    lateral_bounds: [f32; 2],
    pivot_y: f32,
    scale: f32,
) {
    let art = root.join("art/w3d");
    std::fs::create_dir_all(&art).unwrap();
    std::fs::write(
        art.join("bridgefixture.w3d"),
        model(lateral_bounds, pivot_y),
    )
    .unwrap();
    let mut assets = crate::assets::AssetManager::new().unwrap();
    assets
        .archive_file_system_mut()
        .set_local_search_roots([root.to_owned()]);
    world.bind_application_assets(Arc::new(Mutex::new(assets)));

    let mut definitions = TerrainRoadCollection::new();
    for name in names {
        let bridge = definitions.new_bridge(name.into());
        bridge.bridge_model_name = MODEL.into();
        bridge.bridge_scale = scale;
    }
    world.admit_terrain_road_definitions(definitions).unwrap();
}

fn chunk(kind: W3DChunkType, container: bool, data: Vec<u8>) -> Vec<u8> {
    let size = data.len() as u32 | (u32::from(container) << 31);
    [kind as u32, size]
        .into_iter()
        .flat_map(u32::to_le_bytes)
        .chain(data)
        .collect()
}

fn name(value: &str, width: usize) -> Vec<u8> {
    let mut data = value.as_bytes().to_vec();
    data.resize(width, 0);
    data
}

fn floats(values: &[f32]) -> Vec<u8> {
    values
        .iter()
        .flat_map(|value| value.to_le_bytes())
        .collect()
}

pub(crate) fn model([min, max]: [f32; 2], pivot_y: f32) -> Vec<u8> {
    let mesh_header = [
        vec![0; 8],
        name(SECTION, 16),
        name(MODEL, 16),
        1u32.to_le_bytes().to_vec(),
        3u32.to_le_bytes().to_vec(),
        vec![0; 68],
    ]
    .concat();
    let vertices = floats(&[-6.0, min, 0.0, 0.0, max, 0.0, 6.0, min, 0.0]);
    let mesh = chunk(
        Mesh,
        true,
        [
            chunk(MeshHeader3, false, mesh_header),
            chunk(Vertices, false, vertices),
        ]
        .concat(),
    );

    let tree_header = [
        0x0004_0001u32.to_le_bytes().to_vec(),
        name(TREE, 16),
        2u32.to_le_bytes().to_vec(),
        vec![0; 12],
    ]
    .concat();
    let pivots = [(-1i32, 0.0), (0, pivot_y)]
        .into_iter()
        .flat_map(|(parent, y)| {
            [
                vec![0; 16],
                parent.to_le_bytes().to_vec(),
                floats(&[0.0, y, 0.0]),
                vec![0; 12],
                floats(&[0.0, 0.0, 0.0, 1.0]),
            ]
            .concat()
        })
        .collect();
    let tree = chunk(
        Hierarchy,
        true,
        [
            chunk(HierarchyHeader, false, tree_header),
            chunk(Pivots, false, pivots),
        ]
        .concat(),
    );

    let hlod_header = [
        vec![0; 4],
        1u32.to_le_bytes().to_vec(),
        name(MODEL, 16),
        name(TREE, 16),
    ]
    .concat();
    let lod = chunk(
        HlodLodArray,
        true,
        [
            chunk(
                HlodSubObjectArrayHeader,
                false,
                [1u32.to_le_bytes(), f32::MAX.to_le_bytes()].concat(),
            ),
            chunk(
                HlodSubObject,
                false,
                [
                    1u32.to_le_bytes().to_vec(),
                    name(&format!("{MODEL}.{SECTION}"), 32),
                ]
                .concat(),
            ),
        ]
        .concat(),
    );
    let hlod = chunk(
        Hlod,
        true,
        [chunk(HlodHeader, false, hlod_header), lod].concat(),
    );
    [mesh, tree, hlod].concat()
}
