//! FXList.cpp clears a redefined list; definitions belong to the receiving game.
use generals_main::game_logic::GameLogic;

#[test]
fn same_named_fx_lists_are_owned_and_redefinitions_are_atomic() {
    let source = |name: &str| format!("FXList Shared\n Sound\n Name = {name}\n End\nEnd\n");
    let mut left = GameLogic::new();
    let mut right = GameLogic::new();
    assert_eq!(left.admit_fx_source(&source("LeftSound")).unwrap(), 1);
    assert_eq!(right.admit_fx_source(&source("RightSound")).unwrap(), 1);
    assert_eq!(left.fx_list_sound_names("Shared"), ["LeftSound"]);
    assert!(left.fx_list_sound_names("shared").is_empty());
    assert_eq!(right.fx_list_sound_names("Shared"), ["RightSound"]);

    // A mixed map source must skip a nested Object before the FXList block.
    let mixed = format!(
        "Object Unrelated\n Body = ActiveBody Tag\n MaxHealth = 100\n End\nEnd\n{}",
        source("LeftReplacement")
    );
    assert_eq!(left.admit_fx_source(&mixed).unwrap(), 1);
    assert_eq!(left.fx_list_sound_names("Shared"), ["LeftReplacement"]);
    assert_eq!(right.fx_list_sound_names("Shared"), ["RightSound"]);
    let bad = format!(
        "{}FXList Broken\n NotAnFxNugget\n End\nEnd\n",
        source("Uncommitted")
    );
    assert!(left.admit_fx_source(&bad).is_err());
    assert_eq!(left.fx_list_sound_names("Shared"), ["LeftReplacement"]);
    assert_eq!(right.fx_list_sound_names("Shared"), ["RightSound"]);
    left.reset();
    assert_eq!(left.fx_list_sound_names("Shared"), ["LeftReplacement"]);
    assert_eq!(right.fx_list_sound_names("Shared"), ["RightSound"]);
}

#[test]
fn retail_fx_list_is_admitted_when_extracted_content_is_available() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../windows_game/extracted_big_files_v2/INI/FXList.ini");
    let Ok(source) = std::fs::read_to_string(path) else {
        return;
    };
    let mut world = GameLogic::new();
    assert!(world.admit_fx_source(&source).unwrap() > 0);
    assert_eq!(
        world.fx_list_sound_names("FX_CarOverlappedByCrusher"),
        ["CarMount"]
    );
}
