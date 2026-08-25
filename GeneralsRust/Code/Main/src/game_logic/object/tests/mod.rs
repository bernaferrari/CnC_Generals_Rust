use super::*;

fn make_test_object() -> Object {
    let mut template = ThingTemplate::new("TestUnit");
    template.is_trainable = true;
    let mut object = Object::new(template, ObjectId(1), Team::USA);
    object.weapon = Some(Weapon {
        damage: 100.0,
        ..Weapon::default()
    });
    object
}

fn fx_list_die_from_behavior_attrs_for_test()
-> crate::game_logic::host_fx_list_die::HostFxListDieData {
    crate::game_logic::host_fx_list_die::fx_list_die_from_behavior_attrs(&[
        ("DeathFX", "FX_NormalDie"),
        ("ExemptStatus", "BURNED"),
    ])
}

fn make_ground_unit(name: &str, id: u32, kind: KindOf) -> Object {
    let mut t = ThingTemplate::new(name);
    t.add_kind_of(kind);
    let mut o = Object::new(t, ObjectId(id), Team::USA);
    o.set_orientation(0.0);
    o.set_position(glam::Vec3::ZERO);
    o
}

// Behavior-named suites keep each test file below the 4k LOC ceiling.
mod targeting_and_physics;
mod veterancy_and_combat_state;
