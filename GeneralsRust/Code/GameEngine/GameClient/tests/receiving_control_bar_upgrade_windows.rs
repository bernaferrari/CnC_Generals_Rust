use game_client_rust::gui::control_bar::ControlBar;
use game_engine::common::ini::{
    INI,
    ini_mapped_image::{ICoord2D, Image, ImageCollection, OwnedMappedImages},
};
use gamelogic::upgrade::center::UpgradeCenter;

#[test]
fn each_bar_applies_sparse_upgrade_cameos_to_its_own_windows() {
    let mut catalog = UpgradeCenter::new();
    catalog.new_upgrade("Upgrade_WindowSecond".into());
    let mut ini = INI::new();
    ini.with_inline_source(
        "Upgrade Upgrade_WindowFifth\n ButtonImage = CameoWindowTestImage\nEnd\n",
        |ini| {
            ini.read_line()?;
            catalog.parse_upgrade_definition(ini)
        },
    )
    .unwrap();
    let mut image = Image::new();
    image.set_name("CameoWindowTestImage".into());
    image.set_image_size(ICoord2D::new(32, 24));
    let mut collection = ImageCollection::new();
    collection.add_image(image);
    let images = OwnedMappedImages::from_collection(&collection);
    let mut owner = ControlBar::new();
    let mut peer = ControlBar::new();
    owner.set_receiving_upgrade_definitions(&catalog, None);
    owner.set_receiving_cameo_images(&images);
    peer.set_receiving_upgrade_definitions(&catalog, None);
    let slots = [
        "",
        "Upgrade_WindowSecond",
        "",
        "Upgrade_Missing",
        "Upgrade_WindowFifth",
    ]
    .map(str::to_owned);
    owner.sync_upgrade_cameos_from_presentation(
        &slots,
        &["Upgrade_WindowFifth".into()],
        None,
        false,
        0.0,
        0.0,
    );
    peer.sync_upgrade_cameos_from_presentation(
        &["Upgrade_WindowSecond".into()],
        &[],
        None,
        false,
        0.0,
        0.0,
    );
    let mut first = game_client_rust::core::GameClient::new().unwrap();
    let mut second = game_client_rust::core::GameClient::new().unwrap();
    first.set_in_game_ui_subsystem(Default::default());
    second.set_in_game_ui_subsystem(Default::default());
    first.with_window_event_effects(None, |effects| {
        effects.with_manager(|manager, effects| {
            let mut windows = Vec::new();
            for n in 1..=5 {
                let window = manager.create_window(None, 0, 0, 40, 40, effects).unwrap();
                window
                    .borrow_mut()
                    .set_name(&format!("ControlBar.wnd:UnitUpgrade{n}"));
                window.borrow_mut().enable(true).unwrap();
                windows.push(window);
            }
            owner.apply_upgrade_cameos_to_windows(manager);
            owner.sync_upgrade_cameos_from_presentation(
                &slots,
                &["Upgrade_WindowFifth".into()],
                None,
                false,
                0.0,
                0.0,
            );
            owner.apply_upgrade_cameos_to_windows(manager);
            assert!(owner.get_portrait_state().upgrade_cameos[1].is_visible);
            assert!(windows[0].borrow().is_hidden());
            assert!(!windows[1].borrow().is_hidden());
            assert!(!windows[1].borrow().is_enabled());
            assert!(windows[2].borrow().is_hidden() && windows[3].borrow().is_hidden());
            assert!(!windows[4].borrow().is_hidden() && windows[4].borrow().is_enabled());
            assert_eq!(
                windows[4]
                    .borrow()
                    .get_enabled_draw_data(0)
                    .unwrap()
                    .image
                    .unwrap()
                    .name,
                "CameoWindowTestImage"
            );
        })
    });
    second.with_window_event_effects(None, |effects| {
        effects.with_manager(|manager, effects| {
            let window = manager.create_window(None, 0, 0, 40, 40, effects).unwrap();
            window.borrow_mut().set_name("ControlBar.wnd:UnitUpgrade1");
            window.borrow_mut().enable(true).unwrap();
            peer.apply_upgrade_cameos_to_windows(manager);
            peer.sync_upgrade_cameos_from_presentation(
                &["Upgrade_WindowSecond".into()],
                &[],
                None,
                false,
                0.0,
                0.0,
            );
            peer.apply_upgrade_cameos_to_windows(manager);
            assert!(!window.borrow().is_hidden());
            assert!(!window.borrow().is_enabled());
        })
    });
    first.with_window_event_effects(None, |effects| {
        effects.with_manager(|manager, effects| {
            let first_window = manager
                .find_window_by_name("ControlBar.wnd:UnitUpgrade1")
                .unwrap();
            let fifth = manager
                .find_window_by_name("ControlBar.wnd:UnitUpgrade5")
                .unwrap();
            assert!(first_window.borrow().is_hidden());
            assert!(!fifth.borrow().is_hidden() && fifth.borrow().is_enabled());
            owner.set_receiving_cameo_images(&OwnedMappedImages::default());
            owner.sync_upgrade_cameos_from_presentation(
                &slots,
                &["Upgrade_WindowFifth".into()],
                None,
                false,
                0.0,
                0.0,
            );
            owner.apply_upgrade_cameos_to_windows(manager);
            assert!(
                fifth
                    .borrow()
                    .get_enabled_draw_data(0)
                    .unwrap()
                    .image
                    .is_none()
            );
            owner.sync_upgrade_cameos_from_presentation(&[], &[], None, false, 0.0, 0.0);
            owner.apply_upgrade_cameos_to_windows(manager);
            assert!(fifth.borrow().is_hidden());
        })
    });
}
