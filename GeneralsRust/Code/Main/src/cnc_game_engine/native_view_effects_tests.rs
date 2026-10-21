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
        receiving_science_player: None,
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
    let (_, _, drawables) = client.split_frame_snow_and_drawable_owner();
    let mut retirement = game_client::core::game_client::NativeParticleRetirementService;
    NativeRendererViewEffects::new(
        pipeline.borrow_native_installed_poses(),
        graphics,
        drawables,
        EPOCH,
        0,
        receiving_native_drawable_policy(),
        &mut retirement,
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

struct ActualFactoryProbe<'a, 'b> {
    runtime: &'a mut NativeRendererViewEffects<'b>,
    births: Vec<u32>,
}
impl NativeViewEffects for ActualFactoryProbe<'_, '_> {
    fn local_player_id(&self) -> u32 {
        self.runtime.local_player_id()
    }
    fn host_epoch(&self) -> u64 {
        self.runtime.host_epoch()
    }
    fn sample_slave(&mut self, source: &NativeSlaveSource, bone: &str) -> SlaveCameraSample {
        self.runtime.sample_slave(source, bone)
    }
    fn admit_factory_drawable(
        &mut self,
        source: crate::game_logic::NativeFactoryDrawableSource,
        birth: game_client::core::game_client::NativeDrawableBirthContext,
        content: &mut game_engine::common::thing::module_factory::ModuleFactory,
    ) -> Result<(), String> {
        let object = source.drawable.object_id;
        assert!(matches!(
            &source.drawable.module_admission,
            game_client::core::game_client::PresentationModuleAdmission::Authored(_)
        ));
        assert_eq!(source.render.identity.object_id(), object);
        assert_eq!(
            source.drawable.position, [0.0; 3],
            "this template has no onCreate pose mutation; caller placement follows birth"
        );
        assert!(
            self.runtime
                .drawables
                .binding_key(self.runtime.host_epoch, object)
                .is_none(),
            "actual factory callback must precede receiving Object association"
        );
        self.runtime
            .admit_factory_drawable(source, birth, content)?;
        assert!(
            self.runtime
                .drawables
                .binding_key(self.runtime.host_epoch, object)
                .is_some()
        );
        self.births.push(object);
        Ok(())
    }
}

fn receiving_factory_world() -> GameLogic {
    let text = "Object StrictFactoryOwner\nKindOf = VEHICLE\nBody = ActiveBody BodyTag\nMaxHealth = 40\nInitialHealth = 40\nEnd\nDraw = W3DModelDraw DrawTag\nDefaultConditionState\nModel = CameraModel\nEnd\nEnd\nEnd\n";
    let mut world = GameLogic::new();
    world
        .admit_receiving_module_content(
            crate::assets::ReceivingObjectContent::from_sources([(
                "strict-factory.ini".to_owned(),
                text.to_owned(),
            )])
            .unwrap(),
        )
        .unwrap();
    world.require_native_drawable_owner();
    world
}

#[test]
#[ignore = "requires native WGPU; run explicit isolated strict-backend acceptance"]
fn actual_factory_birth_installs_receiving_drawable_before_immediate_query_and_isolates_same_id_peer()
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
    graphics.cache_model(
        crate::assets::mesh_asset_resolve::canonical_model_key("CameraModel"),
        model("CameraModel", 0.),
    );
    let mut pipeline = RenderPipeline::initialize(&graphics).unwrap();
    let mut peer_pipeline = RenderPipeline::initialize(&graphics).unwrap();
    let mut client = GameClient::new().unwrap();
    let mut peer_client = GameClient::new().unwrap();
    let mut world = receiving_factory_world();
    let mut peer = receiving_factory_world();
    let mut retirement = game_client::core::game_client::NativeParticleRetirementService;
    let policy = receiving_native_drawable_policy();
    let (id, binding) = {
        let (_, _, drawables) = client.split_frame_snow_and_drawable_owner();
        let mut runtime = NativeRendererViewEffects::new(
            pipeline.borrow_native_installed_poses(),
            &mut graphics,
            drawables,
            EPOCH,
            0,
            policy,
            &mut retirement,
        );
        let mut probe = ActualFactoryProbe {
            runtime: &mut runtime,
            births: Vec::new(),
        };
        let id = world
            .create_object_with_script_effects_checked(
                "StrictFactoryOwner",
                Team::USA,
                Vec3::new(3., 5., 7.),
                None,
                &mut crate::game_logic::ScriptPresentationEffects {
                    view: Some(&mut probe),
                    ..Default::default()
                },
            )
            .unwrap()
            .unwrap();
        assert_eq!(probe.births, vec![id.0]);
        drop(probe);
        let current = world.current_native_slave_source(id, 0).unwrap();
        // No presentation sync, bulk admission, or new frame occurs here.
        assert_eq!(
            pose(runtime.sample_slave(&current, "CAMERA"))
                .w_axis
                .truncate(),
            Vec3::new(3., 5., 7.)
        );
        let key = runtime
            .drawables
            .binding_key(EPOCH, id.0)
            .expect("actual factory callback admitted receiving association");
        (id, key)
    };
    let peer_id = {
        let (_, _, drawables) = peer_client.split_frame_snow_and_drawable_owner();
        let mut runtime = NativeRendererViewEffects::new(
            peer_pipeline.borrow_native_installed_poses(),
            &mut graphics,
            drawables,
            EPOCH + 1,
            0,
            policy,
            &mut retirement,
        );
        let id = peer
            .create_object_with_script_effects_checked(
                "StrictFactoryOwner",
                Team::USA,
                Vec3::new(303., 5., 7.),
                None,
                &mut crate::game_logic::ScriptPresentationEffects {
                    view: Some(&mut runtime),
                    ..Default::default()
                },
            )
            .unwrap()
            .unwrap();
        let current = peer.current_native_slave_source(id, 0).unwrap();
        assert_eq!(
            pose(runtime.sample_slave(&current, "CAMERA"))
                .w_axis
                .truncate(),
            Vec3::new(303., 5., 7.)
        );
        id
    };
    assert_eq!(id, peer_id);
    let current = world.current_native_slave_source(id, 0).unwrap();
    let foreign = peer.current_native_slave_source(peer_id, 0).unwrap();
    let (_, _, drawables) = client.split_frame_snow_and_drawable_owner();
    let mut runtime = NativeRendererViewEffects::new(
        pipeline.borrow_native_installed_poses(),
        &mut graphics,
        drawables,
        EPOCH,
        0,
        policy,
        &mut retirement,
    );
    assert_eq!(runtime.drawables.binding_key(EPOCH, id.0), Some(binding));
    assert!(matches!(
        runtime.sample_slave(&foreign, "CAMERA"),
        SlaveCameraSample::Unavailable(_)
    ));
    assert_eq!(
        pose(runtime.sample_slave(&current, "CAMERA"))
            .w_axis
            .truncate(),
        Vec3::new(3., 5., 7.)
    );
    drop(runtime);
    struct RejectRetirement;
    impl game_client::core::game_client::NativeDrawableRetirement for RejectRetirement {
        fn retire(
            &mut self,
            _: &mut game_client::core::game_client::NativeDrawableOwner<'_>,
            _: Option<game_client::core::DrawableId>,
        ) -> game_client::core::GameClientResult<()> {
            Err(game_client::core::GameClientError::SubsystemError(
                "retirement unavailable".into(),
            ))
        }
    }
    struct WrongSource<'a, 'b> {
        runtime: &'a mut NativeRendererViewEffects<'b>,
        identity: crate::assets::LiveDrawPlaybackIdentity,
    }
    impl NativeViewEffects for WrongSource<'_, '_> {
        fn local_player_id(&self) -> u32 {
            self.runtime.local_player_id()
        }
        fn host_epoch(&self) -> u64 {
            self.runtime.host_epoch()
        }
        fn sample_slave(&mut self, source: &NativeSlaveSource, bone: &str) -> SlaveCameraSample {
            self.runtime.sample_slave(source, bone)
        }
        fn admit_factory_drawable(
            &mut self,
            mut source: crate::game_logic::NativeFactoryDrawableSource,
            birth: game_client::core::game_client::NativeDrawableBirthContext,
            content: &mut game_engine::common::thing::module_factory::ModuleFactory,
        ) -> Result<(), String> {
            source.render.identity = self.identity.clone();
            self.runtime.admit_factory_drawable(source, birth, content)
        }
    }
    let (_, _, drawables) = client.split_frame_snow_and_drawable_owner();
    let mut rejected_cleanup = RejectRetirement;
    let mut runtime = NativeRendererViewEffects::new(
        pipeline.borrow_native_installed_poses(),
        &mut graphics,
        drawables,
        EPOCH,
        0,
        policy,
        &mut rejected_cleanup,
    );
    let mut wrong = WrongSource {
        runtime: &mut runtime,
        identity: foreign.identity.clone(),
    };
    let failed = world.create_object_with_script_effects_checked(
        "StrictFactoryOwner",
        Team::USA,
        Vec3::new(9., 5., 7.),
        None,
        &mut crate::game_logic::ScriptPresentationEffects {
            view: Some(&mut wrong),
            ..Default::default()
        },
    );
    assert!(failed.is_err());
    assert!(
        failed
            .unwrap_err()
            .to_string()
            .contains("retirement unavailable")
    );
    drop(wrong);
    assert_eq!(runtime.drawables.binding_key(EPOCH, id.0), Some(binding));
    assert_eq!(
        pose(runtime.sample_slave(&current, "CAMERA"))
            .w_axis
            .truncate(),
        Vec3::new(3., 5., 7.)
    );
    drop(runtime);
    let (_, _, drawables) = client.split_frame_snow_and_drawable_owner();
    NativeRendererViewEffects::new(
        pipeline.borrow_native_installed_poses(),
        &mut graphics,
        drawables,
        EPOCH,
        0,
        policy,
        &mut retirement,
    )
    .discard_map_attempt()
    .unwrap();
}

#[test]
#[ignore = "requires native WGPU; run explicit isolated strict-backend acceptance"]
fn detached_no_client_chunk_snapshot_rebuild_removes_absent_actor_and_rejects_old_generation() {
    use crate::game_logic::NativeMapAdmission;
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
    graphics.cache_model(
        crate::assets::mesh_asset_resolve::canonical_model_key("CameraModel"),
        model("CameraModel", 0.),
    );
    let mut pipeline = RenderPipeline::initialize(&graphics).unwrap();
    let mut candidate = super::map_native_admission::NativeMapCandidate::with_epoch(EPOCH);
    let mut world = receiving_factory_world();
    world.add_player(crate::game_logic::Player::new(0, Team::USA, "USA", true));
    let mut retirement = game_client::core::game_client::NativeParticleRetirementService;
    let mut attempt = candidate
        .attempt(
            pipeline.borrow_native_draw_services(),
            &mut graphics,
            0,
            receiving_native_drawable_policy(),
            &mut retirement,
        )
        .unwrap();
    let mut birth = |position| {
        world
            .create_object_with_script_effects_checked(
                "StrictFactoryOwner",
                Team::USA,
                position,
                None,
                &mut attempt.effects(),
            )
            .unwrap()
            .unwrap()
    };
    let retained = birth(Vec3::new(3., 5., 7.));
    let absent = birth(Vec3::new(13., 5., 7.));
    drop(birth);
    let old = world.current_native_slave_source(retained, 0).unwrap();
    world
        .objects
        .get_mut(&retained)
        .unwrap()
        .set_position(Vec3::new(43., 5., 7.));
    let builder = crate::save_load::SnapshotBuilder::new();
    // The saved source excludes this actor; the detached native foundation
    // deliberately still contains it until snapshot reconciliation.
    world.objects.remove(&absent);
    let snapshot = builder.create_world_snapshot(&world).unwrap();
    world.begin_saved_snapshot_admission().unwrap();
    builder
        .restore_from_snapshot(&snapshot, &mut world)
        .unwrap();
    let current = world.current_native_slave_source(retained, 0).unwrap();
    assert_ne!(
        old.identity, current.identity,
        "actual snapshot must replace host instance identity"
    );
    attempt.admit_restored_object_drawables(&mut world).unwrap();
    let mut effects = attempt.effects();
    let view = effects.view.as_deref_mut().unwrap();
    assert!(matches!(
        view.sample_slave(&old, "CAMERA"),
        SlaveCameraSample::Unavailable(_)
    ));
    assert_eq!(
        pose(view.sample_slave(&current, "CAMERA"))
            .w_axis
            .truncate(),
        Vec3::new(43., 5., 7.)
    );
    drop(effects);
    drop(attempt);
    let (owner, _) = candidate.drawable_owners();
    assert!(owner.binding_key(EPOCH, retained.0).is_some());
    assert!(owner.binding_key(EPOCH, absent.0).is_none());
    drop(owner);
    // No CHUNK_GameClient decode or display bulk sync is involved in this route.
    let mut attempt = candidate
        .attempt(
            pipeline.borrow_native_draw_services(),
            &mut graphics,
            0,
            receiving_native_drawable_policy(),
            &mut retirement,
        )
        .unwrap();
    attempt.discard_attempt().unwrap();
}
