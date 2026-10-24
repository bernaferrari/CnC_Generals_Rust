//! C++ containment-specific exit mood contracts.

use super::*;

#[test]
fn capture_kick_exit_drop_preserves_garrison_mood_check_time() {
    // GarrisonContain.cpp:1629–1686 does not run TransportContain.cpp:393.
    use crate::game_logic::containment_audio_test_support as audio;
    let mut logic = GameLogic::new();
    audio::admit(
        &mut logic,
        r#"
Object KICK_BUNKER
 KindOf = STRUCTURE
 SoundExit = KickExit
 Behavior = GarrisonContain ContainTag
  ContainMax = 5
 End
 Body = ActiveBody BodyTag
  MaxHealth = 500
  InitialHealth = 500
 End
End
Object KICK_RANGER
 KindOf = INFANTRY
 SoundFallingFromPlane = KickFalling
 Body = ActiveBody BodyTag
  MaxHealth = 100
  InitialHealth = 100
 End
End
"#,
    );
    let mut output = audio::manager(&["KickExit", "KickFalling"]);
    logic.frame = 55;
    let mut bunker_t = logic.templates.remove("KICK_BUNKER").unwrap();
    bunker_t.add_kind_of(KindOf::Structure).set_health(500.0);
    bunker_t.contain_module = crate::game_logic::ContainModuleMetadata {
        kind: crate::game_logic::ContainModuleKind::Garrison,
        slots: Some(5),
        ..Default::default()
    };
    logic.templates.insert("KICK_BUNKER".into(), bunker_t);
    let mut ranger_t = logic.templates.remove("KICK_RANGER").unwrap();
    ranger_t.add_kind_of(KindOf::Infantry).set_health(100.0);
    logic.templates.insert("KICK_RANGER".into(), ranger_t);
    let bunker = logic
        .create_object("KICK_BUNKER", Team::USA, glam::Vec3::ZERO)
        .unwrap();
    let ranger = logic
        .create_object("KICK_RANGER", Team::USA, glam::Vec3::new(1.0, 0.0, 0.0))
        .unwrap();
    assert!(logic.host_object_mut(bunker).unwrap().add_occupant(ranger));
    if let Some((u, partition)) = logic.host_object_and_partition_mut(ranger) {
        u.set_contained_by_with_partition_owner(Some(bunker), true, partition);
        u.next_mood_check_time = 9999;
        u.randomly_offset_mood_check = false;
    }
    // Capture removal detaches the occupant before issuing its drop command.
    assert!(logic.unit_command_remove_occupant(bunker, ranger));
    assert!(logic.unit_command_exit_drop_with_effects(
        ranger,
        glam::Vec3::new(4.0, 0.0, 0.0),
        &mut crate::game_logic::ScriptPresentationEffects {
            audio: Some(&mut output),
            ..Default::default()
        }
    ));
    let u = logic.host_object(ranger).unwrap();
    assert_eq!(u.next_mood_check_time, 9999);
    assert!(!u.randomly_offset_mood_check);
    assert_eq!(u.contained_by, None);
    assert!(
        !logic
            .host_object(bunker)
            .unwrap()
            .contained_units()
            .contains(&ranger)
    );
    assert_eq!(u.get_position(), glam::Vec3::new(4.0, 0.0, 0.0));
    audio::assert_events(&output, &[("KickExit", bunker), ("KickFalling", ranger)]);
}
