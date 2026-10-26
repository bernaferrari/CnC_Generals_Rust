//! Actual client ownership and original UI/Radar snapshot field continuations.
use game_client_rust::core::game_client::GameClient;
use game_client_rust::core::subsystems::{InGameUISubsystem, RadarPingEvent, RadarPingKind};
use game_client_rust::helpers::PendingCommand;
use game_client_rust::message_stream::game_message::ICoord2D;
use game_engine::{Snapshot, Xfer};
use gamelogic::commands::command::CommandType;

fn client(template: &str, message: &str) -> GameClient {
    let mut client = GameClient::new().expect("construct inert client");
    let mut ui = InGameUISubsystem::default();
    // Equal source IDs deliberately identify different objects in different clients.
    ui.place_build_available(Some(template.into()), Some(42));
    ui.message(message, 100);
    client.set_in_game_ui_subsystem(ui);
    client
}

#[test]
fn clients_keep_pending_input_and_messages_across_peer_reset_and_drop() {
    let mut first = client("OwnerFactory", "owner");
    let mut peer = client("PeerFactory", "peer");
    first
        .in_game_ui_subsystem_mut()
        .unwrap()
        .set_input_enabled(false);
    first.in_game_ui_subsystem_mut().unwrap().create_move_hint(
        game_client_rust::message_stream::game_message::Coord3D::new(0.0, 0.0, 0.0),
        game_client_rust::message_stream::game_message::Coord3D::new(7.0, 11.0, 0.0),
        42,
        5,
    );
    assert!(peer.in_game_ui_subsystem().unwrap().get_hints().is_empty());
    assert!(!first.in_game_ui_subsystem().unwrap().get_input_enabled());
    assert!(peer.in_game_ui_subsystem().unwrap().get_input_enabled());
    assert_eq!(
        first
            .in_game_ui_subsystem()
            .unwrap()
            .get_pending_place_source_object_id(),
        42
    );
    assert_eq!(
        peer.in_game_ui_subsystem()
            .unwrap()
            .get_pending_place_source_object_id(),
        42
    );

    peer.reset().expect("reset actual peer client");
    assert!(
        peer.in_game_ui_subsystem()
            .unwrap()
            .get_pending_place_template()
            .is_none()
    );
    drop(peer);
    let ui = first.in_game_ui_subsystem_mut().unwrap();
    assert_eq!(
        ui.get_pending_place_template().as_deref(),
        Some("OwnerFactory")
    );
    assert!(!ui.get_input_enabled());
    assert_eq!(ui.get_hints().len(), 1);
    assert_eq!(ui.get_hints()[0].source_id, 42);
    assert_eq!(
        (ui.get_hints()[0].end.x, ui.get_hints()[0].end.y),
        (7.0, 11.0)
    );
    ui.message("owner continues", 101);
    let messages = ui.drain_hud_messages();
    assert_eq!(messages, ["owner", "owner continues"]);
}

#[test]
fn ui_and_radar_snapshots_restore_only_the_borrowed_client_and_continue() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("receiving-ui.bin");
    let path = path.to_string_lossy().into_owned();
    let mut source = client("SavedFactory", "not part of the original UI chunk");
    let mut peer = client("PeerFactory", "peer remains");
    {
        let ui = source.in_game_ui_subsystem_mut().unwrap();
        ui.set_placement_start(Some(ICoord2D::new(5, 8)));
        ui.set_placement_end(Some(ICoord2D::new(13, 21)));
        ui.set_placement_angle(0.75);
        ui.set_force_move_to_mode(true);
        ui.set_pending_command_state(Some(PendingCommand {
            command_type: CommandType::DoMoveTo,
            options: 7,
            source_object_id: 42,
            cursor_name: "MOVETO".into(),
            invalid_cursor_name: "GENERIC_INVALID".into(),
            radius_cursor_type: "GUARD_AREA".into(),
        }));
        ui.push_radar_ping(RadarPingEvent {
            position: game_client_rust::message_stream::game_message::Coord3D::new(3.0, 4.0, 5.0),
            kind: RadarPingKind::Attack,
            age_seconds: 1.25,
        });
        let (mut ui_chunk, mut radar_chunk) = ui.snapshot_parts();
        let mut writer = game_engine::System::XferSave::new();
        writer.open(path.clone()).unwrap();
        ui_chunk.xfer(&mut writer).unwrap();
        radar_chunk.xfer(&mut writer).unwrap();
        writer.close().unwrap();
    }
    let mut restored = client("ReplacedFactory", "restored local HUD");
    {
        let ui = restored.in_game_ui_subsystem_mut().unwrap();
        let (mut ui_chunk, mut radar_chunk) = ui.snapshot_parts();
        let mut reader = game_engine::System::XferLoad::new();
        reader.open(path).unwrap();
        ui_chunk.xfer(&mut reader).unwrap();
        radar_chunk.xfer(&mut reader).unwrap();
        reader.close().unwrap();
    }
    let restored_ui = restored.in_game_ui_subsystem_mut().unwrap();
    assert_eq!(
        restored_ui.get_pending_place_template().as_deref(),
        Some("SavedFactory")
    );
    assert_eq!(restored_ui.get_pending_place_source_object_id(), 42);
    let (start, end) = restored_ui.get_placement_points().unwrap();
    assert_eq!((start.x, start.y, end.x, end.y), (5, 8, 13, 21));
    assert_eq!(restored_ui.get_placement_angle(), 0.75);
    assert!(restored_ui.is_in_force_move_to_mode());
    let command = restored_ui.get_pending_command().unwrap();
    assert_eq!(command.command_type, CommandType::DoMoveTo);
    assert_eq!(command.source_object_id, 42);
    assert_eq!(command.radius_cursor_type, "GUARD_AREA");
    let pings = restored_ui.drain_radar_pings();
    assert_eq!(pings.len(), 1);
    assert!(matches!(pings[0].kind, RadarPingKind::Attack));
    assert_eq!(
        (
            pings[0].position.x,
            pings[0].position.y,
            pings[0].position.z
        ),
        (3.0, 4.0, 5.0)
    );
    assert_eq!(pings[0].age_seconds, 1.25);
    // Original chunks do not contain the HUD messages; loading cannot adopt a peer's HUD.
    assert_eq!(restored_ui.drain_hud_messages(), ["restored local HUD"]);
    restored_ui.clear_pending_command();
    restored_ui.set_placement_angle(1.5);
    assert!(restored_ui.get_pending_command().is_none());
    assert_eq!(restored_ui.get_placement_angle(), 1.5);
    assert_eq!(
        source.in_game_ui_subsystem().unwrap().get_placement_angle(),
        0.75
    );
    assert_eq!(
        peer.in_game_ui_subsystem()
            .unwrap()
            .get_pending_place_template()
            .as_deref(),
        Some("PeerFactory")
    );
    assert_eq!(
        peer.in_game_ui_subsystem_mut()
            .unwrap()
            .drain_hud_messages(),
        ["peer remains"]
    );
}

#[test]
fn presentation_caption_deadlines_survive_peer_reset_without_rearming_owner() {
    let mut owner = client("OwnerFactory", "owner");
    let mut peer = client("PeerFactory", "peer");
    owner.apply_presentation_military_caption(Some("Owner caption"), Some(2500), 200);
    peer.apply_presentation_military_caption(Some("Peer caption"), Some(1000), 700);
    let ui = owner.in_game_ui_subsystem().unwrap();
    assert_eq!(
        ui.military_subtitles().back(),
        Some(&("Owner caption".into(), 2500))
    );
    assert!(ui.tooltips_disabled_at(274));
    assert!(!ui.tooltips_disabled_at(275));
    let ui = peer.in_game_ui_subsystem().unwrap();
    assert_eq!(
        ui.military_subtitles().back(),
        Some(&("Peer caption".into(), 1000))
    );
    assert!(ui.tooltips_disabled_at(729));
    assert!(!ui.tooltips_disabled_at(730));
    peer.reset().unwrap();
    drop(peer);
    let ui = owner.in_game_ui_subsystem().unwrap();
    assert_eq!(ui.military_subtitles().len(), 1);
    assert_eq!(
        ui.military_subtitles().back(),
        Some(&("Owner caption".into(), 2500))
    );
    // The same frozen caption with less time remaining must continue, not restart.
    owner.apply_presentation_military_caption(Some("Owner caption"), Some(1000), 245);
    let ui = owner.in_game_ui_subsystem().unwrap();
    assert_eq!(ui.military_subtitles().len(), 1);
    assert!(!ui.tooltips_disabled_at(275));
}

#[test]
fn native_translator_pipeline_does_not_repeat_main_camera_input() {
    use game_client_rust::core::game_client::{ClientInputAuthority, NativeWindowInputDriver};
    use game_client_rust::message_stream::{GameMessage, GameMessageType};
    use game_engine::common::message_stream::{WindowInputDispatch, WindowInputDispatchResult};
    let mut native =
        GameClient::new_with_input_authority(ClientInputAuthority::NativeHosted).unwrap();
    native.set_in_game_ui_subsystem(InGameUISubsystem::default());
    native.init_message_translators().unwrap();
    for message in [
        GameMessageType::RawMouseWheel(1),
        GameMessageType::FrameTick(1),
    ] {
        native
            .message_stream_mut()
            .append_message_value(GameMessage::new(message));
    }
    let mut camera_requests = 0;
    {
        let driver: &mut NativeWindowInputDriver<'_> =
            &mut |_, _, _, _, _, _, _, _, _, request, _| {
                if matches!(request, WindowInputDispatch::Camera(_)) {
                    camera_requests += 1;
                }
                None::<WindowInputDispatchResult>
            };
        native
            .take_message_stream_with_native_input(
                false,
                19,
                gamelogic::system::game_logic::GAME_SKIRMISH,
                driver,
            )
            .unwrap();
    }
    assert_eq!(
        camera_requests, 0,
        "Main's raw wheel and frame step must have one camera writer"
    );
}

#[test]
fn radar_draw_receipts_only_change_the_supplied_event_owner() {
    use game_engine::common::system::radar::{Coord3D, RadarEventType, RadarSystem};
    use std::sync::Arc;
    let mut first = RadarSystem::new();
    let mut peer = RadarSystem::new();
    let position = Coord3D::new(0.5, 0.5, 0.0);
    first.create_event(&position, RadarEventType::Information, 1.0);
    peer.create_event(&position, RadarEventType::Information, 1.0);
    let first_view = Arc::new(first.presentation());
    let peer_view = Arc::new(peer.presentation());
    let mut ui = InGameUISubsystem::default();
    ui.set_radar_presentation(first_view.clone());
    assert_eq!(ui.admit_radar_draw(&peer_view), 0);
    assert_eq!(ui.admit_radar_draw(&first_view), 1);
    assert_eq!(ui.admit_radar_draw(&first_view), 0);
    for (owner, events) in ui.take_radar_acknowledgments() {
        peer.acknowledge_presented_events(&owner, &events);
        first.acknowledge_presented_events(&owner, &events);
    }
    assert!(first.presentation().draw_events()[0].sound_played);
    assert!(!peer.presentation().draw_events()[0].sound_played);
}

mod control_bar_owner {
    use super::*;
    use game_client_rust::gui::callbacks::control_bar_callbacks::*;
    use game_client_rust::gui::control_bar::ControlBar;
    use game_client_rust::gui::{GameWindow, WindowMessage, WindowMsgHandled};
    struct OwnedViewport {
        display: (u32, u32),
        height: i32,
    }
    impl ControlBarViewport for OwnedViewport {
        fn display_size(&self) -> (u32, u32) {
            self.display
        }
        fn set_height_px(&mut self, height: i32) {
            self.height = height;
        }
    }
    #[test]
    fn client_callback_actions_are_independent() {
        let mut a = GameClient::new().unwrap();
        let mut b = GameClient::new().unwrap();
        let mut bar = ControlBar::new();
        let mut viewport = OwnedViewport {
            display: (800, 600),
            height: 600,
        };
        let first_id = a.with_window_event_effects(None, |effects| {
            effects.with_manager(|manager, effects| {
                let parent = manager
                    .create_window(None, 0, 0, 100, 100, effects)
                    .unwrap();
                hide_control_bar_owned(
                    &mut bar,
                    &mut viewport,
                    Some(parent.clone()),
                    true,
                    true,
                    manager,
                    effects,
                );
                assert!(parent.borrow().is_hidden());
                assert!(!residual_control_bar_is_visible(effects));
                assert_eq!(
                    residual_control_bar_last_action(effects),
                    ResidualControlBarAction::Hide
                );
                let id = parent.borrow().get_id();
                id
            })
        });
        let second_id = b.with_window_event_effects(None, |effects| {
            let id = effects.with_manager(|manager, effects| {
                let parent = manager
                    .create_window(None, 0, 0, 100, 100, effects)
                    .unwrap();
                parent.borrow_mut().hide(false, manager).unwrap();
                assert!(!parent.borrow().is_hidden());
                let id = parent.borrow().get_id();
                id
            });
            assert!(residual_control_bar_is_visible(effects));
            assert_eq!(
                residual_control_bar_last_action(effects),
                ResidualControlBarAction::None
            );
            assert!(
                !simulate_control_bar_options_button_gadget_selected(effects),
                "missing button cannot report a click"
            );
            id
        });
        assert_eq!(
            first_id, second_id,
            "identity collision does not select the peer callback state"
        );
        drop(b);
        a.with_window_event_effects(None, |effects| assert!(!is_control_bar_visible(effects)));
    }

    #[test]
    fn empty_callback_does_not_advance_the_control_bar_animation_phase() {
        let mut client = GameClient::new().unwrap();
        client.with_window_event_effects(None, |effects| {
            let mut window = GameWindow::new();
            assert_eq!(
                receiving_control_bar_system(&mut window, WindowMessage::None, 0, 0, effects),
                WindowMsgHandled::Ignored
            );
        });
    }
}

#[test]
fn chat_requests_change_only_the_receiving_client() {
    use game_client_rust::gui::callbacks::ingame_callbacks::*;
    let mut first = client("A", "A");
    let mut peer = client("B", "B");
    first.with_window_event_effects(None, |effects| {
        assert!(simulate_in_game_chat_set_text("first draft", effects));
        assert!(simulate_in_game_chat_set_type(2, effects));
    });
    peer.with_window_event_effects(None, |effects| {
        assert!(simulate_in_game_chat_set_text("peer draft", effects));
        assert!(simulate_in_game_chat_reset(effects));
    });
    assert_eq!(
        residual_in_game_chat_text(first.in_game_callbacks()),
        "first draft"
    );
    assert_eq!(
        residual_in_game_chat_type_ordinal(first.in_game_callbacks()),
        2
    );
    assert!(residual_in_game_chat_text(peer.in_game_callbacks()).is_empty());
    drop(peer);
    assert_eq!(
        residual_in_game_chat_text(first.in_game_callbacks()),
        "first draft"
    );
}
