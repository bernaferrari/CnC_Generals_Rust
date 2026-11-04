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
