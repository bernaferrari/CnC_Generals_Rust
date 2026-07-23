//! Host BoneFXDamage + BoneFXUpdate residual (FX on body-damage transitions).
//!
//! C++: `BoneFXDamage::onBodyDamageStateChange` → `BoneFXUpdate::changeBodyDamageState`
//! schedules FXList / ParticleSystem bursts at bones for the new body state.
//!
//! Residual playability slice:
//! - On Pristine→Damaged→ReallyDamaged→Rubble worsening, queue residual FX names
//! - Template peels for common BoneFX users (toxin trucks, scud, chem units)
//! - Presentation drains FX events once per transition
//!
//! Fail-closed: not full bone position matrix / OnlyOnce delay random / client
//! particle system handles / drawable bone lookup.

use crate::game_logic::host_enum_table_residual::HostBodyDamageType;
use serde::{Deserialize, Serialize};

/// One residual FX burst for a body-damage transition.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HostBoneFxEvent {
    pub old_state: HostBodyDamageType,
    pub new_state: HostBodyDamageType,
    pub fx_list: String,
    pub particle_system: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct HostBoneFxDamageData {
    pub transitions: u32,
    pub last_fx: Option<String>,
    pub pending: Vec<HostBoneFxEvent>,
}

impl HostBoneFxDamageData {
    /// C++ changeBodyDamageState residual when body damage worsens.
    pub fn on_body_damage_state_change(
        &mut self,
        template_name: &str,
        old_state: HostBodyDamageType,
        new_state: HostBodyDamageType,
    ) -> Option<HostBoneFxEvent> {
        if !wants_bone_fx(template_name) {
            return None;
        }
        if new_state.ordinal() <= old_state.ordinal() {
            return None; // only on worsening residual
        }
        let fx = fx_list_for_transition(template_name, new_state);
        let ps = particle_for_transition(template_name, new_state);
        let ev = HostBoneFxEvent {
            old_state,
            new_state,
            fx_list: fx.clone(),
            particle_system: ps,
        };
        self.transitions = self.transitions.saturating_add(1);
        self.last_fx = Some(fx);
        self.pending.push(ev.clone());
        Some(ev)
    }

    pub fn drain_pending(&mut self) -> Vec<HostBoneFxEvent> {
        std::mem::take(&mut self.pending)
    }
}

pub fn wants_bone_fx(template_name: &str) -> bool {
    let n = template_name.to_ascii_lowercase();
    n.contains("scud")
        || n.contains("toxin")
        || n.contains("anthrax")
        || n.contains("chem")
        || n.contains("scudstorm")
        || n.contains("nuclearmissile")
        || n.contains("particlecannon")
        || n.contains("scudlauncher")
        || n.contains("bombtruck")
        || n.contains("scudbus")
        || n.contains("battlebus")
}

fn fx_list_for_transition(template_name: &str, state: HostBodyDamageType) -> String {
    let n = template_name.to_ascii_lowercase();
    let base = if n.contains("scud") {
        "FX_Scud"
    } else if n.contains("toxin") || n.contains("anthrax") || n.contains("chem") {
        "FX_Toxin"
    } else if n.contains("nuclear") {
        "FX_Nuke"
    } else {
        "FX_Structure"
    };
    match state {
        HostBodyDamageType::Damaged => format!("{base}DamagedBoneFX"),
        HostBodyDamageType::ReallyDamaged => format!("{base}ReallyDamagedBoneFX"),
        HostBodyDamageType::Rubble => format!("{base}RubbleBoneFX"),
        HostBodyDamageType::Pristine => format!("{base}PristineBoneFX"),
    }
}

fn particle_for_transition(template_name: &str, state: HostBodyDamageType) -> Option<String> {
    let n = template_name.to_ascii_lowercase();
    if matches!(
        state,
        HostBodyDamageType::ReallyDamaged | HostBodyDamageType::Rubble
    ) {
        if n.contains("toxin") || n.contains("anthrax") || n.contains("chem") {
            return Some("ToxinLeakBonePSys".into());
        }
        if n.contains("scud") {
            return Some("ScudSmokeBonePSys".into());
        }
        return Some("StructureDamageBonePSys".into());
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn transition_queues_fx() {
        let mut d = HostBoneFxDamageData::default();
        let ev = d
            .on_body_damage_state_change(
                "GLAVehicleScudLauncher",
                HostBodyDamageType::Pristine,
                HostBodyDamageType::Damaged,
            )
            .expect("fx");
        assert!(ev.fx_list.contains("Damaged"));
        assert_eq!(d.transitions, 1);
        assert_eq!(d.drain_pending().len(), 1);
    }

    #[test]
    fn non_peel_skipped() {
        let mut d = HostBoneFxDamageData::default();
        assert!(d
            .on_body_damage_state_change(
                "AmericaTankCrusader",
                HostBodyDamageType::Pristine,
                HostBodyDamageType::Damaged,
            )
            .is_none());
    }
}
