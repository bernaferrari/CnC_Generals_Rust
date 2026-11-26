//! Published object state from the receiving simulation.

use super::{
    PresentationBuildingType, PresentationFrame, PresentationObjectType, PresentationVeterancy,
    RenderableObject,
};
use crate::fixture_content::{FixtureObject, admit_fixture_objects};
use crate::fow_rendering::ObjectVisibility;
use crate::game_logic::{
    BuildingData, BuildingType, GameLogic, KindOf, ObjectAudio, ObjectId, Player, Team,
    ThingTemplate, VeterancyLevel,
};
use glam::Vec3;

fn world_with_player() -> GameLogic {
    let mut world = GameLogic::new();
    world.add_player(Player::new(0, Team::USA, "Local", true));
    world
}

fn spawn(world: &mut GameLogic, kinds: &str) -> ObjectId {
    admit_fixture_objects(
        world,
        &[FixtureObject::new("StatusFixture")
            .kind_of(kinds)
            .health(100.0)],
    );
    world
        .create_object_for_player("StatusFixture", 0, Vec3::ZERO)
        .unwrap()
}

fn publish(world: &mut GameLogic) -> PresentationFrame {
    PresentationFrame::try_build_from_logic(world, 0).expect("publish receiving world")
}

fn actor(frame: &PresentationFrame, id: ObjectId) -> &RenderableObject {
    frame
        .objects
        .iter()
        .find(|row| row.id == id)
        .expect("published actor")
}

#[test]
fn selection_and_construction_progress_freeze_with_the_receiving_body() {
    let mut world = world_with_player();
    let id = spawn(&mut world, "STRUCTURE SELECTABLE");
    assert!(!actor(&publish(&mut world), id).selected);

    let object = world.host_object_mut(id).unwrap();
    object.health.maximum = 150.0;
    object.construction_percent = 0.4;
    object.status.under_construction = true;
    world.select_objects_no_sound(0, vec![id]);
    assert!(world.host_object(id).unwrap().status.selected);

    let selected = publish(&mut world);
    let row = actor(&selected, id);
    assert!(row.selected && row.under_construction);
    assert!(!row.destroyed);
    assert_eq!(row.health_max, 150.0);
    assert_eq!(row.construction_percent, 0.4);

    world.select_objects_no_sound(0, vec![]);
    assert!(!actor(&publish(&mut world), id).selected);
    assert!(actor(&selected, id).selected);
}

#[test]
fn combat_flags_publish_and_clear_without_rewriting_the_previous_frame() {
    let mut world = world_with_player();
    let id = spawn(&mut world, "VEHICLE SELECTABLE");
    let object = world.host_object_mut(id).unwrap();
    object.set_status_stealthed(true);
    object.set_status_detected(false);
    object.set_status_attacking(true);
    object.set_status_firing_weapon(true);
    object.set_status_masked(true);
    object.set_status_disguised(true);
    object.set_force_attack(true);
    object.set_status_using_ability(true);
    object.set_status_deployed(true);
    object.set_status_no_collisions(true);
    object.set_status_private_captured(true);
    object.set_status_parachuting(true);
    object.status.airborne_target = true;
    object.status.disabled_underpowered = true;
    object.status.disabled_hacked = true;
    object.stored_resources.supplies = 1500;

    assert!(object.status.no_collisions);

    let active = publish(&mut world);
    let row = actor(&active, id);
    assert!(row.stealthed && !row.detected);
    assert!(row.attacking && row.is_firing_weapon && row.masked && row.disguised);
    assert!(row.force_attack && row.using_ability && row.is_deployed);
    assert!(row.airborne_target && row.captured && row.parachuting);
    assert!(row.disabled_underpowered && row.disabled_hacked);
    assert!(!row.disabled_emp);
    assert!(!row.disabled_unmanned);
    assert_eq!(row.stored_supplies, 1500);

    let (object, health) = world.host_object_and_health_events_mut(id).unwrap();
    object.apply_disabled_emp(300, health, &mut ObjectAudio::record(&mut Vec::new()));
    assert_eq!(object.status.disabled_emp_until_frame, 300);
    let disabled = publish(&mut world);
    let row = actor(&disabled, id);
    assert!(row.disabled_emp);
    assert!(!row.attacking && !row.force_attack);

    let object = world.host_object_mut(id).unwrap();
    object.set_status_stealthed(false);
    object.set_status_detected(true);
    object.set_status_attacking(false);
    object.set_status_firing_weapon(false);
    object.set_force_attack(false);
    let row = actor(&active, id);
    assert!(
        row.stealthed && !row.detected && row.attacking && row.is_firing_weapon && row.force_attack
    );
    let cleared = publish(&mut world);
    let row = actor(&cleared, id);
    assert!(!row.stealthed && row.detected);
    assert!(!row.attacking && !row.is_firing_weapon && !row.force_attack);
}

#[test]
fn command_set_and_geometry_follow_the_receiving_vehicle() {
    fn restore_flat_heightmap(world: &GameLogic, sample: u8) {
        world
            .world_services
            .terrain()
            .write()
            .unwrap()
            .try_restore_logic_height_map_with_effects(8, 8, &[sample; 64], &mut || {})
            .unwrap();
    }

    let mut world = world_with_player();
    let id = spawn(&mut world, "VEHICLE SELECTABLE");
    let object = world.host_object_mut(id).unwrap();
    object.set_command_set_override(Some("Command_DemoSuicide".into()));
    object.set_selection_radius(14.5);
    object.set_ground_height_residual(12.5, true);
    object.crusher_level = 2;
    object.crushable_level = 1;
    object.vision_range = 175.0;
    object.shroud_clearing_range = 200.0;
    assert_eq!(
        (object.ground_height, object.ground_height_from_terrain),
        (12.5, true)
    );
    restore_flat_heightmap(&world, 20);

    let modified = publish(&mut world);
    let row = actor(&modified, id);
    assert_eq!(row.command_set_override, "Command_DemoSuicide");
    assert_eq!(row.selection_radius, 14.5);
    assert_eq!(
        (row.ground_height, row.ground_height_from_terrain),
        (12.5, true)
    );
    assert_eq!((row.crusher_level, row.crushable_level), (2, 1));
    assert_eq!(
        (row.vision_range, row.shroud_clearing_range),
        (175.0, 200.0)
    );

    let object = world.host_object_mut(id).unwrap();
    object.set_command_set_override(None);
    object.set_selection_radius(2.0);
    object.set_ground_height_residual(6.0, true);
    assert_eq!(object.ground_height, 6.0);
    restore_flat_heightmap(&world, 16);
    let updated = publish(&mut world);
    let row = actor(&updated, id);
    assert!(row.command_set_override.is_empty());
    assert_eq!(row.selection_radius, 5.0);
    assert_eq!(
        (row.ground_height, row.ground_height_from_terrain),
        (10.0, true)
    );
    assert_eq!(actor(&modified, id).selection_radius, 14.5);
    assert_eq!(actor(&modified, id).ground_height, 12.5);
}

#[test]
fn authored_hero_model_and_scale_publish_with_the_units_kind() {
    let mut world = world_with_player();
    admit_fixture_objects(
        &mut world,
        &[FixtureObject::new("PublishedRanger")
            .kind_of("INFANTRY SELECTABLE HERO")
            .health(100.0)
            .walker()
            .field("Scale = 1.25")
            .model("avtank")],
    );
    let id = world
        .create_object_for_player("PublishedRanger", 0, Vec3::ZERO)
        .unwrap();
    let object = world.host_object(id).unwrap();
    assert_ne!(object.presentation_kind_of_bits() & (1 << 10), 0);

    let frame = publish(&mut world);
    let row = actor(&frame, id);
    assert_eq!(row.object_type, PresentationObjectType::Infantry);
    assert!(row.kind_of.contains(&KindOf::Hero));
    assert!(row.kind_of.contains(&KindOf::Selectable));
    assert!(row.is_unit && row.is_mobile);
    assert_eq!(row.model_key.as_deref(), Some("avtank"));
    assert_eq!(row.mesh_scale, 1.25);
}

#[test]
fn unowned_building_publishes_its_type_name_color_and_power() {
    let mut world = GameLogic::new();
    admit_fixture_objects(
        &mut world,
        &[FixtureObject::new("PublishedBuilding")
            .kind_of("STRUCTURE SELECTABLE")
            .health(200.0)],
    );
    let id = world
        .create_object("PublishedBuilding", Team::China, Vec3::ZERO)
        .unwrap();
    let object = world.host_object_mut(id).unwrap();
    assert!(object.owner_player_id.is_none());
    object.object_type = crate::game_logic::ObjectType::Building;
    object.building_data = Some(BuildingData::new(BuildingType::Barracks));
    object.name = "ScriptBarracks".into();
    object.team_color = [0.1, 0.2, 0.3, 0.9];
    object.set_entity_power(50, 5);

    let initial = publish(&mut world);
    let row = actor(&initial, id);
    assert_eq!(row.team, Team::China);
    assert_eq!(row.object_type, PresentationObjectType::Building);
    assert_eq!(row.building_type, Some(PresentationBuildingType::Barracks));
    assert!(row.is_structure && !row.is_unit);
    assert_eq!(row.display_name, "ScriptBarracks");
    assert_eq!(row.team_color, [0.1, 0.2, 0.3, 0.9]);
    assert_eq!((row.power_provided, row.power_consumed), (50, 5));

    world.host_object_mut(id).unwrap().set_entity_power(15, 7);
    let updated = publish(&mut world);
    let row = actor(&updated, id);
    assert_eq!((row.power_provided, row.power_consumed), (15, 7));
    assert_eq!(actor(&initial, id).power_provided, 50);
}

#[test]
fn same_faction_fog_uses_the_receiving_viewer_and_exact_controller() {
    use gamelogic::common::ObjectShroudStatus;

    let mut world = world_with_player();
    world.add_player(Player::new(1, Team::USA, "Other", false));
    let id = spawn(&mut world, "INFANTRY SELECTABLE");
    assert!(world.transfer_object_to_player(id, 1).unwrap());
    {
        let handle = world.world_services.shroud();
        let mut shroud = handle.lock().unwrap();
        shroud.mark_host_object_explored(0, id.0);
        shroud.mark_host_vision_ready();
    }

    let fogged = publish(&mut world);
    let owner = PresentationFrame::try_build_from_logic(&mut world, 1).unwrap();
    assert_eq!(actor(&fogged, id).owner_player_id, Some(1));
    assert_eq!(actor(&fogged, id).fow_visibility, ObjectVisibility::FOGGED);
    assert_eq!(
        actor(&owner, id).fow_visibility,
        ObjectVisibility::FULLY_VISIBLE
    );

    world
        .world_services
        .shroud()
        .lock()
        .unwrap()
        .set_host_object_shroud_status(0, id.0, ObjectShroudStatus::Clear);
    assert_eq!(
        actor(&publish(&mut world), id).fow_visibility,
        ObjectVisibility::FULLY_VISIBLE
    );
    assert_eq!(actor(&fogged, id).fow_visibility, ObjectVisibility::FOGGED);
}

#[test]
fn model_conditions_use_current_movement_and_attack_flags() {
    use crate::game_logic::host_enum_table_residual::{MC_BIT_ATTACKING, MC_BIT_MOVING};

    let mut world = world_with_player();
    let id = spawn(&mut world, "INFANTRY SELECTABLE");
    let moving = 1u128 << MC_BIT_MOVING;
    let attacking = 1u128 << MC_BIT_ATTACKING;
    let object = world.host_object_mut(id).unwrap();
    object.status.moving = true;
    object.set_status_attacking(true);

    let prior = publish(&mut world);
    let row = actor(&prior, id);
    assert!(row.moving && row.attacking);
    assert_eq!(
        row.model_condition_bits & (moving | attacking),
        moving | attacking
    );

    let object = world.host_object_mut(id).unwrap();
    object.status.moving = false;
    object.set_status_attacking(false);
    assert_eq!(
        actor(&publish(&mut world), id).model_condition_bits & (moving | attacking),
        0
    );
    assert_eq!(
        actor(&prior, id).model_condition_bits & (moving | attacking),
        moving | attacking
    );
}

#[test]
fn mine_and_cheer_presentation_keeps_the_object_values() {
    use crate::game_logic::host_mines::{HostMineData, HostMineKind};

    let mut world = world_with_player();
    let id = spawn(&mut world, "INFANTRY SELECTABLE");
    let object = world.host_object_mut(id).unwrap();
    object.demo_suicided_detonating = true;
    object.cheer_timer = 2.5;
    object.mine_data = Some(HostMineData::new(HostMineKind::LandMine));

    let prior = publish(&mut world);
    let row = actor(&prior, id);
    assert!(row.demo_suicided_detonating && row.has_mine);
    assert_eq!(row.cheer_timer, 2.5);

    let object = world.host_object_mut(id).unwrap();
    object.demo_suicided_detonating = false;
    object.cheer_timer = 0.0;
    object.mine_data = None;
    let after = publish(&mut world);
    let row = actor(&after, id);
    assert!(!row.demo_suicided_detonating && !row.has_mine);
    assert_eq!(row.cheer_timer, 0.0);
    assert!(actor(&prior, id).has_mine);
}

#[test]
fn camouflage_and_disguise_use_the_receiving_actor() {
    let mut world = world_with_player();
    let id = spawn(&mut world, "MINE SELECTABLE");
    let object = world.host_object_mut(id).unwrap();
    object.vision_spied_mask = 0b101;
    object.camo_friendly_opacity = 0.35;
    object.camo_stealth_look = 2;
    object.disguise_as_template = Some("AmericaVehicleHumvee".into());
    object.disguise_as_team = Some(Team::USA);
    object.innate_stealth = true;
    object.is_tunnel_network = true;
    object.passengers_allowed_to_fire = true;

    let camouflaged = publish(&mut world);
    let row = actor(&camouflaged, id);
    assert_eq!(row.vision_spied_mask, 0b101);
    assert_eq!(
        (row.friendly_stealth_opacity, row.camo_stealth_look),
        (0.35, 2)
    );
    assert_eq!(
        row.disguise_as_template.as_deref(),
        Some("AmericaVehicleHumvee")
    );
    assert_eq!(row.disguise_as_team, Some(Team::USA));
    assert!(row.innate_stealth && row.is_tunnel_network && row.passengers_allowed_to_fire);

    let object = world.host_object_mut(id).unwrap();
    object.disguise_as_template = None;
    object.disguise_as_team = None;
    assert!(
        actor(&publish(&mut world), id)
            .disguise_as_template
            .is_none()
    );
    assert_eq!(actor(&camouflaged, id).disguise_as_team, Some(Team::USA));
}

#[test]
fn faerie_fire_keeps_the_later_deadline_until_cleared() {
    let mut world = world_with_player();
    let id = spawn(&mut world, "INFANTRY SELECTABLE");
    world.host_object_mut(id).unwrap().apply_faerie_fire(1234);
    let initial = publish(&mut world);
    assert_eq!(actor(&initial, id).faerie_fire_until_frame, 1234);

    let object = world.host_object_mut(id).unwrap();
    object.apply_faerie_fire(99);
    assert_eq!(object.faerie_fire_until_frame, 1234);
    assert!(object.is_faerie_fire());
    object.clear_faerie_fire();
    object.apply_faerie_fire(99);
    assert_eq!(actor(&publish(&mut world), id).faerie_fire_until_frame, 99);
    assert_eq!(actor(&initial, id).faerie_fire_until_frame, 1234);
}

#[test]
fn experience_and_veterancy_publish_from_trainable_objects() {
    let mut world = world_with_player();
    admit_fixture_objects(
        &mut world,
        &[FixtureObject::new("TrainableRanger")
            .kind_of("INFANTRY SELECTABLE")
            .health(100.0)
            .field("ExperienceRequired = 0 10 20 30")
            .field("IsTrainable = Yes")],
    );
    let id = world
        .create_object_for_player("TrainableRanger", 0, Vec3::ZERO)
        .unwrap();
    world.award_experience(id, 25.0);
    assert_eq!(
        world.host_object(id).unwrap().experience.level,
        VeterancyLevel::Elite
    );
    let elite = publish(&mut world);
    let row = actor(&elite, id);
    assert_eq!(
        (row.experience_points, row.veterancy),
        (25.0, PresentationVeterancy::Elite)
    );
    world.award_experience(id, 5.0);
    assert_eq!(
        world.host_object(id).unwrap().experience.level,
        VeterancyLevel::Heroic
    );
    let latest = publish(&mut world);
    let row = actor(&latest, id);
    assert_eq!(
        (row.experience_points, row.veterancy),
        (30.0, PresentationVeterancy::Heroic)
    );
    assert_eq!(actor(&elite, id).experience_points, 25.0);
}

#[test]
fn isolated_experience_writes_and_restore_keep_frozen_frames() {
    fn trainable_world() -> (GameLogic, ObjectId) {
        let mut world = GameLogic::new();
        let mut template = ThingTemplate::new("IndependentExperience");
        template.set_health(100.0);
        template.add_kind_of(KindOf::Selectable);
        template.is_trainable = true;
        template.veterancy_xp_thresholds = [1000.0, 2000.0, 3000.0];
        world.templates.insert(template.name.clone(), template);
        let id = world
            .create_object("IndependentExperience", Team::USA, Vec3::ZERO)
            .unwrap();
        (world, id)
    }

    let (mut world, id) = trainable_world();
    let (mut peer, peer_id) = trainable_world();
    assert_eq!(id, peer_id);
    let initial = publish(&mut world);
    world.award_experience(id, 42.0);
    assert_eq!(world.host_object(id).unwrap().experience.current, 42.0);
    peer.award_experience(peer_id, 7.0);
    let before_restore = publish(&mut world);
    let foreign = publish(&mut peer);
    assert_eq!(actor(&before_restore, id).experience_points, 42.0);
    assert_eq!(actor(&foreign, id).experience_points, 7.0);
    world.award_experience(id, 1.0);
    assert_eq!(world.host_object(id).unwrap().experience.current, 43.0);

    let builder = crate::save_load::SnapshotBuilder::new();
    let snapshot = builder.create_world_snapshot(&world).unwrap();
    let mut restored = GameLogic::new();
    restored.templates = world.templates.clone();
    builder
        .restore_from_snapshot(&snapshot, &mut restored)
        .unwrap();
    assert_eq!(actor(&publish(&mut restored), id).experience_points, 43.0);
    world.award_experience(id, 1.0);
    restored.award_experience(id, 1.0);
    peer.reset();
    drop(peer);
    assert_eq!(actor(&publish(&mut world), id).experience_points, 44.0);
    assert_eq!(actor(&publish(&mut restored), id).experience_points, 44.0);
    assert_eq!(actor(&initial, id).experience_points, 0.0);
    assert_eq!(actor(&before_restore, id).experience_points, 42.0);
    assert_eq!(actor(&foreign, id).experience_points, 7.0);
}
