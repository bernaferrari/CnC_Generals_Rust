//! Worker notifications cross the channel; manager state stays with the UI owner.
use super::*;
use notify::event::{CreateKind, ModifyKind, RemoveKind};

fn channel_manager() -> (
    HotReloadManager,
    mpsc::Sender<notify::Result<notify::Event>>,
) {
    let mut manager = HotReloadManager::new(false).unwrap();
    let (sender, receiver) = mpsc::channel();
    manager.enabled = true;
    manager.receiver = Some(receiver);
    advance_debounce(&mut manager);
    (manager, sender)
}

fn advance_debounce(manager: &mut HotReloadManager) {
    manager.last_check = Instant::now() - manager.debounce_time;
}

fn event(kind: notify::EventKind, paths: &[&str]) -> notify::Event {
    let mut event = notify::Event::new(kind);
    event.paths = paths.iter().map(PathBuf::from).collect();
    event
}

fn capture(manager: &mut HotReloadManager, id: &str) -> mpsc::Receiver<PathBuf> {
    let (sender, receiver) = mpsc::channel();
    manager.register_callback(id.to_owned(), move |path| {
        sender.send(path.to_owned()).unwrap();
    });
    receiver
}

#[test]
fn worker_events_preserve_path_order_duplicates_and_filters() {
    let (mut manager, sender) = channel_manager();
    let paths = capture(&mut manager, "capture");
    std::thread::spawn(move || {
        sender
            .send(Ok(event(
                notify::EventKind::Create(CreateKind::File),
                &["first.w3d", ".hidden", "backup~", "edit.tmp", "first.w3d"],
            )))
            .unwrap();
        sender
            .send(Err(notify::Error::generic("fixture watcher error")))
            .unwrap();
        sender
            .send(Ok(event(
                notify::EventKind::Remove(RemoveKind::File),
                &["removed.w3d"],
            )))
            .unwrap();
        sender
            .send(Ok(event(
                notify::EventKind::Modify(ModifyKind::Any),
                &["second.w3d"],
            )))
            .unwrap();
    })
    .join()
    .unwrap();
    assert!(manager.check_for_changes());
    assert_eq!(
        paths.try_iter().collect::<Vec<_>>(),
        vec![
            PathBuf::from("first.w3d"),
            PathBuf::from("first.w3d"),
            PathBuf::from("second.w3d")
        ]
    );
    assert!(manager.changed_files.is_empty());
    advance_debounce(&mut manager);
    assert!(!manager.check_for_changes());
}

#[test]
fn debounce_keeps_pending_events_until_next_check() {
    let (mut manager, sender) = channel_manager();
    let paths = capture(&mut manager, "capture");
    manager.last_check = Instant::now();
    sender
        .send(Ok(event(
            notify::EventKind::Modify(ModifyKind::Any),
            &["pending.w3d"],
        )))
        .unwrap();
    assert!(!manager.check_for_changes());
    assert_eq!(paths.try_iter().count(), 0);
    advance_debounce(&mut manager);
    assert!(manager.check_for_changes());
    assert_eq!(
        paths.try_iter().collect::<Vec<_>>(),
        vec![PathBuf::from("pending.w3d")]
    );
}

#[test]
fn disabled_owner_does_not_consume_pending_notifications() {
    let (mut manager, sender) = channel_manager();
    let paths = capture(&mut manager, "capture");
    manager.enabled = false;
    sender
        .send(Ok(event(
            notify::EventKind::Create(CreateKind::File),
            &["pending.w3d"],
        )))
        .unwrap();
    assert!(!manager.check_for_changes());
    assert_eq!(paths.try_iter().count(), 0);
    manager.enabled = true;
    assert!(manager.check_for_changes());
    assert_eq!(
        paths.try_iter().collect::<Vec<_>>(),
        vec![PathBuf::from("pending.w3d")]
    );
}

#[test]
fn independent_owners_do_not_steal_same_path_notifications() {
    let (mut first, first_sender) = channel_manager();
    let (mut second, second_sender) = channel_manager();
    let first_paths = capture(&mut first, "capture");
    let second_paths = capture(&mut second, "capture");
    first_sender
        .send(Ok(event(
            notify::EventKind::Create(CreateKind::File),
            &["same.w3d"],
        )))
        .unwrap();
    assert!(!second.check_for_changes());
    assert!(first.check_for_changes());
    assert_eq!(first_paths.try_iter().count(), 1);
    assert_eq!(second_paths.try_iter().count(), 0);
    second_sender
        .send(Ok(event(
            notify::EventKind::Modify(ModifyKind::Any),
            &["same.w3d"],
        )))
        .unwrap();
    advance_debounce(&mut second);
    assert!(second.check_for_changes());
    assert_eq!(first_paths.try_iter().count(), 0);
    assert_eq!(second_paths.try_iter().count(), 1);
}

#[test]
fn unregister_stops_callback_without_suppressing_change_detection() {
    let (mut manager, sender) = channel_manager();
    let paths = capture(&mut manager, "capture");
    manager.unregister_callback("capture");
    sender
        .send(Ok(event(
            notify::EventKind::Create(CreateKind::File),
            &["changed.w3d"],
        )))
        .unwrap();
    assert!(manager.check_for_changes());
    assert_eq!(paths.try_iter().count(), 0);
}

// This adapter alone changes for the explicit-borrow migration; assertions stay fixed.
fn register_asset(
    cache: &mut AssetHotReload,
    manager: &mut HotReloadManager,
    path: PathBuf,
) -> Result<()> {
    cache.register_asset(manager, path)
}

fn asset_owners(enabled: bool) -> (AssetHotReload, HotReloadManager) {
    (
        AssetHotReload::new(),
        HotReloadManager::new(enabled).unwrap(),
    )
}

#[test]
fn asset_registration_caches_metadata_and_starts_at_zero_reloads() {
    let (mut first, mut first_manager) = asset_owners(false);
    let (mut second, mut second_manager) = asset_owners(false);
    let path = std::env::temp_dir().join(format!("generals-asset-{}", uuid::Uuid::new_v4()));
    std::fs::write(&path, b"fixture").unwrap();
    register_asset(&mut first, &mut first_manager, path.clone()).unwrap();
    assert!(second.get_reload_stats(&path).is_none());
    register_asset(&mut second, &mut second_manager, path.clone()).unwrap();
    for cache in [&first, &second] {
        let entry = cache.get_reload_stats(&path).unwrap();
        assert_eq!(entry.path, path);
        assert_eq!(
            entry.last_modified,
            std::fs::metadata(&path).unwrap().modified().ok()
        );
        assert_eq!(entry.reload_count, 0);
    }
    std::fs::remove_file(path).unwrap();
}

#[test]
fn failed_asset_watch_does_not_insert_cache_entry() {
    let (mut cache, mut manager) = asset_owners(true);
    let path = std::env::temp_dir().join(format!("generals-missing-{}", uuid::Uuid::new_v4()));
    assert!(register_asset(&mut cache, &mut manager, path.clone()).is_err());
    assert!(cache.get_reload_stats(&path).is_none());
}

#[test]
fn asset_metadata_reload_advances_only_its_own_cache() {
    let (mut first, mut first_manager) = asset_owners(false);
    let (mut second, mut second_manager) = asset_owners(false);
    let path = std::env::temp_dir().join(format!("generals-reload-{}", uuid::Uuid::new_v4()));
    register_asset(&mut first, &mut first_manager, path.clone()).unwrap();
    register_asset(&mut second, &mut second_manager, path.clone()).unwrap();
    std::fs::write(&path, b"fixture").unwrap();
    let runtime = tokio::runtime::Runtime::new().unwrap();
    assert!(runtime.block_on(first.needs_reload(&path)));
    assert!(!runtime.block_on(first.needs_reload(&path)));
    assert_eq!(first.get_reload_stats(&path).unwrap().reload_count, 1);
    assert_eq!(second.get_reload_stats(&path).unwrap().reload_count, 0);
    assert!(runtime.block_on(second.needs_reload(&path)));
    std::fs::remove_file(path).unwrap();
}
