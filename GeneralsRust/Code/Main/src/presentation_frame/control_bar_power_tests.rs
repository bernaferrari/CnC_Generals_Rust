use super::PresentationFrame;
use crate::game_logic::GameLogic;
use game_client::gui::control_bar::ControlBar;
use game_client::gui::game_window::WindowEventEffects;
use game_client::gui::window_manager::WindowManagerOwner;

#[test]
fn frozen_frames_apply_power_only_to_their_control_bar() {
    let owner = WindowManagerOwner::new();
    WindowEventEffects::with_standalone(|effects| {
        owner.with_manager(|manager| {
            manager.destroy_all_windows(effects);
            let power = manager
                .create_window(None, 0, 0, 80, 24, effects)
                .unwrap();
            power.borrow_mut().set_name("ControlBar.wnd:PowerWindow");

            let check = |bar: &mut ControlBar, manager: &mut game_client::gui::WindowManager, produced, consumed| {
                bar.update_money_and_power_windows(manager);
                assert_eq!(
                    power.borrow().get_text(),
                    ControlBar::format_control_bar_power_display(produced, consumed)
                );
            };

            let mut bar_a = ControlBar::new();
            bar_a.apply_presentation_money(100);
            bar_a.apply_presentation_power(80, 20);
            let mut logic = GameLogic::new();
            let base = PresentationFrame::build_from_logic(&mut logic, 0);
            check(&mut bar_a, manager, 80, 20);

            let mut frame_a = base.clone();
            frame_a.local_power_produced = 80;
            frame_a.local_power_consumed = 20;
            let mut frame_b = base;
            frame_b.local_power_produced = 10;
            frame_b.local_power_consumed = 50;
            let mut bar_b = ControlBar::new();
            frame_a.apply_to_control_bar(&mut bar_a);
            frame_b.apply_to_control_bar(&mut bar_b);
            frame_a.local_power_produced = 999;
            check(&mut bar_a, manager, 80, 20);
            check(&mut bar_b, manager, 10, 50);
            frame_a.local_power_produced = 80;
            frame_a.apply_to_control_bar(&mut bar_a);
            check(&mut bar_b, manager, 10, 50);
            check(&mut bar_a, manager, 80, 20);
        });
    });
}
