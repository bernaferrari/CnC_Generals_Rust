//! Actual authored action walk carries the receiving renderer loan synchronously.
use super::super::script_execution_driver::HostScriptExecutionDriver;
use super::*;
use crate::game_logic::{NativeSlaveSource, NativeViewEffects, ScriptPresentationEffects};
use crate::presentation_frame::camera::build::SlaveCameraSample;
struct ReceivingView {
    calls: Vec<(crate::assets::LiveDrawPlaybackIdentity, u128, String)>,
    translation: Vec3,
}
impl NativeViewEffects for ReceivingView {
    #[cfg(feature = "game_client")]
    fn retire_object_drawable(
        &mut self,
        _: &crate::assets::LiveDrawPlaybackIdentity,
    ) -> Result<(), String> {
        Err("camera-only fixture cannot physically retire native Drawables".into())
    }
    #[cfg(feature = "game_client")]
    fn host_epoch(&self) -> u64 {
        1
    }
    #[cfg(feature = "game_client")]
    fn admit_factory_drawable(
        &mut self,
        _: crate::game_logic::NativeFactoryDrawableSource,
        _: game_client::core::game_client::NativeDrawableBirthContext,
        _: &mut game_engine::common::thing::module_factory::ModuleFactory,
    ) -> Result<(), String> {
        Err("camera-only fixture cannot construct native Drawables".into())
    }

    fn local_player_id(&self) -> u32 {
        0
    }
    fn sample_slave(&mut self, source: &NativeSlaveSource, bone: &str) -> SlaveCameraSample {
        self.calls
            .push((source.identity.clone(), source.condition_bits, bone.into()));
        SlaveCameraSample::Pose(glam::Mat4::from_translation(self.translation))
    }
}
fn install_subject(world: &mut GameLogic, moving: bool) {
    let mut parser = crate::assets::IniParser::new();
    parser.parse_ini_content("Object CameraSubject\n Draw = W3DModelDraw ModuleTag_Draw\n DefaultConditionState\n Model = SubjectModel\n End\n End\nEnd\n","view_effect_owner.ini").unwrap();
    let definition = parser.get_definition("CameraSubject").unwrap();
    let mut template = ThingTemplate::new("CameraSubject");
    template.set_authored_draw_models(definition.model_name.as_deref(), &definition.draw_modules);
    let mut subject = crate::game_logic::Object::new(template, ObjectId(42), Team::USA);
    subject.name = "ReceivingCamera".into();
    subject.status.moving = moving;
    world.add_object(subject);
}
fn enable() -> Box<ScriptAction> {
    action(
        ScriptActionType::CameraEnableSlaveMode,
        vec![
            Parameter::with_string(ParameterType::TextString, "ReceivingCamera".into()),
            Parameter::with_string(ParameterType::TextString, "Eye".into()),
        ],
    )
}
#[test]
fn authored_enable_slave_then_home_samples_current_receiving_owner_before_walk_returns() {
    let (mut owner, _, _) = world(120.);
    let (mut peer, _, _) = world(320.);
    install_subject(&mut owner, true);
    install_subject(&mut peer, false);
    let peer_before = peer.capture_camera_continuation().unwrap();
    let expected = owner.current_native_slave_source(ObjectId(42), 0).unwrap();
    let foreign = peer.current_native_slave_source(ObjectId(42), 0).unwrap();
    assert_ne!(expected.condition_bits, foreign.condition_bits);
    let mut list = ScriptList::new();
    list.add_script(
        script(
            "Owned",
            vec![
                enable(),
                Box::new(ScriptAction::new(ScriptActionType::CameraMoveHome)),
            ],
            true,
        ),
        0,
    );
    let interpreter = engine(Arc::clone(&peer.mission_scripts), list);
    let mut backend = ReceivingView {
        calls: Vec::new(),
        translation: Vec3::new(190., 290., 390.),
    };
    interpreter
        .update_with_driver(
            gamelogic::scripting::executor::ScriptContext::new(),
            &mut HostScriptExecutionDriver::with_effects(
                &mut owner,
                &interpreter,
                ScriptPresentationEffects {
                    audio: None,
                    terrain: None,
                    ui: None,
                    view: Some(&mut backend),
                },
            ),
        )
        .unwrap();
    assert_eq!(
        backend.calls.len(),
        2,
        "invalid cache performs both original build queries in this action"
    );
    assert!(
        backend
            .calls
            .iter()
            .all(|(identity, bits, bone)| identity == &expected.identity
                && *bits == expected.condition_bits
                && bone == "Eye")
    );
    assert_eq!(owner.session_view.target, backend.translation);
    assert_eq!(
        owner.session_view.camera.camera_position,
        backend.translation
    );
    assert_eq!(peer.capture_camera_continuation().unwrap(), peer_before);
    assert!(
        peer.mission_scripts
            .drain_camera_slave_mode_enable_requests()
            .is_empty()
    );
}
#[test]
fn missing_native_effects_is_an_error_before_mutating_the_receiving_view() {
    let (mut owner, _, _) = world(120.);
    install_subject(&mut owner, false);
    owner.queue_pending_camera_slave_enable("ReceivingCamera", "Eye");
    let before = owner.capture_camera_continuation().unwrap();
    assert!(
        owner
            .request_camera_focus_with_effects(
                Vec3::new(10., 0., 20.),
                &mut ScriptPresentationEffects::default()
            )
            .is_err()
    );
    assert_eq!(owner.capture_camera_continuation().unwrap(), before);
}

#[test]
fn actual_radar_command_borrows_receiving_slave_view_without_peer_delivery() {
    let (mut owner, _, _) = world(120.);
    let (mut peer, _, _) = world(320.);
    install_subject(&mut owner, true);
    install_subject(&mut peer, false);
    owner.queue_pending_camera_slave_enable("ReceivingCamera", "Eye");
    let peer_before = peer.capture_camera_continuation().unwrap();
    let expected = owner.current_native_slave_source(ObjectId(42), 0).unwrap();
    let mut receiving = ReceivingView {
        calls: Vec::new(),
        translation: Vec3::new(190., 290., 390.),
    };
    let foreign = ReceivingView {
        calls: Vec::new(),
        translation: Vec3::new(790., 890., 990.),
    };
    let command = crate::command_system::GameCommand {
        command_type: crate::command_system::CommandType::ViewRadarAt {
            position: Vec3::new(10., 0., 20.),
        },
        player_id: 0,
        command_id: 0,
        timestamp: std::time::SystemTime::now(),
        selected_units: Vec::new(),
        modifier_keys: Default::default(),
    };
    let result = crate::command_executor::CommandExecutor::new_with_effects(
        &mut owner,
        0,
        ScriptPresentationEffects {
            audio: None,
            terrain: None,
            ui: None,
            view: Some(&mut receiving),
        },
    )
    .execute_command(command)
    .unwrap();
    assert_eq!(result, crate::command_system::CommandResult::Success);
    assert!(
        !receiving.calls.is_empty(),
        "actual radar LookAt must sample its native receiver synchronously"
    );
    assert!(
        receiving
            .calls
            .iter()
            .all(|(identity, bits, bone)| identity == &expected.identity
                && *bits == expected.condition_bits
                && bone == "Eye")
    );
    assert_eq!(owner.session_view.target, receiving.translation);
    assert!(foreign.calls.is_empty());
    assert_eq!(peer.capture_camera_continuation().unwrap(), peer_before);
}
