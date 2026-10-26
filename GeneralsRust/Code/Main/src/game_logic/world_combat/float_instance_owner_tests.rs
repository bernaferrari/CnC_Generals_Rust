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
