use game_engine::common::system::file::{File, FileAccess};
use game_engine::common::system::local_file::LocalFile;

#[test]
fn write_only_open_applies_cpp_create_truncate_defaults() -> Result<(), Box<dyn std::error::Error>>
{
    let dir = tempfile::tempdir()?;
    let path = dir.path().join("write_only_default.txt");

    let mut file = LocalFile::new();
    file.open(path.to_str().expect("test path"), FileAccess::WRITE)?;
    assert!(file.get_access().contains(FileAccess::WRITE));
    assert!(file.get_access().contains(FileAccess::TRUNCATE));
    assert!(file.get_access().contains(FileAccess::BINARY));
    file.write(b"first")?;
    file.close();

    let mut file = LocalFile::new();
    file.open(path.to_str().expect("test path"), FileAccess::WRITE)?;
    file.write(b"x")?;
    file.close();

    let bytes = std::fs::read(&path)?;
    assert_eq!(bytes, b"x");

    Ok(())
}

#[test]
fn only_new_create_fails_when_file_exists() -> Result<(), Box<dyn std::error::Error>> {
    let dir = tempfile::tempdir()?;
    let path = dir.path().join("only_new.txt");
    std::fs::write(&path, b"existing")?;

    let mut file = LocalFile::new();
    let result = file.open(
        path.to_str().expect("test path"),
        FileAccess::WRITE
            .combine(FileAccess::CREATE)
            .combine(FileAccess::ONLY_NEW),
    );

    assert!(
        result.is_err(),
        "ONLY_NEW should map to create_new and reject an existing file"
    );
    assert_eq!(std::fs::read(&path)?, b"existing");

    Ok(())
}

#[test]
fn append_open_positions_at_end_without_truncating() -> Result<(), Box<dyn std::error::Error>> {
    let dir = tempfile::tempdir()?;
    let path = dir.path().join("append.txt");
    std::fs::write(&path, b"base")?;

    let mut file = LocalFile::new();
    file.open(
        path.to_str().expect("test path"),
        FileAccess::WRITE.combine(FileAccess::APPEND),
    )?;
    assert_eq!(file.position(), 4);
    file.write(b"+tail")?;
    file.close();

    assert_eq!(std::fs::read(&path)?, b"base+tail");

    Ok(())
}
