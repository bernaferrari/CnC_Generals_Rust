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
use game_client_rust::gui::gui_callbacks::control_bar_popup_description::UpgradeTooltipSource;
use game_client_rust::gui::window_manager::ControlBarInputContext;
use game_client_rust::gui::{GameWindow, WindowMessage, WindowWidget};
use game_engine::common::name_key_generator::NameKeyGenerator;
use gamelogic::commands::CommandType;
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
fn dispatch(c: &mut GameClient, bar: &mut ControlBar, origin: Origin) {
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
        cursor_mouse: None,
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
    owner.with_manager(|manager| {
        for msg in [WindowMessage::LeftDown, WindowMessage::LeftUp] {
            let _ = manager.process_mouse_event_with_control_bar(
                msg,
                15,
                15,
                ((15usize) << 16) | 15,
                &mut context,
            );
        }
    });
    // The public input route records an owned click. Serialization/authority has not run.
    assert!(
        take_host_control_bar_published_requests(context.ui.as_deref_mut().unwrap()).is_empty()
    );
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
