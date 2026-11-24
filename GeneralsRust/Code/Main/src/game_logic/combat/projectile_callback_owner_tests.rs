//! CPP GameLogic.cpp3698-3736: a callback finishes before the next update.
//! No claim about original heap order: roles follow the existing arena encounter.
use super::*;
use crate::game_logic::game_logic::PlayerSideRole;
use crate::game_logic::weapon_bootstrap::{
    HostMissileFlight, HostProjectileFlight, HostProjectileLifecycle,
};

fn configured_world() -> GameLogic {
    let mut world = GameLogic::new();
    world.frame = 100;
    let mut neutral = Player::new(0, Team::Neutral, "Neutral", false);
    neutral.map_side.role = PlayerSideRole::Neutral;
    neutral.map_side.map_player_name.clear();
    world.add_player(neutral);
    world.add_player(Player::new(1, Team::USA, "Owner", true));
    let mut source = ThingTemplate::new("CallbackSource");
    source.set_health(100.0).add_kind_of(KindOf::Infantry);
    let mut blocker = ThingTemplate::new("CallbackBlocker");
    blocker
        .set_health(100.0)
        .add_kind_of(KindOf::Vehicle)
        .add_kind_of(KindOf::Structure);
    blocker.geometry_info = crate::game_logic::HostGeometryInfo {
        geom_type: crate::game_logic::HostGeometryType::Box,
        height: 10.0,
        major_radius: 8.0,
        minor_radius: 8.0,
        authored: true,
        ..Default::default()
    };
    world.templates.insert(source.name.clone(), source);
    world.templates.insert(blocker.name.clone(), blocker);
    world
}

fn world() -> (GameLogic, ObjectId, ObjectId) {
    let mut world = configured_world();
    let source = world
        .create_object_for_player("CallbackSource", 1, Vec3::new(50.0, 0.0, 0.0))
        .unwrap();
    let blocker = world
        .create_object_for_player("CallbackBlocker", 1, Vec3::ZERO)
        .unwrap();
    (world, source, blocker)
}

fn shell(id: ObjectId, source: ObjectId) -> Projectile {
    let mut shell = Projectile::new(
        id,
        Vec3::ZERO,
        Vec3::new(1000.0, 0.0, 0.0),
        20.0,
        DamageType::Unresistable,
        source,
        None,
    );
    // Keep this controller-order witness independent of armor selection.
    // CPP UNRESISTABLE applies the specified amount without damage scalar.
    shell.speed = 30.0;
    shell.velocity = Vec3::new(30.0, 0.0, 0.0);
    shell.source_team = Team::USA;
    shell.source_owner_player_id = Some(1);
    shell.projectile_collides = crate::game_logic::weapon_bootstrap::PROJECTILE_COLLIDE_STRUCTURES;
    shell
}

const DETONATION_CHILD: &str = "DetonationBasisChild";

fn world_with_owned_detonation_ocl() -> (GameLogic, ObjectId, glam::Mat4) {
    const LIST: &str = "OwnedProjectileDetonation";
    let mut world = configured_world();
    world.add_player(Player::new(2, Team::China, "ProjectileController", false));
    world.templates.insert(DETONATION_CHILD.into(), ThingTemplate::new(DETONATION_CHILD));
    world.admit_object_creation_list_sources([format!(
        "ObjectCreationList {LIST}\nCreateObject\nObjectNames = {DETONATION_CHILD}\nCount = 1\nDisposition = LIKE_EXISTING\nOffset = X:1 Y:2 Z:3\nEnd\nEnd"
    )]).unwrap();
    let shooter = world.create_object_for_player(
        "CallbackSource", 1, Vec3::new(50.0, 0.0, 0.0),
    ).unwrap();
    let position = Vec3::new(20.0, 4.0, 30.0);
    let id = world.create_object_for_player("CallbackSource", 2, position).unwrap();
    let pose = glam::Mat4::from_cols(
        Vec3::Y.extend(0.0),
        Vec3::Z.extend(0.0),
        Vec3::X.extend(0.0),
        position.extend(1.0),
    );
    assert!(world.set_registered_object_transform(id, pose));

    let mut projectile = shell(id, shooter);
    projectile.position = position;
    projectile.velocity = Vec3::ZERO;
    projectile.speed = 0.0;
    projectile.projectile_actor_birth = Some((
        world.objects[&id].visual_object_generation,
        world.visual_world_epoch,
    ));
    projectile.projectile_lifecycle = Some(HostProjectileLifecycle::DumbProjectile {
        max_lifespan_frames: 1,
        detonate_calls_kill: false,
    });
    projectile.detonation_ocl_name = LIST.into();
    world.combat_system.projectiles.insert(id, projectile);
    (world, id, pose)
}

#[test]
fn owned_impact_ocl_inherits_the_projectile_owner_and_basis_before_retirement() {
    let (mut world, id, pose) = world_with_owned_detonation_ocl();
    assert_eq!(world.update_owned_projectile_impacts(1.0 / 30.0).unwrap(), vec![id]);
    assert!(!world.combat_system.get_projectiles().contains_key(&id));

    let mut expected = pose;
    expected.w_axis = pose.transform_point3(Vec3::new(1.0, 3.0, 2.0)).extend(1.0);
    {
        let mut children = world.host_objects().registered_values()
            .filter(|o| o.template_name == DETONATION_CHILD);
        let child = children.next().expect("the terminal weapon creates an object");
        assert!(children.next().is_none());
        assert_eq!(child.producer_id, Some(id));
        assert_eq!(child.owner_player_id, Some(2));
        assert_eq!(child.get_transform_matrix(), expected);
    }
    world.process_destroy_list();
    assert!(world.host_object(id).is_none());
}

#[test]
fn owned_impact_ocl_cannot_use_a_replacement_with_the_same_id() {
    let (mut world, id, pose) = world_with_owned_detonation_ocl();
    let (_, retired, _) = world.combat_system.prepare_projectile_impact_with_runtime(
        id,
        ProjectileCallback::Update(1.0 / 30.0),
        &mut world.objects,
        None,
        world.frame,
        None,
        None,
        &mut world.ownership_events,
        0,
        None,
        &mut (),
    );
    assert_eq!(retired, vec![id]);
    let original = world.objects[&id].clone();
    let generation = original.visual_object_generation;
    world.destroy_object(id).unwrap();
    world.process_destroy_list();
    world.add_object(original);
    assert_ne!(world.objects[&id].visual_object_generation, generation);

    world.flush_projectile_impact_fx().unwrap();
    assert_eq!(world.objects[&id].get_transform_matrix(), pose);
    assert!(!world.host_objects().registered_values()
        .any(|o| o.template_name == DETONATION_CHILD));
}

fn ranked_detonation_world(marker: &str) -> (GameLogic, ObjectId, ObjectId) {
    use crate::assets::ini_template_loader::{
        WeaponDefinitionLoadType, parse_weapon_definitions_ini_into,
    };
    let mut world = configured_world();
    world.templates.insert(marker.into(), ThingTemplate::new(marker));
    world.admit_object_creation_list_sources([format!(
        "ObjectCreationList OCL_{marker}\nCreateObject\nObjectNames = {marker}\nCount = 1\nEnd\nEnd"
    )]).unwrap();
    let mut rows = std::collections::HashMap::new();
    assert_eq!(parse_weapon_definitions_ini_into(
        &mut rows,
        &format!("Weapon RankedImpact\n VeterancyProjectileDetonationOCL = VETERAN OCL_{marker}\n VeterancyProjectileDetonationFX = VETERAN FX_RankedImpact\n VeterancyProjectileDetonationOCL = ELITE None\n VeterancyProjectileDetonationFX = ELITE None\nEnd\n"),
        WeaponDefinitionLoadType::Normal,
    ), 1);
    world.admit_weapon_content_snapshot(rows.into_iter().collect(), &[]).unwrap();
    #[cfg(feature = "game_client")]
    world.admit_fx_source("FXList FX_RankedImpact\n ViewShake\n Type = NORMAL\n End\nEnd").unwrap();

    let position = Vec3::new(20.0, 4.0, 30.0);
    world.session_view.target = position;
    let launcher = world.create_object_for_player(
        "CallbackSource", 1, Vec3::new(50.0, 0.0, 0.0),
    ).unwrap();
    let id = world.create_object_for_player("CallbackSource", 1, position).unwrap();
    world.objects.get_mut(&id).unwrap().experience.level = VeterancyLevel::Veteran;
    let mut projectile = shell(id, launcher);
    projectile.historic_weapon_key = "RankedImpact".into();
    projectile.projectile_object_name = "CallbackSource".into();
    projectile.position = position;
    projectile.flight = Some(HostProjectileFlight::Dumb(Default::default()));
    projectile.projectile_actor_birth = Some((
        world.objects[&id].visual_object_generation, world.visual_world_epoch,
    ));
    projectile.projectile_lifecycle = Some(HostProjectileLifecycle::DumbProjectile {
        max_lifespan_frames: 1,
        detonate_calls_kill: false,
    });
    // Both launch-time names are empty. C++ only reads the weapon slot after
    // the projectile reaches the detonation callback.
    world.combat_system.projectiles.insert(id, projectile);
    (world, launcher, id)
}

#[cfg(feature = "game_client")]
fn reveal_ranked_impact(world: &GameLogic) {
    let player = world.local_player_id().expect("the fixture has a local player");
    let mut shroud = world.world_services.shroud().lock().unwrap();
    shroud.init_shroud_grid(256.0, 256.0);
    shroud.reveal_map_for_player_permanently(player).unwrap();
}

#[test]
fn detonation_effects_use_projectile_veterancy_and_receiving_weapon_after_restore() {
    let (mut world, launcher, id) = ranked_detonation_world("FirstRankedChild");
    let (mut peer, peer_launcher, peer_id) = ranked_detonation_world("SecondRankedChild");
    assert_eq!((launcher, id), (peer_launcher, peer_id));
    assert_eq!(world.objects[&launcher].experience.level, VeterancyLevel::Rookie);
    let builder = crate::save_load::SnapshotBuilder::new();
    let saved = builder.create_world_snapshot(&world).unwrap();
    let bytes = bincode_legacy::serialize(&saved).unwrap();
    let saved = crate::save_load::snapshot::decode_bincode_world_snapshot(&bytes).unwrap();
    let (mut restored, _, _) = ranked_detonation_world("FirstRankedChild");
    builder.restore_from_snapshot(&saved, &mut restored).unwrap();

    for (world, marker) in [
        (&mut peer, "SecondRankedChild"),
        (&mut world, "FirstRankedChild"),
        (&mut restored, "FirstRankedChild"),
    ] {
        #[cfg(feature = "game_client")]
        reveal_ranked_impact(world);
        assert_eq!(world.objects[&id].experience.level, VeterancyLevel::Veteran);
        assert_eq!(world.update_owned_projectile_impacts(1.0 / 30.0).unwrap(), vec![id]);
        let child = world.host_objects().registered_values()
            .find(|object| object.template_name == marker)
            .expect("the receiving definition supplies the projectile's Veteran list");
        assert_eq!(child.producer_id, Some(id));
        #[cfg(feature = "game_client")]
        assert!(world.session_view.impulse.intensity > 0.0, "the Veteran FX must also run");
    }
}

#[test]
fn detonation_empty_projectile_rank_does_not_use_frozen_launch_effects() {
    let (mut world, _, id) = ranked_detonation_world("ForbiddenRankedChild");
    #[cfg(feature = "game_client")]
    reveal_ranked_impact(&world);
    world.objects.get_mut(&id).unwrap().experience.level = VeterancyLevel::Elite;
    let shot = world.combat_system.projectiles.get_mut(&id).unwrap();
    shot.source_veterancy = VeterancyLevel::Veteran;
    shot.detonation_ocl_name = "OCL_ForbiddenRankedChild".into();
    shot.detonation_fx_name = "FX_RankedImpact".into();
    assert_eq!(world.update_owned_projectile_impacts(1.0 / 30.0).unwrap(), vec![id]);
    assert!(!world.host_objects().registered_values()
        .any(|object| object.template_name == "ForbiddenRankedChild"));
    #[cfg(feature = "game_client")]
    assert_eq!(world.session_view.impulse.intensity, 0.0);
}

#[test]
fn impact_callback_error_cannot_replay_retired_shells() {
    use gamelogic::effects::ObjectCreationList;
    use gamelogic::object_creation_list::GenericObjectCreationNugget;
    use std::sync::Arc;

    const IMPACT_OCL: &str = "TestImpactSequence";
    const FIRST_CHILD: &str = "TestImpactChild";
    const REJECTED_CHILD: &str = "TestInvalidImpactChild";
    let (mut owner, source, target) = world();
    let mut list = ObjectCreationList::new();
    for name in [FIRST_CHILD, REJECTED_CHILD] {
        owner.templates.insert(name.into(), ThingTemplate::new(name));
        list.add_nugget(Arc::new(GenericObjectCreationNugget {
            names: vec![name.into()],
            ..Default::default()
        }));
    }
    owner
        .templates
        .get_mut(REJECTED_CHILD)
        .unwrap()
        .body_definition_error = Some("injected impact OCL factory failure".into());
    owner.admit_weapon_ocl_definition(IMPACT_OCL, list).unwrap();
    for id in [ObjectId(9001), ObjectId(9002)] {
        let mut projectile = shell(id, source);
        projectile.target_position = Vec3::ZERO;
        projectile.target_id = Some(target);
        owner.combat_system.projectiles.insert(id, projectile);
    }
    let order = owner.combat_system.projectile_update_ids();
    owner.combat_system.projectiles.get_mut(&order[1]).unwrap().detonation_ocl_name =
        IMPACT_OCL.into();

    let count_children = |world: &GameLogic, name: &str| {
        world.host_objects().registered_values().filter(|o| o.template_name == name).count()
    };
    let error = owner.update_owned_projectile_impacts(0.0).unwrap_err();
    assert!(error.to_string().contains("injected impact OCL factory failure"));
    assert_eq!(count_children(&owner, FIRST_CHILD), 1);
    assert_eq!(count_children(&owner, REJECTED_CHILD), 0);
    assert_eq!(owner.host_object(target).unwrap().health.current, 80.0);
    assert_eq!(owner.combat_system.projectile_count(), 0, "both impacts already started");

    owner.templates.get_mut(REJECTED_CHILD).unwrap().body_definition_error = None;
    assert!(owner.update_owned_projectile_impacts(0.0).unwrap().is_empty());
    assert_eq!(count_children(&owner, FIRST_CHILD), 1);
    assert_eq!(count_children(&owner, REJECTED_CHILD), 0);
    assert_eq!(owner.host_object(target).unwrap().health.current, 80.0);
}

#[test]
fn first_body_callback_neutralizes_controller_before_next_incidental_collision() {
    let (mut owner, source, blocker) = world();
    let (peer, peer_source, peer_blocker) = world();
    assert_eq!((source, blocker), (peer_source, peer_blocker));
    for id in [ObjectId(9001), ObjectId(9002)] {
        owner
            .combat_system
            .projectiles
            .insert(id, shell(id, source));
    }
    // Preserve HashMap's existing encounter, without pretending this is C++
    // equal-priority module order. Mutating values does not change that order.
    let order: Vec<_> = owner.combat_system.projectiles.keys().copied().collect();
    let first = owner.combat_system.projectiles.get_mut(&order[0]).unwrap();
    first.target_id = Some(blocker);
    first.target_position = Vec3::ZERO;
    first.damage = 1.0;
    first.damage_type = DamageType::KillPilot;
    let retired = owner.update_owned_projectile_impacts(0.0).unwrap();
    assert_eq!(owner.host_object(blocker).unwrap().owner_player_id, Some(0));
    assert_eq!(
        owner.host_object(blocker).unwrap().health.current,
        80.0,
        "second STRUCTURES-only shell must observe first callback's neutral controller"
    );
    assert_eq!(retired, order);
    assert_eq!(owner.combat_system.projectile_count(), 0);
    assert_eq!(
        peer.host_object(peer_blocker).unwrap().owner_player_id,
        Some(1)
    );
    assert_eq!(
        peer.host_object(peer_blocker).unwrap().health.current,
        100.0
    );
}

#[test]
fn registered_dead_target_is_not_missile_target_disappearance() {
    let (mut owner, source, target) = world();
    let (target_object, fixture_partition_target_object) =
        owner.host_object_and_partition_mut(target).unwrap();
    target_object.set_position_with_partition_owner(
        Vec3::new(1000.0, 0.0, 0.0),
        0,
        &mut Default::default(),
        fixture_partition_target_object,
    );
    target_object.health.current = 0.0;
    assert!(!target_object.is_alive());
    let id = ObjectId(9001);
    let mut missile = shell(id, source);
    missile.target_id = Some(target);
    missile.target_position = Vec3::new(1234.0, 0.0, 0.0);
    missile.is_homing = true;
    missile.flight = Some(HostProjectileFlight::Missile(HostMissileFlight::default()));
    missile.projectile_lifecycle = Some(HostProjectileLifecycle::Missile {
        try_to_follow_target: true,
        fuel_lifetime_frames: 0,
        detonate_on_no_fuel: false,
        kill_self_delay_frames: 30,
    });
    owner.combat_system.projectiles.insert(id, missile);
    assert!(owner.update_owned_projectile_impacts(1.0 / 30.0).unwrap().is_empty());
    let missile = &owner.combat_system.projectiles[&id];
    assert_eq!(
        missile.missile_kill_self_started_frame, None,
        "CPP getGoalObject returns a registered pointer, not an isAlive filter"
    );
    assert_eq!(missile.target_position.x, 1000.0);
    assert!(missile.position.x > 0.0);
    owner.objects.remove(&target);
    assert!(owner.update_owned_projectile_impacts(1.0 / 30.0).unwrap().is_empty());
    assert!(
        owner.combat_system.projectiles[&id]
            .missile_kill_self_started_frame
            .is_some()
    );
}

#[test]
fn authored_projectile_vertical_extent_controls_intervening_structure_collision_per_world() {
    let text = "Object VerticalShell\n Geometry = SPHERE\n GeometryMajorRadius = 2\n End";
    let mut parser = crate::assets::ini_parser::IniParser::new();
    parser.parse_ini_content(text, "vertical-shell.ini").unwrap();
    let definitions = crate::game_logic::projectile_definitions::ProjectileDefinitions::from_effective_definitions(
        parser.get_all_definitions().iter().map(|(name, row)| (name.as_str(), row)),
        -64.0,
    ).unwrap();
    let mut worlds = Vec::new();
    for height in [12.1, 12.0] {
        let (mut owner, source, blocker) = world();
        let object = owner.objects.get_mut(&blocker).unwrap();
        object.owner_player_id = Some(0);
        let mut pending = super::tests::lifecycle_test_pending_projectile(
            "VerticalShell", None, Vec3::new(1000.0, height, 0.0),
        );
        pending.shooter_id = source;
        pending.shooter_pos = Vec3::new(-1.0, height, 0.0);
        pending.speed = 30.0;
        pending.damage_type = DamageType::Unresistable;
        pending.splash_radius = 0.0;
        pending.projectile_collides = crate::game_logic::weapon_bootstrap::PROJECTILE_COLLIDE_STRUCTURES;
        queue_projectile(&mut owner.combat_system, pending);
        drain_pending_projectiles(
            &mut owner.combat_system, owner.objects.read(), owner.frame, &definitions,
        );
        let id = owner.combat_system.projectiles.keys().copied().next().unwrap();
        assert_eq!(owner.combat_system.projectiles[&id].collision_geometry.unwrap().major_radius, 2.0);
        worlds.push((owner, blocker, id));
    }
    assert_eq!(worlds[0].2, worlds[1].2);
    let (mut above, above_blocker, above_id) = worlds.remove(0);
    let (mut touching, touching_blocker, touching_id) = worlds.remove(0);
    assert!(above.update_owned_projectile_impacts(1.0 / 30.0).unwrap().is_empty());
    assert!(above.combat_system.projectiles.contains_key(&above_id));
    assert!(!touching.update_owned_projectile_impacts(1.0 / 30.0).unwrap().is_empty());
    assert!(!touching.combat_system.projectiles.contains_key(&touching_id));
    assert!(above.combat_system.projectiles.contains_key(&above_id));
    assert_eq!(above.host_object(above_blocker).unwrap().health.current, 100.0);
    assert!(touching.host_object(touching_blocker).unwrap().health.current < 100.0);
}

#[test]
fn walls_bit_hits_authored_fences_independently_of_structure_and_controller() {
    use crate::assets::ini_template_loader::{
        WeaponDefinitionLoadType, parse_weapon_definitions_ini_into,
    };

    // CrusaderTankGun has this mask in the mounted Zero Hour Weapon.ini.
    let mut weapons = std::collections::HashMap::new();
    parse_weapon_definitions_ini_into(
        &mut weapons,
        "Weapon CrusaderTankGun\n  ProjectileCollidesWith = STRUCTURES WALLS\nEnd\n\
         Weapon WallsOnly\n  ProjectileCollidesWith = WALLS\nEnd\n",
        WeaponDefinitionLoadType::Normal,
    );

    let blockers = r#"
Object CollisionSource
    KindOf = INFANTRY
    Body = ActiveBody BodyTag
        MaxHealth = 100
        InitialHealth = 100
    End
End
Object AmericaWall
    KindOf = IMMOBILE STRUCTURE DEFENSIVE_WALL
    FenceWidth = 19
    Body = StructureBody BodyTag
        MaxHealth = 1000
        InitialHealth = 1000
    End
    Geometry = BOX
    GeometryMajorRadius = 10
    GeometryMinorRadius = 2
    GeometryHeight = 10
End
Object ChainLinkFence01
    KindOf = IMMOBILE FORCEATTACKABLE
    FenceWidth = 27.35
    Body = ActiveBody BodyTag
        MaxHealth = 1
        InitialHealth = 1
    End
    Geometry = BOX
    GeometryMajorRadius = 14
    GeometryMinorRadius = 1
    GeometryHeight = 10
End
Object PlainBuilding
    KindOf = IMMOBILE STRUCTURE
    Body = StructureBody BodyTag
        MaxHealth = 1000
        InitialHealth = 1000
    End
    Geometry = BOX
    GeometryMajorRadius = 10
    GeometryMinorRadius = 2
    GeometryHeight = 10
End
"#;

    for (weapon, template, blocker_owner, expected_hit) in [
        ("crusadertankgun", "AmericaWall", 1, true),
        ("crusadertankgun", "ChainLinkFence01", 1, true),
        ("wallsonly", "PlainBuilding", 2, false),
    ] {
        let mut owner = GameLogic::new();
        owner
            .admit_object_ini_sources([("collision-blockers.ini".into(), blockers.into())])
            .unwrap();
        owner.frame = 100;
        let mut neutral = Player::new(0, Team::Neutral, "Neutral", false);
        neutral.map_side.role = PlayerSideRole::Neutral;
        neutral.map_side.map_player_name.clear();
        owner.add_player(neutral);
        owner.add_player(Player::new(1, Team::USA, "Owner", true));
        owner.add_player(Player::new(2, Team::GLA, "Other", true));
        let source = owner
            .create_object_for_player("CollisionSource", 1, Vec3::new(-50.0, 0.0, 0.0))
            .unwrap();
        let blocker = owner
            .create_object_for_player(template, blocker_owner, Vec3::ZERO)
            .unwrap();
        let actual_width = owner.host_object(blocker).unwrap().get_template().fence_width;
        assert_eq!(actual_width > 0.0, template != "PlainBuilding");

        let id = ObjectId(9001);
        let mut projectile = shell(id, source);
        projectile.position = Vec3::new(-1.0, 1.0, 0.0);
        projectile.target_position = Vec3::new(1000.0, 1.0, 0.0);
        projectile.projectile_collides = weapons[weapon].collide_mask.bits();
        // This fixture observes the collision itself. Body damage is covered
        // by the intended-victim and STRUCTURES callback tests above.
        projectile.no_damage = true;
        owner.combat_system.projectiles.insert(id, projectile);

        let retired = owner.update_owned_projectile_impacts(1.0 / 30.0).unwrap();
        assert_eq!(retired.contains(&id), expected_hit, "{weapon} crossing {template}");
    }
}
