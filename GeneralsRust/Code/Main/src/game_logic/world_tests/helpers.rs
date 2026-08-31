//! Shared helpers for host GameLogic unit tests.
#![allow(unused_imports, non_snake_case, unused_variables, dead_code)]
use super::super::*;

// Child module of game_logic.rs via `#[path]`.

pub(super) static HOST_STATE_RESIDUAL_TEST_ENV_LOCK: std::sync::Mutex<()> =
    std::sync::Mutex::new(());

/// Install a tall ridge through the pathfinding-grid mid X cells so
/// `is_clear_line_of_sight_terrain` fails between low endpoints on either side.
pub(super) fn install_test_mid_ridge(game_logic: &mut GameLogic) {
    let w = game_logic.pathfinding_system.grid.width().max(8) as u32;
    let h = game_logic.pathfinding_system.grid.height().max(8) as u32;
    let mut heights = vec![0.0f32; (w * h) as usize];
    let mid = w / 2;
    for y in 0..h {
        for x in mid.saturating_sub(1)..=(mid + 1).min(w - 1) {
            heights[(y * w + x) as usize] = 80.0;
        }
    }
    assert!(
        game_logic.restore_terrain_heights_from_grid(w, h, &heights),
        "height cache install"
    );
}

pub(super) fn ensure_test_tank_template(game_logic: &mut GameLogic) {
    if game_logic.templates.contains_key("TestTank") {
        return;
    }

    let mut test_tank = ThingTemplate::new("TestTank");
    test_tank
        .add_kind_of(KindOf::Vehicle)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .add_kind_of(KindOf::Score)
        .set_health(250.0)
        .set_cost(600, 0);
    // Explicit C++ TransportSlotCount test fixture; tanks consume three
    // passenger slots but are not containers themselves.
    test_tank.transport_slot_count = Some(3);
    game_logic
        .templates
        .insert("TestTank".to_string(), test_tank);
}

pub(super) fn ensure_test_dozer_template(game_logic: &mut GameLogic) {
    if game_logic.templates.contains_key("TestDozer") {
        return;
    }

    let mut test_dozer = ThingTemplate::new("TestDozer");
    test_dozer
        .add_kind_of(KindOf::Vehicle)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Worker)
        .add_kind_of(KindOf::Dozer)
        .add_kind_of(KindOf::Score)
        .set_health(300.0)
        .set_cost(1000, 0);
    game_logic
        .templates
        .insert("TestDozer".to_string(), test_dozer);
}

pub(super) fn ensure_test_infantry_template(game_logic: &mut GameLogic) {
    if game_logic.templates.contains_key("TestInfantry") {
        return;
    }

    let mut test_infantry = ThingTemplate::new("TestInfantry");
    test_infantry
        .add_kind_of(KindOf::Infantry)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .add_kind_of(KindOf::Score)
        .set_health(80.0)
        .set_cost(100, 0);
    test_infantry.transport_slot_count = Some(1);
    // Test fixtures model the explicit Object INI capture module rather than
    // relying on the `TestInfantry` basename.  Individual tests can pause
    // this power to exercise upgrade/readiness behavior.
    test_infantry.capture_power = CapturePowerKind::Ranger;
    test_infantry.capture_start_ability_range = Some(5.0);
    test_infantry.capture_unpack_time_ms = Some(3_000);
    test_infantry.capture_preparation_time_ms = Some(20_000);
    test_infantry.capture_pack_time_ms = Some(2_000);
    game_logic
        .templates
        .insert("TestInfantry".to_string(), test_infantry);
}

pub(super) fn ensure_test_aircraft_template(game_logic: &mut GameLogic) {
    if game_logic.templates.contains_key("TestAircraft") {
        return;
    }

    let mut test_aircraft = ThingTemplate::new("TestAircraft");
    test_aircraft
        // Retail airframes carry VEHICLE as well as AIRCRAFT; C++
        // canGetRepairedAt checks VEHICLE before the airfield branch.
        .add_kind_of(KindOf::Vehicle)
        .add_kind_of(KindOf::Aircraft)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .set_health(220.0)
        .set_cost(1200, 0);
    game_logic
        .templates
        .insert("TestAircraft".to_string(), test_aircraft);
}

pub(super) fn ensure_test_structure_template(game_logic: &mut GameLogic) {
    if game_logic.templates.contains_key("TestBuilding") {
        return;
    }

    let mut test_building = ThingTemplate::new("TestBuilding");
    test_building
        .add_kind_of(KindOf::Structure)
        .add_kind_of(KindOf::MpCountForVictory)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .set_health(1200.0)
        .set_cost(500, -1);
    test_building.capturable = true;
    game_logic
        .templates
        .insert("TestBuilding".to_string(), test_building);
}

pub(super) fn ensure_test_command_center_template(game_logic: &mut GameLogic) {
    if game_logic.templates.contains_key("TestCommandCenter") {
        return;
    }

    let mut command_center = ThingTemplate::new("TestCommandCenter");
    command_center
        .add_kind_of(KindOf::Structure)
        .add_kind_of(KindOf::CommandCenter)
        .add_kind_of(KindOf::MpCountForVictory)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .set_health(1800.0)
        .set_cost(2000, -10);
    game_logic
        .templates
        .insert("TestCommandCenter".to_string(), command_center);
}

pub(super) fn ensure_test_barracks_template(game_logic: &mut GameLogic) {
    if game_logic.templates.contains_key("TestBarracks") {
        return;
    }

    let mut barracks = ThingTemplate::new("TestBarracks");
    barracks
        .add_kind_of(KindOf::Structure)
        .add_kind_of(KindOf::FSBarracks)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .set_health(1000.0)
        .set_cost(600, -1);
    barracks.capturable = true;
    game_logic
        .templates
        .insert("TestBarracks".to_string(), barracks);
}

pub(super) fn ensure_test_garrison_template(game_logic: &mut GameLogic) {
    if game_logic.templates.contains_key("TestBunker") {
        return;
    }

    let mut garrison = ThingTemplate::new("TestBunker");
    garrison
        .add_kind_of(KindOf::Structure)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .set_health(1000.0)
        .set_cost(0, 0);
    garrison.contain_module = ContainModuleMetadata {
        kind: ContainModuleKind::Garrison,
        slots: Some(5),
        admission: ContainAdmission::InfantryOnly,
        allow_allies_inside: true,
        allow_enemies_inside: true,
        allow_neutral_inside: true,
        ..ContainModuleMetadata::default()
    };
    game_logic
        .templates
        .insert("TestBunker".to_string(), garrison);
}

/// Residual Humvee-style transport (vehicle container with explicit slot capacity).
/// Fail-closed: not Chinook multi-door / air path parity.

/// Residual Humvee-style transport (vehicle container with explicit slot capacity).
/// Fail-closed: not Chinook multi-door / air path parity.
pub(super) fn ensure_test_transport_template(game_logic: &mut GameLogic) {
    if game_logic.templates.contains_key("TestTransport") {
        return;
    }

    let mut transport = ThingTemplate::new("TestTransport");
    transport
        .add_kind_of(KindOf::Vehicle)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .set_health(300.0)
        .set_cost(800, 0);
    transport.contain_module = ContainModuleMetadata {
        kind: ContainModuleKind::Transport,
        slots: Some(1),
        admission: ContainAdmission::AnyMobile,
        allow_allies_inside: true,
        allow_enemies_inside: true,
        allow_neutral_inside: true,
        ..ContainModuleMetadata::default()
    };
    game_logic
        .templates
        .insert("TestTransport".to_string(), transport);
}

/// Spawn a residual transport with explicit infantry capacity.

/// Spawn a residual transport with explicit infantry capacity.
pub(super) fn create_test_transport(
    game_logic: &mut GameLogic,
    pos: Vec3,
    capacity: usize,
) -> ObjectId {
    ensure_test_transport_template(game_logic);
    let id = game_logic
        .create_object("TestTransport", Team::USA, pos)
        .expect("TestTransport");
    if let Some(obj) = game_logic.host_object_mut(id) {
        obj.max_transport = capacity;
    }
    id
}

/// Residual China Overlord tank (OverlordContain style vehicle).
/// Fail-closed: not portable-structure payload / W3D rider draw parity.

/// Residual China Overlord tank (OverlordContain style vehicle).
/// Fail-closed: not portable-structure payload / W3D rider draw parity.
pub(super) fn ensure_test_overlord_template(game_logic: &mut GameLogic) {
    if game_logic.templates.contains_key("TestOverlord") {
        return;
    }

    let mut overlord = ThingTemplate::new("TestOverlord");
    overlord
        .add_kind_of(KindOf::Vehicle)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .set_health(1100.0)
        .set_cost(2000, 0);
    game_logic
        .templates
        .insert("TestOverlord".to_string(), overlord);
}

/// Spawn residual Overlord. Without BattleBunker residual (`Some(0)`),
/// infantry enter is rejected. Call `install_overlord_battle_bunker(5)`
/// to match ChinaTankOverlordBattleBunker TransportContain Slots=5.

/// Spawn residual Overlord. Without BattleBunker residual (`Some(0)`),
/// infantry enter is rejected. Call `install_overlord_battle_bunker(5)`
/// to match ChinaTankOverlordBattleBunker TransportContain Slots=5.
pub(super) fn create_test_overlord(
    game_logic: &mut GameLogic,
    pos: Vec3,
    bunker_slots: Option<usize>,
) -> ObjectId {
    ensure_test_overlord_template(game_logic);
    let id = game_logic
        .create_object("TestOverlord", Team::China, pos)
        .expect("TestOverlord");
    if let Some(obj) = game_logic.host_object_mut(id) {
        // Mark overlord-style residual; slots=None means no bunker installed.
        obj.overlord_bunker_capacity = Some(bunker_slots.unwrap_or(0));
    }
    id
}

/// Residual GLA Battle Bus template (TransportContain Slots=8 residual).
/// Fail-closed: not SlowDeath undeath / multi-door exit matrix.

/// Residual GLA Battle Bus template (TransportContain Slots=8 residual).
/// Fail-closed: not SlowDeath undeath / multi-door exit matrix.
pub(super) fn ensure_test_battle_bus_template(game_logic: &mut GameLogic) {
    if game_logic.templates.contains_key("GLAVehicleBattleBus") {
        return;
    }

    let mut bus = ThingTemplate::new("GLAVehicleBattleBus");
    bus.add_kind_of(KindOf::Vehicle)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .set_health(400.0)
        .set_cost(1000, 0);
    game_logic
        .templates
        .insert("GLAVehicleBattleBus".to_string(), bus);
}

/// Spawn residual GLA Battle Bus with C++ TransportContain residual installed.

/// Spawn residual GLA Battle Bus with C++ TransportContain residual installed.
pub(super) fn create_test_battle_bus(game_logic: &mut GameLogic, pos: Vec3) -> ObjectId {
    ensure_test_battle_bus_template(game_logic);
    let id = game_logic
        .create_object("GLAVehicleBattleBus", Team::GLA, pos)
        .expect("GLAVehicleBattleBus");
    // create_object auto-installs Battle Bus residual via template name.
    // Fail-closed reinstall for test honesty if auto-bind missed.
    if let Some(obj) = game_logic.host_object_mut(id) {
        if !obj.is_battle_bus_style_container() {
            obj.install_battle_bus_transport();
        }
    }
    id
}

/// Residual GLA Tunnel Network structure (TunnelContain MaxTunnelCapacity=10).
/// Fail-closed: not GuardTunnelNetwork AI / CaveSystem / TimeForFullHeal.

/// Residual GLA Tunnel Network structure (TunnelContain MaxTunnelCapacity=10).
/// Fail-closed: not GuardTunnelNetwork AI / CaveSystem / TimeForFullHeal.
pub(super) fn ensure_test_tunnel_network_template(game_logic: &mut GameLogic) {
    if game_logic.templates.contains_key("GLATunnelNetwork") {
        return;
    }
    let mut tunnel = ThingTemplate::new("GLATunnelNetwork");
    tunnel
        .add_kind_of(KindOf::Structure)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .set_health(1000.0)
        .set_cost(800, 0);
    // Retail Object GLATunnelNetwork authors Body = TunnelContain
    // (ContainMax 10).  Authoring the module kind keeps the tunnel out of the
    // GarrisonContain branch (`is_garrison_contain`, orders.rs:798) so exit
    // walks the OpenContain ExitStart/End path instead of the Idle drop, and
    // occupants stay DISABLED_HELD per TunnelContain::isGarrisonable FALSE.
    tunnel.contain_module.kind = crate::game_logic::ContainModuleKind::Tunnel;
    tunnel.contain_module.slots =
        Some(crate::game_logic::host_tunnel_network::MAX_TUNNEL_CAPACITY);
    game_logic
        .templates
        .insert("GLATunnelNetwork".to_string(), tunnel);
}

/// Spawn residual GLA Tunnel Network entrance with TunnelContain residual.
pub(super) fn create_test_tunnel_network(game_logic: &mut GameLogic, pos: Vec3) -> ObjectId {
    ensure_test_tunnel_network_template(game_logic);
    let id = game_logic
        .create_object("GLATunnelNetwork", Team::GLA, pos)
        .expect("GLATunnelNetwork");
    if let Some(obj) = game_logic.host_object_mut(id) {
        // The authored TunnelContain kind already marks the template
        // tunnel-style; install unconditionally so the entrance carries the
        // shared MaxTunnelCapacity building_data pool residual.
        obj.install_tunnel_network_residual();
    }
    id
}

/// Residual AirF Combat Chinook template (TransportContain Slots=8 + fire residual).
/// Fail-closed: not ChinookAIUpdate ropes / supply / rappel / combat drop.

/// Residual AirF Combat Chinook template (TransportContain Slots=8 + fire residual).
/// Fail-closed: not ChinookAIUpdate ropes / supply / rappel / combat drop.
pub(super) fn ensure_test_combat_chinook_template(game_logic: &mut GameLogic) {
    if game_logic
        .templates
        .contains_key("AirF_AmericaVehicleChinook")
    {
        return;
    }

    let mut chinook = ThingTemplate::new("AirF_AmericaVehicleChinook");
    chinook
        .add_kind_of(KindOf::Vehicle)
        .add_kind_of(KindOf::Aircraft)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .set_health(350.0)
        .set_cost(1200, 0);
    game_logic
        .templates
        .insert("AirF_AmericaVehicleChinook".to_string(), chinook);
}

/// Spawn residual AirF Combat Chinook with TransportContain residual installed.

/// Spawn residual AirF Combat Chinook with TransportContain residual installed.
pub(super) fn create_test_combat_chinook(game_logic: &mut GameLogic, pos: Vec3) -> ObjectId {
    ensure_test_combat_chinook_template(game_logic);
    let id = game_logic
        .create_object("AirF_AmericaVehicleChinook", Team::USA, pos)
        .expect("AirF_AmericaVehicleChinook");
    if let Some(obj) = game_logic.host_object_mut(id) {
        if !obj.is_combat_chinook_style_container() {
            obj.install_combat_chinook_transport();
        }
    }
    id
}

/// Residual China Listening Outpost template (detect 300 + transport Slots=2).
/// Fail-closed: not IR FX / multi-door / RIDERS_ATTACKING uncloak matrix.

/// Residual China Listening Outpost template (detect 300 + transport Slots=2).
/// Fail-closed: not IR FX / multi-door / RIDERS_ATTACKING uncloak matrix.
pub(super) fn ensure_test_listening_outpost_template(game_logic: &mut GameLogic) {
    if game_logic
        .templates
        .contains_key("ChinaVehicleListeningOutpost")
    {
        return;
    }
    let mut outpost = ThingTemplate::new("ChinaVehicleListeningOutpost");
    outpost
        .add_kind_of(KindOf::Vehicle)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .set_health(240.0)
        .set_cost(800, 0);
    game_logic
        .templates
        .insert("ChinaVehicleListeningOutpost".to_string(), outpost);
}

/// Spawn residual China Listening Outpost with detect + transport residual.
/// InitialPayload TankHunter × 2 docks on create when payload template available.

/// Spawn residual China Listening Outpost with detect + transport residual.
/// InitialPayload TankHunter × 2 docks on create when payload template available.
pub(super) fn create_test_listening_outpost(game_logic: &mut GameLogic, pos: Vec3) -> ObjectId {
    ensure_test_listening_outpost_template(game_logic);
    let id = game_logic
        .create_object("ChinaVehicleListeningOutpost", Team::China, pos)
        .expect("ChinaVehicleListeningOutpost");
    if let Some(obj) = game_logic.host_object_mut(id) {
        if !obj.is_listening_outpost_style_container() {
            obj.install_listening_outpost_transport();
        }
    }
    id
}

/// Residual China Troop Crawler template (TransportContain Slots=8 residual).

/// Residual China Troop Crawler template (TransportContain Slots=8 residual).
pub(super) fn ensure_test_troop_crawler_template(game_logic: &mut GameLogic) {
    if game_logic
        .templates
        .contains_key("ChinaVehicleTroopCrawler")
    {
        return;
    }
    let mut crawler = ThingTemplate::new("ChinaVehicleTroopCrawler");
    crawler
        .add_kind_of(KindOf::Vehicle)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .set_health(240.0)
        .set_cost(1400, 0);
    crawler.sight_range = 175.0;
    game_logic
        .templates
        .insert("ChinaVehicleTroopCrawler".to_string(), crawler);
}

/// Spawn residual China Troop Crawler with transport + detector + assault residual.
/// InitialPayload Redguard × 8 docks on create when payload template available.

/// Spawn residual China Troop Crawler with transport + detector + assault residual.
/// InitialPayload Redguard × 8 docks on create when payload template available.
pub(super) fn create_test_troop_crawler(game_logic: &mut GameLogic, pos: Vec3) -> ObjectId {
    ensure_test_troop_crawler_template(game_logic);
    let id = game_logic
        .create_object("ChinaVehicleTroopCrawler", Team::China, pos)
        .expect("ChinaVehicleTroopCrawler");
    if let Some(obj) = game_logic.host_object_mut(id) {
        if !obj.is_troop_crawler_style_container() {
            obj.install_troop_crawler_transport();
        }
    }
    id
}

pub(super) fn ensure_test_repair_pad_template(game_logic: &mut GameLogic) {
    if game_logic.templates.contains_key("TestRepairPad") {
        return;
    }

    let mut repair_pad = ThingTemplate::new("TestRepairPad");
    repair_pad
        .add_kind_of(KindOf::Structure)
        .add_kind_of(KindOf::RepairPad)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .set_health(1000.0)
        .set_cost(500, -1);
    game_logic
        .templates
        .insert("TestRepairPad".to_string(), repair_pad);
}

pub(super) fn ensure_test_heal_pad_template(game_logic: &mut GameLogic) {
    if game_logic.templates.contains_key("TestHealPad") {
        return;
    }

    let mut heal_pad = ThingTemplate::new("TestHealPad");
    heal_pad
        .add_kind_of(KindOf::Structure)
        .add_kind_of(KindOf::HealPad)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .set_health(900.0)
        .set_cost(400, -1);
    game_logic
        .templates
        .insert("TestHealPad".to_string(), heal_pad);
}

pub(super) fn ensure_test_airfield_template(game_logic: &mut GameLogic) {
    if game_logic.templates.contains_key("TestAirfield") {
        return;
    }

    let mut airfield = ThingTemplate::new("TestAirfield");
    airfield
        .add_kind_of(KindOf::Structure)
        .add_kind_of(KindOf::FSAirfield)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .set_health(1200.0)
        .set_cost(1000, -2);
    // C++ airfields always author ParkingPlaceBehavior; production
    // production-gates fail closed without authored stall metadata.
    airfield.parking_place = Some(crate::game_logic::ParkingPlaceMetadata {
        num_rows: 2,
        num_cols: 2,
        approach_height: 50.0,
        landing_deck_height_offset: 0.0,
        has_runways: true,
        park_in_hangars: true,
        heal_amount_per_second: 10.0,
    });
    game_logic
        .templates
        .insert("TestAirfield".to_string(), airfield);
}

pub(super) fn ensure_test_player_for_team(game_logic: &mut GameLogic, team: Team) {
    let player_id = match team {
        Team::USA => 0,
        Team::China => 1,
        Team::GLA => 2,
        Team::Neutral => 3,
    };

    if game_logic.get_player(player_id).is_none() {
        let mut player = Player::new(player_id, team, "TestPlayer", true);
        player.resources.supplies = 100_000;
        player.power_available = 100;
        player.resources.power = 100;
        game_logic.add_player(player);
    }
}

pub(super) fn setup_ground_attacker(
    game_logic: &mut GameLogic,
    position: Vec3,
    target_location: Vec3,
) -> ObjectId {
    ensure_test_tank_template(game_logic);
    let attacker_id = game_logic
        .create_object("TestTank", Team::USA, position)
        .expect("attacker should be created from template");

    let attacker = game_logic
        .host_object_mut(attacker_id)
        .expect("attacker should exist");
    attacker.set_force_attack(true);
    attacker.set_target_location(Some(target_location));
    attacker.set_ai_state(AIState::AttackingGround);
    attacker.set_status_attacking(true);
    if let Some(weapon) = attacker.weapon.as_mut() {
        weapon.damage = 40.0;
        weapon.range = 150.0;
        weapon.reload_time = 0.25;
        weapon.last_fire_time = 0.0;
    }
    attacker.record_host_weapon_stats();

    attacker_id
}

pub(super) fn ensure_test_saboteur_template(game_logic: &mut GameLogic) {
    if game_logic.templates.contains_key("GLAInfantrySaboteur") {
        return;
    }
    let mut t = ThingTemplate::new("GLAInfantrySaboteur");
    t.add_kind_of(KindOf::Infantry)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .set_health(120.0)
        .set_cost(800, 0);
    game_logic
        .templates
        .insert("GLAInfantrySaboteur".to_string(), t);
}

pub(super) fn ensure_test_power_plant_template(game_logic: &mut GameLogic) {
    if game_logic.templates.contains_key("AmericaPowerPlant") {
        return;
    }
    let mut t = ThingTemplate::new("AmericaPowerPlant");
    t.add_kind_of(KindOf::Structure)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::FSPower)
        .add_kind_of(KindOf::PowerPlant)
        .set_health(1500.0);
    game_logic
        .templates
        .insert("AmericaPowerPlant".to_string(), t);
}

pub(super) fn ensure_test_war_factory_template(game_logic: &mut GameLogic) {
    if game_logic.templates.contains_key("AmericaWarFactory") {
        return;
    }
    let mut t = ThingTemplate::new("AmericaWarFactory");
    t.add_kind_of(KindOf::Structure)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::FSWarFactory)
        .set_health(2000.0);
    game_logic
        .templates
        .insert("AmericaWarFactory".to_string(), t);
}

/// Residual: GLA Saboteur power-plant brownout after reach (consumed).

/// Residual helper: ensure ChinaInfantryBlackLotus / TestBlackLotus template.
pub(super) fn ensure_test_black_lotus_template(game_logic: &mut GameLogic) {
    if game_logic.templates.contains_key("ChinaInfantryBlackLotus") {
        return;
    }
    let mut t = ThingTemplate::new("ChinaInfantryBlackLotus");
    t.add_kind_of(KindOf::Infantry)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .add_kind_of(KindOf::Hero)
        .set_health(200.0)
        .set_cost(1500, 0);
    game_logic
        .templates
        .insert("ChinaInfantryBlackLotus".to_string(), t);
}

/// EmpPulse is not a superweapon residual strike (separate disable residual path).

/// Advance Strategy Center door residual through AnimationTime frames to ACTIVE.
pub(super) fn advance_battle_plan_door_to_active(game_logic: &mut GameLogic) {
    use crate::game_logic::host_strategy_center::BATTLE_PLAN_ANIMATION_FRAMES;
    game_logic.frame = game_logic
        .frame
        .saturating_add(BATTLE_PLAN_ANIMATION_FRAMES);
    game_logic.tick_battle_plan_door_residuals();
}

/// Advance pack (210) then unpack (210) for a plan switch residual.

/// Advance pack (210) then unpack (210) for a plan switch residual.
pub(super) fn advance_battle_plan_switch_to_active(game_logic: &mut GameLogic) {
    advance_battle_plan_door_to_active(game_logic); // pack complete → unpack
    advance_battle_plan_door_to_active(game_logic); // unpack complete → ACTIVE
}

/// Residual: USA Strategy Center battle plans apply army residual bonuses.
///
/// C++ BattlePlanUpdate::setBattlePlan → Player::changeBattlePlan after unpack ACTIVE:
/// Bombardment DAMAGE 120%, HoldTheLine armor 0.9, SearchAndDestroy RANGE 120%.
/// Fail-closed: not full turret pitch matrix / vision-object residual.

/// Observed residual damage under default DAMAGE_AUTHORITY (HP log) or host HP delta.

/// Observed supplies under ECONOMY_AUTHORITY (effective includes pending delta).
/// Observed residual damage under default DAMAGE_AUTHORITY (HP log) or host HP delta.

/// Observed supplies under ECONOMY_AUTHORITY (effective includes pending delta).
pub(super) fn test_observed_supplies(player: &crate::game_logic::Player) -> u32 {
    player.effective_supplies()
}

pub(super) fn test_observed_damage_to(target: ObjectId, hp_before: f32, hp_after: f32) -> f32 {
    if crate::gameworld_shadow::gameworld_damage_authority_live() {
        crate::game_logic::host_damage_log::snapshot()
            .into_iter()
            .filter(|e| e.target == target)
            .map(|e| e.amount)
            .sum()
    } else {
        (hp_before - hp_after).max(0.0)
    }
}

// -----------------------------------------------------------------------
// Bomb Truck disguise residual (SpecialAbilityDisguiseAsVehicle)
// Fail-closed: not full StealthUpdate transition opacity / model swap.
// -----------------------------------------------------------------------

pub(super) fn ensure_test_bomb_truck_template(game_logic: &mut GameLogic) {
    if game_logic.templates.contains_key("TestBombTruck") {
        return;
    }
    let mut t = ThingTemplate::new("TestBombTruck");
    t.add_kind_of(KindOf::Vehicle)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .set_health(200.0)
        .set_cost(1200, 0);
    game_logic.templates.insert("TestBombTruck".to_string(), t);
}

/// Advance disguise transition residual past halfpoint (model swap).

/// Advance disguise transition residual past halfpoint (model swap).
pub(super) fn advance_disguise_halfpoint(game_logic: &mut GameLogic, ids: &[ObjectId]) {
    use crate::game_logic::host_bomb_truck_disguise::BOMB_TRUCK_DISGUISE_TRANSITION_FRAMES;
    let frames = (BOMB_TRUCK_DISGUISE_TRANSITION_FRAMES / 2).saturating_add(1);
    for _ in 0..frames {
        game_logic.update_ai(ids, 1.0 / 30.0);
    }
}

/// C++ `StealthDetectorUpdate` ctor staggers first wake; run detection at that frame.
pub(super) fn run_detector_first_scan(game_logic: &mut GameLogic, detector_id: ObjectId) {
    let due = game_logic
        .host_object(detector_id)
        .map(|o| o.next_detection_scan_frame)
        .unwrap_or(0)
        .max(1);
    game_logic.frame = due;
    game_logic.update_stealth_and_detection();
}

/// Advance disguise reveal transition residual past halfpoint.

/// Advance disguise reveal transition residual past halfpoint.
pub(super) fn advance_disguise_reveal_halfpoint(game_logic: &mut GameLogic, ids: &[ObjectId]) {
    use crate::game_logic::host_bomb_truck_disguise::BOMB_TRUCK_DISGUISE_REVEAL_TRANSITION_FRAMES;
    let frames = (BOMB_TRUCK_DISGUISE_REVEAL_TRANSITION_FRAMES / 2).saturating_add(1);
    for _ in 0..frames {
        game_logic.update_ai(ids, 1.0 / 30.0);
    }
}

/// Residual: DisguiseAsVehicle on bomb truck → DISGUISED + stealthed,
/// apparent team for enemies = disguise team; auto-target skips same-team.
// -----------------------------------------------------------------------
// China Helix NapalmBomb special ability residual
// Fail-closed: not full SpecialObject fall / Firestorm expand animation.
// -----------------------------------------------------------------------

pub(super) fn ensure_test_helix_template(game_logic: &mut GameLogic) {
    if game_logic.templates.contains_key("TestHelix") {
        return;
    }
    let mut t = ThingTemplate::new("TestHelix");
    t.add_kind_of(KindOf::Vehicle)
        .add_kind_of(KindOf::Aircraft)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .set_health(300.0);
    game_logic.templates.insert("TestHelix".to_string(), t);
}

/// Author the retail `VeterancyCrateCollide` pilot module on a hand-built
/// pilot template.  C++ `PilotFindVehicleUpdate` / `VeterancyCrateCollide`
/// gate re-crew on the authored module, not the basename
/// (VeterancyCrateCollide.cpp: IsPilot, RequiredKindOf = VEHICLE,
/// ForbiddenKindOf = DOZER, RangeOfEffect = 0, AddsOwnerVeterancy = Yes,
/// plus the companion `VeterancyGainCreate` StartingLevel that retail
/// AmericaInfantryPilot authors).
pub(super) fn author_pilot_recrew_module(template: &mut ThingTemplate) {
    template.veterancy_crate_collide = Some(VeterancyCrateCollideMetadata {
        is_pilot: true,
        required_kind_of_vehicle: true,
        forbidden_kind_of_dozer: true,
        effect_range: Some(0.0),
        adds_owner_veterancy: true,
        starting_level: Some(VeterancyLevel::Veteran),
    });
}

/// Author a retail-shaped `EjectPilotDie` module on a hand-built vehicle
/// template.  Retail INIZH (e.g. AmericaVehicleHumvee) authors
/// `GroundCreationList = OCL_EjectPilotOnGround` /
/// `AirCreationList = OCL_EjectPilotViaParachute` with the DieMux filter
/// carried by the caller; module `InvulnerableTime` keeps its 0 default
/// (C++ EjectPilotDie.cpp: the selected OCL owns the real grant).
pub(super) fn author_eject_pilot_die_module(
    template: &mut ThingTemplate,
    death_types: EjectPilotDeathTypes,
    veterancy_levels: EjectPilotVeterancyLevels,
    exempt_status: EjectPilotExemptStatus,
) {
    template.eject_pilot_die = Some(EjectPilotDieMetadata {
        ground_creation_list: Some(EjectPilotCreationList::OnGround),
        air_creation_list: Some(EjectPilotCreationList::ViaParachute),
        invulnerable_time_ms: Some(0),
        death_types,
        veterancy_levels,
        exempt_status,
        required_status: EjectPilotRequiredStatus::None,
    });
}

/// Register the two retail `ObjectCreationList.ini` EjectPilot blocks so the
/// host death path can resolve them: this checkout ships no retail BIG data,
/// so `ensure_default_object_creation_lists_loaded` finds no INIZH file.
/// `InvulnerableTime` is authored in ms and parsed as frames at 30 FPS
/// (2000 ms → 60f).  Idempotent: repeated loads write identical nuggets.
pub(super) fn register_retail_eject_pilot_ocls() {
    const RETAIL_EJECT_PILOT_OCLS: &str = r#"
ObjectCreationList OCL_EjectPilotOnGround
  CreateObject
    ObjectNames = AmericaInfantryPilot
    Count = 1
    IgnorePrimaryObstacle = Yes
    InheritsVeterancy = Yes
    Disposition = RANDOM_FORCE
    MinForceMagnitude = 2
    MaxForceMagnitude = 3
    MinForcePitch = 50
    MaxForcePitch = 60
    SpinRate = 0
    InvulnerableTime = 2000ms
    RequiresLivePlayer = Yes
  End
End

ObjectCreationList OCL_EjectPilotViaParachute
  CreateObject
    ObjectNames = AmericaInfantryPilot
    Count = 1
    PutInContainer = AmericaParachute
    IgnorePrimaryObstacle = Yes
    InheritsVeterancy = Yes
    Disposition = RANDOM_FORCE
    MinForceMagnitude = 10
    MaxForceMagnitude = 12
    MinForcePitch = 50
    MaxForcePitch = 60
    SpinRate = 0
    InvulnerableTime = 2000ms
    RequiresLivePlayer = Yes
  End
End
"#;
    let _ = gamelogic::object_creation_list::store::load_object_creation_lists_from_str(
        RETAIL_EJECT_PILOT_OCLS,
    );
}

/// Ensure the shared ejection fixture: retail OCLs registered, the
/// `AmericaInfantryPilot` Object template (OCL_EjectPilot* ObjectNames
/// target), and a live owning USA player (C++ EjectPilotDie requires a live
/// source controller, `RequiresLivePlayer = Yes`).
pub(super) fn ensure_eject_pilot_residual_fixture(game_logic: &mut GameLogic) {
    register_retail_eject_pilot_ocls();
    let eject_pilot_template = crate::game_logic::host_usa_pilot::EJECT_PILOT_TEMPLATE;
    if !game_logic.templates.contains_key(eject_pilot_template) {
        let mut pilot_tpl = ThingTemplate::new(eject_pilot_template);
        pilot_tpl
            .add_kind_of(KindOf::Infantry)
            .add_kind_of(KindOf::Selectable)
            .set_health(100.0);
        game_logic
            .templates
            .insert(eject_pilot_template.to_string(), pilot_tpl);
    }
    if game_logic.get_player(0).is_none() {
        game_logic
            .players
            .insert(0, Player::new(0, Team::USA, "USA", true));
    }
}


/// Author a retail `SpecialPower` module record on a hand-built template
/// (OCLSpecialPower behavior + loaded SpecialPowerTemplate).  C++ refuses an
/// any-unit fallback when the object carries no SpecialPowerModule
/// (SpecialPower.cpp:308 canUseSpecialPower), so command-driven casts need
/// the authored module even on test fixtures.
pub(super) fn author_superweapon_special_power_module(
    game_logic: &mut GameLogic,
    template_name: &str,
    command_power: crate::command_system::SpecialPowerType,
    special_power_template: &str,
    reload_time_frames: u32,
) {
    use crate::game_logic::{SpecialPowerModuleKind, SpecialPowerModuleMetadata};
    let Some(template) = game_logic.templates.get_mut(template_name) else {
        panic!("template {template_name} must exist before authoring its module");
    };
    template.special_power_modules.push(SpecialPowerModuleMetadata {
        source_index: 0,
        module_tag: Some(format!("ModuleTag_{}", special_power_template)),
        module_kind: SpecialPowerModuleKind::OclSpecialPower,
        special_power_template: special_power_template.into(),
        special_power_template_id: 1,
        command_power: Some(command_power),
        reload_time_frames,
        required_science: None,
        public_timer: false,
        shared_n_sync: false,
        shortcut_power: false,
        update_module_starts_attack: false,
        starts_paused: false,
        scripted_special_power_only: false,
    });
}
