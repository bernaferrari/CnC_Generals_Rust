use game_engine::common::ascii_string::AsciiString;
use game_engine::common::system::archive_file_system::ArchiveFileSystem;
use game_engine::common::system::big_file_system::BIG_FILE_IDENTIFIER;
use game_engine::common::system::file_system::FilenameList;
use std::fs;
use std::io::{self, Read, Write};

fn create_test_big_file(path: &std::path::Path, entries: &[(&str, &[u8])]) -> io::Result<()> {
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
fn close_all_files_does_not_unmount_archives() -> Result<(), Box<dyn std::error::Error>> {
    let dir = tempfile::tempdir()?;
    let archive_path = dir.path().join("Data.big");
    create_test_big_file(
        &archive_path,
        &[("Data/INI/GameData.ini", b"GameDataPayload")],
    )?;

    let mut archive_system = ArchiveFileSystem::new();
    archive_system.open_archive_file(archive_path.to_str().expect("archive path"))?;
    assert!(archive_system.does_file_exist("data/ini/gamedata.ini"));

    archive_system.close_all_files();
    assert!(
        archive_system.does_file_exist("Data\\INI\\GameData.ini"),
        "C++ closeAllFiles closes opened subfiles but keeps mounted archives available"
    );

    let owner =
        archive_system.get_archive_filename_for_file(&AsciiString::from("Data/INI/GameData.ini"));
    assert!(
        owner.as_str().ends_with("Data.big"),
        "archive owner should survive close_all_files, got '{}'",
        owner.as_str()
    );

    let mut reader = archive_system.open_file("DATA/INI/GAMEDATA.INI", 0)?;
    let mut content = String::new();
    reader.read_to_string(&mut content)?;
    assert_eq!(content, "GameDataPayload");

    archive_system.close_all_archive_files();
    assert!(!archive_system.does_file_exist("Data/INI/GameData.ini"));

    Ok(())
}

#[test]
fn archive_directory_listing_recurses_like_cpp_even_when_flag_is_false(
) -> Result<(), Box<dyn std::error::Error>> {
    let dir = tempfile::tempdir()?;
    let archive_path = dir.path().join("Data.big");
    create_test_big_file(
        &archive_path,
        &[
            ("Data/INI/Object/AmericaVehicle.ini", b"vehicle"),
            ("Data/INI/Object/Nested/AmericaNested.ini", b"nested"),
            ("Data/INI/Upgrade.ini", b"upgrade"),
        ],
    )?;

    let mut archive_system = ArchiveFileSystem::new();
    archive_system.open_archive_file(archive_path.to_str().expect("archive path"))?;

    let mut filenames = FilenameList::new();
    archive_system.get_file_list_in_directory(
        &AsciiString::from(""),
        &AsciiString::from("data/ini/object"),
        &AsciiString::from("*.ini"),
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
            "data/ini/object/americavehicle.ini".to_string(),
            "data/ini/object/nested/americanested.ini".to_string(),
        ],
        "C++ ArchiveFile::getFileListInDirectory always descends archive subdirectories"
    );

    Ok(())
}
