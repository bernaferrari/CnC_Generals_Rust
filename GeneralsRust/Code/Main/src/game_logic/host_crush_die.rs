//! Host CrushDie residual (Total/Front/Back crush sounds).
//!
//! C++: `CrushDie::onDie` — DieMux + DAMAGE_CRUSH only. `crushLocationCheck`
//! picks TOTAL/FRONT/BACK. If the authored sound is non-empty and
//! `GameLogicRandomValue(0,99) < crushSoundPercent` (default 100), queue
//! `TheAudio->addAudioEvent` with the victim object id.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HostCrushKind {
    Total,
    Front,
    Back,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HostCrushDieData {
    pub total_sound: Option<String>,
    pub front_sound: Option<String>,
    pub back_sound: Option<String>,
    pub total_percent: i32,
    pub front_percent: i32,
    pub back_percent: i32,
    pub fired: bool,
}

impl Default for HostCrushDieData {
    fn default() -> Self {
        Self {
            total_sound: None,
            front_sound: None,
            back_sound: None,
            total_percent: 100,
            front_percent: 100,
            back_percent: 100,
            fired: false,
        }
    }
}

impl HostCrushDieData {
    /// C++ `GameLogicRandomValue(0, 99) < crushSoundPercent` — 0 never, 100 always.
    pub fn pick_sound(&self, kind: HostCrushKind, seed: u32) -> Option<String> {
        let (name, percent) = match kind {
            HostCrushKind::Total => (self.total_sound.as_deref(), self.total_percent),
            HostCrushKind::Front => (self.front_sound.as_deref(), self.front_percent),
            HostCrushKind::Back => (self.back_sound.as_deref(), self.back_percent),
        };
        let name = name.filter(|s| !s.is_empty())?;
        if percent <= 0 {
            return None;
        }
        if (seed % 100) as i32 >= percent {
            return None;
        }
        Some(name.to_string())
    }

    pub fn on_die(&mut self, kind: HostCrushKind, seed: u32) -> Option<String> {
        if self.fired {
            return None;
        }
        self.fired = true;
        self.pick_sound(kind, seed)
    }
}

pub fn crush_kind_from_flags(front_crushed: bool, back_crushed: bool) -> HostCrushKind {
    if front_crushed && back_crushed {
        HostCrushKind::Total
    } else if front_crushed {
        HostCrushKind::Front
    } else {
        HostCrushKind::Back
    }
}

fn parse_percent(raw: &str) -> Option<i32> {
    raw.trim().parse::<i32>().ok()
}

fn sound_or_none(raw: &str) -> Option<String> {
    let t = raw.trim();
    if t.is_empty() || t.eq_ignore_ascii_case("None") {
        None
    } else {
        Some(t.to_string())
    }
}

pub fn crush_die_from_behavior_attrs(attrs: &[(&str, &str)]) -> HostCrushDieData {
    let get = |key: &str| {
        attrs
            .iter()
            .find(|(n, _)| n.eq_ignore_ascii_case(key))
            .map(|(_, v)| *v)
    };
    HostCrushDieData {
        total_sound: get("TotalCrushSound").and_then(sound_or_none),
        front_sound: get("FrontEndCrushSound").and_then(sound_or_none),
        back_sound: get("BackEndCrushSound").and_then(sound_or_none),
        total_percent: get("TotalCrushSoundPercent")
            .and_then(parse_percent)
            .unwrap_or(100),
        front_percent: get("FrontEndCrushSoundPercent")
            .and_then(parse_percent)
            .unwrap_or(100),
        back_percent: get("BackEndCrushSoundPercent")
            .and_then(parse_percent)
            .unwrap_or(100),
        fired: false,
    }
}

fn authored_crush_die(name: &str) -> Option<HostCrushDieData> {
    let manager = crate::assets::get_asset_manager()?;
    let manager = manager.lock().ok()?;
    let definition = manager.get_object_definition(name)?;
    for module in &definition.behavior_modules {
        if !module.class_name.eq_ignore_ascii_case("CrushDie") {
            continue;
        }
        let attrs: Vec<(&str, &str)> = module
            .attributes
            .iter()
            .map(|(k, v)| (k.as_str(), v.as_str()))
            .collect();
        let data = crush_die_from_behavior_attrs(&attrs);
        if data.total_sound.is_some() || data.front_sound.is_some() || data.back_sound.is_some() {
            return Some(data);
        }
    }
    None
}

/// Retail CrushDie peel when Object INI is not loaded (infantry / crushable vehicles).
pub fn crush_die_config_for_template(name: &str) -> Option<HostCrushDieData> {
    if let Some(authored) = authored_crush_die(name) {
        return Some(authored);
    }
    let n = name.to_ascii_lowercase();
    if n.contains("infantry")
        || n.contains("ranger")
        || n.contains("rebel")
        || n.contains("redguard")
        || n.contains("tankhunter")
        || n.contains("rpg")
        || n.contains("terrorist")
        || n.contains("worker")
        || n.contains("dozer")
        || n.contains("pilot")
        || n.contains("jarmen")
        || n.contains("colonel")
        || n.contains("blacklotus")
        || n.contains("pathfinder")
        || n.contains("missiledefender")
        || n.contains("minigunner")
        || n.contains("hacker")
    {
        return Some(HostCrushDieData {
            total_sound: Some("InfantryCrush".into()),
            front_sound: Some("InfantryCrush".into()),
            back_sound: Some("InfantryCrush".into()),
            ..Default::default()
        });
    }
    if n.contains("humvee")
        || n.contains("technical")
        || n.contains("buggy")
        || n.contains("truck")
        || n.contains("car")
        || n.contains("van")
        || n.contains("pickup")
    {
        return Some(HostCrushDieData {
            total_sound: Some("VehicleCrush".into()),
            front_sound: Some("VehicleCrush".into()),
            back_sound: Some("VehicleCrush".into()),
            ..Default::default()
        });
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn percent_100_always_plays() {
        let d = HostCrushDieData {
            total_sound: Some("InfantryCrush".into()),
            ..Default::default()
        };
        assert_eq!(
            d.pick_sound(HostCrushKind::Total, 99).as_deref(),
            Some("InfantryCrush")
        );
    }

    #[test]
    fn percent_0_never_plays() {
        let d = HostCrushDieData {
            total_sound: Some("InfantryCrush".into()),
            total_percent: 0,
            ..Default::default()
        };
        assert!(d.pick_sound(HostCrushKind::Total, 0).is_none());
    }

    #[test]
    fn kind_from_flags() {
        assert_eq!(crush_kind_from_flags(true, true), HostCrushKind::Total);
        assert_eq!(crush_kind_from_flags(true, false), HostCrushKind::Front);
        assert_eq!(crush_kind_from_flags(false, true), HostCrushKind::Back);
    }

    #[test]
    fn infantry_peel() {
        let d = crush_die_config_for_template("AmericaRanger").unwrap();
        assert_eq!(d.total_sound.as_deref(), Some("InfantryCrush"));
    }

    #[test]
    fn on_die_fires_once() {
        let mut d = HostCrushDieData {
            total_sound: Some("Crush".into()),
            ..Default::default()
        };
        assert!(d.on_die(HostCrushKind::Total, 0).is_some());
        assert!(d.on_die(HostCrushKind::Total, 0).is_none());
    }
}
