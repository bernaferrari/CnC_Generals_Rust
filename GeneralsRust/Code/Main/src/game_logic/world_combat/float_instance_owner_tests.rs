//! Actual receiving World phase; recording view supplements real GameClient owner tests.
use super::*;
struct Receiving {
    identity: crate::assets::LiveDrawPlaybackIdentity,
    matrix: glam::Mat4,
    calls: Vec<&'static str>,
}
impl crate::game_logic::NativeViewEffects for Receiving {
    fn local_player_id(&self) -> u32 {
        0
    }
    fn sample_slave(
        &mut self,
        _: &crate::game_logic::NativeSlaveSource,
        _: &str,
    ) -> crate::presentation_frame::camera::build::SlaveCameraSample {
        crate::presentation_frame::camera::build::SlaveCameraSample::MissingObject
    }
    fn drawable_instance_transform(
        &mut self,
        source: &crate::game_logic::NativeSlaveSource,
    ) -> Result<Option<glam::Mat4>, String> {
        if source.identity != self.identity {
            return Err("foreign source".into());
        }
        self.calls.push("read");
        Ok(Some(self.matrix))
    }
    fn set_drawable_instance_transform(
        &mut self,
        source: &crate::game_logic::NativeSlaveSource,
        matrix: Option<glam::Mat4>,
    ) -> Result<bool, String> {
        if source.identity != self.identity {
            return Err("foreign source".into());
        }
        self.calls.push("set");
        self.matrix = matrix.unwrap_or(glam::Mat4::IDENTITY);
        Ok(true)
    }
    #[cfg(feature = "game_client")]
    fn host_epoch(&self) -> u64 {
        1
    }
    #[cfg(feature = "game_client")]
    fn retire_physical_object_drawable(
        &mut self,
        _: &crate::game_logic::NativeObjectRetirement<'_>,
    ) -> Result<(), String> {
        Err("recording view cannot retire".into())
    }
    #[cfg(feature = "game_client")]
    fn admit_factory_drawable(
        &mut self,
        _: crate::game_logic::NativeFactoryDrawableSource,
        _: game_client::core::game_client::NativeDrawableBirthContext,
        _: &mut game_engine::common::thing::module_factory::ModuleFactory,
    ) -> Result<(), String> {
        Err("recording view cannot create".into())
    }
}
fn fixture() -> (GameLogic, ObjectId) {
    let mut parser = crate::assets::IniParser::new();
    parser
        .parse_ini_content(
            "Object OwnedFloat
 Type = Vehicle
 Draw = W3DModelDraw ModuleTag_Draw
  DefaultConditionState
   Model = FloatModel
  End
 End
End
",
            "float-instance.ini",
        )
        .unwrap();
    let mut world = GameLogic::new();
    let template = GameLogic::build_template_from_object_definition(
        "OwnedFloat",
        parser.get_definition("OwnedFloat").unwrap(),
        None,
    );
    world.templates.insert("OwnedFloat".into(), template);
    let id = world
        .create_object("OwnedFloat", Team::USA, Vec3::new(3., 4., 5.))
        .unwrap();
    world.objects.get_mut(&id).unwrap().float_update =
        Some(crate::game_logic::host_float_update::HostFloatUpdateData::new(false));
    world.frame = 100;
    (world, id)
}
#[test]
fn source_float_update_uses_receiving_instance_and_no_same_id_peer() {
    let (mut world, id) = fixture();
    let (peer, other) = fixture();
    assert_eq!(id, other);
    let identity = world.live_draw_playback_identity(world.objects.get(&id).unwrap());
    let original = glam::Mat4::from_rotation_y(0.35);
    let mut recipient = Receiving {
        identity,
        matrix: original,
        calls: Vec::new(),
    };
    drop(peer);
    world
        .update_float_update_with_effects(&mut crate::game_logic::ScriptPresentationEffects {
            view: Some(&mut recipient),
            ..Default::default()
        })
        .unwrap();
    let data = world
        .objects
        .get(&id)
        .unwrap()
        .float_update
        .as_ref()
        .unwrap();
    assert_eq!(recipient.calls, vec!["read", "set"]);
    assert_eq!(
        recipient.matrix,
        crate::game_logic::host_float_update::rebuild_receiving_instance(
            original, data.yaw, data.pitch
        )
    );
}
#[test]
fn source_float_missing_receiving_birth_is_reported() {
    let (mut world, id) = fixture();
    let (peer, other) = fixture();
    let mut recipient = Receiving {
        identity: peer.live_draw_playback_identity(peer.objects.get(&other).unwrap()),
        matrix: glam::Mat4::IDENTITY,
        calls: Vec::new(),
    };
    assert!(
        world
            .update_float_update_with_effects(&mut crate::game_logic::ScriptPresentationEffects {
                view: Some(&mut recipient),
                ..Default::default()
            })
            .is_err()
    );
    assert!(recipient.calls.is_empty());
    assert!(world.objects.get(&id).unwrap().float_update.is_some());
}

fn water_map(root: &std::path::Path, name: &str, water_y: i32) -> std::path::PathBuf {
    use game_engine::common::system::DataChunkOutput;

    let directory = root.join(name);
    std::fs::create_dir(&directory).unwrap();
    let mut map = DataChunkOutput::new();
    // Omitting HeightMapData forces FloatUpdate to use the receiving Core
    // TerrainLogic fallback rather than the host-side TerrainData cache.
    map.open_data_chunk("PolygonTriggers", 4);
    map.write_int(1);
    map.write_ascii_string("FloatOwnerWater");
    map.write_ascii_string("FloatOwnerLayer");
    map.write_int(1);
    map.write_byte(1);
    map.write_byte(0);
    map.write_int(0);
    map.write_int(4);
    for (x, z) in [(0, 0), (200, 0), (200, 200), (0, 200)] {
        map.write_int(x);
        map.write_int(z);
        map.write_int(water_y);
    }
    map.close_data_chunk();
    map.open_data_chunk("ObjectsList", 3);
    map.close_data_chunk();
    let path = directory.join("FloatOwner.map");
    std::fs::write(&path, map.into_ckmp_bytes()).unwrap();
    path
}

fn water_fixture(path: &std::path::Path) -> (GameLogic, ObjectId) {
    use crate::game_logic::ThingTemplate;

    let mut world = GameLogic::new();
    world.start_new_game(crate::game_logic::GameMode::SinglePlayer);
    assert!(world.load_map(path.to_str().unwrap()));
    assert!(
        world.terrain.is_none(),
        "fixture must use the Core fallback"
    );
    world.templates.insert(
        "CivilianVehicleFerry".into(),
        ThingTemplate::new("CivilianVehicleFerry"),
    );
    let id = world
        .create_object(
            "CivilianVehicleFerry",
            crate::game_logic::Team::USA,
            glam::Vec3::new(100.0, 0.0, 100.0),
        )
        .unwrap();
    world.objects.get_mut(&id).unwrap().float_update =
        Some(crate::game_logic::host_float_update::HostFloatUpdateData::new(true));
    (world, id)
}

fn observed_y(world: &GameLogic, id: ObjectId) -> f32 {
    world.objects.get(&id).unwrap().get_position().y
}

#[test]
fn float_update_uses_receiving_water_when_foreign_core_services_are_published() {
    crate::game_logic::game_logic::pose_owner_tests::isolated_at(
        module_path!(),
        "float_update_uses_receiving_water_when_foreign_core_services_are_published",
        || {
            let temp = tempfile::tempdir().unwrap();
            let high_map = water_map(temp.path(), "high", 80);
            let low_map = water_map(temp.path(), "low", 30);
            let (mut high, high_id) = water_fixture(&high_map);
            let (mut low, low_id) = water_fixture(&low_map);
            assert_eq!(high_id, low_id, "same-ID worlds are required");

            let foreign = gamelogic::system::engine_stores::new_for_world();
            {
                let terrain_owner = foreign.services().terrain();
                let mut terrain = terrain_owner.write().unwrap();
                let mut water = gamelogic::polygon_trigger::PolygonTrigger::new(
                    99,
                    "ForeignFloatWater".into(),
                    vec![
                        gamelogic::common::ICoord3D::new(0, 0, 120),
                        gamelogic::common::ICoord3D::new(200, 0, 120),
                        gamelogic::common::ICoord3D::new(200, 200, 120),
                        gamelogic::common::ICoord3D::new(0, 200, 120),
                    ],
                );
                water.set_water_area(true);
                terrain.add_trigger_area(water);
            }

            // If FloatUpdate used `active_services()`, it would snap both
            // objects to this foreign 120-unit water surface. The receiving
            // Core TerrainLogic owners instead dictate 80 and 30 respectively.
            gamelogic::system::engine_stores::with_active_stores(&foreign, || {
                high.update_float_update();
                assert!((observed_y(&high, high_id) - 80.0).abs() < 0.01);
                assert_eq!(observed_y(&low, low_id), 0.0);

                low.update_float_update();
                assert!((observed_y(&low, low_id) - 30.0).abs() < 0.01);
                high.update_float_update();
                assert!((observed_y(&high, high_id) - 80.0).abs() < 0.01);

                low.reset();
                high.update_float_update();
                assert!((observed_y(&high, high_id) - 80.0).abs() < 0.01);
            });
        },
    );
}
