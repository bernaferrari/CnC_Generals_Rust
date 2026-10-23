//! CPP Drawable.cpp:1174–1204 increments decal opacity once per updateDrawable.
//! Exercise Main's admitted object and ordinary frame boundary. Calling the
//! crate decal domain operation explicitly is not retail crate-spawn evidence.
use super::*;

use crate::game_logic::host_battlemaster::{
    CRATE_DECAL_FADE_IN_RATE, TERRAIN_DECAL_CRATE, TERRAIN_DECAL_NONE,
};

fn admitted_decal() -> (GameLogic, ObjectId) {
    const NAME: &str = "DecalCadenceCrate";
    let source = "Object DecalCadenceCrate\n  KindOf = STRUCTURE\n  Body = ActiveBody ModuleTag_Body\n    MaxHealth = 50\n    InitialHealth = 50\n  End\nEnd\n";
    let module = crate::assets::ReceivingObjectContent::from_sources([(
        "decal_cadence.ini".to_owned(),
        source.to_owned(),
    )])
    .unwrap();
    let mut parser = crate::assets::IniParser::new();
    assert_eq!(
        parser
            .parse_ini_content(source, "decal_cadence.ini",)
            .unwrap(),
        1,
    );
    let mut logic = GameLogic::new();
    // Original Object construction resolves a real neutral default Team.
    let mut neutral = Player::new(0, Team::Neutral, "DecalNeutral", false);
    neutral.map_side.role = crate::game_logic::PlayerSideRole::Neutral;
    neutral.map_side.map_player_name.clear();
    logic.add_player(neutral);

    logic.admit_receiving_module_content(module).unwrap();
    assert!(logic.templates[NAME].is_kind_of(KindOf::Structure));
    // C++ creates this actor on the reserved neutral Player's exact default
    // Team. The generic faction-only convenience path carries no controller.
    let team_name = logic.default_host_team_instance_name(Some(0), Team::Neutral);
    let team_id = logic
        .host_default_team_id(0)
        .expect("admitted neutral default Team");
    let id = logic
        .create_object_on_exact_script_team_with_effects_checked(
            NAME,
            0,
            &team_name,
            team_id,
            Vec3::ZERO,
            false,
            None,
            &mut Default::default(),
        )
        .unwrap()
        .unwrap();
    assert_eq!(
        logic.host_object(id).unwrap().team_instance_id,
        Some(team_id)
    );
    assert_eq!(logic.host_object(id).unwrap().owner_player_id, Some(0));
    assert!(logic.host_object(id).unwrap().team_instance_id.is_some());
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
    let (peer, peer_id) = admitted_decal();
    assert_eq!(id, peer_id);
    for frame in 1..=3 {
        logic.update();
        assert_eq!(logic.getFrame(), frame);
        assert_opacity(&logic, id, frame as f32 * CRATE_DECAL_FADE_IN_RATE);
        assert_eq!(peer.getFrame(), 0);
        assert_opacity(&peer, peer_id, 0.0);
        assert_eq!(peer.host_object(peer_id).unwrap().owner_player_id, Some(0));
    }
}

#[test]
fn terrain_decal_fade_advances_once_per_default_owned_frame() {
    let (mut logic, id) = admitted_decal();
    let mut shadow = GameWorldShadow::new(16);
    for frame in 1..=3 {
        owned_frame(&mut shadow, &mut logic);
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
