use super::*;
use crate::assets::models::W3DLoader;
use crate::game_logic::{GameLogic, ObjectId, Player, ScriptPresentationEffects, Team};
use crate::graphics::render_pipeline::RenderPipeline;
use crate::parallel_harness::RetailWorldDefinitions;
use game_client::core::game_client::{GameClient, NativeParticleRetirementService};
use game_client::terrain::height_map::HeightMap;
use game_client::terrain::terrain_tracks::TerrainTracksConfig;
use game_client::terrain::terrain_visual::TerrainVisualImpl;
use game_engine::common::system::archive_file_system::ArchiveFileSystem;
use std::io::Read;

const EPOCH: u64 = 103;
const TRACK_WIDTH: f32 = 18.573_772;

#[test]
#[ignore = "requires mounted retail sources and native WGPU"]
fn retail_tank_factory_binds_before_placement_and_client_reset_retires_own_marks() {
    let install = std::path::PathBuf::from(std::env::var_os("GENERALS_BASE_DIR").unwrap());
    let mut archive = ArchiveFileSystem::new();
    archive
        .open_archive_file(install.join("W3D.big").to_str().unwrap())
        .unwrap();
    let mut code = Vec::new();
    archive
        .open_file("Art/W3D/NVBtMstr.w3d", 0)
        .unwrap()
        .read_to_end(&mut code)
        .unwrap();

    let engine = ww3d_engine::init_headless_blocking(ww3d_engine::EngineConfig {
        width: 32,
        height: 32,
        ..Default::default()
    })
    .unwrap();
    let mut graphics = {
        let engine = engine.lock();
        GraphicsSystem::new(
            engine.device(),
            engine.queue(),
            engine.color_format(),
            engine.depth_format(),
        )
        .unwrap()
    };
    graphics.cache_model(
        crate::assets::mesh_asset_resolve::canonical_model_key("NVBtMstr"),
        W3DLoader::new()
            .load_model_from_bytes(&code, "NVBtMstr")
            .unwrap(),
    );
    let definitions = RetailWorldDefinitions::load().unwrap();
    let mut make_world = |position: Vec3| {
        let mut world = GameLogic::new();
        world
            .with_own_services_mut(|world| definitions.admit(world))
            .unwrap();
        world.add_player(Player::new(0, Team::China, "Track Owner", true));
        world.require_native_drawable_owner();
        let mut logical_map = gamelogic::system::map_loader::MapData::new();
        logical_map.width = 16;
        logical_map.height = 16;
        logical_map.heightmap = vec![0; 16 * 16];
        world
            .world_services
            .terrain()
            .write()
            .unwrap()
            .try_load_map_geometry(logical_map)
            .unwrap();
        let mut terrain = TerrainVisualImpl::new();
        terrain
            .load_heightmap_from_data(HeightMap::new(16, 16, 200.0, 10.0), None, None)
            .unwrap();
        terrain.set_terrain_tracks_detail_with_config(TerrainTracksConfig::from_rules(
            &world.effective_terrain_track_rules(),
        ));
        assert!(terrain.track_mark_admission_enabled());
        let mut client = GameClient::new().unwrap();
        let mut pipeline = RenderPipeline::initialize(engine.clone(), &graphics).unwrap();
        pipeline.install_native_terrain_cpu(Some(terrain));
        pipeline.observe_animation_sync(1234);

        let id = with_view(
            &mut client,
            &mut pipeline,
            &mut graphics,
            &mut world,
            |world, effects| {
                world
                    .create_object_with_script_effects_checked(
                        "ChinaTankBattleMaster",
                        Team::China,
                        position,
                        None,
                        effects,
                    )
                    .unwrap()
                    .unwrap()
            },
        );
        (world, client, pipeline, id)
    };
    let (mut world, mut client, mut render, id) = make_world(Vec3::new(20.0, 0.0, 20.0));
    let (_peer_world, mut peer_client, peer_render, peer_id) =
        make_world(Vec3::new(40.0, 0.0, 50.0));
    assert_eq!(peer_id, id);
    let receipt = track_binding(&mut client, id);
    let peer_receipt = track_binding(&mut peer_client, peer_id);
    assert_eq!(receipt.slot(), peer_receipt.slot());
    assert!(
        !render
            .native_terrain()
            .unwrap()
            .receives_terrain_track(&peer_receipt)
    );

    with_view(
        &mut client,
        &mut render,
        &mut graphics,
        &mut world,
        |world, effects| {
            for x in [45.0, 70.0] {
                assert!(world.set_registered_object_position_with_effects(
                    id,
                    Vec3::new(x, 0.0, 20.0),
                    4,
                    effects,
                ));
            }
        },
    );
    {
        let terrain = render.native_terrain().unwrap();
        let tracks = terrain.terrain_tracks();
        let edges = tracks
            .track(receipt.slot())
            .unwrap()
            .active_edges(tracks.config().max_tank_track_edges);
        assert_eq!(
            edges.len(),
            2,
            "factory placement supplied the first anchor"
        );
        for (edge, x) in edges.iter().zip([45.0, 70.0]) {
            assert_eq!(edge.time_added, 1234);
            assert_eq!(edge.endpoint_pos[0].x, x);
            assert!(
                ((edge.endpoint_pos[0] - edge.endpoint_pos[1]).length() - TRACK_WIDTH).abs() < 1e-5
            );
        }
    }
    assert!(
        render
            .retire_native_drawables_after_client_reset(&client.borrow_native_drawable_owner())
            .is_err()
    );
    assert!(
        render
            .native_terrain()
            .unwrap()
            .receives_terrain_track(&receipt)
    );
    client.reset().unwrap();
    render
        .retire_native_drawables_after_client_reset(&client.borrow_native_drawable_owner())
        .unwrap();
    let terrain = render.native_terrain().unwrap();
    assert!(!terrain.receives_terrain_track(&receipt));
    let track = terrain.terrain_tracks().track(receipt.slot()).unwrap();
    assert!(!track.bound());
    assert_eq!(track.active_edge_count(), 2);
    assert!(
        peer_render
            .native_terrain()
            .unwrap()
            .receives_terrain_track(&peer_receipt)
    );
}

fn with_view<T>(
    client: &mut GameClient,
    render: &mut RenderPipeline,
    graphics: &mut GraphicsSystem,
    world: &mut GameLogic,
    callback: impl FnOnce(&mut GameLogic, &mut ScriptPresentationEffects<'_>) -> T,
) -> T {
    let (poses, terrain) = render.borrow_native_installed_poses_and_terrain();
    let mut retirement = NativeParticleRetirementService;
    let mut view = NativeRendererViewEffects::new(
        poses,
        graphics,
        client.borrow_native_drawable_owner(),
        EPOCH,
        0,
        receiving_native_drawable_policy_for(world),
        &mut retirement,
    );
    callback(
        world,
        &mut ScriptPresentationEffects {
            view: Some(&mut view),
            terrain: terrain
                .map(|terrain| terrain as &mut dyn crate::game_logic::NativeTerrainEffects),
            ..Default::default()
        },
    )
}

fn track_binding(
    client: &mut GameClient,
    id: ObjectId,
) -> gamelogic::object::draw::client_visual::NativeTerrainTrackBinding {
    let mut view = client.borrow_native_drawable_owner();
    let key = view.binding_key(EPOCH, id.0).unwrap();
    let mut bindings = view.terrain_track_bindings_for_retirement(key).unwrap();
    assert_eq!(
        bindings.len(),
        1,
        "actual retail TankDraw constructor must bind one authored marker"
    );
    bindings.pop().unwrap()
}
