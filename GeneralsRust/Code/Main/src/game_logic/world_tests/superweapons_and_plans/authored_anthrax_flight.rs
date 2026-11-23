/// Authored OCLSpecialPower Anthrax Bomb fixture: the bound module's
/// UpgradeOCL science rows and its admitted DeliverPayload decide the tier —
/// never the source template name, OCL name, or faction guess
/// (C++ OCLSpecialPower.cpp:102-119, Player.cpp:2729-2733).
fn authored_anthrax_world() -> GameLogic {
    let mut world = GameLogic::new();
    world.override_world_size(2000.0, 2000.0);
    ensure_test_player_for_team(&mut world, Team::GLA);
    world.add_player(Player::new(4, Team::GLA, "PeerGLA", false));
    world
        .admit_object_creation_list_sources([r#"
ObjectCreationList CompletelyRenamedBase
  DeliverPayload
    Transport = GLAJetCargoPlane
    Payload = AnthraxBomb
  End
End
ObjectCreationList HarborList
  DeliverPayload
    Transport = GLAJetCargoPlane
    Payload = AnthraxBombGamma
  End
End
"#
        .to_owned()])
        .unwrap();
    world
        .admit_object_ini_sources([(
            "authored-anthrax.ini".to_owned(),
            r#"
Object Chem_DeliveryLookalike
  KindOf = STRUCTURE IMMOBILE
  Body = ActiveBody BodyTag
    MaxHealth = 5000
    InitialHealth = 5000
  End
  Behavior = OCLSpecialPower AnthraxPower
    SpecialPowerTemplate = SuperweaponAnthraxBomb
    UpgradeOCL = SCIENCE_AuthoredFirst CompletelyRenamedBase
    UpgradeOCL = SCIENCE_AuthoredSecond HarborList
    OCL = CompletelyRenamedBase
    CreateLocation = CREATE_AT_EDGE_NEAR_SOURCE
  End
End
Object HarborDispatch
  KindOf = STRUCTURE IMMOBILE
  Body = ActiveBody BodyTag
    MaxHealth = 5000
    InitialHealth = 5000
  End
  Behavior = OCLSpecialPower AnthraxPower
    SpecialPowerTemplate = SuperweaponAnthraxBomb
    OCL = HarborList
    CreateLocation = CREATE_AT_EDGE_NEAR_SOURCE
  End
End
Object QuietDispatch
  KindOf = STRUCTURE IMMOBILE
  Body = ActiveBody BodyTag
    MaxHealth = 5000
    InitialHealth = 5000
  End
  Behavior = OCLSpecialPower AnthraxPower
    SpecialPowerTemplate = SuperweaponAnthraxBomb
    OCL = None
    CreateLocation = CREATE_AT_EDGE_NEAR_SOURCE
  End
End
Object GLAJetCargoPlane
  KindOf = VEHICLE AIRCRAFT
  Body = ActiveBody BodyTag
    MaxHealth = 600
    InitialHealth = 600
  End
End
Object AnthraxBlastTarget
  KindOf = VEHICLE
  Body = ActiveBody BodyTag
    MaxHealth = 1000
    InitialHealth = 1000
  End
End
"#
            .to_owned(),
        )])
        .unwrap();
    world
}

fn anthrax_tier_and_owner(
    world: &GameLogic,
    jet: ObjectId,
) -> (
    crate::game_logic::host_anthrax_bomb_flight::AnthraxBombPayloadTier,
    Option<u32>,
) {
    let transport = world
        .host_object(jet)
        .expect("transport")
        .anthrax_bomb_transport
        .as_ref()
        .expect("anthrax flight data");
    (
        transport.tier,
        world.host_object(jet).unwrap().owner_player_id,
    )
}

#[test]
fn authored_anthrax_bomb_reads_module_upgrade_ocl_and_deliver_payload() {
    use crate::game_logic::host_anthrax_bomb_flight::AnthraxBombPayloadTier;

    // Owner holds both UpgradeOCL sciences: the first eligible row wins —
    // the "Chem_" name never upgrades the payload to Gamma.
    let mut world = authored_anthrax_world();
    {
        let p = world.get_player_mut(2).unwrap();
        p.unlocked_sciences.insert("SCIENCE_AuthoredFirst".into());
        p.unlocked_sciences.insert("SCIENCE_AuthoredSecond".into());
    }
    let source = world
        .create_object_for_player("Chem_DeliveryLookalike", 2, Vec3::ZERO)
        .unwrap();
    let jet = world
        .spawn_anthrax_bomb_flight(source, Vec3::new(400.0, 0.0, 0.0))
        .expect("flight");
    assert_eq!(
        anthrax_tier_and_owner(&world, jet),
        (AnthraxBombPayloadTier::Base, Some(2)),
        "first eligible UpgradeOCL row (Base payload) wins for the exact owner"
    );

    // Only the second science: second eligible row → HarborList Gamma payload.
    let mut world = authored_anthrax_world();
    world
        .get_player_mut(2)
        .unwrap()
        .unlocked_sciences
        .insert("SCIENCE_AuthoredSecond".into());
    let source = world
        .create_object_for_player("Chem_DeliveryLookalike", 2, Vec3::ZERO)
        .unwrap();
    let jet = world
        .spawn_anthrax_bomb_flight(source, Vec3::new(400.0, 0.0, 0.0))
        .expect("flight");
    assert_eq!(
        anthrax_tier_and_owner(&world, jet),
        (AnthraxBombPayloadTier::Gamma, Some(2))
    );

    // The science name as a completed upgrade only is not a science: falls
    // through to the module's own OCL.
    let mut world = authored_anthrax_world();
    world
        .get_player_mut(2)
        .unwrap()
        .completed_upgrades
        .insert("SCIENCE_AuthoredSecond".into());
    let source = world
        .create_object_for_player("Chem_DeliveryLookalike", 2, Vec3::ZERO)
        .unwrap();
    let jet = world
        .spawn_anthrax_bomb_flight(source, Vec3::new(400.0, 0.0, 0.0))
        .expect("flight");
    assert_eq!(
        anthrax_tier_and_owner(&world, jet),
        (AnthraxBombPayloadTier::Base, Some(2)),
        "completed-upgrade possession is not Player::hasScience"
    );

    // Same-faction isolation: peer owns the source, owner holds the science —
    // sciences never union across same-faction players.
    let mut world = authored_anthrax_world();
    world
        .get_player_mut(2)
        .unwrap()
        .unlocked_sciences
        .insert("SCIENCE_AuthoredSecond".into());
    let source = world
        .create_object_for_player("Chem_DeliveryLookalike", 4, Vec3::ZERO)
        .unwrap();
    let jet = world
        .spawn_anthrax_bomb_flight(source, Vec3::new(400.0, 0.0, 0.0))
        .expect("flight");
    assert_eq!(
        anthrax_tier_and_owner(&world, jet),
        (AnthraxBombPayloadTier::Base, Some(4)),
        "peer's sciences must not resolve the owner's UpgradeOCL"
    );

    // HarborList's non-anthrax name still yields Gamma because the payload
    // object is AnthraxBombGamma — payload, not the list name, sets tier.
    let mut world = authored_anthrax_world();
    let source = world
        .create_object_for_player("HarborDispatch", 2, Vec3::ZERO)
        .unwrap();
    let jet = world
        .spawn_anthrax_bomb_flight(source, Vec3::new(400.0, 0.0, 0.0))
        .expect("flight");
    assert_eq!(
        anthrax_tier_and_owner(&world, jet),
        (AnthraxBombPayloadTier::Gamma, Some(2))
    );
}

#[test]
#[cfg(not(target_arch = "wasm32"))]
fn authored_anthrax_bomb_bound_no_ocl_suppresses_invented_blast() {
    use crate::command_system::SpecialPowerType;

    // OCL = None is legal authored data: no flight launches, and the queued
    // strike is still marked live so the registry's invented click blast is
    // suppressed for this authored no-op.
    let mut world = authored_anthrax_world();
    let source = world
        .create_object_for_player("QuietDispatch", 2, Vec3::ZERO)
        .unwrap();
    assert!(
        world
            .spawn_anthrax_bomb_flight(source, Vec3::new(400.0, 0.0, 0.0))
            .is_none(),
        "bound module with OCL=None must not synthesize a flight"
    );
    assert!(
        world
            .host_objects()
            .registered_values()
            .all(|o| o.anthrax_bomb_transport.is_none()),
        "no transport exists after an authored no-op"
    );

    let strike = world
        .queue_special_power_strike(
            &SpecialPowerType::AnthraxBomb,
            source,
            Vec3::new(400.0, 0.0, 0.0),
        )
        .expect("queued anthrax strike");
    assert!(
        world
            .special_power_strikes()
            .get(strike)
            .is_some_and(|s| s.live_anthrax_delivery),
        "authored no-op must still suppress the registry's click blast"
    );

    // Save/restore must keep the suppression: the authored no-op never gains
    // an invented click blast after loading.
    let target = world
        .create_object_for_player("AnthraxBlastTarget", 4, Vec3::new(400.0, 0.0, 0.0))
        .unwrap();
    let impact_frame = world
        .special_power_strikes()
        .get(strike)
        .unwrap()
        .impact_frame;
    use crate::save_load::{SnapshotBuilder, snapshot::decode_bincode_world_snapshot};
    let builder = SnapshotBuilder::new();
    let encoded =
        bincode_legacy::serialize(&builder.create_world_snapshot(&world).unwrap()).unwrap();
    let decoded = decode_bincode_world_snapshot(&encoded).unwrap();
    let mut restored = authored_anthrax_world();
    restored.clear_all_players();
    builder
        .restore_from_snapshot(&decoded, &mut restored)
        .unwrap();
    assert!(
        restored
            .special_power_strikes()
            .get(strike)
            .is_some_and(|s| s.live_anthrax_delivery),
        "restore must retain the authored no-op suppression flag"
    );
    restored.frame = impact_frame;
    restored.update_special_power_strikes().unwrap();
    assert_eq!(
        restored.host_object(target).unwrap().health.current,
        1000.0,
        "authored no-op must not apply the invented blast after restore"
    );
    assert!(
        restored.special_power_strikes().toxin_fields().is_empty(),
        "authored no-op must not spawn a toxin field after restore"
    );
}

/// A launched authored flight's transport + payload tier persist through a
/// snapshot — no actor state is missing that would warrant a live-flag reset.
#[cfg(not(target_arch = "wasm32"))]
#[test]
fn authored_anthrax_bomb_flight_survives_snapshot_restore() {
    use crate::save_load::{SnapshotBuilder, snapshot::decode_bincode_world_snapshot};

    let mut world = authored_anthrax_world();
    let source = world
        .create_object_for_player("HarborDispatch", 2, Vec3::ZERO)
        .unwrap();
    let jet = world
        .spawn_anthrax_bomb_flight(source, Vec3::new(400.0, 0.0, 0.0))
        .expect("flight");
    assert_eq!(
        anthrax_tier_and_owner(&world, jet),
        (
            crate::game_logic::host_anthrax_bomb_flight::AnthraxBombPayloadTier::Gamma,
            Some(2)
        )
    );

    let builder = SnapshotBuilder::new();
    let encoded =
        bincode_legacy::serialize(&builder.create_world_snapshot(&world).unwrap()).unwrap();
    let decoded = decode_bincode_world_snapshot(&encoded).unwrap();
    let mut restored = authored_anthrax_world();
    restored.clear_all_players();
    builder
        .restore_from_snapshot(&decoded, &mut restored)
        .unwrap();
    assert_eq!(
        anthrax_tier_and_owner(&restored, jet),
        (
            crate::game_logic::host_anthrax_bomb_flight::AnthraxBombPayloadTier::Gamma,
            Some(2)
        ),
        "transport and its authored payload tier must survive restore"
    );
}
