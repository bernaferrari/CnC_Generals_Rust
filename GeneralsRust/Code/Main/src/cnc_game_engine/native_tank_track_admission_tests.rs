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

struct RetailTrackUnit {
    template: &'static str,
    team: Team,
    model: &'static str,
    expansion_model: bool,
    width: f32,
    dds: Option<&'static str>,
}

const BATTLEMASTER: RetailTrackUnit = RetailTrackUnit {
    template: "ChinaTankBattleMaster",
    team: Team::China,
    model: "NVBtMstr",
    expansion_model: false,
    width: 18.573_772,
    dds: None,
};

const DOZER: RetailTrackUnit = RetailTrackUnit {
    template: "AmericaVehicleDozer",
    team: Team::USA,
    model: "AVCONSTDOZ_A",
    expansion_model: false,
    width: 16.823_484,
    dds: Some("Art/Textures/extiretrack2.dds"),
};

const QUAD_CANNON: RetailTrackUnit = RetailTrackUnit {
    template: "GLAVehicleQuadCannon",
    team: Team::GLA,
    model: "UVQuadCann",
    expansion_model: false,
    width: 16.734_961,
    dds: None,
};

#[test]
#[ignore = "requires mounted retail sources and native WGPU"]
fn retail_tank_factory_binds_before_placement_and_client_reset_retires_own_marks() {
    exercise_retail_track(BATTLEMASTER, TrackExercise::Reset);
}

#[test]
#[ignore = "requires mounted retail sources and native WGPU"]
fn retail_disguise_replacement_detaches_only_the_old_drawable_track() {
    exercise_retail_track(BATTLEMASTER, TrackExercise::Disguise);
}

#[test]
#[ignore = "requires mounted retail sources and native WGPU"]
fn retail_tank_registered_yaw_updates_and_caps_its_track_without_translation() {
    exercise_retail_track(BATTLEMASTER, TrackExercise::TurnInPlace);
}

#[test]
#[ignore = "requires mounted retail sources and native WGPU"]
fn retail_quad_cannon_entering_tunnel_caps_its_track_before_redeployment() {
    exercise_retail_track(QUAD_CANNON, TrackExercise::TunnelEnter);
}

#[test]
#[ignore = "requires mounted retail sources and native WGPU"]
fn retail_truck_dozer_binds_authored_track_and_loads_original_dds() {
    exercise_retail_track(DOZER, TrackExercise::Reset);
}

#[test]
#[ignore = "requires mounted retail sources and native WGPU"]
fn retail_wrapper_tracks_overlord_tank() {
    exercise_retail_track(
        RetailTrackUnit {
            template: "ChinaTankOverlord",
            team: Team::China,
            model: "NVOvrlrd",
            expansion_model: false,
            width: 26.579_417,
            dds: None,
        },
        TrackExercise::Reset,
    );
}

#[test]
#[ignore = "requires mounted retail sources and native WGPU"]
fn retail_wrapper_tracks_overlord_truck() {
    exercise_retail_track(
        RetailTrackUnit {
            template: "AmericaTankAvenger",
            team: Team::USA,
            model: "AVAVNGER",
            expansion_model: true,
            width: 16.883_705,
            dds: None,
        },
        TrackExercise::Reset,
    );
}

#[test]
#[ignore = "requires mounted retail sources and native WGPU"]
fn retail_wrapper_tracks_tank_truck() {
    exercise_retail_track(QUAD_CANNON, TrackExercise::Reset);
}

enum TrackExercise {
    Reset,
    Disguise,
    TurnInPlace,
    TunnelEnter,
}

fn exercise_retail_track(unit: RetailTrackUnit, exercise: TrackExercise) {
    let install = std::path::PathBuf::from(std::env::var_os("GENERALS_BASE_DIR").unwrap());
    let model_archive = if unit.expansion_model {
        std::path::PathBuf::from(std::env::var_os("GENERALS_ASSETS_DIR").unwrap())
            .join("Data/W3DZH.big")
    } else {
        install.join("W3D.big")
    };
    let mut archive = ArchiveFileSystem::new();
    archive
        .open_archive_file(model_archive.to_str().unwrap())
        .unwrap();
    let mut code = Vec::new();
    archive
        .open_file(&format!("Art/W3D/{}.w3d", unit.model), 0)
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
        crate::assets::mesh_asset_resolve::canonical_model_key(unit.model),
        W3DLoader::new()
            .load_model_from_bytes(&code, unit.model)
            .unwrap(),
    );
    let definitions = RetailWorldDefinitions::load().unwrap();
    let mut make_world = |position: Vec3| {
        let mut world = GameLogic::new();
        world
            .with_own_services_mut(|world| definitions.admit(world))
            .unwrap();
        world.add_player(Player::new(0, unit.team, "Track Owner", true));
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
                        unit.template,
                        unit.team,
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

    if matches!(exercise, TrackExercise::TurnInPlace) {
        let terrain = render.native_terrain().unwrap();
        let track = terrain.terrain_tracks().track(receipt.slot()).unwrap();
        assert!(track.have_anchor() && track.have_cap());
        assert_eq!(track.active_edge_count(), 0);
        with_view(
            &mut client,
            &mut render,
            &mut graphics,
            &mut world,
            |world, effects| rotate_registered_tank_in_place(world, id, effects),
        );
        let terrain = render.native_terrain().unwrap();
        let track = terrain.terrain_tracks().track(receipt.slot()).unwrap();
        assert!(
            track.have_anchor() && !track.have_cap(),
            "a registered zero-pivot yaw revisits the original anchor without appending an edge"
        );
        assert_eq!(track.active_edge_count(), 0);
    }

    let placements: &[f32] = if matches!(
        exercise,
        TrackExercise::TurnInPlace | TrackExercise::TunnelEnter
    ) {
        &[45.0, 70.0, 95.0]
    } else {
        &[45.0, 70.0]
    };
    with_view(
        &mut client,
        &mut render,
        &mut graphics,
        &mut world,
        |world, effects| {
            for &x in placements {
                assert!(world.set_registered_object_position_with_effects(
                    id,
                    Vec3::new(x, 0.0, 20.0),
                    4,
                    effects,
                ));
            }
        },
    );
    let edges_before = {
        let terrain = render.native_terrain().unwrap();
        let tracks = terrain.terrain_tracks();
        let edges = tracks
            .track(receipt.slot())
            .unwrap()
            .active_edges(tracks.config().max_tank_track_edges);
        assert_eq!(
            edges.len(),
            placements.len(),
            "factory placement supplied the first anchor"
        );
        for (edge, &x) in edges.iter().zip(placements) {
            assert_eq!(edge.time_added, 1234);
            assert_eq!(edge.endpoint_pos[0].x, x);
            assert!(
                ((edge.endpoint_pos[0] - edge.endpoint_pos[1]).length() - unit.width).abs() < 1e-5
            );
        }
        edges
    };
    if let Some(dds) = unit.dds {
        let mut textures = ArchiveFileSystem::new();
        textures
            .open_archive_file(install.join("Textures.big").to_str().unwrap())
            .unwrap();
        let (device, queue) = {
            let engine = engine.lock();
            (engine.device(), engine.queue())
        };
        let terrain = render.native_terrain_mut().unwrap();
        terrain.init_gpu_resources(device, queue).unwrap();
        let mut requested = Vec::new();
        terrain.prepare_tank_track_draws(|path| {
            requested.push(path.to_owned());
            let mut bytes = Vec::new();
            textures
                .open_file(path, 0)
                .ok()?
                .read_to_end(&mut bytes)
                .ok()?;
            assert_eq!(&bytes[..4], b"DDS ");
            Some(bytes)
        });
        assert_eq!(
            requested,
            [dds],
            "the original DDS uploads without a TGA fallback"
        );
        terrain.prepare_tank_track_draws(|_| panic!("the resident retail tread was reopened"));
    }
    match exercise {
        TrackExercise::Disguise => {
            replace_with_retail_disguise(&mut world, &mut client, &mut render, &mut graphics, id);
        }
        TrackExercise::Reset => {
            assert!(
                render
                    .retire_native_drawables_after_client_reset(
                        &client.borrow_native_drawable_owner()
                    )
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
        }
        TrackExercise::TurnInPlace => {
            assert_eq!(edges_before.last().unwrap().alpha, 1.0);
            world
                .objects
                .get_mut(&id)
                .unwrap()
                .set_status_stealthed(true);
            with_view(
                &mut client,
                &mut render,
                &mut graphics,
                &mut world,
                |world, effects| rotate_registered_tank_in_place(world, id, effects),
            );
            let terrain = render.native_terrain().unwrap();
            let tracks = terrain.terrain_tracks();
            let track = tracks.track(receipt.slot()).unwrap();
            assert!(!track.have_anchor() && track.have_cap());
            let mut expected = edges_before;
            expected.last_mut().unwrap().alpha = 0.0;
            assert_eq!(
                track.active_edges(tracks.config().max_tank_track_edges),
                expected
            );
            let peer = peer_render
                .native_terrain()
                .unwrap()
                .terrain_tracks()
                .track(peer_receipt.slot())
                .unwrap();
            assert!(peer.have_anchor() && peer.have_cap());
            assert_eq!(peer.active_edge_count(), 0);
            return;
        }
        TrackExercise::TunnelEnter => {
            assert_eq!(edges_before.last().unwrap().alpha, 1.0);
            with_view(
                &mut client,
                &mut render,
                &mut graphics,
                &mut world,
                |world, effects| enter_retail_tunnel(world, id, effects),
            );
            let terrain = render.native_terrain().unwrap();
            let tracks = terrain.terrain_tracks();
            let track = tracks.track(receipt.slot()).unwrap();
            let mut expected = edges_before;
            expected.last_mut().unwrap().alpha = 0.0;
            assert_eq!(
                track.active_edges(tracks.config().max_tank_track_edges),
                expected,
                "enclosing OpenContain must cap the existing strip before redeploying the rider"
            );
            assert!(track.have_anchor() && track.have_cap());
            let peer = peer_render
                .native_terrain()
                .unwrap()
                .terrain_tracks()
                .track(peer_receipt.slot())
                .unwrap();
            assert!(peer.have_anchor() && peer.have_cap());
            assert_eq!(peer.active_edge_count(), 0);
            return;
        }
    }
    let terrain = render.native_terrain().unwrap();
    assert!(!terrain.receives_terrain_track(&receipt));
    let track = terrain.terrain_tracks().track(receipt.slot()).unwrap();
    assert!(!track.bound());
    assert_eq!(
        track.active_edges(terrain.terrain_tracks().config().max_tank_track_edges),
        edges_before,
    );
    if matches!(exercise, TrackExercise::Disguise) {
        let tracks = render.native_terrain_mut().unwrap().terrain_tracks_mut();
        let end = 1234 + tracks.config().max_tank_track_fade_delay;
        tracks.update(end);
        tracks.update(end + 1);
        assert_eq!(tracks.track(receipt.slot()).unwrap().active_edge_count(), 0);
    }
    assert!(
        peer_render
            .native_terrain()
            .unwrap()
            .receives_terrain_track(&peer_receipt)
    );
}

fn enter_retail_tunnel(
    world: &mut GameLogic,
    id: ObjectId,
    effects: &mut ScriptPresentationEffects<'_>,
) {
    use crate::command_system::{CommandType, GameCommand};

    world.with_own_services_mut(|world| {
        let tunnel = world
            .create_object_with_script_effects_checked(
                "GLATunnelNetwork",
                Team::GLA,
                Vec3::new(103.0, 0.0, 20.0),
                None,
                effects,
            )
            .unwrap()
            .unwrap();
        world.queue_command(GameCommand {
            command_type: CommandType::Enter { target_id: tunnel },
            player_id: 0,
            command_id: 1,
            timestamp: std::time::SystemTime::UNIX_EPOCH,
            selected_units: vec![id],
            modifier_keys: Default::default(),
        });
        world.process_commands_with_effects(effects);
        world
            .update_ai_with_effects(&[id, tunnel], 1.0 / 30.0, effects)
            .unwrap();
        let rider = world.objects.get(&id).unwrap();
        assert_eq!(rider.contained_by, Some(tunnel));
        assert!(rider.drawable_is_effectively_hidden());
        assert_eq!(rider.get_position(), Vec3::new(103.0, 0.0, 20.0));
        assert_eq!(world.tunnel_network_residual_enters(), 1);
    });
}

fn rotate_registered_tank_in_place(
    world: &mut GameLogic,
    id: ObjectId,
    effects: &mut ScriptPresentationEffects<'_>,
) {
    let object = world.objects.get_mut(&id).unwrap();
    // The original locomotor disables its pivot offset while braking.
    object.is_braking = true;
    let before = (
        object.get_position(),
        object.get_orientation(),
        object.native_pose_revision(),
    );
    let goal = before.0 - Vec3::Z * 100.0;
    world
        .rotate_owned_pivot_with_effects(id, goal, 0.25, 4, effects)
        .unwrap();
    let object = world.objects.get(&id).unwrap();
    assert_eq!(object.get_position(), before.0);
    assert!((object.get_orientation() - before.1 - 0.25).abs() < 1e-6);
    assert_eq!(object.native_pose_revision(), before.2 + 1);
}

fn replace_with_retail_disguise(
    world: &mut GameLogic,
    client: &mut GameClient,
    render: &mut RenderPipeline,
    graphics: &mut GraphicsSystem,
    id: ObjectId,
) {
    const DISGUISE: &str = "AmericaInfantryRanger";
    let old = client
        .borrow_native_drawable_owner()
        .binding_key(EPOCH, id.0)
        .unwrap();
    let mut entry = world
        .current_native_factory_drawable_source(id, EPOCH, 0)
        .unwrap()
        .drawable;
    let object = world.host_object_mut(id).unwrap();
    object.status.disguised = true;
    object.disguise_as_template = Some(DISGUISE.into());

    let frame = crate::presentation_frame::PresentationFrame::build_from_logic(world, 0);
    let replacement = frame
        .direct_host_drawables
        .iter()
        .find(|direct| direct.object.id == id)
        .unwrap();
    assert_eq!(replacement.visual_template_name, DISGUISE);
    entry.visual_template_name = replacement.visual_template_name.clone();
    entry.module_admission = replacement
        .receiving_client_template
        .as_ref()
        .unwrap()
        .client_admission();
    let source = frame
        .native_resident_drawable_inputs()
        .into_iter()
        .find(|input| input.id == id)
        .unwrap();
    let identity = source.draw_playback_identity.as_ref().unwrap();
    let policy = receiving_native_drawable_policy_for(world);
    let content = world.receiving_module_content.as_mut().unwrap();

    let (created, _, _) = client
        .sync_presentation_drawables_observed_with_content(
            [entry],
            1234,
            content.module_factory_mut(),
            policy,
            |event| {
                render.receive_native_drawable_mutation(event, Some(&source), graphics);
            },
        )
        .unwrap();
    assert_eq!(created, 1);
    let mut drawables = client.borrow_native_drawable_owner();
    let new = drawables.binding_key(EPOCH, id.0).unwrap();
    assert_ne!(old, new);
    render.admit_native_drawable_binding(identity, new).unwrap();
    assert!(
        drawables
            .terrain_track_bindings_for_retirement(new)
            .unwrap()
            .is_empty()
    );
    assert_eq!(
        world.host_object(id).unwrap().get_template().name,
        "ChinaTankBattleMaster"
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
        "actual retail Draw constructor must bind one authored marker"
    );
    bindings.pop().unwrap()
}
