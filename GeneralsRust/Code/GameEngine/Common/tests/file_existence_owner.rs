use std::sync::Arc;

use game_engine::common::system::file_system::FileSystem;
use game_engine::common::system::local_file_system::LocalFileSystem;

#[test]
fn existence_cache_uses_file_identity_across_threads_and_owners() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("exists.wav"), b"present").unwrap();
    let mut local = LocalFileSystem::new();
    local.add_search_path(dir.path());
    let mut owner = FileSystem::new();
    owner.register_backend(Box::new(local));
    let owner = Arc::new(owner);

    let first = Arc::clone(&owner);
    assert!(
        std::thread::spawn(move || first.does_file_exist("exists.wav"))
            .join()
            .unwrap()
    );
    let second = Arc::clone(&owner);
    assert!(
        !std::thread::spawn(move || second.does_file_exist("missing.wav"))
            .join()
            .unwrap()
    );
    let third = Arc::clone(&owner);
    assert!(
        std::thread::spawn(move || third.does_file_exist("EXISTS.WAV"))
            .join()
            .unwrap()
    );

    let peer = FileSystem::new();
    assert!(!peer.does_file_exist("exists.wav"));

    let mut local = LocalFileSystem::new();
    local.add_search_path(dir.path());
    let mut reversed = FileSystem::new();
    reversed.register_backend(Box::new(local));
    let reversed = Arc::new(reversed);
    let first = Arc::clone(&reversed);
    assert!(
        !std::thread::spawn(move || first.does_file_exist("missing.wav"))
            .join()
            .unwrap()
    );
    let second = Arc::clone(&reversed);
    assert!(
        std::thread::spawn(move || second.does_file_exist("exists.wav"))
            .join()
            .unwrap()
    );
}
