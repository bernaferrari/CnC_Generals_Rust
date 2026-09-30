//! Money-window refresh from C++ InGameUI::update (1776-1815).
//! The single C++ UI used a static amount cache; Rust owns its cache on the
//! subsystem and validates the actual window text after layout replacement.

use super::InGameUISubsystem;
use crate::gui::game_window::GameWindow;

impl InGameUISubsystem {
    fn refresh_money_text(&mut self, money: i32, target: Option<&mut GameWindow>) {
        let Some(window) = target else {
            self.money_display_cache = None;
            return;
        };
        if self
            .money_display_cache
            .as_ref()
            .is_some_and(|(amount, text)| *amount == money && window.get_text() == text)
        {
            return;
        }
        let text = crate::gui::control_bar::ControlBar::format_control_bar_money_display(money);
        if window.set_text(&text).is_ok() {
            self.money_display_cache = Some((money, text));
        }
    }
}

impl InGameUISubsystem {
    /// C++ `InGameUI::update` money/power window refresh (InGameUI.cpp:1776-1815):
    /// the money player (observer look-at when the observer control bar is on,
    /// else ThePlayerList local player) drives
    /// `GadgetStaticTextSetText(moneyWin, buffer.format(TheGameText->fetch("GUI:ControlBarMoneyDisplay"), currentMoney))`
    /// whenever the amount changed, and both `ControlBar.wnd:MoneyDisplay` /
    /// `ControlBar.wnd:PowerWindow` are `winHide(FALSE)` while a money player
    /// exists (`winHide(TRUE)` when NULL). The WND-authored label is
    /// `GUI:$$$` → "$$$" (ControlBar.wnd:4352), which retail overwrites on the
    /// first update. This subsystem bridges the dual-world PlayerList path;
    /// ControlBar::update and Main's frozen presentation drive the host fallback.
    pub(super) fn update_money_and_power_windows(&mut self) {
        let money_player = if let Some(index) =
            crate::helpers::TheControlBar::get_observer_look_at_player_index()
        {
            gamelogic::player::player_list()
                .read()
                .ok()
                .and_then(|list| {
                    list.get_player(index as gamelogic::player::PlayerIndex)
                        .cloned()
                })
        } else {
            gamelogic::player::player_list()
                .read()
                .ok()
                .and_then(|list| list.get_local_player().cloned())
        };
        let current_money = money_player
            .as_ref()
            .and_then(|player| player.read().ok())
            .map(|player| player.get_money().count_money() as i32);

        let money_window_id = game_engine::common::name_key_generator::NameKeyGenerator::name_to_key(
            "ControlBar.wnd:MoneyDisplay",
        ) as i32;
        let power_window_id = game_engine::common::name_key_generator::NameKeyGenerator::name_to_key(
            "ControlBar.wnd:PowerWindow",
        ) as i32;
        crate::gui::window_manager::with_window_manager(|manager| {
            let money_window = manager.get_window_by_id(money_window_id);
            let power_window = manager.get_window_by_id(power_window_id);
            match current_money {
                Some(money) => {
                    let mut target = money_window.as_ref().map(|window| window.borrow_mut());
                    self.refresh_money_text(money, target.as_deref_mut());
                    drop(target);
                    // C++ 1808-1809: winHide(FALSE) whenever a money player exists.
                    if let Some(window) = money_window.as_ref() {
                        let _ = window.borrow_mut().hide(false);
                    }
                    if let Some(window) = power_window.as_ref() {
                        let _ = window.borrow_mut().hide(false);
                    }
                }
                // C++ 1811-1815 hides both windows when moneyPlayer == NULL,
                // which never happens in a real skirmish: the live crate
                // PlayerList is frequently empty even in an offline match, and
                // hiding here fought the ControlBar residual write (which
                // falls back to the presentation-freeze money, 0 allowed) and
                // left the authored `$$$` box fighting a hide every frame.
                // Leave the windows as-is; the residual path owns the text.
                None => {
                    self.money_display_cache = None;
                }
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn money_window() -> GameWindow {
        let mut window = GameWindow::new();
        window.set_id(77);
        window.set_text("$$$").unwrap();
        window
    }

    #[test]
    fn missing_and_recreated_money_window_get_current_amount() {
        let mut ui = InGameUISubsystem::default();
        ui.refresh_money_text(10000, None);
        assert!(ui.money_display_cache.is_none());
        let expected = crate::gui::control_bar::ControlBar::format_control_bar_money_display(10000);
        let mut first = money_window();
        ui.refresh_money_text(10000, Some(&mut first));
        assert_eq!(first.get_text(), expected);
        let mut replacement = money_window();
        assert_eq!(replacement.get_id(), first.get_id());
        ui.refresh_money_text(10000, Some(&mut replacement));
        assert_eq!(replacement.get_text(), expected);
        ui.refresh_money_text(10000, None);
        assert!(ui.money_display_cache.is_none());
        ui.refresh_money_text(10000, Some(&mut replacement));
        assert_eq!(replacement.get_text(), expected);
    }

    #[test]
    fn money_cache_belongs_to_each_ui_and_resets_inertly() {
        let mut first = InGameUISubsystem::default();
        let mut second = InGameUISubsystem::default();
        assert!(first.money_display_cache.is_none());
        assert!(second.money_display_cache.is_none());
        let mut first_window = money_window();
        let mut second_window = money_window();
        first.refresh_money_text(10000, Some(&mut first_window));
        assert!(second.money_display_cache.is_none());
        second.refresh_money_text(10000, Some(&mut second_window));
        assert_eq!(first_window.get_text(), second_window.get_text());
        first.refresh_money_text(500, Some(&mut first_window));
        assert_ne!(first_window.get_text(), second_window.get_text());
        first.clear_runtime_state();
        assert!(first.money_display_cache.is_none());
        assert_eq!(second.money_display_cache.as_ref().unwrap().0, 10000);
        first.refresh_money_text(10000, Some(&mut first_window));
        assert_eq!(first_window.get_text(), second_window.get_text());
    }
}
