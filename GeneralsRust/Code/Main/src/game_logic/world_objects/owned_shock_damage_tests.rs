use super::*;
use crate::game_logic::combat::{DamageEvent, DamageType};
use crate::game_logic::host_usa_pilot::HostDeathType;
use glam::Vec3;

const VICTIM: ObjectId = ObjectId(901);
const SOURCE: ObjectId = ObjectId(902);

fn world(offset: Vec3) -> GameLogic {
    let mut world = GameLogic::new();
    let mut template = ThingTemplate::new("OwnedShockVictim");
    template.add_kind_of(KindOf::Vehicle).set_health(100.0);
    let mut victim = Object::new(template, VICTIM, Team::USA);
    victim.set_position(offset);
    victim.physics_mass = 1.0;
    world.objects.insert(VICTIM, victim);
    let mut source = Object::new(ThingTemplate::new("OwnedShockSource"), SOURCE, Team::China);
    source.set_position(Vec3::ZERO);
    world.objects.insert(SOURCE, source);
    world
}

fn blast(world: &mut GameLogic) {
    let event = DamageEvent::Area {
        position: Vec3::ZERO,
        damage: 10.0,
        damage_type: DamageType::Explosive,
        death_type: HostDeathType::Normal,
        radius: 2.0,
        shooter_id: SOURCE,
        secondary_damage: 0.0,
        secondary_radius: 0.0,
        shock_wave_amount: 40.0,
        shock_wave_radius: 2.0,
        shock_wave_taper_off: 0.5,
        radius_damage_affects: 0,
        shooter_team: Team::China,
        radius_source_present: true,
        radius_source_id: SOURCE,
        radius_source_producer: None,
        radius_source_team_id: None,
        radius_source_pose: None,
        shooter_owner_player_id: None,
        shooter_team_instance_name: String::new(),
        shooter_template: String::new(),
        primary_victim: Some(VICTIM),
        radius_weapon_name: String::new(),
        damage_status_type: None,
    };
    world.apply_owned_combat_damage_with_effects(
        &event,
        &mut crate::game_logic::ScriptPresentationEffects::default(),
    );
}

fn assert_velocity(world: &GameLogic, expected: Vec3) {
    let actual = world.objects[&VICTIM].movement.velocity;
    assert!((actual - expected).length() < 0.001, "expected {expected:?}, got {actual:?}");
}

#[test]
fn coincident_owned_area_tapers_unit_up_and_leaves_peer_world_untouched() {
    let mut owner = world(Vec3::ZERO);
    let peer = world(Vec3::ZERO);
    blast(&mut owner);
    assert_velocity(&owner, Vec3::new(0.0, 30.0, 0.0));
    assert_velocity(&peer, Vec3::ZERO);
    assert_eq!(peer.objects[&VICTIM].health.current, 100.0);
    assert!(owner.objects[&VICTIM].health.current < 100.0);
}

#[test]
fn owned_area_uses_strict_componentwise_epsilon_for_shock_direction() {
    // All components are below epsilon, although their combined length exceeds it.
    let mut inside = world(Vec3::splat(0.00009));
    blast(&mut inside);
    assert_velocity(&inside, Vec3::new(0.0027, 30.0, 0.0027));

    // Equality on either horizontal component is outside the guard; test both signs.
    for (offset, expected) in [
        (Vec3::new(0.0001, 0.0, 0.0), Vec3::new(39.999, 39.999, 0.0)),
        (Vec3::new(0.0, 0.0, -0.0001), Vec3::new(0.0, 39.999, -39.999)),
        (Vec3::new(0.0, -0.0001, 0.0), Vec3::new(0.0, 39.999, 0.0)),
    ] {
        let mut owner = world(offset);
        blast(&mut owner);
        assert_velocity(&owner, expected);
    }
}
