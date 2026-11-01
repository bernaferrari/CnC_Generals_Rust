#[path = "../src/executable_smoke/retail_map.rs"]
mod retail_map;

#[test]
fn configured_installation_and_data_directory_resolve_only_real_maps() {
    let root = std::env::temp_dir().join(format!("generals_map_resolution_{}", std::process::id()));
    std::fs::create_dir(&root).unwrap();
    let installation = root.join("Command & Conquer Generals Zero Hour");
    let data = installation.join("Data");
    std::fs::create_dir_all(&data).unwrap();
    assert_eq!(retail_map::lone_eagle_from_assets_root(&installation), None);
    let extracted = root.join("extracted_big_files/MapsZH/Maps/Lone Eagle/Lone Eagle.map");
    std::fs::create_dir_all(extracted.parent().unwrap()).unwrap();
    std::fs::write(&extracted, b"map").unwrap();
    let expected = extracted.canonicalize().unwrap();
    assert_eq!(
        retail_map::lone_eagle_from_assets_root(&installation),
        Some(expected.clone())
    );
    assert_eq!(
        retail_map::lone_eagle_from_assets_root(&data),
        Some(expected)
    );
    let local = installation.join("Maps/Lone Eagle/Lone Eagle.map");
    std::fs::create_dir_all(local.parent().unwrap()).unwrap();
    std::fs::write(&local, b"local").unwrap();
    assert_eq!(
        retail_map::lone_eagle_from_assets_root(&installation),
        Some(local.canonicalize().unwrap())
    );
    std::fs::remove_dir_all(root).unwrap();
}
