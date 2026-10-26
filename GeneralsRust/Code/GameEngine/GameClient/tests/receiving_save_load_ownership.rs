//! Receiving UI/transport contracts; these fixtures do not claim full WND asset or gameplay parity.
use game_client_rust::core::game_client::{ClientInputAuthority, GameClient};
use game_client_rust::core::subsystems::InGameUISubsystem;
use game_client_rust::gui::callbacks::popup_save_load::*;
use game_client_rust::gui::gadgets::ListBox;
use game_client_rust::gui::{GameWindow, KeyModifiers, WindowLayout, WindowMessage, WindowWidget};
use game_engine::common::name_key_generator::NameKeyGenerator;
use std::{cell::RefCell, rc::Rc};
type Window = Rc<RefCell<GameWindow>>;
struct Scene {
    layout: WindowLayout,
    parent: Window,
    listbox: Window,
}
fn id(name: &str) -> i32 {
    NameKeyGenerator::name_to_key(&format!("PopupSaveLoad.wnd:{name}")) as i32
}
fn client() -> GameClient {
    let mut c = GameClient::new_with_input_authority(ClientInputAuthority::NativeHosted).unwrap();
    c.set_in_game_ui_subsystem(InGameUISubsystem::default());
    c
}
fn entry(filename: &str) -> PopupSaveLoadEntry {
    PopupSaveLoadEntry {
        filename: filename.into(),
        description: filename.into(),
        ..Default::default()
    }
}
fn scene(c: &mut GameClient) -> Scene {
    c.with_window_event_effects(None, |effects| {
        effects.with_manager(|manager, effects| {
            let parent = manager
                .create_window_with_id(None, 0, 0, 500, 500, id("SaveLoadMenu"), effects)
                .unwrap();
            parent
                .borrow_mut()
                .set_name("PopupSaveLoad.wnd:SaveLoadMenu");
            parent
                .borrow_mut()
                .set_system_callback_with_message_sink(save_load_menu_system);
            parent
                .borrow_mut()
                .set_input_callback_with_message_sink(save_load_menu_input);
            let mut layout = WindowLayout::new("Menus/PopupSaveLoad.wnd".into());
            layout.add_window(parent.clone());
            let mut listbox = None;
            for name in [
                "MenuButtonFrame",
                "ButtonBack",
                "ButtonSave",
                "ButtonLoad",
                "ButtonDelete",
                "ListboxGames",
                "LoadConfirmParent",
                "ButtonLoadConfirm",
                "ButtonLoadCancel",
                "OverwriteConfirmParent",
                "ButtonOverwriteConfirm",
                "ButtonOverwriteCancel",
                "SaveDescParent",
                "ButtonSaveDescConfirm",
                "ButtonSaveDescCancel",
                "EntryDesc",
                "DeleteConfirmParent",
                "ButtonDeleteConfirm",
                "ButtonDeleteCancel",
            ] {
                let w = manager
                    .create_window_with_id(Some(&parent), 0, 0, 100, 100, id(name), effects)
                    .unwrap();
                w.borrow_mut()
                    .set_name(&format!("PopupSaveLoad.wnd:{name}"));
                if name == "ListboxGames" {
                    w.borrow_mut()
                        .set_widget(WindowWidget::ListBox(ListBox::new(
                            id(name) as u32,
                            0,
                            0,
                            100,
                            100,
                        )));
                    listbox = Some(w.clone());
                }
                layout.add_window(w);
            }
            Scene {
                layout,
                parent,
                listbox: listbox.unwrap(),
            }
        })
    })
}
fn init(c: &mut GameClient, s: &Scene) {
    c.with_window_event_effects(None, |effects| {
        save_load_menu_init(&s.layout, None, effects)
    });
}
fn select(s: &Scene, row: usize) {
    assert!(
        s.listbox
            .borrow_mut()
            .list_box_mut()
            .unwrap()
            .select_index(row, KeyModifiers::none())
    );
}
fn confirm_load(c: &mut GameClient, s: &Scene) {
    c.with_window_event_effects(None, |effects| {
        let _ = s.parent.borrow_mut().send_system_message(
            WindowMessage::GadgetSelected,
            id("ButtonLoadConfirm") as usize,
            0,
            effects,
        );
    });
}
#[test]
fn identical_control_ids_have_independent_rows_and_confirmed_requests() {
    let mut a = client();
    let mut b = client();
    a.publish_popup_save_load_entries(vec![entry("alpha")]);
    b.publish_popup_save_load_entries(vec![entry("beta")]);
    let sa = scene(&mut a);
    let sb = scene(&mut b);
    init(&mut a, &sa);
    init(&mut b, &sb);
    assert_eq!(sa.parent.borrow().get_id(), sb.parent.borrow().get_id());
    select(&sa, 1);
    select(&sb, 1);
    assert!(a.take_popup_save_load_requests().is_empty());
    assert!(b.take_popup_save_load_requests().is_empty());
    confirm_load(&mut b, &sb);
    let rb = b.take_popup_save_load_requests();
    assert_eq!(rb.len(), 1);
    assert_eq!(
        rb[0].request,
        PopupSaveLoadRequest::Load {
            filename: "beta".into()
        }
    );
    assert!(a.take_popup_save_load_requests().is_empty());
    b.reset().unwrap();
    drop(b);
    confirm_load(&mut a, &sa);
    let ra = a.take_popup_save_load_requests();
    assert_eq!(ra.len(), 1);
    assert_eq!(
        ra[0].request,
        PopupSaveLoadRequest::Load {
            filename: "alpha".into()
        }
    );
    assert!(!ra[0].input_provenance.is_physical_window_mouse_input());
}
#[test]
fn native_empty_inventory_is_authoritative_even_before_first_publication() {
    let mut a = client();
    let sa = scene(&mut a);
    init(&mut a, &sa);
    {
        let mut w = sa.listbox.borrow_mut();
        let list = w.list_box_mut().unwrap();
        assert_eq!(list.items().len(), 1);
        assert!(list.get_item_data(0).is_none());
    }
    confirm_load(&mut a, &sa);
    assert!(a.take_popup_save_load_requests().is_empty());
    a.publish_popup_save_load_entries(vec![entry("owned")]);
    init(&mut a, &sa);
    assert_eq!(
        sa.listbox
            .borrow_mut()
            .list_box_mut()
            .unwrap()
            .items()
            .len(),
        2
    );
    a.publish_popup_save_load_entries(Vec::new());
    init(&mut a, &sa);
    assert_eq!(
        sa.listbox
            .borrow_mut()
            .list_box_mut()
            .unwrap()
            .items()
            .len(),
        1
    );
    assert!(a.popup_save_load_bridge_installed());
}
#[test]
fn open_confirmation_keeps_the_inventory_that_created_its_rows() {
    let mut a = client();
    a.publish_popup_save_load_entries(vec![entry("original")]);
    let s = scene(&mut a);
    init(&mut a, &s);
    select(&s, 1);
    a.publish_popup_save_load_entries(vec![entry("replacement")]);
    confirm_load(&mut a, &s);
    assert_eq!(
        a.take_popup_save_load_requests()[0].request,
        PopupSaveLoadRequest::Load {
            filename: "original".into()
        }
    );
}
