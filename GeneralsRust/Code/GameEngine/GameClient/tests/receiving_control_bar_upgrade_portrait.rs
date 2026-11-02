use std::sync::{Arc, RwLock};

use game_client_rust::gui::control_bar::ControlBar;
use game_engine::common::ini::INILoadType;
use game_engine::common::name_key_generator::NameKeyNamespace;
use game_engine::common::thing::{
    module_factory::ModuleFactory, receiving_module_content::ReceivingModuleContent,
};
use gamelogic::object::registry::OBJECT_REGISTRY;
use gamelogic::upgrade::center::{UpgradeCenter, with_upgrade_center_mut};

#[test]
fn portrait_uses_receiving_upgrade_masks_and_hides_missing_definitions() {
    const ID: u32 = 719_202;
    const NAME: &str = "Upgrade_PortraitReceiving";
    const TEMPLATE: &str = "PortraitReceivingObject";
    let modules = ModuleFactory::new_receiving(NameKeyNamespace::new());
    let mut content = ReceivingModuleContent::new(modules);
    content
        .load_ini_text(
            &format!("Object {TEMPLATE}\n UpgradeCameo1 = {NAME}\nEnd\n"),
            INILoadType::Overwrite,
        )
        .unwrap();
    let previous_factory = game_engine::common::thing::get_thing_factory()
        .unwrap()
        .replace(content.compatibility_template_view());

    let mut owner_catalog = UpgradeCenter::new();
    let owner_upgrade = owner_catalog.new_upgrade(NAME.into());
    let mut peer_catalog = UpgradeCenter::new();
    peer_catalog.new_upgrade("Upgrade_PeerFirst".into());
    peer_catalog.new_upgrade(NAME.into());
    with_upgrade_center_mut(|center| {
        center.new_upgrade("Upgrade_GlobalFirst".into());
        center.new_upgrade("Upgrade_GlobalSecond".into());
        center.new_upgrade(NAME.into());
    });
    let template: Arc<dyn gamelogic::thing_template::ThingTemplate> = Arc::new(
        gamelogic::thing_template::DefaultThingTemplate::new(TEMPLATE.into()),
    );
    let mut raw = gamelogic::object::Object::new_raw(
        template,
        ID,
        gamelogic::common::types::ObjectStatusMaskType::none(),
        None,
    );
    raw.give_upgrade(&owner_upgrade);
    let object = Arc::new(RwLock::new(raw));
    OBJECT_REGISTRY.register_object(ID, &object);

    let mut owner = ControlBar::new();
    let mut peer = ControlBar::new();
    let mut missing = ControlBar::new();
    let mut unadmitted = ControlBar::new();
    owner.set_receiving_upgrade_definitions(&owner_catalog, None);
    peer.set_receiving_upgrade_definitions(&peer_catalog, None);
    missing.set_receiving_upgrade_definitions(&UpgradeCenter::new(), None);
    owner.set_portrait_by_object_id(Some(ID));
    peer.set_portrait_by_object_id(Some(ID));
    missing.set_portrait_by_object_id(Some(ID));
    unadmitted.set_portrait_by_object_id(Some(ID));
    assert_eq!(owner.get_portrait_state().upgrade_cameos.len(), 5);
    assert!(owner.get_portrait_state().upgrade_cameos[0].is_completed);
    assert_eq!(peer.get_portrait_state().upgrade_cameos.len(), 5);
    assert!(!peer.get_portrait_state().upgrade_cameos[0].is_completed);
    assert_eq!(missing.get_portrait_state().upgrade_cameos.len(), 5);
    assert!(!missing.get_portrait_state().upgrade_cameos[0].is_visible);
    assert!(!unadmitted.get_portrait_state().upgrade_cameos[0].is_visible);
    assert!(
        unadmitted.get_portrait_state().upgrade_cameos[0]
            .button_image
            .is_empty()
    );
    owner.set_portrait_by_object_id(Some(ID));
    assert!(owner.get_portrait_state().upgrade_cameos[0].is_completed);
    OBJECT_REGISTRY.unregister_object(ID);
    *game_engine::common::thing::get_thing_factory().unwrap() = previous_factory;
}

#[test]
fn presentation_keeps_sparse_upgrade_cameos_in_their_authored_slots() {
    let mut catalog = UpgradeCenter::new();
    catalog.new_upgrade("Upgrade_SlotSecond".into());
    catalog.new_upgrade("Upgrade_SlotFifth".into());
    let mut bar = ControlBar::new();
    bar.set_receiving_upgrade_definitions(&catalog, None);
    let slots = [
        "",
        "Upgrade_SlotSecond",
        "",
        "Upgrade_Missing",
        "Upgrade_SlotFifth",
    ]
    .map(str::to_owned);
    bar.sync_upgrade_cameos_from_presentation(
        &slots,
        &["Upgrade_SlotFifth".to_owned()],
        None,
        false,
        0.0,
        0.0,
    );
    let cameos = &bar.get_portrait_state().upgrade_cameos;
    assert_eq!(cameos.len(), 5);
    assert!(!cameos[0].is_visible);
    assert_eq!(cameos[1].upgrade_name, "Upgrade_SlotSecond");
    assert!(cameos[1].is_visible);
    assert!(!cameos[2].is_visible);
    assert_eq!(cameos[3].upgrade_name, "Upgrade_Missing");
    assert!(!cameos[3].is_visible);
    assert_eq!(cameos[4].upgrade_name, "Upgrade_SlotFifth");
    assert!(cameos[4].is_visible && cameos[4].is_completed);
}
