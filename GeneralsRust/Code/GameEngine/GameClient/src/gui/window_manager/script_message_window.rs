//! ScriptActions-owned win/lose window tracking on its actual WindowManager.

use super::*;

impl WindowManager {
    /// C++ `winCreateFromScript`: destroy the prior tracked window first, then
    /// retain the exact newly loaded window if the layout exists.
    pub fn create_script_message_window(&mut self, layout_filename: &str) {
        self.replace_script_message_window_with(|manager| {
            manager.load_window(layout_filename).ok()
        });
    }

    /// C++ `winDestroy(m_messageWindow)`. The weak handle prevents the manager
    /// from owning a window twice and keeps the target tied to this registry.
    pub fn destroy_script_message_window(&mut self) {
        let Some(window) = self
            .script_message_window
            .take()
            .and_then(|window| window.upgrade())
        else {
            return;
        };

        let window_id = window.borrow().get_id();
        let still_registered = self
            .window_by_id
            .get(&window_id)
            .and_then(Weak::upgrade)
            .is_some_and(|registered| Rc::ptr_eq(&registered, &window));
        if !still_registered {
            return;
        }

        let _ = self.destroy_window(window);
        self.flush_destroy_queue();
    }

    fn replace_script_message_window_with(
        &mut self,
        load_window: impl FnOnce(&mut Self) -> Option<Rc<RefCell<GameWindow>>>,
    ) -> Option<WindowId> {
        self.destroy_script_message_window();
        let window = load_window(self)?;
        let window_id = window.borrow().get_id();
        self.script_message_window = Some(Rc::downgrade(&window));
        Some(window_id)
    }

    #[cfg(test)]
    pub(crate) fn script_message_window_id_for_test(&self) -> Option<WindowId> {
        let window = self.script_message_window.as_ref()?.upgrade()?;
        let window_id = window.borrow().get_id();
        self.window_by_id
            .get(&window_id)
            .and_then(Weak::upgrade)
            .filter(|registered| Rc::ptr_eq(registered, &window))
            .map(|_| window_id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_window(manager: &mut WindowManager) -> Option<Rc<RefCell<GameWindow>>> {
        manager.create_window(None, 0, 0, 10, 10).ok()
    }

    #[test]
    fn script_message_window_is_owned_by_its_exact_manager_even_for_same_id() {
        let mut first = WindowManager::new();
        let mut second = WindowManager::new();

        let first_id = first
            .replace_script_message_window_with(create_window)
            .expect("first manager creates its message window");
        let second_id = second
            .replace_script_message_window_with(create_window)
            .expect("second manager creates its message window");
        assert_eq!(first_id, second_id, "fresh managers both allocate ID one");
        assert_eq!(first.script_message_window_id_for_test(), Some(first_id));
        assert_eq!(second.script_message_window_id_for_test(), Some(second_id));

        first.destroy_script_message_window();
        assert!(first.get_window_by_id(first_id).is_none());
        assert!(second.get_window_by_id(second_id).is_some());
        assert_eq!(first.script_message_window_id_for_test(), None);
        assert_eq!(second.script_message_window_id_for_test(), Some(second_id));

        second.destroy_script_message_window();
        assert!(second.get_window_by_id(second_id).is_none());
    }

    #[test]
    fn external_destroy_and_reused_id_do_not_redirect_a_stale_script_pointer() {
        let mut manager = WindowManager::new();
        let tracked_id = manager
            .replace_script_message_window_with(create_window)
            .expect("create tracked window");
        let old_window = manager
            .get_window_by_id(tracked_id)
            .expect("tracked window is registered");

        manager.destroy_window(old_window.clone()).unwrap();
        manager.flush_destroy_queue();
        let replacement = manager
            .create_window_with_id(None, 0, 0, 10, 10, tracked_id)
            .expect("reuse numeric ID after external destruction");
        manager.destroy_script_message_window();

        let still_registered = manager
            .get_window_by_id(tracked_id)
            .expect("replacement remains registered");
        assert!(Rc::ptr_eq(&still_registered, &replacement));
        assert_eq!(manager.script_message_window_id_for_test(), None);
    }

    #[test]
    fn manager_shutdown_leaves_a_stale_weak_owner_harmless() {
        let mut manager = WindowManager::new();
        let tracked_id = manager
            .replace_script_message_window_with(create_window)
            .expect("create tracked window");
        assert_eq!(
            manager.script_message_window_id_for_test(),
            Some(tracked_id)
        );

        manager.destroy_all_windows();
        assert!(manager.get_window_by_id(tracked_id).is_none());
        manager.destroy_script_message_window();
        assert_eq!(manager.script_message_window_id_for_test(), None);
    }

    #[test]
    fn failed_replacement_destroys_and_clears_the_previous_window_first() {
        let mut manager = WindowManager::new();
        let previous_id = manager
            .replace_script_message_window_with(create_window)
            .expect("create previous message window");
        assert_eq!(
            manager.script_message_window_id_for_test(),
            Some(previous_id)
        );

        assert_eq!(
            manager.replace_script_message_window_with(|_| None),
            None,
            "a missing replacement layout leaves no tracked message window"
        );
        assert!(manager.get_window_by_id(previous_id).is_none());
        assert_eq!(manager.script_message_window_id_for_test(), None);
    }
}
