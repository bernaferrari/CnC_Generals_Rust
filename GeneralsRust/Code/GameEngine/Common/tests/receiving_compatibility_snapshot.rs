use std::sync::Arc;

use game_engine::common::ini::INILoadType;
use game_engine::common::name_key_generator::NameKeyNamespace;
use game_engine::common::thing::module_factory::ModuleFactory;
use game_engine::common::thing::receiving_module_content::ReceivingModuleContent;

fn catalog() -> ReceivingModuleContent {
    ReceivingModuleContent::new(ModuleFactory::new_receiving(NameKeyNamespace::new()))
}

#[test]
fn cached_templates_follow_owner_overrides_and_keep_existing_snapshots_isolated() {
    let mut owner = catalog();
    owner
        .load_ini_text(
            "Object DefaultThingTemplate\nEnd\nObject Parent\nBuildCost = 50\nEnd\nChildObject Child Parent\nEnd\nObjectReskin Skin Parent\nFenceWidth = 5.0\nEnd",
            INILoadType::Overwrite,
        )
        .unwrap();
    let original = owner.shared_compatibility_templates();
    assert!(Arc::ptr_eq(
        &original,
        &owner.shared_compatibility_templates()
    ));
    let peer = owner.clone();
    assert!(Arc::ptr_eq(
        &original,
        &peer.shared_compatibility_templates()
    ));
    assert_eq!(original["Parent"].get_build_cost(), 50);
    assert!(!original.contains_key("Missing"));

    owner
        .load_ini_text(
            "Object Parent\nBuildCost = 100\nEnd\nObject MapOnly\nBuildCost = 75\nEnd",
            INILoadType::CreateOverrides,
        )
        .unwrap();
    let changed = owner.shared_compatibility_templates();
    assert!(!Arc::ptr_eq(&original, &changed));
    assert_eq!(changed["Parent"].get_build_cost(), 100);
    assert_eq!(changed["MapOnly"].get_build_cost(), 75);
    assert_eq!(original["Parent"].get_build_cost(), 50);
    assert!(
        !peer
            .shared_compatibility_templates()
            .contains_key("MapOnly")
    );
    for name in ["Parent", "Child", "Skin", "MapOnly"] {
        let detached = owner
            .compatibility_template_view()
            .find_template(name, false)
            .unwrap();
        assert_eq!(changed[name].get_build_cost(), detached.get_build_cost());
    }
    assert!(changed["Skin"].get_reskinned_from().is_some());
    assert!(changed["Skin"].is_equivalent_to(&changed["Parent"]));

    owner.reset_map_overrides();
    let reset = owner.shared_compatibility_templates();
    assert!(!Arc::ptr_eq(&changed, &reset));
    assert_eq!(reset["Parent"].get_build_cost(), 50);
    assert!(!reset.contains_key("MapOnly"));
    assert_eq!(changed["Parent"].get_build_cost(), 100);
    assert_eq!(
        peer.shared_compatibility_templates()["Parent"].get_build_cost(),
        50
    );
}

#[test]
#[ignore = "manual timing comparison; no performance threshold"]
fn compare_repeated_detached_views_with_reused_snapshot() {
    use std::time::Instant;
    let mut owner = catalog();
    let mut source = String::from("Object DefaultThingTemplate\nEnd\n");
    for index in 0..500 {
        source.push_str(&format!(
            "Object Example{index}\nBuildCost = {index}\nEnd\n"
        ));
    }
    owner
        .load_ini_text(&source, INILoadType::Overwrite)
        .unwrap();
    let started = Instant::now();
    for _ in 0..20 {
        std::hint::black_box(owner.compatibility_template_view());
    }
    let rebuild = started.elapsed();
    let started = Instant::now();
    for _ in 0..20 {
        std::hint::black_box(owner.shared_compatibility_templates());
    }
    let shared = started.elapsed();
    eprintln!(
        "20 views, 501 templates: rebuilt={rebuild:?}, shared including first construction={shared:?}"
    );
}
