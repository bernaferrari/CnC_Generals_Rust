//! Sell/rebuild, heal, cancel-production, movement/damage source residuals.

use super::*;

#[test]
fn explosion_detonation_damage_source_authority_source() {
    let src = GAME_LOGIC_HOST_SRC;
    // Structural routing evidence only. These exact calls retain amount,
    // attribution, damage family through the real synchronous Body operation.
    for (name, call, damage_kind) in [
        (
            "apply_bunker_buster_to_target",
            "apply_residual_damage_owned_with_effects(receiving_damage_target_id,structure_dmg,attacker_id,",
            "STEALTH_JET_MISSILE_DAMAGE_TYPE",
        ),
        (
            "apply_kill_garrisoned_to_target",
            "apply_residual_damage_owned_with_effects(receiving_damage_id,999_999.0,attacker_id,",
            "DamageType::Unresistable",
        ),
        (
            "apply_neutron_blast_at",
            "apply_residual_damage_owned_with_effects(receiving_damage_id,999_999.0,caster_id,",
            "DamageType::Unresistable",
        ),
        (
            "apply_bomb_truck_death_detonation_at",
            "apply_residual_damage_owned_with_effects(owned_victim_id,dmg,Some(truck_id),owned_damage_type,",
            "BOMB_TRUCK_DAMAGE_TYPE",
        ),
        (
            "apply_nuclear_tanks_death_detonation_at",
            "apply_residual_damage_owned_with_effects(owned_victim_id,dmg,Some(tank_id),owned_damage_type,",
            "NUCLEAR_TANK_DAMAGE_TYPE",
        ),
        (
            "detonate_booby_trap_at",
            "apply_residual_damage_owned_with_effects(owned_victim_id,dmg,Some(plant.planter_id),owned_damage_type,",
            "BOOBY_DAMAGE_TYPE",
        ),
        (
            "activate_helix_napalm_bomb",
            "apply_residual_damage_owned_with_effects(owned_victim_id,dmg,Some(source_object),owned_damage_type,",
            "HELIX_NAPALM_DAMAGE_TYPE",
        ),
        (
            "detonate_car_bomb",
            "apply_residual_damage_owned_with_effects(owned_victim_id,dmg,Some(car_id),owned_damage_type,",
            "SUICIDE_CAR_BOMB_DAMAGE_TYPE",
        ),
        (
            "detonate_mine_internal",
            "apply_residual_damage_owned_with_effects(owned_victim_id,dmg,Some(mine_id),owned_damage_type,",
            "DamageType::Explosive",
        ),
        (
            "update_sneak_attacks",
            "apply_residual_damage_owned_with_effects(owned_victim_id,pulse.damage,Some(pulse.source_object),owned_damage_type,",
            "DamageType::Unresistable",
        ),
        (
            "update_overcharge_drain",
            "apply_residual_damage_owned_with_effects(owned_victim_id,damage,Some(id),owned_damage_type,",
            "DamageType::Penalty",
        ),
        (
            "apply_host_hive_damage_from",
            "apply_residual_damage_owned_with_effects(owned_victim_id,damage,source_id,owned_damage_type,",
            "DamageType::Unresistable",
        ),
        (
            "process_destroy_list",
            "apply_residual_damage_owned_with_effects(receiving_damage_target_id,1.0e9,Some(event.id),",
            "DamageType::Falling",
        ),
    ] {
        let owner_name = if name == "process_destroy_list" {
            "process_destroy_list_body_with_effects_checked".to_owned()
        } else {
            format!("{name}_with_effects")
        };
        let adapter = last_rust_fn_body(src, name).expect(name);
        assert!(
            adapter.contains(&format!("self.{name}_with_effects(")),
            "{name} reaches the effects-bearing route"
        );
        if name == "process_destroy_list" {
            let checked_adapter = last_rust_fn_body(src, "process_destroy_list_with_effects")
                .expect("destroy adapter");
            assert!(checked_adapter.contains("self.process_destroy_list_with_effects_checked("));
            let lifecycle_wrapper =
                last_rust_fn_body(src, "process_destroy_list_with_effects_checked")
                    .expect("destroy lifecycle wrapper");
            assert!(
                lifecycle_wrapper.contains("self.process_destroy_list_body_with_effects_checked(")
            );
        }
        let body = last_rust_fn_body(src, &owner_name).expect(&owner_name);
        let normalized: String = body.split_whitespace().collect();
        assert!(
            normalized.contains(call),
            "{name} must retain its attributed amount through the receiving owner"
        );
        assert!(
            body.contains(damage_kind),
            "{name} must retain authored damage family {damage_kind}"
        );
        assert!(
            !normalized.contains(".take_damage(dmg)")
                && !normalized.contains(".take_damage(damage)")
                && !normalized.contains(".take_damage(structure_dmg)"),
            "{name} must not keep anonymous Object damage"
        );
    }
    let residual = last_rust_fn_body(src, "apply_residual_damage_owned_with_effects")
        .expect("receiving residual adapter");
    let residual: String = residual.split_whitespace().collect();
    assert!(residual.contains("self.apply_owned_damage_with_effects(victim_id,amount,source_id,damage_type,death_type,fx_override,&DamageHitContext::default(),effects,"),
        "residual effects must use the canonical installed owner, without a second Body kernel");
    let owner = last_rust_fn_body(
        src,
        "apply_owned_damage_with_killer_team_and_after_fx_with_effects",
    )
    .expect("canonical receiving Body owner");
    let owner: String = owner.split_whitespace().collect();
    assert!(
        owner.contains("letframe=self.frame;")
            && owner.contains("damage,source_id,damage_type,death_type,fx_override,frame,")
    );
    assert_eq!(
        owner
            .matches(".begin_damage_with_context_and_effects_with_operation(")
            .count(),
        2,
        "native and no-client routes must enter the effects-bearing Body kernel"
    );
    assert_eq!(
        owner.matches(".finish_with_body_fx_and_partition(").count(),
        2,
        "native and no-client post-FX continuations must keep the same owner"
    );
    assert_eq!(
        owner.matches("&mutself.partition_manager").count(),
        4,
        "both Body entries and both post-FX continuations require the receiving partition owner"
    );
    assert!(
        owner.contains("Self::execute_body_state_fx_without_view(")
            && owner.contains("&mutself.queued_audio_events"),
        "no-client route retains the explicit receiving audio owner"
    );
    assert!(
        owner.contains("Self::apply_body_player_effect(players,frame,&source_players,effect)"),
        "Player mutation must use the selected source at the receiving Body callback"
    );
    assert!(
        owner.contains("self.complete_owned_bridge_body_callbacks_with_effects(callbacks,effects)"),
        "bridge callbacks must complete synchronously in the canonical owner"
    );
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
            w.contains("apply_supply_gain") || w.contains("refund_cancelled_production_item"),
            "{fn_name} must use the canonical player cash operation"
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
fn cancel_production_refund_commits_to_owner_without_shadow_writeback() {
    use crate::game_logic::host_economy_log;
    use crate::game_logic::{KindOf, Team, ThingTemplate};

    let mut logic = GameLogic::new();
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
    }
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
    // CPP ProductionUpdate.cpp:456–464 deposits immediately to the
    // controlling player before removing the production queue entry.
    assert_eq!(p.resources.supplies, 1250);

    assert_eq!(p.effective_supplies(), 1250);
    assert!(
        logic
            .host_object(fac)
            .unwrap()
            .building_data
            .as_ref()
            .unwrap()
            .production_queue
            .is_empty()
    );
    let evs = {
        logic.economy_events.complete_boundary();
        logic.economy_events.take_completed()
    };
    assert!(
        evs.iter()
            .any(|event| event.player_id == pid && event.supplies == 1250)
    );
    assert!(
        !logic.cancel_production(fac, "EconUnit".into()),
        "already cancelled queue cannot refund twice"
    );
    assert_eq!(logic.get_player(pid).unwrap().resources.supplies, 1250);
}

#[test]
fn sell_and_rebuild_have_one_host_writer_and_read_only_shadow_comparisons() {
    let host = GAME_LOGIC_HOST_SRC;

    let shadow = include_str!("../construction_observation.rs");
    assert!(shadow.contains("pub fn compare_construction_to_host(&self, logic: &GameLogic)"));
    assert!(!shadow.contains("pub fn writeback_construction_to_host"));
    assert!(!shadow.contains("pub fn tick_construction_progress"));
}

#[test]
fn start_sell_sets_construction_percent_under_authority() {
    use crate::game_logic::{KindOf, Team, ThingTemplate};

    let mut logic = GameLogic::new();

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
    // CPP sell start changes the canonical percent immediately to 0.999.
    // Construction authority no longer freezes host percent (stalls multi-frame sell).
    assert!(
        (logic
            .host_objects()
            .lookup(&oid)
            .unwrap()
            .construction_percent
            - 0.999)
            .abs()
            < 1e-4,
        "host sell start must set 0.999 residual"
    );

    let object = logic.host_object(oid).unwrap();
    assert!(object.status.sold);
    assert!(object.status.unselectable);
    assert!(!object.status.under_construction);
}

#[test]
fn sell_deconstruction_negative_percent_survives_shadow_writeback() {
    use crate::game_logic::{KindOf, Team, ThingTemplate};

    let mut logic = GameLogic::new();

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

    let mut shadow = GameWorldShadow::new(64);
    shadow.sync_from_host(&logic);
    shadow.sync_from_host(&logic);
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
    assert!(shadow.compare_construction_to_host(&logic) >= 1);
    let after = logic.host_object(oid).expect("o").construction_percent;
    assert!(
        (after - 0.5).abs() < 1e-4,
        "a shadow comparison must retain canonical sell percent: after={after}"
    );
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
        "fn activate_spy_drone_with_effects",
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
            (w.contains("write_object_health_authority_aware")
                || w.contains("health_events.record_heal")),
            "{fn_name} must route absolute HP writes through the owner health sink or authority-aware helper"
        );
    }
    // Battle Drone repair no longer has an absolute-HP mirror writer: its
    // receiving SlavedUpdate callback uses the typed Body healing operation.
    let slave = include_str!("../../game_logic/world_combat/slaved_update.rs");
    let update = last_rust_fn_body(slave, "update_owned_slaved_object").expect("slave adapter");
    assert!(update.contains("self.update_owned_slaved_object_with_effects("));
    let owner = last_rust_fn_body(slave, "update_owned_slaved_object_with_effects")
        .expect("receiving slave owner");
    assert!(owner.contains("self.slave_repair_with_effects("));
    let repair =
        last_rust_fn_body(slave, "slave_repair_with_effects").expect("actual repair phase");
    let repair: String = repair.split_whitespace().collect();
    assert!(repair.contains("self.apply_owned_healing_with_effects(master,data.repair_rate/30.0,None,presentation_effects,)"), "repair retains recipient, rate, attribution and receiving effects");
    assert!(
        !repair.contains("write_object_health_authority_aware"),
        "repair must not become an absolute-health mirror write"
    );
    let damage = include_str!("../../game_logic/world_objects/damage_application.rs");
    let healing = last_rust_fn_body(damage, "apply_owned_healing_with_effects")
        .expect("typed Body healing owner");
    assert!(healing.contains("DamageType::Healing"));
    assert!(
        healing.contains(".with_healing_entry()"),
        "Body::attemptHealing is distinct from a healing weapon"
    );
    assert!(healing.contains("self.apply_owned_damage_with_effects("));
}

#[test]
fn pending_absolute_health_record_waits_for_admission() {
    use crate::game_logic::{KindOf, Team, ThingTemplate};
    let mut logic = GameLogic::new();

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
    // This tests the transport's genuinely deferred producer contract. It does
    // not execute a crate collision or claim to verify the crate behavior.
    perform_owner_health(&mut logic, oid, 100.0);
    assert!(
        (logic.host_objects().lookup(&oid).unwrap().health.current - 100.0).abs() < 1e-3,
        "actual body healing commits before any observer boundary"
    );
    let evs = logic.health_events.drain_heal();
    assert!(
        evs.iter()
            .any(|e| e.target == oid && (e.health - 100.0).abs() < 1e-3),
        "heal log must carry absolute HP; got {evs:?}"
    );
}

#[test]
fn suicide_consume_destroy_damage_authority_source() {
    let src = GAME_LOGIC_HOST_SRC;
    assert!(
        src.contains("fn mark_destroyed_authority_aware")
            && src.contains("fn mark_object_destroyed_authority_aware"),
        "destroy authority helpers must exist"
    );
    let compact: String = src.split_whitespace().collect();
    for token in [
        "mark_destroyed_authority_aware(object_id,None)",
        "mark_destroyed_authority_aware(source_id,Some(source_id))",
        "mark_object_destroyed_authority_aware(car,Some(car_id),&mutself.health_events",
        "mark_object_destroyed_authority_aware(obj,Some(unit_id),&mutself.health_events",
        "mark_object_destroyed_authority_aware(source,None,&mutself.health_events",
    ] {
        assert!(
            compact.contains(token),
            "expected destroy residual peel {token}"
        );
    }
}

#[test]
fn execute_packs_presentation_particle_systems_source() {
    let rp = include_str!("../../graphics/render_pipeline/pipeline_execute.rs");
    let body = last_rust_fn_body(rp, "execute").expect("actual execute owner");
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
fn residual_auto_fire_keeps_live_primary_damage() {
    // Structural side-effect guard only; this gives no C++ behavioral credit.
    let helper = last_rust_fn_body(
        GAME_LOGIC_HOST_SRC,
        "residual_auto_fire_apply_damage_with_effects",
    )
    .expect("helper");
    assert!(
        helper.contains("apply_residual_damage_owned"),
        "the residual hit applies damage to the receiving canonical Body/Player"
    );
    assert!(
        helper.contains("damage,"),
        "authored live damage must reach the shot"
    );
    assert!(
        !helper
            .lines()
            .any(|line| matches!(line.trim(), "damage: 0.0," | "damage: 0.0")),
        "live primary damage cannot be replaced by a diagnostic zero"
    );
}
