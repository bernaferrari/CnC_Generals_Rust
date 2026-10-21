//! Local WindowManager instances and actual public file loaders; no shared test mutex.
use super::*;

struct Baseline {
    count: usize,
    focus: Option<Rc<RefCell<GameWindow>>>,
    roots: Vec<Rc<RefCell<GameWindow>>>,
    layouts: Vec<Rc<RefCell<WindowLayout>>>,
    lookup: HashMap<WindowId, Weak<RefCell<GameWindow>>>,
    tabs: Vec<Weak<RefCell<GameWindow>>>,
    deferred: VecDeque<Rc<RefCell<GameWindow>>>,
}
impl Baseline {
    fn capture(manager: &WindowManager) -> Self {
        Self {
            count: manager.window_count,
            focus: manager.get_focus(),
            roots: manager.root_windows.clone(),
            layouts: manager.layouts.clone(),
            lookup: manager.window_by_id.clone(),
            tabs: manager.tab_list.clone(),
            deferred: manager.destroy_queue.clone(),
        }
    }
    fn assert_restored(&self, manager: &WindowManager) {
        assert_eq!(manager.window_count, self.count);
        match (manager.get_focus(), &self.focus) {
            (Some(actual), Some(expected)) => assert!(Rc::ptr_eq(&actual, expected)),
            (None, None) => {}
            _ => panic!("layout failure changed the preexisting focus"),
        }
        assert_eq!(manager.root_windows.len(), self.roots.len());
        for (actual, expected) in manager.root_windows.iter().zip(&self.roots) {
            assert!(Rc::ptr_eq(actual, expected));
        }
        assert_eq!(manager.layouts.len(), self.layouts.len());
        for (actual, expected) in manager.layouts.iter().zip(&self.layouts) {
            assert!(Rc::ptr_eq(actual, expected));
        }
        assert_eq!(manager.window_by_id.len(), self.lookup.len());
        for (id, expected) in &self.lookup {
            assert!(Weak::ptr_eq(
                manager.window_by_id.get(id).unwrap(),
                expected
            ));
        }
        assert_eq!(manager.tab_list.len(), self.tabs.len());
        for (actual, expected) in manager.tab_list.iter().zip(&self.tabs) {
            assert!(Weak::ptr_eq(actual, expected));
        }
        assert_eq!(manager.destroy_queue.len(), self.deferred.len());
        for (actual, expected) in manager.destroy_queue.iter().zip(&self.deferred) {
            assert!(Rc::ptr_eq(actual, expected));
        }
        for window in &self.roots {
            assert!(
                !window
                    .borrow()
                    .get_status()
                    .contains(WindowStatus::DESTROYED)
            );
        }
    }
}
fn local_manager(free_windows: usize, deferred: bool) -> WindowManager {
    let mut manager = WindowManager::new();
    let layout = manager.create_layout("unrelated".into());
    let existing_id = NameKeyGenerator::name_to_key("Existing") as WindowId;
    let existing = manager
        .create_window_with_id(None, 0, 0, 10, 10, existing_id)
        .unwrap();
    layout.borrow_mut().add_window(existing.clone());
    manager.register_tab_list(vec![existing.clone()]);
    manager.set_focus(Some(&existing)).unwrap();
    while manager.window_count < MAX_WINDOWS - free_windows {
        manager.create_window(None, 0, 0, 10, 10).unwrap();
    }
    if deferred {
        manager.destroy_window(existing).unwrap();
    }
    manager
}
fn root(name: &str) -> String {
    format!(
        "WINDOW\nWINDOWTYPE=USER;\nNAME=\"{name}\";\nSTATUS=ENABLED+TABSTOP;\nSCREENRECT=0 0 100 100;\nEND\n"
    )
}
fn load(
    manager: &mut WindowManager,
    path: &Path,
    with_layout: bool,
) -> WindowResult<WindowLayoutInfo> {
    if with_layout {
        manager
            .create_layout_with_windows(path.to_str().unwrap())
            .map(|(_, info)| info)
    } else {
        manager.create_windows_from_script(path.to_str().unwrap())
    }
}

#[test]
fn public_layout_loaders_remove_first_root_when_second_hits_real_capacity() {
    for with_layout in [false, true] {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("PartialRoot.wnd");
        // Reuse a live unrelated ID: rollback must restore its old lookup.
        std::fs::write(&path, root("Existing") + root("RejectedSecond").as_str()).unwrap();
        let mut manager = local_manager(1, true);
        let before = Baseline::capture(&manager);
        assert!(matches!(
            load(&mut manager, &path, with_layout),
            Err(WindowError::OutOfWindows)
        ));
        before.assert_restored(&manager);
    }
}
#[test]
fn public_layout_loaders_remove_root_and_first_child_when_later_child_hits_capacity() {
    for with_layout in [false, true] {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("PartialChild.wnd");
        let first = root("Existing");
        let child = root("CreatedChild");
        let last = root("RejectedChild");
        std::fs::write(
            &path,
            first.trim_end().strip_suffix("END").unwrap().to_string()
                + child.as_str()
                + last.as_str()
                + "END\n",
        )
        .unwrap();
        let mut manager = local_manager(2, false);
        let before = Baseline::capture(&manager);
        assert!(matches!(
            load(&mut manager, &path, with_layout),
            Err(WindowError::OutOfWindows)
        ));
        before.assert_restored(&manager);
        assert!(
            !manager
                .window_by_id
                .contains_key(&(NameKeyGenerator::name_to_key("CreatedChild") as WindowId))
        );
    }
}
#[test]
fn successful_public_layout_loaders_keep_script_root_and_child_order() {
    for with_layout in [false, true] {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("Success.wnd");
        let first = root("SuccessParent");
        std::fs::write(
            &path,
            first.trim_end().strip_suffix("END").unwrap().to_string()
                + root("SuccessChild").as_str()
                + "END\n"
                + root("SuccessSecond").as_str(),
        )
        .unwrap();
        let mut manager = local_manager(4, false);
        let count = manager.window_count;
        let layouts = manager.layouts.len();
        let info = load(&mut manager, &path, with_layout).unwrap();
        assert_eq!(manager.window_count, count + 3);
        assert_eq!(manager.layouts.len(), layouts + 1);
        assert_eq!(info.windows.len(), 2, "C++ info contains roots only");
        assert_eq!(
            info.windows[0].borrow().get_id(),
            NameKeyGenerator::name_to_key("SuccessParent") as WindowId
        );
        assert_eq!(
            info.windows[1].borrow().get_id(),
            NameKeyGenerator::name_to_key("SuccessSecond") as WindowId
        );
        let children = info.windows[0].borrow().children().to_vec();
        assert_eq!(children.len(), 1);
        assert_eq!(
            children[0].borrow().get_id(),
            NameKeyGenerator::name_to_key("SuccessChild") as WindowId
        );
    }
}
#[test]
fn parser_failure_never_registers_a_candidate_layout() {
    for with_layout in [false, true] {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("Bad.wnd");
        std::fs::write(&path, "END;").unwrap();
        let mut manager = local_manager(3, false);
        let before = Baseline::capture(&manager);
        assert!(load(&mut manager, &path, with_layout).is_err());
        before.assert_restored(&manager);
    }
}

#[test]
fn empty_script_has_no_first_window_and_never_leaves_an_empty_layout() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("Empty.wnd");
    std::fs::write(&path, "").unwrap();
    let mut manager = local_manager(3, false);
    let before = Baseline::capture(&manager);
    for with_layout in [false, true] {
        assert!(matches!(
            load(&mut manager, &path, with_layout),
            Err(WindowError::InvalidParameter)
        ));
        before.assert_restored(&manager);
    }
    assert!(matches!(
        manager.load_window(path.to_str().unwrap()),
        Err(WindowError::InvalidParameter)
    ));
    before.assert_restored(&manager);
}

fn install_selected_original_radio(manager: &mut WindowManager) -> (u32, u32) {
    let existing_id = NameKeyGenerator::name_to_key("Existing") as WindowId;
    let gadget_id = if existing_id > 0 {
        existing_id as u32
    } else {
        0
    };
    let group_id = 77;
    let group = RadioButtonGroup::new(group_id);
    let mut original = RadioButton::new(gadget_id, 0, 0, 10, group.clone());
    original.select();
    manager
        .window_by_id
        .get(&existing_id)
        .unwrap()
        .upgrade()
        .unwrap()
        .borrow_mut()
        .set_widget(WindowWidget::RadioButton(original));
    manager.radio_groups.insert(group_id, group);
    (group_id, gadget_id)
}

#[test]
fn public_capacity_failure_restores_preselected_radio_group_after_colliding_candidate_drop() {
    for with_layout in [false, true] {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("RadioCollision.wnd");
        std::fs::write(&path,
            "WINDOW\nWINDOWTYPE=RADIOBUTTON;\nNAME=\"Existing\";\nSCREENRECT=0 0 10 10;\nRADIOBUTTONDATA=GROUP:77;\nEND\n".to_string()
                + root("CapacityRejected").as_str()).unwrap();
        let mut manager = local_manager(1, false);
        let (group_id, selected) = install_selected_original_radio(&mut manager);
        let baseline = Baseline::capture(&manager);
        let members = manager.radio_groups[&group_id].get_buttons();
        assert_eq!(
            manager.radio_groups[&group_id].get_selected(),
            Some(selected)
        );
        assert!(matches!(
            load(&mut manager, &path, with_layout),
            Err(WindowError::OutOfWindows)
        ));
        baseline.assert_restored(&manager);
        assert_eq!(manager.radio_groups[&group_id].get_buttons(), members);
        assert_eq!(
            manager.radio_groups[&group_id].get_selected(),
            Some(selected)
        );
        // Later unrelated manager work cannot deliver a deferred candidate Drop
        // that removes the original ID after the beforeimage was restored.
        manager.flush_destroy_queue();
        assert_eq!(manager.radio_groups[&group_id].get_buttons(), members);
        assert_eq!(
            manager.radio_groups[&group_id].get_selected(),
            Some(selected)
        );
    }
}

#[test]
fn rollback_reclaims_selected_candidate_widget_before_restoring_existing_group() {
    let mut manager = local_manager(1, false);
    let (group_id, original_selected) = install_selected_original_radio(&mut manager);
    let baseline = Baseline::capture(&manager);
    let members = manager.radio_groups[&group_id].get_buttons();
    let before = manager.capture_layout_registrations();
    let layout = manager.create_layout("SelectedCandidate".into());
    let definition = WindowLayoutDefinition::default();
    let mut info = WindowLayoutInfo {
        version: 0,
        init_callback_name: String::new(),
        update_callback_name: String::new(),
        shutdown_callback_name: String::new(),
        windows: Vec::new(),
    };
    let candidate_name = (0..1024)
        .map(|i| format!("SelectedCandidateRadio{i}"))
        .find(|name| {
            let id = NameKeyGenerator::name_to_key(name) as WindowId;
            id > 0 && id as u32 != original_selected
        })
        .expect("fixture needs a distinct positive radio gadget ID");
    let candidate_definition = WindowDefinition {
        name: candidate_name,
        window_type: "RADIOBUTTON".into(),
        size: (10, 10),
        radio_button_data: Some(crate::gui::window_script::RadioButtonData { group: group_id }),
        ..Default::default()
    };
    let candidate = manager
        .create_window_from_definition(&candidate_definition, None, &layout, &definition, &mut info)
        .unwrap();
    let candidate_id = {
        let mut window = candidate.borrow_mut();
        let WindowWidget::RadioButton(radio) = window.widget_mut().unwrap() else {
            panic!("actual radio widget")
        };
        radio.select();
        manager.radio_groups[&group_id].get_selected().unwrap()
    };
    assert_ne!(candidate_id, original_selected);
    assert!(
        manager.radio_groups[&group_id]
            .get_buttons()
            .contains(&candidate_id)
    );
    let error = manager.create_window_from_definition(
        &WindowDefinition {
            name: "RejectedChild".into(),
            size: (10, 10),
            ..Default::default()
        },
        Some(&candidate),
        &layout,
        &definition,
        &mut info,
    );
    assert!(
        matches!(error, Err(WindowError::OutOfWindows)),
        "actual manager capacity failure after candidate selection"
    );
    manager.rollback_failed_layout(layout, info, before);
    baseline.assert_restored(&manager);
    assert_eq!(manager.radio_groups[&group_id].get_buttons(), members);
    assert_eq!(
        manager.radio_groups[&group_id].get_selected(),
        Some(original_selected)
    );
    assert!(
        candidate
            .borrow()
            .get_status()
            .contains(WindowStatus::DESTROYED)
    );
    assert!(
        candidate.borrow().widget().is_none(),
        "escaped destroyed-window Rc cannot retain a late RadioButton::Drop"
    );
    drop(candidate);
    assert_eq!(manager.radio_groups[&group_id].get_buttons(), members);
    assert_eq!(
        manager.radio_groups[&group_id].get_selected(),
        Some(original_selected)
    );
}

#[test]
fn public_layout_loaders_reparse_same_path_and_keep_prior_live_windows() {
    for with_layout in [false, true] {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("FreshLayout.wnd");
        std::fs::write(&path, root("FreshFirst")).unwrap();
        let mut first_owner = WindowManager::new();
        let first = load(&mut first_owner, &path, with_layout).unwrap();
        let first_window = first.windows[0].clone();
        let first_id = NameKeyGenerator::name_to_key("FreshFirst") as WindowId;
        assert_eq!(first_window.borrow().get_id(), first_id);
        assert_eq!(first_owner.window_count, 1);

        std::fs::write(&path, root("FreshSecond") + root("FreshThird").as_str()).unwrap();
        let mut peer = WindowManager::new();
        let second = load(&mut peer, &path, with_layout).unwrap();
        let same_owner = load(&mut first_owner, &path, with_layout).unwrap();
        let second_ids = ["FreshSecond", "FreshThird"]
            .map(|name| NameKeyGenerator::name_to_key(name) as WindowId);
        for info in [&second, &same_owner] {
            assert_eq!(info.windows.len(), 2);
            for (window, expected_id) in info.windows.iter().zip(second_ids) {
                assert_eq!(window.borrow().get_id(), expected_id);
            }
        }
        assert_eq!(peer.window_count, 2);
        assert_eq!(first_owner.window_count, 3);
        assert_eq!(first_window.borrow().get_id(), first_id);
        assert!(
            !first_window
                .borrow()
                .get_status()
                .contains(WindowStatus::DESTROYED)
        );
        assert!(Rc::ptr_eq(
            &first_owner.get_window_by_id(first_id).unwrap(),
            &first_window
        ));

        // A new invalid source must fail before registering another layout;
        // an old valid parse cannot conceal the failure or the subsequent IO error.
        let first_before = Baseline::capture(&first_owner);
        let peer_before = Baseline::capture(&peer);
        std::fs::write(&path, "END;").unwrap();
        assert!(load(&mut first_owner, &path, with_layout).is_err());
        first_before.assert_restored(&first_owner);
        peer_before.assert_restored(&peer);
        std::fs::remove_file(&path).unwrap();
        assert!(load(&mut first_owner, &path, with_layout).is_err());
        first_before.assert_restored(&first_owner);
        peer_before.assert_restored(&peer);

        peer.reset();
        assert_eq!(peer.window_count, 0);
        first_before.assert_restored(&first_owner);
    }
}
