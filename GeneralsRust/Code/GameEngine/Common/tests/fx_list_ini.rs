use game_engine::common::ini::ini::INI;
use game_engine::common::ini::ini_fx_list::{CameraShakeType, FXNugget, get_fx_list_store};
use std::sync::Mutex;
use std::{fs, path::PathBuf};

static FX_LIST_TEST_LOCK: Mutex<()> = Mutex::new(());

#[test]
fn nested_retail_fx_nuggets_parse_and_repeat() {
    let _guard = FX_LIST_TEST_LOCK.lock().expect("FXList test lock");
    let source = r#"
FXList WeaponFX_Test
  ViewShake
    Type = NORMAL
  End
  LightPulse
    Color = R:255 G:255 B:128
    Radius = 25
    IncreaseTime = 0
    DecreaseTime = 500
  End
  Tracer
    DecayAt = 0.5
    Length = 20
    Width = 0.5
    Color = R:230 G:204 B:179
  End
  ParticleSystem
    Name = TankMuzzleFlashSmoke
    Offset = X:1 Y:2 Z:3
    OrientToObject = Yes
    RotateY = 90
  End
  ParticleSystem
    Name = TankMuzzleFlashFlame
    Ricochet = Yes
  End
  Sound
    Name = TankWeapon
  End
End
"#;

    let mut ini = INI::new();
    ini.with_inline_source(source, |ini| ini.parse_current_file())
        .expect("nested retail FXList syntax should parse");

    let store = get_fx_list_store();
    let fx = store.find_fx_list("WeaponFX_Test").expect("FXList stored");
    assert_eq!(fx.nuggets.len(), 6);
    assert!(matches!(
        fx.nuggets[0],
        FXNugget::ViewShake {
            shake_type: CameraShakeType::Normal
        }
    ));
    assert!(matches!(
        fx.nuggets[1],
        FXNugget::LightPulse {
            decrease_frames: 15,
            ..
        }
    ));
    assert!(matches!(
        &fx.nuggets[3],
        FXNugget::ParticleSystem {
            offset: (1.0, 2.0, 3.0),
            orient_to_object: true,
            ..
        }
    ));
    assert!(matches!(
        &fx.nuggets[4],
        FXNugget::ParticleSystem { ricochet: true, .. }
    ));
}

#[test]
fn empty_fx_list_consumes_its_end_token() {
    let _guard = FX_LIST_TEST_LOCK.lock().expect("FXList test lock");
    let source = r#"
FXList FX_Empty
End

FXList FX_AfterEmpty
  Sound
    Name = VoiceEvent
  End
End
"#;
    let mut ini = INI::new();
    ini.with_inline_source(source, |ini| ini.parse_current_file())
        .expect("empty FXList must not leak End into the top-level parser");

    let store = get_fx_list_store();
    assert_eq!(store.find_fx_list("FX_Empty").unwrap().nuggets.len(), 0);
    assert_eq!(
        store.find_fx_list("FX_AfterEmpty").unwrap().nuggets.len(),
        1
    );
}

#[test]
fn retail_windows_game_fx_list_parses_end_to_end_when_present() {
    let _guard = FX_LIST_TEST_LOCK.lock().expect("FXList test lock");
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../../../windows_game/extracted_big_files_v2/INIZH/Data/INI/FXList.ini");
    let Ok(source) = fs::read_to_string(&path) else {
        return;
    };

    let mut ini = INI::new();
    ini.with_inline_source(&source, |ini| ini.parse_current_file())
        .unwrap_or_else(|error| panic!("retail {} must parse: {error:?}", path.display()));

    let store = get_fx_list_store();
    let tank = store
        .find_fx_list("WeaponFX_GenericTankGun")
        .expect("known retail FXList stored");
    assert!(tank.nuggets.len() >= 4);
}
