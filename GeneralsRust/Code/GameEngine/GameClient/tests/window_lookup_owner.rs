//! Depth-first lookup keeps exact receiving-window identity through reentry.
use game_client_rust::core::game_client::{ClientInputAuthority, GameClient};
use std::rc::Rc;

fn client() -> GameClient {
    let mut client =
        GameClient::new_with_input_authority(ClientInputAuthority::NativeHosted).unwrap();
    client.init_input_subsystems().unwrap();
    client
}

#[test]
fn selected_parent_duplicate_names_and_teardown_keep_lookup_order() {
    let mut owner = client();
    let mut peer = client();
    owner.with_window_and_control_bar_effects(None, |effects, _| {
        effects.with_manager(|manager, effects| {
            let root = manager
                .create_window(None, 0, 0, 100, 100, effects)
                .unwrap();
            root.borrow_mut().set_name("Root");
            let first = manager
                .create_window(Some(&root), 0, 0, 40, 40, effects)
                .unwrap();
            first.borrow_mut().set_name("Duplicate");
            let nested = manager
                .create_window(Some(&first), 0, 0, 20, 20, effects)
                .unwrap();
            nested.borrow_mut().set_name("Nested");
            let second = manager
                .create_window(Some(&root), 40, 0, 40, 40, effects)
                .unwrap();
            second.borrow_mut().set_name("Duplicate");
            let expected = root
                .borrow()
                .children()
                .iter()
                .find(|child| child.borrow().get_name() == "Duplicate")
                .unwrap()
                .clone();
            assert!(Rc::ptr_eq(
                &manager.find_window_by_name("dUpLiCaTe").unwrap(),
                &expected
            ));
            // The selected root is mutably borrowed by a synchronous callback.
            // Descendant lookup must use the lent object rather than reborrow it.
            let selected = root.borrow_mut();
            assert!(Rc::ptr_eq(
                &manager
                    .find_window_by_name_with_selected("Nested", Some(&selected))
                    .unwrap(),
                &nested
            ));
            assert!(Rc::ptr_eq(
                &manager
                    .get_window_by_id_with_selected(nested.borrow().get_id(), Some(&selected))
                    .unwrap(),
                &nested
            ));
            drop(selected);
            first.borrow_mut().set_name("Renamed");
            assert!(Rc::ptr_eq(
                &manager.find_window_by_name("Duplicate").unwrap(),
                &second
            ));
            manager.destroy_window(second.clone()).unwrap();
            manager.flush_destroy_queue_with_effects(effects);
            assert!(manager.find_window_by_name("Duplicate").is_none());
            assert!(Rc::ptr_eq(
                &manager.find_window_by_name("Nested").unwrap(),
                &nested
            ));
        });
    });
    peer.with_window_and_control_bar_effects(None, |effects, _| {
        effects.with_manager(|manager, effects| {
            let root = manager.create_window(None, 0, 0, 20, 20, effects).unwrap();
            root.borrow_mut().set_name("Nested");
            assert!(Rc::ptr_eq(
                &manager.find_window_by_name("Nested").unwrap(),
                &root
            ));
        });
    });
}

#[test]
#[ignore = "manual window lookup benchmark"]
fn benchmark_missing_nested_window_lookup() {
    let mut owner = client();
    owner.with_window_and_control_bar_effects(None, |effects, _| {
        effects.with_manager(|manager, effects| {
            let root = manager
                .create_window(None, 0, 0, 100, 100, effects)
                .unwrap();
            for _ in 0..500 {
                let child = manager
                    .create_window(Some(&root), 0, 0, 10, 10, effects)
                    .unwrap();
                child.borrow_mut().set_name("OtherWindow");
            }
            let start = std::time::Instant::now();
            for _ in 0..1000 {
                std::hint::black_box(manager.find_window_by_name("MissingWindow"));
            }
            println!(
                "1000 missing lookups over 501 windows: {:?}",
                start.elapsed()
            );
        });
    });
}
