use gamelogic::common::Relationship;
use generals_main::game_logic::host_booby_trap::HostBoobyTrapRegistry;
use generals_main::game_logic::{GameLogic, GameMode, ObjectId, Player, Team};
use generals_main::save_load::snapshot::{
    apply_pending_player_team_chunks, stamp_player_team_chunks,
};
use glam::Vec3;

const TRAP_SOURCE: &str = "Object BoobyTrap\n KindOf = IMMOBILE BOOBY_TRAP\n Body = ActiveBody Body\n MaxHealth = 37\n InitialHealth = 19\n End\n Behavior = StickyBombUpdate Sticky\n  OffsetZ = 17\n  GeometryBasedDamageWeapon = TrapBlast\n End\n UnitSpecificSounds\n  StickyBombCreated = StickyCreated\n End\nEnd\nObject AuthoredStickyCharge\n KindOf = IMMOBILE BOOBY_TRAP\n Body = ActiveBody Body\n MaxHealth = 31\n InitialHealth = 17\n End\n Behavior = StickyBombUpdate Sticky\n End\nEnd\nObject PlainAuthoredCharge\n KindOf = IMMOBILE BOOBY_TRAP\n Body = ActiveBody Body\n MaxHealth = 29\n InitialHealth = 13\nEnd\nEnd\nObject BoobyTrapWithoutUpdate\n KindOf = IMMOBILE BOOBY_TRAP\n Body = ActiveBody Body\n MaxHealth = 1\n InitialHealth = 1\nEnd\nEnd\nObject TrapExperiencePlanter\n KindOf = INFANTRY SELECTABLE SCORE\n IsTrainable = Yes\n ExperienceRequired = 0 100 250 450\n Body = ActiveBody Body\n MaxHealth = 100\n InitialHealth = 100\n End\nEnd\nObject TrapExperienceVictim\n KindOf = INFANTRY SELECTABLE SCORE\n ExperienceValue = 125 125 125 125\n Body = ActiveBody Body\n MaxHealth = 100\n InitialHealth = 100\n End\nEnd\nObject TrapPlanter\n KindOf = INFANTRY SELECTABLE SCORE\n Body = ActiveBody Body\n MaxHealth = 100\n InitialHealth = 100\nEnd\nEnd\nObject TrapTarget\n KindOf = STRUCTURE IMMOBILE SELECTABLE SCORE\n Body = ActiveBody Body\n MaxHealth = 1000\n InitialHealth = 1000\nEnd\nEnd\nObject TrapMobileTarget\n KindOf = STRUCTURE SELECTABLE SCORE\n Body = ActiveBody Body\n MaxHealth = 1000\n InitialHealth = 1000\nEnd\nEnd\nObject TrapVictim\n KindOf = INFANTRY SELECTABLE SCORE\n Body = ActiveBody Body\n MaxHealth = 100\n InitialHealth = 100\nEnd\nEnd\n";

fn admit_trap_blast(game: &mut GameLogic) {
    let mut blast = gamelogic::weapon::WeaponTemplate::new("TrapBlast".into());
    blast.primary_damage = 200.0;
    blast.primary_damage_radius = 5.0;
    blast.secondary_damage = 50.0;
    blast.secondary_damage_radius = 15.0;
    blast.attack_range = 25.0;
    blast.projectile_name = "NONE".into();
    game.admit_receiving_weapon_snapshot(vec![("TrapBlast".into(), blast)])
        .expect("admit receiving StickyBomb weapon");
}

fn world() -> (GameLogic, ObjectId, ObjectId) {
    world_with_positions(Vec3::ZERO, Vec3::new(8.0, 0.0, 0.0))
}

fn world_with_positions(
    planter_position: Vec3,
    target_position: Vec3,
) -> (GameLogic, ObjectId, ObjectId) {
    let mut game = GameLogic::new();
    // Module parsing resolves weapon names through the WeaponStore, then the
    // receiving world admits the same authored identity into its owned catalog.
    assert_eq!(
        generals_main::assets::ini_template_loader::register_weapons_from_ini_text(
            "Weapon TrapBlast\n PrimaryDamage = 200\n PrimaryDamageRadius = 5\n SecondaryDamage = 50\n SecondaryDamageRadius = 15\n AttackRange = 25\n ProjectileNugget\n  ProjectileTemplateName = NONE\n End\nEnd\n"
        ),
        1
    );
    game.admit_object_ini_sources([
        ("BoobyTrapChargeFidelity.ini".into(), TRAP_SOURCE.into()),
        (
            "BoobyTrapAsymmetricGeometry.ini".into(),
            "Object TrapAsymmetricTarget\n KindOf = STRUCTURE IMMOBILE SELECTABLE SCORE\n Geometry = CYLINDER\n  GeometryIsSmall = No\n  GeometryHeight = 10\n  GeometryMajorRadius = 5\n  GeometryMinorRadius = 5\n End\n Body = ActiveBody Body\n MaxHealth = 1000\n InitialHealth = 1000\n End\nEnd\nObject TrapTallVictim\n KindOf = INFANTRY SELECTABLE SCORE\n Geometry = BOX\n  GeometryIsSmall = No\n  GeometryHeight = 20\n  GeometryMajorRadius = 4\n  GeometryMinorRadius = 4\n End\n Body = ActiveBody Body\n MaxHealth = 100\n InitialHealth = 100\n End\nEnd\n".into(),
        ),
    ])
    .expect("admit typed charge and host templates");
    admit_trap_blast(&mut game);
    game.start_new_game(GameMode::SinglePlayer);

    let mut planter = Player::new(0, Team::GLA, "GLA", false);
    planter.set_map_relationship(1, Relationship::Enemies);
    planter.set_map_relationship(2, Relationship::Allies);
    let mut target_owner = Player::new(1, Team::USA, "USA", false);
    target_owner.set_map_relationship(0, Relationship::Enemies);
    let mut same_player_alliance_other_faction = Player::new(2, Team::USA, "USA2", false);
    same_player_alliance_other_faction.set_map_relationship(0, Relationship::Allies);
    game.add_player(planter);
    game.add_player(target_owner);
    game.add_player(same_player_alliance_other_faction);

    let planter_id = game
        .create_object_for_player("TrapPlanter", 0, planter_position)
        .expect("planter");
    let target_id = game
        .create_object_for_player("TrapTarget", 1, target_position)
        .expect("target");
    (game, planter_id, target_id)
}

fn scripted_team_boobytrap_engine(
    template: &str,
    team: &str,
) -> gamelogic::scripting::engine::ScriptEngine {
    use gamelogic::scripting::core::{
        Condition, ConditionType, OrCondition, Parameter, ParameterType, Script, ScriptAction,
        ScriptActionType, ScriptList,
    };
    use gamelogic::scripting::engine::ScriptEngine;

    let mut action = ScriptAction::new(ScriptActionType::TeamSetBoobytrapped);
    action
        .add_parameter(Parameter::with_string(
            ParameterType::ObjectType,
            template.into(),
        ))
        .unwrap();
    action
        .add_parameter(Parameter::with_string(ParameterType::Team, team.into()))
        .unwrap();
    let mut condition = OrCondition::new();
    condition.set_first_and_condition(Some(Box::new(Condition::new(ConditionType::ConditionTrue))));
    let mut script = Script::new();
    script.script_name = "AuthoredStickyScript".into();
    script.is_one_shot = true;
    script.condition = Some(Box::new(condition));
    script.action = Some(Box::new(action));
    let mut scripts = ScriptList::new();
    scripts.append_script(Box::new(script));
    let engine = ScriptEngine::new().expect("script templates");
    engine
        .set_script_list_for_player(0, Some(Box::new(scripts)))
        .expect("install test script");
    engine
}

fn team_for_member(world: &GameLogic, target: ObjectId) -> (u32, String) {
    stamp_player_team_chunks(world)
        .expect("Team snapshot")
        .teams
        .unwrap()
        .teams
        .into_iter()
        .find(|team| {
            team.members
                .as_ref()
                .is_some_and(|members| members.contains(&target.0))
        })
        .map(|team| (team.team_id, team.team_name))
        .expect("receiving Team instance")
}

fn team_id_for_members(world: &GameLogic, members: &[ObjectId]) -> u32 {
    stamp_player_team_chunks(world)
        .expect("Team snapshot")
        .teams
        .unwrap()
        .teams
        .into_iter()
        .find(|team| {
            team.members.as_ref().is_some_and(|team_members| {
                members
                    .iter()
                    .all(|member| team_members.contains(&member.0))
            })
        })
        .expect("members share a receiving Team")
        .team_id
}

#[test]
fn scripted_team_boobytrap_uses_authored_module_and_keeps_object_without_module() {
    let (mut owner, _, owner_target) = world();
    let (peer, _, peer_target) = world();
    assert_eq!(
        owner_target, peer_target,
        "worlds reuse the target ObjectId"
    );
    let (owner_team, owner_team_name) = team_for_member(&owner, owner_target);
    let (peer_team, peer_team_name) = team_for_member(&peer, peer_target);
    assert_eq!(owner_team, peer_team, "worlds begin with the same Team ID");
    assert_eq!(owner_team_name, peer_team_name);

    owner
        .update_local_script_engine_with_owner(&scripted_team_boobytrap_engine(
            "AuthoredStickyCharge",
            &owner_team_name,
        ))
        .expect("execute authored TeamSetBoobytrapped");

    assert!(
        owner
            .host_object(owner_target)
            .unwrap()
            .has_object_status_bit("BOOBY_TRAPPED")
    );
    assert!(
        !peer
            .host_object(peer_target)
            .unwrap()
            .has_object_status_bit("BOOBY_TRAPPED")
    );
    let charge = owner
        .host_objects()
        .registered_values()
        .find(|object| object.template_name == "AuthoredStickyCharge")
        .expect("arbitrary authored template is created and initialized");
    assert_eq!(charge.owner_player_id, Some(1));
    assert_eq!(
        team_id_for_members(&owner, &[owner_target, charge.id]),
        owner_team
    );
    assert_eq!(charge.producer_id, Some(owner_target));
    assert_eq!(charge.booby_trap_attached_to, Some(owner_target));
    assert!(charge.booby_trap_special);
    assert!(
        peer.host_objects()
            .registered_values()
            .all(|object| object.template_name != "AuthoredStickyCharge")
    );

    let (mut no_module, _, no_module_target) = world();
    let (_, no_module_team_name) = team_for_member(&no_module, no_module_target);
    let count_before = no_module.host_objects().registered_len();
    no_module
        .update_local_script_engine_with_owner(&scripted_team_boobytrap_engine(
            "PlainAuthoredCharge",
            &no_module_team_name,
        ))
        .expect("execute no-module TeamSetBoobytrapped");
    assert_eq!(
        no_module.host_objects().registered_len(),
        count_before + 1,
        "C++ leaves the created object in the world when StickyBombUpdate is absent"
    );
    assert!(
        !no_module
            .host_object(no_module_target)
            .unwrap()
            .has_object_status_bit("BOOBY_TRAPPED")
    );
    let plain = no_module
        .host_objects()
        .registered_values()
        .find(|object| object.template_name == "PlainAuthoredCharge")
        .expect("template without StickyBombUpdate remains created");
    assert!(!plain.booby_trap_special);
    assert_eq!(plain.producer_id, None);
    assert_eq!(plain.owner_player_id, Some(1));
    assert_eq!(
        team_id_for_members(&no_module, &[no_module_target, plain.id]),
        team_id_for_members(&no_module, &[no_module_target])
    );
}

#[test]
fn scripted_sticky_perimeter_uses_each_targets_full_owned_transform() {
    use generals_main::save_load::snapshot::SnapshotBuilder;
    use glam::Mat4;

    let center = Vec3::new(35.0, 0.0, 45.0);
    let (mut first, _, _) = world();
    let target = first
        .create_object_for_player("TrapTallVictim", 1, center)
        .unwrap();
    let builder = SnapshotBuilder::new();
    let snapshot = builder.create_world_snapshot(&first).unwrap();
    let (mut second, _, _) = world();
    builder
        .restore_from_snapshot(&snapshot, &mut second)
        .unwrap();

    let angled = Mat4::from_translation(center)
        * Mat4::from_rotation_y(0.7)
        * Mat4::from_rotation_x(0.55)
        * Mat4::from_rotation_z(-0.37);
    assert!(second.set_registered_object_transform(target, angled));
    let (first_team, first_name) = team_for_member(&first, target);
    let (second_team, second_name) = team_for_member(&second, target);
    assert_eq!((first_team, &first_name), (second_team, &second_name));
    assert_eq!(
        builder
            .create_world_snapshot(&first)
            .unwrap()
            .logic_rng_seed_words,
        builder
            .create_world_snapshot(&second)
            .unwrap()
            .logic_rng_seed_words,
        "fixture actions start from the same instance-owned random state"
    );

    for (game, name) in [(&mut first, &first_name), (&mut second, &second_name)] {
        game.update_local_script_engine_with_owner(&scripted_team_boobytrap_engine(
            "AuthoredStickyCharge",
            name,
        ))
        .unwrap();
    }
    let charge_position = |game: &GameLogic| {
        game.host_objects()
            .registered_values()
            .find(|object| {
                object.template_name == "AuthoredStickyCharge"
                    && object.booby_trap_attached_to == Some(target)
            })
            .expect("target's charge")
            .get_position()
    };
    let local = charge_position(&first) - center;
    assert!(local.x.abs() > 0.01 || local.z.abs() > 0.01);
    let expected = angled.transform_point3(local);
    let actual = charge_position(&second);
    assert!(
        (actual.x - expected.x).abs() < 0.001,
        "{actual:?} vs {expected:?}"
    );
    assert!(
        (actual.z - expected.z).abs() < 0.001,
        "{actual:?} vs {expected:?}"
    );
    assert_eq!(
        actual.y, 0.0,
        "StickyBombUpdate places the charge on terrain"
    );
}

fn move_into_same_named_team(game: &mut GameLogic, member: ObjectId) -> u32 {
    let mut chunks = stamp_player_team_chunks(game).expect("capture Team continuation");
    let teams = chunks.teams.as_mut().unwrap();
    let source_index = teams
        .teams
        .iter()
        .position(|team| {
            team.members
                .as_ref()
                .is_some_and(|members| members.contains(&member.0))
        })
        .expect("member's current Team");
    let mut sibling = teams.teams[source_index].clone();
    let sibling_id = teams.unique_team_id;
    teams.unique_team_id += 1;
    sibling.team_id = sibling_id;
    sibling.members = Some(vec![member.0]);
    teams.teams[source_index]
        .members
        .as_mut()
        .unwrap()
        .retain(|&id| id != member.0);
    teams.teams.push(sibling);
    apply_pending_player_team_chunks(game, &chunks).expect("install same-name Team sibling");
    sibling_id
}

#[test]
fn public_sticky_bomb_init_uses_each_worlds_pose_owner_and_authored_sound() {
    let (mut first, first_planter, first_target) =
        world_with_positions(Vec3::new(31.0, 12.0, 9.0), Vec3::new(8.0, 0.0, 0.0));
    let (mut peer, peer_planter, peer_target) =
        world_with_positions(Vec3::new(77.0, 20.0, 14.0), Vec3::new(8.0, 0.0, 0.0));
    assert_eq!((first_planter, first_target), (peer_planter, peer_target));
    let first_exact_team = move_into_same_named_team(&mut first, first_planter);
    let peer_exact_team = move_into_same_named_team(&mut peer, peer_planter);
    first.queued_audio_events.drain_requests();
    peer.queued_audio_events.drain_requests();

    let first_charge = first
        .spawn_booby_trap_special_object(first_planter, Team::GLA, first_target)
        .expect("typed StickyBombUpdate charge");
    let peer_charge = peer
        .spawn_booby_trap_special_object(peer_planter, Team::GLA, peer_target)
        .expect("typed StickyBombUpdate charge in peer world");
    assert_eq!(first_charge, peer_charge);

    let first_position = first.host_object(first_charge).unwrap().get_position();
    let peer_position = peer.host_object(peer_charge).unwrap().get_position();
    assert_eq!((first_position.x, first_position.z), (31.0, 9.0));
    assert_eq!((peer_position.x, peer_position.z), (77.0, 14.0));
    assert_eq!(
        first.host_object(first_charge).unwrap().owner_player_id,
        Some(0)
    );
    assert_eq!(
        first.host_object(first_charge).unwrap().team_instance_name,
        first.host_object(first_planter).unwrap().team_instance_name,
        "charge joins the planter's exact Team instance"
    );
    let first_teams = stamp_player_team_chunks(&first).unwrap();
    assert!(
        first_teams.teams.unwrap().teams.iter().any(|team| {
            team.team_id == first_exact_team
                && team.members.as_ref().is_some_and(|members| {
                    members.contains(&first_planter.0) && members.contains(&first_charge.0)
                })
        }),
        "charge joins the exact same-name Team instance as its planter"
    );
    assert_eq!(
        first.host_object(first_charge).unwrap().experience_sink,
        Some(first_planter),
        "SpecialAbilityUpdate sinks charge XP to its planter"
    );
    assert_eq!(
        peer.host_object(peer_charge).unwrap().owner_player_id,
        Some(0)
    );
    assert_eq!(
        peer.host_object(peer_charge).unwrap().team_instance_name,
        peer.host_object(peer_planter).unwrap().team_instance_name,
        "peer charge uses the peer world's planter Team"
    );
    let peer_teams = stamp_player_team_chunks(&peer).unwrap();
    assert!(
        peer_teams.teams.unwrap().teams.iter().any(|team| {
            team.team_id == peer_exact_team
                && team.members.as_ref().is_some_and(|members| {
                    members.contains(&peer_planter.0) && members.contains(&peer_charge.0)
                })
        }),
        "peer charge joins its receiving world's exact Team instance"
    );
    assert_eq!(
        peer.host_object(peer_charge).unwrap().experience_sink,
        Some(peer_planter)
    );
    assert_eq!(
        first.host_object(first_charge).unwrap().health.maximum,
        37.0
    );
    assert_eq!(
        first.host_object(first_charge).unwrap().health.current,
        19.0
    );
    assert_eq!(peer.host_object(peer_charge).unwrap().health.maximum, 37.0);
    assert_eq!(peer.host_object(peer_charge).unwrap().health.current, 19.0);
    assert!(
        first
            .host_object(first_target)
            .unwrap()
            .has_object_status_bit("BOOBY_TRAPPED")
    );
    assert!(
        peer.host_object(peer_target)
            .unwrap()
            .has_object_status_bit("BOOBY_TRAPPED")
    );

    let first_audio = first.queued_audio_events.drain_requests();
    let peer_audio = peer.queued_audio_events.drain_requests();
    assert_eq!(first_audio.len(), 1);
    assert_eq!(first_audio[0].event_type, "StickyCreated");
    assert_eq!(first_audio[0].object_id, None);
    assert_eq!(first_audio[0].position, Some(first_position));
    assert_eq!(peer_audio.len(), 1);
    assert_eq!(peer_audio[0].event_type, "StickyCreated");
    assert_eq!(peer_audio[0].object_id, None);
    assert_eq!(peer_audio[0].position, Some(peer_position));

    let first_mobile_target = first
        .create_object_for_player("TrapMobileTarget", 1, Vec3::new(120.0, 5.0, 40.0))
        .expect("mobile target");
    let peer_mobile_target = peer
        .create_object_for_player("TrapMobileTarget", 1, Vec3::new(120.0, 5.0, 40.0))
        .expect("peer mobile target");
    assert_eq!(first_mobile_target, peer_mobile_target);
    let first_mobile_charge = first
        .spawn_booby_trap_special_object(first_planter, Team::GLA, first_mobile_target)
        .expect("mobile target charge");
    let peer_mobile_charge = peer
        .spawn_booby_trap_special_object(peer_planter, Team::GLA, peer_mobile_target)
        .expect("peer mobile target charge");
    assert_eq!(first_mobile_charge, peer_mobile_charge);
    assert_eq!(
        first
            .host_object(first_mobile_charge)
            .unwrap()
            .get_position(),
        Vec3::new(120.0, 22.0, 40.0),
        "mobile target placement uses receiving StickyBombUpdate OffsetZ"
    );
    assert_eq!(
        peer.host_object(peer_mobile_charge).unwrap().get_position(),
        Vec3::new(120.0, 22.0, 40.0)
    );
}

fn install_public_registry(
    game: &mut GameLogic,
    planter: ObjectId,
    target: ObjectId,
    charge: Option<ObjectId>,
) {
    let mut registry = HostBoobyTrapRegistry::new();
    registry.install(target, planter, Team::GLA, 0, 8.0, charge);
    game.restore_booby_traps(registry, charge.is_some() as u32);
    game.host_object_mut(target)
        .expect("target")
        .set_status_booby_trapped(true);
    // Newly created receiving objects are attached and mark coverage dirty;
    // the production partition has its contents after the ordinary update.
    game.tick_logic_frame(1.0 / 30.0, None, Some(1));
}

#[test]
fn charge_detonation_is_isolated_between_worlds_with_matching_ids() {
    let (mut first, first_planter, first_target) = world();
    let (mut peer, peer_planter, peer_target) = world();
    assert_eq!((first_planter, first_target), (peer_planter, peer_target));
    let first_charge = first
        .spawn_booby_trap_special_object(first_planter, Team::GLA, first_target)
        .unwrap();
    let peer_charge = peer
        .spawn_booby_trap_special_object(peer_planter, Team::GLA, peer_target)
        .unwrap();
    assert_eq!(first_charge, peer_charge);
    install_public_registry(&mut first, first_planter, first_target, Some(first_charge));
    install_public_registry(&mut peer, peer_planter, peer_target, Some(peer_charge));
    let peer_health = peer.host_object(peer_target).unwrap().health.current;

    let first_health = first.host_object(first_target).unwrap().health.current;
    first.detonate_booby_trap_at(first_target, Vec3::new(8.0, 0.0, 0.0), None, false, true);
    assert!(first.host_object(first_target).unwrap().health.current < first_health);
    assert_eq!(
        peer.host_object(peer_target).unwrap().health.current,
        peer_health
    );
    assert!(
        peer.host_object(peer_target)
            .unwrap()
            .has_object_status_bit("BOOBY_TRAPPED")
    );
    assert!(peer.host_object(peer_charge).unwrap().is_alive());
}

#[test]
fn detonation_kills_the_receiving_charge_through_its_authored_die_callback() {
    fn world_with_authored_die() -> (GameLogic, ObjectId, ObjectId) {
        let mut game = GameLogic::new();
        assert_eq!(
            generals_main::assets::ini_template_loader::register_weapons_from_ini_text(
                "Weapon TrapBlast\n PrimaryDamage = 200\n PrimaryDamageRadius = 5\n SecondaryDamage = 50\n SecondaryDamageRadius = 15\n AttackRange = 25\n ProjectileNugget\n  ProjectileTemplateName = NONE\n End\nEnd\n"
            ),
            1
        );
        game.admit_object_creation_list_sources([
            "ObjectCreationList ChargeDeathList\nCreateObject\nObjectNames = ChargeDeathChild\nCount = 1\nEnd\nEnd\n".into(),
        ])
        .unwrap();
        let objects = format!(
            "{}\nObject ChargeDeathChild\n KindOf = INFANTRY SELECTABLE\n Body = ActiveBody Body\n MaxHealth = 100\n InitialHealth = 100\n End\nEnd\n",
            TRAP_SOURCE.replacen(
                " Behavior = StickyBombUpdate Sticky",
                " Behavior = CreateObjectDie Replacement\n  CreationList = ChargeDeathList\n End\n Behavior = StickyBombUpdate Sticky",
                1,
            )
        );
        game.admit_object_ini_sources([("ChargeDie.ini".into(), objects)])
            .unwrap();
        admit_trap_blast(&mut game);
        game.start_new_game(GameMode::SinglePlayer);
        game.add_player(Player::new(0, Team::GLA, "ChargeOwner", false));
        game.add_player(Player::new(1, Team::USA, "TargetOwner", false));
        let target = game
            .create_object_for_player("TrapTarget", 1, Vec3::new(8.0, 0.0, 0.0))
            .unwrap();
        // The charge is in the C++ 25 + target radius scan, but outside the
        // blast radius. Its only death comes from StickyBombUpdate's kill().
        let charge = game
            .create_object_for_player("BoobyTrap", 0, Vec3::new(30.0, 0.0, 0.0))
            .unwrap();
        game.host_object_mut(charge).unwrap().producer_id = Some(target);
        game.host_object_mut(target)
            .unwrap()
            .set_status_booby_trapped(true);
        game.tick_logic_frame(1.0 / 30.0, None, Some(1));
        (game, target, charge)
    }

    let (mut first, target, charge) = world_with_authored_die();
    let (peer, peer_target, peer_charge) = world_with_authored_die();
    assert_eq!((target, charge), (peer_target, peer_charge));
    assert!(!first.host_object(charge).unwrap().booby_trap_special);
    assert!(!first.booby_trap_residual().is_booby_trapped(target));

    first.detonate_booby_trap_at(target, Vec3::new(8.0, 0.0, 0.0), None, false, true);

    assert!(
        first
            .host_objects()
            .registered_values()
            .any(|object| object.template_name == "ChargeDeathChild"),
        "the receiving charge runs its authored CreateObjectDie callback"
    );
    assert!(
        !peer
            .host_objects()
            .registered_values()
            .any(|object| object.template_name == "ChargeDeathChild"),
        "a same-ID peer world does not receive that callback"
    );
    assert!(peer.host_object(peer_charge).unwrap().is_alive());
    assert!(
        peer.host_object(peer_target)
            .unwrap()
            .has_object_status_bit("BOOBY_TRAPPED")
    );
}

#[test]
fn saved_registry_metadata_does_not_repair_a_clear_status() {
    use generals_main::save_load::snapshot::{SnapshotBuilder, WorldSnapshot};

    let (mut source, planter, target) = world();
    install_public_registry(&mut source, planter, target, None);
    source
        .host_object_mut(target)
        .unwrap()
        .set_status_booby_trapped(false);
    let builder = SnapshotBuilder::new();
    let snapshot = builder.create_world_snapshot(&source).unwrap();
    let bytes = bincode_legacy::serialize(&snapshot).unwrap();
    let decoded: WorldSnapshot = bincode_legacy::deserialize(&bytes).unwrap();
    let (mut restored, _, restored_target) = world();
    assert_eq!(target, restored_target);
    builder
        .restore_from_snapshot(&decoded, &mut restored)
        .unwrap();
    assert!(restored.booby_trap_residual().is_booby_trapped(target));
    assert!(
        !restored
            .host_object(target)
            .unwrap()
            .has_object_status_bit("BOOBY_TRAPPED")
    );
}

#[test]
fn public_booby_blast_damages_player_allied_splash_victim_across_factions() {
    let (mut game, planter, target) = world();
    let charge = game
        .spawn_booby_trap_special_object(planter, Team::GLA, target)
        .expect("authored StickyBombUpdate charge");
    assert!(
        game.host_object(target)
            .unwrap()
            .has_object_status_bit("BOOBY_TRAPPED"),
        "StickyBombUpdate initialization owns the target status transition"
    );
    install_public_registry(&mut game, planter, target, Some(charge));
    assert_eq!(
        game.host_object(charge).unwrap().producer_id,
        Some(target),
        "StickyBombUpdate producer is the trapped target"
    );

    let allied_other_faction = game
        .create_object_for_player("TrapVictim", 2, Vec3::new(18.0, 0.0, 0.0))
        .expect("same player-alliance, different faction victim");
    game.tick_logic_frame(1.0 / 30.0, None, Some(1));
    let allied_health = game
        .host_object(allied_other_faction)
        .unwrap()
        .health
        .current;
    let target_health = game.host_object(target).unwrap().health.current;
    let hits = game.detonate_booby_trap_at(target, Vec3::new(8.0, 0.0, 0.0), None, false, true);

    assert!(hits <= 1, "return value is destroyed-victim count");
    assert!(game.host_object(target).unwrap().health.current < target_health);
    assert!(
        game.host_object(allied_other_faction)
            .unwrap()
            .health
            .current
            < allied_health,
        "C++ StickyBombUpdate applies damage to every in-range object"
    );
    assert!(
        !game
            .host_object(target)
            .unwrap()
            .has_object_status_bit("BOOBY_TRAPPED")
    );
}

#[test]
fn sticky_blast_geometry_center_offset_uses_host_height_axis() {
    let (mut game, planter, _) = world();
    let target = game
        .create_object_for_player("TrapAsymmetricTarget", 1, Vec3::new(8.0, 0.0, 0.0))
        .expect("asymmetric geometry target");
    let charge = game
        .spawn_booby_trap_special_object(planter, Team::GLA, target)
        .expect("authored StickyBombUpdate charge");
    install_public_registry(&mut game, planter, target, Some(charge));
    let victim = game
        .create_object_for_player("TrapTallVictim", 1, Vec3::new(18.0, 15.0, 0.0))
        .expect("elevated asymmetric box victim");
    game.tick_logic_frame(1.0 / 30.0, None, Some(1));
    let before = game.host_object(victim).unwrap().health.current;

    let _ = game.detonate_booby_trap_at(target, Vec3::new(8.0, 0.0, 0.0), None, false, true);

    let after = game.host_object(victim).unwrap().health.current;
    assert_eq!(
        before - after,
        50.0,
        "height center offset belongs on host Y: C++ geometry Z; a mistaken host Z offset selects primary damage instead"
    );
}

#[test]
fn public_booby_spawn_rejects_missing_target_without_creating_charge() {
    let (mut game, planter, target) = world();
    let objects_before = game.host_objects().registered_len();
    let spawned_before = game.booby_trap_objects_spawned();
    assert_eq!(
        game.spawn_booby_trap_special_object(planter, Team::GLA, ObjectId(u32::MAX)),
        None
    );
    assert_eq!(game.host_objects().registered_len(), objects_before);
    assert_eq!(game.booby_trap_objects_spawned(), spawned_before);
    assert!(
        !game
            .host_object(target)
            .unwrap()
            .has_object_status_bit("BOOBY_TRAPPED")
    );
}

#[test]
fn public_booby_trigger_gate_uses_charge_owner_player_relationship() {
    let (mut game, planter, target) = world();
    let charge = game
        .spawn_booby_trap_special_object(planter, Team::GLA, target)
        .expect("authored StickyBombUpdate charge");
    install_public_registry(&mut game, planter, target, Some(charge));
    let allied_trigger = game
        .create_object_for_player("TrapVictim", 2, Vec3::new(9.0, 0.0, 0.0))
        .expect("player-allied trigger with a different faction");
    let target_health = game.host_object(target).unwrap().health.current;

    assert_eq!(
        game.detonate_booby_trap_at(
            target,
            Vec3::new(8.0, 0.0, 0.0),
            Some(allied_trigger),
            false,
            false,
        ),
        0,
        "C++ rejects an allied trigger using the selected charge owner"
    );
    assert_eq!(
        game.host_object(target).unwrap().health.current,
        target_health
    );
    assert!(
        game.host_object(target)
            .unwrap()
            .has_object_status_bit("BOOBY_TRAPPED")
    );
    assert!(game.host_object(charge).unwrap().is_alive());
}

#[test]
fn public_booby_trigger_relationship_uses_exact_same_named_team_identity() {
    let (mut game, planter, target) = world();
    let charge = game
        .spawn_booby_trap_special_object(planter, Team::GLA, target)
        .expect("authored StickyBombUpdate charge");
    install_public_registry(&mut game, planter, target, Some(charge));
    let target_team_id = stamp_player_team_chunks(&game)
        .unwrap()
        .teams
        .unwrap()
        .teams
        .into_iter()
        .find(|team| {
            team.members
                .as_ref()
                .is_some_and(|members| members.contains(&target.0))
        })
        .expect("target's exact receiving Team")
        .team_id;
    let target_team_name = game.host_object(target).unwrap().team_instance_name.clone();
    game.get_player_mut(0)
        .unwrap()
        .set_team_relationship_override(target_team_id, Relationship::Allies);

    let trigger = game
        .create_object_for_player("TrapVictim", 1, Vec3::new(9.0, 0.0, 0.0))
        .expect("trigger with same owner and Team identity");
    game.tick_logic_frame(1.0 / 30.0, None, Some(1));
    let health_before = game.host_object(target).unwrap().health.current;
    assert_eq!(
        game.detonate_booby_trap_at(
            target,
            Vec3::new(8.0, 0.0, 0.0),
            Some(trigger),
            false,
            false,
        ),
        0,
        "Player::getRelationship(target Team ID) blocks the matching exact Team"
    );
    assert_eq!(
        game.host_object(target).unwrap().health.current,
        health_before
    );
    assert!(game.host_object(charge).unwrap().is_alive());

    let sibling_team_id = move_into_same_named_team(&mut game, trigger);
    assert_ne!(sibling_team_id, target_team_id);
    assert_eq!(
        game.host_object(trigger).unwrap().team_instance_name,
        target_team_name,
        "the relation distinction is the exact Team ID; both teams share the same name"
    );
    let _ = game.detonate_booby_trap_at(
        target,
        Vec3::new(8.0, 0.0, 0.0),
        Some(trigger),
        false,
        false,
    );
    assert!(game.host_object(target).unwrap().health.current < health_before);
    assert!(
        !game
            .host_object(target)
            .unwrap()
            .has_object_status_bit("BOOBY_TRAPPED")
    );
}

#[test]
fn public_booby_trigger_gate_rejects_missing_trigger_object() {
    let (mut game, planter, target) = world();
    let charge = game
        .spawn_booby_trap_special_object(planter, Team::GLA, target)
        .expect("authored StickyBombUpdate charge");
    install_public_registry(&mut game, planter, target, Some(charge));
    let target_health = game.host_object(target).unwrap().health.current;

    assert_eq!(
        game.detonate_booby_trap_at(
            target,
            Vec3::new(8.0, 0.0, 0.0),
            Some(ObjectId(u32::MAX)),
            false,
            false,
        ),
        0,
        "a supplied but missing trigger object fails closed"
    );
    assert_eq!(
        game.host_object(target).unwrap().health.current,
        target_health
    );
    assert!(
        game.host_object(target)
            .unwrap()
            .has_object_status_bit("BOOBY_TRAPPED")
    );
    assert!(game.host_object(charge).unwrap().is_alive());
}

#[test]
fn lethal_sticky_damage_reentry_detonates_once() {
    let (mut game, planter, target) = world();
    let charge = game
        .spawn_booby_trap_special_object(planter, Team::GLA, target)
        .expect("authored StickyBombUpdate charge");
    install_public_registry(&mut game, planter, target, Some(charge));
    let detonations_before = game.booby_trap_residual().detonations_total;
    game.host_object_mut(target).unwrap().health.current = 100.0;

    // The authored primary blast kills its own target. Death processing can
    // synchronously ask to detonate the same planted trap again.
    let _ = game.detonate_booby_trap_at(target, Vec3::new(8.0, 0.0, 0.0), None, false, true);

    assert_eq!(
        game.booby_trap_residual().detonations_total,
        detonations_before + 1,
        "same-target callback reentry is suppressed until original FX/status/charge completion"
    );
}

#[test]
fn public_booby_trigger_uses_charge_player_to_team_even_for_defector() {
    let (mut game, planter, target) = world();
    let charge = game
        .spawn_booby_trap_special_object(planter, Team::GLA, target)
        .expect("authored StickyBombUpdate charge");
    install_public_registry(&mut game, planter, target, Some(charge));
    let enemy_trigger = game
        .create_object_for_player("TrapVictim", 1, Vec3::new(9.0, 0.0, 0.0))
        .expect("enemy trigger");
    game.host_object_mut(enemy_trigger)
        .unwrap()
        .begin_undetected_defection(0, 100, false);
    assert!(
        game.host_object(enemy_trigger)
            .unwrap()
            .is_undetected_defector()
    );
    let target_health = game.host_object(target).unwrap().health.current;

    let _destroyed = game.detonate_booby_trap_at(
        target,
        Vec3::new(8.0, 0.0, 0.0),
        Some(enemy_trigger),
        false,
        false,
    );
    assert!(game.host_object(target).unwrap().health.current < target_health);
    assert!(
        !game
            .host_object(target)
            .unwrap()
            .has_object_status_bit("BOOBY_TRAPPED")
    );
}

#[test]
fn public_booby_charge_scan_keeps_nearest_invalid_and_rejects_missing_charge() {
    let (mut game, planter, target) = world();
    let valid_charge = game
        .create_object_for_player("BoobyTrap", 0, Vec3::new(12.0, 8.0, 0.0))
        .expect("authored StickyBombUpdate charge");
    game.host_object_mut(valid_charge).unwrap().producer_id = Some(target);
    let nearest_invalid = game
        .create_object_for_player("BoobyTrapWithoutUpdate", 0, Vec3::new(8.0, 8.0, 0.0))
        .expect("nearest charge without StickyBombUpdate");
    game.host_object_mut(nearest_invalid).unwrap().producer_id = Some(target);
    install_public_registry(&mut game, planter, target, Some(valid_charge));

    let health_before = game.host_object(target).unwrap().health.current;
    assert_eq!(
        game.detonate_booby_trap_at(target, Vec3::new(8.0, 0.0, 0.0), None, false, true),
        0,
        "nearest producer-matched charge lacks StickyBombUpdate"
    );
    assert_eq!(
        game.host_object(target).unwrap().health.current,
        health_before
    );
    assert!(
        game.host_object(target)
            .unwrap()
            .has_object_status_bit("BOOBY_TRAPPED")
    );

    // A stale registry id cannot authorize damage when no eligible charge is
    // present in the object scan.
    game.host_object_mut(nearest_invalid)
        .unwrap()
        .status
        .destroyed = true;
    game.host_object_mut(valid_charge).unwrap().status.destroyed = true;
    let stale_target = game.host_object(target).unwrap().health.current;
    install_public_registry(&mut game, planter, target, Some(ObjectId(u32::MAX)));
    assert_eq!(
        game.detonate_booby_trap_at(target, Vec3::new(8.0, 0.0, 0.0), None, false, true),
        0,
        "missing actual charge is rejected despite stale registry metadata"
    );
    assert_eq!(
        game.host_object(target).unwrap().health.current,
        stale_target
    );
    assert!(
        game.host_object(target)
            .unwrap()
            .has_object_status_bit("BOOBY_TRAPPED")
    );
}

#[test]
fn sticky_charge_retains_exact_team_and_experience_sink_after_save() {
    use generals_main::game_logic::VeterancyLevel;
    use generals_main::save_load::snapshot::{SnapshotBuilder, WorldSnapshot};

    fn prepare(game: &mut GameLogic, target: ObjectId) -> (ObjectId, ObjectId, ObjectId, u32) {
        let planter = game
            .create_object_for_player("TrapExperiencePlanter", 0, Vec3::new(100.0, 0.0, 100.0))
            .unwrap();
        let victim = game
            .create_object_for_player("TrapExperienceVictim", 1, Vec3::new(8.0, 0.0, 0.0))
            .unwrap();
        let team = move_into_same_named_team(game, planter);
        let charge = game
            .spawn_booby_trap_special_object(planter, Team::GLA, target)
            .unwrap();
        install_public_registry(game, planter, target, Some(charge));
        game.tick_logic_frame(1.0 / 30.0, None, Some(1));
        (planter, victim, charge, team)
    }

    let (mut source, _, target) = world();
    let (mut peer, _, peer_target) = world();
    let (planter, victim, charge, team) = prepare(&mut source, target);
    let (peer_planter, peer_victim, peer_charge, peer_team) = prepare(&mut peer, peer_target);
    assert_eq!(
        (target, planter, victim, charge, team),
        (
            peer_target,
            peer_planter,
            peer_victim,
            peer_charge,
            peer_team
        )
    );

    let builder = SnapshotBuilder::new();
    let snapshot = builder.create_world_snapshot(&source).unwrap();
    let bytes = bincode_legacy::serialize(&snapshot).unwrap();
    let decoded: WorldSnapshot = bincode_legacy::deserialize(&bytes).unwrap();
    let (mut restored, _, _) = world();
    builder
        .restore_from_snapshot(&decoded, &mut restored)
        .unwrap();
    assert_eq!(
        restored.host_object(charge).unwrap().experience_sink,
        Some(planter)
    );
    let chunks = stamp_player_team_chunks(&restored).unwrap();
    assert!(chunks.teams.unwrap().teams.iter().any(|entry| {
        entry.team_id == team
            && entry
                .members
                .as_ref()
                .is_some_and(|members| members.contains(&planter.0) && members.contains(&charge.0))
    }));

    restored.detonate_booby_trap_at(target, Vec3::new(8.0, 0.0, 0.0), None, false, true);
    assert!(!restored.host_object(victim).unwrap().is_alive());
    assert_eq!(
        restored.host_object(planter).unwrap().experience.level,
        VeterancyLevel::Veteran,
        "the charge forwards the kill's XP to its saved planter"
    );
    assert_eq!(
        peer.host_object(peer_planter).unwrap().experience.level,
        VeterancyLevel::Rookie
    );
    assert!(peer.host_object(peer_victim).unwrap().is_alive());
}
