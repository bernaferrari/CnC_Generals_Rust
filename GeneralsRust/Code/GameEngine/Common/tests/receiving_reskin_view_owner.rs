use game_engine::common::ini::INILoadType;
use game_engine::common::name_key_generator::NameKeyNamespace;
use game_engine::common::thing::module_factory::ModuleFactory;
use game_engine::common::thing::receiving_module_content::ReceivingModuleContent;
use game_engine::common::thing::thing_template::ReceivingTemplateKind;

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

#[test]
fn detached_reskin_retains_parent_after_factory_view_is_dropped() {
    let content = owner();
    let view = content.compatibility_template_view();
    let skin = view.find_template("Skin", false).unwrap();
    let parent = view.find_template("Parent", false).unwrap();
    drop(view);
    assert!(skin.is_equivalent_to(&parent));
    assert!(std::sync::Arc::ptr_eq(
        &skin.get_reskinned_from().unwrap(),
        &parent
    ));
}

#[test]
fn dropping_a_view_releases_its_parent_and_reskin() {
    let content = owner();
    let (parent, skin) = {
        let view = content.compatibility_template_view();
        let parent = view.find_template("Parent", false).unwrap();
        let skin = view.find_template("Skin", false).unwrap();
        (
            std::sync::Arc::downgrade(&parent),
            std::sync::Arc::downgrade(&skin),
        )
    };
    assert!(parent.upgrade().is_none());
    assert!(skin.upgrade().is_none());
}

#[test]
fn admission_from_a_detached_reskin_retains_its_selected_parent() {
    let content = owner();
    let view = content.compatibility_template_view();
    let parent = view.find_template("Parent", false).unwrap();
    let skin = view.find_template("Skin", false).unwrap();
    let mut modules = ModuleFactory::new_receiving(NameKeyNamespace::new());
    let admitted = skin
        .admit_named_receiving_module_source(
            "AdmittedSkin",
            &std::collections::HashMap::new(),
            INILoadType::Overwrite,
            ReceivingTemplateKind::Object,
            &mut modules,
        )
        .unwrap();
    assert_eq!(admitted.reskin_name(), Some("Parent"));
    drop(view);
    assert!(std::sync::Arc::ptr_eq(
        &admitted.template().get_reskinned_from().unwrap(),
        &parent
    ));
}

#[test]
fn detached_view_preserves_order_and_overwrite_identity() {
    let mut content = owner();
    content
        .load_ini_text(
            "Object Earlier\nBuildCost = 3\nEnd\nObject Later\nBuildCost = 71\nEnd\n",
            INILoadType::Overwrite,
        )
        .unwrap();
    content
        .load_ini_text(
            "ObjectReskin Earlier Later\nEnd\nObject Parent\nBuildCost = 900\nEnd\n",
            INILoadType::CreateOverrides,
        )
        .unwrap();
    let view = content.compatibility_template_view();
    let earlier = view.find_template("Earlier", false).unwrap();
    let later = view.find_template("Later", false).unwrap();
    let skin = view.find_template("Skin", false).unwrap();
    let parent = view.find_template("Parent", false).unwrap();
    assert!(earlier.is_equivalent_to(&later));
    assert!(skin.is_equivalent_to(&parent));
    assert_eq!(skin.get_build_cost(), 50);
    assert_eq!(parent.get_build_cost(), 900);
    assert!(std::sync::Arc::ptr_eq(
        &skin.get_reskinned_from().unwrap(),
        &parent
    ));
    let mut names = Vec::new();
    let mut node = view.first_template().cloned();
    while let Some(template) = node {
        names.push(template.get_name().to_string());
        node = template.get_next_template().clone();
    }
    assert_eq!(names, ["Later", "Earlier", "Sibling", "Skin", "Parent"]);
    content.reset_map_overrides();
    let reset = content.compatibility_template_view();
    assert_eq!(
        reset
            .find_template("Parent", false)
            .unwrap()
            .get_build_cost(),
        50
    );
    assert_eq!(parent.get_build_cost(), 900, "prior view stays frozen");
    assert!(
        !reset
            .find_template("Earlier", false)
            .unwrap()
            .is_equivalent_to(&reset.find_template("Later", false).unwrap())
    );
}

#[test]
fn detached_view_handles_retail_sized_catalog() {
    let mut content =
        ReceivingModuleContent::new(ModuleFactory::new_receiving(NameKeyNamespace::new()));
    let source = (0..2104)
        .map(|i| format!("Object Unit{i}\nEnd\n"))
        .collect::<String>();
    content
        .load_ini_text(&source, INILoadType::Overwrite)
        .unwrap();
    let view = content.compatibility_template_view();
    assert!(view.find_template("Unit0", false).is_some());
    assert!(view.find_template("Unit2103", false).is_some());
}
