//! Strict receiving companion reads and immutable source selection.
use super::*;

#[test]
fn frozen_receiving_companions_survive_replacement_without_reopening_source() {
    let directory = tempfile::tempdir().unwrap();
    let map = directory.path().join("Receiving.map");
    fs::write(&map, []).unwrap();
    let map_ini = directory.path().join("Map.ini");
    let solo_ini = directory.path().join("Solo.ini");
    fs::write(&map_ini, "Object Exact\nScale = 2\nEnd\n").unwrap();
    fs::write(&solo_ini, "Object Exact\nScale = 3\nEnd\n").unwrap();
    let frozen = read_receiving_definition_companions(map.to_str().unwrap()).unwrap();
    assert_eq!(
        frozen.iter().map(|(p, _)| p).collect::<Vec<_>>(),
        [&map_ini, &solo_ini]
    );
    fs::write(&map_ini, "Object Malformed\n").unwrap();
    fs::remove_file(&solo_ini).unwrap();
    assert_eq!(
        selected_map_definition_companion(directory.path(), &["Map.ini", "map.ini"], Some(&frozen)),
        Some(frozen[0].clone())
    );
    assert_eq!(
        selected_map_definition_companion(
            directory.path(),
            &["Solo.ini", "solo.ini"],
            Some(&frozen)
        ),
        Some(frozen[1].clone())
    );
    assert!(
        selected_map_definition_companion(directory.path(), &["Map.ini", "map.ini"], Some(&[]))
            .is_none(),
        "authoritative empty sources must not reopen changed files"
    );
}

#[test]
fn strict_receiving_companions_reject_present_utf8_and_read_errors() {
    let directory = tempfile::tempdir().unwrap();
    let map = directory.path().join("Receiving.map");
    fs::write(&map, []).unwrap();
    let map_ini = directory.path().join("Map.ini");
    assert!(
        read_receiving_definition_companions(map.to_str().unwrap())
            .unwrap()
            .is_empty()
    );
    fs::write(&map_ini, [0xff, 0xfe]).unwrap();
    let error = read_receiving_definition_companions(map.to_str().unwrap()).unwrap_err();
    assert!(
        error.contains("Map.ini"),
        "selected invalid source must be an error: {error}"
    );
    fs::remove_file(&map_ini).unwrap();
    fs::create_dir(&map_ini).unwrap();
    assert!(read_receiving_definition_companions(map.to_str().unwrap()).is_err());
}
