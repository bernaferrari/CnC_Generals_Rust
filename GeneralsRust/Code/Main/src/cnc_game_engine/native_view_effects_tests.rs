//! Actual Client sync/key allocation and the strict production view backend.
//! Native WGPU resources, synthetic immutable art; no rendered-match claim.
use super::*;
use crate::game_logic::{GameLogic, Object, ObjectId, Team, ThingTemplate};
use crate::presentation_frame::PresentationFrame;
use game_client::core::game_client::{GameClient, PresentationDrawableSync};
const ID: ObjectId = ObjectId(42);
const EPOCH: u64 = 19;
fn admit(world: &mut GameLogic, x: f32) {
    let mut parser = crate::assets::IniParser::new();
    parser.parse_ini_content("Object StrictCameraOwner\n Draw = W3DModelDraw DrawTag\n DefaultConditionState\n Model = CameraModel\n End\n ConditionState = MOVING\n Model = MovingModel\n End\n End\nEnd\n","strict_camera.ini").unwrap();
    let definition = parser.get_definition("StrictCameraOwner").unwrap();
    let mut template = ThingTemplate::new("StrictCameraOwner");
    template.set_authored_draw_models(definition.model_name.as_deref(), &definition.draw_modules);
    let mut object = Object::new(template, ID, Team::USA);
    object.set_position(Vec3::new(x, 5., 7.));
    world.add_object(object);
}
fn model(name: &str, camera_x: f32) -> W3DModel {
    let mut model = W3DModel::new(name.into());
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
            pivot("CAMERA", 0, [camera_x, 0., 0.]),
        ],
    });
    model.hlods.push(crate::assets::W3dHlod {
        version: 0x10000,
        name: name.into(),
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
fn frame_source(world: &mut GameLogic) -> crate::presentation_frame::UnitRenderInput {
    PresentationFrame::build_from_logic(world, 0)
        .native_resident_drawable_inputs()
        .into_iter()
        .find(|source| source.id == ID)
        .unwrap()
}
// Immutable synthetic host payload only; all constructor/bind/model/key/query
// behavior below uses the actual production Client and renderer operations.
fn entry(unit: &crate::presentation_frame::UnitRenderInput) -> PresentationDrawableSync {
    PresentationDrawableSync {
        object_id: ID.0,
        host_epoch: EPOCH,
        resident: true,
        visual_template_name: unit.template_name.clone(),
        template_name: unit.template_name.clone(),
        position: unit.position.to_array(),
        orientation: 0.,
        float_yaw: 0.,
        float_pitch: 0.,
        destroyed: false,
        model_condition_bits: unit.model_condition_bits,
        body_damage_state: 0,
        kind_names: Arc::new(vec![]),
        team_color: [1.; 4],
        effectively_stealthed: false,
        scene_hidden_by_stealth: false,
        health_current: 100.,
        health_max: 100.,
        selected: false,
        veterancy_level: 0,
        under_construction: false,
        construction_percent: 0.,
        sold: false,
        ammo_pip_total: 0,
        ammo_pip_full: 0,
        occupant_count: 0,
        max_garrison: 0,
        disabled: false,
        is_carbomb: false,
        bomb_type: 0,
        bomb_timer_seconds: 0,
        weapon_bonus_enthusiastic: false,
        show_healing: false,
        healing_icon_type: 0,
        garrisoned_ids: vec![],
        emoticon_name: String::new(),
        emoticon_frames_left: 0,
        formation_id: 0,
        caption: String::new(),
        draw_module_names: Arc::new(vec!["W3DModelDraw".into()]),
        health_box_width: 0.,
        health_box_z_offset: 0.,
    }
}
fn sync(
    client: &mut GameClient,
    pipeline: &mut RenderPipeline,
    graphics: &mut GraphicsSystem,
    source: &crate::presentation_frame::UnitRenderInput,
) {
    client
        .sync_presentation_drawables_observed([entry(source)], 0, |event| {
            pipeline.receive_native_drawable_mutation(event, Some(source), graphics);
        })
        .unwrap();
    let key = client
        .presentation_direct_drawable_state(EPOCH, ID.0)
        .unwrap()
        .binding_key;
    pipeline
        .admit_native_drawable_binding(source.draw_playback_identity.as_ref().unwrap(), key)
        .unwrap();
}
fn sample(
    client: &mut GameClient,
    pipeline: &mut RenderPipeline,
    graphics: &mut GraphicsSystem,
    source: &NativeSlaveSource,
) -> SlaveCameraSample {
    let (_, _, presence) = client.split_frame_snow_and_drawable_presence();
    NativeRendererViewEffects::new(
        pipeline.borrow_native_installed_poses(),
        graphics,
        presence,
        EPOCH,
        0,
    )
    .sample_slave(source, "CAMERA")
}
fn pose(sample: SlaveCameraSample) -> Mat4 {
    match sample {
        SlaveCameraSample::Pose(pose) => pose,
        _ => panic!("strict current owner pose unavailable"),
    }
}
#[test]
#[ignore = "requires native WGPU; run explicit isolated strict-backend acceptance"]
fn actual_view_backend_uses_current_dirty_source_and_rejects_peer_or_unobserved_client_replacement()
{
    ww3d_engine::init_headless_blocking(ww3d_engine::EngineConfig {
        width: 32,
        height: 32,
        ..Default::default()
    })
    .unwrap();
    struct Scope;
    impl Drop for Scope {
        fn drop(&mut self) {
            let _ = ww3d_engine::shutdown();
        }
    }
    let _scope = Scope;
    let mut graphics = GraphicsSystem::new(
        ww3d_engine::device().unwrap(),
        ww3d_engine::queue().unwrap(),
        ww3d_engine::color_format().unwrap(),
        ww3d_engine::depth_format().unwrap(),
    )
    .unwrap();
    for (name, x) in [("CameraModel", 0.), ("MovingModel", 10.)] {
        graphics.cache_model(
            crate::assets::mesh_asset_resolve::canonical_model_key(name),
            model(name, x),
        );
    }
    let mut pipeline = RenderPipeline::initialize(&graphics).unwrap();
    let mut client = GameClient::new().unwrap();
    let mut world = GameLogic::new();
    admit(&mut world, 3.);
    let initial = frame_source(&mut world);
    sync(&mut client, &mut pipeline, &mut graphics, &initial);
    let current = world.current_native_slave_source(ID, 0).unwrap();
    assert_eq!(
        pose(sample(&mut client, &mut pipeline, &mut graphics, &current))
            .w_axis
            .truncate(),
        Vec3::new(3., 5., 7.)
    );
    // No display sync or new PresentationFrame is allowed between these host
    // setters and the actual script-time dirty/bone observation.
    let object = world.objects.get_mut(&ID).unwrap();
    object.set_position(Vec3::new(43., 17., 29.));
    object.status.moving = true;
    object.refresh_model_condition_bits();
    let fresh = world.current_native_slave_source(ID, 0).unwrap();
    assert_ne!(fresh.condition_bits, current.condition_bits);
    assert_eq!(
        pose(sample(&mut client, &mut pipeline, &mut graphics, &fresh))
            .w_axis
            .truncate(),
        Vec3::new(53., 17., 29.)
    );
    let mut peer = GameLogic::new();
    admit(&mut peer, 303.);
    let peer_source = peer.current_native_slave_source(ID, 0).unwrap();
    assert!(matches!(
        sample(&mut client, &mut pipeline, &mut graphics, &peer_source),
        SlaveCameraSample::Unavailable(_)
    ));
    assert_eq!(
        pose(sample(&mut client, &mut pipeline, &mut graphics, &fresh))
            .w_axis
            .truncate(),
        Vec3::new(53., 17., 29.)
    );
    // Real Client destroy/recreate WITHOUT the renderer observer must not let
    // a same-object/same-template association adopt its predecessor's RenderObj.
    let old = client
        .presentation_direct_drawable_state(EPOCH, ID.0)
        .unwrap()
        .binding_key;
    client.destroy_drawable(old.drawable_id).unwrap();
    client
        .sync_presentation_drawables([entry(&initial)])
        .unwrap();
    let replacement = client
        .presentation_direct_drawable_state(EPOCH, ID.0)
        .unwrap()
        .binding_key;
    assert_ne!(old, replacement);
    assert!(matches!(
        sample(&mut client, &mut pipeline, &mut graphics, &fresh),
        SlaveCameraSample::Unavailable(_)
    ));
    assert!(
        pipeline
            .admit_native_drawable_binding(&fresh.identity, replacement)
            .is_err()
    );
    // Only an actual new constructor observation may install that lifetime.
    client.destroy_drawable(replacement.drawable_id).unwrap();
    let recreated = frame_source(&mut world);
    sync(&mut client, &mut pipeline, &mut graphics, &recreated);
    assert_eq!(
        pose(sample(&mut client, &mut pipeline, &mut graphics, &fresh))
            .w_axis
            .truncate(),
        Vec3::new(53., 17., 29.)
    );
}
