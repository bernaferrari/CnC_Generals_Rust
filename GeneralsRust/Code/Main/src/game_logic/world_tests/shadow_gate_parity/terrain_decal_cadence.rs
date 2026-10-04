//! CPP Drawable.cpp:1174–1204 increments decal opacity once per updateDrawable.
//! Exercise Main's admitted object and ordinary frame boundary. Calling the
//! crate decal domain operation explicitly is not retail crate-spawn evidence.
use super::*;
use crate::game_logic::game_logic::gameworld_authority::GameWorldAuthority;
use crate::game_logic::host_battlemaster::{
    CRATE_DECAL_FADE_IN_RATE, TERRAIN_DECAL_CRATE, TERRAIN_DECAL_NONE,
};

fn admitted_decal() -> (GameLogic, ObjectId) {
    const NAME: &str = "DecalCadenceCrate";
    let mut parser = crate::assets::IniParser::new();
    assert_eq!(
        parser
            .parse_ini_content(
                "Object DecalCadenceCrate\n  Body = ActiveBody ModuleTag_Body\n    MaxHealth = 50\n  End\nEnd\n",
                "decal_cadence.ini",
            )
            .unwrap(),
        1,
    );
    let template = GameLogic::build_template_from_object_definition(
        NAME,
        parser.get_definition(NAME).unwrap(),
        None,
    );
    let mut logic = GameLogic::new();
    logic.set_gameworld_authority(|authority| *authority = GameWorldAuthority::DEFAULT_OFF);
    logic.templates.insert(NAME.into(), template);
    let id = logic
        .create_object(NAME, Team::Neutral, Vec3::ZERO)
        .unwrap();
    logic
        .host_object_mut(id)
        .unwrap()
        .apply_crate_terrain_decal();
    let owner = logic.host_object(id).unwrap();
    assert_eq!(owner.terrain_decal_type, TERRAIN_DECAL_CRATE);
    assert_eq!(owner.terrain_decal_opacity, 0.0);
    assert_eq!(owner.terrain_decal_fade_rate, CRATE_DECAL_FADE_IN_RATE);
    (logic, id)
}

fn assert_opacity(logic: &GameLogic, id: ObjectId, expected: f32) {
    let opacity = logic.host_object(id).unwrap().terrain_decal_opacity;
    assert!(
        (opacity - expected).abs() < 1e-6,
        "one opacity increment per ordinary frame: expected {expected}, got {opacity}",
    );
}

#[test]
fn terrain_decal_fade_advances_once_per_bare_host_frame() {
    let (mut logic, id) = admitted_decal();
    for frame in 1..=3 {
        logic.update();
        assert_eq!(logic.getFrame(), frame);
        assert_opacity(&logic, id, frame as f32 * CRATE_DECAL_FADE_IN_RATE);
    }
}

#[test]
fn terrain_decal_fade_advances_once_per_default_coupled_frame() {
    let _env = ShadowEnvPin::new();
    let (mut logic, id) = admitted_decal();
    let mut shadow = GameWorldShadow::new(16);
    for frame in 1..=3 {
        coupled_frame(&mut shadow, &mut logic);
        assert_eq!(logic.getFrame(), frame);
        assert_opacity(&logic, id, frame as f32 * CRATE_DECAL_FADE_IN_RATE);
    }
}

#[test]
fn terrain_decal_fade_clamps_then_clears_once_at_zero() {
    let (mut logic, id) = admitted_decal();
    // Four increments reach 1.0; the third remains below it.
    logic
        .host_object_mut(id)
        .unwrap()
        .set_terrain_decal_fade_target(1.0, 0.25);
    for frame in 1..=4 {
        logic.update();
        assert_opacity(&logic, id, frame as f32 * 0.25);
    }
    let owner = logic.host_object(id).unwrap();
    assert_eq!(owner.terrain_decal_fade_rate, 0.0);
    assert_eq!(owner.terrain_decal_type, TERRAIN_DECAL_CRATE);
    logic.update();
    assert_opacity(&logic, id, 1.0);

    logic
        .host_object_mut(id)
        .unwrap()
        .set_terrain_decal_fade_target(0.0, -0.25);
    for frame in 1..=3 {
        logic.update();
        assert_opacity(&logic, id, 1.0 - frame as f32 * 0.25);
        assert_eq!(
            logic.host_object(id).unwrap().terrain_decal_type,
            TERRAIN_DECAL_CRATE
        );
    }
    logic.update();
    assert_opacity(&logic, id, 0.0);
    let owner = logic.host_object(id).unwrap();
    assert_eq!(owner.terrain_decal_type, TERRAIN_DECAL_NONE);
    assert_eq!(owner.terrain_decal_fade_rate, 0.0);
    logic.update();
    assert_opacity(&logic, id, 0.0);
}
