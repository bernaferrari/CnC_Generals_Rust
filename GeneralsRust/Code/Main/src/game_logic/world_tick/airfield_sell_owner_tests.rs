//! BuildAssistant.cpp:1503-1560 and ParkingPlaceBehavior.cpp:614-640.
//! Sale uses the receiving parking roster and real Body/onDie callbacks;
//! neither containment guesses nor a deletion-list flush implements kill().
use super::*;
use glam::Vec3;

const RACK: &str = "OwnedSellRack";
const INSTANT: &str = "OwnedSellInstantAircraft";
const SLOW: &str = "OwnedSellSlowAircraft";
const INACTIVE: &str = "OwnedSellInactiveAircraft";

fn owner() -> (GameLogic, ObjectId) {
    // No AssetManager, process-owned FX/OCL registration, locomotor catalog,
    // shared fixture lock, or synthetic raw Object is needed for this slice.
    let source = r#"
Object OwnedSellRack
  KindOf = STRUCTURE SELECTABLE FS_AIRFIELD
  Body = ActiveBody BodyTag
    MaxHealth = 1000
    InitialHealth = 1000
  End
  Behavior = AIUpdateInterface AiTag
  End
  Behavior = ParkingPlaceBehavior ParkingTag
    NumRows = 1
    NumCols = 4
    HasRunways = Yes
    ParkInHangars = No
  End
End
Object OwnedSellInstantAircraft
  KindOf = VEHICLE AIRCRAFT SELECTABLE
  Body = ActiveBody BodyTag
    MaxHealth = 100
    InitialHealth = 100
  End
  Behavior = JetAIUpdate AiTag
    NeedsRunway = Yes
  End
  Behavior = InstantDeathBehavior DeathTag
  End
End
Object OwnedSellSlowAircraft
  KindOf = VEHICLE AIRCRAFT SELECTABLE
  Body = ActiveBody BodyTag
    MaxHealth = 100
    InitialHealth = 100
  End
  Behavior = JetAIUpdate AiTag
    NeedsRunway = Yes
  End
  Behavior = SlowDeathBehavior DeathTag
    SinkDelay = 200
    SinkRate = 2
    DestructionDelay = 1000
  End
End
Object OwnedSellInactiveAircraft
  KindOf = VEHICLE AIRCRAFT
  Body = InactiveBody BodyTag
  End
  Behavior = InstantDeathBehavior DeathTag
  End
End
"#;
    let mut world = GameLogic::new();
    world
        .admit_receiving_module_content(
            crate::assets::ReceivingObjectContent::from_sources([(
                "owned-airfield-sale.ini".to_owned(),
                source.to_owned(),
            )])
            .unwrap(),
        )
        .unwrap();
    world.add_player(Player::new(0, Team::USA, "ParkingSeller", true));
    let rack = world
        .create_object_for_player(RACK, 0, Vec3::new(100.0, 12.0, 100.0))
        .unwrap();
    assert_eq!(world.airfield_parking_capacity(rack), Some(4));
    world.frame = 40;
    let airfield = world.host_object_mut(rack).unwrap();
    assert!(airfield.has_ai_update_interface());
    airfield.set_ai_state(
        AIState::GuardingArea,
        &mut crate::game_logic::ObjectAudio::record(&mut Vec::new()),
    );
    airfield.last_command_source =
        crate::game_logic::host_command_button_hunt::HUNT_CMD_FROM_SCRIPT;
    (world, rack)
}

fn aircraft(world: &mut GameLogic, template: &str, ground: f32, height: f32) -> ObjectId {
    let id = world
        .create_object_for_player(template, 0, Vec3::new(100.0, height, 100.0))
        .unwrap();
    let (object, partition) = world.host_object_and_partition_mut(id).unwrap();
    object.ground_height = ground;
    object.ground_height_from_terrain = true;
    object.set_position_with_partition_owner(
        Vec3::new(100.0, height, 100.0),
        0,
        &mut Default::default(),
        partition,
    );
    // This classification is not Object::isAboveTerrain. In particular, a
    // position three units above ground is still above terrain when false.
    object.status.airborne_target = false;
    id
}

fn queued(world: &GameLogic) -> Vec<ObjectId> {
    world
        .objects_to_destroy
        .iter()
        .map(|event| event.id)
        .collect()
}

fn assert_instant_die(world: &GameLogic, id: ObjectId) {
    let object = world
        .host_object(id)
        .expect("kill queues physical retirement");
    assert_eq!(object.health.current, 0.0);
    assert!(object.status.on_die_started, "actual Body dispatches onDie");
    assert!(
        object.status.destroyed,
        "authored InstantDeath queues deletion"
    );
    assert_eq!(object.last_damage_timestamp, Some(40));
    assert_eq!(
        object.status.death_type,
        crate::game_logic::host_usa_pilot::HostDeathType::Normal
    );
    assert_eq!(
        queued(world).iter().filter(|&&queued| queued == id).count(),
        1
    );
}

fn ordered_roster() -> (GameLogic, ObjectId, [ObjectId; 3], ObjectId) {
    let (mut world, rack) = owner();
    let earlier_id = aircraft(&mut world, INSTANT, 12.0, 12.0);
    let later_id = aircraft(&mut world, INSTANT, 12.0, 12.0);
    let slow = aircraft(&mut world, SLOW, 12.0, 12.0);
    let unrelated = aircraft(&mut world, INSTANT, 12.0, 12.0);
    let slots = [later_id, slow, earlier_id];
    // Reserve through the real ordered table. Reversed object IDs distinguish
    // authored parking visitation from an ObjectId-sorted containment scan.
    for (index, id) in slots.into_iter().enumerate() {
        assert_eq!(world.reserve_airfield_parking_space(rack, id), Some(index));
        let object = world.host_object(id).unwrap();
        assert_eq!(object.contained_by, None);
        assert_eq!(object.producer_id, Some(rack));
        assert_eq!(object.airfield_parking_space_index, Some(index as u32));
    }
    (world, rack, slots, unrelated)
}

#[test]
fn authored_airfield_sell_dispatches_ordered_die_callbacks_without_flushing_other_deletions() {
    let (mut world, rack, slots, unrelated) = ordered_roster();
    let (mut peer, peer_rack, peer_slots, peer_unrelated) = ordered_roster();
    assert_eq!(
        (rack, slots, unrelated),
        (peer_rack, peer_slots, peer_unrelated)
    );
    world.destroy_object(unrelated);
    assert_eq!(queued(&world), vec![unrelated]);
    assert!(world.start_sell_object(rack));
    assert!(world.host_object(rack).unwrap().status.sold);
    assert_eq!(world.host_object(rack).unwrap().construction_percent, 0.999);
    assert_eq!(world.host_object(rack).unwrap().ai_state(), AIState::Idle);
    assert_eq!(
        world.host_object(rack).unwrap().last_command_source,
        crate::game_logic::host_command_button_hunt::HUNT_CMD_FROM_AI
    );
    assert_instant_die(&world, slots[0]);
    assert_instant_die(&world, slots[2]);
    assert_eq!(queued(&world), vec![unrelated, slots[0], slots[2]]);
    assert_eq!(world.sell_parked_units_killed, 3);
    let slow = world.host_object(slots[1]).unwrap();
    assert_eq!(slow.health.current, 0.0);
    assert!(slow.status.on_die_started);
    assert!(!slow.status.destroyed, "SlowDeath retains its dead body");
    let delay = slow
        .slow_death
        .as_ref()
        .expect("authored SlowDeath actually ran");
    assert_eq!(delay.begin_frame, 40);
    assert_eq!(delay.sink_at_frame, 46);
    assert_eq!(delay.destroy_at_frame, 70);
    assert!(delay.is_active());
    assert!(!world.start_sell_object(rack), "duplicate sale is rejected");
    assert_eq!(queued(&world), vec![unrelated, slots[0], slots[2]]);
    assert!(
        world.host_object(unrelated).is_some(),
        "sale is not a removal phase"
    );
    assert!(!world.host_object(unrelated).unwrap().status.on_die_started);
    assert!(queued(&peer).is_empty());
    assert_eq!(peer.sell_parked_units_killed, 0);
    assert!(!peer.host_object(peer_rack).unwrap().status.sold);
    for id in peer_slots {
        let object = peer.host_object(id).unwrap();
        assert_eq!(object.health.current, 100.0);
        assert!(!object.status.on_die_started);
        assert!(object.slow_death.is_none());
        assert!(peer.airfield_has_reserved_space(peer_rack, id));
    }
    world.process_destroy_list();
    for id in [unrelated, slots[0], slots[2]] {
        assert!(world.host_object(id).is_none());
        assert!(
            peer.host_object(id).is_some(),
            "same-ID peer is not retired"
        );
    }
    assert!(
        world.host_object(slots[1]).is_some(),
        "delayed body remains"
    );
    peer.reset();
    drop(peer);
    assert_eq!(
        world
            .host_object(slots[1])
            .unwrap()
            .slow_death
            .as_ref()
            .unwrap()
            .destroy_at_frame,
        70
    );
}

#[test]
fn authored_airfield_sell_uses_height_above_ground_and_actual_takeoff_landing_flags() {
    // C++ JetAIUpdate.h:106 returns TAKEOFF || LANDING; reserving a runway
    // alone is not either state. Ground height is independent of world Y=5.
    for (label, ground, height, takeoff, landing, runway, dies) in [
        ("raised_ground", 12.0, 12.0, false, false, false, true),
        ("low_airborne", 0.0, 3.0, false, false, false, false),
        ("airborne_takeoff", 0.0, 20.0, true, false, false, true),
        ("airborne_landing", 0.0, 20.0, false, true, false, true),
        ("runway_only", 0.0, 20.0, false, false, true, false),
    ] {
        let (mut world, rack) = owner();
        let id = aircraft(&mut world, INSTANT, ground, height);
        assert_eq!(world.reserve_airfield_parking_space(rack, id), Some(0));
        {
            let object = world.host_object_mut(id).unwrap();
            object.jet_ai.takeoff_in_progress = takeoff;
            object.jet_ai.landing_in_progress = landing;
            assert_eq!(object.is_above_terrain(), height > ground, "{label}");
        }
        if runway {
            assert_eq!(world.reserve_airfield_runway(rack, id), Some(0));
        }
        assert!(world.start_sell_object(rack), "{label}");
        if dies {
            assert_instant_die(&world, id);
            assert!(!world.airfield_has_reserved_space(rack, id), "{label}");
        } else {
            let object = world.host_object(id).unwrap();
            assert_eq!(object.health.current, 100.0, "{label}");
            assert!(!object.status.on_die_started, "{label}");
            assert!(world.airfield_has_reserved_space(rack, id), "{label}");
            assert_eq!(object.producer_id, Some(rack), "{label}");
            assert_eq!(object.airfield_parking_space_index, Some(0), "{label}");
            assert!(queued(&world).is_empty(), "{label}");
            if runway {
                assert!(world.jet_holds_airfield_runway(rack, id));
            }
        }
    }
}

#[test]
fn authored_airfield_sell_does_not_treat_containment_links_as_parking_reservations() {
    let (mut world, rack) = owner();
    let reserved = aircraft(&mut world, INSTANT, 0.0, 0.0);
    assert_eq!(
        world.reserve_airfield_parking_space(rack, reserved),
        Some(0)
    );
    let unrostered = aircraft(&mut world, INSTANT, 0.0, 0.0);
    let inactive = aircraft(&mut world, INACTIVE, 0.0, 0.0);
    // These counterexamples share the field link, but were never admitted to
    // a ParkingPlaceInfo. The field does not author an OpenContain module.
    for id in [unrostered, inactive] {
        world.set_fixture_object_contained_by(id, Some(rack));
        assert!(!world.airfield_has_reserved_space(rack, id));
    }
    let object = world.host_object(inactive).unwrap();
    assert!(object.is_inactive_body());
    assert_eq!(object.health.maximum, 0.0);
    assert!(!object.is_alive());
    assert!(
        world
            .host_object(rack)
            .unwrap()
            .contained_units()
            .is_empty()
    );
    assert!(world.start_sell_object(rack));
    assert_instant_die(&world, reserved);
    assert_eq!(queued(&world), vec![reserved]);
    let object = world.host_object(unrostered).unwrap();
    assert_eq!(object.health.current, 100.0);
    assert!(!object.status.on_die_started);
    assert!(!object.status.destroyed);
    assert_eq!(object.contained_by, Some(rack));
    let object = world.host_object(inactive).unwrap();
    assert_eq!(object.health.current, 0.0);
    assert!(
        !object.status.on_die_started,
        "effectively dead is skipped, not killed again"
    );
    assert!(!object.status.destroyed);
    world.process_destroy_list();
    assert!(world.host_object(reserved).is_none());
    assert!(world.host_object(unrostered).is_some());
    assert!(world.host_object(inactive).is_some());
}

#[test]
fn authored_airfield_sell_keeps_the_reservation_when_the_actual_body_rejects_kill() {
    let (mut world, rack) = owner();
    let id = aircraft(&mut world, INSTANT, 12.0, 12.0);
    assert_eq!(world.reserve_airfield_parking_space(rack, id), Some(0));
    assert_eq!(world.reserve_airfield_runway(rack, id), Some(0));
    world.host_object_mut(id).unwrap().indestructible = true;
    assert!(world.start_sell_object(rack));
    let object = world.host_object(id).unwrap();
    assert_eq!(object.health.current, 100.0);
    assert!(!object.status.on_die_started);
    assert!(!object.status.destroyed);
    assert_eq!(object.producer_id, Some(rack));
    assert_eq!(object.airfield_parking_space_index, Some(0));
    assert!(world.airfield_has_reserved_space(rack, id));
    assert!(world.jet_holds_airfield_runway(rack, id));
    assert_eq!(world.airfield_parking_spaces[&rack][0].object_id, Some(id));
    assert!(queued(&world).is_empty());
    assert_eq!(world.sell_parked_units_killed, 0);
}

#[test]
fn authored_airfield_sell_does_not_eject_real_docked_aircraft_through_nonexistent_open_contain() {
    let (mut world, rack) = owner();
    assert_eq!(
        world
            .host_object(rack)
            .unwrap()
            .thing()
            .template
            .contain_module
            .kind,
        crate::game_logic::ContainModuleKind::None
    );
    let dying = aircraft(&mut world, INSTANT, 12.0, 12.0);
    let survivor = aircraft(&mut world, INSTANT, 12.0, 12.0);
    let death_pad = Vec3::new(120.0, 12.0, 100.0);
    let survivor_pad = Vec3::new(140.0, 12.0, 100.0);
    for (index, id, pad) in [(0, dying, death_pad), (1, survivor, survivor_pad)] {
        assert_eq!(world.reserve_airfield_parking_space(rack, id), Some(index));
        assert!(world.dock_jet_at_airfield_pad_with_effects(
            id,
            rack,
            pad,
            &mut Default::default()
        ));
        let object = world.host_object(id).unwrap();
        assert_eq!(object.get_position(), pad);
        assert_eq!(object.contained_by, Some(rack));
        assert_eq!(object.ai_state(), AIState::Docked);
        assert!(!object.is_above_terrain());
        assert!(world.airfield_has_reserved_space(rack, id));
    }
    // Docking's real presentation/containment mirror is present even though
    // this authored ParkingPlace has no OpenContain interface to notify.
    assert_eq!(
        world
            .host_object(rack)
            .unwrap()
            .building_data
            .as_ref()
            .unwrap()
            .garrisoned_units,
        vec![dying, survivor]
    );
    world.host_object_mut(survivor).unwrap().indestructible = true;
    assert!(world.start_sell_object(rack));
    assert_instant_die(&world, dying);
    let dead = world.host_object(dying).unwrap();
    assert_eq!(
        dead.get_position(),
        death_pad,
        "onDie receives the actual docked pose"
    );
    assert_eq!(
        dead.last_damage_info_type,
        Some(crate::game_logic::combat::DamageType::Unresistable)
    );
    assert_eq!(queued(&world), vec![dying]);
    let live = world.host_object(survivor).unwrap();
    assert_eq!(live.health.current, 100.0);
    assert!(!live.status.on_die_started);
    assert_eq!(live.last_damage_info_type, None);
    assert_eq!(
        live.get_position(),
        survivor_pad,
        "no synthetic passenger ejection"
    );
    assert_eq!(
        live.ai_state(),
        AIState::Docked,
        "no nonexistent OpenContain aiIdle"
    );
    assert_eq!(live.contained_by, Some(rack));
    assert_eq!(live.producer_id, Some(rack));
    assert_eq!(live.airfield_parking_space_index, Some(1));
    assert!(world.airfield_has_reserved_space(rack, survivor));
    assert_eq!(world.sell_passengers_ejected, 0);
    assert_eq!(world.sell_parked_units_killed, 1);
    assert_eq!(
        world
            .host_object(rack)
            .unwrap()
            .building_data
            .as_ref()
            .unwrap()
            .garrisoned_units,
        vec![survivor]
    );
}

#[path = "airfield_sell_save_owner_tests.rs"]
mod save_owner_tests;

#[path = "airfield_sell_death_owner_tests.rs"]
mod death_owner_tests;

/// Exercise the real parking entry points, authored FX catalog, receiving tree
/// buffer and ordinary audio admission. No test-only FX executor participates.
#[cfg(feature = "game_client")]
#[test]
fn airfield_pose_callbacks_deliver_tree_fx_to_the_receiving_owner_before_parking_tails() {
    use game_engine::common::audio::{AudioEventRts, AudioSubmissionLocality};
    use game_engine::common::ini::ini_fx_list::{FXList, FXNugget};
    #[derive(Default)]
    struct Audio(Vec<String>);
    impl crate::game_logic::NativeAudioEffects for Audio {
        fn add_audio_event(&mut self, event: &AudioEventRts, _: &AudioSubmissionLocality) {
            self.0.push(event.get_event_name().to_owned());
        }
    }
    for entry in 0..3 {
        let (mut world, rack) = owner();
        let (mut peer, peer_rack) = owner();
        let jet = aircraft(&mut world, INSTANT, 12.0, 12.0);
        let peer_jet = aircraft(&mut peer, INSTANT, 12.0, 12.0);
        assert_eq!((rack, jet), (peer_rack, peer_jet));
        assert_eq!(world.reserve_airfield_parking_space(rack, jet), Some(0));
        let info = world.calc_airfield_pp_info(rack, jet).unwrap();
        let position = match entry {
            0 => Vec3::new(120.0, 12.0, 100.0),
            1 => info.hangar_internal,
            _ => {
                assert!(world.dock_jet_at_airfield_pad(jet, rack, info.parking_space));
                // The runway entry fixes deck height before arming the taxi.
                let mut position = world.host_object(jet).unwrap().get_position();
                position.y = info.runway_start.y;
                position
            }
        };
        {
            let (object, partition) = world.host_object_and_partition_mut(jet).unwrap();
            object.selection_radius = 8.0;
            object.crusher_level = 2;
            object.movement.velocity = Vec3::new(4.0, 0.0, 0.0);
            object.set_position_with_partition_owner(
                position - Vec3::new(if entry == 2 { 0.0 } else { 20.0 }, 1.0, 0.0),
                0,
                &mut Default::default(),
                partition,
            );
        }
        world
            .world_services
            .shroud()
            .lock()
            .unwrap()
            .init_shroud_grid(1000.0, 1000.0);
        world
            .world_services
            .shroud()
            .lock()
            .unwrap()
            .reveal_map_for_player_permanently(0)
            .unwrap();
        let mut definition = FXList::new("AirfieldPositionTree".into());
        definition.add_nugget(FXNugget::Sound {
            name: "AirfieldTreeSound".into(),
        });
        let mut catalog = game_client::fx_list::FXListStore::new();
        catalog.add_fx_list(
            "AirfieldPositionTree".into(),
            game_client::fx_list::FXList::from_definition(&definition),
        );
        world.admit_fx_catalog(catalog);
        let mut terrain = game_client::terrain::terrain_visual::TerrainVisualImpl::new();
        terrain
            .tree_buffer_mut()
            .set_bounds(game_client::terrain::TreeRegion2D::new(
                glam::Vec2::splat(-1000.0),
                glam::Vec2::splat(1000.0),
            ));
        let mut data = game_client::terrain::TreeModuleData::default();
        data.model_name = "AirfieldPositionTree".into();
        data.do_topple = true;
        data.topple_fx = Some("AirfieldPositionTree".into());
        terrain
            .tree_buffer_mut()
            .add_tree(
                900,
                Vec3::new(position.x - 1.0, position.z, position.y),
                1.0,
                0.0,
                1.0,
                data,
                game_client::terrain::TreeSphere {
                    center: Vec3::ZERO,
                    radius: 5.0,
                },
            )
            .unwrap();
        let peer_position = peer.host_object(peer_jet).unwrap().get_position();
        let mut audio = Audio::default();
        let mut effects = crate::game_logic::ScriptPresentationEffects {
            terrain: Some(&mut terrain),
            audio: Some(&mut audio),
            ..Default::default()
        };
        let completed = match entry {
            0 => world.dock_jet_at_airfield_pad_with_effects(jet, rack, position, &mut effects),
            1 => world.place_produced_jet_at_parking_pose_with_effects(rack, jet, &mut effects),
            _ => world.try_runway_takeoff_from_airfield_with_effects(jet, &mut effects),
        };
        assert!(completed, "authored airfield entry {entry}");
        drop(effects);
        if entry < 2 {
            assert_eq!(
                audio.0,
                ["AirfieldTreeSound"],
                "entry {entry} executes actual immediate FX"
            );
        } else {
            assert!(
                audio.0.is_empty(),
                "C++ integer XY unchanged: vertical deck correction has no tree notification"
            );
        }
        assert!(terrain.tree_buffer_mut().pending_fx_events().is_empty());
        assert_eq!(
            peer.host_object(peer_jet).unwrap().get_position(),
            peer_position
        );
        assert_eq!(world.host_object(jet).unwrap().get_position(), position);
        if entry < 2 {
            assert_eq!(world.host_object(jet).unwrap().contained_by, Some(rack));
            assert_eq!(world.host_object(jet).unwrap().ai_state(), AIState::Docked);
        }
        let pose = world.host_object(jet).unwrap().get_position();
        peer.reset();
        drop(peer);
        assert_eq!(world.host_object(jet).unwrap().get_position(), pose);
    }
}

#[test]
fn airfield_producer_receipt_accepts_queued_dead_but_rejects_equal_id_peer_birth() {
    let (mut world, rack) = owner();
    let birth =
        std::sync::Arc::downgrade(&world.host_object(rack).unwrap().visual_draw_identity_token);
    world.host_object_mut(rack).unwrap().status.destroyed = true;
    assert!(world.airfield_producer_is_current(rack, &birth));
    let (peer, peer_rack) = owner();
    assert_eq!(rack, peer_rack);
    world
        .objects
        .insert(rack, peer.host_object(peer_rack).unwrap().clone());
    assert!(!world.airfield_producer_is_current(rack, &birth));
}
