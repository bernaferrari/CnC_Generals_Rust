use super::*;
use crate::common::ini::INI;
use crate::common::name_key_generator::NameKeyNamespace;
use crate::common::thing::module::{BaseModuleData, ModuleData, ModuleInterfaceType, ModuleType};

fn content() -> ReceivingModuleContent {
    fn data(_: Option<&mut INI>) -> Box<dyn ModuleData> {
        Box::new(BaseModuleData::new())
    }
    fn parse(ini: Option<&mut INI>) -> Result<Box<dyn ModuleData>, String> {
        Ok(data(ini))
    }
    let mut modules = ModuleFactory::new_receiving(NameKeyNamespace::new());
    modules.add_module_internal(
        None,
        Some(data),
        ModuleType::Draw,
        "ReceivingDraw",
        ModuleInterfaceType::DRAW,
    );
    modules.register_fallible_data_constructor("ReceivingDraw", ModuleType::Draw, parse);
    ReceivingModuleContent::new(modules)
}
fn first_data(plan: &AdmittedReceivingTemplate) -> &Arc<dyn ModuleData> {
    plan.template()
        .get_draw_module_info()
        .get_nth_data(0)
        .unwrap()
}

#[test]
fn ordered_receiving_producer_shares_default_parent_data_and_resets_only_map_versions() {
    let mut owner = content();
    assert_eq!(owner.load_ini_text("Object DefaultThingTemplate\nScale = 2.0\nDraw = ReceivingDraw DefaultTag\nEnd\nEnd\nObject Parent\nKindOf = VEHICLE\nEnd\nChildObject Child Parent\nEnd", INILoadType::Overwrite).unwrap(), 3);
    let default = owner.admitted_template("DefaultThingTemplate").unwrap();
    let parent = owner.admitted_template("Parent").unwrap();
    let child = owner.admitted_template("Child").unwrap();
    assert!(Arc::ptr_eq(first_data(&default), first_data(&parent)));
    assert!(Arc::ptr_eq(first_data(&parent), first_data(&child)));
    assert_eq!(
        owner.modules.retained_module_data_count(),
        1,
        "inherited source must not be parsed again"
    );
    let view = owner.compatibility_template_view();
    let viewed = view.find_template("Parent", false).unwrap();
    assert_eq!(viewed.get_asset_scale(), 2.0);
    assert!(Arc::ptr_eq(
        first_data(&parent),
        viewed.get_draw_module_info().get_nth_data(0).unwrap()
    ));
    assert_eq!(
        owner.modules.retained_module_data_count(),
        1,
        "compatibility view must not reparse module data"
    );
    let peer = owner.clone();
    owner.load_ini_text("Object Parent\nAddModule\nDraw = ReceivingDraw MapTag\nEnd\nEnd\nEnd\nObject MapOnly\nKindOf = VEHICLE\nEnd", INILoadType::CreateOverrides).unwrap();
    let overridden = owner.admitted_template("Parent").unwrap();
    assert!(!Arc::ptr_eq(&parent, &overridden));
    assert_eq!(overridden.template().get_draw_module_info().get_count(), 2);
    assert_eq!(parent.template().get_draw_module_info().get_count(), 1);
    assert!(Arc::ptr_eq(
        &peer.admitted_template("Parent").unwrap(),
        &parent
    ));
    assert!(owner.admitted_template("MapOnly").is_some());
    assert_eq!(owner.modules.retained_module_data_count(), 2);
    owner.reset_map_overrides();
    assert!(Arc::ptr_eq(
        &owner.admitted_template("Parent").unwrap(),
        &parent
    ));
    assert!(owner.admitted_template("MapOnly").is_none());
    assert_eq!(
        owner.modules.retained_module_data_count(),
        2,
        "ModuleFactory reset is an original no-op"
    );
    assert_eq!(peer.modules.retained_module_data_count(), 1);
    assert_eq!(
        owner
            .admitted_templates()
            .map(|(name, _)| name)
            .collect::<Vec<_>>(),
        ["Child", "Parent", "DefaultThingTemplate"]
    );
}

#[test]
fn rejected_receiving_file_preserves_live_plans_namespace_and_module_xfer_list() {
    let mut owner = content();
    owner
        .load_ini_text(
            "Object Resident\nDraw = ReceivingDraw ResidentTag\nEnd\nEnd",
            INILoadType::Overwrite,
        )
        .unwrap();
    let resident = owner.admitted_template("Resident").unwrap();
    let mut expected = owner.modules.receiving_name_keys_mut().unwrap().clone();
    let before = owner.modules.retained_module_data_count();
    let error = owner.load_ini_text("Object Prefix\nDraw = ReceivingDraw PrefixTag\nEnd\nEnd\nObject Broken\nUnknownField = Yes\nEnd", INILoadType::Overwrite).unwrap_err();
    assert!(error.contains("Unknown Object field"), "{error}");
    assert!(owner.admitted_template("Prefix").is_none());
    assert!(owner.admitted_template("Broken").is_none());
    assert!(Arc::ptr_eq(
        &owner.admitted_template("Resident").unwrap(),
        &resident
    ));
    assert_eq!(owner.modules.retained_module_data_count(), before);
    assert_eq!(
        owner
            .modules
            .receiving_name_keys_mut()
            .unwrap()
            .name_to_key("AfterRejected"),
        expected.name_to_key("AfterRejected")
    );
    assert!(
        owner
            .load_ini_text(
                "Object Unterminated\nDraw = ReceivingDraw MissingEnd\nEnd",
                INILoadType::Overwrite
            )
            .is_err()
    );
    assert!(owner.admitted_template("Unterminated").is_none());
}

#[test]
fn receiving_scalar_link_key_and_module_tag_share_source_order_without_peer_namespace() {
    let source = "Object ScalarA\nMaxSimultaneousLinkKey = OwnedGroup\nDraw = ReceivingDraw OwnedTag\nEnd\nEnd\nObject ScalarB\nMaxSimultaneousLinkKey = OwnedGroup\nEnd";
    let mut first = content();
    first
        .module_factory_mut()
        .receiving_name_keys_mut()
        .unwrap()
        .name_to_key("FirstPrefix");
    let mut peer = content();
    peer.module_factory_mut()
        .receiving_name_keys_mut()
        .unwrap()
        .name_to_key("PeerPrefixA");
    peer.module_factory_mut()
        .receiving_name_keys_mut()
        .unwrap()
        .name_to_key("PeerPrefixB");
    first.load_ini_text(source, INILoadType::Overwrite).unwrap();
    peer.load_ini_text(source, INILoadType::Overwrite).unwrap();
    let plan = first.admitted_template("ScalarA").unwrap();
    let group = plan.template().get_max_simultaneous_link_key();
    let tag = first_data(&plan).get_module_tag_name_key();
    assert!(
        group < tag,
        "scalar key precedes the subsequent module declaration/tag"
    );
    assert_eq!(
        first
            .module_factory_mut()
            .receiving_name_keys_mut()
            .unwrap()
            .key_to_name(group)
            .as_deref(),
        Some("OwnedGroup")
    );
    assert_eq!(
        first
            .module_factory_mut()
            .receiving_name_keys_mut()
            .unwrap()
            .key_to_name(tag)
            .as_deref(),
        Some("OwnedTag")
    );
    assert_eq!(
        first
            .admitted_template("ScalarB")
            .unwrap()
            .template()
            .get_max_simultaneous_link_key(),
        group
    );
    let peer_group = peer
        .admitted_template("ScalarA")
        .unwrap()
        .template()
        .get_max_simultaneous_link_key();
    assert_ne!(group, peer_group);
    assert_eq!(
        peer.module_factory_mut()
            .receiving_name_keys_mut()
            .unwrap()
            .key_to_name(peer_group)
            .as_deref(),
        Some("OwnedGroup")
    );
    let before = first
        .module_factory_mut()
        .receiving_name_keys_mut()
        .unwrap()
        .clone();
    let rejected = first.load_ini_text("Object Prefix\nMaxSimultaneousLinkKey = RejectedGroup\nEnd\nObject Bad\nUnknownField = Yes\nEnd",INILoadType::Overwrite);
    assert!(rejected.is_err());
    let next_expected = {
        let mut keys = before;
        keys.name_to_key("NextAfterRejected")
    };
    assert_eq!(
        first
            .module_factory_mut()
            .receiving_name_keys_mut()
            .unwrap()
            .name_to_key("NextAfterRejected"),
        next_expected
    );
    let mut legacy = ThingTemplate::new();
    let fields = [("MaxSimultaneousLinkKey".into(), "CompatibilityGroup".into())]
        .into_iter()
        .collect();
    legacy.parse_object_fields_from_ini(&fields).unwrap();
    assert_eq!(
        legacy.get_max_simultaneous_link_key(),
        crate::common::name_key_generator::NameKeyGenerator::name_to_key("CompatibilityGroup"),
        "explicit standalone compatibility retains its original namespace route"
    );
}

#[test]
fn receiving_production_facts_resolve_boot_forward_names_but_reject_late_override_names_and_unknown_science()
 {
    use crate::common::rts::science::{ScienceInfo, ScienceNameFacts, ScienceStore};
    let mut science_store = ScienceStore::new();
    science_store.add_science(ScienceInfo::new(701, "SCIENCE_Receiving"));
    let sciences = Arc::new(ScienceNameFacts::from_store(&science_store));
    let factory = ModuleFactory::new_receiving_with_sciences(NameKeyNamespace::new(), sciences);
    let mut owner = ReceivingModuleContent::new(factory);
    owner.load_ini_text("Object Candidate\nPrerequisites\nObject = Later\nScience = SCIENCE_Receiving\nEnd\nEnd",INILoadType::Overwrite).unwrap();
    assert!(
        owner.validate_production_prerequisites().is_err(),
        "boot is not complete yet"
    );
    owner
        .load_ini_text("Object Later\nEnd", INILoadType::Overwrite)
        .unwrap();
    owner.validate_production_prerequisites().unwrap();
    let before = owner.admitted_template("Candidate").unwrap();
    assert_eq!(
        before.production_prerequisites()[1].get_science_prereqs(),
        &[701]
    );
    let error = owner
        .load_ini_text(
            "Object Candidate\nPrerequisites\nObject = FutureMap\nEnd\nEnd\nObject FutureMap\nEnd",
            INILoadType::CreateOverrides,
        )
        .unwrap_err();
    assert!(error.contains("FutureMap"), "{error}");
    assert!(Arc::ptr_eq(
        &before,
        &owner.admitted_template("Candidate").unwrap()
    ));
    assert!(owner.admitted_template("FutureMap").is_none());
    let error = owner
        .load_ini_text(
            "Object Candidate\nPrerequisites\nScience = SCIENCE_Unknown\nEnd\nEnd",
            INILoadType::CreateOverrides,
        )
        .unwrap_err();
    assert!(
        error.contains("Unknown receiving science prerequisite"),
        "{error}"
    );
    assert!(Arc::ptr_eq(
        &before,
        &owner.admitted_template("Candidate").unwrap()
    ));
    let mut empty =
        ReceivingModuleContent::new(ModuleFactory::new_receiving(NameKeyNamespace::new()));
    assert!(
        empty
            .load_ini_text(
                "Object Unknown\nPrerequisites\nScience = SCIENCE_Receiving\nEnd\nEnd",
                INILoadType::Overwrite
            )
            .is_err(),
        "no application facts must not silently erase a science requirement"
    );
}

#[test]
fn receiving_editor_sorting_uses_original_first_token_and_rejects_unknown_first_value() {
    use crate::common::thing::thing_template::EditorSortingType;
    let mut owner = content();
    owner
        .load_ini_text(
            "Object RetailProp\nEditorSorting = MISC_MAN_MADE CLEARED_BY_BUILD\nEnd\n",
            INILoadType::Overwrite,
        )
        .expect("genuine CivilianProp first enum plus trailing token");
    let first = owner.admitted_template("RetailProp").unwrap();
    assert_eq!(
        first.template().get_editor_sorting(),
        EditorSortingType::MiscManMade
    );
    for value in [
        "MISC_NATURAL = CLEARED_BY_BUILD",
        "misc_natural\tCLEARED_BY_BUILD",
        "=MISC_NATURAL",
    ] {
        let source = format!("Object RetailProp\nEditorSorting = {value}\nEnd\n");
        owner
            .load_ini_text(&source, INILoadType::CreateOverrides)
            .expect("original standard separators and case-insensitive first token");
        assert_eq!(
            owner
                .admitted_template("RetailProp")
                .unwrap()
                .template()
                .get_editor_sorting(),
            EditorSortingType::MiscNatural
        );
    }
    let retained = owner.admitted_template("RetailProp").unwrap();
    for value in ["UNKNOWN_EDITOR_VALUE MISC_MAN_MADE", "=", ""] {
        let source = format!("Object RetailProp\nEditorSorting = {value}\nEnd\n");
        assert!(
            owner
                .load_ini_text(&source, INILoadType::CreateOverrides)
                .is_err(),
            "unknown or absent first enum must reject: {value}"
        );
        assert!(
            Arc::ptr_eq(&retained, &owner.admitted_template("RetailProp").unwrap()),
            "failed scalar admission leaves accepted owner unchanged"
        );
    }
}

#[test]
fn receiving_catalog_versions_are_frozen_while_detached_views_keep_real_override_slots() {
    let mut owner = content();
    owner
        .load_ini_text(
            "Object DefaultThingTemplate\nScale = 2.0\nDraw = ReceivingDraw DefaultTag\nEnd\nEnd\n\
         Object Parent\nBuildCost = 700\nEnd\n\
         ChildObject Child Parent\nEnd\n\
         ObjectReskin Skin Parent\nFenceWidth = 5.0\nEnd\n",
            INILoadType::Overwrite,
        )
        .expect("actual owned default/child/reskin producer");
    let parent = owner.admitted_template("Parent").unwrap();
    let peer = owner.clone();
    for name in ["DefaultThingTemplate", "Parent", "Child", "Skin"] {
        let plan = owner.admitted_template(name).unwrap();
        assert!(plan.template().has_frozen_override_link(), "{name}");
        assert!(plan.template().get_next_override().is_none(), "{name}");
        assert!(
            Arc::ptr_eq(first_data(&plan), first_data(&parent)),
            "{name}"
        );
    }
    assert_eq!(
        owner.admitted_template("Skin").unwrap().reskin_name(),
        Some("Parent")
    );
    let mut view = owner.compatibility_template_view();
    for name in ["DefaultThingTemplate", "Parent", "Child", "Skin"] {
        let template = view.find_template(name, false).unwrap();
        assert!(!template.has_frozen_override_link(), "view {name}");
        assert!(Arc::ptr_eq(
            first_data(&parent),
            template.get_draw_module_info().get_nth_data(0).unwrap()
        ));
    }
    let view_parent = view.find_template("Parent", false).unwrap();
    let view_override = view.new_override(Arc::clone(&view_parent)).unwrap();
    assert!(Arc::ptr_eq(
        &view_parent.get_next_override().unwrap(),
        &view_override
    ));
    assert!(!view_override.has_frozen_override_link());
    assert!(parent.template().get_next_override().is_none());
    assert!(
        peer.admitted_template("Parent")
            .unwrap()
            .template()
            .has_frozen_override_link()
    );
    owner
        .load_ini_text(
            "Object Parent\nBuildCost = 900\nEnd\nObject MapOnly\nEnd\n",
            INILoadType::CreateOverrides,
        )
        .unwrap();
    assert!(
        owner
            .admitted_template("Parent")
            .unwrap()
            .template()
            .has_frozen_override_link()
    );
    assert!(
        owner
            .admitted_template("MapOnly")
            .unwrap()
            .template()
            .has_frozen_override_link()
    );
    assert_eq!(
        owner
            .admitted_template("Parent")
            .unwrap()
            .template()
            .get_build_cost(),
        900
    );
    assert_eq!(parent.template().get_build_cost(), 700);
    owner.reset_map_overrides();
    assert!(Arc::ptr_eq(
        &owner.admitted_template("Parent").unwrap(),
        &parent
    ));
    assert!(owner.admitted_template("MapOnly").is_none());
    assert!(Arc::ptr_eq(
        &peer.admitted_template("Parent").unwrap(),
        &parent
    ));
    // Detached-view reskin parent rebinding remains the separate hq-uwrah contract.
}
