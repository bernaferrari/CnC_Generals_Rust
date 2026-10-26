//! Native raw events already have one receiving camera owner in Main.
use super::*;
use game_engine::common::message_stream::{WindowInputDispatch, WindowInputDispatchResult};

#[test]
fn native_pipeline_does_not_repeat_raw_camera_actions_through_core_look_at() {
    let mut native =
        GameClient::new_with_input_authority(ClientInputAuthority::NativeHosted).unwrap();
    let mut core = GameClient::new().unwrap();
    native.init_message_translators().unwrap();
    core.init_message_translators().unwrap();
    assert_eq!(native.num_translators + 1, core.num_translators);

    for message in [
        GameMessageType::RawMouseWheel(1),
        GameMessageType::FrameTick(1),
    ] {
        native
            .message_stream
            .append_message_value(GameMessage::new(message));
    }
    let mut ui = InGameUISubsystem::default();
    let mut camera_requests = 0;
    let mut dispatch = |request, _: &mut Vec<GameMessage>, _: &mut dyn std::any::Any| {
        if matches!(request, WindowInputDispatch::Camera(_)) {
            camera_requests += 1;
        }
        None::<WindowInputDispatchResult>
    };
    native
        .message_stream
        .propagate_messages_with_context(&mut dispatch, &mut ui)
        .unwrap();
    assert_eq!(
        camera_requests, 0,
        "native wheel/frame already belongs to Main camera input"
    );
    assert_eq!(core.input_authority, ClientInputAuthority::StandaloneCore);
}
