use super::PresentationFrame;
use crate::game_logic::GameLogic;
use game_client::gui::control_bar::ControlBar;

#[test]
fn frozen_frames_apply_power_only_to_their_control_bar() {
    // W3DControlBar.cpp:90-126 selects the local/observer player before
    // reading Energy. The host's equivalent is the specific frozen frame.
    struct RestorePlayers(gamelogic::player::PlayerList);
    impl Drop for RestorePlayers {
        fn drop(&mut self) {
            *gamelogic::player::player_list().write().unwrap() =
                std::mem::replace(&mut self.0, gamelogic::player::PlayerList::new());
            game_client::gui::with_window_manager(|manager| manager.destroy_all_windows());
        }
    }
    let old = {
        let mut players = gamelogic::player::player_list().write().unwrap();
        std::mem::replace(&mut *players, gamelogic::player::PlayerList::new())
    };
    let _restore = RestorePlayers(old);
    let power = game_client::gui::with_window_manager(|manager| {
        manager.destroy_all_windows();
        let power = manager.create_window(None, 0, 0, 80, 24).unwrap();
        power.borrow_mut().set_name("ControlBar.wnd:PowerWindow");
        power
    });
    let check = |bar: &mut ControlBar, produced, consumed| {
        bar.update_money_and_power_windows();
        assert_eq!(
            power.borrow().get_text(),
            ControlBar::format_control_bar_power_display(produced, consumed)
        );
    };

    let mut bar_a = ControlBar::new();
    bar_a.apply_presentation_money(100);
    bar_a.apply_presentation_power(80, 20);
    let logic = GameLogic::new();
    let base = PresentationFrame::build_from_logic(&logic, 0);
    check(&mut bar_a, 80, 20); // Merely freezing another world is inert.

    let mut frame_a = base.clone();
    frame_a.local_power_produced = 80;
    frame_a.local_power_consumed = 20;
    let mut frame_b = base;
    frame_b.local_power_produced = 10;
    frame_b.local_power_consumed = 50;
    let mut bar_b = ControlBar::new();
    frame_a.apply_to_control_bar(&mut bar_a);
    frame_b.apply_to_control_bar(&mut bar_b);
    // Applying captures values; subsequent source-frame edits cannot leak.
    frame_a.local_power_produced = 999;
    check(&mut bar_a, 80, 20);
    check(&mut bar_b, 10, 50);
    frame_a.local_power_produced = 80;
    frame_a.apply_to_control_bar(&mut bar_a);
    check(&mut bar_b, 10, 50);
    check(&mut bar_a, 80, 20);
}
