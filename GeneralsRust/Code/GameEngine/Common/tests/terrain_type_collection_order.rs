use game_engine::common::rts::AsciiString;
use game_engine::common::terrain_types::{TerrainClass, TerrainTypeCollection};

#[test]
fn collection_uses_exact_names_newest_first_and_copies_default_fields() {
    let mut collection = TerrainTypeCollection::new();

    {
        let default = collection.new_terrain(AsciiString::from("DefaultTerrain"));
        default.set_class(TerrainClass::Rock);
        default.set_texture(AsciiString::from("rock-default.tga"));
        default.set_blend_edge(true);
        default.set_restrict_construction(true);
    }
    collection.new_terrain(AsciiString::from("Grass"));
    collection.new_terrain(AsciiString::from("grass"));
    {
        let duplicate = collection.new_terrain(AsciiString::from("Grass"));
        duplicate.set_texture(AsciiString::from("newest.tga"));
    }
    {
        let copied = collection.new_terrain(AsciiString::from("Copied"));
        assert_eq!(copied.get_class(), TerrainClass::Rock);
        assert_eq!(copied.get_texture().as_str(), "rock-default.tga");
        assert!(copied.is_blend_edge());
        assert!(copied.get_restrict_construction());
    }

    let names: Vec<_> = collection
        .iter()
        .map(|terrain| terrain.get_name().to_string())
        .collect();
    assert_eq!(
        names,
        ["Copied", "Grass", "grass", "Grass", "DefaultTerrain"]
    );
    assert_eq!(
        collection
            .find_terrain("Grass")
            .unwrap()
            .get_texture()
            .as_str(),
        "newest.tga"
    );
    assert_eq!(
        collection
            .find_terrain("grass")
            .unwrap()
            .get_name()
            .as_str(),
        "grass"
    );
    assert!(collection.find_terrain("GRASS").is_none());

    // C++ init and reset are both no-ops on the existing collection.
    collection.init();
    collection.reset();
    assert_eq!(collection.len(), 5);
    assert_eq!(
        collection
            .find_terrain("Grass")
            .unwrap()
            .get_texture()
            .as_str(),
        "newest.tga"
    );
}
