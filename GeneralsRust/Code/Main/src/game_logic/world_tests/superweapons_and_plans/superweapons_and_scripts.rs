//! Behavior suite extracted from `superweapons_and_plans`.
use super::*;

#[test]
fn anthrax_bomb_exits_plane_at_delivery_distance_not_over_click() {
    use crate::game_logic::KindOf;
    use crate::game_logic::host_anthrax_bomb_flight::ANTHRAX_DELIVERY_DISTANCE;
    let mut logic = GameLogic::new();
    logic.override_world_size(2000.0, 2000.0);
    ensure_test_player_for_team(&mut logic, Team::GLA);
    let mut cc = crate::game_logic::ThingTemplate::new("GLACommandCenter");
    cc.add_kind_of(KindOf::Structure).set_health(5000.0);
    logic.templates.insert("GLACommandCenter".into(), cc);
    let cc_id = logic
        .create_object("GLACommandCenter", Team::GLA, Vec3::new(0.0, 0.0, 0.0))
        .unwrap();
    let target = Vec3::new(400.0, 0.0, 0.0);
    let jet = logic
        .spawn_anthrax_bomb_flight(cc_id, target)
        .expect("cargo");
    let mut bomb_pos = None;
    let mut plane_at_drop = None;
    for f in 0..400 {
        logic.frame = f;
        let plane_before = logic.host_object(jet).map(|o| o.get_position());
        logic.update_anthrax_bomb_flights();
        if logic.anthrax_bomb_flight_reg.bombs_dropped >= 1 {
            plane_at_drop = plane_before;
            bomb_pos = logic
                .host_objects()
                .values()
                .find(|o| o.anthrax_bomb_payload)
                .map(|o| o.get_position());
            break;
        }
    }
    let bomb = bomb_pos.expect("bomb must leave the plane");
    let plane = plane_at_drop.expect("plane pose at drop");
    let dx_click = bomb.x - target.x;
    let dz_click = bomb.z - target.z;
    let dist_click = (dx_click * dx_click + dz_click * dz_click).sqrt();
    assert!(
        dist_click > 50.0,
        "bomb must not teleport over the click, dist={dist_click} bomb={bomb:?}"
    );
    let dx_plane = bomb.x - plane.x;
    let dz_plane = bomb.z - plane.z;
    let dist_plane = (dx_plane * dx_plane + dz_plane * dz_plane).sqrt();
    assert!(
        dist_plane < 25.0,
        "bomb must exit at the plane pose, dist={dist_plane} bomb={bomb:?} plane={plane:?}"
    );
    let dx_tgt = plane.x - target.x;
    let dz_tgt = plane.z - target.z;
    let plane_to_tgt = (dx_tgt * dx_tgt + dz_tgt * dz_tgt).sqrt();
    assert!(
        plane_to_tgt <= ANTHRAX_DELIVERY_DISTANCE + 20.0,
        "plane must drop at DeliveryDistance 140, not 70, dist={plane_to_tgt}"
    );
    assert!(
        plane_to_tgt > 70.0,
        "must not wait until the old 70wu half-band, dist={plane_to_tgt}"
    );
}

#[test]
fn anthrax_gamma_science_drops_gamma_bomb_and_field() {
    use crate::game_logic::KindOf;
    use crate::game_logic::host_anthrax_bomb_flight::AnthraxBombPayloadTier;
    use crate::game_logic::host_upgrades::UPGRADE_CHEM_ANTHRAX_GAMMA;
    use crate::game_logic::special_power_strikes::ANTHRAX_TOXIN_OBJECT_NAME_GAMMA;
    let mut logic = GameLogic::new();
    ensure_test_player_for_team(&mut logic, Team::GLA);
    if let Some(p) = logic.get_player_mut(2) {
        p.add_completed_upgrade(UPGRADE_CHEM_ANTHRAX_GAMMA);
    }
    let mut cc = crate::game_logic::ThingTemplate::new("GLACommandCenter");
    cc.add_kind_of(KindOf::Structure).set_health(5000.0);
    logic.templates.insert("GLACommandCenter".into(), cc);
    let cc_id = logic
        .create_object("GLACommandCenter", Team::GLA, Vec3::new(0.0, 0.0, 0.0))
        .unwrap();
    let jet = logic
        .spawn_anthrax_bomb_flight(cc_id, Vec3::new(150.0, 0.0, 0.0))
        .expect("cargo");
    let tier = logic
        .host_object(jet)
        .and_then(|o| o.anthrax_bomb_transport.as_ref())
        .map(|d| d.tier)
        .expect("tier");
    assert_eq!(tier, AnthraxBombPayloadTier::Gamma);
    for f in 0..400 {
        logic.frame = f;
        logic.update_anthrax_bomb_flights();
        if logic.anthrax_bomb_flight_reg.toxin_fields_spawned >= 1 {
            break;
        }
    }
    assert!(logic.anthrax_bomb_flight_reg.bombs_dropped >= 1);
    assert!(logic.anthrax_bomb_flight_reg.toxin_fields_spawned >= 1);
    let field = logic
        .special_power_strikes
        .toxin_fields()
        .iter()
        .find(|f| f.object_template == ANTHRAX_TOXIN_OBJECT_NAME_GAMMA);
    assert!(
        field.is_some(),
        "gamma science must spawn PoisonFieldAnthraxGammaBomb / POISONED_GAMMA"
    );
}

#[test]
fn cluster_mines_bomb_exits_plane_not_over_click() {
    use crate::game_logic::KindOf;
    use crate::game_logic::host_mines::CLUSTER_MINES_DELIVERY_DISTANCE;
    let mut logic = GameLogic::new();
    logic.override_world_size(2000.0, 2000.0);
    ensure_test_player_for_team(&mut logic, Team::China);
    let mut cc = crate::game_logic::ThingTemplate::new("ChinaCommandCenter");
    cc.add_kind_of(KindOf::Structure).set_health(5000.0);
    logic.templates.insert("ChinaCommandCenter".into(), cc);
    let cc_id = logic
        .create_object("ChinaCommandCenter", Team::China, Vec3::new(0.0, 0.0, 0.0))
        .unwrap();
    let target = Vec3::new(400.0, 0.0, 0.0);
    let jet = logic
        .spawn_cluster_mines_flight(cc_id, target)
        .expect("cargo");
    let mut bomb_pos = None;
    let mut plane_at_drop = None;
    for f in 0..400 {
        logic.frame = f;
        let plane_before = logic.host_object(jet).map(|o| o.get_position());
        logic.update_cluster_mines_flights();
        if logic.cluster_mines_flight_reg.bombs_dropped >= 1 {
            plane_at_drop = plane_before;
            bomb_pos = logic
                .host_objects()
                .values()
                .find(|o| o.cluster_mines_bomb)
                .map(|o| o.get_position());
            break;
        }
    }
    let bomb = bomb_pos.expect("cluster bomb must leave the plane");
    let plane = plane_at_drop.expect("plane pose at drop");
    let dx_click = bomb.x - target.x;
    let dz_click = bomb.z - target.z;
    let dist_click = (dx_click * dx_click + dz_click * dz_click).sqrt();
    assert!(
        dist_click > 50.0,
        "cluster bomb must not teleport over the click, dist={dist_click}"
    );
    let dx_plane = bomb.x - plane.x;
    let dz_plane = bomb.z - plane.z;
    let dist_plane = (dx_plane * dx_plane + dz_plane * dz_plane).sqrt();
    assert!(
        dist_plane < 25.0,
        "cluster bomb must exit at the plane, dist={dist_plane}"
    );
    let dx_tgt = plane.x - target.x;
    let dz_tgt = plane.z - target.z;
    let plane_to_tgt = (dx_tgt * dx_tgt + dz_tgt * dz_tgt).sqrt();
    assert!(plane_to_tgt <= CLUSTER_MINES_DELIVERY_DISTANCE + 20.0);
    assert!(
        plane_to_tgt > 70.0,
        "must drop at 140 not 70, dist={plane_to_tgt}"
    );
}

#[test]
fn sneak_attack_spawns_tunnel_start() {
    use crate::command_system::SpecialPowerType;
    use crate::game_logic::KindOf;
    use crate::game_logic::host_sneak_attack::SNEAK_ATTACK_TUNNEL_START_TEMPLATE;
    let mut logic = GameLogic::new();
    ensure_test_player_for_team(&mut logic, Team::GLA);
    if let Some(p) = logic.get_player_mut(2) {
        p.unlock_science("SCIENCE_SneakAttack");
    }
    let mut cc = crate::game_logic::ThingTemplate::new("GLACommandCenter");
    cc.add_kind_of(KindOf::Structure).set_health(5000.0);
    logic.templates.insert("GLACommandCenter".into(), cc);
    let cc_id = logic
        .create_object("GLACommandCenter", Team::GLA, Vec3::new(0.0, 0.0, 0.0))
        .unwrap();
    let id = logic
        .queue_sneak_attack(
            &SpecialPowerType::SneakAttack,
            cc_id,
            Vec3::new(100.0, 0.0, 0.0),
        )
        .expect("sneak");
    assert!(logic.host_sneak_attacks.tunnel_starts_spawned >= 1);
    let start_id = logic
        .host_sneak_attacks
        .get(id)
        .and_then(|m| m.tunnel_start_object)
        .expect("start id");
    assert_eq!(
        logic.host_object(start_id).unwrap().template_name,
        SNEAK_ATTACK_TUNNEL_START_TEMPLATE
    );
    assert!(logic.host_object(start_id).unwrap().sneak_tunnel_start);
    // Advance to tunnel spawn; Start is destroyed and tunnel appears.
    for f in 0..=160 {
        logic.frame = f;
        logic.update_sneak_attacks();
    }
    assert!(logic.host_sneak_attacks.tunnel_spawn_count >= 1);
    assert!(
        logic
            .host_object(start_id)
            .map(|o| !o.is_alive())
            .unwrap_or(true),
        "TunnelStart should die when real tunnel spawns"
    );
    assert!(logic.host_sneak_attacks.honesty_tunnel_start_ok());
}

#[test]
fn sneak_attack_applies_placement_facing() {
    use crate::command_system::SpecialPowerType;
    use crate::game_logic::KindOf;
    let mut logic = GameLogic::new();
    ensure_test_player_for_team(&mut logic, Team::GLA);
    if let Some(p) = logic.get_player_mut(2) {
        p.unlock_science("SCIENCE_SneakAttack");
    }
    let mut cc = crate::game_logic::ThingTemplate::new("GLACommandCenter");
    cc.add_kind_of(KindOf::Structure).set_health(5000.0);
    logic.templates.insert("GLACommandCenter".into(), cc);
    let cc_id = logic
        .create_object("GLACommandCenter", Team::GLA, Vec3::new(0.0, 0.0, 0.0))
        .unwrap();
    let angle = 1.25_f32;
    let id = logic
        .queue_sneak_attack_facing(
            &SpecialPowerType::SneakAttack,
            cc_id,
            Vec3::new(80.0, 0.0, 40.0),
            angle,
        )
        .expect("sneak");
    let start_id = logic
        .host_sneak_attacks
        .get(id)
        .and_then(|m| m.tunnel_start_object)
        .expect("start id");
    assert!(
        (logic.host_object(start_id).unwrap().get_orientation() - angle).abs() < 1.0e-5,
        "TunnelStart must keep PlaceEvent facing"
    );
    for f in 0..=160 {
        logic.frame = f;
        logic.update_sneak_attacks();
    }
    let tunnel_id = logic
        .host_sneak_attacks
        .get(id)
        .and_then(|m| m.spawned_tunnel_id)
        .expect("tunnel id");
    assert!(
        (logic.host_object(tunnel_id).unwrap().get_orientation() - angle).abs() < 1.0e-5,
        "spawned tunnel must keep PlaceEvent facing"
    );
}

#[test]
fn sneak_attack_multi_pulse_shockwaves() {
    use crate::command_system::SpecialPowerType;
    use crate::game_logic::KindOf;
    use crate::game_logic::host_sneak_attack::sneak_attack_shockwave_pulses;
    let mut logic = GameLogic::new();
    ensure_test_player_for_team(&mut logic, Team::GLA);
    ensure_test_player_for_team(&mut logic, Team::USA);
    if let Some(p) = logic.get_player_mut(2) {
        p.unlock_science("SCIENCE_SneakAttack");
    }
    let mut cc = crate::game_logic::ThingTemplate::new("GLACommandCenter");
    cc.add_kind_of(KindOf::Structure).set_health(5000.0);
    logic.templates.insert("GLACommandCenter".into(), cc);
    let mut foe_t = crate::game_logic::ThingTemplate::new("AmericaTankCrusader");
    foe_t.add_kind_of(KindOf::Vehicle).set_health(400.0);
    logic.templates.insert("AmericaTankCrusader".into(), foe_t);
    let cc_id = logic
        .create_object("GLACommandCenter", Team::GLA, Vec3::new(0.0, 0.0, 0.0))
        .unwrap();
    let foe = logic
        .create_object("AmericaTankCrusader", Team::USA, Vec3::new(20.0, 0.0, 0.0))
        .unwrap();
    let hp0 = logic.host_object(foe).unwrap().health.current;
    let id = logic
        .queue_sneak_attack(
            &SpecialPowerType::SneakAttack,
            cc_id,
            Vec3::new(20.0, 0.0, 0.0),
        )
        .expect("sneak");
    assert!(id >= 1);
    assert_eq!(logic.host_sneak_attacks.pending_shockwaves.len(), 3);
    assert_eq!(sneak_attack_shockwave_pulses().len(), 3);
    // Advance through all pulse frames (1, 30, 75) and tunnel spawn (150).
    for f in 0..=160 {
        logic.frame = f;
        logic.update_sneak_attacks();
    }
    assert!(
        logic.host_sneak_attacks.multi_pulse_applies >= 3,
        "expected 3 multi-pulse applies, got {}",
        logic.host_sneak_attacks.multi_pulse_applies
    );
    assert!(logic.host_sneak_attacks.honesty_multi_pulse_ok());
    assert!(logic.host_sneak_attacks.tunnel_spawn_count >= 1);
    let hp1 = logic
        .host_object(foe)
        .map(|o| o.health.current)
        .unwrap_or(0.0);
    assert!(
        hp1 < hp0
            || logic
                .host_object(foe)
                .map(|o| !o.is_alive())
                .unwrap_or(true),
        "multi-pulse shockwaves should damage nearby units"
    );
}

#[test]
fn sneak_attack_residual_spawns_tunnel_and_shockwave() {
    use crate::command_system::{CommandType, GameCommand, PowerTarget, SpecialPowerType};
    use crate::game_logic::host_sneak_attack::{
        HostSneakAttackKind, HostSneakAttackPhase, SNEAK_ATTACK_RESIDUAL_TEMPLATE,
        SNEAK_ATTACK_SHOCKWAVE_DAMAGE, SNEAK_ATTACK_SPAWN_DELAY_FRAMES,
    };

    let mut game_logic = GameLogic::new();
    ensure_test_tank_template(&mut game_logic);
    ensure_test_infantry_template(&mut game_logic);
    ensure_test_player_for_team(&mut game_logic, Team::GLA);
    ensure_test_player_for_team(&mut game_logic, Team::USA);
    if let Some(p) = game_logic.get_player_mut(2) {
        p.unlock_science("SCIENCE_SneakAttack");
    }

    let caster_id = game_logic
        .create_object("TestTank", Team::GLA, Vec3::new(0.0, 0.0, 0.0))
        .expect("caster");
    {
        let caster = game_logic.host_object_mut(caster_id).expect("caster");
        caster.set_special_power_ready(true);
        caster.special_power_cooldown_remaining = 0.0;
        caster.special_power_cooldown = 10.0;
    }

    let target = Vec3::new(200.0, 0.0, 100.0);
    // Enemy near spawn for shockwave residual.
    let enemy_id = game_logic
        .create_object(
            "TestInfantry",
            Team::USA,
            Vec3::new(target.x + 10.0, 0.0, target.z),
        )
        .expect("enemy near tunnel");
    let far_enemy_id = game_logic
        .create_object(
            "TestInfantry",
            Team::USA,
            Vec3::new(target.x + 200.0, 0.0, target.z),
        )
        .expect("far enemy");

    let enemy_hp_before = game_logic.host_object(enemy_id).unwrap().health.current;
    let far_hp_before = game_logic.host_object(far_enemy_id).unwrap().health.current;
    let objects_before = game_logic.host_objects().len();

    assert!(!game_logic.honesty_sneak_attack_ok());

    game_logic.queue_command(GameCommand {
        command_type: CommandType::DoSpecialPower {
            power_type: SpecialPowerType::SneakAttack,
            target: PowerTarget::Location(target),
        },
        player_id: 2, // Team::GLA residual ownership
        command_id: 82,
        timestamp: std::time::SystemTime::now(),
        selected_units: vec![caster_id],
        modifier_keys: crate::command_system::ModifierKeys::default(),
    });
    game_logic.process_commands();

    assert!(
        game_logic.honesty_sneak_attack_activate_ok(),
        "SneakAttack residual must record activation honesty"
    );
    assert_eq!(game_logic.host_sneak_attacks().pending_count(), 1);
    assert!(!game_logic.honesty_sneak_attack_tunnel_ok());
    let caster = game_logic.host_object(caster_id).expect("caster after cmd");
    assert!(!caster.special_power_ready);
    assert!(caster.special_power_cooldown_remaining > 0.0);
    assert_eq!(caster.ai_state, AIState::SpecialAbility);
    assert!(
        game_logic
            .queued_audio_events
            .iter()
            .any(|e| e.event_type == "SneakAttackActivated"),
        "activation must queue SneakAttackActivated audio"
    );
    // TunnelStart residual may spawn immediately; real tunnel must not yet exist.
    let real_tunnels = |gl: &GameLogic| {
        gl.host_objects()
            .values()
            .filter(|o| {
                o.is_alive()
                    && o.is_kind_of(crate::game_logic::KindOf::Structure)
                    && !o.sneak_tunnel_start
                    && (o.template_name.contains("Tunnel") || o.template_name.contains("Sneak"))
            })
            .count()
    };
    assert_eq!(
        real_tunnels(&game_logic),
        0,
        "no real tunnel before Lifetime delay residual (objects={})",
        game_logic.host_objects().len()
    );
    assert!(
        game_logic.host_sneak_attacks.tunnel_starts_spawned >= 1,
        "TunnelStart residual should spawn on queue"
    );
    assert_eq!(
        game_logic.special_power_strikes().strike_count(),
        0,
        "SneakAttack must not enqueue superweapon residual strikes"
    );

    game_logic.frame = SNEAK_ATTACK_SPAWN_DELAY_FRAMES - 1;
    game_logic.update_sneak_attacks();
    assert_eq!(
        real_tunnels(&game_logic),
        0,
        "still no real tunnel one frame before spawn"
    );

    game_logic.frame = SNEAK_ATTACK_SPAWN_DELAY_FRAMES;
    game_logic.update_sneak_attacks();

    assert!(
        game_logic.honesty_sneak_attack_tunnel_ok(),
        "SneakAttack residual must spawn tunnel honesty"
    );
    assert!(
        game_logic.honesty_sneak_attack_ok(),
        "SneakAttack host residual path honesty"
    );
    assert!(
        game_logic.honesty_sneak_attack_shockwave_ok(),
        "SneakAttack residual shockwave must hit nearby units"
    );

    let completed = game_logic
        .host_sneak_attacks()
        .completed_of_kind(HostSneakAttackKind::GLASneakAttack);
    assert_eq!(completed.len(), 1);
    assert_eq!(completed[0].phase, HostSneakAttackPhase::Completed);
    let tunnel_id = completed[0]
        .spawned_tunnel_id
        .expect("tunnel must be spawned");
    assert!(
        completed[0].shockwave_hits >= 1,
        "shockwave residual must hit nearby enemy"
    );

    let tunnel = game_logic.host_object(tunnel_id).expect("tunnel object");
    assert_eq!(tunnel.team, Team::GLA);
    assert!(
        tunnel.is_kind_of(KindOf::Structure),
        "spawned tunnel must be a structure residual"
    );
    assert_eq!(
        tunnel.thing.template.name, SNEAK_ATTACK_RESIDUAL_TEMPLATE,
        "must use residual tunnel template when retail unloaded"
    );
    let tpos = tunnel.get_position();
    assert!(
        (tpos.x - target.x).abs() < 0.5 && (tpos.z - target.z).abs() < 0.5,
        "tunnel must spawn at target location"
    );

    let enemy_hp_after = game_logic.host_object(enemy_id).unwrap().health.current;
    let far_hp_after = game_logic.host_object(far_enemy_id).unwrap().health.current;
    assert!(
        (enemy_hp_before - enemy_hp_after - SNEAK_ATTACK_SHOCKWAVE_DAMAGE).abs() < 0.01
            || enemy_hp_after < enemy_hp_before,
        "nearby enemy must take residual shockwave damage (before={enemy_hp_before}, after={enemy_hp_after})"
    );
    assert_eq!(
        far_hp_before, far_hp_after,
        "out-of-radius enemy must not take shockwave residual"
    );

    assert!(
        game_logic
            .queued_audio_events
            .iter()
            .any(|e| e.event_type == "SneakAttackTunnelSpawn"),
        "spawn must queue SneakAttackTunnelSpawn audio"
    );
    assert_eq!(
        real_tunnels(&game_logic),
        1,
        "exactly one real tunnel structure after Lifetime residual"
    );
    let _ = objects_before;
}

#[test]
fn carbomb_command_converts_vehicle_after_reach() {
    let mut game_logic = GameLogic::new();
    ensure_test_tank_template(&mut game_logic);

    let bomber_id = game_logic
        .create_object("TestTank", Team::USA, Vec3::new(170.0, 0.0, 0.0))
        .expect("bomber should be created");
    let target_id = game_logic
        .create_object("TestTank", Team::GLA, Vec3::new(0.0, 0.0, 0.0))
        .expect("target should be created");

    let initial_health = game_logic
        .host_object(target_id)
        .expect("target should exist")
        .health
        .current;

    game_logic.queue_command(crate::command_system::GameCommand {
        command_type: crate::command_system::CommandType::ConvertToCarbomb { target_id },
        player_id: 0,
        command_id: 1,
        timestamp: std::time::SystemTime::now(),
        selected_units: vec![bomber_id],
        modifier_keys: crate::command_system::ModifierKeys::default(),
    });
    game_logic.process_commands();

    let target_after_command = game_logic
        .host_object(target_id)
        .expect("target should exist");
    assert_eq!(
        target_after_command.health.current, initial_health,
        "carbomb should not apply immediately on command issue"
    );
    assert!(!target_after_command.status.is_carbomb);
    assert_eq!(target_after_command.team, Team::GLA);

    game_logic.update_ai(&[bomber_id, target_id], 1.0 / 60.0);
    let target_after_far_update = game_logic
        .host_object(target_id)
        .expect("target should exist");
    assert_eq!(
        target_after_far_update.health.current, initial_health,
        "carbomb should be pending while bomber is out of range"
    );
    assert!(!target_after_far_update.status.is_carbomb);

    {
        let bomber = game_logic
            .host_object_mut(bomber_id)
            .expect("bomber should exist");
        bomber.set_position(Vec3::new(2.0, 0.0, 0.0));
        bomber.set_ai_state(AIState::SpecialAbility);
        bomber.target = Some(target_id);
    }
    game_logic.update_ai(&[bomber_id, target_id], 1.0 / 60.0);

    let target_after_contact = game_logic
        .host_object(target_id)
        .expect("target should exist");
    assert_eq!(
        target_after_contact.health.current, initial_health,
        "ConvertToCarBomb must not damage vehicle HP on conversion"
    );
    assert!(
        target_after_contact.status.is_carbomb,
        "vehicle must gain IS_CARBOMB"
    );
    assert_eq!(
        target_after_contact.team,
        Team::USA,
        "converted car bomb defects to converter team"
    );
    assert!(
        target_after_contact.weapon.is_some(),
        "car bomb residual binds SuicideCarBomb weapon"
    );
    assert!(
        game_logic.honesty_carbomb_convert_ok(),
        "carbomb convert residual honesty"
    );

    let bomber = game_logic
        .host_object(bomber_id)
        .expect("bomber should exist");
    assert!(
        bomber.status.destroyed,
        "converter infantry is consumed on conversion"
    );
    assert!(
        !game_logic
            .queued_audio_events
            .iter()
            .any(|e| e.event_type == "MakeCarBombSuccess"),
        "convert must not play the FXList name as audio"
    );
    assert!(
        game_logic
            .queued_audio_events
            .iter()
            .any(|e| e.event_type == "TerroristCarBomb")
            || crate::game_logic::dispatch_fx_list_at_object(
                crate::game_logic::host_car_bomb::CAR_BOMB_CONVERT_FX_LIST,
                target_id.0,
                None
            ),
        "convert must dispatch FX_MakeCarBombSuccess or its TerroristCarBomb sound"
    );
}

#[test]
fn carbomb_command_allows_neutral_targets() {
    let mut game_logic = GameLogic::new();
    ensure_test_tank_template(&mut game_logic);

    let bomber_id = game_logic
        .create_object("TestTank", Team::USA, Vec3::new(150.0, 0.0, 0.0))
        .expect("bomber should be created");
    let target_id = game_logic
        .create_object("TestTank", Team::Neutral, Vec3::new(0.0, 0.0, 0.0))
        .expect("neutral target should be created");

    let initial_health = game_logic
        .host_object(target_id)
        .expect("target should exist")
        .health
        .current;

    game_logic.queue_command(crate::command_system::GameCommand {
        command_type: crate::command_system::CommandType::ConvertToCarbomb { target_id },
        player_id: 0,
        command_id: 1,
        timestamp: std::time::SystemTime::now(),
        selected_units: vec![bomber_id],
        modifier_keys: crate::command_system::ModifierKeys::default(),
    });
    game_logic.process_commands();

    {
        let bomber = game_logic
            .host_object(bomber_id)
            .expect("bomber should exist");
        assert_eq!(bomber.ai_state, AIState::SpecialAbility);
        assert_eq!(bomber.target, Some(target_id));
    }

    {
        let bomber = game_logic
            .host_object_mut(bomber_id)
            .expect("bomber should exist");
        bomber.set_position(Vec3::new(2.0, 0.0, 0.0));
        bomber.set_ai_state(AIState::SpecialAbility);
        bomber.target = Some(target_id);
    }
    game_logic.update_ai(&[bomber_id, target_id], 1.0 / 60.0);

    let target_after_contact = game_logic
        .host_object(target_id)
        .expect("target should exist");
    assert_eq!(
        target_after_contact.health.current, initial_health,
        "neutral convert must not damage vehicle"
    );
    assert!(
        target_after_contact.status.is_carbomb,
        "neutral vehicle becomes car bomb"
    );
    assert_eq!(target_after_contact.team, Team::USA);

    let bomber = game_logic
        .host_object(bomber_id)
        .expect("bomber should exist");
    assert!(bomber.status.destroyed);
}

#[test]
fn carbomb_attack_structure_detonates_with_observable_damage() {
    let mut game_logic = GameLogic::new();
    ensure_test_tank_template(&mut game_logic);
    ensure_test_structure_template(&mut game_logic);

    let car_id = game_logic
        .create_object("TestTank", Team::GLA, Vec3::new(0.0, 0.0, 0.0))
        .expect("car should be created");
    let structure_id = game_logic
        .create_object("TestBuilding", Team::USA, Vec3::new(3.0, 0.0, 0.0))
        .expect("structure should be created");

    {
        let car = game_logic
            .host_object_mut(car_id)
            .expect("car should exist");
        car.apply_convert_to_car_bomb();
        car.set_team(Team::GLA);
        // Ensure weapon is ready to fire immediately.
        if let Some(w) = car.weapon.as_mut() {
            w.last_fire_time = 0.0;
            w.reload_time = 0.0;
        }
        car.attack_target(structure_id);
    }
    game_logic.car_bomb.record_conversion();

    let structure_hp_before = game_logic
        .host_object(structure_id)
        .expect("structure should exist")
        .health
        .current;
    assert!(structure_hp_before > 0.0);

    // SuicideCarBomb AttackRange = 5; structure at 3 is in range.
    game_logic.frame = 30;
    game_logic.update_combat(&[car_id, structure_id], 1.0 / 30.0);

    let structure_after = game_logic
        .host_object(structure_id)
        .expect("structure should exist");
    assert!(
        structure_after.health.current < structure_hp_before
            || structure_after.status.destroyed
            || !structure_after.is_alive(),
        "car bomb detonation must damage structure (before={structure_hp_before}, after={})",
        structure_after.health.current
    );
    assert!(
        game_logic.honesty_carbomb_detonate_ok(),
        "carbomb detonate residual honesty (damage={})",
        game_logic.car_bomb_residual().detonation_damage_dealt
    );

    let car = game_logic.host_object(car_id).expect("car should exist");
    assert!(
        car.status.destroyed || !car.is_alive(),
        "car bomb destroys itself on detonation"
    );
}

#[test]
fn attack_order_chases_target_when_out_of_range() {
    let mut game_logic = GameLogic::new();
    ensure_test_tank_template(&mut game_logic);

    let attacker_id = game_logic
        .create_object("TestTank", Team::USA, Vec3::new(0.0, 0.0, 0.0))
        .expect("attacker should be created from template");
    let target_id = game_logic
        .create_object("TestTank", Team::GLA, Vec3::new(300.0, 0.0, 0.0))
        .expect("target should be created from template");

    {
        let attacker = game_logic
            .host_object_mut(attacker_id)
            .expect("attacker should exist");
        attacker.attack_target(target_id);
        if let Some(weapon) = attacker.weapon.as_mut() {
            weapon.range = 50.0;
            weapon.reload_time = 0.0;
            weapon.last_fire_time = 0.0;
        }
    }

    game_logic.frame = 60;
    game_logic.update_combat(&[attacker_id, target_id], 1.0 / 60.0);

    let attacker = game_logic
        .host_object(attacker_id)
        .expect("attacker should exist");
    let chase_target = attacker
        .movement
        .target_position
        .expect("attacker should chase out-of-range target");
    assert!(
        chase_target.distance(Vec3::new(300.0, 0.0, 0.0)) < 0.01,
        "attacker should chase the current target position"
    );
    assert_eq!(attacker.ai_state, AIState::Attacking);
    assert!(attacker.status.moving);
}

#[test]
fn attack_order_clears_dead_target() {
    let mut game_logic = GameLogic::new();
    ensure_test_tank_template(&mut game_logic);

    let attacker_id = game_logic
        .create_object("TestTank", Team::USA, Vec3::new(0.0, 0.0, 0.0))
        .expect("attacker should be created from template");
    let target_id = game_logic
        .create_object("TestTank", Team::GLA, Vec3::new(10.0, 0.0, 0.0))
        .expect("target should be created from template");

    {
        let attacker = game_logic
            .host_object_mut(attacker_id)
            .expect("attacker should exist");
        attacker.attack_target(target_id);
    }
    {
        let target = game_logic
            .host_object_mut(target_id)
            .expect("target should exist");
        target.status.destroyed = true;
    }

    game_logic.frame = 60;
    game_logic.update_combat(&[attacker_id, target_id], 1.0 / 60.0);

    let attacker = game_logic
        .host_object(attacker_id)
        .expect("attacker should exist");
    assert!(attacker.target.is_none(), "dead targets should be cleared");
    assert_eq!(attacker.ai_state, AIState::Idle);
    assert!(!attacker.status.attacking);
}

#[test]
fn ai_production_does_not_spawn_when_player_cannot_afford_unit() {
    let mut game_logic = GameLogic::new();

    let mut war_factory = ThingTemplate::new("WarFactory");
    war_factory
        .add_kind_of(KindOf::Structure)
        .add_kind_of(KindOf::Selectable)
        .set_health(1500.0)
        .set_cost(1000, -2);
    game_logic
        .templates
        .insert("WarFactory".to_string(), war_factory);

    let mut humvee = ThingTemplate::new("USA_Humvee");
    humvee
        .add_kind_of(KindOf::Vehicle)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .set_health(250.0)
        .set_cost(500, 0);
    game_logic
        .templates
        .insert("USA_Humvee".to_string(), humvee);

    let mut player = Player::new(0, Team::USA, "AI", false);
    player.resources.supplies = 250;
    game_logic.add_player(player);

    let factory_id = game_logic
        .create_object("WarFactory", Team::USA, Vec3::new(0.0, 0.0, 0.0))
        .expect("war factory should be created");

    game_logic.frame = 600; // AI production pulse
    game_logic.update_ai(&[factory_id], 1.0 / 60.0);

    assert_eq!(
        game_logic.objects.len(),
        1,
        "AI should not spawn units for free when resources are insufficient"
    );
    assert_eq!(
        game_logic
            .get_player(0)
            .expect("player should exist")
            .resources
            .supplies,
        250,
        "resources should remain unchanged when production cannot be afforded"
    );
}

#[test]
fn ai_production_queues_units_instead_of_spawning_immediately() {
    let mut game_logic = GameLogic::new();

    let mut war_factory = ThingTemplate::new("WarFactory");
    war_factory
        .add_kind_of(KindOf::Structure)
        .add_kind_of(KindOf::Selectable)
        .set_health(1500.0)
        .set_cost(1000, -2);
    game_logic
        .templates
        .insert("WarFactory".to_string(), war_factory);

    let mut humvee = ThingTemplate::new("USA_Humvee");
    humvee
        .add_kind_of(KindOf::Vehicle)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .set_health(250.0)
        .set_cost(500, 0);
    game_logic
        .templates
        .insert("USA_Humvee".to_string(), humvee);

    let mut player = Player::new(0, Team::USA, "AI", false);
    player.resources.supplies = 1_000;
    game_logic.add_player(player);

    let factory_id = game_logic
        .create_object("WarFactory", Team::USA, Vec3::new(0.0, 0.0, 0.0))
        .expect("war factory should be created");

    game_logic.frame = 600; // AI production pulse
    game_logic.update_ai(&[factory_id], 1.0 / 60.0);

    assert_eq!(
        game_logic.objects.len(),
        1,
        "AI production should queue first instead of instantly spawning a unit"
    );
    assert_eq!(
        game_logic
            .get_player(0)
            .expect("player should exist")
            .effective_supplies(),
        500,
        "queued AI production should charge exactly once"
    );
    let queue = &game_logic
        .host_object(factory_id)
        .and_then(|factory| factory.building_data.as_ref())
        .expect("factory should have building data")
        .production_queue;
    assert_eq!(queue.len(), 1);
    assert_eq!(queue[0].template_name, "USA_Humvee");
}

#[test]
fn china_barracks_quantity_modifier_spawns_two_redguards_residual() {
    use crate::game_logic::buildings::BuildingType;
    use crate::game_logic::host_production_buildable_command_residual::{
        QUANTITY_MODIFIER_SAMPLE_COUNT, production_quantity_modifier,
    };
    use crate::game_logic::{
        KindOf, ProductionExitMetadata, ProductionExitStyle, Team, ThingTemplate, VeterancyLevel,
    };
    assert_eq!(QUANTITY_MODIFIER_SAMPLE_COUNT, 2);
    assert_eq!(
        production_quantity_modifier("ChinaBarracks", "ChinaInfantryRedguard"),
        2
    );
    assert_eq!(
        production_quantity_modifier("AmericaBarracks", "AmericaInfantryRanger"),
        1
    );

    let mut logic = GameLogic::new();
    ensure_test_player_for_team(&mut logic, Team::China);
    let mut bar = ThingTemplate::new("ChinaBarracks");
    bar.add_kind_of(KindOf::Structure)
        .add_kind_of(KindOf::FSBarracks)
        .set_health(1000.0);
    bar.production_exit_metadata = Some(ProductionExitMetadata {
        style: ProductionExitStyle::Queue,
        unit_create_point: [0.0, -25.0, 0.0],
        natural_rally_point: [36.0, -25.0, 0.0],
        exit_delay_frames: 9,
        allow_airborne_creation: false,
        initial_burst: 0,
        use_spawn_rally_point: false,
        grant_temporary_stealth_frames: 0,
    });
    logic.templates.insert("ChinaBarracks".into(), bar);
    let mut rg = ThingTemplate::new("ChinaInfantryRedguard");
    rg.add_kind_of(KindOf::Infantry)
        .set_health(100.0)
        .set_cost(100, 0);
    rg.build_time = 0.05;
    logic.templates.insert("ChinaInfantryRedguard".into(), rg);

    let bid = logic
        .create_object(
            "ChinaBarracks",
            Team::China,
            glam::Vec3::new(100.0, 0.0, 100.0),
        )
        .expect("barracks");
    if let Some(o) = logic.host_object_mut(bid) {
        o.building_data = Some(crate::game_logic::BuildingData::new(BuildingType::Barracks));
        // Orient for deterministic natural rally residual.
        o.thing.set_orientation(0.0);
    }
    // One queue entry pays once, QuantityModifier yields two exits.
    assert!(logic.enqueue_production(bid, "ChinaInfantryRedguard".into()));
    let qlen = logic
        .host_object(bid)
        .and_then(|o| o.building_data.as_ref())
        .map(|b| b.production_queue.len())
        .unwrap_or(0);
    assert_eq!(qlen, 1, "single queue entry for modifier batch");
    let qty = logic
        .host_object(bid)
        .and_then(|o| o.building_data.as_ref())
        .and_then(|b| b.production_queue.first())
        .map(|i| i.quantity_total)
        .unwrap_or(0);
    assert_eq!(qty, 2, "Redguard quantity_total residual");

    // The terminal build frame reaches a closed China barracks door first;
    // nothing may exit until it reaches C++ WAITING_OPEN.
    logic.update();
    let before_open = logic
        .host_objects()
        .values()
        .filter(|o| o.template_name == "ChinaInfantryRedguard" && o.is_alive())
        .count();
    assert_eq!(
        before_open, 0,
        "closed door must not release a batch member"
    );
    assert_eq!(
        logic
            .host_object(bid)
            .and_then(|o| o.building_data.as_ref())
            .map(|b| b.production_queue.len()),
        Some(1),
        "closed door retains the completed batch"
    );

    // Simulate the authored door reaching WAITING_OPEN.  A fresh Queue exit
    // has currentDelay=0/currentBurst=InitialBurst(0), so it admits exactly
    // one member before that successful exit arms its retail 300ms/9-frame
    // delay.
    let hold_open_until = logic.get_frame().saturating_add(100);
    if let Some(o) = logic.host_object_mut(bid) {
        o.production_door_phase = 2;
        o.production_door_phase_end_frame = hold_open_until;
    }
    logic.update();
    let living = logic
        .host_objects()
        .values()
        .filter(|o| o.template_name.contains("Redguard") && o.is_alive())
        .count();
    assert_eq!(
        living, 1,
        "fresh Queue exit must release one Red Guard before its delay, living={living}"
    );
    let first_state = logic
        .host_object(bid)
        .and_then(|o| o.building_data.as_ref())
        .expect("building after first Queue exit");
    assert_eq!(first_state.exit_delay_remaining_frames, 9);
    assert_eq!(first_state.exit_burst_remaining, 0);
    assert_eq!(first_state.production_queue[0].quantity_produced, 1);

    for _ in 0..8 {
        logic.update();
    }
    let held = logic
        .host_objects()
        .values()
        .filter(|o| o.template_name.contains("Redguard") && o.is_alive())
        .count();
    assert_eq!(held, 1, "Queue must hold the second Red Guard for 8 frames");
    logic.update();
    let released = logic
        .host_objects()
        .values()
        .filter(|o| o.template_name.contains("Redguard") && o.is_alive())
        .count();
    assert_eq!(
        released, 2,
        "9th Queue update releases the second Red Guard"
    );
    let qlen_end = logic
        .host_object(bid)
        .and_then(|o| o.building_data.as_ref())
        .map(|b| b.production_queue.len())
        .unwrap_or(99);
    assert_eq!(qlen_end, 0);
    // Exit path residual: units should be Moving toward authored natural rally.
    let moving = logic
        .host_objects()
        .values()
        .filter(|o| {
            o.template_name.contains("Redguard")
                && o.is_alive()
                && matches!(o.ai_state, AIState::Moving)
        })
        .count();
    assert!(
        moving >= 1,
        "at least one Redguard should be on exit path residual, moving={moving}"
    );
    assert!(
        logic.host_objects().values().any(|o| {
            o.template_name.contains("Redguard") && o.is_alive() && o.can_path_through_units
        }),
        "Queue exit must aiFollowExitProductionPath (can_path_through_units)"
    );
    let _ = VeterancyLevel::Rookie;
}

#[test]
fn queue_factory_exit_follows_snapped_production_path() {
    use crate::game_logic::buildings::BuildingType;
    use crate::game_logic::{
        KindOf, ProductionExitMetadata, ProductionExitStyle, Team, ThingTemplate,
    };

    let mut logic = GameLogic::new();
    ensure_test_player_for_team(&mut logic, Team::China);
    let mut bar = ThingTemplate::new("ChinaBarracks");
    bar.add_kind_of(KindOf::Structure)
        .add_kind_of(KindOf::FSBarracks)
        .set_health(1000.0);
    bar.production_exit_metadata = Some(ProductionExitMetadata {
        style: ProductionExitStyle::Queue,
        unit_create_point: [0.0, -25.0, 0.0],
        natural_rally_point: [36.0, -25.0, 0.0],
        exit_delay_frames: 9,
        allow_airborne_creation: false,
        initial_burst: 0,
        use_spawn_rally_point: false,
        grant_temporary_stealth_frames: 0,
    });
    logic.templates.insert("ChinaBarracks".into(), bar);
    let mut rg = ThingTemplate::new("ChinaInfantryRedguard");
    rg.add_kind_of(KindOf::Infantry).set_health(100.0);
    logic.templates.insert("ChinaInfantryRedguard".into(), rg);

    let bid = logic
        .create_object(
            "ChinaBarracks",
            Team::China,
            glam::Vec3::new(100.0, 15.0, 100.0),
        )
        .expect("barracks");
    if let Some(o) = logic.host_object_mut(bid) {
        o.building_data = Some(crate::game_logic::BuildingData::new(BuildingType::Barracks));
        o.thing.set_orientation(0.0);
    }
    let uid = logic
        .create_object(
            "ChinaInfantryRedguard",
            Team::China,
            glam::Vec3::new(100.0, 15.0, 75.0),
        )
        .expect("redguard");

    crate::game_logic::host_production_spawn_ready_log::clear();
    crate::game_logic::host_production_spawn_ready_log::record(
        uid,
        bid,
        "ChinaInfantryRedguard".into(),
        [100.0, 15.0, 75.0],
        None,
    );
    assert_eq!(logic.host_apply_production_spawn_ready_completions(), 1);

    let (can_path, ai_state, dest, path) = {
        let unit = logic.host_object(uid).expect("unit after exit");
        (
            unit.can_path_through_units,
            unit.ai_state.clone(),
            unit.movement.target_position,
            unit.movement.path.clone(),
        )
    };
    assert!(
        can_path,
        "FollowExitProductionPath sets can_path_through_units"
    );
    assert!(
        matches!(ai_state, AIState::Moving),
        "exit must start AIFollowExitProductionPath / Moving, got {:?}",
        ai_state
    );
    let dest = dest.expect("exit destination");
    let (producer_pos, forward, exit) = {
        let producer = logic.host_object(bid).expect("producer");
        (
            producer.get_position(),
            producer.thing.get_direction_vector(),
            producer
                .thing
                .template
                .production_exit_metadata
                .expect("queue metadata"),
        )
    };
    let raw_point = exit.natural_rally_point_with_path_offset(
        crate::game_logic::host_ai_path_combat_residual_wave105::PATHFIND_CELL_SIZE_F,
    );
    let raw =
        crate::game_logic::host_production_buildable_command_residual::transform_model_exit_offset(
            producer_pos,
            forward,
            (raw_point[0], raw_point[1], raw_point[2]),
        );
    assert!(
        dest.distance(raw) > 0.5,
        "Queue snapPosition must move the natural rally off the raw model point, dest={dest:?} raw={raw:?}"
    );
    assert!(
        path.len() >= 2,
        "Queue with no custom rally doubles the snapped natural, path={path:?}"
    );
    let last = *path.last().expect("path last");
    let prev = path[path.len() - 2];
    assert!(
        last.distance(prev) < 1.0,
        "doubled Queue natural must repeat the snapped point, prev={prev:?} last={last:?}"
    );
}

#[test]
fn factory_spawn_keeps_unit_create_point_without_selection_radius_jitter() {
    use crate::game_logic::buildings::BuildingType;
    use crate::game_logic::{
        KindOf, ProductionExitMetadata, ProductionExitStyle, Team, ThingTemplate,
    };

    let mut logic = GameLogic::new();
    ensure_test_player_for_team(&mut logic, Team::China);
    let mut bar = ThingTemplate::new("ChinaBarracks");
    bar.add_kind_of(KindOf::Structure)
        .add_kind_of(KindOf::FSBarracks)
        .set_health(1000.0);
    bar.production_exit_metadata = Some(ProductionExitMetadata {
        style: ProductionExitStyle::Queue,
        unit_create_point: [0.0, -25.0, 0.0],
        natural_rally_point: [36.0, -25.0, 0.0],
        exit_delay_frames: 9,
        allow_airborne_creation: false,
        initial_burst: 0,
        use_spawn_rally_point: false,
        grant_temporary_stealth_frames: 0,
    });
    logic.templates.insert("ChinaBarracks".into(), bar);
    let mut rg = ThingTemplate::new("ChinaInfantryRedguard");
    rg.add_kind_of(KindOf::Infantry).set_health(100.0);
    logic.templates.insert("ChinaInfantryRedguard".into(), rg);

    let bid = logic
        .create_object(
            "ChinaBarracks",
            Team::China,
            glam::Vec3::new(100.0, 15.0, 100.0),
        )
        .expect("barracks");
    if let Some(o) = logic.host_object_mut(bid) {
        o.building_data = Some(crate::game_logic::BuildingData::new(BuildingType::Barracks));
        o.thing.set_orientation(0.0);
    }
    let create_pos = glam::Vec3::new(100.0, 15.0, 75.0);
    let uid = logic
        .create_object("ChinaInfantryRedguard", Team::China, create_pos)
        .expect("redguard");
    if let Some(unit) = logic.host_object_mut(uid) {
        unit.selection_radius = 12.0;
    }

    crate::game_logic::host_production_spawn_ready_log::clear();
    crate::game_logic::host_production_spawn_ready_log::record(
        uid,
        bid,
        "ChinaInfantryRedguard".into(),
        [create_pos.x, create_pos.y, create_pos.z],
        None,
    );
    assert_eq!(logic.host_apply_production_spawn_ready_completions(), 1);

    let pos = logic
        .host_object(uid)
        .expect("unit after exit")
        .get_position();
    let dxz = ((pos.x - create_pos.x).powi(2) + (pos.z - create_pos.z).powi(2)).sqrt();
    assert!(
        dxz < 0.05,
        "C++ exitObjectViaDoor keeps UnitCreatePoint; selection-radius jitter is gone, pos={pos:?} create={create_pos:?}"
    );
}

#[test]
fn queue_exit_applies_airborne_motive_and_pitch_after_ground_snap() {
    use crate::game_logic::buildings::BuildingType;
    use crate::game_logic::{
        KindOf, MOTIVE_FRAMES_RESIDUAL, ProductionExitMetadata, ProductionExitStyle, Team,
        ThingTemplate,
    };

    let mut logic = GameLogic::new();
    ensure_test_player_for_team(&mut logic, Team::China);
    let w = logic.pathfinding_system.grid.width().max(8) as u32;
    let h = logic.pathfinding_system.grid.height().max(8) as u32;
    let heights = vec![0.0f32; (w * h) as usize];
    assert!(
        logic.restore_terrain_heights_from_grid(w, h, &heights),
        "flat ground cache"
    );

    let mut bar = ThingTemplate::new("ChinaBarracks");
    bar.add_kind_of(KindOf::Structure)
        .add_kind_of(KindOf::FSBarracks)
        .set_health(1000.0);
    bar.production_exit_metadata = Some(ProductionExitMetadata {
        style: ProductionExitStyle::Queue,
        unit_create_point: [0.0, -25.0, 15.0],
        natural_rally_point: [36.0, -25.0, 0.0],
        exit_delay_frames: 9,
        allow_airborne_creation: false,
        initial_burst: 0,
        use_spawn_rally_point: false,
        grant_temporary_stealth_frames: 0,
    });
    logic.templates.insert("ChinaBarracks".into(), bar);
    let mut rg = ThingTemplate::new("ChinaInfantryRedguard");
    rg.add_kind_of(KindOf::Infantry).set_health(100.0);
    rg.physics_mass = 5.0;
    rg.center_of_mass_offset = 2.5;
    logic.templates.insert("ChinaInfantryRedguard".into(), rg);

    let bid = logic
        .create_object(
            "ChinaBarracks",
            Team::China,
            glam::Vec3::new(20.0, 0.0, 20.0),
        )
        .expect("barracks");
    let producer_vel = glam::Vec3::new(8.0, 3.0, -2.0);
    if let Some(o) = logic.host_object_mut(bid) {
        o.building_data = Some(crate::game_logic::BuildingData::new(BuildingType::Barracks));
        o.thing.set_orientation(0.0);
        o.movement.velocity = producer_vel;
    }
    let create_pos = glam::Vec3::new(20.0, 15.0, 10.0);
    assert_eq!(logic.terrain_height_at(create_pos), Some(0.0));
    let uid = logic
        .create_object("ChinaInfantryRedguard", Team::China, create_pos)
        .expect("redguard");

    crate::game_logic::host_production_spawn_ready_log::clear();
    crate::game_logic::host_production_spawn_ready_log::record(
        uid,
        bid,
        "ChinaInfantryRedguard".into(),
        [create_pos.x, create_pos.y, create_pos.z],
        None,
    );
    assert_eq!(logic.host_apply_production_spawn_ready_completions(), 1);

    let unit = logic.host_object(uid).expect("unit after exit");
    assert!(
        (unit.get_position().y - 0.0).abs() < 1e-4,
        "AllowAirborneCreation false still snaps Y, pos={:?}",
        unit.get_position()
    );
    assert!(
        (unit.physics_accel - producer_vel).length() < 1e-4,
        "startingForce = producer vel * mass → accel == vel, accel={:?} vel={:?}",
        unit.physics_accel,
        producer_vel
    );
    assert_eq!(unit.motive_frames_remaining, MOTIVE_FRAMES_RESIDUAL);
    assert!(
        (unit.shock_pitch_rate - 2.5 * 0.04).abs() < 1e-5,
        "setPitchRate(COM * 0.04), got {}",
        unit.shock_pitch_rate
    );
}

#[test]
fn queue_exit_airborne_kick_keeps_y_when_allow_airborne() {
    use crate::game_logic::buildings::BuildingType;
    use crate::game_logic::{
        KindOf, MOTIVE_FRAMES_RESIDUAL, ProductionExitMetadata, ProductionExitStyle, Team,
        ThingTemplate,
    };

    let mut logic = GameLogic::new();
    ensure_test_player_for_team(&mut logic, Team::China);
    let w = logic.pathfinding_system.grid.width().max(8) as u32;
    let h = logic.pathfinding_system.grid.height().max(8) as u32;
    let heights = vec![0.0f32; (w * h) as usize];
    assert!(logic.restore_terrain_heights_from_grid(w, h, &heights));

    let mut bar = ThingTemplate::new("ChinaBarracks");
    bar.add_kind_of(KindOf::Structure)
        .add_kind_of(KindOf::FSBarracks)
        .set_health(1000.0);
    bar.production_exit_metadata = Some(ProductionExitMetadata {
        style: ProductionExitStyle::Queue,
        unit_create_point: [0.0, -25.0, 15.0],
        natural_rally_point: [36.0, -25.0, 0.0],
        exit_delay_frames: 9,
        allow_airborne_creation: true,
        initial_burst: 0,
        use_spawn_rally_point: false,
        grant_temporary_stealth_frames: 0,
    });
    logic.templates.insert("ChinaBarracks".into(), bar);
    let mut rg = ThingTemplate::new("ChinaInfantryRedguard");
    rg.add_kind_of(KindOf::Infantry).set_health(100.0);
    rg.physics_mass = 4.0;
    rg.center_of_mass_offset = -1.25;
    logic.templates.insert("ChinaInfantryRedguard".into(), rg);

    let bid = logic
        .create_object(
            "ChinaBarracks",
            Team::China,
            glam::Vec3::new(20.0, 0.0, 20.0),
        )
        .expect("barracks");
    let producer_vel = glam::Vec3::new(1.0, 6.0, 0.0);
    if let Some(o) = logic.host_object_mut(bid) {
        o.building_data = Some(crate::game_logic::BuildingData::new(BuildingType::Barracks));
        o.movement.velocity = producer_vel;
    }
    let create_pos = glam::Vec3::new(20.0, 15.0, 10.0);
    let uid = logic
        .create_object("ChinaInfantryRedguard", Team::China, create_pos)
        .expect("redguard");

    crate::game_logic::host_production_spawn_ready_log::clear();
    crate::game_logic::host_production_spawn_ready_log::record(
        uid,
        bid,
        "ChinaInfantryRedguard".into(),
        [create_pos.x, create_pos.y, create_pos.z],
        None,
    );
    assert_eq!(logic.host_apply_production_spawn_ready_completions(), 1);

    let unit = logic.host_object(uid).expect("unit after exit");
    assert!(
        (unit.get_position().y - 15.0).abs() < 1e-4,
        "AllowAirborneCreation keeps transformed Y, pos={:?}",
        unit.get_position()
    );
    assert!((unit.physics_accel - producer_vel).length() < 1e-4);
    assert_eq!(unit.motive_frames_remaining, MOTIVE_FRAMES_RESIDUAL);
    assert!((unit.shock_pitch_rate - (-1.25 * 0.04)).abs() < 1e-5);
}

#[test]
fn queue_exit_ground_spawn_skips_airborne_kick() {
    use crate::game_logic::buildings::BuildingType;
    use crate::game_logic::{
        KindOf, ProductionExitMetadata, ProductionExitStyle, Team, ThingTemplate,
    };

    let mut logic = GameLogic::new();
    ensure_test_player_for_team(&mut logic, Team::China);
    let w = logic.pathfinding_system.grid.width().max(8) as u32;
    let h = logic.pathfinding_system.grid.height().max(8) as u32;
    let heights = vec![0.0f32; (w * h) as usize];
    assert!(logic.restore_terrain_heights_from_grid(w, h, &heights));

    let mut bar = ThingTemplate::new("ChinaBarracks");
    bar.add_kind_of(KindOf::Structure)
        .add_kind_of(KindOf::FSBarracks)
        .set_health(1000.0);
    bar.production_exit_metadata = Some(ProductionExitMetadata {
        style: ProductionExitStyle::Queue,
        unit_create_point: [0.0, -25.0, 0.0],
        natural_rally_point: [36.0, -25.0, 0.0],
        exit_delay_frames: 9,
        allow_airborne_creation: false,
        initial_burst: 0,
        use_spawn_rally_point: false,
        grant_temporary_stealth_frames: 0,
    });
    logic.templates.insert("ChinaBarracks".into(), bar);
    let mut rg = ThingTemplate::new("ChinaInfantryRedguard");
    rg.add_kind_of(KindOf::Infantry).set_health(100.0);
    rg.center_of_mass_offset = 3.0;
    logic.templates.insert("ChinaInfantryRedguard".into(), rg);

    let bid = logic
        .create_object(
            "ChinaBarracks",
            Team::China,
            glam::Vec3::new(20.0, 0.0, 20.0),
        )
        .expect("barracks");
    if let Some(o) = logic.host_object_mut(bid) {
        o.building_data = Some(crate::game_logic::BuildingData::new(BuildingType::Barracks));
        o.movement.velocity = glam::Vec3::new(9.0, 0.0, 0.0);
    }
    let create_pos = glam::Vec3::new(20.0, 0.0, 10.0);
    let uid = logic
        .create_object("ChinaInfantryRedguard", Team::China, create_pos)
        .expect("redguard");

    crate::game_logic::host_production_spawn_ready_log::clear();
    crate::game_logic::host_production_spawn_ready_log::record(
        uid,
        bid,
        "ChinaInfantryRedguard".into(),
        [create_pos.x, create_pos.y, create_pos.z],
        None,
    );
    assert_eq!(logic.host_apply_production_spawn_ready_completions(), 1);

    let unit = logic.host_object(uid).expect("unit after exit");
    assert_eq!(unit.physics_accel, glam::Vec3::ZERO);
    assert_eq!(unit.motive_frames_remaining, 0);
    assert_eq!(unit.shock_pitch_rate, 0.0);
}

#[test]
fn attack_move_to_command_sets_moving_residual() {
    use crate::command_system::{CommandType, GameCommand, ModifierKeys};
    use crate::game_logic::{KindOf, Team, ThingTemplate};
    let mut logic = GameLogic::new();
    ensure_test_player_for_team(&mut logic, Team::USA);
    let mut t = ThingTemplate::new("AmericaInfantryRanger");
    t.add_kind_of(KindOf::Infantry)
        .add_kind_of(KindOf::Selectable)
        .set_health(100.0);
    logic.templates.insert("AmericaInfantryRanger".into(), t);
    let id = logic
        .create_object("AmericaInfantryRanger", Team::USA, glam::Vec3::ZERO)
        .expect("ranger");
    if let Some(p) = logic.get_player_mut(0) {
        p.selected_objects = vec![id];
    }
    logic.queue_command(GameCommand {
        command_type: CommandType::AttackMoveTo {
            destination: glam::Vec3::new(50.0, 0.0, 50.0),
            max_shots: -1,
        },
        player_id: 0,
        command_id: 1,
        timestamp: std::time::SystemTime::now(),
        selected_units: vec![id],
        modifier_keys: ModifierKeys::default(),
    });
    logic.process_commands();
    let o = logic.host_object(id).expect("alive");
    assert!(
        matches!(
            o.ai_state,
            AIState::Moving | AIState::AttackMoving | AIState::Attacking
        ) || o.movement.target_position.is_some()
            || !o.movement.path.is_empty(),
        "attack-move should order unit motion residual: state={:?}",
        o.ai_state
    );
}

#[test]
fn parsed_queue_exit_runtime_uses_authored_initial_burst_and_delay() {
    use crate::game_logic::buildings::BuildingType;
    use crate::game_logic::{
        KindOf, ProductionExitMetadata, ProductionExitStyle, Team, ThingTemplate,
    };

    let mut logic = GameLogic::new();
    ensure_test_player_for_team(&mut logic, Team::China);
    let mut bar = ThingTemplate::new("ChinaBarracks");
    bar.add_kind_of(KindOf::Structure)
        .add_kind_of(KindOf::FSBarracks)
        .set_health(1000.0);
    bar.production_exit_metadata = Some(ProductionExitMetadata {
        style: ProductionExitStyle::Queue,
        unit_create_point: [0.0, 0.0, 0.0],
        natural_rally_point: [0.0, 0.0, 0.0],
        exit_delay_frames: 9,
        allow_airborne_creation: false,
        initial_burst: 2,
        use_spawn_rally_point: false,
        grant_temporary_stealth_frames: 0,
    });
    logic.templates.insert("ChinaBarracks".into(), bar);
    let mut rg = ThingTemplate::new("ChinaInfantryRedguard");
    rg.add_kind_of(KindOf::Infantry)
        .set_health(100.0)
        .set_cost(100, 0);
    // Fast build residual for test.
    rg.build_time = 0.05;
    logic.templates.insert("ChinaInfantryRedguard".into(), rg);

    let bid = logic
        .create_object("ChinaBarracks", Team::China, glam::Vec3::ZERO)
        .expect("barracks");
    if let Some(o) = logic.host_object_mut(bid) {
        o.building_data = Some(crate::game_logic::BuildingData::new(BuildingType::Barracks));
    }
    assert!(logic.enqueue_production(bid, "ChinaInfantryRedguard".into()));
    // C++ Queue state begins at InitialBurst and decrements after each success.
    // This one entry deliberately retains the regular two-member modifier.
    if let Some(building) = logic
        .host_object_mut(bid)
        .and_then(|object| object.building_data.as_mut())
    {
        building.production_queue[0].quantity_total = 3;
        building.production_queue[0].construction_frames = 2;
    }
    let hold_open_until = logic.get_frame().saturating_add(100);
    if let Some(object) = logic.host_object_mut(bid) {
        object.production_door_phase = 2;
        object.production_door_phase_end_frame = hold_open_until;
    }
    logic.update();
    let living = logic
        .host_objects()
        .values()
        .filter(|o| o.template_name.contains("Redguard") && o.is_alive())
        .count();
    assert_eq!(
        living, 2,
        "InitialBurst=2 permits exactly two immediate exits"
    );
    let state = logic
        .host_object(bid)
        .and_then(|o| o.building_data.as_ref())
        .expect("Queue runtime state");
    assert_eq!(state.exit_delay_remaining_frames, 9);
    assert_eq!(state.exit_burst_remaining, 0);
    for _ in 0..8 {
        logic.update();
    }
    let held = logic
        .host_objects()
        .values()
        .filter(|o| o.template_name.contains("Redguard") && o.is_alive())
        .count();
    assert_eq!(held, 2, "delay blocks the third member for 8 frames");
    logic.update();
    let living_end = logic
        .host_objects()
        .values()
        .filter(|o| o.template_name.contains("Redguard") && o.is_alive())
        .count();
    assert_eq!(
        living_end, 3,
        "third redguard releases on the ninth post-exit frame, living={living_end}"
    );
}

#[test]
fn can_make_hero_maxed_out_at_one_residual() {
    use crate::game_logic::host_production_buildable_command_residual::{
        CANMAKE_MAXED_OUT_FOR_PLAYER, CANMAKE_OK, unit_max_simultaneous_of_type_residual,
    };
    use crate::game_logic::{KindOf, Team, ThingTemplate};
    assert_eq!(
        unit_max_simultaneous_of_type_residual("AmericaInfantryColonelBurton"),
        Some(1)
    );

    let mut logic = GameLogic::new();
    ensure_test_player_for_team(&mut logic, Team::USA);
    ensure_test_barracks_template(&mut logic);
    let mut burton = ThingTemplate::new("AmericaInfantryColonelBurton");
    burton
        .add_kind_of(KindOf::Infantry)
        .add_kind_of(KindOf::Hero)
        .set_health(200.0)
        .set_cost(1500, 0);
    burton.max_simultaneous_of_type = 1;
    logic
        .templates
        .insert("AmericaInfantryColonelBurton".into(), burton);

    let barracks = logic
        .create_object("TestBarracks", Team::USA, glam::Vec3::ZERO)
        .expect("barracks");
    assert_eq!(
        logic.can_make_unit(barracks, "AmericaInfantryColonelBurton"),
        CANMAKE_OK
    );
    assert!(logic.enqueue_production(barracks, "AmericaInfantryColonelBurton".into()));
    // Queued counts toward max residual.
    assert_eq!(
        logic.can_make_unit(barracks, "AmericaInfantryColonelBurton"),
        CANMAKE_MAXED_OUT_FOR_PLAYER
    );
    // Complete spawn also maxes.
    let mut logic2 = GameLogic::new();
    ensure_test_player_for_team(&mut logic2, Team::USA);
    ensure_test_barracks_template(&mut logic2);
    logic2.templates.insert(
        "AmericaInfantryColonelBurton".into(),
        logic
            .templates
            .get("AmericaInfantryColonelBurton")
            .unwrap()
            .clone(),
    );
    let b2 = logic2
        .create_object("TestBarracks", Team::USA, glam::Vec3::ZERO)
        .unwrap();
    let _ = logic2
        .create_object(
            "AmericaInfantryColonelBurton",
            Team::USA,
            glam::Vec3::new(50.0, 0.0, 0.0),
        )
        .unwrap();
    assert_eq!(
        logic2.can_make_unit(b2, "AmericaInfantryColonelBurton"),
        CANMAKE_MAXED_OUT_FOR_PLAYER
    );
    assert!(!logic2.enqueue_production(b2, "AmericaInfantryColonelBurton".into()));
}

#[test]
fn unique_building_max_simultaneous_from_ini_blocks_second() {
    use crate::game_logic::{KindOf, Team, ThingTemplate};

    let mut logic = GameLogic::new();
    ensure_test_player_for_team(&mut logic, Team::USA);
    let mut factory = ThingTemplate::new("TestUniqueWarFactory");
    factory
        .add_kind_of(KindOf::Structure)
        .set_health(2_000.0)
        .set_cost(2_000, 0);
    factory.max_simultaneous_of_type = 1;
    logic
        .templates
        .insert("TestUniqueWarFactory".into(), factory);

    let first =
        logic.create_object_under_construction("TestUniqueWarFactory", Team::USA, glam::Vec3::ZERO);
    assert!(first.is_some(), "first unique building must place");
    let second = logic.create_object_under_construction(
        "TestUniqueWarFactory",
        Team::USA,
        glam::Vec3::new(80.0, 0.0, 0.0),
    );
    assert!(
        second.is_none(),
        "hq-ss0x8: INI MaxSimultaneousOfType=1 must block a second copy"
    );
}

#[test]
fn max_simultaneous_link_key_counts_rebuild_hole() {
    use crate::game_logic::{KindOf, Team, ThingTemplate};

    let mut logic = GameLogic::new();
    ensure_test_player_for_team(&mut logic, Team::USA);
    let mut scud = ThingTemplate::new("TestLinkedScud");
    scud.add_kind_of(KindOf::Structure)
        .set_health(4_000.0)
        .set_cost(5_000, 0);
    scud.max_simultaneous_of_type = 1;
    scud.max_simultaneous_link_key = Some("Superweapon".into());
    logic.templates.insert("TestLinkedScud".into(), scud);

    let mut hole = ThingTemplate::new("TestLinkedScudHole");
    hole.add_kind_of(KindOf::Structure).set_health(500.0);
    hole.max_simultaneous_of_type = 1;
    hole.max_simultaneous_link_key = Some("Superweapon".into());
    logic.templates.insert("TestLinkedScudHole".into(), hole);

    let hole_id = logic
        .create_object("TestLinkedScudHole", Team::USA, glam::Vec3::ZERO)
        .expect("hole");
    assert!(logic.host_object(hole_id).is_some());
    let blocked = logic.create_object_under_construction(
        "TestLinkedScud",
        Team::USA,
        glam::Vec3::new(80.0, 0.0, 0.0),
    );
    assert!(
        blocked.is_none(),
        "hq-ss0x8: rebuild hole must consume MaxSimultaneousLinkKey"
    );
}

#[test]
fn can_make_aircraft_blocked_when_airfield_parking_full() {
    use crate::game_logic::buildings::{BuildingType, DEFAULT_PRODUCTION_QUEUE_LIMIT};
    use crate::game_logic::host_production_buildable_command_residual::{
        CANMAKE_OK, CANMAKE_PARKING_PLACES_FULL,
    };
    use crate::game_logic::{KindOf, ParkingPlaceMetadata, Team, ThingTemplate};

    let mut logic = GameLogic::new();
    ensure_test_player_for_team(&mut logic, Team::USA);
    let mut af_t = ThingTemplate::new("TestAirfield");
    af_t.add_kind_of(KindOf::Structure)
        .add_kind_of(KindOf::FSAirfield)
        .set_health(2000.0);
    af_t.parking_place = Some(ParkingPlaceMetadata {
        num_rows: 2,
        num_cols: 2,
        approach_height: 50.0,
        landing_deck_height_offset: 0.0,
        has_runways: true,
        park_in_hangars: true,
        heal_amount_per_second: 10.0,
    });
    logic.templates.insert("TestAirfield".into(), af_t);
    let mut jet_t = ThingTemplate::new("TestRaptor");
    jet_t
        .add_kind_of(KindOf::Aircraft)
        .set_health(200.0)
        .set_cost(1000, 0);
    logic.templates.insert("TestRaptor".into(), jet_t);

    let af = logic
        .create_object("TestAirfield", Team::USA, glam::Vec3::ZERO)
        .expect("af");
    // Force Airfield building type residual.
    if let Some(o) = logic.host_object_mut(af) {
        o.building_data = Some(crate::game_logic::BuildingData::new(BuildingType::Airfield));
    }
    assert_eq!(logic.can_make_unit(af, "TestRaptor"), CANMAKE_OK);

    // Fill the authored four `ParkingPlaceBehavior::m_spaces` entries.  A
    // building garrison is unrelated containment state and must not affect
    // airfield capacity.
    for i in 0..4 {
        let j = logic
            .create_object(
                "TestRaptor",
                Team::USA,
                glam::Vec3::new(10.0 * (i as f32 + 1.0), 0.0, 0.0),
            )
            .expect("jet");
        if let Some(jet) = logic.host_object_mut(j) {
            jet.set_contained_by(Some(af));
            jet.set_ai_state(AIState::Docked);
            jet.producer_id = Some(af);
            jet.airfield_parking_space_index = Some(i);
        }
    }
    assert_eq!(logic.airfield_parking_occupied_or_queued(af), 4);
    assert_eq!(
        logic.can_make_unit(af, "TestRaptor"),
        CANMAKE_PARKING_PLACES_FULL
    );
    assert!(!logic.enqueue_production(af, "TestRaptor".into()));

    // C++ `ProductionUpdate::canQueueCreateUnit` tests ParkingPlaceBehavior
    // before the generic queue-limit branch.  A 3×3 authored airfield can
    // naturally hold nine queued aircraft, so the tenth request reaches both
    // limits and must still report parking full.
    let mut queue_only_airfield = ThingTemplate::new("TestQueuePriorityAirfield");
    queue_only_airfield
        .add_kind_of(KindOf::Structure)
        .add_kind_of(KindOf::FSAirfield)
        .set_health(2000.0);
    queue_only_airfield.parking_place = Some(ParkingPlaceMetadata {
        num_rows: 3,
        num_cols: 3,
        approach_height: 50.0,
        landing_deck_height_offset: 0.0,
        has_runways: true,
        park_in_hangars: true,
        heal_amount_per_second: 10.0,
    });
    logic
        .templates
        .insert("TestQueuePriorityAirfield".into(), queue_only_airfield);
    let queued_airfield = logic
        .create_object(
            "TestQueuePriorityAirfield",
            Team::USA,
            glam::Vec3::new(200.0, 0.0, 0.0),
        )
        .expect("queue-priority airfield");
    if let Some(object) = logic.host_object_mut(queued_airfield) {
        object.building_data = Some(crate::game_logic::BuildingData::new(BuildingType::Airfield));
    }
    for _ in 0..DEFAULT_PRODUCTION_QUEUE_LIMIT {
        assert!(logic.enqueue_production(queued_airfield, "TestRaptor".into()));
    }
    assert_eq!(
        logic.can_make_unit(queued_airfield, "TestRaptor"),
        CANMAKE_PARKING_PLACES_FULL,
        "ParkingPlaceBehavior must win over the generic queue-full status"
    );
}

#[test]
fn can_make_unit_residual_gates_prereq_money_queue_disabled() {
    use crate::game_logic::buildings::DEFAULT_PRODUCTION_QUEUE_LIMIT;
    use crate::game_logic::host_production_buildable_command_residual::{
        CANMAKE_FACTORY_IS_DISABLED, CANMAKE_NO_MONEY, CANMAKE_NO_PREREQ, CANMAKE_OK,
        CANMAKE_QUEUE_FULL,
    };
    use crate::game_logic::{KindOf, Team, ThingTemplate};

    let mut logic = GameLogic::new();
    ensure_test_player_for_team(&mut logic, Team::USA);
    ensure_test_barracks_template(&mut logic);
    ensure_test_infantry_template(&mut logic);

    let barracks = logic
        .create_object("TestBarracks", Team::USA, glam::Vec3::ZERO)
        .expect("barracks");
    assert_eq!(logic.can_make_unit(barracks, "TestInfantry"), CANMAKE_OK);

    // No money residual.
    if let Some(p) = logic.get_player_mut(0) {
        p.resources.supplies = 0;
    }
    assert_eq!(
        logic.can_make_unit(barracks, "TestInfantry"),
        CANMAKE_NO_MONEY
    );
    assert!(!logic.enqueue_production(barracks, "TestInfantry".into()));

    if let Some(p) = logic.get_player_mut(0) {
        p.resources.supplies = 100_000;
    }
    // Disabled factory residual.
    if let Some(o) = logic.host_object_mut(barracks) {
        o.set_status_disabled_underpowered(true);
    }
    assert_eq!(
        logic.can_make_unit(barracks, "TestInfantry"),
        CANMAKE_FACTORY_IS_DISABLED
    );
    if let Some(o) = logic.host_object_mut(barracks) {
        o.set_status_disabled_underpowered(false);
    }

    // Queue full residual.
    for _ in 0..DEFAULT_PRODUCTION_QUEUE_LIMIT {
        assert!(logic.enqueue_production(barracks, "TestInfantry".into()));
    }
    assert_eq!(
        logic.can_make_unit(barracks, "TestInfantry"),
        CANMAKE_QUEUE_FULL
    );

    // Wrong factory type residual (vehicle at barracks).
    let mut veh = ThingTemplate::new("TestVehicleUnit");
    veh.add_kind_of(KindOf::Vehicle)
        .set_health(100.0)
        .set_cost(200, 0);
    logic.templates.insert("TestVehicleUnit".into(), veh);
    assert_eq!(
        logic.can_make_unit(barracks, "TestVehicleUnit"),
        CANMAKE_NO_PREREQ
    );
}

#[test]
fn can_make_unit_missing_or_dead_producer_is_no_prereq() {
    use crate::game_logic::host_production_buildable_command_residual::{
        CANMAKE_NO_PREREQ, CANMAKE_OK,
    };
    use crate::game_logic::{ObjectId, Team};

    let mut logic = GameLogic::new();
    ensure_test_player_for_team(&mut logic, Team::USA);
    ensure_test_barracks_template(&mut logic);
    ensure_test_infantry_template(&mut logic);
    if let Some(p) = logic.get_player_mut(0) {
        p.resources.supplies = 10_000;
    }
    let barracks = logic
        .create_object("TestBarracks", Team::USA, glam::Vec3::ZERO)
        .expect("barracks");
    assert_eq!(logic.can_make_unit(barracks, "TestInfantry"), CANMAKE_OK);
    assert_eq!(
        logic.can_make_unit(ObjectId(0xDEAD), "TestInfantry"),
        CANMAKE_NO_PREREQ
    );
    if let Some(o) = logic.host_object_mut(barracks) {
        o.health.current = 0.0;
    }
    assert_eq!(
        logic.can_make_unit(barracks, "TestInfantry"),
        CANMAKE_NO_PREREQ
    );
}

#[test]
fn can_make_unit_reports_disabled_and_maxed_before_prereq() {
    use crate::game_logic::host_production_buildable_command_residual::{
        CANMAKE_FACTORY_IS_DISABLED, CANMAKE_MAXED_OUT_FOR_PLAYER, CANMAKE_NO_PREREQ,
    };
    use crate::game_logic::{KindOf, Team, ThingTemplate};

    let mut logic = GameLogic::new();
    ensure_test_player_for_team(&mut logic, Team::USA);
    ensure_test_barracks_template(&mut logic);
    ensure_test_infantry_template(&mut logic);

    let barracks = logic
        .create_object("TestBarracks", Team::USA, glam::Vec3::ZERO)
        .expect("barracks");

    // Missing prereq + disabled factory → FACTORY_IS_DISABLED (C++ order).
    if let Some(tmpl) = logic.templates.get_mut("TestInfantry") {
        tmpl.buildable_status =
            crate::game_logic::host_production_buildable_command_residual::BSTATUS_NO;
    }
    if let Some(o) = logic.host_object_mut(barracks) {
        o.set_status_disabled_underpowered(true);
    }
    assert_eq!(
        logic.can_make_unit(barracks, "TestInfantry"),
        CANMAKE_FACTORY_IS_DISABLED
    );
    if let Some(o) = logic.host_object_mut(barracks) {
        o.set_status_disabled_underpowered(false);
    }

    // Missing prereq + maxed → MAXED_OUT (C++ does maxed before isPossibleToMakeUnit).
    if let Some(tmpl) = logic.templates.get_mut("TestInfantry") {
        tmpl.max_simultaneous_of_type = 1;
    }
    let _ = logic.create_object("TestInfantry", Team::USA, glam::Vec3::new(10.0, 0.0, 0.0));
    assert_eq!(
        logic.can_make_unit(barracks, "TestInfantry"),
        CANMAKE_MAXED_OUT_FOR_PLAYER
    );

    // TECHTREE leftover override NO gates live can_make (no factory disable/maxed).
    if let Some(tmpl) = logic.templates.get_mut("TestInfantry") {
        tmpl.max_simultaneous_of_type = 0;
        tmpl.buildable_status =
            crate::game_logic::host_production_buildable_command_residual::BSTATUS_YES;
    }
    let _ = gamelogic::scripting::take_host_buildable_status_override_requests();
    gamelogic::helpers::TheGameLogic::set_buildable_status_override("TestInfantry", 2);
    assert_eq!(
        logic.can_make_unit(barracks, "TestInfantry"),
        CANMAKE_NO_PREREQ
    );
    gamelogic::helpers::TheGameLogic::set_buildable_status_override("TestInfantry", 0);
}

#[test]
fn script_disable_unit_construction_and_techtree_override_write_live() {
    use crate::game_logic::Team;
    use crate::game_logic::host_production_buildable_command_residual::{
        BSTATUS_NO, CANMAKE_FACTORY_IS_DISABLED, CANMAKE_NO_PREREQ, CANMAKE_OK,
    };
    use gamelogic::scripting::HostScriptCanBuildRequest;

    let mut logic = GameLogic::new();
    logic.scripts_loaded = true;
    ensure_test_player_for_team(&mut logic, Team::USA);
    ensure_test_barracks_template(&mut logic);
    ensure_test_infantry_template(&mut logic);
    let barracks = logic
        .create_object("TestBarracks", Team::USA, glam::Vec3::ZERO)
        .expect("barracks");
    assert_eq!(logic.can_make_unit(barracks, "TestInfantry"), CANMAKE_OK);

    let _ = gamelogic::scripting::take_host_can_build_requests();
    gamelogic::scripting::request_host_can_build(HostScriptCanBuildRequest::Units {
        player: "TestPlayer".into(),
        enable: false,
    });
    logic.evaluate_and_execute_scripts(0.0);
    assert!(
        !logic.get_player(0).expect("p").can_build_units,
        "PLAYER_DISABLE_UNIT_CONSTRUCTION must flip live can_build_units"
    );
    assert_eq!(
        logic.can_make_unit(barracks, "TestInfantry"),
        CANMAKE_NO_PREREQ
    );

    gamelogic::scripting::request_host_can_build(HostScriptCanBuildRequest::Units {
        player: "TestPlayer".into(),
        enable: true,
    });
    logic.evaluate_and_execute_scripts(0.0);
    assert!(logic.get_player(0).expect("p").can_build_units);

    gamelogic::scripting::request_host_can_build(HostScriptCanBuildRequest::Factories {
        player: "TestPlayer".into(),
        template: "TestBarracks".into(),
        enable: false,
    });
    logic.evaluate_and_execute_scripts(0.0);
    assert!(
        logic
            .host_object(barracks)
            .expect("barracks")
            .is_script_disabled(),
        "PLAYER_DISABLE_FACTORIES must set live SCRIPT_DISABLED"
    );
    assert_eq!(
        logic.can_make_unit(barracks, "TestInfantry"),
        CANMAKE_FACTORY_IS_DISABLED
    );
    gamelogic::scripting::request_host_can_build(HostScriptCanBuildRequest::Factories {
        player: "TestPlayer".into(),
        template: "TestBarracks".into(),
        enable: true,
    });
    logic.evaluate_and_execute_scripts(0.0);

    let _ = gamelogic::scripting::take_host_buildable_status_override_requests();
    gamelogic::scripting::request_host_buildable_status_override("TestInfantry", BSTATUS_NO as i32);
    logic.evaluate_and_execute_scripts(0.0);
    assert_eq!(
        logic.can_make_unit(barracks, "TestInfantry"),
        CANMAKE_NO_PREREQ
    );
    assert!(
        logic
            .templates
            .get("TestInfantry")
            .expect("tmpl")
            .human_control_bar_buildable_hidden()
    );
    gamelogic::helpers::TheGameLogic::set_buildable_status_override("TestInfantry", 0);
}

#[test]
fn script_named_receive_upgrade_writes_live_object() {
    use gamelogic::scripting::HostScriptNamedUpgradeRequest;

    let mut logic = GameLogic::new();
    logic.scripts_loaded = true;
    ensure_test_player_for_team(&mut logic, Team::USA);
    ensure_test_infantry_template(&mut logic);
    let id = logic
        .create_object("TestInfantry", Team::USA, glam::Vec3::ZERO)
        .expect("unit");
    if let Some(obj) = logic.host_object_mut(id) {
        obj.name = "Hero".into();
    }

    let _ = gamelogic::scripting::take_host_script_named_upgrade_requests();
    gamelogic::scripting::request_host_script_named_upgrade(HostScriptNamedUpgradeRequest {
        unit: "Hero".into(),
        upgrade: "Upgrade_AmericaChemicalSuits".into(),
    });
    logic.evaluate_and_execute_scripts(0.0);
    assert!(
        logic
            .host_object(id)
            .expect("unit")
            .has_upgrade_tag("Upgrade_AmericaChemicalSuits"),
        "NAMED_RECEIVE_UPGRADE must apply giveUpgrade on the live host object"
    );
}

#[test]
fn script_player_sell_everything_sells_live_faction_structures() {
    use gamelogic::scripting::HostScriptPlayerMiscRequest;

    let mut logic = GameLogic::new();
    logic.scripts_loaded = true;
    ensure_test_player_for_team(&mut logic, Team::USA);
    ensure_test_barracks_template(&mut logic);
    ensure_test_infantry_template(&mut logic);
    let barracks = logic
        .create_object("TestBarracks", Team::USA, glam::Vec3::ZERO)
        .expect("barracks");
    let unit = logic
        .create_object("TestInfantry", Team::USA, glam::Vec3::new(50.0, 0.0, 0.0))
        .expect("unit");

    let _ = gamelogic::scripting::take_host_script_player_misc_requests();
    gamelogic::scripting::request_host_script_player_misc(
        HostScriptPlayerMiscRequest::SellEverything {
            player: "TestPlayer".into(),
        },
    );
    logic.evaluate_and_execute_scripts(0.0);
    assert!(
        logic.host_object(barracks).expect("barracks").status.sold,
        "PLAYER_SELL_EVERYTHING must start_sell_object on faction structures"
    );
    assert!(
        logic.sell_list.iter().any(|s| s.id == barracks),
        "sold barracks must be on the live sell list"
    );
    assert!(
        !logic.host_object(unit).expect("unit").status.sold,
        "non-structure units must not be sold"
    );
}

#[test]
fn script_exclude_from_score_and_select_skillset_write_live() {
    use crate::ai::AIDifficulty;
    use gamelogic::scripting::HostScriptPlayerMiscRequest;

    let mut logic = GameLogic::new();
    logic.scripts_loaded = true;
    ensure_test_player_for_team(&mut logic, Team::USA);
    logic
        .ai_manager
        .add_ai_player(0, Team::USA, AIDifficulty::Medium);
    assert!(logic.get_player(0).expect("p").list_in_score_screen);

    let _ = gamelogic::scripting::take_host_script_player_misc_requests();
    gamelogic::scripting::request_host_script_player_misc(
        HostScriptPlayerMiscRequest::ExcludeFromScore {
            player: "TestPlayer".into(),
        },
    );
    gamelogic::scripting::request_host_script_player_misc(
        HostScriptPlayerMiscRequest::SelectSkillset {
            player: "TestPlayer".into(),
            skillset: 2,
        },
    );
    logic.evaluate_and_execute_scripts(0.0);
    assert!(
        !logic.get_player(0).expect("p").list_in_score_screen,
        "PLAYER_EXCLUDE_FROM_SCORE_SCREEN must set live list_in_score_screen false"
    );
    assert_eq!(
        logic
            .ai_manager
            .ai_players
            .get(&0)
            .expect("ai")
            .selected_skillset(),
        1,
        "PLAYER_SELECT_SKILLSET is 1-based; live selector is 0-based"
    );
}

#[test]
fn script_player_kill_destroys_live_army() {
    use gamelogic::scripting::HostScriptPlayerMiscRequest;

    let mut logic = GameLogic::new();
    logic.scripts_loaded = true;
    ensure_test_player_for_team(&mut logic, Team::USA);
    ensure_test_infantry_template(&mut logic);
    let unit = logic
        .create_object("TestInfantry", Team::USA, glam::Vec3::ZERO)
        .expect("unit");

    let _ = gamelogic::scripting::take_host_script_player_misc_requests();
    gamelogic::scripting::request_host_script_player_misc(HostScriptPlayerMiscRequest::Kill {
        player: "TestPlayer".into(),
    });
    logic.evaluate_and_execute_scripts(0.0);
    assert!(
        logic.host_object(unit).is_none()
            || logic
                .host_object(unit)
                .is_some_and(|o| !o.is_alive() || o.status.destroyed),
        "PLAYER_KILL must destroy the live host army"
    );
}

#[test]
fn script_unit_destroy_all_contained_kills_live_occupants() {
    use gamelogic::scripting::{
        HostScriptKillDeleteDamageRequest, request_host_script_kill_delete_damage,
    };

    let mut logic = GameLogic::new();
    logic.scripts_loaded = true;
    ensure_test_player_for_team(&mut logic, Team::USA);
    ensure_test_infantry_template(&mut logic);
    ensure_test_barracks_template(&mut logic);
    let container = logic
        .create_object("TestBarracks", Team::USA, glam::Vec3::ZERO)
        .expect("container");
    let rider = logic
        .create_object("TestInfantry", Team::USA, glam::Vec3::new(10.0, 0.0, 0.0))
        .expect("rider");
    if let Some(obj) = logic.host_object_mut(container) {
        obj.name = "NamedChinook".into();
        let _ = obj.add_occupant(rider);
    }
    if let Some(obj) = logic.host_object_mut(rider) {
        obj.contained_by = Some(container);
    }

    let _ = gamelogic::scripting::take_host_script_kill_delete_damage_requests();
    request_host_script_kill_delete_damage(
        HostScriptKillDeleteDamageRequest::DestroyAllContained {
            unit: "NamedChinook".into(),
        },
    );
    logic.evaluate_and_execute_scripts(0.0);
    let rider_gone = logic
        .host_object(rider)
        .map(|o| !o.is_alive() || o.status.destroyed)
        .unwrap_or(true);
    assert!(
        rider_gone,
        "UNIT_DESTROY_ALL_CONTAINED must kill live occupants"
    );
    assert!(
        logic
            .host_object(container)
            .map(|o| o.contained_units().is_empty())
            .unwrap_or(true),
        "container roster must be empty after destroy-all-contained"
    );
}

#[test]
fn script_idle_all_units_and_resume_supply_write_live() {
    use crate::game_logic::AIState;
    use gamelogic::scripting::{HostScriptIdleRequest, request_host_script_idle};

    let mut logic = GameLogic::new();
    logic.scripts_loaded = true;
    ensure_test_player_for_team(&mut logic, Team::USA);
    ensure_test_infantry_template(&mut logic);
    ensure_test_dozer_template(&mut logic);
    let tank = logic
        .create_object("TestInfantry", Team::USA, glam::Vec3::ZERO)
        .expect("unit");
    let truck = logic
        .create_object("TestDozer", Team::USA, glam::Vec3::new(20.0, 0.0, 0.0))
        .expect("truck");
    let far = glam::Vec3::new(400.0, 0.0, 0.0);
    let _ = logic.unit_command_move_to(tank, far);
    if let Some(obj) = logic.host_object_mut(truck) {
        obj.set_ai_state(AIState::Idle);
        obj.supply_truck_force_pending = false;
    }

    let _ = gamelogic::scripting::take_host_script_idle_requests();
    request_host_script_idle(HostScriptIdleRequest::IdleAll {
        player: "TestPlayer".into(),
    });
    logic.evaluate_and_execute_scripts(0.0);
    let tank_pos = logic.host_object(tank).expect("tank").get_position();
    let dest = logic
        .host_object(tank)
        .and_then(|o| o.movement.target_position);
    assert!(
        dest.map(|d| (d - tank_pos).length() < 1.0).unwrap_or(true)
            || logic
                .host_object(tank)
                .map(|o| o.ai_state == AIState::Idle)
                .unwrap_or(false),
        "IDLE_ALL_UNITS must aiMoveToPosition(self) on live non-structures"
    );

    request_host_script_idle(HostScriptIdleRequest::ResumeSupply {
        player: "TestPlayer".into(),
    });
    logic.evaluate_and_execute_scripts(0.0);
    assert!(
        logic
            .host_object(truck)
            .expect("truck")
            .supply_truck_force_pending,
        "RESUME_SUPPLY_TRUCKING must setForceWantingState on idle collectors"
    );
}

#[test]
fn script_idle_and_guard_for_framecount_write_live() {
    use crate::game_logic::AIState;
    use gamelogic::scripting::{
        HostScriptHuntGuardRequest, HostScriptIdleRequest, request_host_script_hunt_guard,
        request_host_script_idle,
    };

    let mut logic = GameLogic::new();
    logic.scripts_loaded = true;
    ensure_test_player_for_team(&mut logic, Team::USA);
    ensure_test_infantry_template(&mut logic);
    let named = logic
        .create_object("TestInfantry", Team::USA, glam::Vec3::ZERO)
        .expect("named");
    let teammate = logic
        .create_object("TestInfantry", Team::USA, glam::Vec3::new(20.0, 0.0, 0.0))
        .expect("teammate");
    if let Some(obj) = logic.host_object_mut(named) {
        obj.name = "NamedRanger".into();
    }
    if let Some(obj) = logic.host_object_mut(teammate) {
        obj.team_instance_name = "USA_RangerSquad".into();
    }
    let far = glam::Vec3::new(400.0, 0.0, 0.0);
    let _ = logic.unit_command_move_to(named, far);
    let _ = logic.unit_command_move_to(teammate, far);

    let _ = gamelogic::scripting::take_host_script_idle_requests();
    request_host_script_idle(HostScriptIdleRequest::NamedStop {
        unit: "NamedRanger".into(),
    });
    request_host_script_idle(HostScriptIdleRequest::TeamStop {
        team: "USA_RangerSquad".into(),
        disband: false,
    });
    logic.evaluate_and_execute_scripts(0.0);
    assert!(
        logic
            .host_object(named)
            .map(|o| o.ai_state == AIState::Idle || !o.status.moving)
            .unwrap_or(false),
        "UNIT_IDLE_FOR_FRAMECOUNT must aiIdle the live named unit"
    );
    assert!(
        logic
            .host_object(teammate)
            .map(|o| o.ai_state == AIState::Idle || !o.status.moving)
            .unwrap_or(false),
        "TEAM_IDLE_FOR_FRAMECOUNT must groupIdle live team members"
    );

    request_host_script_hunt_guard(HostScriptHuntGuardRequest::NamedGuard {
        unit: "NamedRanger".into(),
    });
    logic.evaluate_and_execute_scripts(0.0);
    assert!(
        logic
            .host_object(named)
            .map(|o| matches!(
                o.ai_state,
                AIState::GuardingArea | AIState::GuardingObject | AIState::Idle
            ))
            .unwrap_or(false),
        "UNIT_GUARD_FOR_FRAMECOUNT must aiGuardPosition(self) on the live unit"
    );
}

#[test]
fn script_move_towards_nearest_writes_live() {
    use gamelogic::common::{AsciiString, ICoord3D};
    use gamelogic::polygon_trigger::PolygonTrigger;
    use gamelogic::scripting::{HostScriptMoveAttackRequest, request_host_script_move_attack};

    let trigger = PolygonTrigger::new(
        8801,
        AsciiString::from("NearestPad"),
        vec![
            ICoord3D::new(0, 0, 0),
            ICoord3D::new(80, 0, 0),
            ICoord3D::new(80, 80, 0),
            ICoord3D::new(0, 80, 0),
        ],
    );
    gamelogic::terrain::get_terrain_logic()
        .write()
        .expect("terrain")
        .add_trigger_area(trigger);

    let mut logic = GameLogic::new();
    logic.scripts_loaded = true;
    ensure_test_player_for_team(&mut logic, Team::USA);
    ensure_test_infantry_template(&mut logic);
    let mut cc = ThingTemplate::new("AmericaCommandCenter");
    cc.set_health(1000.0);
    logic.templates.insert("AmericaCommandCenter".into(), cc);

    let scout = logic
        .create_object("TestInfantry", Team::USA, glam::Vec3::new(5.0, 0.0, 5.0))
        .expect("scout");
    let dest = logic
        .create_object(
            "AmericaCommandCenter",
            Team::USA,
            glam::Vec3::new(40.0, 0.0, 40.0),
        )
        .expect("cc");
    if let Some(obj) = logic.host_object_mut(scout) {
        obj.name = "NamedScout".into();
        obj.team_instance_name = "USA_Scout".into();
    }

    let _ = gamelogic::scripting::take_host_script_move_attack_requests();
    request_host_script_move_attack(HostScriptMoveAttackRequest::NamedMoveTowardsNearest {
        unit: "NamedScout".into(),
        object_type: "AmericaCommandCenter".into(),
        trigger: "NearestPad".into(),
    });
    logic.evaluate_and_execute_scripts(0.0);
    let dest_pos = logic.host_object(dest).expect("cc").get_position();
    let moving = logic
        .host_object(scout)
        .and_then(|o| o.movement.target_position);
    assert!(
        moving
            .map(|p| (p - dest_pos).length() < 2.0)
            .unwrap_or(false),
        "MOVE_TOWARDS_NEAREST must aiMoveToObject the closest live type in the trigger: {moving:?}"
    );

    let _ = logic.unit_command_stop(scout);
    request_host_script_move_attack(HostScriptMoveAttackRequest::TeamMoveTowardsNearest {
        team: "USA_Scout".into(),
        object_type: "AmericaCommandCenter".into(),
        trigger: "NearestPad".into(),
    });
    logic.evaluate_and_execute_scripts(0.0);
    let dest_pos = logic.host_object(dest).expect("cc").get_position();
    let moving = logic
        .host_object(scout)
        .and_then(|o| o.movement.target_position);
    assert!(
        moving
            .map(|p| (p - dest_pos).length() < 2.0)
            .unwrap_or(false),
        "TEAM_MOVE_TOWARDS_NEAREST must aiMoveToObject the closest live type: {moving:?}"
    );
}

#[test]
fn script_wait_for_not_contained_uses_live_contained_by_census() {
    use gamelogic::scripting::core::{Parameter, ParameterType, ScriptAction, ScriptActionType};
    use gamelogic::scripting::executor::{
        ScriptActionDispatcher, ScriptActionResult, ScriptContext,
    };
    use std::sync::{Arc, RwLock};

    let mut logic = GameLogic::new();
    ensure_test_player_for_team(&mut logic, Team::USA);
    ensure_test_infantry_template(&mut logic);
    let rider = logic
        .create_object("TestInfantry", Team::USA, glam::Vec3::ZERO)
        .expect("rider");
    if let Some(obj) = logic.host_object_mut(rider) {
        obj.team_instance_name = "USA_Contained".into();
        obj.set_contained_by(Some(ObjectId(99)));
    }
    logic.inject_host_script_query_snapshot();
    assert_eq!(
        gamelogic::scripting::host_script_query_object_by_id(rider.0).map(|o| o.contained_by),
        Some(99)
    );

    let mut dispatcher = ScriptActionDispatcher::new(Arc::new(RwLock::new(ScriptContext::new())));
    let mut wait = ScriptAction::new(ScriptActionType::TeamWaitForNotContainedAll);
    wait.add_parameter(Parameter::with_string(
        ParameterType::Team,
        "USA_Contained".into(),
    ))
    .unwrap();
    assert_eq!(
        dispatcher.execute_action(&wait).unwrap(),
        ScriptActionResult::Pending(1.0)
    );

    if let Some(obj) = logic.host_object_mut(rider) {
        obj.set_contained_by(None);
    }
    logic.inject_host_script_query_snapshot();
    assert_eq!(
        dispatcher.execute_action(&wait).unwrap(),
        ScriptActionResult::Success
    );
}

#[test]
fn script_named_use_command_button_stops_live_unit() {
    use crate::game_logic::AIState;
    use gamelogic::scripting::{
        HostScriptUseCommandButtonRequest, request_host_script_use_command_button,
    };

    let mut logic = GameLogic::new();
    logic.scripts_loaded = true;
    ensure_test_player_for_team(&mut logic, Team::USA);
    ensure_test_infantry_template(&mut logic);
    let id = logic
        .create_object("TestInfantry", Team::USA, glam::Vec3::ZERO)
        .expect("unit");
    if let Some(obj) = logic.host_object_mut(id) {
        obj.name = "Hero".into();
        obj.team_instance_name = "teamAmerica".into();
    }
    let _ = logic.unit_command_move_to(id, glam::Vec3::new(300.0, 0.0, 0.0));

    let _ = gamelogic::scripting::take_host_script_use_command_button_requests();
    request_host_script_use_command_button(HostScriptUseCommandButtonRequest::Named {
        unit: "Hero".into(),
        button: "Command_Stop".into(),
    });
    logic.evaluate_and_execute_scripts(0.0);
    assert_eq!(
        logic.host_object(id).expect("unit").ai_state,
        AIState::Idle,
        "NAMED_USE_COMMANDBUTTON Command_Stop must idle the live unit"
    );

    let _ = logic.unit_command_move_to(id, glam::Vec3::new(300.0, 0.0, 0.0));
    request_host_script_use_command_button(HostScriptUseCommandButtonRequest::Team {
        team: "teamAmerica".into(),
        button: "Command_Stop".into(),
    });
    logic.evaluate_and_execute_scripts(0.0);
    assert_eq!(
        logic.host_object(id).expect("unit").ai_state,
        AIState::Idle,
        "TEAM_USE_COMMANDBUTTON Command_Stop must idle live team members"
    );
}

#[test]
fn script_set_base_construction_speed_writes_live_ai_team_seconds() {
    use crate::ai::AIDifficulty;
    use gamelogic::scripting::request_host_set_base_construction_speed;

    let mut logic = GameLogic::new();
    logic.scripts_loaded = true;
    ensure_test_player_for_team(&mut logic, Team::USA);
    logic
        .ai_manager
        .add_ai_player(0, Team::USA, AIDifficulty::Medium);
    assert_eq!(
        logic
            .ai_manager
            .ai_players
            .get(&0)
            .expect("ai")
            .team_seconds,
        crate::ai::AIPlayer::TEAM_SECONDS
    );

    let _ = gamelogic::scripting::take_host_set_base_construction_speed_requests();
    request_host_set_base_construction_speed("PlyrUSA", 3);
    logic.evaluate_and_execute_scripts(0.0);
    assert_eq!(
        logic
            .ai_manager
            .ai_players
            .get(&0)
            .expect("ai")
            .team_seconds,
        3.0,
        "SET_BASE_CONSTRUCTION_SPEED must write live AIPlayer.team_seconds"
    );
}

#[test]
fn script_team_nearest_and_partial_command_button_live() {
    use crate::game_logic::AIState;
    use gamelogic::scripting::{
        HostScriptTeamPartialCommandButtonRequest, HostScriptUseCommandButtonRequest,
        request_host_script_use_command_button, request_host_team_partial_command_button,
    };

    let mut logic = GameLogic::new();
    logic.scripts_loaded = true;
    ensure_test_player_for_team(&mut logic, Team::USA);
    ensure_test_player_for_team(&mut logic, Team::China);
    ensure_test_infantry_template(&mut logic);
    let mut hijacker = ThingTemplate::new("GLAInfantryHijacker");
    hijacker
        .add_kind_of(KindOf::Infantry)
        .add_kind_of(KindOf::Selectable)
        .set_health(100.0);
    logic
        .templates
        .insert("GLAInfantryHijacker".into(), hijacker);
    let near = logic
        .create_object("GLAInfantryHijacker", Team::USA, glam::Vec3::ZERO)
        .expect("near");
    let far = logic
        .create_object(
            "GLAInfantryHijacker",
            Team::USA,
            glam::Vec3::new(200.0, 0.0, 0.0),
        )
        .expect("far");
    if let Some(obj) = logic.host_object_mut(near) {
        obj.team_instance_name = "teamAmerica".into();
        obj.owner_player_id = Some(0);
    }
    if let Some(obj) = logic.host_object_mut(far) {
        obj.team_instance_name = "teamAmerica".into();
        obj.owner_player_id = Some(0);
    }
    let _ = logic.unit_command_move_to(near, glam::Vec3::new(300.0, 0.0, 0.0));
    let _ = logic.unit_command_move_to(far, glam::Vec3::new(400.0, 0.0, 0.0));

    let _ = gamelogic::scripting::take_host_team_partial_command_button_requests();
    request_host_team_partial_command_button(HostScriptTeamPartialCommandButtonRequest {
        team: "teamAmerica".into(),
        button: "Command_Stop".into(),
        percentage: 50.0,
    });
    logic.evaluate_and_execute_scripts(0.0);
    let near_idle = logic.host_object(near).expect("near").ai_state == AIState::Idle;
    let far_idle = logic.host_object(far).expect("far").ai_state == AIState::Idle;
    assert!(
        near_idle ^ far_idle,
        "TEAM_PARTIAL 50% of two members must stop exactly one, near_idle={near_idle} far_idle={far_idle}"
    );

    let enemy_near = logic
        .create_object("TestInfantry", Team::China, glam::Vec3::new(40.0, 0.0, 0.0))
        .expect("enemy near");
    let enemy_far = logic
        .create_object(
            "TestInfantry",
            Team::China,
            glam::Vec3::new(400.0, 0.0, 0.0),
        )
        .expect("enemy far");
    if let Some(obj) = logic.host_object_mut(enemy_near) {
        obj.owner_player_id = Some(1);
    }
    if let Some(obj) = logic.host_object_mut(enemy_far) {
        obj.owner_player_id = Some(1);
    }

    let _ = gamelogic::scripting::take_host_script_use_command_button_requests();
    request_host_script_use_command_button(HostScriptUseCommandButtonRequest::TeamOnNearestEnemy {
        team: "teamAmerica".into(),
        button: "Command_Hijack".into(),
    });
    logic.evaluate_and_execute_scripts(0.0);
    let near_target = logic.host_object(near).and_then(|o| o.target);
    let far_target = logic.host_object(far).and_then(|o| o.target);
    assert!(
        near_target == Some(enemy_near) || far_target == Some(enemy_near),
        "TEAM_ALL_USE_COMMANDBUTTON_ON_NEAREST_ENEMY must pick the closer China unit, near={near_target:?} far={far_target:?}"
    );
    assert_ne!(
        near_target.or(far_target),
        Some(enemy_far),
        "nearest search must not pick the distant enemy"
    );
}

#[test]
fn script_skirmish_command_button_most_valuable_does_button_not_force_attack() {
    use crate::game_logic::AIState;
    use gamelogic::scripting::request_host_skirmish_command_button_most_valuable;

    let mut logic = GameLogic::new();
    logic.scripts_loaded = true;
    ensure_test_player_for_team(&mut logic, Team::USA);
    ensure_test_player_for_team(&mut logic, Team::China);
    ensure_test_infantry_template(&mut logic);

    let attacker = logic
        .create_object("TestInfantry", Team::USA, glam::Vec3::ZERO)
        .expect("attacker");
    if let Some(obj) = logic.host_object_mut(attacker) {
        obj.team_instance_name = "teamAmerica".into();
        obj.owner_player_id = Some(0);
    }
    let _ = logic.unit_command_move_to(attacker, glam::Vec3::new(300.0, 0.0, 0.0));
    assert_ne!(
        logic.host_object(attacker).expect("attacker").ai_state,
        AIState::Idle
    );

    let cheap = logic
        .create_object("TestInfantry", Team::China, glam::Vec3::new(20.0, 0.0, 0.0))
        .expect("cheap");
    let mut expensive_tmpl = ThingTemplate::new("ExpensiveInfantry");
    expensive_tmpl
        .add_kind_of(KindOf::Infantry)
        .add_kind_of(KindOf::Selectable)
        .set_health(100.0);
    expensive_tmpl.build_cost.supplies = 5000;
    logic
        .templates
        .insert("ExpensiveInfantry".into(), expensive_tmpl);
    let expensive = logic
        .create_object(
            "ExpensiveInfantry",
            Team::China,
            glam::Vec3::new(40.0, 0.0, 0.0),
        )
        .expect("expensive");
    if let Some(obj) = logic.host_object_mut(cheap) {
        obj.owner_player_id = Some(1);
    }
    if let Some(obj) = logic.host_object_mut(expensive) {
        obj.owner_player_id = Some(1);
    }

    let _ = gamelogic::scripting::take_host_skirmish_command_button_most_valuable_requests();
    request_host_skirmish_command_button_most_valuable("teamAmerica", "Command_Stop", 200.0);
    logic.evaluate_and_execute_scripts(0.0);

    let after = logic.host_object(attacker).expect("attacker");
    assert_eq!(
        after.ai_state,
        AIState::Idle,
        "SKIRMISH most-valuable must execute the scripted button, not force-attack"
    );
    assert_eq!(
        after.target, None,
        "Command_Stop must not leave a force-attack target"
    );
}
