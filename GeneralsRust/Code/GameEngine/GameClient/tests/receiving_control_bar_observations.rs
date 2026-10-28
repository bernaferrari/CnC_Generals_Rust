//! Receiving callback observations, not full C++ gameplay or hardware-device evidence.
use game_client_rust::core::game_client::{ClientInputAuthority, GameClient};
use game_client_rust::core::subsystems::InGameUISubsystem;
use game_client_rust::gui::callbacks::control_bar_callbacks::{
    beacon_window_input, left_hud_input, receiving_control_bar_system, show_control_bar,
};
use game_client_rust::gui::callbacks::control_bar_observation::*;
use game_client_rust::gui::campaign_launch_host_bridge::MenuHostRequest;
use game_client_rust::gui::control_bar::{
    HostControlBarRequest, take_host_control_bar_published_requests, take_host_minimap_interactions,
};
use game_client_rust::gui::{GameWindow, WindowMessage, WindowMsgHandled};
use game_engine::common::name_key_generator::NameKeyGenerator;
use game_engine::common::system::radar::{Coord3D, RadarSystem};
use std::{cell::RefCell, rc::Rc, sync::Arc};
fn facts(id: u32) -> ControlBarCallbackObservation {
    ControlBarCallbackObservation {
        game_ending: false,
        multiplayer: true,
        local_player: Some(ControlBarPlayerObservation {
            id,
            active: true,
            selection_count: 1,
        }),
        local_player_side: Some(format!("Faction{id}")),
        mouse: Some(ControlBarMouseObservation {
            middle_held: false,
            alternate_mouse: true,
        }),
    }
}
fn control_bar_parent(c: &mut GameClient) -> Rc<RefCell<GameWindow>> {
    c.with_window_event_effects(None, |effects| {
        effects.with_manager(|manager, effects| {
            manager
                .create_window_with_id(
                    None,
                    0,
                    0,
                    640,
                    80,
                    NameKeyGenerator::name_to_key("ControlBar.wnd:ControlBarParent") as i32,
                    effects,
                )
                .unwrap()
        })
    })
}
fn show_control_bar_for_side(c: &mut GameClient, side: &str) {
    let observation = ControlBarCallbackObservation {
        local_player_side: Some(side.to_string()),
        ..facts(0)
    };
    show_control_bar_with_observation(c, observation);
}
fn show_control_bar_with_observation(
    c: &mut GameClient,
    observation: ControlBarCallbackObservation,
) {
    c.with_window_event_effects(None, |effects| {
        effects.admit_control_bar_callback_observation(observation);
        effects.with_manager(|manager, effects| {
            show_control_bar(true, manager, effects).expect("receiving ShowControlBar succeeds");
        });
    });
}
fn set_legacy_global_side(side: &str) {
    let list = gamelogic::player::player_list();
    let mut list = list.write().expect("legacy global player list");
    list.clear();
    let mut player = gamelogic::player::Player::new(17);
    player.set_side(side);
    list.add_player(Arc::new(std::sync::RwLock::new(player)));
    list.set_local_player_index(17);
}
fn clear_legacy_global_side() {
    gamelogic::player::player_list()
        .write()
        .expect("legacy global player list")
        .clear();
}
fn client() -> GameClient {
    let mut c = GameClient::new_with_input_authority(ClientInputAuthority::NativeHosted).unwrap();
    let mut ui = InGameUISubsystem::default();
    let mut radar = RadarSystem::new();
    radar.new_map_owned(
        Coord3D::new(0.0, 0.0, 0.0),
        Coord3D::new(100.0, 100.0, 0.0),
        &[],
    );
    ui.set_radar_presentation(Arc::new(radar.presentation()));
    c.set_in_game_ui_subsystem(ui);
    c
}
fn window(c: &mut GameClient) -> Rc<RefCell<GameWindow>> {
    c.with_window_event_effects(None, |effects| {
        effects.with_manager(|manager, effects| {
            manager
                .create_window_with_id(None, 0, 0, 200, 200, 42, effects)
                .unwrap()
        })
    })
}
fn idle(
    c: &mut GameClient,
    w: &Rc<RefCell<GameWindow>>,
    observation: Option<ControlBarCallbackObservation>,
) -> WindowMsgHandled {
    c.with_window_event_effects(None, |effects| {
        if let Some(facts) = observation {
            effects.admit_control_bar_callback_observation(facts);
        }
        receiving_control_bar_system(
            &mut w.borrow_mut(),
            WindowMessage::GadgetSelected,
            NameKeyGenerator::name_to_key("ControlBar.wnd:ButtonIdleWorker") as usize,
            0,
            effects,
        )
    })
}
#[test]
fn game_ending_is_receiving_and_native_missing_observation_is_unavailable() {
    let mut a = client();
    let mut b = client();
    let wa = window(&mut a);
    let wb = window(&mut b);
    let mut ending = facts(1);
    ending.game_ending = true;
    assert_eq!(
        idle(&mut a, &wa, Some(ending.clone())),
        WindowMsgHandled::Ignored
    );
    assert_eq!(idle(&mut a, &wa, None), WindowMsgHandled::Ignored);
    assert!(
        take_host_control_bar_published_requests(a.in_game_ui_subsystem_mut().unwrap()).is_empty()
    );
    assert_eq!(idle(&mut b, &wb, Some(facts(2))), WindowMsgHandled::Handled);
    assert!(
        matches!(take_host_control_bar_published_requests(b.in_game_ui_subsystem_mut().unwrap()).as_slice(),[published] if published.request==HostControlBarRequest::SelectNextIdleWorker)
    );
    b.reset().unwrap();
    drop(b);
    assert_eq!(idle(&mut a, &wa, Some(ending)), WindowMsgHandled::Ignored);
}
#[test]
fn middle_and_alternate_mouse_facts_never_come_from_peer_input() {
    let mut a = client();
    let mut b = client();
    let wa = window(&mut a);
    let wb = window(&mut b);
    let mut held = facts(1);
    held.mouse.as_mut().unwrap().middle_held = true;
    let click = 30usize | (40usize << 16);
    assert_eq!(
        a.with_window_event_effects(None, |effects| {
            effects.admit_control_bar_callback_observation(held);
            left_hud_input(
                &mut wa.borrow_mut(),
                WindowMessage::LeftDown,
                click,
                0,
                effects,
            )
        }),
        WindowMsgHandled::Ignored
    );
    assert_eq!(
        b.with_window_event_effects(None, |effects| {
            effects.admit_control_bar_callback_observation(facts(2));
            left_hud_input(
                &mut wb.borrow_mut(),
                WindowMessage::LeftDown,
                click,
                0,
                effects,
            )
        }),
        WindowMsgHandled::Handled
    );
    assert!(take_host_minimap_interactions(a.in_game_ui_subsystem_mut().unwrap()).is_empty());
    assert!(
        take_host_minimap_interactions(b.in_game_ui_subsystem_mut().unwrap())[0].alternate_mouse
    );
    let mut missing = facts(1);
    missing.mouse = None;
    assert_eq!(
        a.with_window_event_effects(None, |effects| {
            effects.admit_control_bar_callback_observation(missing);
            left_hud_input(
                &mut wa.borrow_mut(),
                WindowMessage::LeftDown,
                click,
                0,
                effects,
            )
        }),
        WindowMsgHandled::Ignored
    );
    assert!(take_host_minimap_interactions(a.in_game_ui_subsystem_mut().unwrap()).is_empty());
}
#[test]
fn beacon_predicates_and_escape_capture_the_actual_local_recipient() {
    let mut a = client();
    let mut b = client();
    let wa = window(&mut a);
    let wb = window(&mut b);
    let mut inactive = facts(17);
    inactive.local_player.as_mut().unwrap().active = false;
    for (c, w, observation) in [(&mut a, &wa, inactive), (&mut b, &wb, facts(29))] {
        c.with_window_event_effects(None, |effects| {
            effects.admit_control_bar_callback_observation(observation);
            receiving_control_bar_system(
                &mut w.borrow_mut(),
                WindowMessage::GadgetSelected,
                NameKeyGenerator::name_to_key("ControlBar.wnd:ButtonPlaceBeacon") as usize,
                0,
                effects,
            );
        });
    }
    assert!(
        a.in_game_ui_subsystem()
            .unwrap()
            .get_pending_command()
            .is_none()
    );
    assert!(
        b.in_game_ui_subsystem()
            .unwrap()
            .get_pending_command()
            .is_some()
    );
    assert_eq!(
        b.with_window_event_effects(None, |effects| {
            effects.admit_control_bar_callback_observation(facts(29));
            beacon_window_input(&mut wb.borrow_mut(), WindowMessage::Char, 0x1b, 0, effects)
        }),
        WindowMsgHandled::Handled
    );
    assert!(a.take_menu_host_requests().is_empty());
    assert!(matches!(
        b.take_menu_host_requests().as_slice(),
        [MenuHostRequest::ClearSelection { player_id: 29 }]
    ));
}

#[test]
fn control_bar_scheme_uses_the_receiving_session_player_side() {
    let mut a = client();
    let mut b = client();
    let _parent_a = control_bar_parent(&mut a);
    let _parent_b = control_bar_parent(&mut b);

    show_control_bar_for_side(&mut a, "AmericaAirForceGeneral");
    show_control_bar_for_side(&mut a, "AmericaSuperWeaponGeneral");
    show_control_bar_for_side(&mut b, "ChinaTankGeneral");

    let events_b = game_client_rust::helpers::drain_live_control_bar_events(
        b.in_game_ui_subsystem_mut().unwrap(),
    );
    let events_a = game_client_rust::helpers::drain_live_control_bar_events(
        a.in_game_ui_subsystem_mut().unwrap(),
    );
    assert_eq!(
        events_a.set_scheme_by_player.as_deref(),
        Some("AmericaSuperWeaponGeneral")
    );
    assert_eq!(
        events_b.set_scheme_by_player.as_deref(),
        Some("ChinaTankGeneral")
    );
}

#[test]
fn native_show_without_observation_ignores_conflicting_global_side() {
    set_legacy_global_side("PeerGlobalSide");
    let mut native = client();
    let _parent = control_bar_parent(&mut native);
    native.with_window_event_effects(None, |effects| {
        effects.with_manager(|manager, effects| {
            show_control_bar(true, manager, effects)
                .expect("native ShowControlBar without observation still completes");
        });
    });
    let events = game_client_rust::helpers::drain_live_control_bar_events(
        native.in_game_ui_subsystem_mut().unwrap(),
    );
    assert_eq!(events.set_scheme_by_player, None);
    clear_legacy_global_side();
}

#[test]
fn scheme_side_requires_a_local_player_and_normalizes_an_empty_side() {
    set_legacy_global_side("PeerGlobalSide");
    let mut native = client();
    let _parent = control_bar_parent(&mut native);

    let mut no_player = facts(0);
    no_player.local_player = None;
    no_player.local_player_side = None;
    show_control_bar_with_observation(&mut native, no_player);
    let no_player_events = game_client_rust::helpers::drain_live_control_bar_events(
        native.in_game_ui_subsystem_mut().unwrap(),
    );
    assert_eq!(no_player_events.set_scheme_by_player, None);

    let mut empty_side = facts(0);
    empty_side.local_player_side = Some(String::new());
    show_control_bar_with_observation(&mut native, empty_side);
    let empty_side_events = game_client_rust::helpers::drain_live_control_bar_events(
        native.in_game_ui_subsystem_mut().unwrap(),
    );
    assert_eq!(
        empty_side_events.set_scheme_by_player.as_deref(),
        Some("Observer")
    );

    clear_legacy_global_side();
}
