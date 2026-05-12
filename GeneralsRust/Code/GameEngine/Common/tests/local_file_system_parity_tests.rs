use game_engine::common::ascii_string::AsciiString;
use game_engine::common::system::file_system::{FileSystemBackend, FilenameList};
use game_engine::common::system::local_file_system::LocalFileSystem;
use std::fs;
use std::path::PathBuf;

#[test]
fn local_directory_listing_matches_masks_case_insensitively(
) -> Result<(), Box<dyn std::error::Error>> {
    let test_root = tempfile::tempdir()?;
    let actual_dir = test_root.path().join("Data").join("INI").join("Object");
    fs::create_dir_all(&actual_dir)?;
    fs::write(actual_dir.join("AmericaVehicle.ini"), b"vehicle")?;
    fs::write(actual_dir.join("AmericaInfantry.INI"), b"infantry")?;
    fs::write(actual_dir.join("Readme.txt"), b"readme")?;

    let mut fs_backend = LocalFileSystem::new();
    fs_backend.add_search_path(test_root.path());

    let mut filenames = FilenameList::new();
    fs_backend.get_file_list_in_directory(
        &AsciiString::from(""),
        &AsciiString::from("data/ini/object"),
        &AsciiString::from("*.INI"),
        &mut filenames,
        false,
    );

    let listed = filenames
        .iter()
        .map(|name| name.as_str().replace('\\', "/").to_lowercase())
        .collect::<Vec<_>>();

    assert_eq!(
        listed,
        vec![
            "data/ini/object/americainfantry.ini".to_string(),
            "data/ini/object/americavehicle.ini".to_string(),
        ]
    );

    Ok(())
}

#[test]
fn local_directory_listing_supports_question_wildcard() -> Result<(), Box<dyn std::error::Error>> {
    let test_root = tempfile::tempdir()?;
    let actual_dir = test_root.path().join("Data").join("INI");
    fs::create_dir_all(&actual_dir)?;
    fs::write(actual_dir.join("MapA.ini"), b"a")?;
    fs::write(actual_dir.join("MapB.INI"), b"b")?;
    fs::write(actual_dir.join("MapLong.ini"), b"long")?;

    let mut fs_backend = LocalFileSystem::new();
    fs_backend.add_search_path(PathBuf::from(test_root.path()));

    let mut filenames = FilenameList::new();
    fs_backend.get_file_list_in_directory(
        &AsciiString::from(""),
        &AsciiString::from("data/ini"),
        &AsciiString::from("Map?.INI"),
        &mut filenames,
        false,
    );

    let listed = filenames
        .iter()
        .map(|name| name.as_str().replace('\\', "/").to_lowercase())
        .collect::<Vec<_>>();

    assert_eq!(
        listed,
        vec![
            "data/ini/mapa.ini".to_string(),
            "data/ini/mapb.ini".to_string()
        ]
    );

    Ok(())
}
