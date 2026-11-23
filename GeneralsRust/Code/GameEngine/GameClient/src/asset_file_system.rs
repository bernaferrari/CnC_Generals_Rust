//! Virtual asset reads through the application's local and BIG archive backends.

use game_engine::common::system::file_system::get_file_system;

/// Read the selected virtual file, including loose overrides over mounted archives.
pub(crate) fn read_game_fs_bytes(virtual_path: &str) -> Option<Vec<u8>> {
    ensure_game_fs_backends();
    let file_system = get_file_system();
    let mut fs = file_system.lock().ok()?;
    let mut file = fs.open_file(
        virtual_path,
        game_engine::common::system::file::FileAccess::READ
            .combine(game_engine::common::system::file::FileAccess::BINARY),
    )?;
    file.read_entire_and_close().ok()
}

/// Ensure the engine file system carries the local + BIG backends with the
/// standard runtime search roots before the first virtual asset read.
pub(crate) fn add_game_fs_search_roots(roots: &[std::path::PathBuf], reload_archives: bool) {
    use game_engine::common::system::big_file_system::BigArchiveBackend;
    use game_engine::common::system::file_system::FileSystemBackend;
    use game_engine::common::system::local_file_system::LocalFileSystem;
    use game_engine::common::system::subsystem_interface::SubsystemInterface as CommonSubsystemInterface;

    let file_system = get_file_system();
    let Ok(mut guard) = file_system.lock() else {
        return;
    };

    {
        let local_backend: &mut LocalFileSystem = guard.ensure_backend(LocalFileSystem::new);
        for path in roots {
            local_backend.add_search_path(path);
        }
    }
    {
        let big_backend: &mut BigArchiveBackend = guard.ensure_backend(BigArchiveBackend::new);
        for path in roots {
            big_backend.add_search_path(path);
        }
        if reload_archives {
            // Close and re-scan so archives in newly added roots are picked
            // up even after the backend already initialized once.
            big_backend.reset();
        }
    }

    guard.clear_cache();
    let _ = CommonSubsystemInterface::init(&mut *guard);
}

fn ensure_game_fs_backends() {
    enum GameFsRoots {}
    game_engine::common::common_services::once_per_application::<GameFsRoots>(|| {
        let mut roots = vec![std::path::PathBuf::from(".")];
        if let Ok(cwd) = std::env::current_dir() {
            roots.push(cwd.clone());
            // Worktree retail + extracted BIG siblings (INIZH.big lives under Data/).
            roots.push(cwd.join("windows_game/Command & Conquer Generals Zero Hour"));
            roots.push(cwd.join("windows_game/Command & Conquer Generals Zero Hour/Data"));
            roots.push(cwd.join("../windows_game/Command & Conquer Generals Zero Hour"));
            roots.push(cwd.join("../windows_game/Command & Conquer Generals Zero Hour/Data"));
        }
        if let Ok(exe) = std::env::current_exe() {
            if let Some(parent) = exe.parent() {
                roots.push(parent.to_path_buf());
            }
        }
        if let Ok(assets) = std::env::var("GENERALS_ASSETS_DIR") {
            let assets = std::path::PathBuf::from(assets);
            roots.push(assets.clone());
            roots.push(assets.join("Data"));
            // Sibling extracted tree next to the retail install.
            if let Some(parent) = assets.parent() {
                roots.push(parent.join("extracted_big_files_v2"));
            }
        }
        add_game_fs_search_roots(&roots, true);
    });
}
