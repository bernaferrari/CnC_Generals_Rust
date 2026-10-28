//! Public receiving ControlBar admission must consume its owner's roster.
use game_client_rust::core::GameClient;
use game_client_rust::core::game_client::ClientInputAuthority;
use game_client_rust::gui::control_bar::ControlBar;
use game_client_rust::gui::control_bar::control_bar_observer::{
    NativeObserverPlayer, NativeObserverRoster,
};
use game_client_rust::gui::gui_callbacks::control_bar_popup_description::UpgradeTooltipSource;
use gamelogic::player::{Player, PlayerList, player_list};
use std::sync::{Arc, RwLock};
use std::time::Duration;

struct RestorePlayerList(Option<PlayerList>);

impl Drop for RestorePlayerList {
    fn drop(&mut self) {
        if let Some(previous) = self.0.take() {
            *player_list().write().unwrap() = previous;
        }
    }
}

fn roster(name: &str, money: i32, produced: i32, consumed: i32) -> NativeObserverRoster {
    NativeObserverRoster {
        players: vec![NativeObserverPlayer {
            index: 5,
            slot_index: 0,
            team_key: Some("TeamReceiving".into()),
            observer: false,
            human: true,
            display_name: name.into(),
            color_rgba: 0xff12_3456,
            enabled_image: "flag-receiving".into(),
            flag_water_mark: "watermark-receiving".into(),
            money,
            power_produced: produced,
            power_consumed: consumed,
            num_units: money / 100,
            num_buildings: produced / 10,
            units_killed: consumed / 2,
            units_lost: consumed / 3,
        }],
        multiplayer: false,
        frame: 30,
    }
}

fn update(
    client: &mut GameClient,
    bar: &mut ControlBar,
    roster: &NativeObserverRoster,
) -> (String, String) {
    let owner = client.window_manager_owner();
    client.with_window_and_control_bar_effects(None, |effects, videos| {
        effects.with_manager(|manager, effects| {
            for name in ["ControlBar.wnd:MoneyDisplay", "ControlBar.wnd:PowerWindow"] {
                if manager.find_window_by_name(name).is_none() {
                    let window = manager.create_window(None, 0, 0, 80, 24, effects).unwrap();
                    window.borrow_mut().set_name(name);
                }
            }

            bar.update_owned_receiving(
                Duration::from_millis(33),
                Some(roster),
                UpgradeTooltipSource::Standalone,
                false,
                &owner,
                videos,
                manager,
                effects,
            )
            .unwrap();
            let money = manager
                .find_window_by_name("ControlBar.wnd:MoneyDisplay")
                .unwrap()
                .borrow()
                .get_text()
                .to_string();
            let power = manager
                .find_window_by_name("ControlBar.wnd:PowerWindow")
                .unwrap()
                .borrow()
                .get_text()
                .to_string();
            (money, power)
        })
    })
}

#[test]
fn interleaved_receiving_updates_keep_same_id_observer_facts_per_client() {
    // The compatibility singleton deliberately holds a conflicting same-ID
    // player. Neither native client may enumerate it or the peer's roster.
    let mut conflicting = PlayerList::new();
    conflicting.add_player(Arc::new(RwLock::new(Player::new(5))));
    let previous = std::mem::replace(&mut *player_list().write().unwrap(), conflicting);
    let _restore = RestorePlayerList(Some(previous));

    let receiving = roster("Receiving Session", 731, 41, 17);
    let peer_roster = roster("Peer Session", 284, 23, 6);
    let mut receiving_client =
        GameClient::new_with_input_authority(ClientInputAuthority::NativeHosted).unwrap();
    receiving_client.set_in_game_ui_subsystem(Default::default());
    let mut peer_client =
        GameClient::new_with_input_authority(ClientInputAuthority::NativeHosted).unwrap();
    peer_client.set_in_game_ui_subsystem(Default::default());
    let mut receiving_bar = ControlBar::new();
    receiving_bar.set_observer_mode(true);
    receiving_bar.admit_observer_button_owned(Some(5), &receiving);
    let mut peer_bar = ControlBar::new();
    peer_bar.set_observer_mode(true);
    peer_bar.admit_observer_button_owned(Some(5), &peer_roster);

    let expected_receiving = (
        ControlBar::format_control_bar_money_display(731),
        ControlBar::format_control_bar_power_display(41, 17),
    );
    let expected_peer = (
        ControlBar::format_control_bar_money_display(284),
        ControlBar::format_control_bar_power_display(23, 6),
    );

    assert_eq!(
        update(&mut receiving_client, &mut receiving_bar, &receiving),
        expected_receiving
    );
    assert_eq!(
        update(&mut peer_client, &mut peer_bar, &peer_roster),
        expected_peer
    );
    assert_eq!(
        update(&mut receiving_client, &mut receiving_bar, &receiving),
        expected_receiving
    );
    assert_eq!(
        receiving_bar.get_context().observer_player_stats,
        vec![("Receiving Session".into(), 731, 7, 4, 8, 5)]
    );
    assert_eq!(
        peer_bar.get_context().observer_player_stats,
        vec![("Peer Session".into(), 284, 2, 2, 3, 2)]
    );
}
