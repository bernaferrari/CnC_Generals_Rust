//! Host TransitionDamageFX residual (FX on body-damage state worsening).
//!
//! C++: `TransitionDamageFX::onBodyDamageStateChange` plays FXList / particles
//! when `IS_CONDITION_WORSE(new, old)` (new ordinal > old).
//! `ActiveBody::attemptDamage` plays template `SoundOnDamaged` /
//! `SoundOnReallyDamaged` (not invented BuildingDamaged / VehicleDamaged)
//! and 25% `VoiceFear` when health crosses `YELLOW_DAMAGE_PERCENT`.
//!
//! Residual playability slice:
//! - Detect Pristine→Damaged→ReallyDamaged→Rubble transitions
//! - Queue named FX/audio residual keys for presentation + audio
//! - Template peels for DamagedFXList / SoundOnDamaged / VoiceFear
//!
//! Fail-closed:
//! - Not full bone-local FX positions / random bone prefix
//! - Not full particle system ID tracking / destroy-on-heal
//! - Not full DamageTypeFlags restriction matrix

use crate::game_logic::host_enum_table_residual::HostBodyDamageType;
use crate::game_logic::ObjectId;
use serde::{Deserialize, Serialize};
use std::cell::{Cell, RefCell};
use std::collections::HashMap;

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
            audio_for_state: [None, None, None, None],
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
            audio_for_state: [None, None, None, None],
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
            audio_for_state: [None, None, None, None],
        }
    }

    pub fn infantry_audio_residual() -> Self {
        Self {
            enabled: true,
            fx_for_state: [None, None, None, None],
            audio_for_state: [None, None, None, None],
        }
    }

    fn overlay_template_audio(&mut self, name: &str) {
        let audio = lookup_template_damage_audio(name);
        if let Some(s) = audio.sound_on_damaged {
            self.audio_for_state[HostBodyDamageType::Damaged.ordinal() as usize] = Some(s);
        }
        if let Some(s) = audio.sound_on_really_damaged {
            self.audio_for_state[HostBodyDamageType::ReallyDamaged.ordinal() as usize] = Some(s);
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
    let mut data = if n.contains("toxic") && n.contains("bunker") {
        HostTransitionDamageFxData::toxic_bunker_residual()
    } else if is_structure {
        HostTransitionDamageFxData::generic_structure_residual()
    } else if is_vehicle
        || n.contains("tank")
        || n.contains("vehicle")
        || n.contains("truck")
        || n.contains("dozer")
    {
        HostTransitionDamageFxData::vehicle_residual()
    } else {
        // Infantry / everything else: still emit template SoundOnDamaged.
        HostTransitionDamageFxData::infantry_audio_residual()
    };
    data.overlay_template_audio(name);
    Some(data)
}

/// C++ `ActiveBody.cpp` `#define YELLOW_DAMAGE_PERCENT (0.25f)`.
pub const YELLOW_DAMAGE_PERCENT: f32 = 0.25;

/// Template `SoundOnDamaged` / `SoundOnReallyDamaged` / `VoiceFear` names.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TemplateDamageAudio {
    pub sound_on_damaged: Option<String>,
    pub sound_on_really_damaged: Option<String>,
    pub voice_fear: Option<String>,
}

thread_local! {
    static TEMPLATE_AUDIO_OVERRIDE: RefCell<HashMap<String, TemplateDamageAudio>> =
        RefCell::new(HashMap::new());
    static DISPATCHED_ARMOR_DAMAGE_FX: RefCell<Vec<String>> = const { RefCell::new(Vec::new()) };
    static PENDING_ATTACKED_BY: RefCell<Vec<(u32, ObjectId)>> = const { RefCell::new(Vec::new()) };
    static ATTACKED_BY_LOG: RefCell<Vec<(i32, i32)>> = const { RefCell::new(Vec::new()) };
    static VOICE_FEAR_ROLL: Cell<Option<i32>> = const { Cell::new(None) };
}

fn nonempty_event_name(event: &game_engine::common::audio::AudioEventRts) -> Option<String> {
    let name = event.get_event_name();
    if name.is_empty() {
        None
    } else {
        Some(name.to_string())
    }
}

pub fn lookup_template_damage_audio(template_name: &str) -> TemplateDamageAudio {
    if let Some(over) = TEMPLATE_AUDIO_OVERRIDE.with(|m| m.borrow().get(template_name).cloned()) {
        return over;
    }
    // Non-blocking Common factory only. TheThingFactory::find_template can
    // rebuild the Object INI database on a miss (seconds) — never on hit FX.
    if let Some(guard) = game_engine::common::thing::thing_factory::try_get_thing_factory() {
        if let Some(factory) = guard.as_ref() {
            if let Some(tmpl) = factory.find_template(template_name, false) {
                return TemplateDamageAudio {
                    sound_on_damaged: tmpl.get_sound_on_damaged().and_then(nonempty_event_name),
                    sound_on_really_damaged: tmpl
                        .get_sound_on_really_damaged()
                        .and_then(nonempty_event_name),
                    voice_fear: tmpl.get_voice_fear().and_then(nonempty_event_name),
                };
            }
        }
    }
    TemplateDamageAudio::default()
}

pub fn set_test_template_audio(template_name: &str, audio: TemplateDamageAudio) {
    TEMPLATE_AUDIO_OVERRIDE.with(|m| {
        m.borrow_mut().insert(template_name.to_string(), audio);
    });
}

pub fn clear_test_template_audio() {
    TEMPLATE_AUDIO_OVERRIDE.with(|m| m.borrow_mut().clear());
}

/// C++ `ActiveBody.cpp:625-631` yellow-health VoiceFear gate + 25% roll.
pub fn voice_fear_should_play(prev_health: f32, current_health: f32, max_health: f32, roll_0_99: i32) -> bool {
    if max_health <= 0.0 || current_health <= 0.0 {
        return false;
    }
    let prev_ratio = prev_health / max_health;
    let cur_ratio = current_health / max_health;
    prev_ratio > YELLOW_DAMAGE_PERCENT
        && cur_ratio < YELLOW_DAMAGE_PERCENT
        && roll_0_99 < 25
}

pub fn set_test_voice_fear_roll(roll: Option<i32>) {
    VOICE_FEAR_ROLL.set(roll);
}

pub fn take_voice_fear_roll() -> i32 {
    VOICE_FEAR_ROLL.get().unwrap_or_else(|| {
        game_engine::common::random_value::get_game_logic_random_value(0, 99)
    })
}

/// Best ArmorSet `DamageFX =` name for live flags (C++ `validateArmorAndDamageFX`).
pub fn find_best_armor_set_damage_fx(
    sets: &[crate::game_logic::HostArmorSet],
    flags: u8,
) -> Option<String> {
    if sets.is_empty() {
        return None;
    }
    let mut best: Option<&crate::game_logic::HostArmorSet> = None;
    let mut best_yes = 0u32;
    let mut best_extra = u32::MAX;
    for set in sets {
        let yes = u32::from(set.conditions & flags).count_ones();
        let extra = u32::from(set.conditions & !flags).count_ones();
        if yes > best_yes || (yes >= best_yes && extra < best_extra) {
            best = Some(set);
            best_yes = yes;
            best_extra = extra;
        }
    }
    best.and_then(|set| set.damage_fx.clone())
        .filter(|n| !n.is_empty() && !n.eq_ignore_ascii_case("none"))
}

fn host_to_ini_damage_type(
    host: crate::game_logic::combat::DamageType,
) -> game_engine::common::ini::ini_damage_fx::DamageType {
    use game_engine::common::ini::ini_damage_fx::DamageType as I;
    match host.to_store() {
        gamelogic::damage::DamageType::Explosion => I::Explosion,
        gamelogic::damage::DamageType::Crush => I::Crush,
        gamelogic::damage::DamageType::ArmorPiercing => I::ArmorPiercing,
        gamelogic::damage::DamageType::SmallArms => I::SmallArms,
        gamelogic::damage::DamageType::Gattling => I::Gattling,
        gamelogic::damage::DamageType::Radiation => I::Radiation,
        gamelogic::damage::DamageType::Flame => I::Flame,
        gamelogic::damage::DamageType::Laser => I::Laser,
        gamelogic::damage::DamageType::Sniper => I::Sniper,
        gamelogic::damage::DamageType::Poison => I::Poison,
        gamelogic::damage::DamageType::Healing => I::Healing,
        gamelogic::damage::DamageType::Unresistable => I::Unresistable,
        gamelogic::damage::DamageType::Water => I::Water,
        gamelogic::damage::DamageType::Deploy => I::Deploy,
        gamelogic::damage::DamageType::Surrender => I::Surrender,
        gamelogic::damage::DamageType::Hack => I::Hack,
        gamelogic::damage::DamageType::KillPilot => I::KillPilot,
        gamelogic::damage::DamageType::Penalty => I::Penalty,
        gamelogic::damage::DamageType::Falling => I::Falling,
        gamelogic::damage::DamageType::Melee => I::Melee,
        gamelogic::damage::DamageType::Disarm => I::Disarm,
        gamelogic::damage::DamageType::HazardCleanup => I::HazardCleanup,
        gamelogic::damage::DamageType::ParticleBeam => I::ParticleBeam,
        gamelogic::damage::DamageType::Toppling => I::Toppling,
        gamelogic::damage::DamageType::InfantryMissile => I::InfantryMissile,
        gamelogic::damage::DamageType::AuroraBomb => I::AuroraBomb,
        gamelogic::damage::DamageType::LandMine => I::LandMine,
        gamelogic::damage::DamageType::JetMissiles => I::JetMissiles,
        gamelogic::damage::DamageType::StealthJetMissiles => I::StealthJetMissiles,
        gamelogic::damage::DamageType::MolotovCocktail => I::MolotovCocktail,
        gamelogic::damage::DamageType::ComancheVulcan => I::ComancheVulcan,
        gamelogic::damage::DamageType::SubdualMissile => I::SubdualMissile,
        gamelogic::damage::DamageType::SubdualVehicle => I::SubdualVehicle,
        gamelogic::damage::DamageType::SubdualBuilding => I::SubdualBuilding,
        gamelogic::damage::DamageType::SubdualUnresistable => I::SubdualUnresistable,
        gamelogic::damage::DamageType::Microwave => I::Microwave,
        gamelogic::damage::DamageType::KillGarrisoned => I::KillGarrisoned,
        gamelogic::damage::DamageType::Status => I::Status,
        gamelogic::damage::DamageType::DamageNumTypes => I::Unresistable,
    }
}

struct HostDamageFxVictim {
    name: String,
    id: u32,
    vet: usize,
}

impl game_engine::common::ini::ini_damage_fx::Object for HostDamageFxVictim {
    fn get_name(&self) -> &str {
        &self.name
    }
    fn get_id(&self) -> u32 {
        self.id
    }
    fn get_veterancy_level(&self) -> usize {
        self.vet
    }
}

/// C++ `ActiveBody::doDamageFX` after `attemptDamage`.
/// Calls Common `DamageFX::do_damage_fx` (`FXList::doFXObj`) and
/// `TheFXList::do_fx_at_position` with the resolved FXList name.
pub fn dispatch_armor_damage_fx(
    obj: &crate::game_logic::Object,
    damage_type: crate::game_logic::combat::DamageType,
    actual_damage: f32,
) -> Option<String> {
    if actual_damage == 0.0 {
        return None;
    }
    let flags = crate::game_logic::host_armor_residual::live_armor_set_flags(obj);
    let Some(dfx_name) = find_best_armor_set_damage_fx(&obj.thing.template.armor_sets, flags) else {
        return None;
    };
    DISPATCHED_ARMOR_DAMAGE_FX.with(|v| v.borrow_mut().push(dfx_name.clone()));
    game_engine::common::ini::ini_damage_fx::init_global_damage_fx_store();
    let ini_dt = host_to_ini_damage_type(damage_type);
    let vet = match obj.experience.level {
        crate::game_logic::VeterancyLevel::Rookie => 0,
        crate::game_logic::VeterancyLevel::Veteran => 1,
        crate::game_logic::VeterancyLevel::Elite => 2,
        crate::game_logic::VeterancyLevel::Heroic => 3,
    };
    let victim = HostDamageFxVictim {
        name: obj.template_name.clone(),
        id: obj.id.0,
        vet,
    };
    let list_name = {
        let store = game_engine::common::ini::ini_damage_fx::get_damage_fx_store()?;
        let dfx = store.find_damage_fx(&dfx_name)?;
        let list = dfx.get_damage_fx_list(ini_dt, actual_damage, Some(&victim));
        dfx.do_damage_fx(ini_dt, actual_damage, None, Some(&victim));
        list
    };
    if let Some(list) = &list_name {
        DISPATCHED_ARMOR_DAMAGE_FX.with(|v| v.borrow_mut().push(list.clone()));
        let pos = obj.get_position();
        let _ = crate::game_logic::dispatch_fx_list_at_pos(list, pos);
        if let Some(fx) = gamelogic::helpers::TheFXList::get() {
            fx.do_fx_at_position(list, &pos);
        }
    }
    list_name.or(Some(dfx_name))
}

pub fn take_dispatched_armor_damage_fx() -> Vec<String> {
    DISPATCHED_ARMOR_DAMAGE_FX.with(|v| std::mem::take(&mut *v.borrow_mut()))
}

/// C++ `ActiveBody.cpp:574-581` — victim `Player::setAttackedBy(srcIndex)`.
pub fn queue_attacked_by(victim_player: Option<u32>, source: Option<ObjectId>) {
    let (Some(victim), Some(src)) = (victim_player, source) else {
        return;
    };
    PENDING_ATTACKED_BY.with(|p| p.borrow_mut().push((victim, src)));
}

pub fn take_pending_attacked_by() -> Vec<(u32, ObjectId)> {
    PENDING_ATTACKED_BY.with(|p| std::mem::take(&mut *p.borrow_mut()))
}

pub fn apply_victim_attacked_by(victim_player: i32, attacker_player: i32) {
    ATTACKED_BY_LOG.with(|l| l.borrow_mut().push((victim_player, attacker_player)));
    let Ok(list) = gamelogic::player::ThePlayerList().read() else {
        return;
    };
    for arc in list.iter() {
        let Ok(player) = arc.read() else {
            continue;
        };
        if player.get_player_index() != victim_player {
            continue;
        }
        drop(player);
        if let Ok(mut player) = arc.write() {
            player.set_attacked_by(attacker_player);
        }
        return;
    }
}

/// Queue C++ VoiceFear when health crosses yellow (ActiveBody.cpp:624-637).
pub fn queue_voice_fear_event(
    pending: &mut Vec<HostTransitionDamageFxEvent>,
    template_name: &str,
    old_state: HostBodyDamageType,
    new_state: HostBodyDamageType,
    prev_health: f32,
    current_health: f32,
    max_health: f32,
) {
    if !voice_fear_should_play(
        prev_health,
        current_health,
        max_health,
        take_voice_fear_roll(),
    ) {
        return;
    }
    let fear = lookup_template_damage_audio(template_name)
        .voice_fear
        .filter(|name| !name.is_empty())
        .unwrap_or_else(|| format!("{template_name}VoiceFear"));
    crate::game_logic::host_voice_fear_log::record(
        ObjectId(0),
        glam::Vec3::ZERO,
        None,
        fear.clone(),
    );
    pending.push(HostTransitionDamageFxEvent {
        old_state: old_state.ordinal(),
        new_state: new_state.ordinal(),
        fx_name: None,
        audio_name: Some(fear),
    });
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
        assert!(e.audio_name.is_none(), "must not invent BuildingDamaged");
    }

    #[test]
    fn template_sound_on_damaged_not_invented_names() {
        // C++ ActiveBody.cpp:605-621 getSoundOnDamaged / getSoundOnReallyDamaged.
        set_test_template_audio(
            "AmericaInfantryRanger",
            TemplateDamageAudio {
                sound_on_damaged: Some("RangerVoiceDamaged".into()),
                sound_on_really_damaged: Some("RangerVoiceReallyDamaged".into()),
                voice_fear: Some("RangerVoiceFear".into()),
            },
        );
        let cfg = transition_damage_fx_config_for_template("AmericaInfantryRanger", false, false)
            .expect("infantry must get a config");
        let e = transition_event(
            &cfg,
            HostBodyDamageType::Pristine,
            HostBodyDamageType::Damaged,
        )
        .expect("template audio");
        assert_eq!(e.audio_name.as_deref(), Some("RangerVoiceDamaged"));
        assert_ne!(e.audio_name.as_deref(), Some("BuildingDamaged"));
        assert_ne!(e.audio_name.as_deref(), Some("VehicleDamaged"));
        clear_test_template_audio();
    }

    #[test]
    fn voice_fear_yellow_25_percent() {
        // C++ ActiveBody.cpp:624-631 YELLOW_DAMAGE_PERCENT + GameLogicRandomValue < 25.
        assert!(voice_fear_should_play(40.0, 20.0, 100.0, 0));
        assert!(voice_fear_should_play(40.0, 20.0, 100.0, 24));
        assert!(!voice_fear_should_play(40.0, 20.0, 100.0, 25));
        assert!(!voice_fear_should_play(20.0, 10.0, 100.0, 0));
        assert!(!voice_fear_should_play(80.0, 40.0, 100.0, 0));
        assert!(!voice_fear_should_play(40.0, 0.0, 100.0, 0));
    }
}
