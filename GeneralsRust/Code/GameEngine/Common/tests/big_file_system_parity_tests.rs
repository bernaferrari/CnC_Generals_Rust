use game_engine::common::ascii_string::AsciiString;
use game_engine::common::system::big_file_system::{BigFileSystem, BIG_FILE_IDENTIFIER};
use game_engine::common::system::file_system::FilenameList;
use std::fs;
use std::io::{self, Read, Write};

fn create_test_big_file(path: &str, entries: &[(&str, &[u8])]) -> Result<(), io::Error> {
    let mut file = fs::File::create(path)?;
    let directory_size: usize = entries
        .iter()
        .map(|(name, _)| 8usize + name.len() + 1)
        .sum();
    let first_file_offset = 0x10usize + directory_size;
    let archive_size =
        first_file_offset + entries.iter().map(|(_, data)| data.len()).sum::<usize>();

    file.write_all(BIG_FILE_IDENTIFIER)?;
    file.write_all(
        &u32::try_from(archive_size)
            .expect("test archive size should fit u32")
            .to_le_bytes(),
    )?;
    file.write_all(
        &u32::try_from(entries.len())
            .expect("test entry count should fit u32")
            .to_be_bytes(),
    )?;
    file.write_all(
        &u32::try_from(first_file_offset)
            .expect("test first file offset should fit u32")
            .to_be_bytes(),
    )?;

    let mut offset = first_file_offset;
    for (name, data) in entries {
        file.write_all(
            &u32::try_from(offset)
                .expect("test file offset should fit u32")
                .to_be_bytes(),
        )?;
        file.write_all(
            &u32::try_from(data.len())
                .expect("test file size should fit u32")
                .to_be_bytes(),
        )?;
        file.write_all(name.as_bytes())?;
        file.write_all(&[0])?;
        offset += data.len();
    }

    for (_, data) in entries {
        file.write_all(data)?;
    }

    Ok(())
}

#[test]
fn open_archive_file_uses_later_archive_as_loose_override() {
    let dir = tempfile::tempdir().unwrap();
    let base_path = dir.path().join("Base.big");
    let patch_path = dir.path().join("Patch.big");
    create_test_big_file(
        base_path.to_str().unwrap(),
        &[("Data/INI/Object.ini", b"base")],
    )
    .unwrap();
    create_test_big_file(
        patch_path.to_str().unwrap(),
        &[("data\\ini\\object.ini", b"patch")],
    )
    .unwrap();

    let mut system = BigFileSystem::new();
    system.open_archive_file(&base_path).unwrap();
    system.open_archive_file(&patch_path).unwrap();

    assert!(system.does_file_exist("DATA/INI/OBJECT.INI"));

    let mut reader = system.open_file("Data/INI/Object.ini", 0).unwrap();
    let mut content = String::new();
    reader.read_to_string(&mut content).unwrap();
    assert_eq!(content, "patch");

    let owner = system
        .resolve_archive_filename(&AsciiString::from("data/ini/object.ini"))
        .expect("owner archive");
    assert!(owner.as_str().ends_with("Patch.big"));
}

#[test]
fn directory_listing_normalizes_case_and_slashes() {
    let dir = tempfile::tempdir().unwrap();
    let archive_path = dir.path().join("Assets.big");
    create_test_big_file(
        archive_path.to_str().unwrap(),
        &[
            ("Data\\INI\\Object\\AmericaVehicle.ini", b"vehicle"),
            ("Data/INI/Object/AmericaInfantry.ini", b"infantry"),
            ("Data/INI/Object/Nested/AmericaNested.ini", b"nested"),
            ("Data/INI/Upgrade.ini", b"upgrade"),
        ],
    )
    .unwrap();

    let mut system = BigFileSystem::new();
    system.open_archive_file(&archive_path).unwrap();

    let mut non_recursive = FilenameList::new();
    system.collect_matching_files(
        &AsciiString::from("data/ini/object"),
        &AsciiString::from("*.INI"),
        &mut non_recursive,
        false,
    );
    let non_recursive_files = non_recursive
        .iter()
        .map(|name| name.as_str().replace('\\', "/").to_lowercase())
        .collect::<Vec<_>>();

    assert_eq!(
        non_recursive_files,
        vec![
            "data/ini/object/americainfantry.ini".to_string(),
            "data/ini/object/americavehicle.ini".to_string(),
            "data/ini/object/nested/americanested.ini".to_string()
        ]
    );

    let mut recursive = FilenameList::new();
    system.collect_matching_files(
        &AsciiString::from("data/ini"),
        &AsciiString::from("*.ini"),
        &mut recursive,
        true,
    );

    assert_eq!(recursive.len(), 4);
    assert!(recursive.contains(&AsciiString::from("Data/INI/Upgrade.ini")));
}
