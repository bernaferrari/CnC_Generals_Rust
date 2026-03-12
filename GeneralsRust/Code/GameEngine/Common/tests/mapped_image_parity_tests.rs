use game_engine::common::ini::ini::{INILoadType, INI};
use game_engine::common::ini::ini_mapped_image::{
    ensure_mapped_image_collection, init_global_mapped_image_collection, parse_image_coords,
    ICoord2D, Image,
};
use game_engine::common::system::big_file_system::BigArchiveBackend;
use game_engine::common::system::file::FileAccess;
use game_engine::common::system::file_system::get_file_system;
use game_engine::common::system::local_file_system::LocalFileSystem;
use game_engine::common::system::subsystem_interface::SubsystemInterface;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

#[test]
fn parse_image_coords_accepts_space_separated_subtokens() {
    let mut image = Image::new();
    image.set_texture_width(128);
    image.set_texture_height(64);

    let mut ini = INI::new();
    parse_image_coords(
        &mut ini,
        &mut image,
        &["Left", "16", "Top", "8", "Right", "80", "Bottom", "40"],
    )
    .expect("coords parse should accept spaced subtoken form");

    assert_eq!(image.get_image_size(), ICoord2D::new(64, 32));
    let uv = image.get_uv();
    assert!((uv.left - (16.0 / 128.0)).abs() < 1e-6);
    assert!((uv.top - (8.0 / 64.0)).abs() < 1e-6);
    assert!((uv.right - (80.0 / 128.0)).abs() < 1e-6);
    assert!((uv.bottom - (40.0 / 64.0)).abs() < 1e-6);
}

#[test]
fn parse_mapped_image_definition_overwrites_in_place_with_raw_texture_present() {
    init_global_mapped_image_collection();
    let collection_handle = ensure_mapped_image_collection();
    {
        let mut collection = collection_handle.write();
        collection.clear();
        let mut existing = Image::new();
        existing.set_name("RawTextureImage".to_string());
        existing.set_filename("old_texture.tga".to_string());
        existing.set_texture_width(64);
        existing.set_texture_height(64);
        existing.set_raw_texture_data(vec![1, 2, 3, 4]);
        collection.add_image(existing);
    }

    let mut ini = INI::new();
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let ini_path = std::env::temp_dir().join(format!("mapped_image_parity_{unique}.ini"));
    let ini_text = "\
MappedImage RawTextureImage
  Texture = new_texture.tga
  TextureWidth = 256
  TextureHeight = 128
  Coords = Left 0 Top 0 Right 64 Bottom 32
End
";
    std::fs::write(&ini_path, ini_text).expect("write mapped image ini");
    let load_result = ini.load(&ini_path, INILoadType::Overwrite);
    let _ = std::fs::remove_file(&ini_path);
    assert!(load_result.is_ok(), "mapped image parse should succeed");

    let collection = collection_handle.read();
    let parsed = collection
        .find_image_by_name("RawTextureImage")
        .expect("parsed image should exist");
    assert_eq!(parsed.get_filename(), "new_texture.tga");
    assert_eq!(parsed.get_texture_size(), ICoord2D::new(256, 128));
    assert_eq!(parsed.get_image_size(), ICoord2D::new(64, 32));
    assert!(
        parsed.get_raw_texture_data().is_some(),
        "raw texture payload should remain present after in-place parse"
    );
}

#[test]
fn common_mapped_image_load_finds_shell_menu_images_from_repo_assets() {
    init_global_mapped_image_collection();
    game_engine::common::ini::ini_mapped_image::ImageCollection::load_global(512);

    let collection_handle = ensure_mapped_image_collection();
    let collection = collection_handle.read();
    let total = collection.len();
    let backdrop = collection.find_image_by_name("MainMenuBackdrop");
    let ruler = collection.find_image_by_name("MainMenuRuler");
    let logo = collection.find_image_by_name("GeneralsLogo");
    let pulse = collection.find_image_by_name("MainMenuPulse");

    assert!(
        backdrop.is_some(),
        "MainMenuBackdrop missing from common mapped image load; total={total}"
    );
    assert!(
        ruler.is_some(),
        "MainMenuRuler missing from common mapped image load; total={total}"
    );
    assert!(
        logo.is_some(),
        "GeneralsLogo missing from common mapped image load; total={total}"
    );
    assert!(
        pulse.is_some(),
        "MainMenuPulse missing from common mapped image load; total={total}"
    );

    let backdrop = backdrop.unwrap();
    let filename = backdrop.get_filename().to_ascii_lowercase();
    assert!(
        filename.contains("mainmenubackdrop"),
        "unexpected MainMenuBackdrop filename: {}",
        backdrop.get_filename()
    );

    let roots: Vec<PathBuf> = std::env::current_dir()
        .ok()
        .into_iter()
        .flat_map(|cwd| cwd.ancestors().map(PathBuf::from).collect::<Vec<_>>())
        .collect();
    assert!(
        !roots.is_empty(),
        "repo-root ancestor discovery unexpectedly empty"
    );
}

#[test]
fn shell_menu_mapped_images_report_raw_texture_state() {
    init_global_mapped_image_collection();
    game_engine::common::ini::ini_mapped_image::ImageCollection::load_global(512);

    let collection_handle = ensure_mapped_image_collection();
    let collection = collection_handle.read();

    for name in [
        "MainMenuBackdrop",
        "MainMenuPulse",
        "GeneralsLogo",
        "MainMenuRuler",
    ] {
        let image = collection
            .find_image_by_name(name)
            .unwrap_or_else(|| panic!("expected mapped image '{name}'"));
        eprintln!(
            "SHELL_MAPPED_IMAGE {name} file={} raw={} status=0x{:08x} size={}x{} tex={}x{}",
            image.get_filename(),
            image.get_raw_texture_data().is_some(),
            image.get_status(),
            image.get_image_size().x,
            image.get_image_size().y,
            image.get_texture_size().x,
            image.get_texture_size().y,
        );
    }
}

fn configure_repo_asset_filesystem() {
    let fs = get_file_system();
    let mut guard = fs.lock().expect("FileSystem mutex poisoned");
    let _ = guard.reset();

    let cwd = std::env::current_dir().expect("cwd");
    let mut search_paths = vec![
        cwd.join("windows_game"),
        cwd.join("windows_game/Command & Conquer Generals Zero Hour"),
        cwd.join("windows_game/extracted_big_files"),
        cwd.join("windows_game/extracted_big_files/TexturesZH"),
        cwd.join("windows_game/extracted_big_files/EnglishZH"),
        cwd.join("windows_game/extracted_big_files_v2"),
        cwd.join("windows_game/extracted_big_files_v2/TexturesZH"),
        cwd.join("windows_game/extracted_big_files_v2/EnglishZH"),
    ];
    for ancestor in cwd.ancestors() {
        search_paths.push(ancestor.join("windows_game"));
        search_paths.push(ancestor.join("windows_game/Command & Conquer Generals Zero Hour"));
        search_paths.push(ancestor.join("windows_game/extracted_big_files"));
        search_paths.push(ancestor.join("windows_game/extracted_big_files/TexturesZH"));
        search_paths.push(ancestor.join("windows_game/extracted_big_files/EnglishZH"));
        search_paths.push(ancestor.join("windows_game/extracted_big_files_v2"));
        search_paths.push(ancestor.join("windows_game/extracted_big_files_v2/TexturesZH"));
        search_paths.push(ancestor.join("windows_game/extracted_big_files_v2/EnglishZH"));
    }

    {
        let backend: &mut LocalFileSystem = guard.ensure_backend(LocalFileSystem::new);
        for path in &search_paths {
            backend.add_search_path(path);
        }
    }
    {
        let backend: &mut BigArchiveBackend = guard.ensure_backend(BigArchiveBackend::new);
        for path in &search_paths {
            backend.add_search_path(path);
        }
    }

    guard.clear_cache();
    let _ = guard.init();
}

#[test]
fn mounted_filesystem_opens_available_shell_menu_art_from_repo_assets() {
    configure_repo_asset_filesystem();

    let fs = get_file_system();
    let mut guard = fs.lock().expect("FileSystem mutex poisoned");

    let candidates = [
        "MainMenuBackdropuserinterface.tga",
        "Art/Textures/MainMenuBackdropuserinterface.tga",
        "SCShellUserInterface512_001.tga",
        "Data/English/Art/Textures/SCShellUserInterface512_001.tga",
        "GeneralsLogouserinterface.tga",
        "Art/Textures/GeneralsLogouserinterface.tga",
        "SCSmShellUserInterface512_001.tga",
        "Data/English/Art/Textures/SCSmShellUserInterface512_001.tga",
        "MainMenuRuleruserinterface.tga",
        "Art/Textures/MainMenuRuleruserinterface.tga",
    ];

    let mut resolved = Vec::new();
    for candidate in candidates {
        if let Some(mut file) = guard.open_file(candidate, FileAccess::READ) {
            let bytes = file
                .read_entire_and_close()
                .expect("shell art bytes should be readable");
            assert!(
                !bytes.is_empty(),
                "shell art '{}' opened but returned no payload",
                candidate
            );
            resolved.push(candidate);
        }
    }

    assert!(
        resolved
            .iter()
            .any(|path| path.contains("SCSmShellUserInterface512_001")),
        "failed to open shell atlas art via mounted filesystem; resolved={resolved:?}"
    );
    assert!(
        resolved
            .iter()
            .any(|path| path.contains("SCShellUserInterface512_001")),
        "failed to open MainMenuPulse shell atlas art via mounted filesystem; resolved={resolved:?}"
    );
    assert!(
        resolved
            .iter()
            .any(|path| path.contains("SCSmShellUserInterface512_001")),
        "failed to open GeneralsLogo atlas art via mounted filesystem; resolved={resolved:?}"
    );
    assert!(
        resolved.iter().any(|path| path.contains("MainMenuRuler")),
        "failed to open MainMenuRuler shell art via mounted filesystem; resolved={resolved:?}"
    );
}

#[test]
fn mounted_archive_index_reports_shell_menu_art_candidates() {
    configure_repo_asset_filesystem();

    let fs = get_file_system();
    let mut guard = fs.lock().expect("FileSystem mutex poisoned");
    let backend: &mut BigArchiveBackend = guard
        .get_backend_mut::<BigArchiveBackend>()
        .expect("BIG backend should exist");

    let virtual_paths = backend.virtual_paths();
    let shell_candidates: Vec<_> = virtual_paths
        .into_iter()
        .filter(|path| {
            let lower = path.to_ascii_lowercase();
            lower.contains("mainmenu")
                || lower.contains("generalslogo")
                || lower.contains("scsmshell")
                || lower.contains("scshell")
        })
        .collect();

    eprintln!("SHELL_ARCHIVE_CANDIDATES {}", shell_candidates.len());
    for path in &shell_candidates {
        eprintln!("SHELL_ARCHIVE_PATH {path}");
    }

    assert!(
        shell_candidates.iter().any(|path| path
            .to_ascii_lowercase()
            .contains("scsmshelluserinterface512_001")),
        "expected shell archive atlas path to exist; candidates={shell_candidates:?}"
    );
    assert!(
        shell_candidates.iter().any(|path| path
            .to_ascii_lowercase()
            .contains("scshelluserinterface512_001")),
        "expected shell pulse atlas path to exist; candidates={shell_candidates:?}"
    );
}
