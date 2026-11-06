//! Shared LAN setup state for menu coordination.

use game_network::GameInfo;
use std::sync::{Arc, Mutex};

#[derive(Debug, Default)]
pub struct LanSetup {
    selected_map: String,
    use_system_maps: bool,
    game_info: GameInfo,
}

impl LanSetup {
    pub fn selected_map(&self) -> &str {
        &self.selected_map
    }

    pub fn set_selected_map(&mut self, map: String) {
        self.selected_map = map;
    }

    pub fn use_system_maps(&self) -> bool {
        self.use_system_maps
    }

    pub fn set_use_system_maps(&mut self, value: bool) {
        self.use_system_maps = value;
    }

    pub fn game_info(&self) -> &GameInfo {
        &self.game_info
    }

    pub fn game_info_mut(&mut self) -> &mut GameInfo {
        &mut self.game_info
    }
}

#[cfg(test)]
thread_local! {
    static TEST_LAN_SETUP: std::cell::RefCell<Option<Arc<Mutex<LanSetup>>>> =
        const { std::cell::RefCell::new(None) };
}

/// Owner-less unit-test lobby (paired with the options-menu test state).
#[cfg(test)]
pub(crate) fn test_lan_setup_handle() -> Arc<Mutex<LanSetup>> {
    TEST_LAN_SETUP.with(|slot| {
        slot.borrow_mut()
            .get_or_insert_with(|| Arc::new(Mutex::new(LanSetup::default())))
            .clone()
    })
}

/// This client's `TheLANGameInfo` lobby.
///
/// Callers holding their UI tree pass its `WindowManagerOwner` (window
/// handlers: `effects.window_manager_owner`; hosts: the owning
/// `GameClient::lan_setup()`). Without one, the registered live client is
/// consulted; production Main never publishes that slot, so a missing owner
/// there is a lifecycle bug and fails loudly instead of handing out a scratch
/// lobby whose writes would be silently discarded.
pub fn get_lan_setup(
    owner: Option<&crate::gui::window_manager::WindowManagerOwner>,
) -> Arc<Mutex<LanSetup>> {
    if let Some(owner) = owner {
        return owner.lan_setup();
    }
    if let Some(handle) =
        crate::core::game_client::with_live_game_client_mut(|client| client.lan_setup())
    {
        return handle;
    }
    #[cfg(test)]
    {
        return test_lan_setup_handle();
    }
    #[cfg(not(test))]
    {
        panic!("LAN setup requires a WindowManagerOwner or live GameClient; no process scratch");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gui::window_manager::WindowManagerOwner;

    #[test]
    fn each_owner_has_its_own_lobby_and_menu_state_edits_it() {
        let a = WindowManagerOwner::new();
        let b = WindowManagerOwner::new();
        // The options menu bound to owner `a` writes through to a's lobby only.
        a.with_lan_game_options_mut(|state| {
            let handle = state.borrow().setup_handle();
            handle.lock().unwrap().set_selected_map("Maps\\Alpha".into());
        });
        assert_eq!(
            get_lan_setup(Some(&a)).lock().unwrap().selected_map(),
            "Maps\\Alpha"
        );
        assert_eq!(get_lan_setup(Some(&b)).lock().unwrap().selected_map(), "");
        assert!(Arc::ptr_eq(&get_lan_setup(Some(&a)), &a.lan_setup()));
    }
}
