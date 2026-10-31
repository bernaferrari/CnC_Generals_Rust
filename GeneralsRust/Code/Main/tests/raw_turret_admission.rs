use generals_main::game_logic::{GameLogic, Object, ObjectId, Team, ThingTemplate};
use glam::Vec3;

#[test]
fn raw_object_admission_rejects_invalid_receiving_turrets() {
    for alternate in [false, true] {
        let turret = if alternate {
            "Turret\n ControlledWeaponSlots = PRIMARY\nEnd\nAltTurret\n TurretTurnRate = 30\nEnd\n"
        } else {
            "Turret\n TurretTurnRate = 30\nEnd\n"
        };
        let source = format!(
            "Object BadRaw\n KindOf = VEHICLE\n Body = ActiveBody Body\n MaxHealth = 100\n InitialHealth = 100\n End\n Behavior = AIUpdateInterface Brain\n {turret} End\nEnd\n"
        );
        let mut world = GameLogic::new();
        world
            .admit_object_ini_sources([("BadRaw.ini".into(), source)])
            .unwrap();
        assert!(
            world
                .create_object("BadRaw", Team::USA, Vec3::ZERO)
                .is_none()
        );
        let template = world.templates.get("BadRaw").unwrap().clone();
        let error = Object::try_new(template.clone(), ObjectId(700), Team::USA)
            .expect_err("invalid turret must not be constructed");
        assert!(error.to_string().contains("controlled weapon slots"));
        assert!(
            Object::try_new_with_logic_frame(template.clone(), ObjectId(700), Team::USA, 17)
                .is_err()
        );
        for construct in [
            Object::new as fn(ThingTemplate, ObjectId, Team) -> Object,
            Object::new_under_construction,
        ] {
            let invalid = template.clone();
            assert!(
                std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    construct(invalid, ObjectId(700), Team::USA)
                }))
                .is_err()
            );
        }
        assert_eq!(world.host_objects().registered_len(), 0);
    }
}

#[test]
fn programmatic_template_can_be_constructed_and_admitted() {
    let mut world = GameLogic::new();
    let object = Object::try_new(ThingTemplate::new("Programmatic"), ObjectId(701), Team::USA)
        .expect("programmatic template needs no authored AI data");
    assert_eq!(world.try_add_object(object).unwrap(), ObjectId(701));
}

#[test]
fn checked_constructor_uses_its_receiving_template_not_another_session() {
    fn session(slots: &str) -> GameLogic {
        let source = format!(
            "Object SharedRaw\n KindOf = VEHICLE\n Body = ActiveBody Body\n MaxHealth = 100\n InitialHealth = 100\n End\n Behavior = AIUpdateInterface Brain\n Turret\n TurretTurnRate = 30\n {slots}\n End\n End\nEnd\n"
        );
        let mut world = GameLogic::new();
        world
            .admit_object_ini_sources([("SharedRaw.ini".into(), source)])
            .unwrap();
        world
    }
    let invalid = session("");
    let valid = session("ControlledWeaponSlots = SECONDARY");
    let bad = invalid.templates.get("SharedRaw").unwrap().clone();
    let good = valid.templates.get("SharedRaw").unwrap().clone();
    let id = ObjectId(900);
    assert!(Object::try_new(bad, id, Team::USA).is_err());
    let object = Object::try_new(good, id, Team::USA).unwrap();
    assert!(object.is_weapon_slot_on_turret(1));
    assert_eq!(invalid.host_objects().registered_len(), 0);
    assert_eq!(valid.host_objects().registered_len(), 0);
}

#[test]
fn external_object_cannot_bypass_invalid_receiving_template_by_name() {
    let mut world = GameLogic::new();
    world.admit_object_ini_sources([("Bad.ini".into(), "Object NamedBad\n KindOf = VEHICLE\n Behavior = AIUpdateInterface Brain\n Turret\n TurretTurnRate = 30\n End\n End\nEnd\n".into())]).unwrap();
    let object = Object::try_new(ThingTemplate::new("NamedBad"), ObjectId(901), Team::USA).unwrap();
    assert!(
        world.try_add_object(object).is_err(),
        "an object with no receiving definition must not bypass an authored invalid template of the same name"
    );
    assert_eq!(world.host_objects().registered_len(), 0);
}
