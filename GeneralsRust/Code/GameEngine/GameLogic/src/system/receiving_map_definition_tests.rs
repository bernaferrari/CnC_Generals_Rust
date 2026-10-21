//! The full receiving UI parser consumes pre-admitted Object grammar locally.
use super::*;
use game_engine::common::{
    ascii_string::AsciiString,
    ini::ini_terrain::TerrainTypes,
    ini::{INILoadType, ini::armor_definitions::ArmorDefinitions},
    name_key_generator::NameKeyNamespace,
    thing::{module_factory::ModuleFactory, receiving_module_content::ReceivingModuleContent},
};

#[test]
fn admitted_nested_objects_reach_following_owned_terrain_without_global_factory() {
    let source = "Object SkipParent\nAddModule\nDraw = W3DModelDraw DrawTag\nDefaultConditionState\nModel = NONE\nEnd\nEnd\nEnd\nAddModule\nBehavior = AutoHealBehavior HealTag\nHealingAmount = 1\nEnd\nEnd\nEnd\nChildObject SkipChild SkipParent\nEnd\nObjectReskin SkipReskin SkipParent\nGeometry = BOX\nGeometryMajorRadius = 2\nEnd\nTerrain OwnedTerrain\nTexture = owned.tga\nEnd\n";
    let mut modules = ModuleFactory::new_receiving(NameKeyNamespace::new());
    crate::contain_module_overrides::register_native_module_overrides(&mut modules).unwrap();
    let mut content = ReceivingModuleContent::new(modules);
    assert_eq!(
        content
            .load_ini_text(source, INILoadType::CreateOverrides)
            .unwrap(),
        3
    );
    let peer = TerrainTypes::new();
    let (_, terrain, armor) = load_map_ini_ui_overrides_with_receiving_definitions(
        source,
        None,
        TerrainTypes::new(),
        ArmorDefinitions::default(),
        true,
    )
    .unwrap();
    assert_eq!(peer.get_terrain_count(), 0);
    assert_eq!(terrain.get_terrain_count(), 1);
    assert_eq!(
        terrain
            .find_terrain(&AsciiString::from("OwnedTerrain"))
            .unwrap()
            .texture_name
            .as_str(),
        "owned.tga"
    );
    let next = "Terrain OwnedTerrain\nTexture = replaced.tga\nEnd\n";
    content
        .load_ini_text(next, INILoadType::CreateOverrides)
        .unwrap();
    let (_, terrain, _) =
        load_map_ini_ui_overrides_with_receiving_definitions(next, None, terrain, armor, true)
            .unwrap();
    assert_eq!(terrain.get_terrain_count(), 1);
    assert_eq!(
        terrain
            .find_terrain(&AsciiString::from("OwnedTerrain"))
            .unwrap()
            .texture_name
            .as_str(),
        "replaced.tga"
    );
    let malformed = "Object NeverAdmitted\nAddModule\nDraw = W3DModelDraw Bad\n";
    assert!(
        content
            .load_ini_text(malformed, INILoadType::CreateOverrides)
            .is_err()
    );
    assert!(content.admitted_template("NeverAdmitted").is_none());
}
