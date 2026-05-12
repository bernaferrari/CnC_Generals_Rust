use game_engine::common::system::cd_manager::{
    CDDrive, CDDriveInterface, CDManager, CDManagerInterface, Disk,
};

#[test]
fn base_cd_drive_matches_cpp_unknown_disk_defaults() {
    let mut drive = CDDrive::new();

    assert_eq!(drive.get_disk(), Disk::UnknownDisk);
    assert!(drive.get_disk_name().is_empty());
    assert!(drive.get_path().is_empty());

    drive.set_path("/definitely/not/a/retail/generals/cdrom");
    assert_eq!(
        drive.get_path().as_str(),
        "/definitely/not/a/retail/generals/cdrom"
    );
    assert_eq!(drive.get_disk(), Disk::UnknownDisk);
    assert!(drive.get_disk_name().is_empty());

    drive.refresh_info();
    assert_eq!(drive.get_disk(), Disk::UnknownDisk);
    assert!(drive.get_disk_name().is_empty());
}

#[test]
#[cfg(not(target_os = "windows"))]
fn non_windows_cd_manager_does_not_create_synthetic_mount_drives() {
    let mut manager = CDManager::new();
    manager.init().expect("cd manager init");

    assert_eq!(manager.drive_count(), 0);
}

#[test]
fn manually_added_base_drive_stays_unknown_until_platform_refresh_identifies_disc() {
    let mut manager = CDManager::new();
    let index = manager
        .new_drive("/definitely/not/a/retail/generals/cdrom")
        .expect("manual drive");

    let drive = manager.get_drive(index).expect("drive exists");
    assert_eq!(
        drive.get_path().as_str(),
        "/definitely/not/a/retail/generals/cdrom"
    );
    assert_eq!(drive.get_disk(), Disk::UnknownDisk);

    manager.refresh_drives();
    let drive = manager
        .get_drive(index)
        .expect("drive exists after refresh");
    assert_eq!(drive.get_disk(), Disk::UnknownDisk);
    assert!(drive.get_disk_name().is_empty());
}
