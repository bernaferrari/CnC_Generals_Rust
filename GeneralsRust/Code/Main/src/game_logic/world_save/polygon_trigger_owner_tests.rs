//! Receiving map admission keeps ordered polygons even while another owner is locked.
use super::*;
use game_engine::common::dict::Dict;
use game_engine::common::system::DataChunkOutput;
use game_engine::common::well_known_keys::{
    key_player_faction, key_player_is_human, key_player_name,
};
use std::path::Path;
fn map(rows: &[(&str, i32, i32)]) -> Vec<u8> {
    let mut out = DataChunkOutput::new();
    out.open_data_chunk("HeightMapData", 3);
    for value in [8, 8, 0, 64] {
        out.write_int(value);
    }
    for _ in 0..64 {
        out.write_byte(0);
    }
    out.close_data_chunk();
    out.open_data_chunk("PolygonTriggers", 4);
    out.write_int(rows.len() as i32);
    for &(name, id, height) in rows {
        out.write_ascii_string(name);
        out.write_ascii_string("OwnerLayer");
        out.write_int(id);
        out.write_byte(1);
        out.write_byte(0);
        out.write_int(0);
        out.write_int(4);
        for (x, y) in [(0, 0), (40, 0), (40, 40), (0, 40)] {
            out.write_int(x);
            out.write_int(y);
            out.write_int(height);
        }
    }
    out.close_data_chunk();
    let mut side = Dict::new();
    side.set_ascii_string(key_player_name(), "USA");
    side.set_ascii_string(key_player_faction(), "FactionAmerica");
    side.set_bool(key_player_is_human(), true);
    out.open_data_chunk("SidesList", 2);
    out.write_int(1);
    out.write_dict(&side);
    out.write_int(0);
    out.write_int(0);
    out.close_data_chunk();
    out.into_ckmp_bytes()
}
fn observed(terrain: &gamelogic::terrain::TerrainLogic) -> Vec<(String, i32, i32)> {
    terrain
        .get_trigger_areas()
        .get_triggers()
        .iter()
        .map(|trigger| {
            (
                trigger.get_trigger_name().to_string(),
                trigger.get_id(),
                trigger.get_point(0).unwrap().z,
            )
        })
        .collect()
}
fn expected(rows: &[(&str, i32, i32)]) -> Vec<(String, i32, i32)> {
    rows.iter()
        .map(|&(name, id, height)| (name.into(), id, height))
        .collect()
}
fn admit(world: &mut GameLogic, path: &Path, rows: &[(&str, i32, i32)]) {
    std::fs::write(path, map(rows)).unwrap();
    let chunky = crate::game_logic::script_loader::ChunkyMap::decode(
        path.to_path_buf(),
        std::fs::read(path).unwrap(),
    )
    .unwrap();
    let owner = world.world_services.terrain().clone();
    let mut saw_early = false;
    assert!(
        world.sync_legacy_runtime_from_fast_chunky_with_script_difficulty(
            path,
            &chunky,
            None,
            |_, phase| {
                if phase == "Fast sync roads" {
                    // This is a real native callback boundary: load progress can run
                    // authored WindowManager.update before canonical terrain admission.
                    assert_eq!(observed(&owner.read().unwrap()), expected(rows));
                    saw_early = true;
                }
            }
        )
    );
    assert!(
        saw_early,
        "the original early observable phase must execute"
    );
    let terrain = owner.read().unwrap();
    assert_eq!(
        observed(&terrain),
        expected(rows),
        "canonical admission must preserve the same ordered payload"
    );
    if let Some(&(name, id, height)) = rows.first() {
        assert_eq!(
            terrain
                .get_trigger_areas()
                .get_by_id(id)
                .unwrap()
                .get_trigger_name()
                .as_str(),
            name
        );
        assert_eq!(terrain.get_trigger_area_by_name(name).unwrap().get_id(), id);
        assert_eq!(
            terrain
                .get_trigger_area_by_name(name)
                .unwrap()
                .get_point(0)
                .unwrap()
                .z,
            height
        );
    }
}
#[test]
fn actual_map_polygon_admission_preserves_duplicates_and_same_id_receiving_owners() {
    crate::game_logic::game_logic::pose_owner_tests::isolated_at(
        module_path!(),
        "actual_map_polygon_admission_preserves_duplicates_and_same_id_receiving_owners",
        || {
            crate::skirmish_config::lobby_definition_tests::admit_slot_definitions();
            let temp = tempfile::tempdir().unwrap();
            let foreign = gamelogic::system::engine_stores::new_for_world();
            let mut foreign_terrain = foreign.services().terrain().write().unwrap();
            foreign_terrain.add_trigger_area(gamelogic::polygon_trigger::PolygonTrigger::new(
                99,
                "Foreign".into(),
                vec![
                    gamelogic::common::ICoord3D::new(0, 0, 99),
                    gamelogic::common::ICoord3D::new(40, 0, 99),
                    gamelogic::common::ICoord3D::new(40, 40, 99),
                    gamelogic::common::ICoord3D::new(0, 40, 99),
                ],
            ));
            let foreign_before = observed(&foreign_terrain);
            gamelogic::system::engine_stores::with_active_stores(&foreign, || {
                let mut a = GameLogic::new();
                let mut b = GameLogic::new();
                assert_eq!(
                    observed(&foreign_terrain),
                    foreign_before,
                    "constructors are inert"
                );
                let rows_a = [("Same", 10, 10), ("Same", 11, 20), ("Second", 10, 30)];
                let rows_b = [("Same", 10, 70), ("Same", 11, 80), ("Second", 10, 90)];
                admit(&mut a, &temp.path().join("A.map"), &rows_a);
                admit(&mut b, &temp.path().join("B.map"), &rows_b);
                let owner_a = a.world_services.terrain().clone();
                assert_eq!(observed(&owner_a.read().unwrap()), expected(&rows_a));
                b.reset();
                admit(
                    &mut b,
                    &temp.path().join("ResetB.map"),
                    &[("ResetOnly", 42, 40)],
                );
                assert_eq!(
                    observed(&owner_a.read().unwrap()),
                    expected(&rows_a),
                    "peer reset/admission cannot replace A"
                );
                assert_eq!(
                    observed(&foreign_terrain),
                    foreign_before,
                    "locked foreign owner never receives or suppresses native admission"
                );
            });
            // Keep the foreign guard alive across both actual admissions/reset.
            assert_eq!(observed(&foreign_terrain), foreign_before);
        },
    );
}
