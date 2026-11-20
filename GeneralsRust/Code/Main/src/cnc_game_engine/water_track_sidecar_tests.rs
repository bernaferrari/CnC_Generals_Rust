use super::*;
use crate::game_logic::GameLogic;
use game_client::terrain::water_tracks::{
    WaterTrackSaveRecord, WaterTrackType, encode_wak_records,
};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

const MAP: &str = "Maps/TournamentB/TournamentB.map";

#[test]
#[ignore = "requires the retail Zero Hour MapsZH.big"]
fn mounted_map_preparation_admits_each_receiving_water_track_sidecar() {
    let assets_dir = PathBuf::from(
        std::env::var_os("GENERALS_ASSETS_DIR").expect("retail Zero Hour assets required"),
    );
    let archive = assets_dir.join("Data").join("MapsZH.big");
    let common = Arc::new(game_engine::common::common_services::CommonServices::new());
    let main = Arc::new(crate::main_services::MainServices::new());
    game_engine::common::common_services::with_common_services(&common, || {
        crate::main_services::with_main_services(&main, || {
            let peer_root = tempfile::tempdir().unwrap();
            let local_map = peer_root.path().join(MAP);
            std::fs::create_dir_all(local_map.parent().unwrap()).unwrap();

            let mut first_files = crate::assets::AssetManager::new().unwrap();
            let files = first_files.archive_file_system_mut();
            files.set_local_search_roots(Vec::new());
            pollster::block_on(files.load_big_file(&archive)).unwrap();
            std::fs::write(&local_map, files.open_file_sync(MAP).unwrap()).unwrap();
            let mut first = receiver(first_files);
            let mut first_candidate = prepare(&mut first, 73);
            let first_terrain = first_candidate.visual.as_mut().unwrap();
            let first_wakes = first_terrain.water_tracks_mut();
            assert_eq!(first_wakes.used_handles().len(), 921);
            assert!(
                first_wakes
                    .used_handles()
                    .iter()
                    .all(|&handle| { first_wakes.track(handle).unwrap().elapsed_ms() == 0.0 }),
                "loading the authored sidecar must not advance the wake clock"
            );
            assert!(first_terrain.last_water_tracks_flush().ranges.is_empty());

            let peer_record = WaterTrackSaveRecord {
                start: glam::Vec2::new(210.0, 350.0),
                end: glam::Vec2::new(238.0, 350.0),
                wave_type: WaterTrackType::Pond,
            };
            std::fs::write(
                local_map.with_extension("wak"),
                encode_wak_records(&[peer_record]),
            )
            .unwrap();
            let mut peer_files = crate::assets::AssetManager::new().unwrap();
            peer_files
                .archive_file_system_mut()
                .set_local_search_roots([peer_root.path().to_owned()]);
            let mut peer = receiver(peer_files);
            let mut peer_candidate = prepare(&mut peer, 73);
            let peer_terrain = peer_candidate.visual.as_mut().unwrap();
            assert_eq!(
                peer_terrain.water_tracks_mut().save_records(),
                [peer_record]
            );
            assert!(peer_terrain.last_water_tracks_flush().ranges.is_empty());
            assert_eq!(first_terrain.water_tracks_mut().used_handles().len(), 921);

            // The same virtual source in a new attempt can have no sidecar.
            std::fs::remove_file(local_map.with_extension("wak")).unwrap();
            peer_candidate = prepare(&mut peer, 74);
            assert!(
                peer_candidate
                    .visual
                    .as_mut()
                    .unwrap()
                    .water_tracks_mut()
                    .used_handles()
                    .is_empty()
            );
            assert_eq!(first_terrain.water_tracks_mut().used_handles().len(), 921);

            // Saved games load the extracted geometry file. Pristine sources
            // still supply map definitions, but not the geometry's .wak.
            let mut restored = NativeMapCandidate::with_epoch(75);
            assert!(
                first
                    .load_map_with_definition_source_and_water_delivery_and_native_checked(
                        local_map.to_str().unwrap(),
                        MAP,
                        None,
                        restored.receiver(),
                    )
                    .unwrap()
            );
            assert!(
                restored
                    .visual
                    .as_mut()
                    .unwrap()
                    .water_tracks_mut()
                    .used_handles()
                    .is_empty()
            );
            assert_eq!(first_terrain.water_tracks_mut().used_handles().len(), 921);
        });
    });
}

fn receiver(files: crate::assets::AssetManager) -> GameLogic {
    let mut world = GameLogic::new();
    world.admit_camera_application_movement(
        game_engine::common::ini::ini_game_data::CameraMovementRules::default()
            .overlay_ini_text("GameData\n PartitionCellSize = 40\nEnd\n")
            .unwrap(),
    );
    world.bind_application_assets(Arc::new(Mutex::new(files)));
    world
}

fn prepare(world: &mut GameLogic, epoch: u64) -> NativeMapCandidate {
    let mut candidate = NativeMapCandidate::with_epoch(epoch);
    let mut phase = String::new();
    assert!(
        world.load_map_with_progress_and_native(
            MAP,
            |_, label| phase = label.to_owned(),
            candidate.receiver()
        ),
        "native terrain candidate did not load {MAP} after {phase}"
    );
    candidate
}
