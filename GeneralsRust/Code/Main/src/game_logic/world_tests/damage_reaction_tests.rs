//! Real released Body callbacks, independent role Weapons and saved continuation.
use crate::fixture_content::{apply_fixture_damage, set_fixture_health};
use crate::game_logic::host_temporary_weapon_behavior::FireWeaponWhenDamagedWeaponRole as Role;
use crate::game_logic::{GameLogic, ObjectId, Player, Team};
use crate::presentation_frame::PresentationFrame;
use glam::Vec3;

fn injured_world_with_reactions() -> (GameLogic, ObjectId) {
    injured_world_with_reaction_threshold(1.0)
}

fn injured_world_with_reaction_threshold(threshold: f32) -> (GameLogic, ObjectId) {
    // Actual parsed module and weapon content is admitted BEFORE construction.
    // Distinct roles retain distinct Weapons, even when two use one template.
    let weapons = [
        ("OwnerReactionDamaged539", 2.0),
        ("OwnerReactionCritical539", 2.0),
        ("OwnerReactionPristine539", 2.5),
    ]
    .into_iter()
    .map(|(name, damage)| format!(
        "Weapon {name}\n PrimaryDamage = {damage}\n PrimaryDamageRadius = 120\n DamageType = POISON\n DeathType = POISONED\n RadiusDamageAffects = ENEMIES NEUTRALS\n ProjectileObject = NONE\n ClipSize = 8\n ClipReloadTime = 0\n DelayBetweenShots = 0\nEnd\n"
    ))
    .collect::<String>();
    let object = format!(
        "Object OwnerHealingVehicle\n KindOf = VEHICLE\n Body = ActiveBody BodyTag\n MaxHealth = 100\n InitialHealth = 100\n End\n Behavior = FireWeaponWhenDamagedBehavior OwnedDamageReaction\n StartsActive = Yes\n DamageAmount = {threshold}\n ReactionWeaponDamaged = OwnerReactionDamaged539\n ReactionWeaponReallyDamaged = OwnerReactionCritical539\n ReactionWeaponPristine = OwnerReactionPristine539\n ReactionWeaponRubble = OwnerReactionPristine539\n End\n Behavior = InstantDeathBehavior OwnedInstantDeath\n DeathTypes = ALL\n End\nEnd\n"
    );
    let mut logic = GameLogic::new();
    logic.add_player(Player::new(0, Team::USA, "Owner", true));
    let mut definitions = std::collections::HashMap::new();
    assert_eq!(
        crate::assets::ini_template_loader::parse_weapon_definitions_ini_into(
            &mut definitions,
            &weapons,
            crate::assets::ini_template_loader::WeaponDefinitionLoadType::Normal,
        ),
        3,
    );
    let catalog = generals_sim::weapon::WeaponCatalog::from_named_definitions(definitions).unwrap();
    logic
        .admit_receiving_module_content_with_weapons(
            crate::assets::ReceivingObjectContent::from_sources([(
                "owned-damage-reactions539.ini".to_owned(),
                object + "Object ActualReactionTarget\n KindOf = VEHICLE\n ArmorSet\n Conditions = None\n Armor = OwnerReactionArmor\n End\n Body = ActiveBody TargetBody\n MaxHealth = 100\n InitialHealth = 100\n End\nEnd\n",
            )])
            .unwrap(),
            Some(catalog),
        )
        .unwrap();
    logic.admit_armor_definitions(std::sync::Arc::new(
        generals_sim::armor::ArmorDefinitions::load_from_str(
            "Armor OwnerReactionArmor\n Armor = Default 100%\n Armor = Poison 25%\nEnd\n",
        )
        .unwrap(),
    ));
    let (logic, id) = super::body_order_tests::admit_injured_body(logic);
    let source = logic.host_object(id).unwrap();
    assert!(
        source.fire_weapon_when_damaged.is_none(),
        "no hand-set residual adapter"
    );
    assert_eq!(source.temporary_weapon_runtime.damaged.len(), 1);
    assert_eq!(
        source.temporary_weapon_runtime.damaged[0]
            .weapons
            .iter()
            .flatten()
            .count(),
        4
    );
    (logic, id)
}

fn install_reaction_target(logic: &mut GameLogic) -> ObjectId {
    logic.add_player(Player::new(1, Team::China, "ReactionEnemy", false));
    for (id, alliance) in [(0, 0), (1, 1)] {
        let player = logic.get_player_mut(id).unwrap();
        player.is_alive = true;
        player.alliance_team = alliance;
    }
    assert!(logic.templates.contains_key("ActualReactionTarget"));
    let id = logic
        .create_object(
            "ActualReactionTarget",
            Team::China,
            Vec3::new(1.0, 0.0, 0.0),
        )
        .unwrap();
    assert_eq!(
        crate::game_logic::armor::armor_for_object(logic.host_object(id).unwrap())
            .adjust_damage(gamelogic::damage::DamageType::Poison, 1.0),
        0.25
    );
    // C++ ThingFactory registers with the partition, but cell coverage only
    // lands in PartitionManager::update; Weapon.cpp:1283 iterateObjectsInRange
    // sees covered objects only, so run the partition phase before reactions.
    logic
        .update_owned_partition_with_effects(
            &mut crate::game_logic::ScriptPresentationEffects::default(),
        )
        .unwrap();
    id
}

fn finish_three_states(logic: &mut GameLogic, id: ObjectId) {
    apply_fixture_damage(logic, id, 19.0);
    set_fixture_health(logic, id, 90.0);
    apply_fixture_damage(logic, id, 5.0);
}

#[test]
fn authored_same_frame_reaction_roles_fire_before_return_and_ai_does_not_replay() {
    let (mut owner, id) = injured_world_with_reactions();
    let target = install_reaction_target(&mut owner);
    let (mut peer, peer_id) = injured_world_with_reactions();
    let peer_target = install_reaction_target(&mut peer);
    assert_eq!((id, target), (peer_id, peer_target));
    let peer_runtime = peer
        .host_object(peer_id)
        .unwrap()
        .temporary_weapon_runtime
        .clone();
    apply_fixture_damage(&mut owner, id, 1.0);
    assert_eq!(owner.host_object(target).unwrap().health.current, 99.5);
    apply_fixture_damage(&mut owner, id, 19.0);
    assert_eq!(owner.host_object(target).unwrap().health.current, 99.0);
    set_fixture_health(&mut owner, id, 90.0);
    apply_fixture_damage(&mut owner, id, 5.0);
    assert_eq!(owner.host_object(target).unwrap().health.current, 98.375);
    assert_eq!(owner.host_object(target).unwrap().previous_health, 99.0);
    let runtime = owner
        .host_object(id)
        .unwrap()
        .temporary_weapon_runtime
        .clone();
    for role in [
        Role::ReactionDamaged,
        Role::ReactionReallyDamaged,
        Role::ReactionPristine,
    ] {
        assert_eq!(
            runtime.damaged[0].weapons[role.index()]
                .as_ref()
                .unwrap()
                .ammo_in_clip,
            7
        );
    }
    // The zero-delay weapons would fire again if ordinary AI replayed the
    // three still-pending health receipts using the final Pristine state.
    assert!(!owner.health_events.snapshot_damage().is_empty());
    let rendered = PresentationFrame::try_build_from_logic(&mut owner, 0).unwrap();
    let health = |object| {
        rendered
            .objects
            .iter()
            .find(|row| row.id == object)
            .unwrap()
            .health_current
    };
    assert_eq!(health(id), 85.0);
    assert_eq!(health(target), 98.375);
    assert_eq!(owner.host_authoritative_health(target), Some(98.375));
    let frame = owner.get_frame();
    owner.update();
    assert_eq!(owner.get_frame(), frame + 1);
    assert_eq!(owner.host_object(target).unwrap().health.current, 98.375);
    assert_eq!(
        owner.host_object(id).unwrap().temporary_weapon_runtime,
        runtime
    );
    assert_eq!(peer.host_object(peer_id).unwrap().health.current, 40.0);
    assert_eq!(peer.host_authoritative_health(peer_target), Some(100.0));
    assert_eq!(
        peer.host_object(peer_id).unwrap().temporary_weapon_runtime,
        peer_runtime
    );
}

#[test]
fn authored_reaction_uses_actual_overkill_damage_for_the_module_threshold() {
    let (mut owner, id) = injured_world_with_reaction_threshold(500.0);
    let target = install_reaction_target(&mut owner);
    // C++ FireWeaponWhenDamagedBehavior.cpp:162 reads actual post-armor damage.
    // A 1000-point blow exceeds the threshold even with only 40 HP left.
    apply_fixture_damage(&mut owner, id, 1000.0);
    assert_eq!(owner.host_object(target).unwrap().health.current, 99.375);
    assert_eq!(
        owner
            .host_object(id)
            .unwrap()
            .temporary_weapon_runtime
            .damaged[0]
            .weapons[Role::ReactionRubble.index()]
        .as_ref()
        .unwrap()
        .ammo_in_clip,
        7
    );
}

#[test]
fn authored_reaction_runtime_restores_and_continues_without_foreign_or_ai_replay() {
    let (mut source, id) = injured_world_with_reactions();
    let target = install_reaction_target(&mut source);
    let (mut receiver, receiver_id) = injured_world_with_reactions();
    let receiver_target = install_reaction_target(&mut receiver);
    let (peer, peer_id) = injured_world_with_reactions();
    assert_eq!((id, target), (receiver_id, receiver_target));
    apply_fixture_damage(&mut source, id, 1.0);
    let builder = crate::save_load::snapshot::SnapshotBuilder::new();
    let saved = builder.create_world_snapshot(&source).unwrap();
    builder
        .restore_from_snapshot(&saved, &mut receiver)
        .unwrap();
    assert_eq!(
        source.host_object(id).unwrap().temporary_weapon_runtime,
        receiver.host_object(id).unwrap().temporary_weapon_runtime
    );
    for world in [&mut source, &mut receiver] {
        finish_three_states(world, id);
        assert_eq!(world.host_object(target).unwrap().health.current, 98.375);
        assert_eq!(world.host_object(target).unwrap().previous_health, 99.0);
        world.update();
        assert_eq!(world.host_object(target).unwrap().health.current, 98.375);
    }
    assert_eq!(
        source.host_object(id).unwrap().temporary_weapon_runtime,
        receiver.host_object(id).unwrap().temporary_weapon_runtime
    );
    assert_eq!(peer.host_object(peer_id).unwrap().health.current, 40.0);
}

#[test]
fn fatal_reaction_fires_before_physical_retirement() {
    let (mut owner, id) = injured_world_with_reactions();
    let target = install_reaction_target(&mut owner);
    owner
        .with_host_logic_step(|world| {
            apply_fixture_damage(world, id, 100.0);
            assert_eq!(world.host_authoritative_health(id), Some(0.0));
            assert_eq!(world.host_authoritative_health(target), Some(99.375));
        })
        .unwrap();

    let hits = owner.health_events.snapshot_damage();
    let fatal_hits: Vec<_> = hits.iter().filter(|event| event.target == id).collect();
    assert_eq!(fatal_hits.len(), 1);
    assert!(fatal_hits[0].destroyed);
    assert_eq!(fatal_hits[0].body_observation().current, 0.0);
    owner.complete_presentation_observations();
    assert!(owner.objects.ordered_ids().contains(&id));
    assert!(owner.has_pending_destroy_work());
    let before = PresentationFrame::try_build_from_logic(&mut owner, 0).unwrap();
    let dying = before.objects.iter().find(|row| row.id == id).unwrap();
    assert!(dying.destroyed);
    assert_eq!(dying.health_current, 0.0);
    owner
        .process_destroy_list_with_effects_checked(&mut Default::default())
        .unwrap();
    owner.complete_presentation_observations();
    let after = PresentationFrame::try_build_from_logic(&mut owner, 0).unwrap();
    assert!(after.objects.iter().all(|row| row.id != id));
    assert_eq!(owner.host_authoritative_health(target), Some(99.375));
    assert_eq!(dying.health_current, 0.0);
}
