//! hq-2qedt.2.3.3: TurretAI SM ticks drive both m_turretAI[TURRET_MAIN] and
//! m_turretAI[TURRET_ALT] (AIUpdate.cpp AIUpdateInterface::update loops
//! `for (int i = 0; i < MAX_TURRETS; ++i) if (m_turretAI[i]) m_turretAI[i]->updateTurretAI();`).
use super::super::*;
use crate::game_logic::object::turret_ai_state::TurretAiState;
use crate::game_logic::object::turret_runtime::HostTurretExtra;
use crate::game_logic::object::TurretSubState;
use crate::game_logic::ThingTemplate;

fn recentering(angle: f32) -> TurretAiState {
    let mut s = TurretAiState::default();
    s.turret_enabled = true;
    s.turret_substate = TurretSubState::Recenter;
    s.turret_angle_deg = angle;
    s.turret_natural_angle_deg = 0.0;
    s.turret_turn_rate_rad = 1.0;
    s
}

#[test]
fn alt_turret_state_machine_ticks_alongside_primary() {
    let mut logic = GameLogic::new();
    let id = ObjectId(9731);
    let mut unit = Object::new(ThingTemplate::new("AltTank"), id, Team::USA);
    unit.turret_ai = recentering(40.0);
    unit.alt_turret_ai = Some(recentering(-60.0));
    unit.alt_turret_runtime = Some(HostTurretExtra::default());
    logic.objects.insert(id, unit);
    logic.tick_all_turret_state_machines(&[id], 0.0, 1);
    let obj = logic.objects.get(&id).expect("tank");
    assert!(obj.turret_ai.turret_angle_deg < 40.0, "primary recenters");
    let alt = obj.alt_turret_ai.as_ref().expect("alt slot kept");
    assert!(
        alt.turret_angle_deg > -60.0,
        "alt TurretAI must update independently, angle={}",
        alt.turret_angle_deg
    );
    // Slots stay in place (no swap leak).
    assert!(obj.turret_ai.turret_angle_deg > 0.0 && alt.turret_angle_deg < 0.0);
}

#[test]
fn primary_only_turret_unchanged_by_alt_ticking() {
    let mut logic = GameLogic::new();
    let id = ObjectId(9732);
    let mut unit = Object::new(ThingTemplate::new("Tank"), id, Team::USA);
    unit.turret_ai = recentering(40.0);
    logic.objects.insert(id, unit);
    logic.tick_all_turret_state_machines(&[id], 0.0, 1);
    let obj = logic.objects.get(&id).expect("tank");
    assert!(obj.turret_ai.turret_angle_deg < 40.0);
    assert!(obj.alt_turret_ai.is_none() && obj.alt_turret_runtime.is_none());
}

/// hq-2qedt.2.3.4: setTurretTarget* lands on the TurretAI that controls the
/// active weapon slot (AIStates getWhichTurretForCurWeapon).
fn alt_tank(logic: &mut GameLogic, id: ObjectId, active_slot: u8) {
    let mut unit = Object::new(ThingTemplate::new("AltTank"), id, Team::USA);
    let mut main = TurretAiState::default();
    main.turret_enabled = true;
    main.turret_substate = TurretSubState::Idle;
    unit.turret_ai = main.clone();
    unit.turret_runtime.has_authored_turret = true;
    unit.turret_runtime.controlled_weapon_slots = 1 << 0;
    unit.alt_turret_ai = Some(main);
    let mut alt_rt = HostTurretExtra::default();
    alt_rt.has_authored_turret = true;
    alt_rt.controlled_weapon_slots = 1 << 1;
    unit.alt_turret_runtime = Some(alt_rt);
    unit.active_weapon_slot = active_slot;
    logic.objects.insert(id, unit);
}

#[test]
fn set_turret_target_position_routes_to_alt_for_alt_slot() {
    let mut logic = GameLogic::new();
    let id = ObjectId(9733);
    alt_tank(&mut logic, id, 1);
    let p = glam::Vec3::new(10.0, 20.0, 0.0);
    logic.set_turret_target_position(id, Some(p));
    let obj = logic.objects.get(&id).unwrap();
    assert_eq!(obj.alt_turret_runtime.as_ref().unwrap().target_pos, Some(p));
    assert_eq!(obj.alt_turret_ai.as_ref().unwrap().turret_substate, TurretSubState::Aim);
    assert_eq!(obj.turret_runtime.target_pos, None, "primary untouched");
    assert_ne!(obj.turret_ai.turret_substate, TurretSubState::Aim);
}

#[test]
fn set_turret_target_object_routes_by_active_slot() {
    let mut logic = GameLogic::new();
    let id = ObjectId(9734);
    let victim = ObjectId(9735);
    logic.objects.insert(victim, Object::new(ThingTemplate::new("Target"), victim, Team::China));
    alt_tank(&mut logic, id, 1);
    logic.set_turret_target_object(id, Some(victim), false);
    {
        let obj = logic.objects.get(&id).unwrap();
        assert_eq!(obj.alt_turret_ai.as_ref().unwrap().turret_target_id, Some(victim));
        assert_eq!(obj.turret_ai.turret_target_id, None, "primary untouched");
    }
    // Primary slot still drives TURRET_MAIN.
    logic.objects.get_mut(&id).unwrap().active_weapon_slot = 0;
    logic.set_turret_target_object(id, Some(victim), false);
    assert_eq!(logic.objects.get(&id).unwrap().turret_ai.turret_target_id, Some(victim));
}
