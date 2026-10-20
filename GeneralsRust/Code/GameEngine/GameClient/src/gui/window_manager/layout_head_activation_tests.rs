//! Actual local manager + WindowLayout::run_init loan, without ambient manager setup.
use super::*;

#[test]
fn actual_init_callback_activates_layout_head_immediately_under_shared_layout_loan() {
    let manager = Rc::new(RefCell::new(WindowManager::new()));
    let (layout, older, target, unrelated) = {
        let mut manager = manager.borrow_mut();
        let layout = manager.create_layout("HeadActivation.wnd".into());
        let older = manager.create_window(None, 0, 0, 40, 40).unwrap();
        layout.borrow_mut().add_window(older.clone());
        let target = manager.create_window(None, 40, 0, 40, 40).unwrap();
        layout.borrow_mut().add_window(target.clone());
        let unrelated = manager.create_window(None, 80, 0, 40, 40).unwrap();
        target.borrow_mut().hide(true).unwrap();
        assert!(Rc::ptr_eq(&manager.root_windows[0], &unrelated));
        (layout, older, target, unrelated)
    };
    let calls = Rc::new(Cell::new(0));
    let weak_manager = Rc::downgrade(&manager);
    let callback_calls = calls.clone();
    let callback_target = target.clone();
    let callback_older = older.clone();
    let callback_unrelated = unrelated.clone();
    layout.borrow_mut().init_callback = Some(Box::new(move |observed_layout, _| {
        let manager = weak_manager.upgrade().unwrap();
        manager
            .borrow_mut()
            .activate_window(&callback_target)
            .unwrap();
        callback_calls.set(callback_calls.get() + 1);
        // Observe effects before the callback or shared layout loan ends.
        let manager = manager.borrow();
        assert!(Rc::ptr_eq(&manager.root_windows[0], &callback_target));
        assert!(Rc::ptr_eq(&manager.root_windows[1], &callback_unrelated));
        assert!(Rc::ptr_eq(&manager.root_windows[2], &callback_older));
        assert!(
            callback_target
                .borrow()
                .get_status()
                .contains(WindowStatus::ACTIVE)
        );
        assert!(!callback_target.borrow().is_hidden());
        assert!(Rc::ptr_eq(&observed_layout.windows()[0], &callback_target));
        assert!(Rc::ptr_eq(&observed_layout.windows()[1], &callback_older));
        assert!(callback_target.borrow().get_prev_in_layout().is_none());
        assert!(Rc::ptr_eq(
            &callback_target.borrow().get_next_in_layout().unwrap(),
            &callback_older
        ));
        assert!(Rc::ptr_eq(
            &callback_older.borrow().get_prev_in_layout().unwrap(),
            &callback_target
        ));
    }));
    {
        // BasicWindowLayout::run_init and ScoreScreen use exactly this loan shape.
        let observed_layout = layout.borrow();
        observed_layout.run_init(None);
    }
    assert_eq!(calls.get(), 1);
    assert!(Rc::ptr_eq(&layout.borrow().windows()[0], &target));
    // Second activation is also safe and does not reorder the layout.
    layout.borrow().run_init(None);
    assert_eq!(calls.get(), 2);
}

#[test]
fn ordinary_nonfirst_activation_still_promotes_manager_and_layout_synchronously() {
    let mut manager = WindowManager::new();
    let layout = manager.create_layout("OrdinaryPromotion.wnd".into());
    let first = manager.create_window(None, 0, 0, 40, 40).unwrap();
    layout.borrow_mut().add_window(first.clone());
    let second = manager.create_window(None, 40, 0, 40, 40).unwrap();
    layout.borrow_mut().add_window(second.clone());
    let unrelated = manager.create_window(None, 80, 0, 40, 40).unwrap();
    assert!(Rc::ptr_eq(&layout.borrow().windows()[0], &second));
    manager.activate_window(&first).unwrap();
    assert!(Rc::ptr_eq(&manager.root_windows[0], &first));
    assert!(Rc::ptr_eq(&manager.root_windows[1], &unrelated));
    assert!(Rc::ptr_eq(&layout.borrow().windows()[0], &first));
    assert!(Rc::ptr_eq(&layout.borrow().windows()[1], &second));
    assert!(first.borrow().get_prev_in_layout().is_none());
    assert!(Rc::ptr_eq(
        &first.borrow().get_next_in_layout().unwrap(),
        &second
    ));
    assert!(Rc::ptr_eq(
        &second.borrow().get_prev_in_layout().unwrap(),
        &first
    ));
}
