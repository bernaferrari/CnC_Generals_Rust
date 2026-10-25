//! Bounded transition access to the receiving WND tree and visibility state.
use super::{CaptureFlags, GameWindow, ModalWindow, WindowManager};
use std::cell::{Ref, RefCell, RefMut};
use std::ops::{Deref, DerefMut};
use std::rc::{Rc, Weak};

pub(crate) enum TransitionWindowRead<'a> {
    Selected(&'a GameWindow),
    Stored(Ref<'a, GameWindow>),
}
impl Deref for TransitionWindowRead<'_> {
    type Target = GameWindow;
    fn deref(&self) -> &GameWindow {
        match self {
            Self::Selected(window) => window,
            Self::Stored(window) => window,
        }
    }
}
pub(crate) enum TransitionWindowWrite<'a> {
    Selected(&'a mut GameWindow),
    Stored(RefMut<'a, GameWindow>),
}
impl Deref for TransitionWindowWrite<'_> {
    type Target = GameWindow;
    fn deref(&self) -> &GameWindow {
        match self {
            Self::Selected(window) => window,
            Self::Stored(window) => window,
        }
    }
}
impl DerefMut for TransitionWindowWrite<'_> {
    fn deref_mut(&mut self) -> &mut GameWindow {
        match self {
            Self::Selected(window) => window,
            Self::Stored(window) => window,
        }
    }
}

/// Deliver to the canonical callback window when it is already mutably
/// borrowed; other targets use their ordinary tree storage. No busy-as-absent
/// path or duplicate window state is permitted.
pub(super) fn borrow_callback_window<'a>(
    target: &'a Rc<RefCell<GameWindow>>,
    selected: Option<&'a mut GameWindow>,
) -> TransitionWindowWrite<'a> {
    if let Some(selected) = selected {
        if std::ptr::eq(target.as_ptr(), std::ptr::from_ref(&*selected)) {
            return TransitionWindowWrite::Selected(selected);
        }
    }
    TransitionWindowWrite::Stored(target.borrow_mut())
}

/// The selected window is already borrowed by its callback. Access that exact
/// value directly; the other windows retain their ordinary tree handles.
pub(crate) struct TransitionWindowAccess<'state, 'selected> {
    keyboard_focus: &'state mut Option<Weak<RefCell<GameWindow>>>,
    modal_stack: &'state mut Option<Box<ModalWindow>>,
    mouse_capture: &'state mut Option<Weak<RefCell<GameWindow>>>,
    capture_flags: &'state mut CaptureFlags,
    selected: Option<&'selected mut GameWindow>,
}
impl<'state, 'selected> TransitionWindowAccess<'state, 'selected> {
    pub(super) fn new(
        keyboard_focus: &'state mut Option<Weak<RefCell<GameWindow>>>,
        modal_stack: &'state mut Option<Box<ModalWindow>>,
        mouse_capture: &'state mut Option<Weak<RefCell<GameWindow>>>,
        capture_flags: &'state mut CaptureFlags,
        selected: Option<&'selected mut GameWindow>,
    ) -> Self {
        Self {
            keyboard_focus,
            modal_stack,
            mouse_capture,
            capture_flags,
            selected,
        }
    }
    pub(crate) fn borrow_window<'a>(
        &'a self,
        target: &'a Rc<RefCell<GameWindow>>,
    ) -> TransitionWindowRead<'a> {
        if let Some(selected) = self.selected.as_deref() {
            if std::ptr::eq(target.as_ptr(), std::ptr::from_ref(selected)) {
                return TransitionWindowRead::Selected(selected);
            }
        }
        TransitionWindowRead::Stored(target.borrow())
    }
    pub(crate) fn borrow_window_mut<'a>(
        &'a mut self,
        target: &'a Rc<RefCell<GameWindow>>,
    ) -> TransitionWindowWrite<'a> {
        if let Some(selected) = self.selected.as_deref_mut() {
            if std::ptr::eq(target.as_ptr(), std::ptr::from_ref(&*selected)) {
                return TransitionWindowWrite::Selected(selected);
            }
        }
        TransitionWindowWrite::Stored(target.borrow_mut())
    }
    pub(crate) fn with_window<R>(
        &self,
        target: &Rc<RefCell<GameWindow>>,
        read: impl FnOnce(&GameWindow) -> R,
    ) -> R {
        read(&self.borrow_window(target))
    }
    pub(crate) fn hide_window(&mut self, target: &Rc<RefCell<GameWindow>>, hidden: bool) {
        let children = {
            let mut window = self.borrow_window_mut(target);
            window.set_hidden_status(hidden);
            window.children().to_vec()
        };
        if hidden {
            self.hiding(target.as_ptr(), children);
        }
    }
    pub(super) fn hiding(
        &mut self,
        target: *const GameWindow,
        children: Vec<Rc<RefCell<GameWindow>>>,
    ) {
        if self
            .keyboard_focus
            .as_ref()
            .and_then(Weak::upgrade)
            .is_some_and(|window| std::ptr::eq(window.as_ptr(), target.cast_mut()))
        {
            *self.keyboard_focus = None;
        }
        if self
            .modal_stack
            .as_ref()
            .is_some_and(|modal| std::ptr::eq(modal.window.as_ptr(), target.cast_mut()))
        {
            if let Some(modal) = self.modal_stack.take() {
                *self.modal_stack = modal.next;
            }
        }
        if self
            .mouse_capture
            .as_ref()
            .and_then(Weak::upgrade)
            .is_some_and(|window| std::ptr::eq(window.as_ptr(), target.cast_mut()))
        {
            *self.mouse_capture = None;
            *self.capture_flags &= !CaptureFlags::MOUSE;
        }
        for child in children {
            let children = self.with_window(&child, |window| window.children().to_vec());
            self.hiding(child.as_ptr(), children);
        }
    }
}

impl WindowManager {
    pub(crate) fn transition_parts<'a>(
        &'a mut self,
        selected: Option<&'a mut GameWindow>,
    ) -> (
        &'a mut crate::gui::game_window_transitions::GameWindowTransitionsHandler,
        &'a std::collections::HashMap<i32, Weak<RefCell<GameWindow>>>,
        TransitionWindowAccess<'a, 'a>,
    ) {
        (
            &mut self.transitions,
            &self.window_by_id,
            TransitionWindowAccess::new(
                &mut self.keyboard_focus,
                &mut self.modal_stack,
                &mut self.mouse_capture,
                &mut self.capture_flags,
                selected,
            ),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn selected_window_uses_canonical_state_and_clears_only_its_owner() {
        let window = Rc::new(RefCell::new(GameWindow::new()));
        window.borrow_mut().set_user_data(123u32);
        let mut owner = WindowManager::new();
        owner.keyboard_focus = Some(Rc::downgrade(&window));
        owner.mouse_capture = Some(Rc::downgrade(&window));
        owner.capture_flags = CaptureFlags::MOUSE;
        let peer_window = Rc::new(RefCell::new(GameWindow::new()));
        let mut peer = WindowManager::new();
        peer.keyboard_focus = Some(Rc::downgrade(&peer_window));
        let mut selected = window.borrow_mut();
        {
            let (_, _, mut access) = owner.transition_parts(Some(&mut selected));
            assert_eq!(
                access.with_window(&window, |w| w.get_user_data::<u32>().copied()),
                Some(123)
            );
            access.hide_window(&window, true);
            assert!(access.with_window(&window, GameWindow::is_hidden));
        }
        assert!(selected.is_hidden());
        assert!(owner.keyboard_focus.is_none());
        assert!(owner.mouse_capture.is_none());
        assert!(!owner.capture_flags.contains(CaptureFlags::MOUSE));
        assert!(
            peer.keyboard_focus
                .as_ref()
                .and_then(Weak::upgrade)
                .is_some_and(|w| Rc::ptr_eq(&w, &peer_window))
        );
    }
}
