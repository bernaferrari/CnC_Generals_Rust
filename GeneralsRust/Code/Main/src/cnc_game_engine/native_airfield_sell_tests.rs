//! BuildAssistant1503-1560 / ParkingPlaceBehavior614-640 native phase acceptance.
//! Real Client constructors and the strict WGPU receiver, with synthetic art.
use super::*;

const RACK: &str = "NativeSellRack540";
const JET: &str = "NativeSellAircraft540";
const UNRELATED: &str = "NativeSellUnrelated540";

fn airfield_sell_world() -> GameLogic {
    let text = r#"
Object NativeSellRack540
  KindOf = STRUCTURE SELECTABLE FS_AIRFIELD
  Body = ActiveBody BodyTag
    MaxHealth = 1000
    InitialHealth = 1000
  End
  Behavior = ParkingPlaceBehavior ParkingTag
    NumRows = 1
    NumCols = 1
    HasRunways = Yes
    ParkInHangars = No
  End
  Draw = W3DModelDraw DrawTag
    DefaultConditionState
      Model = CameraModel
    End
  End
End
Object NativeSellAircraft540
  KindOf = VEHICLE AIRCRAFT SELECTABLE
  Body = ActiveBody BodyTag
    MaxHealth = 100
    InitialHealth = 100
  End
  Behavior = JetAIUpdate AiTag
    NeedsRunway = Yes
  End
  Behavior = InstantDeathBehavior DeathTag
  End
  Draw = W3DModelDraw DrawTag
    DefaultConditionState
      Model = CameraModel
    End
  End
End
Object NativeSellUnrelated540
  KindOf = VEHICLE
  Body = ActiveBody BodyTag
    MaxHealth = 40
    InitialHealth = 40
  End
  Behavior = DestroyDie DeleteTag
  End
  Draw = W3DModelDraw DrawTag
    DefaultConditionState
      Model = CameraModel
    End
  End
End
"#;
    let mut world = GameLogic::new();
    world
        .admit_receiving_module_content(
            crate::assets::ReceivingObjectContent::from_sources([(
                "native-airfield-sale.ini".to_owned(),
                text.to_owned(),
            )])
            .unwrap(),
        )
        .unwrap();
    world.add_player(crate::game_logic::Player::new(
        0,
        Team::USA,
        "NativeParkingSeller",
        true,
    ));
    world.require_native_drawable_owner();
    world.frame = 40;
    world
}

#[test]
#[ignore = "requires native WGPU; explicit isolated airfield sale phase acceptance"]
fn actual_airfield_sale_runs_body_die_before_native_retirement_and_checks_the_endphase_owner() {
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
    let mut world = airfield_sell_world();
    let mut peer = airfield_sell_world();
    let rack_position = Vec3::new(100., 12., 100.);
    let jet_position = Vec3::new(120., 12., 100.);
    let rack = birth_physical_retirement_actor(
        &mut world,
        RACK,
        rack_position,
        &mut client,
        &mut pipeline,
        &mut graphics,
        EPOCH,
    );
    let jet = birth_physical_retirement_actor(
        &mut world,
        JET,
        jet_position,
        &mut client,
        &mut pipeline,
        &mut graphics,
        EPOCH,
    );
    let unrelated = birth_physical_retirement_actor(
        &mut world,
        UNRELATED,
        Vec3::new(160., 12., 100.),
        &mut client,
        &mut pipeline,
        &mut graphics,
        EPOCH,
    );
    let peer_rack = birth_physical_retirement_actor(
        &mut peer,
        RACK,
        rack_position + Vec3::X * 300.,
        &mut peer_client,
        &mut peer_pipeline,
        &mut graphics,
        EPOCH + 1,
    );
    let peer_jet = birth_physical_retirement_actor(
        &mut peer,
        JET,
        jet_position + Vec3::X * 300.,
        &mut peer_client,
        &mut peer_pipeline,
        &mut graphics,
        EPOCH + 1,
    );
    assert_eq!((rack, jet), (peer_rack, peer_jet));
    for (owner, airfield, aircraft) in [(&mut world, rack, jet), (&mut peer, peer_rack, peer_jet)] {
        let object = owner.objects.get_mut(&aircraft).unwrap();
        object.ground_height = 12.;
        object.ground_height_from_terrain = true;
        object.health.current = 35.;
        assert!(!object.is_above_terrain());
        // This is the actual source parking reservation operation, not a raw
        // contained_by assignment, guessed name, or fabricated reservation map.
        assert!(owner.reserve_produced_aircraft_parking_space_for_test(airfield, aircraft));
        let object = &owner.objects[&aircraft];
        assert_eq!(object.owner_player_id, Some(0));
        assert_eq!(object.producer_id, Some(airfield));
        assert_eq!(object.airfield_parking_space_index, Some(0));
    }
    let peer_source = peer.current_native_slave_source(peer_jet, 0).unwrap();
    let peer_key = with_physical_retirement_view(
        &mut peer_client,
        &mut peer_pipeline,
        &mut graphics,
        EPOCH + 1,
        |runtime| {
            runtime
                .drawables
                .binding_key(EPOCH + 1, peer_jet.0)
                .unwrap()
        },
    );
    with_physical_retirement_view(
        &mut client,
        &mut pipeline,
        &mut graphics,
        EPOCH,
        |runtime| {
            let rack_key = runtime.drawables.binding_key(EPOCH, rack.0).unwrap();
            let jet_key = runtime.drawables.binding_key(EPOCH, jet.0).unwrap();
            let unrelated_key = runtime.drawables.binding_key(EPOCH, unrelated.0).unwrap();
            let source = world.current_native_slave_source(jet, 0).unwrap();
            assert_eq!(
                pose(runtime.sample_slave(&source, "CAMERA"))
                    .w_axis
                    .truncate(),
                jet_position
            );
            world.destroy_object_with_effects(
                unrelated,
                &mut crate::game_logic::ScriptPresentationEffects {
                    view: Some(&mut *runtime),
                    ..Default::default()
                },
            );
            assert!(world.start_sell_object_with_effects(
                rack,
                &mut crate::game_logic::ScriptPresentationEffects {
                    view: Some(&mut *runtime),
                    ..Default::default()
                },
            ));
            let object = &world.objects[&jet];
            assert_eq!(
                object.health.current, 0.,
                "sale must use Body kill, not direct destroy"
            );
            assert!(object.status.effectively_dead);
            assert!(object.status.on_die_started);
            assert!(
                object.status.destroyed,
                "authored InstantDeath queued removal"
            );
            assert_eq!(object.last_damage_timestamp, Some(40));
            assert_eq!(
                object.last_damage_source, None,
                "Object::kill has INVALID_ID source"
            );
            assert_eq!(
                object.last_damage_info_type,
                Some(crate::game_logic::combat::DamageType::Unresistable)
            );
            assert_eq!(
                object.status.death_type,
                crate::game_logic::host_usa_pilot::HostDeathType::Normal
            );
            assert_eq!(object.get_position(), jet_position);
            assert_eq!(world.objects[&unrelated].health.current, 40.);
            assert!(!world.objects[&unrelated].status.on_die_started);
            let dead_source = world.current_native_slave_source(jet, 0).unwrap();
            assert_eq!(
                pose(runtime.sample_slave(&dead_source, "CAMERA"))
                    .w_axis
                    .truncate(),
                jet_position
            );
            let mut shadow = crate::gameworld_shadow::GameWorldShadow::new(16);
            crate::gameworld_shadow::run_owned_host_boundary(Some(&mut shadow), &mut world);
            crate::gameworld_shadow::run_owned_host_boundary(None, &mut world);
            for (id, key) in [(jet, jet_key), (unrelated, unrelated_key)] {
                assert!(world.objects.contains_key(&id));
                assert!(world.objects.ordered_ids().contains(&id));
                assert!(shadow.entity_for_host(id).is_some());
                assert_eq!(runtime.drawables.binding_key(EPOCH, id.0), Some(key));
                assert!(runtime.poses.has_drawable_lifetime(id.0));
            }
            assert!(world.has_pending_object_removals());
            assert_eq!(peer.objects[&peer_jet].health.current, 35.);
            assert!(!peer.objects[&peer_jet].status.on_die_started);
            assert!(!peer.objects[&peer_rack].status.sold);
            world
                .process_destroy_list_with_effects_checked(
                    &mut crate::game_logic::ScriptPresentationEffects {
                        view: Some(&mut *runtime),
                        ..Default::default()
                    },
                )
                .unwrap();
            crate::gameworld_shadow::run_owned_host_boundary(Some(&mut shadow), &mut world);
            for id in [jet, unrelated] {
                assert!(!world.objects.contains_key(&id));
                assert!(!world.objects.ordered_ids().contains(&id));
                assert!(shadow.entity_for_host(id).is_none());
                assert!(runtime.drawables.binding_key(EPOCH, id.0).is_none());
                assert!(!runtime.poses.has_drawable_lifetime(id.0));
            }
            assert!(matches!(
                runtime.sample_slave(&dead_source, "CAMERA"),
                SlaveCameraSample::MissingDrawable
            ));
            assert_eq!(runtime.drawables.binding_key(EPOCH, rack.0), Some(rack_key));
            assert!(runtime.poses.has_drawable_lifetime(rack.0));
            assert!(!world.has_pending_object_removals());
            world.destroy_object_with_effects(
                rack,
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
            assert!(world.objects.is_empty());
            assert!(runtime.drawables.binding_key(EPOCH, rack.0).is_none());
            assert!(!runtime.poses.has_drawable_lifetime(rack.0));
        },
    );
    with_physical_retirement_view(
        &mut peer_client,
        &mut peer_pipeline,
        &mut graphics,
        EPOCH + 1,
        |runtime| {
            assert_eq!(
                runtime.drawables.binding_key(EPOCH + 1, peer_jet.0),
                Some(peer_key)
            );
            assert_eq!(
                pose(runtime.sample_slave(&peer_source, "CAMERA"))
                    .w_axis
                    .truncate(),
                jet_position + Vec3::X * 300.
            );
            assert_eq!(peer.objects[&peer_jet].health.current, 35.);
            assert!(!peer.has_pending_object_removals());
            assert!(peer.start_sell_object_with_effects(
                peer_rack,
                &mut crate::game_logic::ScriptPresentationEffects {
                    view: Some(&mut *runtime),
                    ..Default::default()
                },
            ));
            assert_eq!(peer.objects[&peer_jet].health.current, 0.);
            assert_eq!(
                runtime.drawables.binding_key(EPOCH + 1, peer_jet.0),
                Some(peer_key)
            );
            assert!(runtime.poses.has_drawable_lifetime(peer_jet.0));
        },
    );
    // Missing the lexical native owner is a checked terminal failure, never a
    // successful CPU-only delete that silently strands or erases the Draw owner.
    let error = peer
        .process_destroy_list_with_effects_checked(&mut Default::default())
        .unwrap_err();
    let gamelogic::GameLogicError::Configuration(reason) = error else {
        panic!("expected typed native retirement owner error");
    };
    assert!(reason.contains("requires its native Drawable owner"));
    assert!(!peer.objects.contains_key(&peer_jet));
    assert_eq!(
        peer.objects.failed_retirement_object().unwrap().id,
        peer_jet
    );
    with_physical_retirement_view(
        &mut peer_client,
        &mut peer_pipeline,
        &mut graphics,
        EPOCH + 1,
        |runtime| {
            assert_eq!(
                runtime.drawables.binding_key(EPOCH + 1, peer_jet.0),
                Some(peer_key)
            );
            assert!(runtime.poses.has_drawable_lifetime(peer_jet.0));
            assert_eq!(
                pose(runtime.sample_slave(&peer_source, "CAMERA"))
                    .w_axis
                    .truncate(),
                jet_position + Vec3::X * 300.
            );
            // Existing explicit native failed-attempt recovery releases real
            // resources; do not retry the terminal Object destructor.
            runtime.discard_map_attempt().unwrap();
            for id in [peer_rack, peer_jet] {
                assert!(runtime.drawables.binding_key(EPOCH + 1, id.0).is_none());
                assert!(!runtime.poses.has_drawable_lifetime(id.0));
            }
        },
    );
}
