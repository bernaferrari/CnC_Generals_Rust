//! Original INITerrain.cpp15-35 existing-name overlay and TerrainTypes.cpp110-141
//! DefaultTerrain inheritance; GameLogic.cpp2404-2415 Map.ini then Solo.ini.
use super::*;
use crate::game_logic::GameLogic;

fn base(texture: &str) -> Arc<TerrainTypes> {
    parse_application_terrain_sources(&[
        ("Default/Terrain.ini", "Terrain DefaultTerrain\n Texture = default.tga\n Class = GRASS\n BlendEdges = Yes\nEnd\n".into()),
        ("Terrain.ini", format!("Terrain SameName\n Texture = {texture}\n Class = ROCK\nEnd\n")),
    ]).unwrap()
}
fn texture(definitions: &TerrainTypes) -> &str {
    definitions
        .find_terrain(&game_engine::common::ascii_string::AsciiString::from(
            "SameName",
        ))
        .unwrap()
        .texture_name
        .as_str()
}
fn map_at(directory: &std::path::Path) -> std::path::PathBuf {
    let mut data = game_engine::common::system::DataChunkOutput::new();
    data.open_data_chunk("HeightMapData", 3);
    for value in [8, 8, 0, 64] {
        data.write_int(value);
    }
    for _ in 0..64 {
        data.write_byte(40);
    }
    data.close_data_chunk();
    let path = directory.join("OwnedTerrain.map");
    std::fs::write(&path, data.into_ckmp_bytes()).unwrap();
    path
}

#[test]
fn inert_terrain_definition_owners_ignore_held_foreign_registry_and_keep_default_inheritance() {
    crate::game_logic::game_logic::pose_owner_tests::isolated_at(
        module_path!(),
        "inert_terrain_definition_owners_ignore_held_foreign_registry_and_keep_default_inheritance",
        || {
            let foreign = game_engine::common::ini::ini_terrain::initialize_terrain_types();
            let guard = foreign.write();
            let before = guard.get_terrain_count();
            let mut a = GameLogic::new();
            let mut b = GameLogic::new();
            assert_eq!(a.effective_terrain_definitions().get_terrain_count(), 0);
            a.admit_terrain_definitions(base("a.tga")).unwrap();
            b.admit_terrain_definitions(base("b.tga")).unwrap();
            assert_eq!(texture(a.effective_terrain_definitions()), "a.tga");
            assert_eq!(texture(b.effective_terrain_definitions()), "b.tga");
            let row = a
                .effective_terrain_definitions()
                .find_terrain(&game_engine::common::ascii_string::AsciiString::from(
                    "SameName",
                ))
                .unwrap();
            assert!(
                row.blend_edge_texture,
                "new terrain inherits complete DefaultTerrain before overlay"
            );
            assert_eq!(row.terrain_class.as_str(), "ROCK");
            b.reset();
            drop(b);
            assert_eq!(texture(a.effective_terrain_definitions()), "a.tga");
            assert_eq!(
                guard.get_terrain_count(),
                before,
                "constructor/parser/reset never publishes TerrainTypes"
            );
        },
    );
}

#[test]
fn actual_map_solo_catalog_survives_reset_next_map_and_rejected_replacement() {
    crate::game_logic::game_logic::pose_owner_tests::isolated_at(
        module_path!(),
        "actual_map_solo_catalog_survives_reset_next_map_and_rejected_replacement",
        || {
            let dir = tempfile::tempdir().unwrap();
            let path = map_at(dir.path());
            std::fs::write(
                dir.path().join("Map.ini"),
                "Terrain SameName\n Texture = map.tga\nEnd\n",
            )
            .unwrap();
            std::fs::write(
                dir.path().join("Solo.ini"),
                "Terrain SameName\n Texture = solo.tga\nEnd\n",
            )
            .unwrap();
            let peer_dir = tempfile::tempdir().unwrap();
            let peer_path = map_at(peer_dir.path());
            let mut owner = GameLogic::new();
            let mut peer = GameLogic::new();
            let initial = base("base.tga");
            owner
                .admit_terrain_definitions(Arc::clone(&initial))
                .unwrap();
            peer.admit_terrain_definitions(base("peer.tga")).unwrap();
            let foreign = game_engine::common::ini::ini_terrain::initialize_terrain_types();
            let guard = foreign.write();
            let before = guard.get_terrain_count();
            assert!(owner.load_map(path.to_str().unwrap()));
            assert_eq!(texture(owner.effective_terrain_definitions()), "solo.tga");
            assert_eq!(texture(&initial), "base.tga");
            let row = owner
                .effective_terrain_definitions()
                .find_terrain(&game_engine::common::ascii_string::AsciiString::from(
                    "SameName",
                ))
                .unwrap();
            assert_eq!(
                row.terrain_class.as_str(),
                "ROCK",
                "existing-name overrides retain fields not present in Map/Solo"
            );
            assert!(row.blend_edge_texture);
            let frozen = crate::presentation_frame::PresentationWorldEnv::from_logic(&owner);
            assert!(Arc::ptr_eq(
                frozen.terrain_definitions.as_ref().unwrap(),
                owner.effective_terrain_definitions()
            ));
            assert!(peer.load_map(peer_path.to_str().unwrap()));
            peer.reset();
            drop(peer);
            assert_eq!(texture(owner.effective_terrain_definitions()), "solo.tga");
            assert_eq!(
                texture(frozen.terrain_definitions.as_ref().unwrap()),
                "solo.tga"
            );
            owner.reset();
            assert_eq!(
                texture(owner.effective_terrain_definitions()),
                "solo.tga",
                "C++ TerrainTypeCollection::reset is a no-op"
            );
            assert!(owner.load_map(peer_path.to_str().unwrap()));
            assert_eq!(
                texture(owner.effective_terrain_definitions()),
                "solo.tga",
                "next map without Terrain blocks retains the receiving catalog"
            );
            std::fs::write(
                dir.path().join("Map.ini"),
                "Terrain SameName\n Class = NOT_A_SOURCE_CLASS\nEnd\n",
            )
            .unwrap();
            assert!(
                !owner.load_map(path.to_str().unwrap()),
                "invalid complete owned CREATE_OVERRIDES must reject admission"
            );
            assert_eq!(texture(&initial), "base.tga");
            assert_eq!(
                texture(owner.effective_terrain_definitions()),
                "solo.tga",
                "failed new map preserves the receiving catalog, not partially parsed overrides"
            );
            assert_eq!(
                texture(frozen.terrain_definitions.as_ref().unwrap()),
                "solo.tga",
                "old frozen immutable view survives owner reset and rejected replacement"
            );
            assert_eq!(guard.get_terrain_count(), before);
        },
    );
}
