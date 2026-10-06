//! Private adapter storage is owned by its AI, as C++ m_turretAI is.
//! Canonical UnitAI admission and turret-machine continuation have their own tests.
use super::*;

#[test]
fn turret_controls_preserve_absent_and_invalid_slots() {
    let mut ai = AIUpdateInterface::new(Arc::new(AIUpdateModuleData::default()));
    for slot in [
        WhichTurretType::Main,
        WhichTurretType::Alt,
        WhichTurretType::Invalid,
    ] {
        assert_eq!(ai.get_turret_rot_and_pitch(slot), None);
        assert!(!ai.is_turret_in_natural_position(slot));
        ai.set_turret_target_object(slot, INVALID_ID);
        ai.set_turret_target_position(slot, Coord3D::ZERO);
        ai.set_turret_enabled(slot, false);
        ai.recenter_turret(slot);
        assert_eq!(ai.get_turret_rot_and_pitch(slot), None);
    }
}

#[test]
fn turret_controls_borrow_only_the_selected_ai_and_slot() {
    let data = Arc::new(AIUpdateModuleData::default());
    let mut first = AIUpdateInterface::new(Arc::clone(&data));
    let mut second = AIUpdateInterface::new(data);
    // Explicit private fixture installation: this adapter has no production
    // installation yet. Do not claim these are admitted native world turrets.
    for (ai, angles) in [(&mut first, [0.25, 0.75]), (&mut second, [1.25, 1.75])] {
        for (index, angle) in angles.into_iter().enumerate() {
            let mut turret = TurretAI::new(INVALID_ID);
            turret.set_current_angle(angle);
            turret.set_natural_angle(angle);
            turret.set_current_pitch(angle / 2.0);
            ai.turret_ai[index] = Some(turret);
        }
    }
    first.set_turret_enabled(WhichTurretType::Main, false);
    first.set_turret_enabled(WhichTurretType::Invalid, false);
    assert!(!first.turret_ai[0].as_ref().unwrap().is_turret_enabled());
    assert!(first.turret_ai[1].as_ref().unwrap().is_turret_enabled());
    assert!(second.turret_ai[0].as_ref().unwrap().is_turret_enabled());
    for (ai, angles) in [(&first, [0.25, 0.75]), (&second, [1.25, 1.75])] {
        for (slot, angle) in [WhichTurretType::Main, WhichTurretType::Alt]
            .into_iter()
            .zip(angles)
        {
            assert_eq!(
                ai.get_turret_rot_and_pitch(slot),
                Some((angle, angle / 2.0))
            );
            assert!(ai.is_turret_in_natural_position(slot));
        }
    }
}
