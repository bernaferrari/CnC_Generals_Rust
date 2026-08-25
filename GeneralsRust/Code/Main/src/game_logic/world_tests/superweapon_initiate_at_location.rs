//! C++ `SpecialPowerModule::aboutToDoSpecialPower` InitiateAtLocationSound at click.
#![allow(unused_imports, non_snake_case, unused_variables, dead_code)]
use super::super::*;
use super::helpers::*;

/// Given a command center, when a superweapon fires at a world click, then the
/// authored `InitiateAtLocationSound` is queued at that click — not only the
/// invented `activate_audio()` template label.
///
/// C++ SpecialPowerModule.cpp:622-628 `getInitiateAtTargetSound()` at Coord3D.
#[test]
fn superweapon_fire_queues_initiate_at_location_sound_at_click() {
    use crate::command_system::SpecialPowerType;
    use crate::game_logic::special_power_strikes::{
        CRUISE_MISSILE_INITIATE_AT_LOCATION_SOUND, HostSuperweaponKind,
        NUCLEAR_MISSILE_INITIATE_AT_LOCATION_SOUND,
    };
    use crate::game_logic::{KindOf, Team, ThingTemplate};

    let cases = [
        (
            SpecialPowerType::NuclearMissile,
            HostSuperweaponKind::NuclearMissile,
            NUCLEAR_MISSILE_INITIATE_AT_LOCATION_SOUND,
        ),
        (
            SpecialPowerType::CruiseMissile,
            HostSuperweaponKind::CruiseMissile,
            CRUISE_MISSILE_INITIATE_AT_LOCATION_SOUND,
        ),
    ];

    for (power, kind, authored) in cases {
        assert!(
            !authored.is_empty(),
            "{power:?} must have an authored InitiateAtLocationSound"
        );
        assert_ne!(
            authored,
            kind.activate_audio(),
            "at-location cue is distinct from invented activate_audio"
        );

        let mut logic = GameLogic::new();
        ensure_test_player_for_team(&mut logic, Team::USA);
        let mut cc = ThingTemplate::new("AmericaCommandCenter");
        cc.add_kind_of(KindOf::Structure)
            .add_kind_of(KindOf::CommandCenter)
            .set_health(5000.0);
        logic.templates.insert("AmericaCommandCenter".into(), cc);
        let source = logic
            .create_object("AmericaCommandCenter", Team::USA, glam::Vec3::ZERO)
            .expect("source");
        let click = glam::Vec3::new(220.0, 0.0, 180.0);
        logic.queued_audio_events.clear();
        assert!(
            logic
                .queue_special_power_strike(&power, source, click)
                .is_some(),
            "{power:?} must queue a host strike"
        );

        let at_click: Vec<_> = logic
            .queued_audio_events
            .iter()
            .filter(|e| e.event_type == authored)
            .collect();
        assert!(
            !at_click.is_empty(),
            "{power:?} fire must queue authored InitiateAtLocationSound {authored:?} at click, got {:?}",
            logic
                .queued_audio_events
                .iter()
                .map(|e| e.event_type.as_str())
                .collect::<Vec<_>>()
        );
        assert!(
            at_click.iter().any(|e| e.position == Some(click)),
            "{power:?} InitiateAtLocationSound must play at the click {click:?}"
        );
        assert!(
            logic
                .queued_audio_events
                .iter()
                .any(|e| e.event_type == authored && e.event_type != kind.activate_audio()),
            "{power:?} must not rely only on invented activate_audio {}",
            kind.activate_audio()
        );
    }
}
