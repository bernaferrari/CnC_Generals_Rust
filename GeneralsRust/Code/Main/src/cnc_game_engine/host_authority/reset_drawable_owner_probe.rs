//! Actual receiving GameClient/renderer/GameLogic reset phases, avoiding the
//! unrelated application asset-indexing work of constructing a full engine.
use super::*;
use crate::cnc_game_engine::native_view_effects::NativeRendererViewEffects;
use crate::game_logic::{GameLogic, ScriptPresentationEffects, Team};

const EPOCH: u64 = 71;

fn camera_model() -> crate::assets::W3DModel {
    let mut model = crate::assets::W3DModel::new("CameraModel".into());
    let pivot = |name: &str, parent_idx, translation| crate::assets::W3dPivot {
        name: name.into(),
        parent_idx,
        translation,
        euler_angles: [0.; 3],
        rotation: [0., 0., 0., 1.],
    };
    model.hierarchy = Some(crate::assets::W3dHierarchy {
        name: "TREE".into(),
        pivot_fixups: vec![],
        pivots: vec![
            pivot("ROOT", u32::MAX, [0.; 3]),
            pivot("CAMERA", 0, [0.; 3]),
        ],
    });
    model.hlods.push(crate::assets::W3dHlod {
        version: 0x10000,
        name: "CameraModel".into(),
        hierarchy_name: "TREE".into(),
        lods: vec![crate::assets::W3dHlodLod {
            max_screen_size: f32::MAX,
            subobjects: vec![],
        }],
        aggregates: None,
        proxies: None,
        has_unrendered_aggregates: false,
        has_invalid_trailing_records: false,
    });
    model
}

fn reset_probe_world() -> anyhow::Result<GameLogic> {
    let mut world = GameLogic::new();
    world
        .admit_receiving_module_content(crate::assets::ReceivingObjectContent::from_sources([(
            "reset-drawable-owner-probe.ini".into(),
            "Object ResetDrawableOwnerProbe\nKindOf = VEHICLE\nBody = ActiveBody BodyTag\nMaxHealth = 40\nInitialHealth = 40\nEnd\nBehavior = DestroyDie DeleteTag\nEnd\nDraw = W3DModelDraw DrawTag\nDefaultConditionState\nModel = CameraModel\nEnd\nEnd\nEnd\n".into(),
        )])
        .map_err(anyhow::Error::msg)?)
        .map_err(anyhow::Error::msg)?;
    world.require_native_drawable_owner();
    Ok(world)
}

fn with_receiving_view<R>(
    client: &mut game_client::core::game_client::GameClient,
    pipeline: &mut crate::graphics::RenderPipeline,
    graphics: &mut crate::graphics::GraphicsSystem,
    run: impl FnOnce(&mut NativeRendererViewEffects<'_>) -> R,
) -> R {
    let (_, _, drawables) = client.split_frame_snow_and_drawable_owner();
    let mut retirement = game_client::core::game_client::NativeParticleRetirementService;
    let mut view = NativeRendererViewEffects::new(
        pipeline.borrow_native_installed_poses(),
        graphics,
        drawables,
        EPOCH,
        0,
        crate::cnc_game_engine::native_view_effects::receiving_native_drawable_policy(),
        &mut retirement,
    );
    run(&mut view)
}

pub(crate) fn run() -> anyhow::Result<()> {
    let private_root = std::env::var_os("GENERALS_RESET_DRAWABLE_PROBE_PRIVATE_ROOT")
        .ok_or_else(|| anyhow::anyhow!("requires private launch root"))?;
    std::fs::create_dir_all(std::path::PathBuf::from(private_root).join("userdata"))?;
    let content_root = std::env::var_os("GENERALS_RESET_DRAWABLE_PROBE_CONTENT_ROOT")
        .ok_or_else(|| anyhow::anyhow!("requires installed game content root"))?;
    std::env::set_current_dir(content_root)?;

    ww3d_engine::init_headless_blocking(ww3d_engine::EngineConfig {
        width: 32,
        height: 32,
        ..Default::default()
    })?;
    eprintln!("reset probe: WGPU initialized");
    struct Scope;
    impl Drop for Scope {
        fn drop(&mut self) {
            let _ = ww3d_engine::shutdown();
        }
    }
    let _scope = Scope;
    let mut graphics = crate::graphics::GraphicsSystem::new(
        ww3d_engine::device()?,
        ww3d_engine::queue()?,
        ww3d_engine::color_format()?,
        ww3d_engine::depth_format()?,
    )?;
    graphics.cache_model(
        crate::assets::mesh_asset_resolve::canonical_model_key("CameraModel"),
        camera_model(),
    );
    eprintln!("reset probe: graphics owner constructed");
    let mut pipeline = crate::graphics::RenderPipeline::initialize(&graphics)?;
    let mut peer_pipeline = crate::graphics::RenderPipeline::initialize(&graphics)?;
    eprintln!("reset probe: render pipelines constructed");
    let mut client = game_client::core::game_client::GameClient::new()?;
    let mut peer_client = game_client::core::game_client::GameClient::new()?;
    // This fixture declares no particle systems. Admit an empty per-client
    // catalog explicitly so the probe needs no global retail content loader.
    client.particle_systems_mut().finish_definition_admission();
    peer_client
        .particle_systems_mut()
        .finish_definition_admission();
    eprintln!("reset probe: GameClients constructed");
    let mut world = reset_probe_world()?;
    let mut peer = reset_probe_world()?;
    eprintln!("reset probe: GameLogic worlds admitted");
    eprintln!(
        "reset probe: ensure template: {}",
        world.ensure_host_spawn_template("ResetDrawableOwnerProbe")
    );
    world
        .admit_object_client_template("ResetDrawableOwnerProbe")
        .map_err(anyhow::Error::msg)?;
    eprintln!("reset probe: receiving object template admitted");
    let id = with_receiving_view(&mut client, &mut pipeline, &mut graphics, |view| {
        world
            .create_object_with_script_effects_checked(
                "ResetDrawableOwnerProbe",
                Team::USA,
                glam::Vec3::ZERO,
                None,
                &mut ScriptPresentationEffects {
                    view: Some(view),
                    ..Default::default()
                },
            )
            .map_err(anyhow::Error::msg)
    })?
    .ok_or_else(|| anyhow::anyhow!("actual receiving object creation failed"))?;
    let peer_id = with_receiving_view(
        &mut peer_client,
        &mut peer_pipeline,
        &mut graphics,
        |view| {
            peer.create_object_with_script_effects_checked(
                "ResetDrawableOwnerProbe",
                Team::USA,
                glam::Vec3::new(303., 5., 7.),
                None,
                &mut ScriptPresentationEffects {
                    view: Some(view),
                    ..Default::default()
                },
            )
            .map_err(anyhow::Error::msg)
        },
    )?
    .ok_or_else(|| anyhow::anyhow!("peer receiving object creation failed"))?;
    eprintln!("reset probe: both actual objects created");
    assert_eq!(id, peer_id, "same numeric IDs are session-local");
    assert!(
        pipeline
            .borrow_native_installed_poses()
            .has_drawable_lifetime(id.0)
    );
    assert!(
        peer_pipeline
            .borrow_native_installed_poses()
            .has_drawable_lifetime(peer_id.0)
    );

    // Mirrors start_game.rs / host_game_engine_reset ordering: complete the
    // real receiving Client reset; retire this pipeline's renderer receipts;
    // clear the host visual bridge; then invoke checked GameLogic teardown.
    client.reset()?;
    let (_, _, client_drawables) = client.split_frame_snow_and_drawable_owner();
    pipeline
        .retire_native_drawables_after_client_reset(&client_drawables)
        .map_err(anyhow::Error::msg)?;
    gamelogic::helpers::ClientVisualHandle::new(std::sync::Arc::clone(&world.world_services))
        .clear_visual_state_for_reset();
    with_receiving_view(&mut client, &mut pipeline, &mut graphics, |view| {
        world.reset_with_effects_checked(&mut ScriptPresentationEffects {
            view: Some(view),
            ..Default::default()
        })
    })
    .map_err(anyhow::Error::msg)?;
    assert!(world.objects.registered_is_empty());
    assert!(
        !client
            .borrow_native_drawable_owner()
            .has_object_drawable(id.0)
    );
    assert!(
        !pipeline
            .borrow_native_installed_poses()
            .has_drawable_lifetime(id.0)
    );
    assert!(peer.host_object(peer_id).is_some());
    assert!(
        peer_client
            .borrow_native_drawable_owner()
            .has_object_drawable(peer_id.0)
    );
    assert!(
        peer_pipeline
            .borrow_native_installed_poses()
            .has_drawable_lifetime(peer_id.0)
    );
    println!(
        "PASS native reset receiver: Client -> renderer receipt -> host visual bridge -> checked GameLogic; peer preserved"
    );
    Ok(())
}
