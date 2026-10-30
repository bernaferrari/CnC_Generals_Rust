use game_engine::common::ini::INILoadType;
use game_engine::common::name_key_generator::NameKeyNamespace;
use game_engine::common::thing::module_factory::ModuleFactory;
use game_engine::common::thing::receiving_module_content::ReceivingModuleContent;

fn owner() -> ReceivingModuleContent {
    let mut owner =
        ReceivingModuleContent::new(ModuleFactory::new_receiving(NameKeyNamespace::new()));
    owner.load_ini_text(
        "Object Parent\nBuildCost = 50\nEnd\nObjectReskin Skin Parent\nEnd\nObjectReskin Sibling Parent\nEnd\n",
        INILoadType::Overwrite,
    ).unwrap();
    owner
}

#[test]
fn detached_reskins_belong_to_their_own_view() {
    let a = owner();
    let b = owner();
    let first = a.compatibility_template_view();
    let peer = b.compatibility_template_view();
    let parent = first.find_template("Parent", false).unwrap();
    let skin = first.find_template("Skin", false).unwrap();
    let sibling = first.find_template("Sibling", false).unwrap();
    let peer_parent = peer.find_template("Parent", false).unwrap();
    assert!(skin.is_equivalent_to(&parent));
    assert!(skin.is_equivalent_to(&sibling));
    assert!(!skin.is_equivalent_to(&peer_parent));
}
