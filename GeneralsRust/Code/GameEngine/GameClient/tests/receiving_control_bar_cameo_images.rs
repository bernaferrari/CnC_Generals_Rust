use game_client_rust::gui::control_bar::ControlBar;
use game_engine::common::ini::{
    INI,
    ini_mapped_image::{ICoord2D, Image, ImageCollection, OwnedMappedImages},
};
use gamelogic::upgrade::center::UpgradeCenter;

// OLD control: an implementation without admission continues using its shared
// image collection. Once the inherent method exists, it takes precedence.
trait ImageAdmission {
    fn set_receiving_cameo_images(&mut self, images: &OwnedMappedImages);
}
impl ImageAdmission for ControlBar {
    fn set_receiving_cameo_images(&mut self, _: &OwnedMappedImages) {}
}

fn images(width: i32) -> OwnedMappedImages {
    let mut source = ImageCollection::new();
    let mut image = Image::new();
    image.set_name("Cameo_SameName".into());
    image.set_image_size(ICoord2D::new(width, 24));
    source.add_image(image);
    OwnedMappedImages::from_collection(&source)
}

#[test]
fn cameo_image_metadata_comes_from_each_receiving_catalog() {
    let mut global = game_client_rust::display::image::Image::with_name("Cameo_SameName");
    global.set_image_size(game_client_rust::display::image::ICoord2D::new(99, 24));
    game_client_rust::display::image::get_mapped_image_collection()
        .write()
        .add_image(global);
    let mut upgrades = UpgradeCenter::new();
    let mut ini = INI::new();
    ini.with_inline_source(
        "Upgrade Upgrade_CameoOwned\n ButtonImage = Cameo_SameName\nEnd\n",
        |ini| {
            ini.read_line()?;
            upgrades.parse_upgrade_definition(ini)
        },
    )
    .unwrap();
    let mut first = ControlBar::new();
    let mut peer = ControlBar::new();
    first.set_receiving_upgrade_definitions(&upgrades, None);
    peer.set_receiving_upgrade_definitions(&upgrades, None);
    first.set_receiving_cameo_images(&images(32));
    peer.set_receiving_cameo_images(&images(64));
    for bar in [&mut first, &mut peer] {
        bar.sync_upgrade_cameos_from_presentation(
            &["Upgrade_CameoOwned".into()],
            &[],
            None,
            false,
            0.0,
            0.0,
        );
    }
    let mut first_client = game_client_rust::core::GameClient::new().unwrap();
    let mut peer_client = game_client_rust::core::GameClient::new().unwrap();
    fn check(client: &mut game_client_rust::core::GameClient, bar: &ControlBar, width: i32) {
        client.with_window_event_effects(None, |effects| {
            effects.with_manager(|manager, effects| {
                let window = manager
                    .find_window_by_name("ControlBar.wnd:UnitUpgrade1")
                    .unwrap_or_else(|| {
                        let window = manager.create_window(None, 0, 0, 40, 40, effects).unwrap();
                        window.borrow_mut().set_name("ControlBar.wnd:UnitUpgrade1");
                        window
                    });
                bar.apply_upgrade_cameos_to_windows(manager);
                let image = window
                    .borrow()
                    .get_enabled_draw_data(0)
                    .unwrap()
                    .image
                    .unwrap();
                assert_eq!(image.width, width);
            })
        });
    }
    check(&mut first_client, &first, 32);
    check(&mut peer_client, &peer, 64);
    check(&mut first_client, &first, 32);
    first.set_receiving_cameo_images(&images(48));
    check(&mut first_client, &first, 48);
    check(&mut peer_client, &peer, 64);
    first_client.with_window_event_effects(None, |effects| {
        effects.with_manager(|manager, _| first.reset(manager).unwrap())
    });
    first.sync_upgrade_cameos_from_presentation(
        &["Upgrade_CameoOwned".into()],
        &[],
        None,
        false,
        0.0,
        0.0,
    );
    check(&mut first_client, &first, 48);
    check(&mut peer_client, &peer, 64);
}

#[test]
fn cameo_does_not_use_a_command_button_image_when_template_has_none() {
    use game_engine::common::thing::receiving_module_content::ReceivingActionDefinitions;
    use std::sync::Arc;
    let mut upgrades = UpgradeCenter::new();
    upgrades.new_upgrade("Upgrade_NoCameoImage".into());
    let mut actions = ReceivingActionDefinitions::default();
    let button = actions
        .buttons
        .new_command_button("Command_NoCameoImage".into());
    button.upgrade = "Upgrade_NoCameoImage".into();
    button.button_image = "UnrelatedCommandArt".into();
    let mut bar = ControlBar::new();
    bar.set_receiving_upgrade_definitions(&upgrades, Some(Arc::new(actions)));
    bar.sync_upgrade_cameos_from_presentation(
        &["Upgrade_NoCameoImage".into()],
        &[],
        None,
        false,
        0.0,
        0.0,
    );
    assert!(bar.get_portrait_state().upgrade_cameos[0].is_visible);
    assert!(
        bar.get_portrait_state().upgrade_cameos[0]
            .button_image
            .is_empty()
    );
}
