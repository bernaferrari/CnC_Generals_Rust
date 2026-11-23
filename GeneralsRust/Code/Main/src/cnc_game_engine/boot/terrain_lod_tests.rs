use super::*;
use game_engine::common::common_services::{CommonServices, with_common_services};
use game_engine::common::game_lod::load_game_lod_ini_presets_and_options;
use game_engine::common::ini::ini_game_data::{TerrainTrackRules, ensure_global_data};

#[test]
fn actual_startup_terrain_lod_admits_the_selected_owner_across_boot_phases() {
    let peer = Arc::new(CommonServices::new());
    let poison = with_common_services(&peer, || {
        let global = ensure_global_data();
        let mut global = global.write();
        global.max_tank_track_edges = 4;
        global.max_tank_track_opaque_edges = 1;
        global.max_tank_track_fade_delay = 8;
        TerrainTrackRules::from_global_data(&global)
    });

    for (option, enabled, level) in [
        ("High", true, StaticGameLODLevel::High),
        ("Custom", true, StaticGameLODLevel::Custom),
        ("Low", false, StaticGameLODLevel::Custom),
        ("Unrecognized", true, StaticGameLODLevel::Unknown),
    ] {
        let startup = Arc::new(CommonServices::new());
        let user_data = tempfile::tempdir().unwrap();
        std::fs::write(
            user_data.path().join("Options.ini"),
            format!("StaticGameLOD = {option}\nIdealStaticGameLOD = High\n"),
        )
        .unwrap();
        let (terrain_lod, authored, use_trees) = with_common_services(&startup, || {
            let global = ensure_global_data();
            {
                let mut global = global.write();
                global.set_path_user_data(user_data.path().to_str().unwrap().into());
                global.enable_static_lod = enabled;
                global.max_tank_track_edges = 37;
                global.max_tank_track_opaque_edges = 11;
                global.max_tank_track_fade_delay = 173;
            }
            load_game_lod_ini_presets_and_options();
            let lod = startup.ini.game_lod.read();
            assert_eq!(lod.get_static_lod_level(), level, "{option}");
            let authored = TerrainTrackRules::from_global_data(&global.read());
            let use_trees = global.read().use_trees;
            (StartupTerrainLod::capture(&lod, use_trees), authored, use_trees)
        });

        let actual = with_common_services(&peer, || {
            let mut world = GameLogic::new();
            let mut late_lighting = LightingContentRules::default();
            late_lighting.terrain_tracks.max_terrain_tracks = 13;
            late_lighting.terrain_tracks.make_track_marks = false;
            terrain_lod.admit(&mut world, late_lighting);
            (
                world.effective_terrain_track_rules(),
                world.current_static_lod_level(),
                world.current_map_tree_setting(),
            )
        });
        let expected = TerrainTrackRules {
            max_terrain_tracks: 13,
            make_track_marks: false,
            ..authored
        };
        assert_eq!(actual, (expected, Some(level), use_trees), "{option}");
    }

    with_common_services(&peer, || {
        assert_eq!(
            TerrainTrackRules::from_global_data(&ensure_global_data().read()),
            poison
        );
    });
}
