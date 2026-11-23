use super::*;
use crate::game_logic::{GameLogic, GameMode};
use game_client::terrain::{
    TreeSaveRecord,
    tree_buffer::{MAX_SWAY_TYPES, W3DToppleState},
};
use game_engine::common::{dict::Dict, system::DataChunkOutput};

const TREE: &str = "MountedOptimizedMapDogwood";
const MODEL: &str = "PTDogwodHI1";
const TEXTURE: &str = "PTDogwod01.tga";

fn write_map(path: &std::path::Path, x: f32, angle: f32) {
    let mut map = DataChunkOutput::new();
    map.open_data_chunk("HeightMapData", 3);
    for value in [41, 41, 0, 1681] {
        map.write_int(value);
    }
    for _ in 0..1681 {
        map.write_byte(30);
    }
    map.close_data_chunk();
    map.open_data_chunk("ObjectsList", 3);
    map.open_data_chunk("Object", 3);
    for value in [x, 110.0, 2.0, angle] {
        map.write_real(value);
    }
    map.write_int(0);
    map.write_ascii_string(TREE);
    map.write_dict(&Dict::new());
    map.close_data_chunk();
    map.close_data_chunk();
    std::fs::write(path, map.into_ckmp_bytes()).unwrap();
}

fn admit_tree_map(
    pipeline: &mut RenderPipeline,
    graphics: &mut GraphicsSystem,
    path: &std::path::Path,
    epoch: u64,
    saved_foundation: bool,
    use_trees: bool,
    x: f32,
    angle: f32,
) -> (NativeMapCandidate, GameLogic, Option<TreeSaveRecord>) {
    let content = crate::assets::ReceivingObjectContent::from_sources([(
        "mounted-optimized-tree.ini".into(),
        format!(
            "Object {TREE}\nKindOf = IMMOBILE SHRUBBERY OPTIMIZED_TREE\nDraw = W3DTreeDraw TreeTag\nModelName = {MODEL}\nTextureName = {TEXTURE}\nEnd\nEnd\n"
        ),
    )])
    .unwrap();
    let mut world = GameLogic::new();
    world.admit_receiving_module_content(content).unwrap();
    if saved_foundation {
        world
            .start_new_game_for_restore(GameMode::SinglePlayer)
            .unwrap();
        world.begin_saved_snapshot_admission().unwrap();
    } else {
        world.start_new_game(GameMode::SinglePlayer).unwrap();
    }
    world.admit_map_tree_setting(use_trees);
    world.require_native_drawable_owner();
    let mut expected_random = world.session_view.client_random.clone();
    // Both source calls occur. A real interval of [1,1] consumes no word.
    let expected_sway = use_trees
        .then(|| expected_random.next_int(0, MAX_SWAY_TYPES as i32 - 1));
    let mut candidate = NativeMapCandidate::with_epoch(epoch);
    let mut retirement = game_client::core::game_client::NativeParticleRetirementService;
    {
        let mut attempt = candidate
            .attempt(
                pipeline.borrow_native_draw_services(),
                graphics,
                0,
                super::super::native_view_effects::receiving_native_drawable_policy_for(&world),
                &mut retirement,
                None,
            )
            .unwrap();
        let map = path.to_str().unwrap();
        assert!(
            world
                .load_map_with_definition_source_and_water_delivery_and_native_checked(
                    map,
                    map,
                    None,
                    Some(&mut attempt),
                )
                .unwrap(),
            "the authored CkMp map was accepted"
        );
    }
    assert!(world.host_objects().registered_is_empty());
    assert!(
        candidate
            .drawable_owners()
            .0
            .native_draw_candidates()
            .unwrap()
            .is_empty(),
        "the exact objectless Drawable retired during map admission"
    );
    let ground = world
        .terrain_ground_height_at(glam::Vec3::new(x, 0.0, 110.0))
        .unwrap();
    let trees = candidate.visual.as_mut().unwrap().tree_buffer_mut();
    let expected_tree_count = if use_trees { 1 } else { 0 };
    assert_eq!(trees.trees().len(), expected_tree_count);
    assert_eq!(trees.tree_types().len(), expected_tree_count);
    let record = trees.save_records().pop();
    if use_trees {
        assert!(trees.tree_types()[0].mesh.is_some(), "mounted W3D model");
        assert_eq!(trees.trees()[0].sway_type, expected_sway.unwrap());
        let record = record.as_ref().unwrap();
        assert_eq!(record.model_name, MODEL);
        assert_eq!(record.model_texture, TEXTURE);
        assert_eq!(record.location, glam::Vec3::new(x, 110.0, ground + 2.0));
        assert!((record.sin - angle.sin()).abs() < 1e-6);
        assert!((record.cos - angle.cos()).abs() < 1e-6);
    } else {
        assert!(record.is_none(), "disabled SHRUBBERY row creates no tree record");
    }
    assert_eq!(
        world.session_view.client_random.seed_words(),
        expected_random.seed_words()
    );
    (candidate, world, record)
}

#[test]
fn mounted_optimized_map_tree_retires_its_drawable_on_fresh_and_saved_foundations() {
    crate::game_logic::game_logic::pose_owner_tests::isolated_at(
        module_path!(),
        "mounted_optimized_map_tree_retires_its_drawable_on_fresh_and_saved_foundations",
        || {
            let directory = tempfile::tempdir().unwrap();
            let fresh_map = directory.path().join("FreshOptimizedTree.map");
            let saved_map = directory.path().join("SavedOptimizedTree.map");
            write_map(&fresh_map, 90.0, 0.75);
            write_map(&saved_map, 170.0, -0.5);

            let engine = ww3d_engine::init_headless_blocking(ww3d_engine::EngineConfig {
                width: 32,
                height: 32,
                ..Default::default()
            })
            .unwrap();
            let (device, queue, color, depth) = {
                let engine = engine.lock();
                (
                    engine.device(),
                    engine.queue(),
                    engine.color_format(),
                    engine.depth_format(),
                )
            };
            let mut graphics = GraphicsSystem::new(device, queue, color, depth).unwrap();
            let mut pipeline = RenderPipeline::initialize(engine, &graphics).unwrap();
            let mut assets = crate::assets::AssetManager::new().unwrap();
            let retail = crate::fixture_content::retail_content_root()
                .join("Command & Conquer Generals Zero Hour/Data");
            let archive = assets.archive_file_system_mut();
            archive.set_local_search_roots(Vec::new());
            for name in ["W3DZH.big", "TerrainZH.big"] {
                pollster::block_on(archive.load_big_file(retail.join(name))).unwrap();
            }
            pipeline.bind_application_assets(std::sync::Arc::new(std::sync::Mutex::new(assets)));

            let (mut fresh, fresh_world, fresh_record) =
                admit_tree_map(
                    &mut pipeline,
                    &mut graphics,
                    &fresh_map,
                    41,
                    false,
                    true,
                    90.0,
                    0.75,
                );
            let fresh_record = fresh_record.unwrap();
            let fresh_rng = fresh_world.session_view.client_random.seed_words();
            let (mut saved, mut saved_world, saved_record) =
                admit_tree_map(
                    &mut pipeline,
                    &mut graphics,
                    &saved_map,
                    42,
                    true,
                    true,
                    170.0,
                    -0.5,
                );
            let saved_record = saved_record.unwrap();
            assert_eq!(fresh_record.drawable_id, saved_record.drawable_id);
            assert_ne!(fresh_record.location, saved_record.location);

            use crate::save_load::{GameDifficulty, SaveFileManager, SaveFileType, SaveGameInfo};
            let source_visual = fresh.visual.as_mut().unwrap();
            let source_trees = source_visual.tree_buffer_mut();
            assert!(source_trees.apply_toppling_force(
                fresh_record.drawable_id,
                glam::Vec3::X,
                0.2,
                0,
            ));
            let falling = source_trees.save_records().pop().unwrap();
            assert_eq!(falling.topple_state, W3DToppleState::Falling);
            let mut saves = SaveFileManager::with_save_directory(directory.path().join("Save"));
            let mut client = game_client::core::game_client::GameClient::new().unwrap();
            saves
                .save_game_with_client_save_state_and_native_terrain(
                    "falling-tree",
                    &fresh_world,
                    Default::default(),
                    &mut client,
                    &SaveGameInfo {
                        filename: "falling-tree".into(),
                        display_name: "Falling tree".into(),
                        description: String::new(),
                        map_name: fresh_map.to_string_lossy().into_owned(),
                        pristine_map_name: None,
                        campaign_side: None,
                        mission_number: None,
                        save_date: std::time::SystemTime::now(),
                        game_version: String::new(),
                        play_time: std::time::Duration::ZERO,
                        difficulty: GameDifficulty::Medium,
                        save_type: SaveFileType::Normal,
                    },
                    Default::default(),
                    Some(source_visual),
                )
                .unwrap();
            let archive = saves.load_game_snapshot("falling-tree").unwrap();
            let terrain = crate::save_load::snapshot::prepare_receiving_terrain_visual(
                &saved_world,
                archive.terrain_visual_xfer.as_deref(),
            )
            .unwrap()
            .unwrap();
            assert_eq!(terrain.tree_records().unwrap(), [falling.clone()]);
            let receiver = saved.visual.as_mut().unwrap();
            assert_eq!(receiver.capture_tree_records(), [saved_record]);
            let mut expected_random = saved_world.session_view.client_random.clone();
            let expected_sway = expected_random.next_int(0, MAX_SWAY_TYPES as i32 - 1);
            terrain.apply_native(
                receiver,
                &mut super::super::native_frame_receiver::ReceivingTreeRandom(
                    &mut saved_world.session_view.client_random,
                ),
            );
            let restored = receiver.capture_tree_records().pop().unwrap();
            let mut expected_restored = falling.clone();
            expected_restored.sin = 0.0;
            expected_restored.cos = 1.0;
            expected_restored.angular_accumulation = 0.0;
            assert_eq!(restored, expected_restored);
            assert_eq!(receiver.tree_buffer_mut().trees()[0].sway_type, expected_sway);
            assert_eq!(
                saved_world.session_view.client_random.seed_words(),
                expected_random.seed_words(),
            );
            assert_eq!(
                fresh
                    .visual
                    .as_mut()
                    .unwrap()
                    .tree_buffer_mut()
                    .save_records(),
                [falling],
            );
            assert_eq!(
                fresh_world.session_view.client_random.seed_words(),
                fresh_rng
            );

            let disabled_map = directory.path().join("TreesDisabled.map");
            write_map(&disabled_map, 130.0, 0.25);
            let (mut disabled, disabled_world, disabled_record) = admit_tree_map(
                &mut pipeline,
                &mut graphics,
                &disabled_map,
                43,
                false,
                false,
                130.0,
                0.25,
            );
            assert!(disabled_record.is_none());
            assert!(
                disabled
                    .visual
                    .as_mut()
                    .unwrap()
                    .tree_buffer_mut()
                    .trees()
                    .is_empty()
            );
            assert_eq!(fresh.visual.as_mut().unwrap().tree_buffer_mut().trees().len(), 1);
            assert!(!disabled_world.current_map_tree_setting());
            assert!(fresh_world.current_map_tree_setting());
        },
    );
}
