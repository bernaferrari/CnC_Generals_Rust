//! Actual borrowed producer, admitted definitions and live command effects.
use super::*;
use crate::command_system::SpecialPowerType;
use game_engine::common::ini::ini_command_button::CommandButton;
use gamelogic::scripting::HostScriptUseCommandButtonRequest as Request;

const LASER_POWER: &str = "SpecialAbilityMissileDefenderLaserGuidedMissiles";

fn fixture() -> (GameLogic, ObjectId, ObjectId, CommandButton, u32) {
    // SpecialPower.ini and Weapon.ini load with Object.ini at startup: the
    // Missile Defender's SpecialAbility names its laser power, its WeaponSet
    // names the laser gun.
    use crate::fixture_content::{FixtureObject, FixtureWeapon};
    let (mut world, unrelated, invalid) = super::named_command_test_support::world_with(
        &[
            FixtureObject::new("AmericaInfantryMissileDefender")
                .kind_of("INFANTRY")
                .health(100.0)
                .weapon("SECONDARY", "OwnedNearestLaserGun")
                .field("Locomotor = SET_NORMAL BasicHumanLocomotor")
                .module("Behavior = AIUpdateInterface ModuleTag_AI\nEnd")
                .module(format!(
                    "Behavior = SpecialAbility ModuleTag_Laser\n SpecialPowerTemplate = {LASER_POWER}\nEnd"
                )),
            FixtureObject::new("OwnedNearestVehicle")
                .kind_of("VEHICLE")
                .health(500.0),
        ],
        &[FixtureWeapon::from_stats(
            "OwnedNearestLaserGun",
            &crate::game_logic::host_missile_defender::missile_defender_laser_guided_weapon(),
        )],
        &format!(
            "SpecialPower {LASER_POWER}\n Enum = SPECIAL_MISSILE_DEFENDER_LASER_GUIDED_MISSILES\n ReloadTime = 0\nEnd\n"
        ),
    );
    world.override_world_size(2000.0, 2000.0);
    world
        .players
        .get_mut(&1)
        .unwrap()
        .set_map_relationship(2, gamelogic::common::Relationship::Enemies);
    world
        .players
        .get_mut(&2)
        .unwrap()
        .set_map_relationship(1, gamelogic::common::Relationship::Enemies);
    let source = world
        .create_object_for_player(
            "AmericaInfantryMissileDefender",
            1,
            Vec3::new(50.0, 0.0, 50.0),
        )
        .unwrap();
    let target = world
        .create_object_for_player("OwnedNearestVehicle", 2, Vec3::new(300.0, 0.0, 50.0))
        .unwrap();
    world.set_fixture_object_position(invalid, Vec3::new(51.0, 0.0, 50.0));
    let team = {
        let mut factory = world.team_factory.lock().unwrap();
        factory
            .init_team_owned(
                "NearestOwner".into(),
                "NamedOwner".into(),
                false,
                None,
                Some(1),
            )
            .unwrap();
        factory.create_team("NearestOwner").unwrap()
    };
    let team_id = {
        let mut team = team.write().unwrap();
        team.add_member(unrelated.0);
        team.add_member(source.0);
        team.set_active();
        team.get_id()
    };
    for id in [unrelated, source] {
        world.host_object_mut(id).unwrap().team_instance_name = "NearestOwner".into();
    }
    let mut button = CommandButton::new("OwnedNearestButton".into());
    button.command = "SPECIAL_POWER".into();
    button.special_power_template = Some(LASER_POWER.into());
    button.options_bits = gamelogic::object::update::special_power_update::SpecialPowerCommandOption::NEED_TARGET_ENEMY_OBJECT.bits();
    (world, source, target, button, team_id)
}

#[test]
fn nearest_authored_button_skips_invalid_nearer_targets_and_drives_only_its_owner() {
    let (mut owner, source, target, button, team_id) = fixture();
    let (peer, peer_source, _, _, peer_team) = fixture();
    assert_eq!(source, peer_source);
    assert_eq!(team_id, peer_team);
    let request = Request::TeamOnNearestEnemy {
        team: "NearestOwner".into(),
        button: button.name.clone(),
    };
    owner
        .apply_owned_team_nearest_button_definition(&request, &button, Some(team_id), None)
        .unwrap();
    assert_eq!(owner.host_object(source).unwrap().target, Some(target));
    assert_eq!(
        owner.host_object(source).unwrap().ai_state(),
        AIState::SpecialAbility
    );
    assert!(owner.hero_abilities.leftover_channel(source).is_some());
    assert!(peer.hero_abilities.leftover_channel(peer_source).is_none());
    assert_eq!(
        peer.host_object(peer_source).unwrap().ai_state(),
        AIState::Idle
    );
}

#[test]
fn nearest_authored_button_uses_first_source_map_status_and_ai_nonheld_center() {
    let (mut owner, source, target, button, _) = fixture();
    let unrelated = owner
        .objects
        .registered_values()
        .find(|object| object.id != source && object.owner_player_id == Some(1))
        .unwrap()
        .id;
    owner
        .host_object_mut(unrelated)
        .unwrap()
        .set_status_disabled_held(true);
    let members = [unrelated.0, source.0];
    let request = Request::TeamOnNearestKindof {
        team: "NearestOwner".into(),
        button: button.name.clone(),
        kindof: "VEHICLE".into(),
    };
    assert_eq!(
        owner
            .owned_team_nearest_button_target(&members, &button, &request, 1)
            .unwrap()
            .0,
        target
    );
    owner.set_fixture_object_position(target, Vec3::new(-1001.0, 0.0, 50.0));
    assert!(
        owner
            .owned_team_nearest_button_target(&members, &button, &request, 1)
            .is_none()
    );
    // Module source is selected before readiness; a future-ready first source
    // cannot silently be replaced with a later member's map status.
    owner.host_object_mut(source).unwrap()
        .set_special_power_ready_seconds(&SpecialPowerType::MissileDefenderLaserGuided, 9999.0);
    owner.set_fixture_object_position(source, Vec3::new(-1002.0, 0.0, 50.0));
    assert_eq!(
        owner
            .owned_team_nearest_button_target(&members, &button, &request, 1)
            .unwrap()
            .0,
        target
    );
    let mut missing = button.clone();
    missing.special_power_template = Some("MissingModule".into());
    assert!(
        owner
            .owned_team_nearest_button_target(&members, &missing, &request, 1)
            .is_none()
    );
}
