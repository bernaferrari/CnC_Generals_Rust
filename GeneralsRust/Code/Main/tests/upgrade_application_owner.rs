use game_engine::common::ascii_string::AsciiString;
use game_engine::common::system::upgrade::{UpgradeCenter, process_lifetime_upgrade_center};
use generals_main::assets::archive::ArchiveFileSystem;
use generals_main::assets::ini_template_loader::{load_upgrade_ini_text, load_upgrade_templates};

fn new_center() -> UpgradeCenter {
    let mut center = UpgradeCenter::new();
    center.init();
    center
}

#[test]
fn applications_keep_independent_upgrade_contents_and_mask_order() {
    // A foreign process catalog is a negative control, not a parser target.
    let foreign = process_lifetime_upgrade_center();
    let foreign_count = {
        let mut center = foreign.write().unwrap();
        center.new_upgrade(AsciiString::from("ForeignUpgrade"));
        center.count()
    };
    let mut first = new_center();
    let mut peer = new_center();
    load_upgrade_ini_text(
        &mut first,
        "Upgrade SharedUpgrade\n BuildCost = 125\n BuildTime = 7\nEnd\n",
    )
    .unwrap();
    load_upgrade_ini_text(
        &mut peer,
        "Upgrade PeerOnly\n BuildCost = 10\nEnd\nUpgrade SharedUpgrade\n BuildCost = 250\n BuildTime = 19\nEnd\n",
    )
    .unwrap();
    let first_before = first.find_upgrade("SharedUpgrade").unwrap();
    let peer_shared = peer.find_upgrade("SharedUpgrade").unwrap();
    assert_eq!(first_before.get_mask().bits(), 1 << 3);
    assert_eq!(peer_shared.get_mask().bits(), 1 << 4);
    assert_eq!(first_before.get_cost(), 125);
    assert_eq!(peer_shared.get_cost(), 250);
    assert!(first.find_upgrade("ForeignUpgrade").is_none());
    assert_eq!(foreign.read().unwrap().count(), foreign_count);

    load_upgrade_ini_text(&mut first, "Upgrade SharedUpgrade\n BuildCost = 175\nEnd\n").unwrap();
    let overlaid = first.find_upgrade("SharedUpgrade").unwrap();
    assert_eq!(overlaid.get_mask(), first_before.get_mask());
    assert_eq!(overlaid.get_build_time(), 7.0);
    assert_eq!(overlaid.get_cost(), 175);
    assert_eq!(peer.find_upgrade("SharedUpgrade").unwrap().get_cost(), 250);
}

fn write_big(path: &std::path::Path, entries: &[(&str, &str)]) {
    let directory_len = 16
        + entries
            .iter()
            .map(|(name, _)| 8 + name.len() + 1)
            .sum::<usize>();
    let archive_len = directory_len + entries.iter().map(|(_, data)| data.len()).sum::<usize>();
    let mut bytes = Vec::with_capacity(archive_len);
    bytes.extend_from_slice(b"BIGF");
    bytes.extend_from_slice(&(archive_len as u32).to_le_bytes());
    bytes.extend_from_slice(&(entries.len() as u32).to_be_bytes());
    bytes.extend_from_slice(&(directory_len as u32).to_be_bytes());
    let mut offset = directory_len;
    for (name, data) in entries {
        bytes.extend_from_slice(&(offset as u32).to_be_bytes());
        bytes.extend_from_slice(&(data.len() as u32).to_be_bytes());
        bytes.extend_from_slice(name.as_bytes());
        bytes.push(0);
        offset += data.len();
    }
    for (_, data) in entries {
        bytes.extend_from_slice(data.as_bytes());
    }
    std::fs::write(path, bytes).unwrap();
}

#[test]
fn each_archive_loads_its_default_then_override_into_its_own_owner() {
    let dir = tempfile::tempdir().unwrap();
    let archive_a = dir.path().join("a.big");
    let archive_b = dir.path().join("b.big");
    // Store the override first: GameEngine.cpp order comes from the named
    // sources, not the archive directory's order.
    write_big(
        &archive_a,
        &[
            (
                "Data/INI/Upgrade.ini",
                "Upgrade Shared\n BuildCost = 175\nEnd\n",
            ),
            (
                "Data/INI/Default/Upgrade.ini",
                "Upgrade Shared\n BuildCost = 125\n BuildTime = 7\nEnd\n",
            ),
        ],
    );
    write_big(
        &archive_b,
        &[
            (
                "Data/INI/Upgrade.ini",
                "Upgrade Shared\n BuildCost = 300\nEnd\n",
            ),
            (
                "Data/INI/Default/Upgrade.ini",
                "Upgrade PeerOnly\n BuildCost = 10\nEnd\nUpgrade Shared\n BuildCost = 250\n BuildTime = 19\nEnd\n",
            ),
        ],
    );
    let mut a = ArchiveFileSystem::new();
    let mut b = ArchiveFileSystem::new();
    a.set_local_search_roots(Vec::new());
    b.set_local_search_roots(Vec::new());
    futures::executor::block_on(a.load_big_file(&archive_a)).unwrap();
    futures::executor::block_on(b.load_big_file(&archive_b)).unwrap();
    let (a_count, first) = futures::executor::block_on(load_upgrade_templates(&mut a)).unwrap();
    let (b_count, peer) = futures::executor::block_on(load_upgrade_templates(&mut b)).unwrap();
    assert_eq!((a_count, b_count), (1, 2));
    let first_shared = first.find_upgrade("Shared").unwrap();
    let peer_shared = peer.find_upgrade("Shared").unwrap();
    assert_eq!(
        (first_shared.get_cost(), first_shared.get_build_time()),
        (175, 7.0)
    );
    assert_eq!(
        (peer_shared.get_cost(), peer_shared.get_build_time()),
        (300, 19.0)
    );
    assert_eq!(first_shared.get_mask().bits(), 1 << 3);
    assert_eq!(peer_shared.get_mask().bits(), 1 << 4);
}
