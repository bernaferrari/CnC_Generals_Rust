//! Exact UI origin/identity forwarding, not physical-device or completed-gameplay evidence.
use game_client_rust::core::game_client::{ClientInputAuthority, GameClient};
use game_client_rust::core::subsystems::InGameUISubsystem;
use game_client_rust::gui::callbacks::control_bar_callbacks::{
    ControlBarViewport, receiving_control_bar_system,
};
use game_client_rust::gui::control_bar::{
    CommandButton, ControlBar, HostControlBarInputProvenance as Origin, HostControlBarRequest,
    take_host_control_bar_published_requests,
};
use game_client_rust::gui::gadgets::PushButton;
use game_client_rust::gui::game_window::{WindowInputReturnCode, WindowMsgHandled};
use game_client_rust::gui::gui_callbacks::control_bar_popup_description::UpgradeTooltipSource;
use game_client_rust::gui::window_manager::{ControlBarInputContext, WindowManager};
use game_client_rust::gui::{GameWindow, WindowMessage, WindowWidget};
use game_client_rust::input::Mouse;
use game_engine::common::name_key_generator::NameKeyGenerator;
use gamelogic::commands::CommandType;
use gamelogic::system::engine_stores::{new_world_services, with_world_services};
use std::{cell::RefCell, rc::Rc};
type Window = Rc<RefCell<GameWindow>>;
struct Viewport;
impl ControlBarViewport for Viewport {
    fn display_size(&self) -> (u32, u32) {
        (200, 200)
    }
    fn set_height_px(&mut self, _height: i32) {}
}
fn id(name: &str) -> i32 {
    NameKeyGenerator::name_to_key(name) as i32
}
fn client() -> GameClient {
    let mut c = GameClient::new_with_input_authority(ClientInputAuthority::NativeHosted).unwrap();
    let ui = InGameUISubsystem::default();
    c.set_in_game_ui_subsystem(ui);
    c
}
fn scene(c: &mut GameClient, command: &str) -> Window {
    c.with_window_event_effects(None, |effects| {
        effects.with_manager(|manager, effects| {
            let parent = manager
                .create_window_with_id(
                    None,
                    0,
                    0,
                    200,
                    200,
                    id("ControlBar.wnd:ControlBarParent"),
                    effects,
                )
                .unwrap();
            parent
                .borrow_mut()
                .set_name("ControlBar.wnd:ControlBarParent");
            parent
                .borrow_mut()
                .set_system_callback_with_message_sink(receiving_control_bar_system);
            parent.borrow_mut().hide(false, manager);
            let button = manager
                .create_window_with_id(
                    Some(&parent),
                    10,
                    10,
                    40,
                    20,
                    id("ControlBar.wnd:ButtonCommand01"),
                    effects,
                )
                .unwrap();
            button
                .borrow_mut()
                .set_name("ControlBar.wnd:ButtonCommand01");
            let button_id = button.borrow().get_id() as u32;
            button
                .borrow_mut()
                .set_widget(WindowWidget::PushButton(PushButton::new(
                    button_id, 10, 10, 40, 20,
                )));
            button.borrow_mut().set_user_data(CommandButton {
                command_name: command.into(),
                command_type: CommandType::PurchaseScience,
                sciences: vec![format!("SCIENCE_{command}")],
                ..Default::default()
            });
            button.borrow_mut().hide(false, manager);
            button
        })
    })
}
fn with_receiving_input_context<R>(
    c: &mut GameClient,
    bar: &mut ControlBar,
    origin: Origin,
    mouse: Option<&mut Mouse>,
    operation: impl FnOnce(&mut WindowManager, &mut ControlBarInputContext<'_>) -> R,
) -> R {
    let frame = c.get_frame();
    let mut viewport = Viewport;
    let mut emitted = Vec::new();
    let (owner, campaign, main_menu, mailbox, display, videos, ime, mut menus, ui, _mouse) =
        c.window_owner_campaign_menu_and_host_mut();
    let mut context = ControlBarInputContext {
        control_bar_observation: Some(
            game_client_rust::gui::callbacks::control_bar_observation::ControlBarCallbackObservation {
                game_ending: false,
                multiplayer: false,
                local_player: None,
                local_player_side: None,
                mouse: None,
            },
        ),
        menu_audio: None,
        input_provenance: origin,
        diplomacy_world: None,
        client_frame: Some(frame),
        logic_frame: None,
        ui,
        menus: Some(menus.reborrow()),
        ime: Some(ime),
        upgrade_tooltip: UpgradeTooltipSource::Standalone,
        camera_options: None,
        replay_menu: None,
        replay_version: None,
        observer_roster: None,
        control_bar: bar,
        viewport: &mut viewport,
        replay: false,
        game_ending: false,
        tooltips_disabled: true,
        cursor_mouse: mouse,
        window_manager_owner: Some(owner),
        campaign_manager: Some(campaign),
        menu_world: None,
        menu_host: Some(mailbox),
        window_videos: Some(&mut videos.challenge),
        in_game_movies: Some(&mut videos.in_game),
        script_display: Some(display),
        main_menu: Some(main_menu),
        emitted_messages: Some(&mut emitted),
    };
    owner.with_manager(|manager| operation(manager, &mut context))
}

fn dispatch(c: &mut GameClient, bar: &mut ControlBar, origin: Origin) {
    with_receiving_input_context(c, bar, origin, None, |manager, context| {
        for msg in [WindowMessage::LeftDown, WindowMessage::LeftUp] {
            let _ = manager.process_mouse_event_with_control_bar(
                msg,
                15,
                15,
                ((15usize) << 16) | 15,
                context,
            );
        }
        // The public input route records an owned click. Serialization/authority has not run.
        assert!(
            take_host_control_bar_published_requests(context.ui.as_deref_mut().unwrap()).is_empty()
        );
    });
}
fn replay(
    c: &mut GameClient,
    bar: &mut ControlBar,
) -> Vec<game_client_rust::gui::control_bar::HostControlBarPublishedRequest> {
    let mut messages = Vec::new();
    let (owner, _, _, _, _, _, _, _, ui, _mouse) = c.window_owner_campaign_menu_and_host_mut();
    let ui = ui.unwrap();
    owner.with_manager(|manager| {
        bar.replay_pending_command_clicks_receiving(owner, &mut messages, manager, ui)
    });
    take_host_control_bar_published_requests(ui)
}
fn command_name(request: &HostControlBarRequest) -> &str {
    match request {
        HostControlBarRequest::DirectCommand { command_name, .. } => command_name,
        _ => panic!("expected actual command-button publication"),
    }
}
#[test]
fn two_same_id_clients_replay_their_own_copied_origin_and_definition() {
    let mut a = client();
    let mut b = client();
    let wa = scene(&mut a, "Command_Alpha");
    let wb = scene(&mut b, "Command_Beta");
    assert_eq!(wa.borrow().get_id(), wb.borrow().get_id());
    let mut bar_a = ControlBar::new();
    let mut bar_b = ControlBar::new();
    dispatch(&mut a, &mut bar_a, Origin::PhysicalWindowMouseInput);
    dispatch(&mut b, &mut bar_b, Origin::InjectedOrUnknown);
    let rb = replay(&mut b, &mut bar_b);
    assert_eq!(rb.len(), 1);
    assert_eq!(rb[0].input_provenance, Origin::InjectedOrUnknown);
    assert_eq!(command_name(&rb[0].request), "Command_Beta");
    b.reset().unwrap();
    drop(b);
    let ra = replay(&mut a, &mut bar_a);
    assert_eq!(ra.len(), 1);
    assert_eq!(ra[0].input_provenance, Origin::PhysicalWindowMouseInput);
    assert_eq!(command_name(&ra[0].request), "Command_Alpha");
    dispatch(&mut a, &mut bar_a, Origin::InjectedOrUnknown);
    assert_eq!(
        replay(&mut a, &mut bar_a)[0].input_provenance,
        Origin::InjectedOrUnknown,
        "later input cannot inherit previous physical origin"
    );
}
#[test]
fn deferred_source_receipt_cannot_adopt_peer_window_with_the_same_id() {
    let mut a = client();
    let mut b = client();
    let _wa = scene(&mut a, "Command_Alpha");
    let _wb = scene(&mut b, "Command_Beta");
    let mut bar = ControlBar::new();
    dispatch(&mut a, &mut bar, Origin::PhysicalWindowMouseInput);
    assert!(
        replay(&mut b, &mut bar).is_empty(),
        "the weak original source must match this manager's pointer registration"
    );
    assert!(
        replay(&mut a, &mut bar).is_empty(),
        "rejected input is consumed once"
    );
}

type CallbackTrace = Rc<RefCell<Vec<String>>>;

fn mouse_callback_scene(c: &mut GameClient) -> (Window, CallbackTrace) {
    let trace = CallbackTrace::default();
    let target = c.with_window_event_effects(None, |effects| {
        effects.with_manager(|manager, effects| {
            let old = manager
                .create_window_with_id(None, 5, 5, 30, 30, id("PreviousHover"), effects)
                .unwrap();
            let close = manager
                .create_window_with_id(None, 5, 60, 30, 30, id("LonePopup"), effects)
                .unwrap();
            let parent = manager
                .create_window_with_id(None, 50, 5, 60, 30, id("MouseParent"), effects)
                .unwrap();
            let target = manager
                .create_window_with_id(Some(&parent), 5, 5, 30, 20, id("MouseTarget"), effects)
                .unwrap();
            for win in [&old, &close, &parent, &target] {
                win.borrow_mut().hide(false, manager).unwrap();
            }
            let old_trace = Rc::clone(&trace);
            old.borrow_mut().set_input_callback(move |_, msg, _, _| {
                if msg == WindowMessage::MouseLeaving {
                    old_trace.borrow_mut().push("leave".into());
                }
                WindowMsgHandled::Ignored
            });
            let popup_trace = Rc::clone(&trace);
            close.borrow_mut().set_system_callback(move |_, msg, _, _| {
                if msg == WindowMessage::User(16389) {
                    popup_trace.borrow_mut().push("close".into());
                }
                WindowMsgHandled::Ignored
            });
            let parent_trace = Rc::clone(&trace);
            parent.borrow_mut().set_input_callback(move |_, msg, _, _| {
                if matches!(msg, WindowMessage::RightUp | WindowMessage::LeftDown) {
                    parent_trace.borrow_mut().push("parent".into());
                    WindowMsgHandled::Handled
                } else {
                    WindowMsgHandled::Ignored
                }
            });
            let tooltip_trace = Rc::clone(&trace);
            let target_trace = Rc::clone(&trace);
            {
                let mut window = target.borrow_mut();
                window.set_tooltip("stale");
                window.set_tooltip_callback(move |window, _, _, _| {
                    tooltip_trace
                        .borrow_mut()
                        .push(format!("tooltip:{}", window.get_tooltip()));
                });
                window.set_input_callback(move |window, msg, _, _| {
                    match msg {
                        WindowMessage::RightUp | WindowMessage::LeftDown => {
                            target_trace.borrow_mut().push("target".into());
                            window.set_tooltip("updated");
                        }
                        WindowMessage::MouseEntering => {
                            target_trace.borrow_mut().push("enter".into())
                        }
                        _ => {}
                    }
                    WindowMsgHandled::Ignored
                });
            }
            manager.set_lone_window(Some(&close), effects);
            target
        })
    });
    (target, trace)
}

#[test]
fn native_mouse_dispatch_closes_lone_popup_before_live_tooltip_and_hover() {
    let world = new_world_services();
    with_world_services(&world, || {
        let mut first = client();
        let mut second = client();
        let (target_a, trace_a) = mouse_callback_scene(&mut first);
        let (target_b, trace_b) = mouse_callback_scene(&mut second);
        assert_eq!(target_a.borrow().get_id(), target_b.borrow().get_id());
        let input = |client: &mut GameClient, msg, x: i32| {
            with_receiving_input_context(
                client,
                &mut ControlBar::new(),
                Origin::PhysicalWindowMouseInput,
                Some(&mut Mouse::new()),
                |manager, context| {
                    let result = manager.process_mouse_event_with_control_bar(
                        msg,
                        x,
                        15,
                        (15usize << 16) | x as usize,
                        context,
                    );
                    let grab = manager
                        .get_grab_window()
                        .map(|window| window.borrow().get_id());
                    (result, grab)
                },
            )
        };
        assert_eq!(
            input(&mut first, WindowMessage::MousePos, 15).0,
            WindowInputReturnCode::NotUsed
        );
        assert_eq!(
            input(&mut second, WindowMessage::MousePos, 15).0,
            WindowInputReturnCode::NotUsed
        );
        assert!(trace_a.borrow().is_empty());
        assert!(trace_b.borrow().is_empty());
        assert_eq!(
            input(&mut first, WindowMessage::RightUp, 60),
            (WindowInputReturnCode::Used, None)
        );
        assert_eq!(
            &*trace_a.borrow(),
            &[
                "target",
                "parent",
                "close",
                "tooltip:updated",
                "leave",
                "enter"
            ]
        );
        assert!(
            trace_b.borrow().is_empty(),
            "peer windows with the same IDs received no callbacks"
        );
        trace_a.borrow_mut().clear();
        assert_eq!(
            input(&mut first, WindowMessage::MousePos, 60).0,
            WindowInputReturnCode::NotUsed
        );
        assert_eq!(
            &*trace_a.borrow(),
            &["tooltip:updated"],
            "same hover refresh sends no generated region events"
        );
        assert_eq!(
            input(&mut second, WindowMessage::LeftDown, 60),
            (WindowInputReturnCode::Used, Some(id("MouseParent")))
        );
        assert_eq!(
            &*trace_b.borrow(),
            &["target", "parent", "close", "tooltip:updated"],
            "newly grabbed mouse defers hover changes"
        );
    });
}
