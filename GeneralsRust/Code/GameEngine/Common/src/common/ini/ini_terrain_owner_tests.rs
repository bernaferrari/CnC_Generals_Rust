//! Owned parser and actual INI target contracts; no shared-registry fixture locks.
use super::*;
use crate::common::ini::{INI, INILoadType};

fn parse_target(base: TerrainTypes, text: &str) -> Result<TerrainTypes, String> {
    let mut ini = INI::new();
    ini.set_terrain_types_target(base);
    ini.with_inline_source_and_load_type(text, INILoadType::CreateOverrides, |ini| {
        ini.parse_current_file()
    })
    .map_err(|error| error.to_string())?;
    Ok(ini.take_terrain_types_target().unwrap())
}

#[cfg(not(target_arch = "wasm32"))]
fn isolated(name: &str, run: impl FnOnce()) {
    let full = format!("{}::{name}", module_path!().split_once("::").unwrap().1);
    if std::env::var("GENERALS_TERRAIN_DEFINITION_OWNER_TEST")
        .ok()
        .as_deref()
        == Some(full.as_str())
    {
        run();
        return;
    }
    let directory = tempfile::tempdir().unwrap();
    let output = std::process::Command::new(std::env::current_exe().unwrap())
        .args(["--exact", &full, "--nocapture", "--test-threads=1"])
        .env("GENERALS_TERRAIN_DEFINITION_OWNER_TEST", &full)
        .current_dir(directory.path())
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "owned terrain child failed: {stdout}\n{stderr}"
    );
    assert!(
        stdout.contains("running 1 test") && stdout.contains("1 passed; 0 failed"),
        "owned terrain child must execute exactly one test: {stdout}\n{stderr}"
    );
}
#[cfg(target_arch = "wasm32")]
fn isolated(_: &str, run: impl FnOnce()) {
    run();
}

#[test]
fn owned_file_and_archive_bytes_share_defaults_override_and_cpp_head_order() {
    let root = tempfile::tempdir().unwrap();
    let default = b"Terrain DefaultTerrain\nTexture = Base.tga\nClass = ROCK\nBlendEdges = Yes\nEnd\nTerrain Shared\nTexture = Child.tga\nEnd\nTerrain Other\nTexture = Other.tga\nEnd\n";
    let overrides = b"Terrain Shared\nClass = SNOW_FLAT\nEnd\nTerrain DefaultTerrain\nTexture = FutureDefault.tga\nEnd\nTerrain Future\nEnd\n";
    let default_path = root.path().join("DefaultTerrain.ini");
    let override_path = root.path().join("Terrain.ini");
    std::fs::write(&default_path, default).unwrap();
    std::fs::write(&override_path, overrides).unwrap();
    let files = read_terrain_definition_files(&[default_path, override_path]).unwrap();
    let archive = read_terrain_definition_sources(&[
        ("Data/INI/Default/Terrain.ini", default),
        ("Data/INI/Terrain.ini", overrides),
    ])
    .unwrap();
    assert_eq!(
        files, archive,
        "file and owned archive bytes must run the same parser"
    );
    assert_eq!(
        archive.get_terrain_names(),
        vec!["Future", "Other", "Shared", "DefaultTerrain"]
    );
    let shared = archive.find_terrain(&AsciiString::from("Shared")).unwrap();
    assert_eq!(shared.texture_name.as_str(), "Child.tga");
    assert_eq!(shared.surface_type, TerrainSurface::Snow);
    assert!(shared.blend_edge_texture);
    let future = archive.find_terrain(&AsciiString::from("Future")).unwrap();
    assert_eq!(future.texture_name.as_str(), "FutureDefault.tga");
    assert_eq!(future.surface_type, TerrainSurface::Rock);
}

#[test]
fn owned_ini_map_then_solo_overrides_keep_base_and_peer_independent_without_publication() {
    isolated(
        "owned_ini_map_then_solo_overrides_keep_base_and_peer_independent_without_publication",
        || {
            assert!(
                get_terrain_types().is_none(),
                "child starts without a published Terrain registry"
            );
            let base = parse_target(TerrainTypes::new(), "Terrain DefaultTerrain\nClass = ROCK\nBlendEdges = Yes\nEnd\nTerrain Shared\nTexture = Base.tga\nEnd\n").unwrap();
            let a = parse_target(base.clone(), "Terrain Shared\nTexture = A.tga\nEnd\n").unwrap();
            let b = parse_target(
                base.clone(),
                "Terrain Shared\nTexture = B.tga\nClass = SNOW_FLAT\nEnd\n",
            )
            .unwrap();
            let a = parse_target(a, "Terrain Shared\nClass = SAND\nEnd\n").unwrap();
            assert_eq!(
                a.find_terrain(&AsciiString::from("Shared"))
                    .unwrap()
                    .texture_name
                    .as_str(),
                "A.tga"
            );
            assert_eq!(
                a.find_terrain(&AsciiString::from("Shared"))
                    .unwrap()
                    .surface_type,
                TerrainSurface::Sand
            );
            assert_eq!(
                b.find_terrain(&AsciiString::from("Shared"))
                    .unwrap()
                    .texture_name
                    .as_str(),
                "B.tga"
            );
            assert_eq!(
                b.find_terrain(&AsciiString::from("Shared"))
                    .unwrap()
                    .surface_type,
                TerrainSurface::Snow
            );
            assert_eq!(
                base.find_terrain(&AsciiString::from("Shared"))
                    .unwrap()
                    .texture_name
                    .as_str(),
                "Base.tga"
            );
            assert!(
                a.find_terrain(&AsciiString::from("Shared"))
                    .unwrap()
                    .blend_edge_texture
            );
            assert!(
                get_terrain_types().is_none(),
                "owned full INI dispatch must not publish its catalogue"
            );
        },
    );
}

#[test]
fn failed_owned_ini_target_keeps_prior_catalogue_and_never_publishes_partial_rows() {
    isolated(
        "failed_owned_ini_target_keeps_prior_catalogue_and_never_publishes_partial_rows",
        || {
            let base = parse_target(
                TerrainTypes::new(),
                "Terrain Shared\nTexture = Accepted.tga\nClass = ROCK\nEnd\n",
            )
            .unwrap();
            assert!(
                parse_target(
                    base.clone(),
                    "Terrain Shared\nTexture = Rejected.tga\nUnknownTerrainField = 1\nEnd\n"
                )
                .is_err()
            );
            assert_eq!(
                base.find_terrain(&AsciiString::from("Shared"))
                    .unwrap()
                    .texture_name
                    .as_str(),
                "Accepted.tga"
            );
            assert_eq!(base.get_terrain_names(), vec!["Shared"]);
            assert!(
                get_terrain_types().is_none(),
                "failed owned parser must not create or replace a global registry"
            );
        },
    );
}

#[test]
fn same_basename_owned_sources_do_not_share_temp_files_or_definition_storage() {
    let a = tempfile::tempdir().unwrap();
    let b = tempfile::tempdir().unwrap();
    let a_bytes = b"Terrain Shared\nTexture = OwnerA.tga\nClass = ROCK\nEnd\n";
    let b_bytes = b"Terrain Shared\nTexture = OwnerB.tga\nClass = SNOW_FLAT\nEnd\n";
    std::fs::write(a.path().join("Terrain.ini"), a_bytes).unwrap();
    std::fs::write(b.path().join("Terrain.ini"), b_bytes).unwrap();
    let a_catalogue =
        read_terrain_definition_sources(&[("Data/INI/Terrain.ini", a_bytes)]).unwrap();
    let b_catalogue =
        read_terrain_definition_sources(&[("Data/INI/Terrain.ini", b_bytes)]).unwrap();
    assert_eq!(
        a_catalogue,
        read_terrain_definition_files(&[a.path().join("Terrain.ini")]).unwrap()
    );
    assert_eq!(
        b_catalogue,
        read_terrain_definition_files(&[b.path().join("Terrain.ini")]).unwrap()
    );
    assert_eq!(
        a_catalogue
            .find_terrain(&AsciiString::from("Shared"))
            .unwrap()
            .texture_name
            .as_str(),
        "OwnerA.tga"
    );
    assert_eq!(
        b_catalogue
            .find_terrain(&AsciiString::from("Shared"))
            .unwrap()
            .texture_name
            .as_str(),
        "OwnerB.tga"
    );
}
