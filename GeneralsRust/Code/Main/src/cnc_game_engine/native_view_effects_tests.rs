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
    PresentationFrame::try_build_from_logic(world, 0).unwrap()
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
        ambient_audio_locality: None,
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
    let engine = ww3d_engine::init_headless_blocking(ww3d_engine::EngineConfig {
        width: 32,
        height: 32,
        ..Default::default()
    })
    .unwrap();
        // One guard for these reads: separate lock() temporaries in one call
        // keep the first guard until the statement ends (non-reentrant Mutex).
        let (device, queue, color_format, depth_format) = {
            let engine = engine.lock();
            (
                engine.device(),
                engine.queue(),
                engine.color_format(),
                engine.depth_format(),
            )
        };
        let mut graphics = GraphicsSystem::new(device, queue, color_format, depth_format)
            .unwrap();
    for (name, x) in [("CameraModel", 0.), ("MovingModel", 10.)] {
        graphics.cache_model(
            crate::assets::mesh_asset_resolve::canonical_model_key(name),
            model(name, x),
        );
    }
    let mut pipeline = RenderPipeline::initialize(engine.clone(), &graphics).unwrap();
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
    let (object, fixture_partition_object) = world.host_object_and_partition_mut(ID).unwrap();
    object.set_position_with_partition_owner(
        Vec3::new(43., 17., 29.),
        0,
        &mut Default::default(),
        fixture_partition_object,
    );
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
    fn projected_radius_allocator(
        &mut self,
    ) -> Option<&mut dyn crate::game_logic::NativeProjectedRadiusAllocator> {
        self.runtime.projected_radius_allocator()
    }

    #[cfg(feature = "game_client")]
    fn body_visual_changed(
        &mut self,
        change: &crate::game_logic::NativeBodyVisualChange,
        audio: Option<&mut dyn game_client::drawable::drawable::NativeAmbientAudio>,
    ) -> Result<bool, String> {
        self.runtime.body_visual_changed(change, audio)
    }

    fn local_player_id(&self) -> u32 {
        self.runtime.local_player_id()
    }
    fn host_epoch(&self) -> u64 {
        self.runtime.host_epoch()
    }
    fn retire_physical_object_drawable(
        &mut self,
        source: &crate::game_logic::NativeObjectRetirement<'_>,
    ) -> Result<(), String> {
        self.runtime.retire_physical_object_drawable(source)
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
    let engine = ww3d_engine::init_headless_blocking(ww3d_engine::EngineConfig {
        width: 32,
        height: 32,
        ..Default::default()
    })
    .unwrap();
        // One guard for these reads: separate lock() temporaries in one call
        // keep the first guard until the statement ends (non-reentrant Mutex).
        let (device, queue, color_format, depth_format) = {
            let engine = engine.lock();
            (
                engine.device(),
                engine.queue(),
                engine.color_format(),
                engine.depth_format(),
            )
        };
        let mut graphics = GraphicsSystem::new(device, queue, color_format, depth_format)
            .unwrap();
    graphics.cache_model(
        crate::assets::mesh_asset_resolve::canonical_model_key("CameraModel"),
        model("CameraModel", 0.),
    );
    let mut pipeline = RenderPipeline::initialize(engine.clone(), &graphics).unwrap();
    let mut peer_pipeline = RenderPipeline::initialize(engine.clone(), &graphics).unwrap();
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
        fn projected_radius_allocator(
            &mut self,
        ) -> Option<&mut dyn crate::game_logic::NativeProjectedRadiusAllocator> {
            self.runtime.projected_radius_allocator()
        }

        #[cfg(feature = "game_client")]
        fn body_visual_changed(
            &mut self,
            change: &crate::game_logic::NativeBodyVisualChange,
            audio: Option<&mut dyn game_client::drawable::drawable::NativeAmbientAudio>,
        ) -> Result<bool, String> {
            self.runtime.body_visual_changed(change, audio)
        }

        fn local_player_id(&self) -> u32 {
            self.runtime.local_player_id()
        }
        fn host_epoch(&self) -> u64 {
            self.runtime.host_epoch()
        }
        fn retire_physical_object_drawable(
            &mut self,
            source: &crate::game_logic::NativeObjectRetirement<'_>,
        ) -> Result<(), String> {
            self.runtime.retire_physical_object_drawable(source)
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
    let engine = ww3d_engine::init_headless_blocking(ww3d_engine::EngineConfig {
        width: 32,
        height: 32,
        ..Default::default()
    })
    .unwrap();
        // One guard for these reads: separate lock() temporaries in one call
        // keep the first guard until the statement ends (non-reentrant Mutex).
        let (device, queue, color_format, depth_format) = {
            let engine = engine.lock();
            (
                engine.device(),
                engine.queue(),
                engine.color_format(),
                engine.depth_format(),
            )
        };
        let mut graphics = GraphicsSystem::new(device, queue, color_format, depth_format)
            .unwrap();
    graphics.cache_model(
        crate::assets::mesh_asset_resolve::canonical_model_key("CameraModel"),
        model("CameraModel", 0.),
    );
    let mut pipeline = RenderPipeline::initialize(engine.clone(), &graphics).unwrap();
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
            None,
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
    // The archive comes from a distinct, valid logical save owner containing
    // only the retained actor. Its real factory maintains matching team roster;
    // the outgoing native foundation initially still has both Drawables.
    // A logical-only archive writer has no native renderer requirement.
    let mut saved_source = GameLogic::new();
    saved_source.admit_receiving_module_content(
        crate::assets::ReceivingObjectContent::from_sources([(
            "strict-factory-save-source.ini".to_owned(),
            "Object StrictFactoryOwner\nKindOf = VEHICLE\nBody = ActiveBody BodyTag\nMaxHealth = 40\nInitialHealth = 40\nEnd\nDraw = W3DModelDraw DrawTag\nDefaultConditionState\nModel = CameraModel\nEnd\nEnd\nEnd\n".to_owned(),
        )]).unwrap(),
    ).unwrap();
    saved_source.add_player(crate::game_logic::Player::new(0, Team::USA, "USA", true));
    let saved_retained = saved_source
        .create_object("StrictFactoryOwner", Team::USA, Vec3::new(43., 5., 7.))
        .unwrap();
    assert_eq!(saved_retained, retained);
    assert!(!saved_source.objects.contains_key(&absent));
    assert!(
        world.objects.contains_key(&absent),
        "native map foundation remains intact before reconciliation"
    );
    let builder = crate::save_load::SnapshotBuilder::new();
    let snapshot = builder.create_world_snapshot(&saved_source).unwrap();
    // Saved replacement retires the outgoing logical/native foundation through
    // its actual receiver before decoding incoming objects with reused IDs.
    world
        .clear_all_objects_with_effects_checked(&mut attempt.effects())
        .unwrap();
    assert!(world.objects.registered_is_empty());
    assert!(world.pending_object_removal_snapshot().is_empty());
    {
        let mut effects = attempt.effects();
        assert!(
            matches!(
                effects
                    .view
                    .as_deref_mut()
                    .unwrap()
                    .sample_slave(&old, "CAMERA"),
                SlaveCameraSample::MissingDrawable
            ),
            "old constructor must already be retired before snapshot restoration"
        );
    }
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
            None,
        )
        .unwrap();
    world
        .clear_all_objects_with_effects_checked(&mut attempt.effects())
        .unwrap();
    assert!(world.objects.registered_is_empty());
    assert!(world.pending_object_removal_snapshot().is_empty());
    attempt.discard_attempt().unwrap();
}

fn physical_retirement_world() -> GameLogic {
    let text = "Object StrictFactoryOwner\nKindOf = VEHICLE\nBody = ActiveBody BodyTag\nMaxHealth = 40\nInitialHealth = 40\nEnd\nBehavior = DestroyDie DeleteTag\nEnd\nDraw = W3DModelDraw DrawTag\nDefaultConditionState\nModel = CameraModel\nEnd\nEnd\nEnd\nObject StrictNoDrawOwner\nKindOf = VEHICLE\nBody = ActiveBody BodyTag\nMaxHealth = 40\nInitialHealth = 40\nEnd\nEnd\n";
    let mut world = GameLogic::new();
    world
        .admit_receiving_module_content(
            crate::assets::ReceivingObjectContent::from_sources([(
                "strict-retirement.ini".to_owned(),
                text.to_owned(),
            )])
            .unwrap(),
        )
        .unwrap();
    world.require_native_drawable_owner();
    world
}
fn with_physical_retirement_view<R>(
    client: &mut GameClient,
    pipeline: &mut RenderPipeline,
    graphics: &mut GraphicsSystem,
    epoch: u64,
    run: impl FnOnce(&mut NativeRendererViewEffects<'_>) -> R,
) -> R {
    let (_, _, owner) = client.split_frame_snow_and_drawable_owner();
    let mut retirement = game_client::core::game_client::NativeParticleRetirementService;
    let mut runtime = NativeRendererViewEffects::new(
        pipeline.borrow_native_installed_poses(),
        graphics,
        owner,
        epoch,
        0,
        receiving_native_drawable_policy(),
        &mut retirement,
    );
    run(&mut runtime)
}
fn birth_physical_retirement_actor(
    world: &mut GameLogic,
    name: &str,
    position: Vec3,
    client: &mut GameClient,
    pipeline: &mut RenderPipeline,
    graphics: &mut GraphicsSystem,
    epoch: u64,
) -> ObjectId {
    with_physical_retirement_view(client, pipeline, graphics, epoch, |runtime| {
        world
            .create_object_with_script_effects_checked(
                name,
                Team::USA,
                position,
                None,
                &mut crate::game_logic::ScriptPresentationEffects {
                    view: Some(&mut *runtime),
                    ..Default::default()
},
            )
            .unwrap()
            .unwrap()
    })
}

#[test]
#[ignore = "requires native WGPU; run explicit isolated terrain-track retirement acceptance"]
fn actual_tank_track_retires_with_its_own_drawable_and_keeps_the_peer() {
    use game_client::terrain::terrain_visual::TerrainVisualImpl;

    let engine = ww3d_engine::init_headless_blocking(ww3d_engine::EngineConfig {
        width: 32,
        height: 32,
        ..Default::default()
    })
    .unwrap();
    let mut graphics = {
        let engine = engine.lock();
        GraphicsSystem::new(engine.device(), engine.queue(), engine.color_format(), engine.depth_format())
            .unwrap()
    };
    graphics.cache_model(
        crate::assets::mesh_asset_resolve::canonical_model_key("CameraModel"),
        model("CameraModel", 0.0),
    );
    let name = "StrictTrackTank";
    let trackless_name = "StrictTracklessActor";
    let make_world = || {
        let mut world = GameLogic::new();
        let source = "Object StrictTrackTank\nKindOf = VEHICLE\nBody = ActiveBody BodyTag\nMaxHealth = 40\nInitialHealth = 40\nEnd\nDraw = W3DTankDraw TrackTag\nDefaultConditionState\nModel = CameraModel\nEnd\nTrackMarks = EXTnkTrack.tga\nEnd\nEnd\n";
        let trackless = "Object StrictTracklessActor\nKindOf = VEHICLE\nBody = ActiveBody BodyTag\nMaxHealth = 40\nInitialHealth = 40\nEnd\nDraw = W3DModelDraw ModelTag\nDefaultConditionState\nModel = CameraModel\nEnd\nEnd\nEnd\n";
        world.admit_receiving_module_content(
            crate::assets::ReceivingObjectContent::from_sources([
                ("track-test.ini".into(), source.into()),
                ("trackless-test.ini".into(), trackless.into()),
            ])
                .unwrap(),
        ).unwrap();
        world.require_native_drawable_owner();
        world
    };
    let mut world = make_world();
    let mut peer = make_world();
    let mut client = GameClient::new().unwrap();
    let mut peer_client = GameClient::new().unwrap();
    let mut pipeline = RenderPipeline::initialize(engine.clone(), &graphics).unwrap();
    let mut peer_pipeline = RenderPipeline::initialize(engine, &graphics).unwrap();
    let id = birth_physical_retirement_actor(
        &mut world, name, Vec3::new(20.0, 10.0, 20.0), &mut client, &mut pipeline, &mut graphics, EPOCH,
    );
    let peer_id = birth_physical_retirement_actor(
        &mut peer, name, Vec3::new(60.0, 10.0, 20.0), &mut peer_client, &mut peer_pipeline, &mut graphics, EPOCH,
    );
    assert_eq!(id, peer_id);
    let bind = |client: &mut GameClient, terrain: &mut TerrainVisualImpl| {
        let mut owner = client.borrow_native_drawable_owner();
        let key = owner.binding_key(EPOCH, id.0).unwrap();
        assert!(
            owner
                .bind_installed_model_track(key, "TrackTag", || Some(14.0), terrain)
                .unwrap()
        );
        assert!(
            !owner
                .bind_installed_model_track(
                    key,
                    "TrackTag",
                    || panic!("an already-bound model cannot resample track width"),
                    terrain,
                )
                .unwrap()
        );
        let mut bindings = owner.terrain_track_bindings_for_retirement(key).unwrap();
        assert_eq!(bindings.len(), 1);
        bindings.pop().unwrap()
    };
    let trackless_id = birth_physical_retirement_actor(
        &mut world,
        trackless_name,
        Vec3::new(50.0, 0.0, 50.0),
        &mut client,
        &mut pipeline,
        &mut graphics,
        EPOCH,
    );
    let mut terrain = TerrainVisualImpl::new();
    let mut peer_terrain = TerrainVisualImpl::new();
    let local_binding = bind(&mut client, &mut terrain);
    for (actor, expected_queries) in [(trackless_id, 0), (id, 1)] {
        let mut logic_queries = 0;
        client.borrow_native_drawable_owner().advance_installed_model_tracks(
            EPOCH,
            actor.0,
            &mut terrain,
            |key| {
                assert_eq!(key.object_id, actor.0);
                logic_queries += 1;
                Some(world.world_services.terrain().read().unwrap())
            },
            Vec3::new(20.0, 20.0, 10.0),
            generals_sim::pathfinding::PathfindLayerEnum::Ground,
            false,
            1234,
            world.host_object(actor).unwrap().physics_gravity(),
        );
        assert_eq!(logic_queries, expected_queries);
    }
    let peer_binding = bind(&mut peer_client, &mut peer_terrain);
    assert_eq!(local_binding.slot(), peer_binding.slot());
    assert!(!terrain.receives_terrain_track(&peer_binding));
    pipeline.observe_animation_sync(1_234);
    with_physical_retirement_view(&mut client, &mut pipeline, &mut graphics, EPOCH, |view| {
        let mut effects = crate::game_logic::ScriptPresentationEffects {
            view: Some(view),
            terrain: Some(&mut terrain),
            ..Default::default()
        };
        for x in [20.0, 40.0, 60.0] {
            assert!(world.set_registered_object_position_with_effects(
                id, Vec3::new(x, 10.0, 20.0), 4, &mut effects,
            ));
        }
        world.objects.get_mut(&id).unwrap().status.stealthed = true;
        assert!(world.set_registered_object_position_with_effects(
            id, Vec3::new(80.0, 10.0, 20.0), 4, &mut effects,
        ));
        world.destroy_object_with_effects(id, &mut effects);
        world.process_destroy_list_with_effects_checked(&mut effects).unwrap();
        assert!(!world.objects.contains_key(&id));
    });
    assert!(client.borrow_native_drawable_owner().binding_key(EPOCH, id.0).is_none());
    assert!(!terrain.receives_terrain_track(&local_binding));
    let retired = terrain.terrain_tracks().track(local_binding.slot()).unwrap();
    let edges = retired.active_edges(terrain.terrain_tracks().config().max_tank_track_edges);
    assert_eq!(edges.len(), 3, "all intermediate source setter visits ran before retirement");
    for (edge, x) in edges.iter().zip([40.0, 60.0, 80.0]) {
        assert_eq!(edge.time_added, 1_234);
        assert_eq!(edge.endpoint_pos[0], Vec3::new(x, 27.0, 2.0));
    }
    assert_eq!(edges.last().unwrap().alpha, 0.0);
    assert!(peer_terrain.receives_terrain_track(&peer_binding));
    assert_eq!(peer_terrain.terrain_tracks().track(peer_binding.slot()).unwrap().active_edge_count(), 0);
    assert!(peer_client.borrow_native_drawable_owner().binding_key(EPOCH, peer_id.0).is_some());
}

#[test]
#[ignore = "requires native WGPU; run explicit isolated physical-retirement acceptance"]
fn actual_physical_delete_retires_semantic_receiver_only_after_queue_and_rejects_stale_rebirth_including_no_draw()
 {
    let engine = ww3d_engine::init_headless_blocking(ww3d_engine::EngineConfig {
        width: 32,
        height: 32,
        ..Default::default()
    })
    .unwrap();
        // One guard for these reads: separate lock() temporaries in one call
        // keep the first guard until the statement ends (non-reentrant Mutex).
        let (device, queue, color_format, depth_format) = {
            let engine = engine.lock();
            (
                engine.device(),
                engine.queue(),
                engine.color_format(),
                engine.depth_format(),
            )
        };
        let mut graphics = GraphicsSystem::new(device, queue, color_format, depth_format)
            .unwrap();
    graphics.cache_model(
        crate::assets::mesh_asset_resolve::canonical_model_key("CameraModel"),
        model("CameraModel", 0.),
    );
    let mut pipeline = RenderPipeline::initialize(engine.clone(), &graphics).unwrap();
    let mut peer_pipeline = RenderPipeline::initialize(engine.clone(), &graphics).unwrap();
    let mut client = GameClient::new().unwrap();
    let mut peer_client = GameClient::new().unwrap();
    let mut world = physical_retirement_world();
    let mut peer = physical_retirement_world();
    let id = birth_physical_retirement_actor(
        &mut world,
        "StrictFactoryOwner",
        Vec3::new(3., 5., 7.),
        &mut client,
        &mut pipeline,
        &mut graphics,
        EPOCH,
    );
    let peer_id = birth_physical_retirement_actor(
        &mut peer,
        "StrictFactoryOwner",
        Vec3::new(303., 5., 7.),
        &mut peer_client,
        &mut peer_pipeline,
        &mut graphics,
        EPOCH + 1,
    );
    assert_eq!(
        id, peer_id,
        "independent allocators deliberately reuse the same ObjectID"
    );
    let old = world.current_native_slave_source(id, 0).unwrap();
    let peer_source = peer.current_native_slave_source(peer_id, 0).unwrap();
    let old_key = with_physical_retirement_view(
        &mut client,
        &mut pipeline,
        &mut graphics,
        EPOCH,
        |runtime| {
            let key = runtime.drawables.binding_key(EPOCH, id.0).unwrap();
            assert_eq!(
                pose(runtime.sample_slave(&old, "CAMERA")).w_axis.truncate(),
                Vec3::new(3., 5., 7.)
            );
            world.destroy_object_with_effects(
                id,
                &mut crate::game_logic::ScriptPresentationEffects {
                    view: Some(&mut *runtime),
                    ..Default::default()
},
            );
            assert!(world.has_pending_object_removals());
            assert!(
                world.objects.contains_key(&id),
                "marking is not physical removal"
            );
            assert!(world.objects[&id].status.destroyed);
            assert_eq!(
                world.objects[&id].health.current, 40.,
                "direct deletion must not manufacture body damage"
            );
            assert_eq!(runtime.drawables.binding_key(EPOCH, id.0), Some(key));
            assert_eq!(
                pose(runtime.sample_slave(&old, "CAMERA")).w_axis.truncate(),
                Vec3::new(3., 5., 7.)
            );
            world
                .process_destroy_list_with_effects_checked(
                    &mut crate::game_logic::ScriptPresentationEffects {
                        view: Some(&mut *runtime),
                        ..Default::default()
},
                )
                .unwrap();
            assert!(!world.objects.contains_key(&id));
            assert!(!world.has_pending_object_removals());
            assert!(runtime.drawables.binding_key(EPOCH, id.0).is_none());
            assert!(matches!(
                runtime.sample_slave(&old, "CAMERA"),
                SlaveCameraSample::MissingDrawable
            ));
            key
        },
    );
    with_physical_retirement_view(
        &mut peer_client,
        &mut peer_pipeline,
        &mut graphics,
        EPOCH + 1,
        |runtime| {
            assert!(
                runtime.retire_object_drawable(&old.identity, None).is_err(),
                "a foreign world cannot retire the same numeric ID"
            );
            assert!(
                runtime
                    .drawables
                    .binding_key(EPOCH + 1, peer_id.0)
                    .is_some()
            );
            assert_eq!(
                pose(runtime.sample_slave(&peer_source, "CAMERA"))
                    .w_axis
                    .truncate(),
                Vec3::new(303., 5., 7.)
            );
        },
    );
    // Reuse only after actual physical removal, through the real restore allocator.
    world.set_next_object_id_for_restore(id);
    let reborn = birth_physical_retirement_actor(
        &mut world,
        "StrictFactoryOwner",
        Vec3::new(43., 5., 7.),
        &mut client,
        &mut pipeline,
        &mut graphics,
        EPOCH,
    );
    assert_eq!(reborn, id);
    let current = world.current_native_slave_source(reborn, 0).unwrap();
    assert_ne!(old.identity, current.identity);
    let mut audio = game_engine::common::audio::AudioManager::new();
    audio.init();
    with_physical_retirement_view(
        &mut client,
        &mut pipeline,
        &mut graphics,
        EPOCH,
        |runtime| {
            let key = runtime.drawables.binding_key(EPOCH, id.0).unwrap();
            assert_ne!(key, old_key);
            assert!(runtime.retire_object_drawable(&old.identity, None).is_err());
            assert_eq!(runtime.drawables.binding_key(EPOCH, id.0), Some(key));
            assert_eq!(
                pose(runtime.sample_slave(&current, "CAMERA"))
                    .w_axis
                    .truncate(),
                Vec3::new(43., 5., 7.)
            );
            // A lethal Body operation is distinct from direct deletion above, and
            // still does not itself erase the installed semantic receiver.
            // Use the complete production admission, which executes Body's
            // synchronous callbacks and admits the resulting death lifecycle.
            // The lower apply_host_damage adapter only returns Body's result.
            let damage = world
                .apply_owned_damage_with_effects(
                    reborn,
                    80.,
                    None,
                    crate::game_logic::combat::DamageType::Unresistable,
                    crate::game_logic::host_usa_pilot::HostDeathType::Normal,
                    None,
                    &crate::game_logic::object::DamageHitContext::default(),
                    &mut crate::game_logic::ScriptPresentationEffects {
                        view: Some(&mut *runtime),
                        audio: Some(&mut audio),
                        ..Default::default()
},
                )
                .unwrap()
                .expect("receiving Body accepts damage");
            assert!(damage.destroyed());
            assert!(world.objects.contains_key(&reborn));
            assert!(world.objects[&reborn].health.current <= 0.);
            assert!(world.has_pending_object_removals());
            assert_eq!(runtime.drawables.binding_key(EPOCH, reborn.0), Some(key));
            world
                .process_destroy_list_with_effects_checked(
                    &mut crate::game_logic::ScriptPresentationEffects {
                        view: Some(&mut *runtime),
                        ..Default::default()
},
                )
                .unwrap();
            assert!(!world.objects.contains_key(&reborn));
            assert!(runtime.drawables.binding_key(EPOCH, reborn.0).is_none());
            assert!(matches!(
                runtime.sample_slave(&current, "CAMERA"),
                SlaveCameraSample::MissingDrawable
            ));
        },
    );
    // NoDraw still has a semantic constructor lifetime; zero Draw modules must
    // not allow a stale identity to alias a newly bound Object of the same ID.
    world.set_next_object_id_for_restore(id);
    let no_draw = birth_physical_retirement_actor(
        &mut world,
        "StrictNoDrawOwner",
        Vec3::ZERO,
        &mut client,
        &mut pipeline,
        &mut graphics,
        EPOCH,
    );
    assert_eq!(no_draw, id);
    let no_draw_old = world.live_draw_playback_identity(&world.objects[&no_draw]);
    with_physical_retirement_view(
        &mut client,
        &mut pipeline,
        &mut graphics,
        EPOCH,
        |runtime| {
            assert!(runtime.drawables.binding_key(EPOCH, no_draw.0).is_some());
            world.destroy_object_with_effects(
                no_draw,
                &mut crate::game_logic::ScriptPresentationEffects {
                    view: Some(&mut *runtime),
                    ..Default::default()
},
            );
            assert!(world.objects.contains_key(&no_draw));
            assert!(runtime.drawables.binding_key(EPOCH, no_draw.0).is_some());
            world
                .process_destroy_list_with_effects_checked(
                    &mut crate::game_logic::ScriptPresentationEffects {
                        view: Some(&mut *runtime),
                        ..Default::default()
},
                )
                .unwrap();
            assert!(!world.objects.contains_key(&no_draw));
            assert!(runtime.drawables.binding_key(EPOCH, no_draw.0).is_none());
        },
    );
    world.set_next_object_id_for_restore(no_draw);
    let no_draw_reborn = birth_physical_retirement_actor(
        &mut world,
        "StrictNoDrawOwner",
        Vec3::ZERO,
        &mut client,
        &mut pipeline,
        &mut graphics,
        EPOCH,
    );
    assert_eq!(no_draw_reborn, no_draw);
    let no_draw_current = world.live_draw_playback_identity(&world.objects[&no_draw_reborn]);
    assert_ne!(no_draw_old, no_draw_current);
    with_physical_retirement_view(
        &mut client,
        &mut pipeline,
        &mut graphics,
        EPOCH,
        |runtime| {
            let key = runtime
                .drawables
                .binding_key(EPOCH, no_draw_reborn.0)
                .unwrap();
            assert!(runtime.retire_object_drawable(&no_draw_old, None).is_err());
            assert_eq!(
                runtime.drawables.binding_key(EPOCH, no_draw_reborn.0),
                Some(key)
            );
            world.destroy_object_with_effects(
                no_draw_reborn,
                &mut crate::game_logic::ScriptPresentationEffects {
                    view: Some(&mut *runtime),
                    ..Default::default()
},
            );
            world
                .process_destroy_list_with_effects_checked(
                    &mut crate::game_logic::ScriptPresentationEffects {
                        view: Some(&mut *runtime),
                        ..Default::default()
},
                )
                .unwrap();
            assert!(
                runtime
                    .drawables
                    .binding_key(EPOCH, no_draw_reborn.0)
                    .is_none()
            );
        },
    );
    with_physical_retirement_view(
        &mut peer_client,
        &mut peer_pipeline,
        &mut graphics,
        EPOCH + 1,
        |runtime| {
            assert!(peer.objects.contains_key(&peer_id));
            assert_eq!(
                pose(runtime.sample_slave(&peer_source, "CAMERA"))
                    .w_axis
                    .truncate(),
                Vec3::new(303., 5., 7.)
            );
            peer.destroy_object_with_effects(
                peer_id,
                &mut crate::game_logic::ScriptPresentationEffects {
                    view: Some(&mut *runtime),
                    ..Default::default()
},
            );
            peer.process_destroy_list_with_effects_checked(
                &mut crate::game_logic::ScriptPresentationEffects {
                    view: Some(&mut *runtime),
                    ..Default::default()
},
            )
            .unwrap();
        },
    );
}

#[test]
#[ignore = "requires native WGPU; run explicit isolated missing-binding acceptance"]
fn missing_client_key_with_canonical_constructor_is_not_prebirth_absence() {
    let engine = ww3d_engine::init_headless_blocking(ww3d_engine::EngineConfig {
        width: 32,
        height: 32,
        ..Default::default()
    })
    .unwrap();
        // One guard for these reads: separate lock() temporaries in one call
        // keep the first guard until the statement ends (non-reentrant Mutex).
        let (device, queue, color_format, depth_format) = {
            let engine = engine.lock();
            (
                engine.device(),
                engine.queue(),
                engine.color_format(),
                engine.depth_format(),
            )
        };
        let mut graphics = GraphicsSystem::new(device, queue, color_format, depth_format)
            .unwrap();
    let mut pipeline = RenderPipeline::initialize(engine.clone(), &graphics).unwrap();
    let mut client = GameClient::new().unwrap();
    let mut world = physical_retirement_world();
    let id = birth_physical_retirement_actor(
        &mut world,
        "StrictNoDrawOwner",
        Vec3::ZERO,
        &mut client,
        &mut pipeline,
        &mut graphics,
        EPOCH,
    );
    let identity = world.live_draw_playback_identity(&world.objects[&id]);
    // Execute a real CPU-only owner invalidation. Renderer constructor remains,
    // exactly the mismatch the failclosed view must distinguish from prebirth.
    client.invalidate_presentation_drawable_world();
    with_physical_retirement_view(
        &mut client,
        &mut pipeline,
        &mut graphics,
        EPOCH,
        |runtime| {
            assert!(runtime.drawables.binding_key(EPOCH, id.0).is_none());
            assert!(!runtime.drawables.has_object_drawable(id.0));
            assert!(runtime.poses.has_drawable_lifetime(id.0));
            assert!(runtime.retire_object_drawable(&identity, None).is_err());
            assert!(
                runtime.poses.has_drawable_lifetime(id.0),
                "failed preflight cannot erase surviving owner"
            );
            runtime.discard_map_attempt().unwrap();
            assert!(!runtime.poses.has_drawable_lifetime(id.0));
            assert!(
                runtime.retire_object_drawable(&identity, None).is_ok(),
                "both owners absent is genuine prebirth absence"
            );
        },
    );
}

#[test]
#[ignore = "requires native WGPU; exercises the production Client-before-GameLogic reset phase"]
fn actual_client_reset_retires_renderer_receipts_before_logic_reset_and_preserves_peer() {
    let engine = ww3d_engine::init_headless_blocking(ww3d_engine::EngineConfig {
        width: 32,
        height: 32,
        ..Default::default()
    })
    .unwrap();
        // One guard for these reads: separate lock() temporaries in one call
        // keep the first guard until the statement ends (non-reentrant Mutex).
        let (device, queue, color_format, depth_format) = {
            let engine = engine.lock();
            (
                engine.device(),
                engine.queue(),
                engine.color_format(),
                engine.depth_format(),
            )
        };
        let mut graphics = GraphicsSystem::new(device, queue, color_format, depth_format)
            .unwrap();
    graphics.cache_model(
        crate::assets::mesh_asset_resolve::canonical_model_key("CameraModel"),
        model("CameraModel", 0.),
    );
    let mut pipeline = RenderPipeline::initialize(engine.clone(), &graphics).unwrap();
    let mut peer_pipeline = RenderPipeline::initialize(engine.clone(), &graphics).unwrap();
    let mut client = GameClient::new().unwrap();
    let mut peer_client = GameClient::new().unwrap();
    let mut world = physical_retirement_world();
    let mut peer = physical_retirement_world();
    let id = birth_physical_retirement_actor(
        &mut world,
        "StrictFactoryOwner",
        Vec3::ZERO,
        &mut client,
        &mut pipeline,
        &mut graphics,
        EPOCH,
    );
    let peer_id = birth_physical_retirement_actor(
        &mut peer,
        "StrictFactoryOwner",
        Vec3::new(303., 5., 7.),
        &mut peer_client,
        &mut peer_pipeline,
        &mut graphics,
        EPOCH + 1,
    );
    assert_eq!(id, peer_id, "same numeric IDs are session-local");
    let peer_source = peer.current_native_slave_source(peer_id, 0).unwrap();
    assert!(pipeline
        .borrow_native_installed_poses()
        .has_drawable_lifetime(id.0));

    // Match start_game.rs: GameClient::reset, world visual bridge clear,
    // then host_reset_game_logic. The production pipeline hook retires only
    // this engine's Main renderer receipts after Client reset succeeds.
    client.reset().unwrap();
    let (_, _, client_drawables) = client.split_frame_snow_and_drawable_owner();
    pipeline
        .retire_native_drawables_after_client_reset(&client_drawables)
        .unwrap();
    gamelogic::helpers::ClientVisualHandle::new(std::sync::Arc::clone(
        &world.world_services,
    ))
    .clear_visual_state_for_reset();
    assert!(!pipeline
        .borrow_native_installed_poses()
        .has_drawable_lifetime(id.0));

    with_physical_retirement_view(
        &mut client,
        &mut pipeline,
        &mut graphics,
        EPOCH,
        |runtime| {
            assert!(runtime.drawables.binding_key(EPOCH, id.0).is_none());
            assert!(runtime.retire_object_drawable(
                &world.live_draw_playback_identity(&world.objects[&id]),
                None,
            ).is_ok());
            world
                .reset_with_effects_checked(&mut crate::game_logic::ScriptPresentationEffects {
                    view: Some(&mut *runtime),
                    ..Default::default()
})
                .unwrap();
            assert!(world.objects.registered_is_empty());
        },
    );
    with_physical_retirement_view(
        &mut peer_client,
        &mut peer_pipeline,
        &mut graphics,
        EPOCH + 1,
        |runtime| {
            assert!(runtime.drawables.binding_key(EPOCH + 1, peer_id.0).is_some());
            assert!(runtime.poses.has_drawable_lifetime(peer_id.0));
            assert_eq!(
                pose(runtime.sample_slave(&peer_source, "CAMERA"))
                    .w_axis
                    .truncate(),
                Vec3::new(303., 5., 7.)
            );
        },
    );
}

#[path = "native_physical_callback_tests.rs"]
mod native_physical_callback_tests;

#[path = "native_deletion_phase_tests.rs"]
mod native_deletion_phase_tests;

#[path = "native_airfield_sell_tests.rs"]
mod native_airfield_sell_tests;

#[test]
#[ignore = "requires native WGPU; run isolated actual bulk receiving-owner acceptance"]
fn actual_native_bulk_clear_retires_each_owner_without_touching_peer() {
    let engine = ww3d_engine::init_headless_blocking(ww3d_engine::EngineConfig {
        width: 32,
        height: 32,
        ..Default::default()
    })
    .unwrap();
        // One guard for these reads: separate lock() temporaries in one call
        // keep the first guard until the statement ends (non-reentrant Mutex).
        let (device, queue, color_format, depth_format) = {
            let engine = engine.lock();
            (
                engine.device(),
                engine.queue(),
                engine.color_format(),
                engine.depth_format(),
            )
        };
        let mut graphics = GraphicsSystem::new(device, queue, color_format, depth_format)
            .unwrap();
    graphics.cache_model(
        crate::assets::mesh_asset_resolve::canonical_model_key("CameraModel"),
        model("CameraModel", 0.),
    );
    {
        let mut pipeline = RenderPipeline::initialize(engine.clone(), &graphics).unwrap();
        let mut peer_pipeline = RenderPipeline::initialize(engine.clone(), &graphics).unwrap();
        let mut client = GameClient::new().unwrap();
        let mut peer_client = GameClient::new().unwrap();
        let mut world = physical_retirement_world();
        let mut peer = physical_retirement_world();
        let mut ids = Vec::new();
        for name in [
            "StrictFactoryOwner",
            "StrictNoDrawOwner",
            "StrictFactoryOwner",
        ] {
            ids.push(birth_physical_retirement_actor(
                &mut world,
                name,
                Vec3::ZERO,
                &mut client,
                &mut pipeline,
                &mut graphics,
                EPOCH,
            ));
        }
        let peer_id = birth_physical_retirement_actor(
            &mut peer,
            "StrictFactoryOwner",
            Vec3::new(303., 5., 7.),
            &mut peer_client,
            &mut peer_pipeline,
            &mut graphics,
            EPOCH + 1,
        );
        assert_eq!(ids[0], peer_id);
        let peer_source = peer.current_native_slave_source(peer_id, 0).unwrap();
        with_physical_retirement_view(
            &mut client,
            &mut pipeline,
            &mut graphics,
            EPOCH,
            |runtime| {
                for id in &ids {
                    assert!(runtime.drawables.binding_key(EPOCH, id.0).is_some());
                    assert!(runtime.poses.has_drawable_lifetime(id.0));
                }
                let mut effects = crate::game_logic::ScriptPresentationEffects {
                    view: Some(&mut *runtime),
                    ..Default::default()
};
                world
                    .clear_all_objects_with_effects_checked(&mut effects)
                    .unwrap();
                drop(effects);
                assert!(world.objects.registered_is_empty());
                assert!(world.pending_object_removal_snapshot().is_empty());
                for id in &ids {
                    assert!(runtime.drawables.binding_key(EPOCH, id.0).is_none());
                    assert!(!runtime.poses.has_drawable_lifetime(id.0));
                }
            },
        );
        with_physical_retirement_view(
            &mut peer_client,
            &mut peer_pipeline,
            &mut graphics,
            EPOCH + 1,
            |runtime| {
                assert!(
                    runtime
                        .drawables
                        .binding_key(EPOCH + 1, peer_id.0)
                        .is_some()
                );
                assert_eq!(
                    pose(runtime.sample_slave(&peer_source, "CAMERA"))
                        .w_axis
                        .truncate(),
                    Vec3::new(303., 5., 7.)
                );
                peer.clear_all_objects_with_effects_checked(
                    &mut crate::game_logic::ScriptPresentationEffects {
                        view: Some(&mut *runtime),
                        ..Default::default()
},
                )
                .unwrap();
            },
        );
    }
}

#[test]
#[ignore = "requires native WGPU; actual Tree/Prop collection and physical retirement"]
fn actual_tree_prop_render_collection_preserves_constructor_retirement_and_peer() {
    let engine = ww3d_engine::init_headless_blocking(ww3d_engine::EngineConfig {
        width: 32,
        height: 32,
        ..Default::default()
    })
    .unwrap();
        // One guard for these reads: separate lock() temporaries in one call
        // keep the first guard until the statement ends (non-reentrant Mutex).
        let (device, queue, color_format, depth_format) = {
            let engine = engine.lock();
            (
                engine.device(),
                engine.queue(),
                engine.color_format(),
                engine.depth_format(),
            )
        };
        let mut graphics = GraphicsSystem::new(device, queue, color_format, depth_format)
            .unwrap();
    graphics.cache_model(
        crate::assets::mesh_asset_resolve::canonical_model_key("CameraModel"),
        model("CameraModel", 0.),
    );
    for class in ["W3DTreeDraw", "W3DPropDraw"] {
        let name = format!("Retirement{class}");
        let text = format!(
            "Object {name}\nKindOf = VEHICLE\nBody = ActiveBody BodyTag\nMaxHealth = 40\nInitialHealth = 40\nEnd\nDraw = {class} DrawTag\nModelName = CameraModel\nEnd\nEnd\n"
        );
        let mut world = GameLogic::new();
        world
            .admit_receiving_module_content(
                crate::assets::ReceivingObjectContent::from_sources([(
                    "non-camera-retirement.ini".to_owned(),
                    text,
                )])
                .unwrap(),
            )
            .unwrap();
        world.require_native_drawable_owner();
        let mut peer = physical_retirement_world();
        let mut pipeline = RenderPipeline::initialize(engine.clone(), &graphics).unwrap();
        let mut peer_pipeline = RenderPipeline::initialize(engine.clone(), &graphics).unwrap();
        let mut client = GameClient::new().unwrap();
        let mut peer_client = GameClient::new().unwrap();
        let id = birth_physical_retirement_actor(
            &mut world,
            &name,
            Vec3::ZERO,
            &mut client,
            &mut pipeline,
            &mut graphics,
            EPOCH,
        );
        let peer_id = birth_physical_retirement_actor(
            &mut peer,
            "StrictFactoryOwner",
            Vec3::new(303., 5., 7.),
            &mut peer_client,
            &mut peer_pipeline,
            &mut graphics,
            EPOCH + 1,
        );
        assert_eq!(id, peer_id);
        let source = world.current_native_slave_source(id, 0).unwrap();
        let peer_source = peer.current_native_slave_source(peer_id, 0).unwrap();
        assert!(
            source
                .receiving_draw_models
                .first_object_draw(0)
                .unwrap()
                .is_none(),
            "native lifetime admission must not invent a camera ObjectDrawInterface"
        );
        with_physical_retirement_view(
            &mut client,
            &mut pipeline,
            &mut graphics,
            EPOCH,
            |runtime| {
                assert!(!matches!(
                    runtime.sample_slave(&source, "CAMERA"),
                    SlaveCameraSample::Pose(_)
                ));
            },
        );
        let mut frame = PresentationFrame::try_build_from_logic(&mut world, 0).unwrap();
        frame.fow_shell_bypass = true;
        pipeline.set_presentation_frame(Some(std::rc::Rc::new(frame)));
        let (installed, items) =
            pipeline.collect_native_retirement_acceptance_frame(&mut graphics, id.0);
        assert!(installed > 0, "the Drawable keeps its installed registration");
        if class == "W3DTreeDraw" {
            assert_eq!(items, 0, "tree Draw belongs to the terrain renderer");
        }
        with_physical_retirement_view(
            &mut client,
            &mut pipeline,
            &mut graphics,
            EPOCH,
            |runtime| {
                assert!(!matches!(
                    runtime.sample_slave(&source, "CAMERA"),
                    SlaveCameraSample::Pose(_)
                ));
                let key = runtime.drawables.binding_key(EPOCH, id.0).unwrap();
                let mut effects = crate::game_logic::ScriptPresentationEffects {
                    view: Some(&mut *runtime),
                    ..Default::default()
};
                world.destroy_object_with_effects(id, &mut effects);
                world
                    .process_destroy_list_with_effects_checked(&mut effects)
                    .unwrap();
                drop(effects);
                assert!(!world.objects.contains_key(&id));
                assert!(world.pending_object_removal_snapshot().is_empty());
                assert!(runtime.drawables.binding_key(EPOCH, id.0).is_none());
                assert!(!runtime.poses.has_drawable_lifetime(id.0));
                assert_eq!(key.object_id, id.0);
            },
        );
        with_physical_retirement_view(
            &mut peer_client,
            &mut peer_pipeline,
            &mut graphics,
            EPOCH + 1,
            |runtime| {
                assert!(
                    runtime
                        .drawables
                        .binding_key(EPOCH + 1, peer_id.0)
                        .is_some()
                );
                assert_eq!(
                    pose(runtime.sample_slave(&peer_source, "CAMERA"))
                        .w_axis
                        .truncate(),
                    Vec3::new(303., 5., 7.)
                );
                peer.clear_all_objects_with_effects_checked(
                    &mut crate::game_logic::ScriptPresentationEffects {
                        view: Some(&mut *runtime),
                        ..Default::default()
},
                )
                .unwrap();
            },
        );
    }
}

#[test]
fn actual_drawable_admission_uses_each_world_lod_despite_ambient_changes() {
    use game_client::core::game_client::PresentationModuleAdmission;
    use game_client::drawable::BasicDrawable;
    use game_engine::common::game_lod::{get_static_lod, set_static_lod_from_string_for_session};
    use game_engine::common::ini::ini_game_lod::StaticGameLODLevel as Lod;

    fn world(level: Option<Lod>) -> GameLogic {
        let mut world = GameLogic::new();
        world.admit_static_lod_level(level);
        let mut rules = world.lighting_rules_baseline().clone();
        rules.use_draw_module_lod = true;
        world.admit_lighting_rules_base(rules);
        world.admit_receiving_module_content(
            crate::assets::ReceivingObjectContent::from_sources([(
                "drawable-lod-owner.ini".into(),
                "Object LodOwner\nKindOf = VEHICLE\nDraw = W3DModelDraw LowTag\nDefaultConditionState\nModel = NONE\nEnd\nEnd\nDraw = W3DModelDraw HighTag\nMinLODRequired = HIGH\nDefaultConditionState\nModel = NONE\nEnd\nEnd\nEnd\n".into(),
            )]).unwrap(),
        ).unwrap();
        world
    }

    fn admit_to_client(
        world: &mut GameLogic,
        client: &mut GameClient,
        source: &crate::presentation_frame::UnitRenderInput,
    ) {
        let policy = receiving_native_drawable_policy_for(world);
        let content = world.receiving_module_content.as_mut().unwrap();
        let mut drawable = entry(source);
        drawable.module_admission = PresentationModuleAdmission::Authored(
            content.admitted_template("LodOwner").unwrap(),
        );
        client.sync_presentation_drawables_observed_with_content(
            [drawable], 0, content.module_factory_mut(), policy, |_| {},
        ).unwrap();
    }

    fn tags(client: &mut GameClient) -> Vec<String> {
        let id = client.get_drawable_for_object(ID.0).unwrap();
        client.find_drawable_by_id_mut(id).unwrap().as_any_mut()
            .downcast_mut::<BasicDrawable>().unwrap().get_draw_modules_mut().iter()
            .map(|module| module.snapshot_module_identifier().unwrap().to_owned())
            .collect()
    }

    let _common = game_engine::common::common_services::CommonServicesScope::new();
    game_engine::common::ini::ini_game_data::init_global_data();
    game_engine::common::ini::get_global_data().unwrap().write().enable_static_lod = true;
    let mut low = world(Some(Lod::Low));
    let mut high = world(Some(Lod::High));
    let mut source_world = GameLogic::new();
    admit(&mut source_world, 0.0);
    let source = frame_source(&mut source_world);
    let mut low_first = GameClient::new().unwrap();
    let mut high_first = GameClient::new().unwrap();
    let mut low_second = GameClient::new().unwrap();
    let mut high_second = GameClient::new().unwrap();

    set_static_lod_from_string_for_session("High");
    assert_eq!(get_static_lod(), "High");
    admit_to_client(&mut low, &mut low_first, &source);
    admit_to_client(&mut high, &mut high_first, &source);
    set_static_lod_from_string_for_session("Low");
    assert_eq!(get_static_lod(), "Low");
    admit_to_client(&mut high, &mut high_second, &source);
    admit_to_client(&mut low, &mut low_second, &source);
    assert_eq!(tags(&mut low_first), ["LowTag"]);
    assert_eq!(tags(&mut high_first), ["LowTag", "HighTag"]);
    assert_eq!(tags(&mut high_second), ["LowTag", "HighTag"]);
    assert_eq!(tags(&mut low_second), ["LowTag"]);

    // Drawable.cpp compares the real ordered LOD values: Custom retains all
    // these modules, whereas Unknown precedes even Low.
    for (level, expected) in [
        (Some(Lod::Custom), vec!["LowTag", "HighTag"]),
        (Some(Lod::Unknown), vec![]),
        (Some(Lod::Medium), vec!["LowTag"]),
        (None, vec!["LowTag"]),
    ] {
        let mut receiving = world(level);
        let mut client = GameClient::new().unwrap();
        admit_to_client(&mut receiving, &mut client, &source);
        assert_eq!(tags(&mut client), expected, "{level:?}");
    }
}

#[test]
fn native_constructor_policy_keeps_receiving_tod_across_peer_changes_and_invalid_values() {
    use game_client::system::TimeOfDay as Native;
    use game_engine::common::ini::TimeOfDay as Source;
    let mut owner = GameLogic::new();
    let mut peer = GameLogic::new();
    owner.set_receiving_time_of_day(Source::Night);
    peer.set_receiving_time_of_day(Source::Morning);
    assert_eq!(
        receiving_native_drawable_policy_for(&owner).time_of_day,
        Some(Native::Night)
    );
    assert_eq!(
        receiving_native_drawable_policy_for(&peer).time_of_day,
        Some(Native::Morning)
    );
    peer.set_receiving_time_of_day(Source::Evening);
    assert_eq!(
        receiving_native_drawable_policy_for(&owner).time_of_day,
        Some(Native::Night)
    );
    assert_eq!(
        receiving_native_drawable_policy_for(&peer).time_of_day,
        Some(Native::Evening)
    );
    peer.set_receiving_time_of_day(Source::Invalid);
    assert_eq!(
        receiving_native_drawable_policy_for(&peer).time_of_day,
        None
    );
    assert_eq!(
        receiving_native_drawable_policy_for(&owner).time_of_day,
        Some(Native::Night)
    );
}

#[test]
#[ignore = "requires native WGPU; run explicit isolated strict-backend acceptance"]
fn actual_early_body_bridge_uses_real_native_actor_and_captured_local_audio() {
    use game_client::system::TimeOfDay;
    use game_engine::common::audio::{AC_LOOP, AudioEventInfo, AudioManager, ST_GLOBAL};
    use generals_sim::object::BodyDamageType;
    let engine = ww3d_engine::init_headless_blocking(ww3d_engine::EngineConfig {
        width: 32,
        height: 32,
        ..Default::default()
    })
    .unwrap();
        // One guard for these reads: separate lock() temporaries in one call
        // keep the first guard until the statement ends (non-reentrant Mutex).
        let (device, queue, color_format, depth_format) = {
            let engine = engine.lock();
            (
                engine.device(),
                engine.queue(),
                engine.color_format(),
                engine.depth_format(),
            )
        };
        let mut graphics = GraphicsSystem::new(device, queue, color_format, depth_format)
            .unwrap();
    graphics.cache_model(
        crate::assets::mesh_asset_resolve::canonical_model_key("CameraModel"),
        model("CameraModel", 0.0),
    );
    let mut pipeline = RenderPipeline::initialize(engine.clone(), &graphics).unwrap();
    let mut client = GameClient::new().unwrap();
    let mut world = GameLogic::new();
    world.admit_receiving_module_content(crate::assets::ReceivingObjectContent::from_sources([(
        "early-native-runtime.ini".into(),
        "Object NativeEarlyBody\nKindOf = VEHICLE\nSoundAmbientDamaged = NativeEarlyDamaged\nBody = ActiveBody BodyTag\nMaxHealth = 100\nInitialHealth = 100\nEnd\nDraw = W3DModelDraw DrawTag\nDefaultConditionState\nModel = CameraModel\nEnd\nEnd\nEnd\n".into()
    )]).unwrap()).unwrap();
    world.add_player(crate::game_logic::Player::new(0, Team::USA, "Owner", true));
    world.set_receiving_time_of_day(game_engine::common::ini::TimeOfDay::Night);
    world.require_native_drawable_owner();
    let mut audio = AudioManager::new();
    audio.init();
    audio.register_audio_event_info(AudioEventInfo {
        audio_name: "NativeEarlyDamaged".into(),
        sounds: vec!["native_early_sample".into()],
        type_field: ST_GLOBAL,
        control: AC_LOOP,
        loop_count: 0,
        ..Default::default()
    });
    let mut retirement = game_client::core::game_client::NativeParticleRetirementService;
    let (_, _, drawables) = client.split_frame_snow_and_drawable_owner();
    let mut runtime = NativeRendererViewEffects::new(
        pipeline.borrow_native_installed_poses(),
        &mut graphics,
        drawables,
        EPOCH,
        0,
        receiving_native_drawable_policy_for(&world),
        &mut retirement,
    );
    let id = world
        .create_object_with_script_effects_checked(
            "NativeEarlyBody",
            Team::USA,
            Vec3::new(3., 5., 7.),
            None,
            &mut crate::game_logic::ScriptPresentationEffects {
                view: Some(&mut runtime),
                ..Default::default()
},
        )
        .unwrap()
        .unwrap();
    let identity = world.live_draw_playback_identity(world.host_object(id).unwrap());
    let locality = world.receiving_audio_locality_for_player(0);
    // Actual native lifetime requires audio outside map load; reject before
    // model flag callback, not a fabricated service or lost queued request.
    assert!(
        runtime
            .react_owned_body_state(
                &identity,
                BodyDamageType::Damaged,
                false,
                Some(TimeOfDay::Night),
                None,
                &locality,
                &game_engine::common::audio::script_policy::ScriptAudioPolicy::default(),
            )
            .is_err()
    );
    assert!(
        runtime
            .react_owned_body_state(
                &identity,
                BodyDamageType::Damaged,
                true,
                Some(TimeOfDay::Night),
                None,
                &locality,
                &game_engine::common::audio::script_policy::ScriptAudioPolicy::default(),
            )
            .unwrap()
    );
    assert_eq!(audio.pending_play_request_count(), 0);
    // A real object with no admitted native lifetime remains an ordinary null
    // Drawable even without TOD/audio, not an invariant failure.
    let mut detached = GameLogic::new();
    let mut template = ThingTemplate::new("PreNativeBody");
    template.set_health(100.0);
    detached.add_object(Object::new(template, ObjectId(999), Team::USA));
    let missing =
        detached.live_draw_playback_identity(detached.host_object(ObjectId(999)).unwrap());
    assert!(
        !runtime
            .react_owned_body_state(
                &missing,
                BodyDamageType::Damaged,
                false,
                None,
                None,
                &locality,
                &game_engine::common::audio::script_policy::ScriptAudioPolicy::default(),
            )
            .unwrap()
    );
    world
        .apply_owned_damage_with_effects(
            id,
            40.0,
            None,
            crate::game_logic::combat::DamageType::Unresistable,
            crate::game_logic::host_usa_pilot::HostDeathType::Normal,
            None,
            &crate::game_logic::object::DamageHitContext::default(),
            &mut crate::game_logic::ScriptPresentationEffects {
                view: Some(&mut runtime),
                audio: Some(&mut audio),
                ..Default::default()
},
        )
        .unwrap();
    assert_eq!(world.host_object(id).unwrap().health.current, 60.0);
    assert_eq!(
        audio.pending_play_request_count_for("NativeEarlyDamaged"),
        1
    );
    let event = audio
        .pending_play_events()
        .find(|event| event.get_event_name() == "NativeEarlyDamaged")
        .unwrap();
    assert_eq!(
        event.get_time_of_day(),
        game_engine::common::audio::audio_event_rts::TimeOfDay::Night
    );
    assert_ne!(event.get_playing_handle(), 0);
}

#[test]
#[ignore = "requires native WGPU; run explicit installed particle pose acceptance"]
fn visible_scaffold_particles_take_the_held_clip_bone_from_the_real_draw_receiver() {
    exercise_scaffold_draw_particles(false);
}

#[test]
#[ignore = "requires native WGPU; run explicit animated particle bone acceptance"]
fn animated_scaffold_emitter_keeps_its_id_and_follows_the_installed_draw_pose() {
    exercise_scaffold_draw_particles(true);
}

fn exercise_scaffold_draw_particles(follows_animated_bone: bool) {
    use crate::assets::models::test_animation::{motion, raw_animation};
    use game_client::core::game_client::PresentationModuleAdmission;
    use game_client::effects::particle_manager::ParticleSystemTemplate;

    let _ = env_logger::builder().is_test(true).try_init();
    const SOURCE: &str = r#"
Object StrictCameraOwner
 KindOf = STRUCTURE IMMOBILE
 Body = StructureBody StructureBodyTag
  MaxHealth = 100
  InitialHealth = 100
 End
 Draw = W3DModelDraw OptionalDraw
  MinLODRequired = HIGH
  DefaultConditionState
   Model = BaseModel
  End
 End
 Draw = W3DModelDraw ScaffoldDraw
  DefaultConditionState
   Model = None
   TransitionKey = DOWN
  End
  ConditionState = PARTIALLY_CONSTRUCTED
   Model = ScaffoldModel
   Animation = TREE.RAISE
   AnimationMode = MANUAL
   Flags = START_FRAME_LAST
   TransitionKey = UP
   ParticleSysBone = CAMERA BuildSmoke
  End
  TransitionState = DOWN UP
   Model = ScaffoldModel
   Animation = TREE.RAISE
   AnimationMode = ONCE
   Flags = START_FRAME_FIRST
  End
  TransitionState = UP DOWN
   Model = ScaffoldModel
   Animation = TREE.RAISE
   AnimationMode = ONCE_BACKWARDS
   Flags = START_FRAME_LAST
  End
 End
End
"#;
    let engine = ww3d_engine::init_headless_blocking(ww3d_engine::EngineConfig {
        width: 32,
        height: 32,
        ..Default::default()
    })
    .unwrap();
    let (device, queue, color_format, depth_format) = {
        let engine = engine.lock();
        (engine.device(), engine.queue(), engine.color_format(), engine.depth_format())
    };
    let mut graphics = GraphicsSystem::new(device, queue, color_format, depth_format).unwrap();
    let mut scaffold = model("ScaffoldModel", 0.);
    scaffold.animations.push(raw_animation(
        "RAISE", "TREE", 4, 1, vec![motion(1, 0, 0, 3, vec![0., 2., 4., 6.])], vec![],
    ));
    for (name, model) in [("BaseModel", model("BaseModel", 0.)), ("ScaffoldModel", scaffold)] {
        graphics.cache_model(crate::assets::mesh_asset_resolve::canonical_model_key(name), model);
    }
    let mut pipeline = RenderPipeline::initialize(engine, &graphics).unwrap();
    let mut world = GameLogic::new();
    let mut rules = world.lighting_rules_baseline().clone();
    rules.use_draw_module_lod = true;
    world.admit_lighting_rules_base(rules);
    let authored_source = if follows_animated_bone {
        SOURCE.replace(
            "Draw = W3DModelDraw ScaffoldDraw",
            "Draw = W3DModelDraw ScaffoldDraw\n  ParticlesAttachedToAnimatedBones = Yes",
        )
    } else {
        SOURCE.to_owned()
    };
    world.admit_receiving_module_content(
        crate::assets::ReceivingObjectContent::from_sources([(
            "scaffold-gpu.ini".into(), authored_source,
        )]).unwrap(),
    ).unwrap();
    let template = world.templates["StrictCameraOwner"].clone();
    world.add_object(Object::new(template, ID, Team::USA));
    let mut client = GameClient::new().unwrap();
    client.borrow_native_drawable_owner().particle_systems_mut()
        .register_template(Arc::new(ParticleSystemTemplate::new("BuildSmoke".into())));
    let mut bridge = game_client::render_bridge::RenderBridge::new();
    let snow = game_client::snow::SnowRenderState {
        manager: Default::default(),
        definition: Default::default(),
    };
    let assets = crate::assets::AssetManager::new().unwrap();
    let mut modes = super::mouse::LookAtHostModes::default();
    let active = 1u128 << crate::game_logic::host_enum_table_residual::partially_constructed_model_bit();
    let mut installed_emitter = None;
    let visual_world = gamelogic::helpers::ClientVisualHandle::new(Arc::clone(&world.world_services));
    for (clock, bits) in [
        (0, 0), (10, active), (1010, active), (2010, active), (3010, active),
        (3011, active), (3012, active), (4000, active), (5000, active),
        (5001, active), (5010, 0),
    ] {
        let offset = if follows_animated_bone && clock >= 5000 { 27. } else { 0. };
        if follows_animated_bone {
            world.set_fixture_object_position(ID, Vec3::new(offset, 0., 0.));
        }
        let object = world.host_object_mut(ID).unwrap();
        object.set_status_under_construction(bits == active);
        object.construction_percent = if bits == active { 0.25 } else { 1.0 };
        if bits == active {
            object.set_under_construction_model_conditions(true);
        } else {
            object.clear_under_construction_model_conditions();
        }
        let frame = std::rc::Rc::new(PresentationFrame::try_build_from_logic(&mut world, 0).unwrap());
        let source = frame.native_resident_drawable_inputs().into_iter()
            .find(|source| source.id == ID).unwrap();
        let expected_destination = if bits == active { "ScaffoldModel" } else { "" };
        let ordinary = frame.unit_render_inputs().into_iter()
            .find(|input| input.id == ID).unwrap();
        let receiving = ordinary.receiving_draw_models.as_ref().unwrap();
        let conditions = ordinary.model_condition_bits_with_combat_flags();
        let selected = receiving.native_render_draws(conditions).unwrap();
        assert_eq!(selected[1].model_key, expected_destination, "clock {clock}, mask {conditions:#x}");
        assert!(ordinary.fow_should_render());
        let policy = receiving_native_drawable_policy_for(&world);
        let content = world.receiving_module_content.as_mut().unwrap();
        let mut drawable = entry(&source);
        drawable.under_construction = source.under_construction;
        drawable.construction_percent = source.construction_percent;
        drawable.module_admission = PresentationModuleAdmission::Authored(
            content.admitted_template("StrictCameraOwner").unwrap(),
        );
        pipeline.observe_animation_sync(clock);
        client.sync_presentation_drawables_observed_with_content(
            [drawable], clock, content.module_factory_mut(), policy, |event| {
                pipeline.receive_native_drawable_mutation(event, Some(&source), &mut graphics);
            },
        ).unwrap();
        if clock == 0 {
            let id = client.get_drawable_for_object(ID.0).unwrap();
            let drawable = client.find_drawable_by_id_mut(id).unwrap().as_any_mut()
                .downcast_mut::<game_client::drawable::BasicDrawable>().unwrap();
            let tags: Vec<_> = drawable.get_draw_modules().iter()
                .filter_map(|module| module.snapshot_module_identifier()).collect();
            assert_eq!(tags, ["ScaffoldDraw"], "the preceding high-LOD Draw was omitted");
        }
        let mut drawables = client.borrow_native_drawable_owner();
        let key = drawables.binding_key(EPOCH, ID.0).unwrap();
        if !follows_animated_bone {
            assert!(drawables.accepts_installed_state_particles(key, "ScaffoldDraw"));
        }
        pipeline.admit_native_drawable_binding(source.draw_playback_identity.as_ref().unwrap(), key)
            .unwrap();
        pipeline.set_presentation_frame(Some(frame));
        let mut receiver = super::native_frame_receiver::NativeFrameReceiver {
            audio: None,
            world: &mut world,
            drawables,
            look_at_modes: &mut modes,
            world_bounds: (Vec3::splat(-100.), Vec3::splat(100.)),
            slave_source: None,
            slave_binding_key: None,
            host_epoch: EPOCH,
            local_player_id: 0,
            assets: &assets,
            logic_frame: clock,
            use_shadow_decals: true,
        };
        pipeline.execute(
            &mut graphics, &mut bridge, &Mat4::IDENTITY, &Mat4::IDENTITY, Vec3::ZERO,
            0., false, 0, false, &mut receiver, vec![], None, &snow, Vec2::ZERO, clock, None,
        ).unwrap();
        let active_systems: Vec<_> = receiver.drawables.particle_systems().draw_particle_systems()
            .into_iter().filter(|system| !system.is_destroyed()).collect();
        let saved = pipeline.capture_client_drawable_snapshot(&visual_world);
        let scaffold = saved.drawables.iter().find(|row| row.draw_module_index == 1);
        use crate::save_load::snapshot::ClientDrawableAnimationMode;
        let animation = scaffold.and_then(|row| row.animation.as_ref());
        let held = bits == active && animation.is_some_and(|clip| {
            clip.mode == ClientDrawableAnimationMode::Manual
        });
        assert_eq!(active_systems.len(), usize::from(held), "clock {clock}: {scaffold:?}");
        if clock == 1010 {
            assert_eq!(
                animation.map(|clip| &clip.mode), Some(&ClientDrawableAnimationMode::Once),
                "clock {clock}, projected mask {conditions:#x}, actual rows {saved:?}",
            );
        }
        if clock == 5000 {
            assert!(held, "scaffold never admitted the held clip: {saved:?}");
        }
        if let Some(system) = active_systems.first() {
            assert_eq!(system.attached_drawable_id(), key.drawable_id);
            assert!(!system.is_saveable());
            assert_eq!(system.position(), Vec3::new(6. + offset, 0., 0.));
            let initial = installed_emitter.get_or_insert(system.system_id());
            assert_eq!(*initial, system.system_id());
        }
    }
    assert!(installed_emitter.is_some());
}

#[test]
#[ignore = "requires native WGPU and mounted Zero Hour content"]
fn mounted_emp_pulse_installs_stock_sparks_with_original_geometry_and_lifetime() {
    use crate::game_logic::{CombatParticleKind, Player, ScriptPresentationEffects};

    let retail = crate::fixture_content::retail_content_root()
        .join("Command & Conquer Generals Zero Hour/Data/INIZH.big");
    let mut files = crate::assets::ArchiveFileSystem::new();
    files.set_local_search_roots(Vec::new());
    futures::executor::block_on(files.load_big_file(retail)).unwrap();
    let mut read_block = |path: &str, header: &str| {
        let bytes = files
            .read_optional_definition_source(path)
            .unwrap()
            .unwrap();
        let source = String::from_utf8_lossy(&bytes);
        let mut lines = source
            .lines()
            .skip_while(|line| !line.trim().eq_ignore_ascii_case(header));
        let mut block = format!("{}\n", lines.next().expect(header));
        for line in lines {
            block.push_str(line);
            block.push('\n');
            if line.trim_end().eq_ignore_ascii_case("End") {
                return block;
            }
        }
        panic!("missing end for {header}");
    };
    let pulse = read_block(
        "Data/INI/Object/System.ini",
        "Object EMPPatriotEffectSpheroid",
    );
    let sparks = read_block("Data/INI/ParticleSystem.ini", "ParticleSystem EMPSparks");
    let victim_definition = "
Object EmpSparkSphereFixture
  KindOf = VEHICLE
  Body = ActiveBody Body
    MaxHealth = 1000
    InitialHealth = 1000
  End
  Geometry = Sphere
  GeometryMajorRadius = 32
  GeometryHeight = 1
  GeometryIsSmall = No
End
";
    let mut world = GameLogic::new();
    world.frame = 40;
    world
        .admit_receiving_module_content(
            crate::assets::ReceivingObjectContent::from_sources([
                ("Object/System.ini".into(), pulse),
                ("emp-victim.ini".into(), victim_definition.into()),
            ])
            .unwrap(),
        )
        .unwrap();
    world.add_player(Player::new(0, Team::USA, "USA", true));
    let mut enemy = Player::new(1, Team::China, "China", true);
    enemy.alliance_team = 1;
    world.add_player(enemy);

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
    let mut client = GameClient::new().unwrap();
    let mut retirement = game_client::core::game_client::NativeParticleRetirementService;
    let position = Vec3::new(160.0, 0.0, 160.0);
    let victim = {
        let (_, _, drawables) = client.split_frame_snow_and_drawable_owner();
        let mut view = NativeRendererViewEffects::new(
            pipeline.borrow_native_installed_poses(),
            &mut graphics,
            drawables,
            EPOCH,
            0,
            receiving_native_drawable_policy(),
            &mut retirement,
        );
        view.admit_map_particle_definitions(&[sparks]).unwrap();
        let mut effects = ScriptPresentationEffects {
            view: Some(&mut view),
            ..Default::default()
        };
        let victim = world
            .create_object_for_player_with_effects_checked(
                "EmpSparkSphereFixture",
                1,
                position,
                None,
                &mut effects,
            )
            .unwrap()
            .unwrap();
        world
            .create_object_for_player_with_effects_checked(
                "EMPPatriotEffectSpheroid",
                0,
                position,
                None,
                &mut effects,
            )
            .unwrap()
            .unwrap();
        victim
    };
    for frame in 40..=43 {
        assert_eq!(world.frame, frame);
        assert_eq!(world.supw_patriot_emp_sparks_spawned(), 0);
        let (_, _, drawables) = client.split_frame_snow_and_drawable_owner();
        let mut view = NativeRendererViewEffects::new(
            pipeline.borrow_native_installed_poses(),
            &mut graphics,
            drawables,
            EPOCH,
            0,
            receiving_native_drawable_policy(),
            &mut retirement,
        );
        world.tick_logic_frame_with_script_effects(
            1.0 / 30.0,
            None,
            None,
            ScriptPresentationEffects {
                view: Some(&mut view),
                ..Default::default()
            },
            |_| {},
            |_| {},
        );
    }

    assert_eq!(
        world
            .host_object(victim)
            .unwrap()
            .status
            .disabled_emp_until_frame,
        343
    );
    let instances = world
        .combat_particles()
        .systems_of_kind(CombatParticleKind::DisableFx);
    // EMPUpdate.cpp uses a sphere's radius above its position and caps that
    // height at 10 for the particle count. Its GeometryHeight field is irrelevant.
    let expected = game_engine::base_type::real_to_int_ceil(
        0.001 * (std::f32::consts::PI * 32.0 * 32.0 * 10.0),
    ) as usize;
    assert_eq!(expected, 33);
    assert_eq!(instances.len(), expected);
    let (_, _, mut drawables) = client.split_frame_snow_and_drawable_owner();
    let particles = drawables.particle_systems_mut();
    for receipt in instances {
        assert_eq!(receipt.source_object, Some(victim));
        assert_eq!(receipt.spawned_frame, 43);
        assert_eq!(receipt.template_name, "EMPSparks");
        assert_eq!(receipt.attachment.as_ref().unwrap().lifetime, Some(270));
        let system = particles
            .find_particle_system(receipt.client_system_id.expect("native EMPSparks binding"))
            .expect("live receiving particle system");
        assert_eq!(system.attached_object(), Some(victim.0));
        assert!((1..=100).contains(&system.initial_delay_left()));
    }
}
#[test]
#[ignore = "requires native WGPU and mounted Zero Hour content"]
fn mounted_supply_draw_latches_stock_pristine_children_only_at_real_status() {
    use crate::game_logic::{Player, ScriptPresentationEffects, Team};

    let _ = env_logger::builder().is_test(true).try_init();

    let mounted = crate::fixture_content::retail_content_root();
    let install = mounted.join("Command & Conquer Generals Zero Hour");
    let stock =
        crate::parallel_harness::RetailWorldDefinitions::load_from_install(Some(&install)).unwrap();
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
    let mut assets = crate::assets::AssetManager::new().unwrap();
    pollster::block_on(assets.init(device.as_ref(), queue.as_ref())).unwrap();
    assets
        .archive_file_system_mut()
        .set_local_search_roots(Vec::new());
    for archive in [
        mounted.join("Command & Conquer Generals/Data/W3D.big"),
        install.join("Data/W3DZH.big"),
    ] {
        pollster::block_on(assets.archive_file_system_mut().load_big_file(archive)).unwrap();
    }
    let assets = std::sync::Arc::new(std::sync::Mutex::new(assets));
    let mut graphics = GraphicsSystem::new(device, queue, color, depth).unwrap();
    let mut retirement = game_client::core::game_client::NativeParticleRetirementService;

    let mut pipelines = [
        RenderPipeline::initialize(engine.clone(), &graphics).unwrap(),
        RenderPipeline::initialize(engine, &graphics).unwrap(),
    ];
    let mut worlds = [GameLogic::new(), GameLogic::new()];
    let mut clients = [GameClient::new().unwrap(), GameClient::new().unwrap()];
    let mut depot_ids = Vec::new();
    let mut control_ids = None;
    for index in 0..worlds.len() {
        let world = &mut worlds[index];
        stock.admit(world).unwrap();
        world.add_player(Player::new(0, Team::USA, "Local", true));
        let pipeline = &mut pipelines[index];
        pipeline.bind_application_assets(assets.clone());
        let (_, _, drawables) = clients[index].split_frame_snow_and_drawable_owner();
        let mut view = NativeRendererViewEffects::new(
            pipeline.borrow_native_installed_poses(),
            &mut graphics,
            drawables,
            EPOCH,
            0,
            receiving_native_drawable_policy(),
            &mut retirement,
        );
        let mut effects = ScriptPresentationEffects {
            view: Some(&mut view),
            ..Default::default()
        };
        let i = world
            .create_object_with_script_effects_checked(
                "SupplyWarehouse",
                Team::Neutral,
                Vec3::new(100., 0., 100.),
                None,
                &mut effects,
            )
            .unwrap()
            .expect("retail warehouse");
        assert!(world.set_host_object_name(i, "Depot"));
        depot_ids.push(i);
        if index == 0 {
            control_ids = Some([("SupplyDock", "Dock"), ("SupplyPile", "Pile")].map(
                |(template, name)| {
                    let id = world
                        .create_object_with_script_effects_checked(
                            template,
                            Team::Neutral,
                            Vec3::new(140., 0., 100.),
                            None,
                            &mut effects,
                        )
                        .unwrap()
                        .expect("retail supply control");
                    assert!(world.set_host_object_name(id, name));
                    id
                },
            ));
        }
        world.initialize_receiving_named_cache_for_fixture();
    }
    assert_eq!(depot_ids[0], depot_ids[1]);
    let id = depot_ids[0];
    // Only the second draw is W3DSupplyDraw. Start stock is 400 in both
    // worlds, but the warehouse constructor runs before Drawable admission.
    let read = |pipe: &RenderPipeline| pipe.observe_native_supply_draw(id, 1).unwrap();
    assert_eq!(read(&pipelines[0]).0, None);
    assert_eq!(read(&pipelines[0]).1.len(), 35);
    assert!(read(&pipelines[0]).2.is_empty());
    assert!(
        assets
            .lock()
            .unwrap()
            .cached_w3d_render_object_prototype("ZBSupplyWH_B")
            .is_none(),
        "the warehouse mesh must already draw before the pristine cache is admitted",
    );
    assert!(
        pipelines[0]
            .observe_native_supply_draw(id, 0)
            .unwrap()
            .2
            .is_empty()
    );

    for (receiver, boxes, shown) in [(0, 200, 18), (1, 100, 9), (0, 400, 35)] {
        let world = &mut worlds[receiver];
        let pipeline = &mut pipelines[receiver];
        {
            let (_, _, drawables) = clients[receiver].split_frame_snow_and_drawable_owner();
            let mut view = NativeRendererViewEffects::new(
                pipeline.borrow_native_installed_poses(),
                &mut graphics,
                drawables,
                EPOCH,
                0,
                receiving_native_drawable_policy(),
                &mut retirement,
            );
            let cash = boxes * world.supply_box_value();
            assert!(world.apply_warehouse_set_value_with_effects(
                "Depot",
                cash,
                &mut ScriptPresentationEffects {
                    view: Some(&mut view),
                    ..Default::default()
                },
            ));
        }
        let (latch, visible, writes) = read(pipeline);
        assert_eq!(latch, Some((35, shown)));
        assert!(
            assets
                .lock()
                .unwrap()
                .cached_w3d_render_object_prototype("ZBSupplyWH_B")
                .is_some(),
            "the first real supply status must resolve the exact authored prototype",
        );
        assert_eq!(visible.len(), shown as usize);
        assert!(visible.contains(&format!("SUP{shown:02}")));
        if shown != 35 {
            assert!(!visible.contains(&format!("SUP{:02}", shown + 1)));
            assert_eq!(writes.len(), (35 - shown) as usize);
        }
        assert_eq!(world.host_object(id).unwrap().warehouse_boxes(), boxes);
        if receiver == 1 {
            assert_eq!(read(&pipelines[0]).0, Some((35, 18)));
        }
    }
    assert_eq!(read(&pipelines[1]).0, Some((35, 9)));

    let [dock, pile] = control_ids.unwrap();
    for (name, id, module, boxes, total, shown) in
        [("Dock", dock, 1, 200, 64, 32), ("Pile", pile, 0, 75, 0, 0)]
    {
        let pipeline = &mut pipelines[0];
        assert!(
            pipeline
                .observe_native_supply_draw(id, module)
                .unwrap()
                .0
                .is_none()
        );
        let world = &mut worlds[0];
        {
            let (_, _, drawables) = clients[0].split_frame_snow_and_drawable_owner();
            let mut view = NativeRendererViewEffects::new(
                pipeline.borrow_native_installed_poses(),
                &mut graphics,
                drawables,
                EPOCH,
                0,
                receiving_native_drawable_policy(),
                &mut retirement,
            );
            assert!(world.apply_warehouse_set_value_with_effects(
                name,
                boxes * world.supply_box_value(),
                &mut ScriptPresentationEffects {
                    view: Some(&mut view),
                    ..Default::default()
                },
            ));
        }
        let (latch, visible, writes) = pipeline.observe_native_supply_draw(id, module).unwrap();
        assert_eq!(latch, Some((total, shown)));
        assert_eq!(visible.len(), shown as usize);
        assert_eq!(writes.len(), (total - shown) as usize);
    }

    // A native snapshot replaces an already drawn SupplyDock at the same ID
    // with a SupplyWarehouse. The peer keeps its own 35-child, 9-shown latch.
    let mut sender = GameLogic::new();
    stock.admit(&mut sender).unwrap();
    sender.add_player(Player::new(0, Team::USA, "Local", true));
    let saved_id = sender
        .create_object("SupplyWarehouse", Team::Neutral, Vec3::new(160., 0., 100.))
        .unwrap();
    let full_id = sender
        .create_object("SupplyWarehouse", Team::Neutral, Vec3::new(200., 0., 100.))
        .unwrap();
    assert_eq!(saved_id, id);
    assert!(sender.set_host_object_name(saved_id, "SavedDepot"));
    sender.initialize_receiving_named_cache_for_fixture();
    assert!(sender.apply_warehouse_set_value("SavedDepot", 200 * sender.supply_box_value()));
    let builder = crate::save_load::SnapshotBuilder::new();
    let snapshot = builder.create_world_snapshot(&sender).unwrap();

    for client_xfer in [false, true] {
        let mut staged = GameLogic::new();
        stock.admit(&mut staged).unwrap();
        staged.add_player(Player::new(0, Team::USA, "Local", true));
        let mut candidate = super::map_native_admission::NativeMapCandidate::with_epoch(EPOCH);
        let mut attempt = candidate
            .attempt(
                pipelines[0].borrow_native_draw_services(),
                &mut graphics,
                0,
                receiving_native_drawable_policy(),
                &mut retirement,
                None,
            )
            .unwrap();
        let outgoing = staged
            .create_object_with_script_effects_checked(
                "SupplyDock",
                Team::Neutral,
                Vec3::new(80., 0., 100.),
                None,
                &mut attempt.effects(),
            )
            .unwrap()
            .unwrap();
        assert_eq!(outgoing, id);
        let old = staged.current_native_slave_source(id, 0).unwrap();
        assert_eq!(old.template_name, "SupplyDock");
        staged
            .clear_all_objects_with_effects_checked(&mut attempt.effects())
            .unwrap();
        staged.begin_saved_snapshot_admission().unwrap();
        builder.restore_from_snapshot(&snapshot, &mut staged).unwrap();
        let incoming = staged.current_native_slave_source(id, 0).unwrap();
        assert_ne!(incoming.identity, old.identity);
        assert_eq!(incoming.template_name, "SupplyWarehouse");
        attempt.admit_restored_object_drawables(&mut staged).unwrap();
        if !client_xfer {
            staged
                .post_process_restored_supply_warehouses(&mut attempt.effects())
                .unwrap();
        }
        drop(attempt);
        let mut receiver = GameClient::new().unwrap();
        candidate
            .install_owned_worlds(&mut receiver, &mut pipelines[0])
            .map_err(|(reason, _)| reason)
            .unwrap();

        if client_xfer {
            // The attached fresh Drawable still has every native child and no
            // supply latch. Importing its real Client payload retires its
            // binding and leaves the next key unadopted until postprocess.
            assert_eq!(read(&pipelines[0]).0, None);
            assert!(read(&pipelines[0]).2.is_empty());
            let bytes = receiver.capture_xfer_bytes_with_owned_auxiliaries(&[]).unwrap();
            let pre_xfer = receiver
                .presentation_direct_drawable_state(EPOCH, id.0)
                .unwrap()
                .binding_key;
            let policy = super::native_view_effects::receiving_native_drawable_policy_for(&staged);
            let (content, colors) = staged.drawable_restore_inputs(false);
            receiver
                .restore_from_xfer_bytes_with_receiving_content_and_indicator_colors(
                    &bytes, content, policy, colors,
                )
                .unwrap();
            assert!(receiver.presentation_direct_drawable_state(EPOCH, id.0).is_none());
            let (_, _, drawables) = receiver.split_frame_snow_and_drawable_owner();
            let mut view = NativeRendererViewEffects::new(
                pipelines[0].borrow_native_installed_poses(),
                &mut graphics,
                drawables,
                EPOCH,
                0,
                policy,
                &mut retirement,
            );
            view.adopt_saved_supply_warehouse_drawables(&staged).unwrap();
            staged
                .post_process_restored_supply_warehouses(&mut ScriptPresentationEffects {
                    view: Some(&mut view),
                    ..Default::default()
                })
                .unwrap();
            assert_ne!(
                receiver.presentation_direct_drawable_state(EPOCH, id.0).unwrap().binding_key,
                pre_xfer
            );
        }

        let (latch, visible, writes) = read(&pipelines[0]);
        assert_eq!(latch, Some((35, 18)));
        assert_eq!(visible.len(), 18);
        assert_eq!(writes.len(), 17);
        assert_eq!(staged.host_object(id).unwrap().warehouse_boxes(), 200);
        let full = pipelines[0].observe_native_supply_draw(full_id, 1).unwrap();
        assert_eq!(full.0, Some((35, 35)));
        assert_eq!(full.1.len(), 35);
        assert!(full.2.is_empty());
        assert_eq!(staged.host_object(full_id).unwrap().warehouse_boxes(), 400);
        assert_eq!(read(&pipelines[1]).0, Some((35, 9)));
        assert_eq!(worlds[1].host_object(id).unwrap().warehouse_boxes(), 100);
    }

    // Use real warehouse and dock prototypes to distinguish the active
    // RenderObj from the logical CARRYING selection on the same Drawable.
    let mut swapping_world = GameLogic::new();
    swapping_world.admit_object_ini_sources([(
        "supply-swap.ini".into(),
        r#"
Object SupplySwapWarehouse
    KindOf = STRUCTURE SUPPLY_SOURCE
    Body = ActiveBody BodyTag
        MaxHealth = 100
        InitialHealth = 100
    End
    Behavior = SupplyWarehouseDockUpdate DockTag
        StartingBoxes = 400
    End
    Draw = W3DSupplyDraw SupplyDrawTag
        DefaultConditionState
            Model = ZBSupplyDK_B
        End
        ConditionState = CARRYING
            Model = ZBSupplyWH_B
        End
        ConditionState = CARRYING USER_1
            Model = ZBSupplyWH_B
            ShowSubObject = ZBSupplyWH_B.SUP19
        End
        ExtraPublicBone = SUP
        SupplyBonePrefix = SUP
    End
End
"#.into(),
    )]).unwrap();
    swapping_world.add_player(Player::new(0, Team::USA, "Local", true));
    let mut swapping_client = GameClient::new().unwrap();
    // Keep the actual app asset service and native WGPU owner. Retire existing
    // test bindings before this separate scenario reuses the same engine.
    pipelines[0].install_native_draw_world(Default::default());
    let pipeline = &mut pipelines[0];
    let swapping_id = {
        let (_, _, drawables) = swapping_client.split_frame_snow_and_drawable_owner();
        let mut view = NativeRendererViewEffects::new(
            pipeline.borrow_native_installed_poses(),
            &mut graphics,
            drawables,
            EPOCH,
            0,
            receiving_native_drawable_policy(),
            &mut retirement,
        );
        swapping_world
            .create_object_with_script_effects_checked(
                "SupplySwapWarehouse",
                Team::Neutral,
                Vec3::new(100., 0., 100.),
                None,
                &mut ScriptPresentationEffects {
                    view: Some(&mut view),
                    ..Default::default()
                },
            )
            .unwrap()
            .unwrap()
    };
    let observe = |pipeline: &RenderPipeline| pipeline.observe_native_supply_draw(swapping_id, 0).unwrap();
    assert!(observe(pipeline).0.is_none());
    assert_eq!(observe(pipeline).1.len(), 64);
    let status = |boxes: i32,
                  world: &mut GameLogic,
                  client: &mut GameClient,
                  pipeline: &mut RenderPipeline,
                  graphics: &mut GraphicsSystem,
                  retirement: &mut game_client::core::game_client::NativeParticleRetirementService| {
        let (_, _, drawables) = client.split_frame_snow_and_drawable_owner();
        let mut view = NativeRendererViewEffects::new(
            pipeline.borrow_native_installed_poses(),
            graphics,
            drawables,
            EPOCH,
            0,
            receiving_native_drawable_policy(),
            retirement,
        );
        assert!(world.apply_warehouse_set_value_to_object_with_effects(
            swapping_id,
            boxes * world.supply_box_value(),
            &mut ScriptPresentationEffects {
                view: Some(&mut view),
                ..Default::default()
            }
        ));
    };
    status(200, &mut swapping_world, &mut swapping_client, pipeline, &mut graphics, &mut retirement);
    let (latch, visible, writes) = observe(pipeline);
    assert_eq!(latch, Some((35, 18)));
    assert_eq!(visible.len(), 18);
    assert_eq!(writes.len(), 17, "first latch edits the new 35-child RenderObj");
    assert!(visible.contains(&"SUP18".to_owned()));
    assert!(!visible.contains(&"SUP19".to_owned()));

    let carrying = swapping_world.current_native_slave_source(swapping_id, 0).unwrap();
    use game_engine::common::bit_flags::ModelConditionFlags;
    let carry = 1u128 << ModelConditionFlags::CARRYING;
    let user = 1u128 << ModelConditionFlags::USER_1;
    assert_ne!(carrying.condition_bits & carry, 0, "live projection preserves the receiving bit");
    for bits in [carrying.condition_bits | user, carrying.condition_bits] {
        let mut poses = pipeline.borrow_native_installed_poses();
        poses.mark_receiving_drawable_conditions(&carrying, bits, &mut graphics);
        poses.consume_native_camera_conditions(swapping_id, &mut graphics);
        let (latch, visible, writes) = observe(pipeline);
        assert_eq!(latch, Some((35, 18)));
        assert_eq!(visible.len(), 19);
        assert!(visible.contains(&"SUP19".to_owned()));
        assert!(!visible.contains(&"SUP20".to_owned()));
        assert_eq!(writes.len(), 17, "full/leaf alias keeps just one latest target");
    }

    status(0, &mut swapping_world, &mut swapping_client, pipeline, &mut graphics, &mut retirement);
    let (latch, visible, writes) = observe(pipeline);
    assert_eq!(latch, Some((35, 0)));
    assert_eq!(visible, ["SUP19"], "a later delta does not touch SUP19");
    assert_eq!(writes.len(), 35, "later decrement still edits the old 35-child RenderObj");
    pipeline.borrow_native_installed_poses().consume_native_camera_conditions(swapping_id, &mut graphics);
    let (latch, visible, writes) = observe(pipeline);
    assert_eq!(latch, Some((35, 0)));
    assert_eq!(visible.len(), 64);
    assert!(writes.is_empty(), "replacement inherits no writes from the retired RenderObj");
}
