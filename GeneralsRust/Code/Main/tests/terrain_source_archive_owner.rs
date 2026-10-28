use std::io::Write;

use game_client::terrain::{
    HeightMap,
    terrain_visual::{TerrainSourceTileClass, TerrainVisualImpl},
};
use generals_main::assets::archive::ArchiveFileSystem;

fn write_single_file_big(path: &std::path::Path, virtual_path: &str, data: &[u8]) {
    let mut file = std::fs::File::create(path).unwrap();
    let data_offset = 0x10 + 8 + virtual_path.len() + 1;
    let archive_size = data_offset + data.len();
    file.write_all(b"BIGF").unwrap();
    file.write_all(&(archive_size as u32).to_le_bytes())
        .unwrap();
    file.write_all(&1u32.to_be_bytes()).unwrap();
    file.write_all(&(data_offset as u32).to_be_bytes()).unwrap();
    file.write_all(&(data_offset as u32).to_be_bytes()).unwrap();
    file.write_all(&(data.len() as u32).to_be_bytes()).unwrap();
    file.write_all(virtual_path.as_bytes()).unwrap();
    file.write_all(&[0]).unwrap();
    file.write_all(data).unwrap();
}

fn solid_64_tga(rgb: [u8; 3]) -> Vec<u8> {
    let mut bytes = vec![0; 18];
    bytes[2] = 2; // Uncompressed true-color image.
    bytes[12..14].copy_from_slice(&64u16.to_le_bytes());
    bytes[14..16].copy_from_slice(&64u16.to_le_bytes());
    bytes[16] = 24;
    for _ in 0..64 * 64 {
        bytes.extend_from_slice(&[rgb[2], rgb[1], rgb[0]]);
    }
    bytes
}

fn terrain_color(files: &mut ArchiveFileSystem, class_name: &str) -> [f32; 3] {
    let mut terrain = TerrainVisualImpl::new();
    let mut heightmap = HeightMap::new(8, 8, 255.0, 10.0);
    heightmap.tile_ndxes = vec![0; 8 * 8];
    terrain
        .load_heightmap_from_data(heightmap, None, None)
        .unwrap();
    let classes = [TerrainSourceTileClass {
        first_tile: 0,
        num_tiles: 1,
        width: 1,
        name: class_name.into(),
    }];
    terrain
        .load_source_tiles_from_texture_classes_with_reader(&classes, |_| None)
        .unwrap();
    assert!(terrain.radar_terrain_color_at(0.0, 0.0).is_none());
    let loaded = terrain
        .load_source_tiles_from_texture_classes_with_reader(&classes, |path| {
            files.open_file_sync(path).ok()
        })
        .unwrap();
    assert_eq!(loaded, 1);
    terrain.radar_terrain_color_at(0.0, 0.0).unwrap()
}

struct RemoveOnDrop(std::path::PathBuf);

impl Drop for RemoveOnDrop {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

#[test]
fn terrain_source_tiles_read_from_the_receiving_archive_mount() {
    let first_dir = tempfile::tempdir().unwrap();
    let second_dir = tempfile::tempdir().unwrap();
    let suffix = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let class_name = format!("TerrainSourceOwner-{}-{suffix}", std::process::id());
    let virtual_path = format!("{class_name}.tga");
    let cwd_conflict = std::env::current_dir().unwrap().join(&virtual_path);
    assert!(!cwd_conflict.exists());
    std::fs::write(&cwd_conflict, solid_64_tga([10, 20, 230])).unwrap();
    let _remove_cwd_conflict = RemoveOnDrop(cwd_conflict);
    write_single_file_big(
        &first_dir.path().join("first.big"),
        &virtual_path,
        &solid_64_tga([220, 30, 40]),
    );
    write_single_file_big(
        &second_dir.path().join("second.big"),
        &virtual_path,
        &solid_64_tga([20, 190, 70]),
    );

    let mut first_mount = ArchiveFileSystem::new();
    let mut second_mount = ArchiveFileSystem::new();
    first_mount.set_local_search_roots(Vec::new());
    second_mount.set_local_search_roots(Vec::new());
    pollster::block_on(first_mount.load_big_file(first_dir.path().join("first.big"))).unwrap();
    pollster::block_on(second_mount.load_big_file(second_dir.path().join("second.big"))).unwrap();
    assert_eq!(
        first_mount.open_file_sync(&virtual_path).unwrap(),
        solid_64_tga([220, 30, 40])
    );
    assert_eq!(
        second_mount.open_file_sync(&virtual_path).unwrap(),
        solid_64_tga([20, 190, 70])
    );

    let first = terrain_color(&mut first_mount, &class_name);
    let second = terrain_color(&mut second_mount, &class_name);
    assert_eq!(first, [220.0 / 255.0, 30.0 / 255.0, 40.0 / 255.0]);
    assert_eq!(second, [20.0 / 255.0, 190.0 / 255.0, 70.0 / 255.0]);
    assert_ne!(first, second);

    let first_after_interleave = terrain_color(&mut first_mount, &class_name);
    assert_eq!(first_after_interleave, first);
}

#[test]
fn startup_ini_reads_ordered_sources_from_the_receiving_archive() {
    use game_engine::common::ini::ini_game_data::{PowerBarRules, ProductionEnergyRules};

    const DEFAULT: &str = "Data/INI/Default/GameData.ini";
    const OVERRIDE: &str = "Data/INI/GameData.ini";
    const INVALID: &str = "Data/INI/InvalidOwnerTest.ini";
    let first_dir = tempfile::tempdir().unwrap();
    let second_dir = tempfile::tempdir().unwrap();

    async fn mounted(
        directory: &std::path::Path,
        default: i32,
        override_value: i32,
    ) -> ArchiveFileSystem {
        let mut archive = ArchiveFileSystem::new();
        archive.set_local_search_roots(Vec::new());
        for (name, path, data) in [
            (
                "default.big",
                DEFAULT,
                format!("GameData\n MaxLineBuildObjects = {default}\n PowerBarBase = {default}\n PowerBarIntervals = 4.5\nEnd\n").into_bytes(),
            ),
            (
                "override.big",
                OVERRIDE,
                format!("GameData\n MaxLineBuildObjects = {override_value}\n PowerBarBase = {override_value}\nEnd\n").into_bytes(),
            ),
            ("invalid.big", INVALID, vec![0xff]),
        ] {
            let big = directory.join(name);
            write_single_file_big(&big, path, &data);
            archive.load_big_file(big).await.unwrap();
        }
        archive
    }

    fn admitted_limit(archive: &mut ArchiveFileSystem) -> i32 {
        let sources = archive
            .read_ordered_ini_sources(&[DEFAULT, "Data/INI/Missing.ini", OVERRIDE], |_| None)
            .unwrap();
        assert_eq!(
            sources.iter().map(|(path, _)| *path).collect::<Vec<_>>(),
            [DEFAULT, OVERRIDE]
        );
        sources
            .into_iter()
            .fold(ProductionEnergyRules::default(), |rules, (_, text)| {
                rules.overlay_ini_text(&text).unwrap()
            })
            .max_line_build_objects
    }

    fn admitted_power_rules(archive: &mut ArchiveFileSystem) -> PowerBarRules {
        archive
            .read_ordered_ini_sources(&[DEFAULT, OVERRIDE], |_| None)
            .unwrap()
            .into_iter()
            .fold(PowerBarRules::default(), |rules, (_, text)| {
                rules.overlay_ini_text(&text).unwrap()
            })
    }

    let mut first = pollster::block_on(mounted(first_dir.path(), 11, 19));
    let mut second = pollster::block_on(mounted(second_dir.path(), 23, 37));
    assert_eq!(admitted_limit(&mut first), 19);
    assert_eq!(admitted_limit(&mut second), 37);
    assert_eq!(admitted_limit(&mut first), 19);
    let first_power = admitted_power_rules(&mut first);
    let second_power = admitted_power_rules(&mut second);
    assert_eq!((first_power.base, first_power.intervals), (19, 4.5));
    assert_eq!((second_power.base, second_power.intervals), (37, 4.5));
    let map_power = first_power
        .overlay_ini_text("GameData\nPowerBarBase = 13\nEnd\n")
        .unwrap()
        .overlay_ini_text("GameData\nPowerBarYellowRange = 9\nEnd\n")
        .unwrap();
    assert_eq!((map_power.base, map_power.yellow_range), (13, 9));
    assert_eq!(admitted_power_rules(&mut second), second_power);
    assert_eq!(admitted_power_rules(&mut first), first_power);
    let loose = first
        .read_ordered_ini_sources(&[OVERRIDE], |_| {
            Some("GameData\n MaxLineBuildObjects = 59\nEnd\n".into())
        })
        .unwrap();
    assert_eq!(
        ProductionEnergyRules::default()
            .overlay_ini_text(&loose[0].1)
            .unwrap()
            .max_line_build_objects,
        59
    );
    assert!(
        first
            .read_ordered_ini_sources(&[INVALID], |_| None)
            .unwrap_err()
            .starts_with(INVALID)
    );
    drop(second);
    assert_eq!(admitted_limit(&mut first), 19);
}
