//! Behavior suite extracted from `vehicles_and_lasers`.
use super::*;

#[test]
fn production_door_cycle_skips_waiting_to_close() {
    use crate::game_logic::host_enum_table_residual::{
        door_1_closing_model_bit, door_1_opening_model_bit, door_1_waiting_open_model_bit,
        door_1_waiting_to_close_model_bit, host_model_condition_has,
    };
    use crate::game_logic::{KindOf, Team, ThingTemplate};
    let mut logic = GameLogic::new();
    let mut st = ThingTemplate::new("AmericaBarracks");
    st.add_kind_of(KindOf::Structure)
        .add_kind_of(KindOf::FSBarracks)
        .set_health(1000.0);
    logic.templates.insert("AmericaBarracks".into(), st);
    let id = logic
        .create_object("AmericaBarracks", Team::USA, glam::Vec3::new(0.0, 0.0, 0.0))
        .expect("b");
    if let Some(o) = logic.host_object_mut(id) {
        o.start_production_door_cycle(0);
        assert_eq!(o.production_door_phase, 1);
        assert!(host_model_condition_has(
            o.model_condition_bits,
            door_1_opening_model_bit()
        ));
        assert!(!o.tick_production_door(15));
        assert_eq!(o.production_door_phase, 2);
        assert!(host_model_condition_has(
            o.model_condition_bits,
            door_1_waiting_open_model_bit()
        ));
        // WAITING_OPEN → CLOSING (C++ never sets theWaitingToCloseFlags).
        assert!(!o.tick_production_door(45));
        assert_eq!(o.production_door_phase, 4);
        assert!(host_model_condition_has(
            o.model_condition_bits,
            door_1_closing_model_bit()
        ));
        assert!(!host_model_condition_has(
            o.model_condition_bits,
            door_1_waiting_to_close_model_bit()
        ));
        assert!(o.tick_production_door(46));
        assert_eq!(o.production_door_phase, 0);
    }
}

#[test]
fn dozer_bored_mine_clear_assigns_enemy_mine() {
    use crate::game_logic::host_mines::{HostMineData, HostMineKind};
    use crate::game_logic::{KindOf, Team, ThingTemplate};
    let mut logic = GameLogic::new();
    logic
        .players
        .insert(0, Player::new(0, Team::USA, "USA", true));
    let mut dozer_t = ThingTemplate::new("AmericaVehicleDozer");
    dozer_t
        .add_kind_of(KindOf::Vehicle)
        .add_kind_of(KindOf::Worker)
        .add_kind_of(KindOf::Dozer)
        .set_health(200.0);
    // Retail FactionUnit.ini WeaponSet authors MINE_CLEARING_DETAIL
    // (DozerMineDisarmingWeapon); mine-clear authority is authored-only.
    dozer_t.mine_clearing_primary_weapon_name = Some("DozerMineDisarmingWeapon".into());
    logic
        .templates
        .insert("AmericaVehicleDozer".into(), dozer_t);
    let mut mine_t = ThingTemplate::new("GLAStandardMine");
    mine_t.set_health(50.0);
    logic.templates.insert("GLAStandardMine".into(), mine_t);
    let mid = logic
        .create_object(
            "GLAStandardMine",
            Team::GLA,
            glam::Vec3::new(10.0, 0.0, 0.0),
        )
        .expect("mine");
    if let Some(o) = logic.host_object_mut(mid) {
        o.mine_data = Some(HostMineData::new(HostMineKind::LandMine));
    }
    let did = logic
        .create_object(
            "AmericaVehicleDozer",
            Team::USA,
            glam::Vec3::new(0.0, 0.0, 0.0),
        )
        .expect("dozer");
    if let Some(o) = logic.host_object_mut(did) {
        o.set_ai_state(AIState::Idle);
        o.idle_since_frame = 1;
    }
    crate::game_logic::host_ai_decision_log::clear();
    logic.frame = 1 + crate::game_logic::host_repair::DOZER_BORED_TIME_FRAMES;
    logic.update_dozer_bored_repair();
    let d = logic.host_object(did).expect("d");
    assert!(logic.honesty_dozer_bored_mine_clear_ok());
    // Mine-clear engagement is decision-authority last-write (default on):
    // AttackTarget + SetAIState(Attacking) logged; host target/ai_state not mutated.
    if crate::gameworld_shadow::gameworld_ai_decision_authority_live() {
        assert_eq!(d.target, None);
        assert_eq!(d.ai_state, AIState::Idle);
        let events = crate::game_logic::host_ai_decision_log::snapshot();
        let attacking =
            crate::gameworld_shadow::GameWorldShadow::host_ai_state_ordinal(&AIState::Attacking);
        assert!(
            events.iter().any(|e| {
                e.host_object == did
                    && e.kind == crate::game_logic::host_ai_decision_log::AI_DECISION_ATTACK
                    && e.target_host == mid.0
            }),
            "dozer bored mine-clear must log AttackTarget under decision authority"
        );
        assert!(
            events.iter().any(|e| {
                e.host_object == did
                    && e.kind == crate::game_logic::host_ai_decision_log::AI_DECISION_SET_STATE
                    && e.ai_state_ordinal == attacking
            }),
            "dozer bored mine-clear must log SetAIState(Attacking) under decision authority"
        );
    } else {
        assert_eq!(d.target, Some(mid));
        assert_eq!(d.ai_state, AIState::Attacking);
    }
}

#[test]
fn dozer_bored_auto_repair_assigns_damaged_structure() {
    use crate::game_logic::{KindOf, Team, ThingTemplate};
    let mut logic = GameLogic::new();
    logic
        .players
        .insert(0, Player::new(0, Team::USA, "USA", true));
    let mut st = ThingTemplate::new("AmericaPowerPlant");
    st.add_kind_of(KindOf::Structure)
        .add_kind_of(KindOf::FSPower)
        .set_health(1000.0);
    logic.templates.insert("AmericaPowerPlant".into(), st);
    let mut dozer_t = ThingTemplate::new("AmericaVehicleDozer");
    dozer_t
        .add_kind_of(KindOf::Vehicle)
        .add_kind_of(KindOf::Worker)
        .add_kind_of(KindOf::Dozer)
        .set_health(200.0);
    logic
        .templates
        .insert("AmericaVehicleDozer".into(), dozer_t);
    let sid = logic
        .create_object(
            "AmericaPowerPlant",
            Team::USA,
            glam::Vec3::new(0.0, 0.0, 0.0),
        )
        .expect("pp");
    if let Some(o) = logic.host_object_mut(sid) {
        o.set_status_under_construction(false);
        o.construction_percent = 1.0;
        o.health.current = 400.0;
    }
    let did = logic
        .create_object(
            "AmericaVehicleDozer",
            Team::USA,
            glam::Vec3::new(5.0, 0.0, 0.0),
        )
        .expect("dozer");
    if let Some(o) = logic.host_object_mut(did) {
        o.set_ai_state(AIState::Idle);
        o.idle_since_frame = 1;
    }
    // Before bored time: no assign.
    logic.frame = 50;
    logic.update_dozer_bored_repair();
    assert_eq!(logic.host_object(did).unwrap().ai_state, AIState::Idle);
    // After bored time (150f).
    crate::game_logic::host_ai_decision_log::clear();
    logic.frame = 1 + crate::game_logic::host_repair::DOZER_BORED_TIME_FRAMES;
    logic.update_dozer_bored_repair();
    let d = logic.host_object(did).expect("d");
    // Host residual association still stores the repair target.
    assert_eq!(d.target, Some(sid));
    assert!(logic.honesty_dozer_bored_repair_ok());
    // AI state last-write under AI_DECISION_AUTHORITY (default): host ai_state stays
    // Idle; SetAIState(Repairing) is logged for GameWorld writeback.
    if crate::gameworld_shadow::gameworld_ai_decision_authority_live() {
        assert_eq!(d.ai_state, AIState::Idle);
        let events = crate::game_logic::host_ai_decision_log::snapshot();
        let repairing =
            crate::gameworld_shadow::GameWorldShadow::host_ai_state_ordinal(&AIState::Repairing);
        assert!(
            events.iter().any(|e| {
                e.host_object == did
                    && e.kind == crate::game_logic::host_ai_decision_log::AI_DECISION_SET_STATE
                    && e.ai_state_ordinal == repairing
            }),
            "dozer bored repair must log SetAIState(Repairing) under decision authority"
        );
    } else {
        assert_eq!(d.ai_state, AIState::Repairing);
    }
}

#[test]
fn dozer_repair_sole_benefactor_rejects_second_dozer() {
    use crate::game_logic::{KindOf, Team, ThingTemplate};
    let mut logic = GameLogic::new();
    logic
        .players
        .insert(0, Player::new(0, Team::USA, "USA", true));
    let mut st = ThingTemplate::new("AmericaPowerPlant");
    st.add_kind_of(KindOf::Structure)
        .add_kind_of(KindOf::FSPower)
        .set_health(1000.0);
    logic.templates.insert("AmericaPowerPlant".into(), st);
    let mut dozer_t = ThingTemplate::new("AmericaVehicleDozer");
    dozer_t
        .add_kind_of(KindOf::Vehicle)
        .add_kind_of(KindOf::Worker)
        .set_health(200.0);
    logic
        .templates
        .insert("AmericaVehicleDozer".into(), dozer_t);
    let sid = logic
        .create_object(
            "AmericaPowerPlant",
            Team::USA,
            glam::Vec3::new(0.0, 0.0, 0.0),
        )
        .expect("pp");
    if let Some(o) = logic.host_object_mut(sid) {
        o.set_status_under_construction(false);
        o.construction_percent = 1.0;
        o.health.current = 200.0;
    }
    let d1 = logic
        .create_object(
            "AmericaVehicleDozer",
            Team::USA,
            glam::Vec3::new(1.0, 0.0, 0.0),
        )
        .expect("d1");
    let d2 = logic
        .create_object(
            "AmericaVehicleDozer",
            Team::USA,
            glam::Vec3::new(2.0, 0.0, 0.0),
        )
        .expect("d2");
    logic.frame = 10;
    // First dozer claims sole heal.
    let ok1 = {
        let o = logic.host_object_mut(sid).unwrap();
        o.attempt_healing_from_sole_benefactor(5.0, d1, 2, 10)
    };
    assert!(ok1);
    // Second dozer rejected while claim active.
    let ok2 = {
        let o = logic.host_object_mut(sid).unwrap();
        o.attempt_healing_from_sole_benefactor(5.0, d2, 2, 10)
    };
    assert!(!ok2);
    // Same dozer can heal again within claim.
    let ok1b = {
        let o = logic.host_object_mut(sid).unwrap();
        o.attempt_healing_from_sole_benefactor(5.0, d1, 2, 11)
    };
    assert!(ok1b);
    // After expiration (strict now > expiry; claim at 11 → expiry 13 → open at 14).
    let ok2b = {
        let o = logic.host_object_mut(sid).unwrap();
        o.attempt_healing_from_sole_benefactor(5.0, d2, 2, 14)
    };
    assert!(ok2b);
    assert_eq!(
        logic.host_object(sid).unwrap().sole_healing_benefactor,
        Some(d2)
    );
}
