use game_engine::common::ini::INILoadType;
use game_engine::common::name_key_generator::NameKeyNamespace;
use game_engine::common::thing::module_factory::ModuleFactory;
use game_engine::common::thing::receiving_module_content::ReceivingModuleContent;

#[test]
fn same_named_templates_keep_their_own_bridge_flag_and_override() {
    let mut first =
        ReceivingModuleContent::new(ModuleFactory::new_receiving(NameKeyNamespace::new()));
    let mut peer =
        ReceivingModuleContent::new(ModuleFactory::new_receiving(NameKeyNamespace::new()));
    first
        .load_ini_text(
            "Object SharedName\nIsBridge = Yes\nSoundOnDamaged = FirstDamage\nEnd\n",
            INILoadType::Overwrite,
        )
        .unwrap();
    peer.load_ini_text(
        "Object SharedName\nIsBridge = No\nSoundOnDamaged = PeerDamage\nEnd\n",
        INILoadType::Overwrite,
    )
    .unwrap();
    let first_object = first.admitted_template("SharedName").unwrap();
    assert!(first_object.is_bridge());
    assert_eq!(first_object.damage_sounds()[0], Some("FirstDamage"));
    assert_eq!(
        peer.admitted_template("SharedName")
            .unwrap()
            .damage_sounds()[0],
        Some("PeerDamage")
    );
    assert!(!peer.admitted_template("SharedName").unwrap().is_bridge());
    first
        .load_ini_text(
            "Object SharedName\nIsBridge = No\nEnd\n",
            INILoadType::CreateOverrides,
        )
        .unwrap();
    assert!(first_object.is_bridge());
    assert!(!first.admitted_template("SharedName").unwrap().is_bridge());
    drop(first);
    assert!(!peer.admitted_template("SharedName").unwrap().is_bridge());
}
