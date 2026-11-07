use super::*;

#[test]
fn heal_cave_tunnel_contain_kinds_map_and_heal_contain_auto_exits() {
    use glam::Vec3;

    // C++ HealContain.cpp:68-157 heals then auto-exits.
    // C++ TunnelTracker.cpp:225-268 healObjects sliver / snap-to-max.
    // Pre-fix: spawn_templates mapped Heal/Cave/Tunnel to ContainModuleKind::None.
    const SOURCE: &str = r#"
Object ProbeHealContainBarracks
  KindOf = STRUCTURE SELECTABLE HEAL_PAD
  Body = ActiveBody ModuleTag_01
    MaxHealth = 1000.0
    InitialHealth = 1000.0
  End
  Behavior = HealContain ModuleTag_Heal
    ContainMax = 10
    TimeForFullHeal = 2000
    PassengersInTurret = Yes
    AllowInsideKindOf = INFANTRY
    AllowAlliesInside = Yes
    AllowEnemiesInside = No
    AllowNeutralInside = No
  End
End
Object ProbeCaveContain
  KindOf = STRUCTURE SELECTABLE
  Behavior = CaveContain ModuleTag_Cave
    ContainMax = 10
    PassengersInTurret = No
    CaveIndex = 4
  End
End
Object ProbeTunnelContain
  KindOf = STRUCTURE SELECTABLE
  Body = ActiveBody ModuleTag_Body
    MaxHealth = 100.0
    InitialHealth = 100.0
  End
  Behavior = TunnelContain ModuleTag_Tunnel
    TimeForFullHeal = 5000
  End
End
Object ProbeHealInfantry
  KindOf = INFANTRY SELECTABLE
  TransportSlotCount = 1
  Body = ActiveBody ModuleTag_01
    MaxHealth = 80.0
    InitialHealth = 80.0
  End
End

"#;
    let mut parser = crate::assets::IniParser::new();
    parser
        .parse_ini_content(SOURCE, "heal_cave_tunnel_contain_probe.ini")
        .expect("parse contain kind probe");

    let barracks = GameLogic::build_template_from_object_definition(
        "ProbeHealContainBarracks",
        parser
            .get_definition("ProbeHealContainBarracks")
            .expect("heal barracks"),
        None,
    );
    assert_eq!(barracks.contain_module.kind, ContainModuleKind::Heal);
    assert_eq!(barracks.contain_module.slots, Some(10));
    assert!(barracks.contain_module.passengers_in_turret);
    assert_eq!(barracks.contain_module.frames_for_full_heal, Some(60));
    assert_eq!(
        barracks.contain_module.admission,
        ContainAdmission::InfantryOnly
    );
    assert!(barracks.contain_module.allow_allies_inside);
    assert!(!barracks.contain_module.allow_enemies_inside);
    assert!(!barracks.contain_module.allow_neutral_inside);
    assert!(barracks.garrison_contain_max.is_none());

    let cave = GameLogic::build_template_from_object_definition(
        "ProbeCaveContain",
        parser.get_definition("ProbeCaveContain").expect("cave"),
        None,
    );
    assert_eq!(cave.contain_module.kind, ContainModuleKind::Cave);
    assert!(!cave.contain_module.passengers_in_turret);
    assert_eq!(cave.contain_module.slots, Some(10));
    assert_eq!(cave.contain_module.cave_index, 4);

    let tunnel = GameLogic::build_template_from_object_definition(
        "ProbeTunnelContain",
        parser.get_definition("ProbeTunnelContain").expect("tunnel"),
        None,
    );
    assert_eq!(tunnel.contain_module.kind, ContainModuleKind::Tunnel);
    assert!(!tunnel.contain_module.passengers_in_turret);
    assert_eq!(tunnel.contain_module.frames_for_full_heal, Some(150));

    let mut logic = GameLogic::new();
    let usa = Player::new(1, Team::USA, "USA", true);
    logic.add_player(usa);
    admit_fixture_source(
        &mut logic,
        "heal_cave_tunnel_contain_probe.ini",
        SOURCE,
        &[
            "ProbeHealContainBarracks",
            "ProbeHealInfantry",
            "ProbeTunnelContain",
        ],
    );

    let pad = logic
        .create_object_for_player("ProbeHealContainBarracks", 1, Vec3::ZERO)
        .expect("heal barracks");
    let unit = logic
        .create_object_for_player("ProbeHealInfantry", 1, Vec3::ZERO)
        .expect("infantry");
    // ActiveBody initializes current/previous from InitialHealth, not MaxHealth.
    for (id, health) in [(pad, 1000.0), (unit, 80.0)] {
        let object = logic.host_object(id).expect("live authored fixture");
        assert_eq!(
            (object.health.current, object.health.maximum),
            (health, health)
        );
        assert!(object.is_alive());
    }
    let object = logic.host_object_mut(unit).expect("infantry");
    object.internal_change_health(20.0 - object.health.current);

    assert!(
        logic.can_unit_enter_normal_target(unit, pad),
        "damaged infantry must enter HealContain"
    );
    if let Some(obj) = logic.host_object_mut(unit) {
        obj.internal_change_health(80.0 - obj.health.current);
    }
    assert!(
        !logic.can_unit_enter_normal_target(unit, pad),
        "C++ ActionManager.cpp:636-644 rejects full-health HealContain enter"
    );
    let frame = logic.frame;
    if let Some(obj) = logic.host_object_mut(unit) {
        obj.internal_change_health(20.0 - obj.health.current);
        obj.set_ai_state(
            AIState::Entering,
            frame,
            &mut crate::game_logic::ObjectAudio::record(&mut Vec::new()),
        );
        obj.target = Some(pad);
    }

    logic.frame = 10;
    logic.update_support_states(&[unit], 1.0 / 30.0);
    assert_eq!(
        logic.host_object(unit).and_then(|o| o.contained_by),
        Some(pad)
    );
    let after_enter = logic
        .host_object(unit)
        .map(|o| o.health.current)
        .unwrap_or(0.0);
    // First contained update applies max/60 sliver (HealContain.cpp:148).
    assert!(
        (after_enter - (20.0 + 80.0 / 60.0)).abs() < 0.05 || after_enter > 20.0,
        "HealContain must sliver-heal, got {after_enter}"
    );

    for _ in 0..60 {
        logic.frame = logic.frame.saturating_add(1);
        logic.update_support_states(&[unit], 1.0 / 30.0);
    }
    let after = logic.host_object(unit).expect("unit after heal");
    assert!(
        after.health.current >= after.health.maximum - 0.01,
        "HealContain must finish at max health"
    );
    assert!(
        after.contained_by.is_none(),
        "HealContain.cpp:97-101 auto-exits when done healing"
    );
    assert!(logic.tunnel_network.honesty_heal_contain_auto_exit_ok());

    let tunnel_id = logic
        .create_object_for_player("ProbeTunnelContain", 1, Vec3::new(40.0, 0.0, 0.0))
        .expect("tunnel");
    let rider = logic
        .create_object_for_player("ProbeHealInfantry", 1, Vec3::new(40.0, 0.0, 0.0))
        .expect("tunnel rider");
    for (id, health) in [(tunnel_id, 100.0), (rider, 80.0)] {
        let object = logic.host_object(id).expect("live authored tunnel fixture");
        assert_eq!(
            (object.health.current, object.health.maximum),
            (health, health)
        );
        assert!(object.is_alive());
    }
    let object = logic.host_object_mut(rider).expect("tunnel rider");
    object.internal_change_health(30.0 - object.health.current);
    assert!(logic.tunnel_network.record_enter(
        crate::game_logic::host_tunnel_network::tunnel_system_key(Some(1), Team::USA),
        rider,
        tunnel_id,
    ));
    logic
        .tunnel_network
        .stamp_contained_by_frame(rider, logic.frame);
    let before_tunnel = logic
        .host_object(rider)
        .map(|o| o.health.current)
        .unwrap_or(0.0);
    logic.frame = logic.frame.saturating_add(1);
    logic.update_support_states(&[rider], 1.0 / 30.0);
    let after_tunnel = logic
        .host_object(rider)
        .map(|o| o.health.current)
        .unwrap_or(0.0);
    assert!(
        after_tunnel > before_tunnel,
        "TunnelTracker::healObjects must sliver-heal, {before_tunnel} -> {after_tunnel}"
    );
    assert!(logic.tunnel_network.honesty_heal_objects_ok());
}

#[test]
fn chinook_forbid_huge_vehicle_does_not_fail_close_enter() {
    use crate::game_logic::{ContainAdmission, ContainModuleKind};
    use game_engine::common::system::kind_of::KindOfMask;
    use glam::Vec3;

    // C++ OpenContain.cpp:856-866 + TransportContain.cpp:136-193:
    // AllowInsideKindOf = INFANTRY VEHICLE,
    // ForbidInsideKindOf = AIRCRAFT HUGE_VEHICLE.
    // Infantry and Humvees board; Overlord (HUGE_VEHICLE) does not.
    const SOURCE: &str = r#"
Object ProbeCombatChinook
  KindOf = VEHICLE SELECTABLE TRANSPORT
  TransportSlotCount = 1
  Body = ActiveBody ModuleTag_01
    MaxHealth = 300.0
    InitialHealth = 300.0
  End
  Behavior = TransportContain ModuleTag_Contain
    Slots = 8
    AllowInsideKindOf = INFANTRY VEHICLE
    ForbidInsideKindOf = AIRCRAFT HUGE_VEHICLE
  End
End
Object ProbeChinookRanger
  KindOf = INFANTRY SELECTABLE
  TransportSlotCount = 1
  Body = ActiveBody ModuleTag_01
    MaxHealth = 100.0
    InitialHealth = 100.0
  End
End
Object ProbeChinookHumvee
  KindOf = VEHICLE SELECTABLE
  TransportSlotCount = 1
  Body = ActiveBody ModuleTag_01
    MaxHealth = 200.0
    InitialHealth = 200.0
  End
End
Object ProbeChinookOverlord
  KindOf = VEHICLE SELECTABLE HUGE_VEHICLE
  TransportSlotCount = 1
  Body = ActiveBody ModuleTag_01
    MaxHealth = 1100.0
    InitialHealth = 1100.0
  End
End
"#;
    let mut parser = crate::assets::IniParser::new();
    parser
        .parse_ini_content(SOURCE, "chinook_huge_vehicle_enter.ini")
        .expect("parse chinook huge-vehicle probe");

    let chinook = GameLogic::build_template_from_object_definition(
        "ProbeCombatChinook",
        parser
            .get_definition("ProbeCombatChinook")
            .expect("chinook"),
        None,
    );
    assert_eq!(chinook.contain_module.kind, ContainModuleKind::Transport);
    assert_eq!(
        chinook.contain_module.admission,
        ContainAdmission::InfantryOrVehicle
    );
    assert_ne!(
        chinook.contain_module.forbid_inside_kind_of & KindOfMask::HUGE_VEHICLE.bits(),
        0,
        "ForbidInsideKindOf HUGE_VEHICLE must survive parse"
    );

    let overlord = GameLogic::build_template_from_object_definition(
        "ProbeChinookOverlord",
        parser
            .get_definition("ProbeChinookOverlord")
            .expect("overlord"),
        None,
    );
    assert!(
        overlord.is_kind_of(crate::game_logic::KindOf::HugeVehicle),
        "Overlord KindOf HUGE_VEHICLE must be retained"
    );

    let mut logic = GameLogic::new();
    logic.add_player(Player::new(1, Team::USA, "USA", true));
    admit_fixture_source(
        &mut logic,
        "chinook_huge_vehicle_enter.ini",
        SOURCE,
        &[
            "ProbeCombatChinook",
            "ProbeChinookRanger",
            "ProbeChinookHumvee",
            "ProbeChinookOverlord",
        ],
    );

    let bird = logic
        .create_object_for_player("ProbeCombatChinook", 1, Vec3::ZERO)
        .expect("chinook");
    let infantry = logic
        .create_object_for_player("ProbeChinookRanger", 1, Vec3::ZERO)
        .expect("ranger");
    let vehicle = logic
        .create_object_for_player("ProbeChinookHumvee", 1, Vec3::ZERO)
        .expect("humvee");
    let huge = logic
        .create_object_for_player("ProbeChinookOverlord", 1, Vec3::ZERO)
        .expect("overlord");

    // These admission assertions require live, explicitly authored Bodies.
    for (id, health) in [
        (bird, 300.0),
        (infantry, 100.0),
        (vehicle, 200.0),
        (huge, 1100.0),
    ] {
        let object = logic.host_object(id).expect("live authored fixture");
        assert_eq!(
            (object.health.current, object.health.maximum),
            (health, health)
        );
        assert!(object.is_alive());
    }

    assert!(
        logic.can_unit_enter_normal_target(infantry, bird),
        "Ranger must board Chinook"
    );
    assert!(
        logic.can_unit_enter_normal_target(vehicle, bird),
        "Humvee must board Chinook"
    );
    assert!(
        !logic.can_unit_enter_normal_target(huge, bird),
        "Overlord HUGE_VEHICLE must be forbidden"
    );
}

#[test]
fn unauthored_garrison_transport_default_infantry_only_not_any_mobile() {
    use crate::game_logic::{ContainAdmission, ContainModuleKind};
    use game_engine::common::system::kind_of::KindOfMask;
    use glam::Vec3;

    // C++ GarrisonContain.cpp / TransportContain.cpp / MobNexusContain.cpp
    // ctors and leftover Defaults: allow_inside = KINDOF_INFANTRY.
    // Un-authored AllowInsideKindOf must not become AnyMobile.
    const SOURCE: &str = r#"
Object ProbeDefaultGarrison
  KindOf = STRUCTURE SELECTABLE
  Body = ActiveBody ModuleTag_Body
    MaxHealth = 100.0
    InitialHealth = 100.0
  End
  Behavior = GarrisonContain ModuleTag_Garrison
    ContainMax = 5
  End
End
Object ProbeDefaultTransport
  KindOf = VEHICLE SELECTABLE TRANSPORT
  TransportSlotCount = 1
  Body = ActiveBody ModuleTag_Body
    MaxHealth = 100.0
    InitialHealth = 100.0
  End
  Behavior = TransportContain ModuleTag_Transport
    Slots = 5
  End
End
Object ProbeDefaultCave
  KindOf = STRUCTURE SELECTABLE
  Behavior = CaveContain ModuleTag_Cave
    ContainMax = 10
    CaveIndex = 0
  End
End
Object ProbeDefaultRanger
  KindOf = INFANTRY SELECTABLE
  TransportSlotCount = 1
  Body = ActiveBody ModuleTag_01
    MaxHealth = 100.0
    InitialHealth = 100.0
  End
End
Object ProbeDefaultHumvee
  KindOf = VEHICLE SELECTABLE
  TransportSlotCount = 1
  Body = ActiveBody ModuleTag_01
    MaxHealth = 200.0
    InitialHealth = 200.0
  End
End
"#;
    let mut parser = crate::assets::IniParser::new();
    parser
        .parse_ini_content(SOURCE, "unauthored_allow_inside_kindof_probe.ini")
        .expect("parse unauthored allow-inside probe");

    let garrison = GameLogic::build_template_from_object_definition(
        "ProbeDefaultGarrison",
        parser
            .get_definition("ProbeDefaultGarrison")
            .expect("garrison"),
        None,
    );
    assert_eq!(garrison.contain_module.kind, ContainModuleKind::Garrison);
    assert_eq!(
        garrison.contain_module.admission,
        ContainAdmission::InfantryOnly
    );
    assert_eq!(
        garrison.contain_module.allow_inside_kind_of,
        KindOfMask::INFANTRY.bits()
    );

    let transport = GameLogic::build_template_from_object_definition(
        "ProbeDefaultTransport",
        parser
            .get_definition("ProbeDefaultTransport")
            .expect("transport"),
        None,
    );
    assert_eq!(transport.contain_module.kind, ContainModuleKind::Transport);
    assert_eq!(
        transport.contain_module.admission,
        ContainAdmission::InfantryOnly
    );
    assert_eq!(
        transport.contain_module.allow_inside_kind_of,
        KindOfMask::INFANTRY.bits()
    );

    let cave = GameLogic::build_template_from_object_definition(
        "ProbeDefaultCave",
        parser.get_definition("ProbeDefaultCave").expect("cave"),
        None,
    );
    assert_eq!(cave.contain_module.kind, ContainModuleKind::Cave);
    assert_eq!(cave.contain_module.admission, ContainAdmission::AnyMobile);
    assert_eq!(cave.contain_module.allow_inside_kind_of, 0);

    let mut logic = GameLogic::new();
    logic.add_player(Player::new(1, Team::USA, "USA", true));
    admit_fixture_source(
        &mut logic,
        "unauthored_allow_inside_kindof_probe.ini",
        SOURCE,
        &[
            "ProbeDefaultGarrison",
            "ProbeDefaultTransport",
            "ProbeDefaultRanger",
            "ProbeDefaultHumvee",
        ],
    );

    let bunker = logic
        .create_object_for_player("ProbeDefaultGarrison", 1, Vec3::ZERO)
        .expect("garrison");
    let truck = logic
        .create_object_for_player("ProbeDefaultTransport", 1, Vec3::ZERO)
        .expect("transport");
    let infantry = logic
        .create_object_for_player("ProbeDefaultRanger", 1, Vec3::ZERO)
        .expect("ranger");
    let vehicle = logic
        .create_object_for_player("ProbeDefaultHumvee", 1, Vec3::ZERO)
        .expect("humvee");

    // These admission assertions require live, explicitly authored Bodies.
    for (id, health) in [
        (bunker, 100.0),
        (truck, 100.0),
        (infantry, 100.0),
        (vehicle, 200.0),
    ] {
        let object = logic.host_object(id).expect("live authored fixture");
        assert_eq!(
            (object.health.current, object.health.maximum),
            (health, health)
        );
        assert!(object.is_alive());
    }

    assert!(
        logic.can_unit_enter_normal_target(infantry, bunker),
        "Infantry must enter un-authored GarrisonContain"
    );
    assert!(
        !logic.can_unit_enter_normal_target(vehicle, bunker),
        "Vehicle must not enter un-authored GarrisonContain"
    );
    assert!(
        logic.can_unit_enter_normal_target(infantry, truck),
        "Infantry must enter un-authored TransportContain"
    );
    assert!(
        !logic.can_unit_enter_normal_target(vehicle, truck),
        "Vehicle must not enter un-authored TransportContain"
    );
}

#[test]
fn garrison_initial_roster_parses_template_and_count() {
    let mut parser = crate::assets::IniParser::new();
    parser
        .parse_ini_content(
            r#"
Object ProbeGarrisonRosterBunker
  Type = Structure
  KindOf = STRUCTURE SELECTABLE
  Behavior = GarrisonContain ModuleTag_Garrison
    ContainMax = 5
    InitialRoster = GLAInfantryRebel 3
  End
End
"#,
            "garrison_initial_roster_probe.ini",
        )
        .expect("parse garrison roster probe");

    let bunker = GameLogic::build_template_from_object_definition(
        "ProbeGarrisonRosterBunker",
        parser
            .get_definition("ProbeGarrisonRosterBunker")
            .expect("roster bunker"),
        None,
    );
    assert_eq!(bunker.contain_module.kind, ContainModuleKind::Garrison);
    assert_eq!(
        bunker.contain_module.initial_roster_template,
        "GLAInfantryRebel"
    );
    assert_eq!(bunker.contain_module.initial_roster_count, 3);
}

#[test]
fn contain_module_parses_enter_and_exit_sounds() {
    let mut parser = crate::assets::IniParser::new();
    parser
        .parse_ini_content(
            r#"
Object ProbeGarrisonEnterSound
  Type = Structure
  KindOf = STRUCTURE SELECTABLE
  Behavior = GarrisonContain ModuleTag_Garrison
    ContainMax = 5
    EnterSound = GarrisonEnter
    ExitSound = GarrisonExit
  End
End
Object ProbeTransportEnterSound
  Type = Vehicle
  KindOf = VEHICLE SELECTABLE
  Behavior = TransportContain ModuleTag_Transport
    Slots = 5
    EnterSound = GarrisonEnter
    ExitSound = GarrisonExit
  End
End
Object ProbeHumveeSilentContain
  Type = Vehicle
  KindOf = VEHICLE SELECTABLE
  Behavior = TransportContain ModuleTag_Transport
    Slots = 5
  End
End
"#,
            "contain_enter_exit_sound_probe.ini",
        )
        .expect("parse contain sound probe");

    let bunker = GameLogic::build_template_from_object_definition(
        "ProbeGarrisonEnterSound",
        parser
            .get_definition("ProbeGarrisonEnterSound")
            .expect("garrison sound"),
        None,
    );
    assert_eq!(bunker.contain_module.enter_sound, "GarrisonEnter");
    assert_eq!(bunker.contain_module.exit_sound, "GarrisonExit");

    let transport = GameLogic::build_template_from_object_definition(
        "ProbeTransportEnterSound",
        parser
            .get_definition("ProbeTransportEnterSound")
            .expect("transport sound"),
        None,
    );
    assert_eq!(transport.contain_module.kind, ContainModuleKind::Transport);
    assert_eq!(transport.contain_module.enter_sound, "GarrisonEnter");
    assert_eq!(transport.contain_module.exit_sound, "GarrisonExit");

    let humvee = GameLogic::build_template_from_object_definition(
        "ProbeHumveeSilentContain",
        parser
            .get_definition("ProbeHumveeSilentContain")
            .expect("silent transport"),
        None,
    );
    assert_eq!(humvee.contain_module.kind, ContainModuleKind::Transport);
    assert!(
        humvee.contain_module.enter_sound.is_empty() && humvee.contain_module.exit_sound.is_empty(),
        "Humvee comments EnterSound out — C++ stays silent"
    );
}

#[test]
fn garrison_heal_objects_parses_and_heals_occupants() {
    use glam::Vec3;

    const SOURCE: &str = r#"
Object ProbeGarrisonHealBunker
  KindOf = STRUCTURE SELECTABLE
  Body = ActiveBody ModuleTag_Body
    MaxHealth = 100
    InitialHealth = 100
  End
  Behavior = GarrisonContain ModuleTag_Garrison
    ContainMax = 5
    HealObjects = Yes
    TimeForFullHeal = 2000
  End
End
Object ProbeGarrisonHealInfantry
  KindOf = INFANTRY SELECTABLE
  TransportSlotCount = 1
  Body = ActiveBody ModuleTag_01
    MaxHealth = 80.0
    InitialHealth = 80.0
  End
End
"#;
    let mut parser = crate::assets::IniParser::new();
    parser
        .parse_ini_content(SOURCE, "garrison_heal_objects_probe.ini")
        .expect("parse garrison heal probe");

    let bunker = GameLogic::build_template_from_object_definition(
        "ProbeGarrisonHealBunker",
        parser
            .get_definition("ProbeGarrisonHealBunker")
            .expect("heal bunker"),
        None,
    );
    assert!(bunker.contain_module.heal_objects);
    assert_eq!(bunker.contain_module.frames_for_full_heal, Some(60));

    let mut logic = GameLogic::new();
    admit_fixture_source(
        &mut logic,
        "garrison_heal_objects_probe.ini",
        SOURCE,
        &["ProbeGarrisonHealBunker", "ProbeGarrisonHealInfantry"],
    );

    let pad = logic
        .create_object("ProbeGarrisonHealBunker", Team::USA, Vec3::ZERO)
        .expect("heal bunker");
    let unit = logic
        .create_object("ProbeGarrisonHealInfantry", Team::USA, Vec3::ZERO)
        .expect("infantry");
    assert!(
        logic
            .host_object_mut(pad)
            .expect("bunker")
            .add_occupant(unit)
    );
    if let Some(obj) = logic.host_object_mut(unit) {
        obj.health.current = 20.0;
        obj.health.maximum = 80.0;
    }
    logic.set_fixture_object_contained_by(unit, Some(pad));
    logic
        .tunnel_network
        .stamp_contained_by_frame(unit, logic.frame);
    logic.frame = logic.frame.saturating_add(1);
    logic.update_support_states(&[unit], 1.0 / 30.0);
    let after = logic
        .host_object(unit)
        .map(|o| o.health.current)
        .unwrap_or(0.0);
    let sliver = 80.0 / 60.0;
    assert!(
        (after - (20.0 + sliver)).abs() < 0.05,
        "HealObjects must apply one sliver, not TimeForFullHeal+HealObjects double, got {after}"
    );

    assert!(
        logic
            .host_object(unit)
            .is_some_and(|o| o.contained_by == Some(pad)),
        "garrison heal must not auto-exit like HealContain"
    );
}
