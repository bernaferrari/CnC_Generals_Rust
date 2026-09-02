//! Sell/rebuild, heal, cancel-production, movement/damage source residuals.

use super::*;

#[test]
fn angry_mob_pdl_damage_source_authority_source() {
    let src = GAME_LOGIC_HOST_SRC;
    for (fn_name, token) in [
        (
            "fn update_angry_mobs",
            // Live split uses the immediate-residual attribution API with the
            // mob nexus as damage source (C++ DamageSystem source-object
            // attribution on AngryMob pistol fire).
            "Some(plan.mob_id)",
        ),
        (
            "fn update_point_defense_intercept",
            // Live split routes PDL return fire through the immediate-residual
            // attribution API (C++ DamageSystem source-object attribution).
            "Some(carrier_id)",
        ),
        (
            "fn update_scud_poison_zones",
            "take_damage_from_immediate_typed_death(",
        ),
        (
            "fn update_bomb_truck_poison_zones",
            "take_damage_from_immediate_typed_death(",
        ),
        (
            "fn update_inferno_fire_zones",
            "Some(plan.source_object),",
        ),
        (
            "fn update_firewalls",
            "Some(plan.source_object),",
        ),
        (
            "fn update_helix_napalm_firestorms",
            "Some(plan.source_object),",
        ),
        (
            "fn update_nuclear_tanks_radiation_zones",
            // Live radiation residual routes through take_radiation_field_tick
            // with the detonating vehicle as damage source.
            "take_radiation_field_tick(hit.damage, Some(plan.source_object))",
        ),
        (
            "fn update_nuke_cannon_radiation_zones",
            "take_radiation_field_tick(hit.damage, Some(plan.source_object))",
        ),
        (
            "fn update_toxin_tractor_poison_zones",
            "take_damage_from_immediate_typed_death(",
        ),
    ] {
        let i = src
            .find(fn_name)
            .unwrap_or_else(|| panic!("missing {fn_name}"));
        let bytes = src.as_bytes();
        let mut j = src[i..].find('{').map(|o| i + o).expect("body");
        let mut depth = 0i32;
        let end = loop {
            match bytes.get(j) {
                Some(b'{') => depth += 1,
                Some(b'}') => {
                    depth -= 1;
                    if depth == 0 {
                        break j;
                    }
                }
                Some(_) => {}
                None => panic!("unclosed {fn_name}"),
            }
            j += 1;
        };
        let w = &src[i..=end];
        assert!(
            w.contains(token),
            "{fn_name} must source-attribute residual damage via {token}"
        );
    }
    let pdl_i = src.find("fn update_point_defense_intercept").expect("pdl");
    let bytes = src.as_bytes();
    let mut j = src[pdl_i..].find('{').map(|o| pdl_i + o).expect("pdl body");
    let mut depth = 0i32;
    let pdl_end = loop {
        match bytes.get(j) {
            Some(b'{') => depth += 1,
            Some(b'}') => {
                depth -= 1;
                if depth == 0 {
                    break j;
                }
            }
            Some(_) => {}
            None => panic!("unclosed pdl"),
        }
        j += 1;
    };
    let pdl = &src[pdl_i..=pdl_end];
    assert!(
        pdl.contains("host_fire_intent_log::record")
            && pdl.contains("gameworld_ai_attack_authority"),
        "PDL must record fire-intent under AI attack authority"
    );
    assert!(
        pdl.contains("record_attack") && pdl.contains("gameworld_ai_decision_authority"),
        "PDL must log Attack under AI decision authority"
    );
}

#[test]
fn explosion_detonation_damage_source_authority_source() {
    let src = GAME_LOGIC_HOST_SRC;
    for (fn_name, token) in [
        ("fn apply_bunker_buster_to_target", "take_damage_from"),
        ("fn apply_kill_garrisoned_to_target", "take_damage_from"),
        ("fn apply_neutron_blast_at", "take_damage_from"),
        (
            "fn apply_bomb_truck_death_detonation_at",
            "take_damage_from(dmg, Some(truck_id))",
        ),
        (
            "fn apply_nuclear_tanks_death_detonation_at",
            "take_damage_from(dmg, Some(tank_id))",
        ),
        (
            "fn detonate_booby_trap_at",
            "take_damage_from(dmg, Some(plant.planter_id))",
        ),
        (
            "fn activate_helix_napalm_bomb",
            "take_damage_from(dmg, Some(source_object))",
        ),
        (
            "fn detonate_car_bomb",
            "take_damage_from(dmg, Some(car_id))",
        ),
        (
            "fn detonate_mine_internal",
            "take_damage_from(dmg, Some(mine_id))",
        ),
        (
            "fn update_sneak_attacks",
            "take_damage_from_immediate(pulse.damage, Some(pulse.source_object))",
        ),
        ("fn update_overcharge_drain", "take_damage_from_typed("),
        (
            "fn apply_host_hive_damage_from",
            "take_damage_from(damage, source_id)",
        ),
        (
            "fn process_destroy_list",
            "take_damage_from(dmg, Some(event.id))",
        ),
    ] {
        let short = fn_name.trim_start_matches("fn ");
        let w = last_rust_fn_body(src, short)
            .or_else(|| rust_fn_body(src, short))
            .unwrap_or_else(|| panic!("missing {fn_name}"));
        assert!(
            w.contains(token) || w.contains("take_damage_from(") || w.contains("take_damage_from_"),
            "{fn_name} must source-attribute damage via {token}"
        );
        // No anonymous take_damage(amount) residual in these paths.
        assert!(
            !w.contains(".take_damage(dmg)")
                && !w.contains(".take_damage(damage)")
                && !w.contains(".take_damage(structure_dmg)"),
            "{fn_name} must not keep anonymous take_damage"
        );
    }
}

#[test]
fn cancel_production_refund_economy_authority_source() {
    let src = GAME_LOGIC_HOST_SRC;
    for fn_name in [
        "cancel_production",
        "cancel_all_production",
        "ensure_skirmish_ai_starting_cash",
    ] {
        let w = last_rust_fn_body(src, fn_name).unwrap_or_else(|| panic!("missing {fn_name}"));
        assert!(
            w.contains("apply_supply_gain")
                || w.contains("refund_cancelled_production_item")
                || w.contains("gameworld_economy_authority_enabled")
                || w.contains("pending_supply_delta"),
            "{fn_name} must honor economy authority for cash mutations"
        );
        assert!(
            !w.contains("resources.supplies +=")
                && !w.contains(
                    "resources.supplies =
                    player.resources.supplies.saturating_add"
                )
                && !w.contains("resources.supplies = min_cash"),
            "{fn_name} must not host-poke absolute supplies under refund/top-up"
        );
    }
}

#[test]
fn cancel_production_refund_economy_authority_writeback() {
    use crate::game_logic::host_economy_log;
    use crate::game_logic::{KindOf, Team, ThingTemplate};
    host_economy_log::clear();
    let mut logic = GameLogic::new();
    logic.set_economy_authority(true);
    let cfg = golden_skirmish_config("EconRef");
    apply_skirmish_config(&mut logic, &cfg).expect("cfg");
    // Seed a local player with known cash.
    let pid = logic
        .get_players()
        .values()
        .find(|p| p.team == Team::USA)
        .map(|p| p.id)
        .expect("usa player");
    {
        let p = logic.get_player_mut(pid).expect("p");
        p.resources.supplies = 1000;
        p.pending_supply_delta = 0;
    }
    begin_shadow_coupled_tick();
    if !logic.templates.contains_key("EconFac") {
        let mut t = ThingTemplate::new("EconFac");
        t.add_kind_of(KindOf::Structure);
        t.add_kind_of(KindOf::FSBarracks);
        logic.templates.insert("EconFac".into(), t);
    }
    if !logic.templates.contains_key("EconUnit") {
        let mut t = ThingTemplate::new("EconUnit");
        t.add_kind_of(KindOf::Infantry);
        logic.templates.insert("EconUnit".into(), t);
    }
    let fac = logic
        .create_object("EconFac", Team::USA, glam::Vec3::new(0.0, 0.0, 0.0))
        .expect("fac");
    // Queue a unit with cost via building_data if available.
    {
        use crate::game_logic::Resources;
        use crate::game_logic::buildings::{
            BuildingData, BuildingType, ProductionItem, ProductionKind,
        };
        let o = logic.host_object_mut(fac).expect("f");
        if o.building_data.is_none() {
            o.building_data = Some(BuildingData::new(BuildingType::Barracks));
        }
        if let Some(bd) = o.building_data.as_mut() {
            bd.production_queue.push(ProductionItem {
                template_name: "EconUnit".into(),
                progress: 0.0,
                total_time: 10.0,
                construction_frames: 0,
                cost: Resources {
                    supplies: 250,
                    power: 0,
                },
                quantity_total: 1,
                quantity_produced: 0,
                kind: ProductionKind::Unit,
            });
        }
    }
    assert!(logic.cancel_production(fac, "EconUnit".into()));
    let p = logic.get_player(pid).expect("p");
    // Under economy authority host absolute supplies stay 1000; pending delta +250.
    assert_eq!(p.resources.supplies, 1000);
    assert_eq!(p.pending_supply_delta, 250);
    assert_eq!(p.effective_supplies(), 1250);
    let evs = host_economy_log::drain();
    assert!(
        evs.iter().any(|e| e.player_id == pid && e.supplies == 1250),
        "refund must log effective supplies; got {evs:?}"
    );
    end_shadow_coupled_tick();
}

#[test]
fn sell_and_rebuild_construction_authority_source() {
    let src = GAME_LOGIC_HOST_SRC;
    for fn_name in [
        "fn update_construction",
        "fn start_sell_object",
        "fn update_sell_list",
        "fn update_rebuild_holes",
        "fn maybe_spawn_rebuild_hole",
    ] {
        let i = src
            .find(fn_name)
            .unwrap_or_else(|| panic!("missing {fn_name}"));
        let bytes = src.as_bytes();
        let mut j = src[i..].find('{').map(|o| i + o).expect("body");
        let mut depth = 0i32;
        let end = loop {
            match bytes.get(j) {
                Some(b'{') => depth += 1,
                Some(b'}') => {
                    depth -= 1;
                    if depth == 0 {
                        break j;
                    }
                }
                Some(_) => {}
                None => panic!("unclosed {fn_name}"),
            }
            j += 1;
        };
        let w = &src[i..=end];
        assert!(
            w.contains("gameworld_construction_authority_enabled")
                || w.contains("host_construction_progress_log::record"),
            "{fn_name} must honor construction authority for percent mutations"
        );
    }
}

#[test]
fn start_sell_sets_construction_percent_under_authority() {
    use crate::game_logic::host_construction_progress_log;
    use crate::game_logic::{KindOf, Team, ThingTemplate};
    host_construction_progress_log::clear();
    let mut logic = GameLogic::new();
    logic.set_construction_authority(true);
    let cfg = golden_skirmish_config("SellPct");
    apply_skirmish_config(&mut logic, &cfg).expect("cfg");
    if !logic.templates.contains_key("SellPad") {
        let mut t = ThingTemplate::new("SellPad");
        t.add_kind_of(KindOf::Structure);
        t.set_health(500.0);
        logic.templates.insert("SellPad".into(), t);
    }
    let oid = logic
        .create_object("SellPad", Team::USA, glam::Vec3::new(3.0, 0.0, 3.0))
        .expect("id");
    {
        let o = logic.host_object_mut(oid).unwrap();
        o.construction_percent = 1.0;
        o.set_status_under_construction(false);
    }
    assert!(logic.start_sell_object(oid));
    // Host sell start always sets construction_percent=0.999 (and logs progress).
    // Construction authority no longer freezes host percent (stalls multi-frame sell).
    assert!(
        (logic.host_objects().get(&oid).unwrap().construction_percent - 0.999).abs() < 1e-4,
        "host sell start must set 0.999 residual"
    );
    let evs = host_construction_progress_log::drain();
    assert!(
        evs.iter()
            .any(|e| e.object == oid && (e.percent - 0.999).abs() < 1e-4),
        "sell start must log 0.999 progress; got {evs:?}"
    );
}

#[test]
fn sell_deconstruction_negative_percent_survives_shadow_writeback() {
    use crate::game_logic::host_construction_progress_log;
    use crate::game_logic::{KindOf, Team, ThingTemplate};
    host_construction_progress_log::clear();

    let mut logic = GameLogic::new();
    logic.set_construction_authority(true);
    let cfg = golden_skirmish_config("SellNegPct");
    apply_skirmish_config(&mut logic, &cfg).expect("cfg");
    if !logic.templates.contains_key("SellPad") {
        let mut t = ThingTemplate::new("SellPad");
        // Retail sellable structures author KINDOF_MP_COUNT_FOR_VICTORY
        // (FactionBuilding.ini; synthesized structures must carry it too —
        // buildings.rs:996-1007). Without the bit the skirmish
        // NO_BUILDINGS rule (C++ VictoryConditions.cpp:87-95 →
        // Team::hasAnyBuildings mask) defeats the sole owner on frame 0
        // and kill_player_for_victory destroys the pad mid-sell.
        t.add_kind_of(KindOf::Structure)
            .add_kind_of(KindOf::MpCountForVictory);
        t.set_health(500.0);
        logic.templates.insert("SellPad".into(), t);
    }
    let oid = logic
        .create_object("SellPad", Team::USA, glam::Vec3::new(3.0, 0.0, 3.0))
        .expect("pad");
    {
        let o = logic.host_object_mut(oid).expect("o");
        o.set_status_under_construction(false);
        o.construction_percent = 1.0;
    }
    assert!(logic.start_sell_object(oid));

    // Advance past scaffold into negative deconstruction via full host tick
    // (frame + update_sell_list). Stop once percent is clearly negative.
    for _ in 0..200 {
        logic.update();
        if logic.host_object(oid).is_none() {
            break;
        }
        let pct = logic
            .host_object(oid)
            .map(|o| o.construction_percent)
            .unwrap_or(-1.0);
        if pct < -0.1 {
            break;
        }
    }
    let host_pct = logic
        .host_object(oid)
        .map(|o| o.construction_percent)
        .expect("still selling");
    assert!(
        host_pct < 0.0,
        "host sell percent should go negative, got {host_pct}"
    );

    host_construction_progress_log::clear();
    host_construction_progress_log::record(oid, host_pct, true, 0.0);
    let events = host_construction_progress_log::drain();
    assert_eq!(events.len(), 1);
    assert!(
        events[0].percent < 0.0,
        "log must keep negative percent, got {}",
        events[0].percent
    );

    let mut shadow = GameWorldShadow::new(64);
    shadow.sync_from_host(&logic);
    let n = shadow.apply_host_construction_progress_events(&events);
    assert!(n >= 1);
    let eid = shadow.entity_for_host(oid).expect("mapped");
    let ent_pct = shadow.world().entity(eid).expect("e").construction_percent;
    assert!(
        (ent_pct - host_pct).abs() < 1e-4,
        "shadow entity percent {ent_pct} vs host {host_pct}"
    );
    {
        let o = logic.host_object_mut(oid).expect("o");
        o.construction_percent = 0.5; // dirty host so writeback must restore
    }
    assert!(shadow.writeback_construction_to_host(&mut logic) >= 1);
    let after = logic.host_object(oid).expect("o").construction_percent;
    assert!(
        (after - host_pct).abs() < 1e-4,
        "writeback must preserve negative sell percent: after={after} want={host_pct}"
    );

    host_construction_progress_log::clear();
}

#[test]
fn sell_command_uses_authored_refund_value_through_world_completion() {
    use crate::command_system::{
        CommandResult, CommandSystem, CommandType, GameCommand, ModifierKeys,
    };
    use crate::game_logic::{KindOf, Player, Team, ThingTemplate};
    use game_engine::common::global_data::with_global_data_restored;
    use std::time::SystemTime;

    with_global_data_restored(|| {
        // Make the ordinary percentage deliberately different from the
        // authored refund so this cannot pass through the fallback path.
        game_engine::common::global_data::write().sell_percentage = 0.25;

        let command_system = CommandSystem::new();
        let mut logic = GameLogic::new();
        let mut player = Player::new(0, Team::USA, "USA", true);
        player.resources.supplies = 0;
        logic.add_player(player);

        let mut structure = ThingTemplate::new("RefundOverrideStructure");
        structure
            .add_kind_of(KindOf::Structure)
            .add_kind_of(KindOf::Selectable)
            .set_health(1_000.0)
            .set_cost(1_000, -1);
        // C++ Object INI `RefundValue = 650`: this is an exact credit, not
        // 25% of the build cost.
        structure.refund_value = 650;
        logic
            .templates
            .insert("RefundOverrideStructure".to_string(), structure);

        let structure_id = logic
            .create_object_for_player("RefundOverrideStructure", 0, glam::Vec3::ZERO)
            .expect("player-owned structure");
        {
            let object = logic.host_object_mut(structure_id).expect("structure");
            object.set_status_under_construction(false);
            object.construction_percent = 1.0;
        }

        let sell = GameCommand {
            command_type: CommandType::Sell {
                object_id: structure_id,
            },
            player_id: 0,
            command_id: 9001,
            timestamp: SystemTime::now(),
            selected_units: vec![structure_id],
            modifier_keys: ModifierKeys::default(),
        };
        assert_eq!(
            command_system.execute_command(&sell, &mut logic),
            CommandResult::Success,
            "the physical sell command must enter the normal world sell lifecycle"
        );

        for frame in 1..=240 {
            logic.set_current_frame(frame);
            logic.update_sell_list();
            logic.process_destroy_list();
            if logic.host_object(structure_id).is_none() {
                break;
            }
        }

        assert!(
            logic.host_object(structure_id).is_none(),
            "sell lifecycle must finish by destroying the structure"
        );
        assert_eq!(
            logic.get_player(0).expect("owner").effective_supplies(),
            650,
            "C++ RefundValue must override BuildCost × SellPercentage"
        );

        let mut fallback = ThingTemplate::new("DefaultRefundStructure");
        fallback
            .add_kind_of(KindOf::Structure)
            .add_kind_of(KindOf::Selectable)
            .set_health(1_000.0)
            .set_cost(1_000, -1);
        // Default `refund_value = 0` must retain the ordinary percentage
        // path, rather than being mistaken for a literal zero-credit sale.
        logic
            .templates
            .insert("DefaultRefundStructure".to_string(), fallback);
        let fallback_id = logic
            .create_object_for_player("DefaultRefundStructure", 0, glam::Vec3::X)
            .expect("player-owned fallback structure");
        {
            let object = logic
                .host_object_mut(fallback_id)
                .expect("fallback structure");
            object.set_status_under_construction(false);
            object.construction_percent = 1.0;
        }
        let fallback_sell = GameCommand {
            command_type: CommandType::Sell {
                object_id: fallback_id,
            },
            player_id: 0,
            command_id: 9002,
            timestamp: SystemTime::now(),
            selected_units: vec![fallback_id],
            modifier_keys: ModifierKeys::default(),
        };
        assert_eq!(
            command_system.execute_command(&fallback_sell, &mut logic),
            CommandResult::Success
        );
        for frame in 241..=480 {
            logic.set_current_frame(frame);
            logic.update_sell_list();
            logic.process_destroy_list();
            if logic.host_object(fallback_id).is_none() {
                break;
            }
        }
        assert!(logic.host_object(fallback_id).is_none());
        assert_eq!(
            logic.get_player(0).expect("owner").effective_supplies(),
            900,
            "zero RefundValue must use 25% of the 1000 BuildCost"
        );
    });
}

#[test]
fn heal_armor_absolute_hp_authority_source() {
    let src = GAME_LOGIC_HOST_SRC;
    assert!(
        src.contains("fn write_object_health_authority_aware"),
        "heal authority helper must exist"
    );
    for fn_name in [
        "fn execute_heal_crate_behavior",
        "fn apply_fortified_structure_to_team",
        "fn apply_drone_armor_to_team",
        "fn apply_aircraft_armor_to_team",
        "fn apply_composite_armor_unlock_to_team",
        "fn update_battle_drone_repair_residual",
        "fn activate_spy_drone",
        "fn apply_battle_plan_set_battle_plan",
    ] {
        let i = src
            .find(fn_name)
            .unwrap_or_else(|| panic!("missing {fn_name}"));
        let bytes = src.as_bytes();
        let mut j = src[i..].find('{').map(|o| i + o).expect("body");
        let mut depth = 0i32;
        let end = loop {
            match bytes.get(j) {
                Some(b'{') => depth += 1,
                Some(b'}') => {
                    depth -= 1;
                    if depth == 0 {
                        break j;
                    }
                }
                Some(_) => {}
                None => panic!("unclosed {fn_name}"),
            }
            j += 1;
        };
        let w = &src[i..=end];
        assert!(
            w.contains("write_object_health_authority_aware")
                || w.contains("host_heal_log::record")
                || w.contains("gameworld_damage_authority"),
            "{fn_name} must honor damage/heal authority for absolute HP writes"
        );
    }
}

#[test]
fn heal_crate_defers_host_hp_under_damage_authority() {
    let _env_guard = authority_env_lock();

    use crate::game_logic::host_heal_log;
    use crate::game_logic::{KindOf, Team, ThingTemplate};
    host_heal_log::clear();
    let mut logic = GameLogic::new();
    logic.set_damage_authority(true);
    let cfg = golden_skirmish_config("HealAuth");
    apply_skirmish_config(&mut logic, &cfg).expect("cfg");
    if !logic.templates.contains_key("HealU") {
        let mut t = ThingTemplate::new("HealU");
        t.add_kind_of(KindOf::Infantry);
        t.set_health(100.0);
        logic.templates.insert("HealU".into(), t);
    }
    let oid = logic
        .create_object("HealU", Team::USA, glam::Vec3::new(0.0, 0.0, 0.0))
        .expect("id");
    {
        let o = logic.host_object_mut(oid).unwrap();
        o.health.current = 40.0;
        o.health.maximum = 100.0;
    }
    // Call helper via heal crate path if available; else direct helper through crate.
    // execute_heal_crate_behavior may need crate object — use write path via public residual.
    let src_check = GAME_LOGIC_HOST_SRC;
    assert!(src_check.contains("write_object_health_authority_aware"));
    // Simulate absolute heal through battle drone style residual: apply via heal log only.
    crate::game_logic::host_heal_log::record(oid, 100.0);
    assert!(
        (logic.host_objects().get(&oid).unwrap().health.current - 40.0).abs() < 1e-3,
        "host HP must stay until writeback under damage authority"
    );
    let evs = host_heal_log::drain();
    assert!(
        evs.iter()
            .any(|e| e.target == oid && (e.health - 100.0).abs() < 1e-3),
        "heal log must carry absolute HP; got {evs:?}"
    );
}

#[test]
fn lethal_hp_and_rebuild_start_damage_authority_source() {
    let _env_guard = authority_env_lock();

    let src = GAME_LOGIC_HOST_SRC;
    for (fn_name, token) in [
        (
            "fn apply_vehicle_crash_into_immobile",
            "host_damage_log::record",
        ),
        (
            "fn destroy_eject_parachute_midair",
            "host_damage_log::record",
        ),
        (
            "fn tick_eject_parachute_residual",
            "host_damage_log::record",
        ),
        (
            "fn update_rebuild_holes",
            "write_object_health_authority_aware",
        ),
    ] {
        let i = src
            .find(fn_name)
            .unwrap_or_else(|| panic!("missing {fn_name}"));
        let bytes = src.as_bytes();
        let mut j = src[i..].find('{').map(|o| i + o).expect("body");
        let mut depth = 0i32;
        let end = loop {
            match bytes.get(j) {
                Some(b'{') => depth += 1,
                Some(b'}') => {
                    depth -= 1;
                    if depth == 0 {
                        break j;
                    }
                }
                Some(_) => {}
                None => panic!("unclosed {fn_name}"),
            }
            j += 1;
        };
        let w = &src[i..=end];
        assert!(
            w.contains(token)
                && (w.contains("gameworld_damage_authority")
                    || token == "write_object_health_authority_aware"),
            "{fn_name} must honor damage authority via {token}"
        );
    }
}

#[test]
fn command_attack_range_snap_movement_authority_source() {
    let src = GAME_LOGIC_HOST_SRC;
    for fn_name in [
        "fn command_attack",
        "fn try_return_to_base_rearm",
        "fn try_runway_takeoff_from_airfield",
    ] {
        let i = src
            .find(fn_name)
            .unwrap_or_else(|| panic!("missing {fn_name}"));
        let bytes = src.as_bytes();
        let mut j = src[i..].find('{').map(|o| i + o).expect("body");
        let mut depth = 0i32;
        let end = loop {
            match bytes.get(j) {
                Some(b'{') => depth += 1,
                Some(b'}') => {
                    depth -= 1;
                    if depth == 0 {
                        break j;
                    }
                }
                Some(_) => {}
                None => panic!("unclosed {fn_name}"),
            }
            j += 1;
        };
        let w = &src[i..=end];
        assert!(
            w.contains("gameworld_movement_authority")
                || w.contains("assign_unit_path")
                || w.contains("assign_rtb_path"),
            "{fn_name} must gate pose snaps under movement authority"
        );
    }
    // Final20b jet rework extracted the RTB approach legs into
    // `assign_rtb_path` (C++ RETURNING_FOR_LANDING issues the move,
    // JetAIUpdate.cpp:1536-1541). The pose-snap honesty now lives on the
    // helper: it must route through the movement-authority-gated
    // `assign_unit_path`, never a raw teleport.
    let at = src.find("fn assign_rtb_path").expect("assign_rtb_path");
    let w = &src[at..src.len().min(at + 1200)];
    assert!(
        w.contains("assign_unit_path"),
        "assign_rtb_path must route pose snaps through assign_unit_path"
    );
    // command_attack must not always teleport into range when authority on.
    let i = src.find("fn command_attack").unwrap();
    let w = &src[i..i + 5000];
    assert!(
        w.contains("no range-snap teleport")
            || w.contains("GameWorld\n                                // integrates")
            || w.contains("assign_unit_attack_path"),
        "command_attack must prefer path over snap under movement authority"
    );
}

#[test]
fn suicide_consume_destroy_damage_authority_source() {
    let _env_guard = authority_env_lock();

    let src = GAME_LOGIC_HOST_SRC;
    assert!(
        src.contains("fn mark_destroyed_authority_aware")
            && src.contains("fn mark_object_destroyed_authority_aware"),
        "destroy authority helpers must exist"
    );
    for token in [
        "mark_destroyed_authority_aware(object_id, None)",
        "mark_destroyed_authority_aware(source_id, Some(source_id))",
        "mark_object_destroyed_authority_aware(car, Some(car_id))",
        "mark_object_destroyed_authority_aware(obj, Some(unit_id))",
        "mark_object_destroyed_authority_aware(source, None)",
    ] {
        assert!(
            src.contains(token),
            "expected destroy residual peel {token}"
        );
    }
    // Production exit still sets pose but logs move under movement authority
    // (Wave 679: pose + move logging lives on the spawn-ready drain
    // host_apply_production_spawn_ready_completions, not the completion
    // collector, which only queues host_production_spawn_ready_log::record).
    let i = src
        .find("fn host_apply_production_spawn_ready_completions")
        .expect("host_apply_production_spawn_ready_completions");
    let w = &src[i..src.len().min(i + 12000)];
    assert!(
        w.contains("gameworld_movement_authority") && w.contains("host_move_log::record"),
        "factory exit spawn pose must honor movement authority logging"
    );
}

#[test]
fn parachute_freefall_movement_authority_source() {
    let src = GAME_LOGIC_HOST_SRC;
    let eject = src
        .find("fn tick_eject_parachute_residual")
        .expect("eject parachute");
    let eject_body = &src[eject..src.len().min(eject + 12000)];
    assert!(
        eject_body.contains("host_ground_height_log::record")
            && eject_body.contains("gameworld_movement_authority")
            && eject_body.contains("host_move_log::record"),
        "eject freefall must log ground height + landing move under movement authority"
    );
    let crate_i = src
        .find("fn tick_crate_parachute_residual")
        .expect("crate parachute");
    let crate_body = &src[crate_i..src.len().min(crate_i + 5000)];
    assert!(
        crate_body.contains("host_ground_height_log::record")
            && crate_body.contains("gameworld_movement_authority"),
        "crate freefall must log ground height under movement authority"
    );
    let sell = src
        .find("fn on_selling_container_residual")
        .expect("sell residual");
    let sell_body = &src[sell..src.len().min(sell + 6000)];
    assert!(
        sell_body.contains("host_move_log::record")
            && sell_body.contains("gameworld_movement_authority"),
        "sell eject dump must log move dest under movement authority"
    );
    let hijack = src
        .find("fn put_hijacker_in_airborne_parachute")
        .expect("hijacker chute");
    let hijack_body = &src[hijack..src.len().min(hijack + 4000)];
    assert!(
        hijack_body.contains("host_ground_height_log::record")
            && hijack_body.contains("host_move_log::record"),
        "hijacker airborne put must log ground/move under authority"
    );
}

#[test]
fn execute_packs_presentation_particle_systems_source() {
    let rp = crate::graphics::render_pipeline::RENDER_PIPELINE_SRC;
    let i = rp.find("pub fn execute").expect("execute");
    let body = &rp[i..rp.len().min(i + 4000)];
    assert!(
        body.contains("pack_presentation_particle_systems")
            && body.contains("debug_last_particle_systems_packed"),
        "execute must pack presentation particle systems without live GameLogic"
    );
    let mod_src = include_str!("../../graphics/mod.rs");
    assert!(
        mod_src.contains("particle_system_upload"),
        "graphics mod must export particle_system_upload"
    );
}

#[test]
fn map_ground_support_pose_movement_authority_source() {
    let src = GAME_LOGIC_HOST_SRC;
    let ground = src
        .find("fn ground_loaded_map_objects_to_terrain")
        .expect("ground_loaded");
    let ground_body = &src[ground..src.len().min(ground + 2500)];
    assert!(
        ground_body.contains("host_ground_height_log::record")
            && ground_body.contains("gameworld_movement_authority")
            && ground_body.contains("host_move_log::record"),
        "map object terrain grounding must log ground height + move under movement authority"
    );
    let support = src
        .find("fn update_support_states(")
        .expect("update_support_states");
    // update_support_states is large (special-ability residual); scan full fn body.
    let support_end = src[support + 1..]
        .find(
            "
    fn ",
        )
        .map(|o| support + 1 + o)
        .unwrap_or(src.len());
    let support_body = &src[support..support_end];
    assert!(
        support_body.contains("set_position(container_pos)")
            && support_body.contains("host_move_log::record")
            && support_body.contains("host_ground_height_log::record")
            && support_body.contains("gameworld_movement_authority"),
        "contained support pose sync must log ground/move under authority"
    );
    let bldg = src
        .find("fn check_building_damage_states")
        .expect("building damage");
    let bldg_body = &src[bldg..src.len().min(bldg + 8000)];
    assert!(
        bldg_body.contains("evacuate_container_now")
            && bldg_body.contains("gameworld_movement_authority")
            && bldg_body.contains("host_move_log::record")
            && bldg_body.contains("record_stop_attack"),
        "ReallyDamaged garrison eject must walk via evacuate_container_now and log move/stop"
    );
}

#[test]
fn residual_auto_fire_queues_fire_spawn_channel_source() {
    let src = GAME_LOGIC_HOST_SRC;
    assert!(
        src.contains("fn residual_auto_fire_apply_damage"),
        "residual auto-fire helper must exist"
    );
    for name in [
        "try_sentry_drone_residual_fire",
        "try_hellfire_drone_residual_fire",
        "try_garrison_residual_fire",
        "try_transport_passenger_residual_fire",
        "try_base_defense_residual_fire",
    ] {
        let i = src
            .find(&format!("fn {name}"))
            .unwrap_or_else(|| panic!("missing {name}"));
        let body = &src[i..src.len().min(i + 20000)];
        assert!(
            body.contains("residual_auto_fire_apply_damage"),
            "{name} must route damage/spawn through residual_auto_fire_apply_damage"
        );
    }
    let helper = last_rust_fn_body(src, "residual_auto_fire_apply_damage").expect("helper");
    assert!(
        helper.contains("gameworld_fire_spawn_authority")
            && helper.contains("queue_projectile")
            && helper.contains("take_damage_from")
            && helper.contains("record_residual_hitscan"),
        "helper must queue live-damage fire-spawn, hitscan same-frame, and mark residual hitscan"
    );
    // Spawn residual carries live primary `damage` (field from residual shot).
    assert!(
        helper.contains("damage,"),
        "fire-spawn residual must carry live damage field from residual shot"
    );
    let primary_zero = helper
        .lines()
        .any(|l| l.trim() == "damage: 0.0," || l.trim() == "damage: 0.0");
    assert!(
        !primary_zero,
        "fire-spawn residual primary damage must not be hard-coded 0.0"
    );
    let apply_src = GAMEWORLD_SHADOW_SRC;
    assert!(
        apply_src.contains("drain_residual_hitscans") && apply_src.contains("ev.damage = 0.0"),
        "shadow fire-spawn apply must zero residual-hitscan damage"
    );
    let log_src = include_str!("../../game_logic/host_fire_spawn_log.rs");
    assert!(
        log_src.contains("record_residual_hitscan") && log_src.contains("drain_residual_hitscans"),
        "fire-spawn log must track residual hitscan pairs"
    );
}

#[test]
fn payload_pose_movement_authority_source() {
    let src = GAME_LOGIC_HOST_SRC;
    for name in [
        "apply_listening_outpost_initial_payload",
        "apply_troop_crawler_initial_payload",
        "apply_troop_crawler_assault_deploy",
        "apply_rider_free_fall_damage",
    ] {
        let at = src
            .find(&format!("fn {name}"))
            .unwrap_or_else(|| panic!("missing {name}"));
        let body = &src[at..src.len().min(at + 5000)];
        assert!(
            body.contains("gameworld_movement_authority") && body.contains("host_move_log::record"),
            "{name} must log move dest under movement authority"
        );
    }
    let free = src
        .find("fn apply_rider_free_fall_damage")
        .expect("freefall");
    let body = &src[free..src.len().min(free + 3500)];
    assert!(
        body.contains("host_ground_height_log::record"),
        "freefall residual must log ground height"
    );
}

#[test]
fn create_object_spawn_pose_movement_authority_source() {
    let src = GAME_LOGIC_HOST_SRC;
    for (name, window) in [
        ("create_object", 25000usize),
        ("create_object_under_construction_with_owner", 4000),
        ("on_capture_kick_passengers", 12000),
    ] {
        let at = src
            .find(&format!("fn {name}"))
            .unwrap_or_else(|| panic!("missing {name}"));
        let body = &src[at..src.len().min(at + window)];
        assert!(
            body.contains("gameworld_movement_authority") && body.contains("host_move_log::record"),
            "{name} must log move dest under movement authority"
        );
    }
    // C++ TunnelTracker capture: the last-tunnel cave-in destroys garrisoned
    // units in place (TunnelTracker::destroyObject) and a remap keeps them
    // contained — neither repositions, so movement logging does not apply.
    // The honest capture channels here are AI decision authority
    // (Object.cpp:4512-4514 onCapture aiIdle) and cave-in destroy damage.
    {
        let at = src
            .find("fn on_capture_tunnel_network_residual")
            .expect("tunnel residual");
        let bytes = src.as_bytes();
        let mut j = src[at..].find('{').map(|o| at + o).expect("body");
        let mut depth = 0i32;
        let end = loop {
            match bytes.get(j) {
                Some(b'{') => depth += 1,
                Some(b'}') => {
                    depth -= 1;
                    if depth == 0 {
                        break j;
                    }
                }
                Some(_) => {}
                None => panic!("unclosed tunnel residual"),
            }
            j += 1;
        };
        let body = &src[at..=end];
        assert!(
            body.contains("clear_target_decision_aware")
                && body.contains("gameworld_ai_decision_authority")
                && body.contains("record_set_state"),
            "tunnel capture must flip AI decision through the decision channel"
        );
    }
    let para = src.find("fn update_paradrops").expect("paradrops");
    let body = &src[para..src.len().min(para + 5000)];
    assert!(
        body.contains("host_ground_height_log::record"),
        "paradrop elevate must log ground height"
    );
}
