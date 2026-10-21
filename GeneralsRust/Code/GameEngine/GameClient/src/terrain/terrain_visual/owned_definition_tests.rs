use super::*;

fn source(texture: &Path) -> String {
    format!(
        "Terrain SharedOwner\n Class = ROCK\n Texture = {}\n End\n",
        texture.display()
    )
}
fn catalog(text: &str) -> Arc<ini_terrain::TerrainTypes> {
    Arc::new(
        ini_terrain::read_terrain_definition_sources(&[("owned/Terrain.ini", text.as_bytes())])
            .unwrap(),
    )
}
fn texture(path: &Path, color: [u8; 3]) {
    image::RgbImage::from_pixel(4, 4, image::Rgb(color))
        .save(path)
        .unwrap();
}
fn selected(visual: &TerrainVisualImpl) -> String {
    let texture = TerrainTexture::new(0, "SharedOwner".into(), "SharedOwner.tga".into());
    visual
        .find_terrain_type_for_texture(&texture)
        .unwrap()
        .texture_name
        .as_str()
        .to_string()
}

#[test]
fn actual_visuals_keep_same_named_definition_and_source_tile_resolution_owned() {
    let directory = tempfile::tempdir().unwrap();
    let a = directory.path().join("a.tga");
    let b = directory.path().join("b.tga");
    texture(&a, [20, 40, 60]);
    texture(&b, [80, 100, 120]);
    let mut receiving = TerrainVisualImpl::new();
    let mut peer = TerrainVisualImpl::new();
    receiving.admit_terrain_definitions(catalog(&source(&a)));
    peer.admit_terrain_definitions(catalog(&source(&b)));
    assert_eq!(selected(&receiving), a.to_string_lossy());
    assert_eq!(selected(&peer), b.to_string_lossy());
    assert_eq!(
        receiving.resolve_source_tile_texture_path("SharedOwner"),
        Some(a.clone())
    );
    assert_eq!(
        peer.resolve_source_tile_texture_path("SharedOwner"),
        Some(b.clone())
    );
    receiving.ensure_default_textures();
    peer.ensure_default_textures();
    let first = receiving
        .texture_system
        .get_texture(receiving.texture_rules[0].texture_id)
        .unwrap();
    let other = peer
        .texture_system
        .get_texture(peer.texture_rules[0].texture_id)
        .unwrap();
    assert_eq!(first.diffuse_path, a.to_string_lossy());
    assert_eq!(other.diffuse_path, b.to_string_lossy());
    SubsystemInterface::reset(&mut peer).unwrap();
    drop(peer);
    assert_eq!(selected(&receiving), a.to_string_lossy());
}

#[test]
fn failed_standalone_definition_candidate_preserves_prior_catalog() {
    let directory = tempfile::tempdir().unwrap();
    std::fs::write(
        directory.path().join("Terrain.ini"),
        "Terrain BadCandidate\n Class = NOT_A_TERRAIN_CLASS\nEnd\n",
    )
    .unwrap();
    let good = directory.path().join("good");
    std::fs::create_dir(&good).unwrap();
    std::fs::write(
        good.join("Terrain.ini"),
        "Terrain SharedOwner\n Texture = previous.tga\nEnd\n",
    )
    .unwrap();
    let mut visual = TerrainVisualImpl::new();
    visual
        .ensure_terrain_definitions(Some(&good.join("map.map")))
        .unwrap();
    let previous = match &visual.terrain_definitions {
        TerrainDefinitionOwner::Standalone(catalog) => Arc::clone(catalog),
        TerrainDefinitionOwner::Admitted(_) => panic!("compatibility load changed owner mode"),
    };
    let prior_sources = visual.loaded_terrain_sources.clone();
    assert!(
        visual
            .ensure_terrain_definitions(Some(&directory.path().join("map.map")))
            .is_err()
    );
    assert_eq!(selected(&visual), "previous.tga");
    assert_eq!(visual.loaded_terrain_sources, prior_sources);
    match &visual.terrain_definitions {
        TerrainDefinitionOwner::Standalone(catalog) => assert!(Arc::ptr_eq(catalog, &previous)),
        TerrainDefinitionOwner::Admitted(_) => {
            panic!("failed compatibility parse changed owner mode")
        }
    }
}

#[test]
fn explicitly_empty_catalog_skips_invalid_map_directory_definition_io() {
    let directory = tempfile::tempdir().unwrap();
    std::fs::write(
        directory.path().join("Terrain.ini"),
        "Terrain Broken\n UnknownField = invalid\nEnd\n",
    )
    .unwrap();
    let mut visual = TerrainVisualImpl::new();
    visual.admit_terrain_definitions(Arc::new(ini_terrain::TerrainTypes::new()));
    visual
        .ensure_terrain_definitions(Some(&directory.path().join("map.map")))
        .unwrap();
    assert_eq!(visual.terrain_definitions.catalog().get_terrain_count(), 0);
    assert!(visual.loaded_terrain_sources.is_empty());
}

#[test]
fn owned_archive_bytes_and_same_basename_files_share_exact_override_kernel() {
    let directory = tempfile::tempdir().unwrap();
    let a = directory.path().join("default");
    let b = directory.path().join("override");
    std::fs::create_dir(&a).unwrap();
    std::fs::create_dir(&b).unwrap();
    let default = "Terrain DefaultTerrain\n Class = ROCK\n Texture = inherited.tga\nEnd\nTerrain SharedOwner\n BlendEdges = Yes\nEnd\n";
    let override_text = "Terrain SharedOwner\n Texture = override.tga\nEnd\n";
    let paths = [a.join("Terrain.ini"), b.join("Terrain.ini")];
    std::fs::write(&paths[0], default).unwrap();
    std::fs::write(&paths[1], override_text).unwrap();
    let disk = ini_terrain::read_terrain_definition_files(&paths).unwrap();
    let archive = ini_terrain::read_terrain_definition_sources(&[
        ("Data/INI/Default/Terrain.ini", default.as_bytes()),
        ("Data/INI/Terrain.ini", override_text.as_bytes()),
    ])
    .unwrap();
    assert_eq!(disk, archive);
    let owned = archive
        .find_terrain(&AsciiString::from("SharedOwner"))
        .unwrap();
    assert_eq!(owned.texture_name.as_str(), "override.tga");
    assert!(owned.blend_edge_texture);
    assert_eq!(owned.surface_type, TerrainSurface::Rock);
    let mut first = TerrainVisualImpl::new();
    let mut second = TerrainVisualImpl::new();
    first.admit_terrain_definitions(Arc::new(archive));
    second.admit_terrain_definitions(catalog("Terrain SharedOwner\n Texture = peer.tga\nEnd\n"));
    assert_eq!(selected(&first), "override.tga");
    assert_eq!(selected(&second), "peer.tga");
}
