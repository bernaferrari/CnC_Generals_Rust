use super::*;

fn set_radar_fixture_owner(
    world: &mut GameLogic,
    id: ObjectId,
    team: Team,
    owner: Option<u32>,
    events: &mut crate::game_logic::host_owner_log::OwnershipEvents,
) {
    let (object, partition) = world.host_object_and_partition_mut(id).unwrap();
    object.set_team_and_owner_with_partition_owner(team, owner, events, partition);
}

fn radar_world() -> (GameLogic, ObjectId) {
    let mut world = GameLogic::new();
    world.add_player(Player::new(1, Team::GLA, "First", true));
    world.add_player(Player::new(2, Team::GLA, "Second", true));
    let mut template = ThingTemplate::new("GLAVehicleRadarVan");
    template.set_health(200.0);
    let id = ObjectId(41);
    let name = template.name.clone();
    world.templates.insert(name.clone(), template);
    world.next_object_id = id;
    assert_eq!(
        world.create_object_for_player(&name, 1, Vec3::ZERO),
        Some(id)
    );
    world.host_object_mut(id).unwrap().construction_percent = 1.0;
    world.host_radar_add_object(id);
    (world, id)
}

#[test]
fn same_id_sessions_keep_provider_capture_deletion_and_snapshots_local() {
    let mut _ownership_output = crate::game_logic::host_owner_log::OwnershipEvents::default();

    let (mut first, id) = radar_world();
    let (mut peer, peer_id) = radar_world();
    assert_eq!(id, peer_id);
    assert_eq!(first.players[&1].radar_count, 1);
    let peer_before = peer.host_radar_snapshot();
    set_radar_fixture_owner(&mut first, id, Team::GLA, Some(2), &mut _ownership_output);
    first.on_capture_object_residual(id, Team::GLA, Team::GLA);
    assert_eq!(first.players[&1].radar_count, 0);
    assert_eq!(first.players[&2].radar_count, 1);
    assert_eq!(peer.host_radar_snapshot(), peer_before);
    peer.set_radar_client_quiet(true);
    peer.update_player_radar();
    first.destroy_live_object(id);
    assert_eq!(first.players[&2].radar_count, 0);
    assert_eq!(peer.players[&1].radar_count, 1);
    assert!(
        first.queued_audio_events.is_empty(),
        "out-of-logic lifecycle is silent"
    );
}

#[test]
fn restoring_admission_does_not_regrant_saved_counts() {
    let (mut world, _) = radar_world();
    let before = world.host_radar_snapshot();
    world.host_radar.providers.clear();
    world.rebuild_host_radar_provider_admission();
    world.update_player_radar();
    assert_eq!(world.host_radar_snapshot(), before);
}

#[test]
fn radar_audio_checks_owned_frame_logic_quiet_dead_and_live_victory_facts() {
    let (mut world, _) = radar_world();
    world.frame = 9;
    world.host_logic_step_active = true;
    world
        .victory_conditions
        .set_victory_conditions(crate::game_logic::victory_conditions::VictoryType::empty());
    world.set_radar_client_quiet(true);
    world.record_host_radar_transition(1, 1, false, true);
    assert!(world.queued_audio_events.is_empty());
    world.set_radar_client_quiet(false);
    world.players.get_mut(&1).unwrap().is_alive = false;
    world.record_host_radar_transition(1, 1, false, true);
    assert!(world.queued_audio_events.is_empty());
    world.players.get_mut(&1).unwrap().is_alive = true;
    world.frame = 0;
    world.record_host_radar_transition(1, 1, false, true);
    assert!(world.queued_audio_events.is_empty());
    world.frame = 9;
    world.record_host_radar_transition(1, 1, false, true);
    assert_eq!(world.queued_audio_events.len(), 1);
}

#[test]
fn player_owned_research_does_not_unlock_a_same_faction_peer() {
    let mut world = GameLogic::new();
    world.add_player(Player::new(1, Team::China, "First", true));
    world.add_player(Player::new(2, Team::China, "Second", true));
    let mut template = ThingTemplate::new("ChinaCommandCenter");
    template
        .add_kind_of(KindOf::CommandCenter)
        .set_health(500.0);
    for owner in [1, 2] {
        let id = ObjectId(owner + 80);
        world
            .templates
            .insert(template.name.clone(), template.clone());
        world.next_object_id = id;
        assert_eq!(
            world.create_object_for_player(&template.name, owner, Vec3::ZERO),
            Some(id)
        );
        world.host_object_mut(id).unwrap().construction_percent = 1.0;
        world.host_radar_add_object(id);
    }
    world.apply_host_upgrade_complete(Team::China, 1, "Upgrade_ChinaRadar");
    assert_eq!(world.players[&1].radar_count, 1);
    assert_eq!(world.players[&2].radar_count, 0);
    assert!(
        world.players[&1]
            .unlocked_sciences
            .contains("Upgrade_ChinaRadar")
    );
    assert!(
        !world.players[&2]
            .unlocked_sciences
            .contains("Upgrade_ChinaRadar")
    );
    assert!(!world.objects[&ObjectId(82)].has_upgrade_tag("Upgrade_ChinaRadar"));
}

#[test]
fn snapshot_restores_provider_admission_before_resumed_capture() {
    let mut _ownership_output = crate::game_logic::host_owner_log::OwnershipEvents::default();

    let (mut world, id) = radar_world();
    let builder = crate::save_load::snapshot::SnapshotBuilder::new();
    let snapshot = builder.create_world_snapshot(&world).expect("capture");
    builder
        .restore_from_snapshot(&snapshot, &mut world)
        .expect("restore");
    assert_eq!(world.players[&1].radar_count, 1);
    world.update_player_radar();
    assert_eq!(
        world.players[&1].radar_count, 1,
        "load must not grant twice"
    );
    set_radar_fixture_owner(&mut world, id, Team::GLA, Some(2), &mut _ownership_output);
    world.on_capture_object_residual(id, Team::GLA, Team::GLA);
    assert_eq!(world.players[&1].radar_count, 0);
    assert_eq!(world.players[&2].radar_count, 1);
}

#[test]
fn capture_emits_remove_then_add_at_the_actual_logic_boundary() {
    let mut _ownership_output = crate::game_logic::host_owner_log::OwnershipEvents::default();

    let (mut world, id) = radar_world();
    world.frame = 12;
    world
        .victory_conditions
        .set_victory_conditions(crate::game_logic::victory_conditions::VictoryType::empty());
    world.with_host_logic_step(|world| {
        set_radar_fixture_owner(world, id, Team::GLA, Some(2), &mut _ownership_output);
        world.on_capture_object_residual(id, Team::GLA, Team::GLA);
    });
    assert_eq!(world.queued_audio_events.len(), 2);
    assert_eq!(
        world.queued_audio_events[0].event_type,
        crate::game_logic::host_radar::RADAR_OFFLINE_AUDIO
    );
    assert_eq!(
        world.queued_audio_events[1].event_type,
        crate::game_logic::host_radar::RADAR_ONLINE_AUDIO
    );
    assert_eq!(world.players[&1].radar_count, 0);
    assert_eq!(world.players[&2].radar_count, 1);
}

fn authored_template(world: &mut GameLogic, name: &str, modules: &str) -> ThingTemplate {
    authored_template_with_initial_health(world, name, modules, None)
}

fn authored_template_with_initial_health(
    world: &mut GameLogic,
    name: &str,
    modules: &str,
    initial_health: Option<f32>,
) -> ThingTemplate {
    let initial = initial_health
        .map(|health| format!(" InitialHealth = {health}\n"))
        .unwrap_or_default();
    let source = format!(
        "Object {name}\n KindOf = STRUCTURE COMMANDCENTER FS_TECHNOLOGY SELECTABLE MP_COUNT_FOR_VICTORY\n Body = ActiveBody ModuleTag_Body\n MaxHealth = 500\n{initial} End\n{modules}End\n"
    );
    world
        .admit_receiving_module_content(
            crate::assets::ReceivingObjectContent::from_sources([(
                "radar_owner_authored.ini".to_owned(),
                source,
            )])
            .expect("paired authored radar definitions"),
        )
        .expect("receiving radar module owner");
    world
        .templates
        .get(name)
        .expect("receiving radar template")
        .clone()
}

fn authored_world() -> (GameLogic, ObjectId) {
    authored_world_with_initial_health(None)
}

fn authored_world_with_initial_health(initial_health: Option<f32>) -> (GameLogic, ObjectId) {
    let mut world = GameLogic::new();
    world.add_player(Player::new(1, Team::GLA, "First", true));
    world.add_player(Player::new(2, Team::GLA, "Second", true));
    crate::game_logic::host_upgrade_rules::register_test_upgrade(
        &world,
        "Upgrade_GLARadar",
        "OBJECT",
        0,
        1,
    );
    let _template = authored_template_with_initial_health(
        &mut world,
        "ArbitraryRadarFacility",
        " Behavior = RadarUpgrade ModuleTag_Radar\n TriggeredBy = Upgrade_GLARadar\n DisableProof = Yes\n End\n Behavior = RadarUpdate ModuleTag_Update\n RadarExtendTime = 50\n End\n Behavior = GrantUpgradeCreate ModuleTag_Grant\n UpgradeToGrant = Upgrade_GLARadar\n ExemptStatus = UNDER_CONSTRUCTION\n End\n",
        initial_health,
    );
    let id = world
        .create_object_for_player("ArbitraryRadarFacility", 1, glam::Vec3::ZERO)
        .unwrap();
    (world, id)
}

#[test]
fn arbitrary_authored_radar_uses_real_grant_module_and_own_duration() {
    let (world, id) = authored_world();
    assert_eq!(world.players[&1].radar_count, 1);
    assert_eq!(world.players[&1].disable_proof_radar_count, 1);
    assert_eq!(world.players[&2].radar_count, 0);
    assert_eq!(world.objects[&id].radar_upgrade_executed, [true]);
    assert_eq!(
        world.objects[&id].radar_extend_done_frame, 1,
        "CPP Real frame duration truncates at assignment"
    );
    assert!(world.objects[&id].radar_active);
}

#[test]
fn authored_absence_and_invalid_boolean_never_infer_a_provider_from_its_name() {
    for modules in [
        "",
        " Behavior = RadarUpgrade ModuleTag_Radar\n TriggeredBy = Upgrade_GLARadar\n DisableProof = invalid\n End\n",
    ] {
        let mut world = GameLogic::new();
        world.add_player(Player::new(1, Team::USA, "First", true));
        if modules.contains("DisableProof = invalid") {
            // CPP RadarUpgrade uses INI::parseBool: malformed data is rejected
            // before any provider is admitted. The same source with a valid
            // boolean must parse, so unrelated fixture failures cannot pass.
            let source = format!(
                "Object AmericaCommandCenter\n KindOf = STRUCTURE COMMANDCENTER\n Body = ActiveBody BodyTag\n MaxHealth = 500\n End\n{modules}End\n"
            );
            let error = crate::assets::ReceivingObjectContent::from_sources([(
                "radar_owner_authored.ini".to_owned(),
                source.clone(),
            )])
            .err()
            .expect("invalid authored radar boolean must be rejected");
            assert!(
                error.to_ascii_lowercase().contains("invalid")
                    || error.to_ascii_lowercase().contains("bool"),
                "{error}"
            );
            assert!(
                crate::assets::ReceivingObjectContent::from_sources([(
                    "radar_owner_authored.ini".to_owned(),
                    source.replace("DisableProof = invalid", "DisableProof = No"),
                )])
                .is_ok(),
                "valid same-source boolean control"
            );
            assert_eq!(world.players[&1].radar_count, 0);
            continue;
        }
        let template = authored_template(&mut world, "AmericaCommandCenter", modules);
        assert!(template.radar_upgrade_rules.is_some());
        let id = world
            .create_object_for_player("AmericaCommandCenter", 1, glam::Vec3::ZERO)
            .unwrap();
        world.apply_upgrade_to_object(id, "Upgrade_GLARadar");
        world.update_player_radar();
        assert_eq!(world.players[&1].radar_count, 0);
    }
}

#[test]
fn owned_disabled_operations_are_observable_before_the_next_operation() {
    let (mut world, id) = authored_world();
    let (peer, peer_id) = authored_world();
    assert_eq!(id, peer_id);
    world.frame = 10;
    world
        .victory_conditions
        .set_victory_conditions(crate::game_logic::victory_conditions::VictoryType::empty());
    world.with_host_logic_step(|world| {
        world.with_owned_radar_disabled_operation(id, |object| {
            object.set_status_disabled_held(true)
        });
        assert_eq!(world.players[&1].radar_count, 0);
        assert_eq!(world.players[&1].disable_proof_radar_count, 0);
        assert_eq!(peer.players[&1].radar_count, 1);
        world.with_owned_radar_disabled_operation(id, |object| {
            object.set_status_disabled_held(false)
        });
        assert_eq!(world.players[&1].radar_count, 1);
        assert_eq!(world.objects[&id].radar_upgrade_executed, [true]);
    });
    let edges: Vec<_> = world
        .queued_audio_events
        .iter()
        .map(|event| event.event_type.as_str())
        .collect();
    assert_eq!(
        edges,
        [
            crate::game_logic::host_radar::RADAR_OFFLINE_AUDIO,
            crate::game_logic::host_radar::RADAR_ONLINE_AUDIO
        ]
    );
}

#[test]
fn authored_executed_save_survives_removed_trigger_and_capture() {
    let mut _ownership_output = crate::game_logic::host_owner_log::OwnershipEvents::default();

    let (mut world, id) = authored_world();
    world
        .objects
        .get_mut(&id)
        .unwrap()
        .remove_upgrade_tag("Upgrade_GLARadar");
    let builder = crate::save_load::snapshot::SnapshotBuilder::new();
    let snapshot = builder.create_world_snapshot(&world).unwrap();
    builder
        .restore_from_snapshot(&snapshot, &mut world)
        .unwrap();
    assert_eq!(world.objects[&id].radar_upgrade_executed, [true]);
    world.update_player_radar();
    assert_eq!(
        world.players[&1].radar_count, 1,
        "executed is state, not current trigger availability"
    );
    set_radar_fixture_owner(&mut world, id, Team::GLA, Some(2), &mut _ownership_output);
    world.on_capture_object_residual(id, Team::GLA, Team::GLA);
    assert_eq!(world.players[&1].radar_count, 0);
    assert_eq!(world.players[&2].radar_count, 1);
    world.destroy_live_object(id);
    assert_eq!(world.players[&2].radar_count, 0);
}

#[test]
fn authored_module_order_preserves_all_conflict_and_removal_observation() {
    let mut world = GameLogic::new();
    world.add_player(Player::new(1, Team::GLA, "First", true));
    for name in ["Upgrade_First", "Upgrade_Second", "Upgrade_Blocked"] {
        crate::game_logic::host_upgrade_rules::register_test_upgrade(&world, name, "OBJECT", 0, 1);
    }
    let _template = authored_template(
        &mut world,
        "OrderedRadarModules",
        " Behavior = RadarUpgrade ModuleTag_First\n TriggeredBy = Upgrade_First Upgrade_Second\n RequiresAllTriggers = Yes\n ConflictsWith = Upgrade_Blocked\n RemovesUpgrades = Upgrade_First\n End\n Behavior = RadarUpgrade ModuleTag_Second\n TriggeredBy = Upgrade_First\n End\n",
    );
    let id = world
        .create_object_for_player("OrderedRadarModules", 1, glam::Vec3::ZERO)
        .unwrap();
    world
        .objects
        .get_mut(&id)
        .unwrap()
        .apply_upgrade_tag("Upgrade_Blocked");
    world.apply_upgrade_to_object(id, "Upgrade_First");
    assert_eq!(world.objects[&id].radar_upgrade_executed, [false, true]);
    world.apply_upgrade_to_object(id, "Upgrade_Second");
    assert_eq!(world.objects[&id].radar_upgrade_executed, [false, true]);
    world
        .objects
        .get_mut(&id)
        .unwrap()
        .remove_upgrade_tag("Upgrade_Blocked");
    world.update_player_radar();
    assert_eq!(world.objects[&id].radar_upgrade_executed, [true, true]);
    assert!(!world.objects[&id].has_upgrade_tag("Upgrade_First"));
    assert_eq!(world.players[&1].radar_count, 2);
    world.with_owned_radar_disabled_operation(id, |object| object.set_status_disabled_held(true));
    assert_eq!(
        world.players[&1].radar_count, 1,
        "CPP findModule adjusts only first RadarUpgrade"
    );
    world.with_owned_radar_disabled_operation(id, |object| object.set_status_disabled_held(false));
    assert_eq!(world.players[&1].radar_count, 2);
}

#[test]
fn disabled_grant_capture_and_future_callbacks_preserve_exact_contributors() {
    let mut world = GameLogic::new();
    world.add_player(Player::new(1, Team::GLA, "First", true));
    world.add_player(Player::new(2, Team::GLA, "Second", true));
    crate::game_logic::host_upgrade_rules::register_test_upgrade(
        &world,
        "Upgrade_ExplicitRadar",
        "OBJECT",
        0,
        1,
    );
    let _template = authored_template(
        &mut world,
        "DisabledGrant",
        " Behavior = RadarUpgrade ModuleTag_Radar\n TriggeredBy = Upgrade_ExplicitRadar\n End\n",
    );
    let id = world
        .create_object_for_player("DisabledGrant", 1, glam::Vec3::ZERO)
        .unwrap();
    world.with_owned_radar_disabled_operation(id, |object| object.set_status_disabled_held(true));
    world.apply_upgrade_to_object(id, "Upgrade_ExplicitRadar");
    assert_eq!(
        world.players[&1].radar_count, 1,
        "CPP upgradeImplementation grants despite already disabled"
    );
    world.update_player_radar();
    assert_eq!(world.players[&1].radar_count, 1);
    {
        let (object, events, partition) = world.host_object_ownership_partition_mut(id).unwrap();
        object.set_team_and_owner_with_partition_owner(Team::GLA, Some(2), events, partition);
    }
    world.on_capture_object_residual(id, Team::GLA, Team::GLA);
    assert_eq!(
        world.players[&1].radar_count, 1,
        "disabled onCapture returns without moving contribution"
    );
    assert_eq!(world.players[&2].radar_count, 0);
    let builder = crate::save_load::snapshot::SnapshotBuilder::new();
    let snapshot = builder.create_world_snapshot(&world).unwrap();
    builder
        .restore_from_snapshot(&snapshot, &mut world)
        .unwrap();
    assert_eq!(world.objects[&id].radar_upgrade_contributors, [vec![1]]);
    world.with_owned_radar_disabled_operation(id, |object| object.set_status_disabled_held(false));
    assert_eq!(world.players[&1].radar_count, 1);
    assert_eq!(world.players[&2].radar_count, 1);
    assert_eq!(world.objects[&id].radar_upgrade_contributors, [vec![1, 2]]);
    world.destroy_live_object(id);
    assert_eq!(
        world.players[&1].radar_count, 1,
        "CPP deletion removes current controller once, retaining exceptional old grant"
    );
    assert_eq!(world.players[&2].radar_count, 0);
}

#[test]
fn unknown_authored_upgrade_names_cannot_be_supplied_by_another_world() {
    let mut world = GameLogic::new();
    world.add_player(Player::new(1, Team::GLA, "First", true));
    let peer = GameLogic::new();
    crate::game_logic::host_upgrade_rules::register_test_upgrade(
        &peer,
        "Upgrade_PeerOnly",
        "OBJECT",
        0,
        1,
    );
    let _template = authored_template(
        &mut world,
        "UnknownCatalogRadar",
        " Behavior = RadarUpgrade ModuleTag_Radar\n TriggeredBy = Upgrade_PeerOnly\n End\n",
    );
    let id = world
        .create_object_for_player("UnknownCatalogRadar", 1, glam::Vec3::ZERO)
        .unwrap();
    world.apply_upgrade_to_object(id, "Upgrade_PeerOnly");
    assert_eq!(world.players[&1].radar_count, 0);
    crate::game_logic::host_upgrade_rules::register_test_upgrade(
        &world,
        "Upgrade_PeerOnly",
        "OBJECT",
        0,
        1,
    );
    world.update_player_radar();
    assert_eq!(world.players[&1].radar_count, 1);
}

#[test]
fn actual_emp_field_admits_radar_before_impact_presentation_and_next_operation() {
    // CPP ActiveBodyModuleData defaults InitialHealth to zero independently
    // of MaxHealth. This real EMP candidate must explicitly start alive.
    let (mut world, id) = authored_world_with_initial_health(Some(500.0));
    let (peer, peer_id) = authored_world_with_initial_health(Some(500.0));
    assert_eq!(id, peer_id);
    for object in [&world.objects[&id], &peer.objects[&peer_id]] {
        assert_eq!(object.health.current, 500.0);
        assert_eq!(object.health.maximum, 500.0);
        assert!(object.is_alive());
        assert_eq!(
            object.body_damage_state,
            crate::game_logic::host_enum_table_residual::HostBodyDamageType::Pristine
        );
    }
    // CPP EMPUpdate253 accepts structures only with KINDOFMASK_FS. An
    // arbitrary fixture basename must not provide that authored capability.
    assert!(world.objects[&id].is_kind_of(KindOf::FSTechnology));
    assert!(world.objects[&id].is_faction_structure());
    world.frame = 20;
    world
        .victory_conditions
        .set_victory_conditions(crate::game_logic::victory_conditions::VictoryType::empty());
    world.with_host_logic_step(|world| {
        assert!(world.apply_emp_pulse_disable_field_at(1, glam::Vec3::ZERO, None));
        assert!(world.objects[&id].status.disabled_emp);
        assert_eq!(world.players[&1].radar_count, 0);
        assert_eq!(peer.players[&1].radar_count, 1);
        assert!(world.queued_audio_events.iter().any(|event| event.event_type == crate::game_logic::host_radar::RADAR_OFFLINE_AUDIO));
    });
}

#[cfg(test)]
mod bulk_retirement_order_tests {
    use super::*;
    use crate::save_load::snapshot::SnapshotBuilder;

    fn admitted_map_player(display_name: &str) -> Player {
        use game_engine::common::{dict::Dict, well_known_keys::key_player_name};
        let mut player = Player::new(1, Team::GLA, display_name, true);
        let mut side = Dict::new();
        side.set_ascii_string(key_player_name(), "RadarRestoreSlot");
        // The same admitted map side owns both continuations. Display names
        // are mutable save state, not new map Team prototype definitions.
        player.apply_map_side_dict(
            &side,
            false,
            &mut crate::game_logic::host_economy_log::EconomyEvents::default(),
        );
        player
    }

    #[test]
    fn full_restore_retires_outgoing_radar_against_old_owner_and_frame_before_incoming_state() {
        let mut incoming = GameLogic::new();
        incoming.frame = 111;
        let mut player = admitted_map_player("incoming");
        player.radar_count = 1;
        player.disable_proof_radar_count = 1;
        incoming.add_player(player);
        let builder = SnapshotBuilder::new();
        let saved = builder.create_world_snapshot(&incoming).unwrap();
        let encoded = bincode_legacy::serialize(&saved).unwrap();
        let saved = crate::save_load::snapshot::decode_bincode_world_snapshot(&encoded).unwrap();
        let mut outgoing = GameLogic::new();
        outgoing.frame = 0;
        outgoing.host_logic_step_active = true;
        outgoing.add_player(admitted_map_player("outgoing"));
        let incoming_prototypes = incoming.team_factory.lock().unwrap().prototype_names();
        let outgoing_prototypes = outgoing.team_factory.lock().unwrap().prototype_names();
        assert_eq!(incoming_prototypes, ["teamRadarRestoreSlot"]);
        assert_eq!(outgoing_prototypes, incoming_prototypes);
        outgoing
            .victory_conditions
            .set_victory_conditions(crate::game_logic::victory_conditions::VictoryType::empty());
        let mut template = ThingTemplate::new("GLAVehicleRadarVan");
        template.set_health(200.);
        outgoing
            .templates
            .insert(template.name.clone(), template.clone());
        let id = ObjectId(41);
        let mut object = Object::new(template, id, Team::GLA);
        object.construction_percent = 1.;
        object.owner_player_id = Some(1);
        outgoing.add_object(object);
        outgoing.host_radar_add_object(id);
        assert_eq!(outgoing.get_player(1).unwrap().radar_count, 1);
        assert!(
            outgoing.queued_audio_events.is_empty(),
            "frame-zero outgoing lifecycle is silent"
        );
        builder
            .restore_from_snapshot(&saved, &mut outgoing)
            .unwrap();
        assert_eq!(outgoing.frame, 111);
        assert!(outgoing.host_objects().registered_is_empty());
        assert_eq!(outgoing.get_player(1).unwrap().name, "incoming");
        assert_eq!(
            outgoing.get_player(1).unwrap().map_side.map_player_name,
            "RadarRestoreSlot"
        );
        assert_eq!(
            outgoing.team_factory.lock().unwrap().prototype_names(),
            incoming_prototypes
        );
        assert_eq!(
            outgoing.get_player(1).unwrap().radar_count,
            1,
            "outgoing onDelete cannot decrement restored Player state"
        );
        assert_eq!(outgoing.get_player(1).unwrap().disable_proof_radar_count, 1);
        assert!(
            outgoing.queued_audio_events.is_empty(),
            "outgoing callback cannot observe incoming nonzero frame and emit offline sound"
        );
    }
}
