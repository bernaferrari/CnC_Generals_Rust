//! Additional future-affecting TurretAI fields on the canonical Main owner.
//! CPP TurretAI::xfer319–353; authored data is rebuilt, explicit runtime tuning
//! overrides are saved because the Main setter API makes them observable.
use super::entity_lifecycle_inventory::{decode_payload, encode_payload};
use super::{Object, turret_runtime::HostTurretExtra};
use gamelogic::world::entities::EntityLifecycleCodecError;
use serde::{Deserialize, Serialize};
const MAGIC: &[u8; 4] = b"TURT";
const VERSION: u8 = 1;
#[derive(Debug, Clone, Serialize, Deserialize)]
struct Tuning {
    fire_angle_sweep: [f32; 3],
    sweep_speed_mod: [f32; 3],
    allows_pitch: bool,
    fire_pitch: f32,
    min_pitch: f32,
    ground_unit_pitch: f32,
    pitch_rate: f32,
    min_idle_scan_angle: f32,
    max_idle_scan_angle: f32,
    min_idle_scan_interval: u32,
    max_idle_scan_interval: u32,
    fires_while_turning: bool,
}
impl Tuning {
    fn capture(e: &HostTurretExtra) -> Self {
        Self {
            fire_angle_sweep: e.fire_angle_sweep,
            sweep_speed_mod: e.sweep_speed_mod,
            allows_pitch: e.allows_pitch,
            fire_pitch: e.fire_pitch,
            min_pitch: e.min_pitch,
            ground_unit_pitch: e.ground_unit_pitch,
            pitch_rate: e.pitch_rate,
            min_idle_scan_angle: e.min_idle_scan_angle,
            max_idle_scan_angle: e.max_idle_scan_angle,
            min_idle_scan_interval: e.min_idle_scan_interval,
            max_idle_scan_interval: e.max_idle_scan_interval,
            fires_while_turning: e.fires_while_turning,
        }
    }
    fn apply(self, e: &mut HostTurretExtra) {
        e.fire_angle_sweep = self.fire_angle_sweep;
        e.sweep_speed_mod = self.sweep_speed_mod;
        e.allows_pitch = self.allows_pitch;
        e.fire_pitch = self.fire_pitch;
        e.min_pitch = self.min_pitch;
        e.ground_unit_pitch = self.ground_unit_pitch;
        e.pitch_rate = self.pitch_rate;
        e.min_idle_scan_angle = self.min_idle_scan_angle;
        e.max_idle_scan_angle = self.max_idle_scan_angle;
        e.min_idle_scan_interval = self.min_idle_scan_interval;
        e.max_idle_scan_interval = self.max_idle_scan_interval;
        e.fires_while_turning = self.fires_while_turning;
        e.seeded = true;
        e.definition_overridden = true;
    }
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct Persist {
    runtime: HostTurretExtra,
    tuning: Option<Tuning>,
}
impl Persist {
    pub(crate) fn validate_for_owner(
        &self,
        owner: &Object,
    ) -> Result<(), EntityLifecycleCodecError> {
        validate(self)?;
        if owner.turret_target_id.is_some() && self.runtime.target_pos.is_some() {
            return Err(invalid());
        }
        Ok(())
    }
    pub(crate) fn apply(self, owner: &mut Object) -> Result<(), EntityLifecycleCodecError> {
        self.validate_for_owner(owner)?;
        let mut state = self.runtime;
        // Serde omits immutable data. Restore constructor defaults first,
        // then rebuild from the receiving definitions, without running an update.
        let defaults = &owner.turret_runtime;
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
        state.has_authored_turret = defaults.has_authored_turret;
        state.controlled_weapon_slots = defaults.controlled_weapon_slots;
        state.seeded = false;
        if let Some(tuning) = self.tuning {
            tuning.apply(&mut state);
        }
        owner.turret_runtime = state;
        Ok(())
    }
}
pub(crate) fn encode(owner: &Object) -> Result<Vec<u8>, EntityLifecycleCodecError> {
    let payload = Persist {
        runtime: owner.turret_runtime.clone(),
        tuning: owner
            .turret_runtime
            .definition_overridden
            .then(|| Tuning::capture(&owner.turret_runtime)),
    };
    validate(&payload)?;
    if owner.turret_target_id.is_some() && payload.runtime.target_pos.is_some() {
        return Err(invalid());
    }
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
fn invalid() -> EntityLifecycleCodecError {
    EntityLifecycleCodecError::InvalidOwnerComponent {
        component: "turret runtime",
    }
}
fn validate(p: &Persist) -> Result<(), EntityLifecycleCodecError> {
    let e = &p.runtime;
    if !e.idle_scan_desired_rad.is_finite() || e.target_pos.is_some_and(|p| !p.is_finite()) {
        return Err(invalid());
    }
    if let Some(t) = &p.tuning {
        let values = [
            t.fire_pitch,
            t.min_pitch,
            t.ground_unit_pitch,
            t.pitch_rate,
            t.min_idle_scan_angle,
            t.max_idle_scan_angle,
        ];
        if values
            .into_iter()
            .chain(t.fire_angle_sweep)
            .chain(t.sweep_speed_mod)
            .any(|v| !v.is_finite())
            || t.pitch_rate < 0.0
            || t.min_idle_scan_angle < 0.0
            || t.max_idle_scan_angle < t.min_idle_scan_angle
            || t.max_idle_scan_interval < t.min_idle_scan_interval
            || t.fire_angle_sweep.iter().any(|v| *v < 0.0)
        {
            return Err(invalid());
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game_logic::object::entity_lifecycle_inventory::PathFollowResidual;
    use crate::game_logic::object::entity_lifecycle_tags::{
        TAG_PATH_EXTRA_DISTANCE, TAG_TURRET_RUNTIME,
    };
    use crate::game_logic::{ObjectId, Team, ThingTemplate};
    use gamelogic::world::entities::EntityModuleState;

    #[test]
    fn malformed_turret_runtime_rejects_before_any_owner_field_changes() {
        let mut owner = Object::new(ThingTemplate::new("InvalidTurret"), ObjectId(1), Team::USA);
        owner.turret_enabled = true;
        owner.path_extra_distance = 7.0;
        owner.turret_runtime.enable_sweep_until = 40;
        let mut bad = Persist {
            runtime: owner.turret_runtime.clone(),
            tuning: None,
        };
        bad.runtime.idle_scan_desired_rad = f32::NAN;
        let mut bytes = MAGIC.to_vec();
        bytes.push(VERSION);
        bytes.extend(encode_payload(&bad).unwrap());
        let before = EntityModuleState {
            tag: TAG_PATH_EXTRA_DISTANCE.to_owned(),
            payload: encode_payload(&PathFollowResidual {
                path_extra_distance: 99.0,
                is_exact_path: false,
                is_attack_path: false,
            })
            .unwrap(),
        };
        let malformed = EntityModuleState {
            tag: TAG_TURRET_RUNTIME.to_owned(),
            payload: bytes,
        };
        assert!(
            super::super::entity_lifecycle_apply::apply_module_states(
                &mut owner,
                &[before, malformed]
            )
            .is_err()
        );
        assert_eq!(owner.path_extra_distance, 7.0);
        assert_eq!(owner.turret_runtime.enable_sweep_until, 40);
        let valid = EntityModuleState {
            tag: TAG_TURRET_RUNTIME.to_owned(),
            payload: encode(&owner).unwrap(),
        };
        assert!(
            super::super::entity_lifecycle_apply::apply_module_states(
                &mut owner,
                &[valid.clone(), valid]
            )
            .is_err()
        );
        let mut unknown = encode(&owner).unwrap();
        unknown[4] = 99;
        assert!(decode(&unknown).is_err());
    }
}

#[cfg(test)]
mod mixed_goal_tests {
    use super::*;
    use crate::game_logic::object::entity_lifecycle_tags::TAG_TURRET_RUNTIME;
    use crate::game_logic::object::{
        decode_lifecycle_snapshot_block, encode_lifecycle_snapshot_block,
    };
    use crate::game_logic::{GameLogic, ObjectId, Team, ThingTemplate};

    #[test]
    fn component_mixed_goal_rejects_without_mutating_receiver() {
        let mut owner = Object::new(ThingTemplate::new("MixedTurret"), ObjectId(1), Team::USA);
        owner.turret_target_id = Some(ObjectId(2));
        owner.turret_runtime.enable_sweep_until = 40;
        let mut payload = Persist {
            runtime: owner.turret_runtime.clone(),
            tuning: None,
        };
        payload.runtime.target_pos = Some(glam::Vec3::X);
        assert!(payload.apply(&mut owner).is_err());
        assert_eq!(owner.turret_runtime.enable_sweep_until, 40);
        assert!(owner.turret_runtime.target_pos.is_none());
        assert_eq!(owner.turret_target_id, Some(ObjectId(2)));
    }

    #[test]
    fn normal_encoded_snapshot_rejects_component_position_combined_with_scalar_object_goal() {
        let mut world = GameLogic::new();
        world
            .templates
            .insert("MixedTurret".into(), ThingTemplate::new("MixedTurret"));
        let owner = world
            .create_object("MixedTurret", Team::USA, glam::Vec3::ZERO)
            .unwrap();
        let victim = world
            .create_object("MixedTurret", Team::China, glam::Vec3::X)
            .unwrap();
        world.host_object_mut(owner).unwrap().turret_enabled = true;
        world.set_turret_target_object(owner, Some(victim), false);
        let builder = crate::save_load::snapshot::SnapshotBuilder::new();
        let mut snapshot = builder.create_world_snapshot(&world).unwrap();
        let mut envelopes = decode_lifecycle_snapshot_block(&snapshot.lifecycle_tail).unwrap();
        let prefix_len = encode_lifecycle_snapshot_block(&envelopes).len();
        let component = envelopes
            .iter_mut()
            .find(|e| e.entity_id == owner.0)
            .unwrap()
            .module_states
            .iter_mut()
            .find(|m| m.tag == TAG_TURRET_RUNTIME)
            .unwrap();
        let mut payload = decode(&component.payload).unwrap();
        payload.runtime.target_pos = Some(glam::Vec3::X);
        // Forge only the component. Normal scalar TRAI/OXOB tails still carry
        // the valid original object goal and must be checked after all tails.
        component.payload = MAGIC.to_vec();
        component.payload.push(VERSION);
        component.payload.extend(encode_payload(&payload).unwrap());
        let suffix = snapshot.lifecycle_tail[prefix_len..].to_vec();
        snapshot.lifecycle_tail = encode_lifecycle_snapshot_block(&envelopes);
        snapshot.lifecycle_tail.extend(suffix);
        let bytes = bincode_legacy::serialize(&snapshot).unwrap();
        let decoded = crate::save_load::snapshot::decode_bincode_world_snapshot(&bytes).unwrap();
        let mut candidate = GameLogic::new();
        candidate.templates = world.templates.clone();
        assert!(
            builder
                .restore_from_snapshot(&decoded, &mut candidate)
                .is_err()
        );
        // The running world is an independent owner, not the failed candidate.
        assert_eq!(
            world.host_object(owner).unwrap().turret_target_id,
            Some(victim)
        );
        assert!(
            world
                .host_object(owner)
                .unwrap()
                .turret_runtime
                .target_pos
                .is_none()
        );
    }
}
