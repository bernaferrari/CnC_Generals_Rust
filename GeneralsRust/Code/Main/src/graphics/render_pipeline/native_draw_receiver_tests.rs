//! Actual GameClient → receiving RenderPipeline lifecycle regression.
//! Synthetic immutable art; no rendered-match or whole-game parity claim.
use super::*;
use crate::assets::{W3dAnimChannel, W3dAnimation, W3dHierarchy, W3dPivot};
use crate::game_logic::{GameLogic, Object, ObjectId, Team, ThingTemplate};
use crate::presentation_frame::{PresentationFrame, UnitRenderInput};
use game_client::core::game_client::{GameClient, PresentationDrawableSync};

const ID: ObjectId = ObjectId(42);
const DRAW: &str = "Draw = W3DModelDraw First\n DefaultConditionState\n Model = CameraModel\n Animation = TREE.MOVE\n AnimationMode = LOOP\n End\n ConditionState = MOVING\n Model = CameraModel\n Animation = TREE.MOVE\n AnimationMode = LOOP\n End\nEnd\n";

fn template(draw: &str) -> ThingTemplate {
    let mut parser = crate::assets::IniParser::new();
    parser
        .parse_ini_content(
            &format!("Object NativeReceiver\n{draw}End\n"),
            "receiver.ini",
        )
        .unwrap();
    let definition = parser.get_definition("NativeReceiver").unwrap();
    let mut template = ThingTemplate::new("NativeReceiver");
    template.set_authored_draw_models(definition.model_name.as_deref(), &definition.draw_modules);
    template
}
fn admit(world: &mut GameLogic, draw: &str, x: f32) {
    let mut object = Object::new(template(draw), ID, Team::USA);
    object.set_position(Vec3::new(x, 5., 7.));
    world.add_object(object);
}
fn source(world: &mut GameLogic) -> UnitRenderInput {
    let frame = PresentationFrame::build_from_logic(world, 0);
    frame
        .native_resident_drawable_inputs()
        .into_iter()
        .find(|unit| unit.id == ID)
        .unwrap()
}
fn model() -> W3DModel {
    let mut model = W3DModel::new("CameraModel".into());
    model.hierarchy = Some(W3dHierarchy {
        name: "TREE".into(),
        pivot_fixups: vec![],
        pivots: vec![
            W3dPivot {
                name: "ROOT".into(),
                parent_idx: u32::MAX,
                translation: [0.; 3],
                euler_angles: [0.; 3],
                rotation: [0., 0., 0., 1.],
            },
            W3dPivot {
                name: "CAMERA".into(),
                parent_idx: 0,
                translation: [0.; 3],
                euler_angles: [0.; 3],
                rotation: [0., 0., 0., 1.],
            },
        ],
    });
    model.hlods.push(crate::assets::W3dHlod {
        version: 0x0001_0000,
        name: "CAMERA_MODEL".into(),
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
    model.animations.push(W3dAnimation {
        name: "MOVE".into(),
        hierarchy_name: "TREE".into(),
        num_frames: 4,
        frame_rate: 1,
        source_is_compressed: false,
        raw_visibility_channels: vec![],
        unsupported_visibility_pivots: vec![],
        channels: vec![W3dAnimChannel {
            first_frame: 0,
            last_frame: 3,
            vector_len: 1,
            flags: 0,
            pivot: 1,
            data: vec![0., 10., 20., 30.],
        }],
    });
    model
}
// Only fixture payload construction. Admission and all mutable lifecycle state
// go through actual GameClient and the production native receiver methods.
fn entry(unit: &UnitRenderInput) -> PresentationDrawableSync {
    PresentationDrawableSync {
        receiving_science_player: None,
        object_id: ID.0,
        host_epoch: 19,
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
        module_admission:
            game_client::core::game_client::PresentationModuleAdmission::Compatibility,
        draw_logic_frame: 0,
        draw_indicator_color: [0, 0, 0],
        draw_physics: Default::default(),
        draw_velocity_magnitude_squared: 0.0,
        draw_module_names: Arc::new(vec!["W3DModelDraw".into()]),
        health_box_width: 0.,
        health_box_z_offset: 0.,
    }
}
fn sync(
    client: &mut GameClient,
    pipeline: &mut RenderPipeline,
    graphics: &mut GraphicsSystem,
    unit: &UnitRenderInput,
    sync_ms: u32,
) {
    pipeline.observe_animation_sync(sync_ms);
    client
        .sync_presentation_drawables_observed([entry(unit)], sync_ms, |event| {
            pipeline.receive_native_drawable_mutation(event, Some(unit), graphics);
        })
        .unwrap();
}
fn eye(
    pipeline: &mut RenderPipeline,
    graphics: &mut GraphicsSystem,
    unit: &UnitRenderInput,
    bone: &str,
) -> Result<Vec3, NativeCameraPoseUnavailable> {
    let first = unit.first_object_draw().unwrap().unwrap();
    pipeline.consume_native_camera_conditions(ID, graphics);
    pipeline
        .native_installed_camera_pose(ID, first.module_index, bone)
        .map(|matrix| matrix.w_axis.truncate())
}
#[test]
#[ignore = "requires native WGPU; run explicit isolated receiver acceptance"]
fn real_receiver_owns_constructor_dirty_setters_and_peer_reset_lifetimes_before_draw() {
    ww3d_engine::init_headless_blocking(ww3d_engine::EngineConfig {
        width: 32,
        height: 32,
        ..Default::default()
    })
    .expect("native receiver acceptance requires a headless device");
    struct EngineScope;
    impl Drop for EngineScope {
        fn drop(&mut self) {
            let _ = ww3d_engine::shutdown();
        }
    }
    let _scope = EngineScope;
    let mut graphics = GraphicsSystem::new(
        ww3d_engine::device().unwrap(),
        ww3d_engine::queue().unwrap(),
        ww3d_engine::color_format().unwrap(),
        ww3d_engine::depth_format().unwrap(),
    )
    .unwrap();
    // Production canonicalization preserves authored case; the cache is an
    // exact-key map, so lowercase-only fixture insertion is a missing asset.
    graphics.cache_model(
        crate::assets::mesh_asset_resolve::canonical_model_key("CameraModel"),
        model(),
    );
    let mut pipeline = RenderPipeline::initialize(&graphics).unwrap();
    let mut peer_pipeline = RenderPipeline::initialize(&graphics).unwrap();
    let mut client = GameClient::new().unwrap();
    let mut peer_client = GameClient::new().unwrap();
    let mut a = GameLogic::new();
    admit(&mut a, DRAW, 3.);
    let mut b = GameLogic::new();
    admit(&mut b, DRAW, 103.);
    let initial = source(&mut a);
    let selected = initial.first_object_draw().unwrap().unwrap();
    let asset = graphics
        .get_model(&crate::assets::mesh_asset_resolve::canonical_model_key(
            &selected.model_key,
        ))
        .expect("synthetic art must be admitted through the exact production model key");
    assert!(
        asset
            .local_animation_binding_for_draw_identity(&selected.animations[0].name)
            .is_some()
    );
    assert_eq!(initial.mesh_scale, 1.);
    sync(&mut client, &mut pipeline, &mut graphics, &initial, 0);
    // Constructor installed its HAnim before any collect/Draw invocation.
    assert_eq!(
        eye(&mut pipeline, &mut graphics, &initial, "CAMERA").unwrap(),
        Vec3::new(3., 5., 7.)
    );
    pipeline.observe_animation_sync(1000);
    // A valid hierarchy is not invalidated merely by observing a newer Sync.
    assert_eq!(
        eye(&mut pipeline, &mut graphics, &initial, "CAMERA")
            .unwrap()
            .x,
        3.
    );
    a.objects
        .get_mut(&ID)
        .unwrap()
        .set_position(Vec3::new(3., 5., 7.));
    let moved = source(&mut a);
    sync(&mut client, &mut pipeline, &mut graphics, &moved, 1000);
    assert_eq!(
        eye(&mut pipeline, &mut graphics, &moved, "CAMERA")
            .unwrap()
            .x,
        13.
    );
    let peer = source(&mut b);
    sync(
        &mut peer_client,
        &mut peer_pipeline,
        &mut graphics,
        &peer,
        2000,
    );
    assert_eq!(
        eye(&mut peer_pipeline, &mut graphics, &peer, "CAMERA")
            .unwrap()
            .x,
        103.
    );
    // Same frozen setter fact repeated through real mirror sync cannot advance A.
    sync(&mut client, &mut pipeline, &mut graphics, &moved, 2000);
    assert_eq!(
        eye(&mut pipeline, &mut graphics, &moved, "CAMERA")
            .unwrap()
            .x,
        13.
    );
    let visual_world = gamelogic::helpers::ClientVisualHandle::new(Arc::clone(&a.world_services));
    let saved = pipeline.capture_client_drawable_snapshot(&visual_world);
    assert_eq!(saved.drawables.len(), 1);
    assert_eq!(saved.drawables[0].animation.as_ref().unwrap().frame, 1.);
    a.objects.get_mut(&ID).unwrap().status.moving = true;
    a.objects
        .get_mut(&ID)
        .unwrap()
        .refresh_model_condition_bits();
    let dirty = source(&mut a);
    sync(&mut client, &mut pipeline, &mut graphics, &dirty, 2000);
    assert!(
        pipeline.drawable_visual_states[&(42, 0)]
            .native_pending_condition
            .is_some()
    );
    assert_eq!(
        eye(&mut pipeline, &mut graphics, &dirty, "CAMERA")
            .unwrap()
            .x,
        3.
    );
    assert!(
        pipeline.drawable_visual_states[&(42, 0)]
            .native_pending_condition
            .is_none()
    );
    assert_eq!(
        eye(&mut pipeline, &mut graphics, &dirty, "MISSING").unwrap(),
        Vec3::ZERO
    );
    // Failed selection of a later interface must invalidate the old first pose,
    // even though first_object_draw still succeeds for that first module.
    let invalid = format!(
        "{DRAW}Draw = W3DModelDraw BadLater\n DefaultConditionState\n Model = CameraModel\n End\n ConditionState = NOT_A_MODEL_FLAG\n Model = CameraModel\n End\nEnd\n"
    );
    let invalid_template = template(&invalid);
    *a.objects.get_mut(&ID).unwrap().template_mut() = invalid_template;
    let invalid_source = source(&mut a);
    assert!(invalid_source.first_object_draw().is_ok());
    sync(
        &mut client,
        &mut pipeline,
        &mut graphics,
        &invalid_source,
        2500,
    );
    assert!(
        eye(&mut pipeline, &mut graphics, &invalid_source, "CAMERA").is_err(),
        "failed dirty flush cannot reuse old Ready pose"
    );
    *a.objects.get_mut(&ID).unwrap().template_mut() = template(DRAW);
    let recovered = source(&mut a);
    assert_eq!(
        recovered.model_condition_bits,
        invalid_source.model_condition_bits
    );
    sync(&mut client, &mut pipeline, &mut graphics, &recovered, 2500);
    assert_eq!(
        eye(&mut pipeline, &mut graphics, &recovered, "CAMERA")
            .unwrap()
            .x,
        3.,
        "same bits and selected first identity must retry after rejected sibling admission"
    );
    // Reset/re-admission changes the frozen world/object lease even with same
    // mirror host epoch, numeric ObjectId, visual name, and pose revision.
    a.reset();
    admit(&mut a, DRAW, 203.);
    // A resident passenger is excluded by the visible mesh input query, but
    // its real Drawable still owns the saved HAnim and must serve a camera.
    a.add_object(Object::new(template(DRAW), ObjectId(99), Team::USA));
    a.objects.get_mut(&ID).unwrap().contained_by = Some(ObjectId(99));
    let frame = Rc::new(PresentationFrame::build_from_logic(&mut a, 0));
    assert!(!frame.unit_render_inputs().iter().any(|unit| unit.id == ID));
    let reset = frame
        .native_resident_drawable_inputs()
        .into_iter()
        .find(|unit| unit.id == ID)
        .unwrap();
    pipeline.set_presentation_frame(Some(frame));
    pipeline.queue_client_drawable_restore(saved);
    sync(&mut client, &mut pipeline, &mut graphics, &reset, 3000);
    assert_eq!(
        eye(&mut pipeline, &mut graphics, &reset, "CAMERA")
            .unwrap()
            .x,
        213.,
        "first post-load camera query must observe saved frame1, not constructor frame0"
    );
    assert_eq!(
        eye(&mut peer_pipeline, &mut graphics, &peer, "CAMERA")
            .unwrap()
            .x,
        103.
    );
    // Exact first interface Model=None cannot fall through to visible module2.
    let none = "Draw = W3DModelDraw First\n DefaultConditionState\n Model = NONE\n End\nEnd\nDraw = W3DModelDraw VisibleLater\n DefaultConditionState\n Model = CameraModel\n End\nEnd\n";
    a.reset();
    admit(&mut a, none, 303.);
    let absent = source(&mut a);
    sync(&mut client, &mut pipeline, &mut graphics, &absent, 4000);
    assert_eq!(
        eye(&mut pipeline, &mut graphics, &absent, "CAMERA"),
        Err(NativeCameraPoseUnavailable::NoRenderObject)
    );
    assert!(matches!(
        pipeline.drawable_visual_states[&(42, 1)].installed_draw_pose,
        installed_draw_pose::InstalledDrawObject::Ready(_)
    ));
}
