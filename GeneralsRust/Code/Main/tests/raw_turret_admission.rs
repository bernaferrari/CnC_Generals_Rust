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
        let error = Object::try_new(template, ObjectId(700), Team::USA)
            .expect_err("invalid turret must not be constructed");
        assert!(error.to_string().contains("controlled weapon slots"));
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
