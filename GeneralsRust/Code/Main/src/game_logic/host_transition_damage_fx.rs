//! Host TransitionDamageFX residual (FX on body-damage state worsening).
//!
//! C++: `TransitionDamageFX::onBodyDamageStateChange` plays FXList / particles
//! when `IS_CONDITION_WORSE(new, old)` (new ordinal > old).
//!
//! Residual playability slice:
//! - Detect Pristine→Damaged→ReallyDamaged→Rubble transitions
//! - Queue named FX/audio residual keys for presentation + audio
//! - Template peels for common DamagedFXList / ReallyDamagedFXList names
//!
//! Fail-closed:
//! - Not full bone-local FX positions / random bone prefix
//! - Not full particle system ID tracking / destroy-on-heal
//! - Not full DamageTypeFlags restriction matrix

use crate::game_logic::host_enum_table_residual::HostBodyDamageType;
use serde::{Deserialize, Serialize};

/// C++ DAMAGE_MODULE_MAX_FX residual (we store one primary name per state).
pub const TRANSITION_DAMAGE_FX_SLOTS: usize = 4;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct HostTransitionDamageFxData {
    /// FX name residual keyed by body state ordinal (0..3).
    pub fx_for_state: [Option<String>; TRANSITION_DAMAGE_FX_SLOTS],
    /// Audio residual keyed by body state ordinal.
    pub audio_for_state: [Option<String>; TRANSITION_DAMAGE_FX_SLOTS],
    pub enabled: bool,
}

impl HostTransitionDamageFxData {
    pub fn generic_structure_residual() -> Self {
        Self {
            enabled: true,
            fx_for_state: [
                None,
                Some("FX_StructureDamagedTransition".into()),
                Some("FX_StructureReallyDamagedTransition".into()),
                Some("FX_StructureRubbleTransition".into()),
            ],
            audio_for_state: [
                None,
                Some("BuildingDamaged".into()),
                Some("BuildingReallyDamaged".into()),
                Some("BuildingCollapse".into()),
            ],
        }
    }

    pub fn toxic_bunker_residual() -> Self {
        Self {
            enabled: true,
            fx_for_state: [
                None,
                Some("FX_ToxicBunkerDamageTransition".into()),
                Some("FX_ToxicBunkerDamageTransition".into()),
                Some("FX_ToxicBunkerRubble".into()),
            ],
            audio_for_state: [
                None,
                Some("BuildingDamaged".into()),
                Some("BuildingReallyDamaged".into()),
                Some("BuildingCollapse".into()),
            ],
        }
    }

    pub fn vehicle_residual() -> Self {
        Self {
            enabled: true,
            fx_for_state: [
                None,
                Some("FX_VehicleDamagedTransition".into()),
                Some("FX_VehicleReallyDamagedTransition".into()),
                Some("FX_VehicleRubbleTransition".into()),
            ],
            audio_for_state: [
                None,
                Some("VehicleDamaged".into()),
                Some("VehicleReallyDamaged".into()),
                Some("VehicleDestroyed".into()),
            ],
        }
    }
}

/// One transition residual event (presentation/audio consumers).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HostTransitionDamageFxEvent {
    pub old_state: u8,
    pub new_state: u8,
    pub fx_name: Option<String>,
    pub audio_name: Option<String>,
}

/// C++ IS_CONDITION_WORSE(a,b) := a > b (BodyDamageType ordinal).
pub fn is_condition_worse(new_state: HostBodyDamageType, old_state: HostBodyDamageType) -> bool {
    new_state.ordinal() > old_state.ordinal()
}

/// Build residual event when state worsens.
pub fn transition_event(
    data: &HostTransitionDamageFxData,
    old_state: HostBodyDamageType,
    new_state: HostBodyDamageType,
) -> Option<HostTransitionDamageFxEvent> {
    if !data.enabled || !is_condition_worse(new_state, old_state) {
        return None;
    }
    let idx = new_state.ordinal() as usize;
    if idx >= TRANSITION_DAMAGE_FX_SLOTS {
        return None;
    }
    let fx = data.fx_for_state[idx].clone();
    let audio = data.audio_for_state[idx].clone();
    if fx.is_none() && audio.is_none() {
        return None;
    }
    Some(HostTransitionDamageFxEvent {
        old_state: old_state.ordinal(),
        new_state: new_state.ordinal(),
        fx_name: fx,
        audio_name: audio,
    })
}

pub fn transition_damage_fx_config_for_template(
    name: &str,
    is_structure: bool,
    is_vehicle: bool,
) -> Option<HostTransitionDamageFxData> {
    let n = name.to_ascii_lowercase();
    if n.contains("toxic") && n.contains("bunker") {
        return Some(HostTransitionDamageFxData::toxic_bunker_residual());
    }
    if is_structure {
        return Some(HostTransitionDamageFxData::generic_structure_residual());
    }
    if is_vehicle
        || n.contains("tank")
        || n.contains("vehicle")
        || n.contains("truck")
        || n.contains("dozer")
    {
        return Some(HostTransitionDamageFxData::vehicle_residual());
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game_logic::host_enum_table_residual::HostBodyDamageType;

    #[test]
    fn worse_transition_emits_fx() {
        let d = HostTransitionDamageFxData::generic_structure_residual();
        assert!(transition_event(
            &d,
            HostBodyDamageType::Damaged,
            HostBodyDamageType::Pristine
        )
        .is_none());
        let e = transition_event(
            &d,
            HostBodyDamageType::Pristine,
            HostBodyDamageType::Damaged,
        )
        .expect("worse");
        assert_eq!(e.new_state, 1);
        assert!(e.fx_name.unwrap().contains("Damaged"));
    }
}
