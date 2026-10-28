use generals_main::game_logic::{GameLogic, Team};
use glam::Vec3;

fn world(setting: &str) -> (GameLogic, generals_main::game_logic::ObjectId) {
    let source = format!(
        "Object TurretContainProbe\n\
         KindOf = STRUCTURE SELECTABLE\n\
         Body = ActiveBody BodyTag\n MaxHealth = 100\n InitialHealth = 100\n End\n\
         Behavior = HealContain Cargo\n ContainMax = 4\n {setting}\n End\n End\n"
    );
    let mut world = GameLogic::new();
    world
        .admit_object_ini_sources([("turret_contain_probe.ini".into(), source)])
        .expect("admit receiving containment definition");
    let object = world
        .create_object("TurretContainProbe", Team::USA, Vec3::ZERO)
        .expect("create receiving container");
    (world, object)
}

#[test]
fn turret_containment_configuration_belongs_to_each_receiving_world() {
    let (enabled, enabled_id) = world("PassengersInTurret = Yes");
    let (disabled, disabled_id) = world("PassengersInTurret = No");
    let (defaulted, default_id) = world("");
    assert_eq!((enabled_id, disabled_id), (default_id, default_id));
    let field = |world: &GameLogic, id| {
        world
            .host_object(id)
            .unwrap()
            .thing()
            .template
            .contain_module
            .passengers_in_turret
    };
    assert!(field(&enabled, enabled_id));
    assert!(!field(&disabled, disabled_id));
    assert!(!field(&defaulted, default_id));
}
