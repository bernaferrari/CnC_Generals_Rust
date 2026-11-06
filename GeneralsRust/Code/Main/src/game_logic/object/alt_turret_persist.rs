//! Second TurretAI (TURRET_ALT) save residual.
//! CPP AIUpdateInterface::xfer loops `m_turretAI[MAX_TURRETS]` and calls
//! TurretAI::xfer on each present turret, so ALT angle / SM / target / sweep
//! state survives save-load exactly like MAIN. Main stores ALT as the pair
//! `alt_turret_ai` + `alt_turret_runtime`; authored data is rebuilt from the
//! receiving template (serde-skipped), never read from the save.
use super::Object;
use super::entity_lifecycle_inventory::{decode_payload, encode_payload};
use super::turret_ai_state::TurretAiState;
use super::turret_runtime::HostTurretExtra;
use gamelogic::world::entities::EntityLifecycleCodecError;
use serde::{Deserialize, Serialize};

const MAGIC: &[u8; 4] = b"ALTT";
const VERSION: u8 = 1;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct Persist {
    ai: TurretAiState,
    runtime: HostTurretExtra,
}

pub(crate) fn present(owner: &Object) -> bool {
    owner.alt_turret_ai.is_some() && owner.alt_turret_runtime.is_some()
}

fn invalid() -> EntityLifecycleCodecError {
    EntityLifecycleCodecError::InvalidOwnerComponent {
        component: "alt turret",
    }
}

fn validate(p: &Persist) -> Result<(), EntityLifecycleCodecError> {
    let a = &p.ai;
    let e = &p.runtime;
    if !a.turret_angle_deg.is_finite()
        || !a.turret_pitch_deg.is_finite()
        || !a.turret_idle_scan_desired_angle_deg.is_finite()
        || !e.idle_scan_desired_rad.is_finite()
        || e.target_pos.is_some_and(|p| !p.is_finite())
        || (a.turret_target_id.is_some() && e.target_pos.is_some())
    {
        return Err(invalid());
    }
    Ok(())
}

pub(crate) fn encode(owner: &Object) -> Result<Vec<u8>, EntityLifecycleCodecError> {
    let (Some(ai), Some(rt)) = (&owner.alt_turret_ai, &owner.alt_turret_runtime) else {
        return Err(invalid());
    };
    let payload = Persist {
        ai: ai.clone(),
        runtime: rt.clone(),
    };
    validate(&payload)?;
    let mut bytes = MAGIC.to_vec();
    bytes.push(VERSION);
    bytes.extend(encode_payload(&payload)?);
    Ok(bytes)
}

pub(crate) fn decode(bytes: &[u8]) -> Result<Persist, EntityLifecycleCodecError> {
    if bytes.len() < 5 || &bytes[..4] != MAGIC || bytes[4] != VERSION {
        return Err(invalid());
    }
    let payload: Persist = decode_payload(&bytes[5..])?;
    validate(&payload)?;
    Ok(payload)
}

impl Persist {
    /// C++ only xfers TURRET_ALT when the receiving template built one.
    pub(crate) fn validate_for_owner(
        &self,
        owner: &Object,
    ) -> Result<(), EntityLifecycleCodecError> {
        if present(owner) {
            Ok(())
        } else {
            Err(invalid())
        }
    }

    pub(crate) fn apply(self, owner: &mut Object) -> Result<(), EntityLifecycleCodecError> {
        self.validate_for_owner(owner)?;
        let defaults = owner.alt_turret_runtime.as_ref().ok_or_else(invalid)?;
        let mut state = self.runtime;
        state.victim_team = defaults.victim_team;
        state.fire_angle_sweep = defaults.fire_angle_sweep;
        state.sweep_speed_mod = defaults.sweep_speed_mod;
        state.allows_pitch = defaults.allows_pitch;
        state.fire_pitch = defaults.fire_pitch;
        state.min_pitch = defaults.min_pitch;
        state.ground_unit_pitch = defaults.ground_unit_pitch;
        state.pitch_rate = defaults.pitch_rate;
        state.min_idle_scan_angle = defaults.min_idle_scan_angle;
        state.max_idle_scan_angle = defaults.max_idle_scan_angle;
        state.min_idle_scan_interval = defaults.min_idle_scan_interval;
        state.max_idle_scan_interval = defaults.max_idle_scan_interval;
        state.fires_while_turning = defaults.fires_while_turning;
        state.seeded = defaults.seeded;
        state.has_authored_turret = defaults.has_authored_turret;
        state.controlled_weapon_slots = defaults.controlled_weapon_slots;
        state.definition_overridden = defaults.definition_overridden;
        owner.alt_turret_ai = Some(self.ai);
        owner.alt_turret_runtime = Some(state);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::super::turret_ai_state::TurretAiState;
    use super::super::turret_runtime::HostTurretExtra;
    use crate::game_logic::object::Object;
    use crate::game_logic::{ObjectId, Team, ThingTemplate};

    fn alt_owner() -> Object {
        let mut o = Object::new(ThingTemplate::new("AltTurretSave"), ObjectId(1), Team::USA);
        o.turret_ai.turret_enabled = true;
        let mut ai = o.turret_ai.clone();
        ai.turret_angle_deg = 0.0;
        o.alt_turret_ai = Some(ai);
        let mut rt = HostTurretExtra::default();
        rt.has_authored_turret = true;
        rt.controlled_weapon_slots = 0b10;
        o.alt_turret_runtime = Some(rt);
        o
    }

    #[test]
    fn alt_turret_runtime_survives_lifecycle_round_trip_hq_2qedt_2_7() {
        let mut src = alt_owner();
        let ai: &mut TurretAiState = src.alt_turret_ai.as_mut().unwrap();
        ai.turret_angle_deg = 73.0;
        ai.turret_pitch_deg = 12.0;
        ai.turret_holding = true;
        ai.turret_hold_until_frame = 900;
        let rt = src.alt_turret_runtime.as_mut().unwrap();
        rt.enable_sweep_until = 321;
        rt.target_pos = Some(glam::Vec3::new(10.0, 20.0, 0.0));
        rt.did_fire = true;
        let modules =
            super::super::entity_lifecycle_inventory::collect_module_states(&src, None).unwrap();
        let mut dst = alt_owner();
        super::super::entity_lifecycle_apply::apply_module_states(&mut dst, &modules).unwrap();
        let ai = dst.alt_turret_ai.as_ref().unwrap();
        assert_eq!(ai.turret_angle_deg, 73.0);
        assert_eq!(ai.turret_pitch_deg, 12.0);
        assert!(ai.turret_holding);
        assert_eq!(ai.turret_hold_until_frame, 900);
        let rt = dst.alt_turret_runtime.as_ref().unwrap();
        assert_eq!(rt.enable_sweep_until, 321);
        assert_eq!(rt.target_pos, Some(glam::Vec3::new(10.0, 20.0, 0.0)));
        assert!(rt.did_fire);
        // Authored config comes from the receiver, not the save.
        assert!(rt.has_authored_turret);
        assert_eq!(rt.controlled_weapon_slots, 0b10);
        // Primary untouched by ALT restore.
        assert_eq!(
            dst.turret_ai.turret_angle_deg,
            src.turret_ai.turret_angle_deg
        );
    }

    #[test]
    fn alt_turret_payload_rejects_receiver_without_alt_hq_2qedt_2_7() {
        let src = alt_owner();
        let bytes = super::encode(&src).unwrap();
        let mut plain = Object::new(ThingTemplate::new("NoAlt"), ObjectId(2), Team::USA);
        assert!(super::decode(&bytes).unwrap().apply(&mut plain).is_err());
        assert!(plain.alt_turret_ai.is_none());
    }
}
