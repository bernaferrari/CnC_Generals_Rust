//! Salvage crate behaviour - awards weapon/armor upgrades, veterancy, or money.

use super::{format_add_cash, format_cash_template};
use crate::common::ModelConditionFlags;
use crate::common::*;
use crate::experience::ExperienceTracker;
use crate::helpers::{TheAudio, TheGameLogic, TheInGameUI};
use crate::object::collide::crate_collide::crate_collide::{
    CrateCollide as BaseCrateCollide, CrateCollideBehavior, CrateCollideModuleData,
};
use crate::object::collide::{
    CollideModule, CollisionError, Coord3D as CollisionCoord3D, GameObject,
};
use crate::object::{ArmorSetFlag, Object};
use crate::player::Player;
use crate::weapon::WeaponSetType;
use crate::{GameLogicRandomValue, GameLogicRandomValueReal};
use game_engine::common::ini::{FieldParse as IniFieldParse, INIError, INI};
use std::sync::{Arc, Mutex, RwLock};

fn resolve_crate_object(id: ObjectID) -> Option<Arc<RwLock<Object>>> {
    if id == crate::common::INVALID_ID {
        return None;
    }
    TheGameLogic::find_object_by_id(id)
        .or_else(|| crate::object::registry::OBJECT_REGISTRY.get_object(id))
}

/// INI configuration for salvage crates.
#[derive(Debug, Clone)]
pub struct SalvageCrateCollideModuleData {
    /// Base crate metadata (required/forbidden kinds etc.).
    pub base: CrateCollideModuleData,
    /// Chance to award a weapon upgrade if the object qualifies.
    pub weapon_chance: f32,
    /// Chance to award a veterancy level if weapon bonus fails.
    pub level_chance: f32,
    /// Chance to award money if weapon and level both fail.
    pub money_chance: f32,
    /// Minimum cash payout.
    pub minimum_money: i32,
    /// Maximum cash payout.
    pub maximum_money: i32,
}

impl Default for SalvageCrateCollideModuleData {
    fn default() -> Self {
        Self {
            base: CrateCollideModuleData::new(),
            weapon_chance: 1.0,
            level_chance: 0.25,
            money_chance: 0.75,
            minimum_money: 25,
            maximum_money: 75,
        }
    }
}

impl SalvageCrateCollideModuleData {
    pub fn parse_from_ini(&mut self, ini: &mut INI) -> Result<(), INIError> {
        ini.init_from_ini_with_fields(self, SALVAGE_CRATE_COLLIDE_FIELDS)
    }

    pub fn build_field_parse() -> Vec<FieldParse> {
        let mut fields = CrateCollideModuleData::build_field_parse();
        fields.extend([
            FieldParse::new("WeaponChance", FieldType::PercentToReal, "weapon_chance"),
            FieldParse::new("LevelChance", FieldType::PercentToReal, "level_chance"),
            FieldParse::new("MoneyChance", FieldType::PercentToReal, "money_chance"),
            FieldParse::new("MinMoney", FieldType::Int, "minimum_money"),
            FieldParse::new("MaxMoney", FieldType::Int, "maximum_money"),
        ]);
        fields
    }
}

fn parse_kind_of_mask(tokens: &[&str]) -> Result<u64, INIError> {
    if tokens.is_empty() {
        return Err(INIError::InvalidData);
    }

    let mut mask = 0u64;
    for token in tokens
        .iter()
        .filter(|token| **token != "=")
        .flat_map(|token| token.split('|'))
    {
        let token = token.trim();
        if token.is_empty() {
            continue;
        }
        let Some(kind) = kindof_from_name(token) else {
            return Err(INIError::InvalidData);
        };
        mask |= 1u64 << (kind as u32);
    }
    Ok(mask)
}

fn first_token<'a>(tokens: &'a [&'a str]) -> Result<&'a str, INIError> {
    tokens
        .iter()
        .copied()
        .find(|token| *token != "=")
        .ok_or(INIError::InvalidData)
}

fn parse_required_kind_of(
    _ini: &mut INI,
    data: &mut SalvageCrateCollideModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    data.base.required_kind_of = parse_kind_of_mask(tokens)?;
    Ok(())
}

fn parse_forbidden_kind_of(
    _ini: &mut INI,
    data: &mut SalvageCrateCollideModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    data.base.forbidden_kind_of = parse_kind_of_mask(tokens)?;
    Ok(())
}

fn parse_forbid_owner_player(
    _ini: &mut INI,
    data: &mut SalvageCrateCollideModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    data.base.is_forbid_owner_player = INI::parse_bool(first_token(tokens)?)?;
    Ok(())
}

fn parse_building_pickup(
    _ini: &mut INI,
    data: &mut SalvageCrateCollideModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    data.base.is_building_pickup = INI::parse_bool(first_token(tokens)?)?;
    Ok(())
}

fn parse_human_only(
    _ini: &mut INI,
    data: &mut SalvageCrateCollideModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    data.base.is_human_only_pickup = INI::parse_bool(first_token(tokens)?)?;
    Ok(())
}

fn parse_pickup_science(
    _ini: &mut INI,
    data: &mut SalvageCrateCollideModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    super::parse_crate_pickup_science(&mut data.base, first_token(tokens)?)
}

fn parse_execute_fx(
    _ini: &mut INI,
    data: &mut SalvageCrateCollideModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    data.base.execute_fx = Some(first_token(tokens)?.to_string());
    Ok(())
}

fn parse_execute_animation(
    _ini: &mut INI,
    data: &mut SalvageCrateCollideModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    data.base.execution_animation_template = first_token(tokens)?.to_string();
    Ok(())
}

fn parse_execute_animation_time(
    _ini: &mut INI,
    data: &mut SalvageCrateCollideModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    data.base.execute_animation_display_time_seconds = INI::parse_real(first_token(tokens)?)?;
    Ok(())
}

fn parse_execute_animation_z_rise(
    _ini: &mut INI,
    data: &mut SalvageCrateCollideModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    data.base.execute_animation_z_rise_per_second = INI::parse_real(first_token(tokens)?)?;
    Ok(())
}

fn parse_execute_animation_fades(
    _ini: &mut INI,
    data: &mut SalvageCrateCollideModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    data.base.execute_animation_fades = INI::parse_bool(first_token(tokens)?)?;
    Ok(())
}

fn parse_weapon_chance(
    _ini: &mut INI,
    data: &mut SalvageCrateCollideModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    data.weapon_chance = INI::parse_percent_to_real(first_token(tokens)?)?;
    Ok(())
}

fn parse_level_chance(
    _ini: &mut INI,
    data: &mut SalvageCrateCollideModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    data.level_chance = INI::parse_percent_to_real(first_token(tokens)?)?;
    Ok(())
}

fn parse_money_chance(
    _ini: &mut INI,
    data: &mut SalvageCrateCollideModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    data.money_chance = INI::parse_percent_to_real(first_token(tokens)?)?;
    Ok(())
}

fn parse_min_money(
    _ini: &mut INI,
    data: &mut SalvageCrateCollideModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    data.minimum_money = INI::parse_int(first_token(tokens)?)?;
    Ok(())
}

fn parse_max_money(
    _ini: &mut INI,
    data: &mut SalvageCrateCollideModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    data.maximum_money = INI::parse_int(first_token(tokens)?)?;
    Ok(())
}

const SALVAGE_CRATE_COLLIDE_FIELDS: &[IniFieldParse<SalvageCrateCollideModuleData>] = &[
    IniFieldParse {
        token: "RequiredKindOf",
        parse: parse_required_kind_of,
    },
    IniFieldParse {
        token: "ForbiddenKindOf",
        parse: parse_forbidden_kind_of,
    },
    IniFieldParse {
        token: "ForbidOwnerPlayer",
        parse: parse_forbid_owner_player,
    },
    IniFieldParse {
        token: "BuildingPickup",
        parse: parse_building_pickup,
    },
    IniFieldParse {
        token: "HumanOnly",
        parse: parse_human_only,
    },
    IniFieldParse {
        token: "PickupScience",
        parse: parse_pickup_science,
    },
    IniFieldParse {
        token: "ExecuteFX",
        parse: parse_execute_fx,
    },
    IniFieldParse {
        token: "ExecuteAnimation",
        parse: parse_execute_animation,
    },
    IniFieldParse {
        token: "ExecuteAnimationTime",
        parse: parse_execute_animation_time,
    },
    IniFieldParse {
        token: "ExecuteAnimationZRise",
        parse: parse_execute_animation_z_rise,
    },
    IniFieldParse {
        token: "ExecuteAnimationFades",
        parse: parse_execute_animation_fades,
    },
    IniFieldParse {
        token: "WeaponChance",
        parse: parse_weapon_chance,
    },
    IniFieldParse {
        token: "LevelChance",
        parse: parse_level_chance,
    },
    IniFieldParse {
        token: "MoneyChance",
        parse: parse_money_chance,
    },
    IniFieldParse {
        token: "MinMoney",
        parse: parse_min_money,
    },
    IniFieldParse {
        token: "MaxMoney",
        parse: parse_max_money,
    },
];

/// Concrete implementation of the Salvage crate behaviour.
pub struct SalvageCrateCollide {
    base: BaseCrateCollide,
    module_data: SalvageCrateCollideModuleData,
}

impl SalvageCrateCollide {
    pub fn new(object_id: ObjectId, module_data: SalvageCrateCollideModuleData) -> Self {
        Self {
            base: BaseCrateCollide::new(object_id, module_data.base.clone()),
            module_data,
        }
    }

    fn require_object_handle(
        &self,
        other: &dyn GameObject,
    ) -> Result<Arc<RwLock<Object>>, CollisionError> {
        other.as_object_handle().ok_or_else(|| {
            CollisionError::InvalidObject(
                "Salvage crate requires a concrete Object handle".to_string(),
            )
        })
    }

    fn determine_salvage_type(&self, other_id: ObjectID) -> Result<SalvageType, CollisionError> {
        let Some(other) = resolve_crate_object(other_id) else {
            return Err(CollisionError::InvalidObject(
                "collector unavailable".into(),
            ));
        };

        if self.eligible_for_armor_set(other_id)? {
            return Ok(SalvageType::ArmorSet);
        }

        if self.eligible_for_weapon_set(other_id)? && self.test_weapon_chance() {
            return Ok(SalvageType::WeaponSet);
        }

        if self.eligible_for_level(other_id)? && self.test_level_chance() {
            return Ok(SalvageType::Level);
        }

        Ok(SalvageType::Money)
    }

    fn eligible_for_weapon_set(&self, other_id: ObjectID) -> Result<bool, CollisionError> {
        let Some(other) = resolve_crate_object(other_id) else {
            return Ok(false);
        };

        let guard = other
            .read()
            .map_err(|_| CollisionError::InvalidObject("object lock poisoned".into()))?;

        if !guard.is_kind_of(KindOf::WeaponSalvager) {
            return Ok(false);
        }

        Ok(!guard.test_weapon_set_flag(WeaponSetType::CrateUpgradeTwo))
    }

    fn eligible_for_armor_set(&self, other_id: ObjectID) -> Result<bool, CollisionError> {
        let Some(other) = resolve_crate_object(other_id) else {
            return Ok(false);
        };

        let guard = other
            .read()
            .map_err(|_| CollisionError::InvalidObject("object lock poisoned".into()))?;

        if !guard.is_kind_of(KindOf::ArmorSalvager) {
            return Ok(false);
        }

        Ok(!guard.test_armor_set_flag(ArmorSetFlag::CrateUpgradeTwo))
    }

    fn eligible_for_level(&self, other_id: ObjectID) -> Result<bool, CollisionError> {
        let Some(other) = resolve_crate_object(other_id) else {
            return Ok(false);
        };

        let tracker = {
            let guard = other
                .read()
                .map_err(|_| CollisionError::InvalidObject("object lock poisoned".into()))?;
            match guard.get_experience_tracker() {
                Some(tracker) => tracker,
                None => return Ok(false),
            }
        };

        let tracker = tracker.lock().map_err(|_| {
            CollisionError::InvalidObject("experience tracker lock poisoned".into())
        })?;

        if tracker.get_veterancy_level() == VeterancyLevel::Heroic {
            return Ok(false);
        }

        Ok(tracker.is_trainable())
    }

    fn test_weapon_chance(&self) -> bool {
        if self.module_data.weapon_chance == 1.0 {
            return true;
        }
        GameLogicRandomValueReal(0.0, 1.0) < self.module_data.weapon_chance
    }

    fn test_level_chance(&self) -> bool {
        if self.module_data.level_chance == 1.0 {
            return true;
        }
        GameLogicRandomValueReal(0.0, 1.0) < self.module_data.level_chance
    }

    fn do_weapon_set(&self, other_id: ObjectID) -> Result<(), CollisionError> {
        let Some(other) = resolve_crate_object(other_id) else {
            return Ok(());
        };

        let mut guard = other
            .write()
            .map_err(|_| CollisionError::InvalidObject("object lock poisoned".into()))?;

        if guard.test_weapon_set_flag(WeaponSetType::CrateUpgradeOne) {
            guard.clear_weapon_set_flag(WeaponSetType::CrateUpgradeOne);
            guard.set_weapon_set_flag(WeaponSetType::CrateUpgradeTwo);
        } else {
            guard.set_weapon_set_flag(WeaponSetType::CrateUpgradeOne);
        }
        Ok(())
    }

    fn do_armor_set(&self, other_id: ObjectID) -> Result<(), CollisionError> {
        let Some(other) = resolve_crate_object(other_id) else {
            return Ok(());
        };

        let mut guard = other
            .write()
            .map_err(|_| CollisionError::InvalidObject("object lock poisoned".into()))?;

        if guard.test_armor_set_flag(ArmorSetFlag::CrateUpgradeOne) {
            guard.clear_armor_set_flag(ArmorSetFlag::CrateUpgradeOne);
            guard.set_armor_set_flag(ArmorSetFlag::CrateUpgradeTwo);
            guard
                .clear_and_set_model_condition_flags(
                    ModelConditionFlags::ArmorsetCrateUpgradeOne,
                    ModelConditionFlags::ArmorsetCrateUpgradeTwo,
                )
                .map_err(CollisionError::InvalidObject)?;
        } else {
            guard.set_armor_set_flag(ArmorSetFlag::CrateUpgradeOne);
            guard.set_model_condition_state(ModelConditionFlags::ArmorsetCrateUpgradeOne);
        }
        Ok(())
    }

    fn do_level_gain(&self, other_id: ObjectID) -> Result<(), CollisionError> {
        let Some(other) = resolve_crate_object(other_id) else {
            return Ok(());
        };

        let tracker = {
            let guard = other
                .read()
                .map_err(|_| CollisionError::InvalidObject("object lock poisoned".into()))?;
            match guard.get_experience_tracker() {
                Some(tracker) => tracker,
                None => return Ok(()),
            }
        };

        let mut tracker = tracker.lock().map_err(|_| {
            CollisionError::InvalidObject("experience tracker lock poisoned".into())
        })?;
        let old_level = tracker.get_veterancy_level();
        if tracker.gain_exp_for_level(1, true, &ExperienceTracker::DEFAULT_EXPERIENCE_REQUIRED) {
            let new_level = tracker.get_veterancy_level();
            drop(tracker);
            if old_level != new_level {
                other
                    .write()
                    .map_err(|_| CollisionError::InvalidObject("object lock poisoned".into()))?
                    .on_veterancy_level_changed(old_level, new_level, true);
            }
        }
        Ok(())
    }

    fn do_money(&self, other_id: ObjectID) -> Result<(), CollisionError> {
        let Some(other) = resolve_crate_object(other_id) else {
            return Ok(());
        };

        let payout = if self.module_data.minimum_money != self.module_data.maximum_money {
            GameLogicRandomValue(
                self.module_data.minimum_money,
                self.module_data.maximum_money,
            )
        } else {
            self.module_data.minimum_money
        };

        if payout <= 0 {
            return Ok(());
        }

        let owner = {
            let guard = other
                .read()
                .map_err(|_| CollisionError::InvalidObject("object lock poisoned".into()))?;
            guard.get_controlling_player()
        };

        let Some(player_arc) = owner else {
            return Ok(());
        };

        {
            let mut player = player_arc
                .write()
                .map_err(|_| CollisionError::InvalidObject("player lock poisoned".into()))?;
            player.get_money_mut().add_money(payout);
            player
                .get_score_keeper_mut()
                .add_money_earned(payout as u32);
        }

        self.display_money_floating_text(payout as u32, &other, &player_arc)?;
        Ok(())
    }

    fn display_money_floating_text(
        &self,
        amount: u32,
        object: &Arc<RwLock<Object>>,
        player: &Arc<RwLock<Player>>,
    ) -> Result<(), CollisionError> {
        let (position, color) = {
            let object_id = object
                .read()
                .map(|g| g.get_id())
                .unwrap_or(crate::common::INVALID_ID);
            let position = self.money_floating_text_position(object_id)?;
            let player_guard = player
                .read()
                .map_err(|_| CollisionError::InvalidObject("player lock poisoned".into()))?;
            let mut color = player_guard.get_player_color();
            color.a = 230;
            (position, color)
        };

        let caption = format_add_cash(amount);
        TheInGameUI::add_floating_text(&caption, &position, color)
            .map_err(|err| CollisionError::InvalidObject(err.to_string()))
    }

    fn money_floating_text_position(
        &self,
        collector_id: ObjectID,
    ) -> Result<Coord3D, CollisionError> {
        let Some(collector) = resolve_crate_object(collector_id) else {
            return Err(CollisionError::InvalidObject(
                "collector unavailable".into(),
            ));
        };

        let source = self
            .base
            .get_object()
            .unwrap_or_else(|_| Arc::clone(&collector));
        let mut position = *source
            .read()
            .map_err(|_| CollisionError::InvalidObject("object lock poisoned".into()))?
            .get_position();
        position.z += 10.0;
        Ok(position)
    }

    fn play_salvage_sound(&self, other_id: ObjectID) -> Result<(), CollisionError> {
        let Some(other) = resolve_crate_object(other_id) else {
            return Ok(());
        };

        let id = other
            .read()
            .map_err(|_| CollisionError::InvalidObject("object lock poisoned".into()))?
            .get_id();
        if let Some(audio) = TheAudio::get() {
            let mut audio_event = TheAudio::get_misc_audio().crate_salvage.clone();
            audio_event.set_object_id(id);
            audio.add_audio_event(&audio_event);
        }
        Ok(())
    }

    fn play_money_sound(&self, other_id: ObjectID) -> Result<(), CollisionError> {
        let Some(other) = resolve_crate_object(other_id) else {
            return Ok(());
        };

        let id = other
            .read()
            .map_err(|_| CollisionError::InvalidObject("object lock poisoned".into()))?
            .get_id();
        if let Some(audio) = TheAudio::get() {
            let mut audio_event = TheAudio::get_misc_audio().crate_money.clone();
            audio_event.set_object_id(id);
            audio.add_audio_event(&audio_event);
        }
        Ok(())
    }

    fn record_salvage_collected(&self, other_id: ObjectID) -> Result<(), CollisionError> {
        let Some(other) = resolve_crate_object(other_id) else {
            return Ok(());
        };

        let owner = {
            let guard = other
                .read()
                .map_err(|_| CollisionError::InvalidObject("object lock poisoned".into()))?;
            guard.get_controlling_player()
        };

        let Some(player_arc) = owner else {
            return Ok(());
        };

        let mut player = player_arc
            .write()
            .map_err(|_| CollisionError::InvalidObject("player lock poisoned".into()))?;
        player.get_academy_stats_mut().record_salvage_collected();
        Ok(())
    }
}

impl CrateCollideBehavior for SalvageCrateCollide {
    fn execute_crate_behavior(&mut self, other: &dyn GameObject) -> Result<bool, CollisionError> {
        let handle = self.require_object_handle(other)?;
        let other_id = handle
            .read()
            .map(|g| g.get_id())
            .unwrap_or(crate::common::INVALID_ID);
        let salvage_type = self.determine_salvage_type(other_id)?;

        match salvage_type {
            SalvageType::ArmorSet => {
                self.do_armor_set(other_id)?;
                self.play_salvage_sound(other_id)?;
            }
            SalvageType::WeaponSet => {
                self.do_weapon_set(other_id)?;
                self.play_salvage_sound(other_id)?;
            }
            SalvageType::Level => {
                self.do_level_gain(other_id)?;
            }
            SalvageType::Money => {
                self.do_money(other_id)?;
                self.play_money_sound(other_id)?;
            }
        }

        self.record_salvage_collected(other_id)?;

        Ok(true)
    }

    fn is_valid_to_execute(&self, other: &dyn GameObject) -> bool {
        if !self.base.is_valid_to_execute(other) {
            return false;
        }

        let Ok(handle) = self.require_object_handle(other) else {
            return false;
        };

        let Ok(guard) = handle.read() else {
            return false;
        };

        guard.get_template().is_kind_of(KindOf::Salvager)
    }
}

impl CollideModule for SalvageCrateCollide {
    fn on_collide(
        &mut self,
        other: Option<&dyn GameObject>,
        _loc: &CollisionCoord3D,
        _normal: &CollisionCoord3D,
    ) -> Result<(), CollisionError> {
        let Some(other_obj) = other else {
            return Ok(());
        };

        if !CrateCollideBehavior::is_valid_to_execute(self, other_obj) {
            return Ok(());
        }

        let success = self.execute_crate_behavior(other_obj)?;
        self.base.finish_execution_attempt(other_obj, success)?;

        Ok(())
    }

    fn would_like_to_collide_with(&self, other: &dyn GameObject) -> bool {
        CrateCollideBehavior::is_valid_to_execute(self, other)
    }

    fn is_salvage_crate_collide(&self) -> bool {
        true
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SalvageType {
    ArmorSet,
    WeaponSet,
    Level,
    Money,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::object::registry::OBJECT_REGISTRY;
    use crate::object::ObjectStatusMaskType;
    use crate::player::{player_list, Player};
    use crate::team::Team;
    use std::collections::HashMap;

    struct AudioEventsGuard(bool);

    impl AudioEventsGuard {
        fn disabled() -> Self {
            Self(crate::helpers::set_audio_events_enabled_for_tests(false))
        }
    }

    impl Drop for AudioEventsGuard {
        fn drop(&mut self) {
            crate::helpers::set_audio_events_enabled_for_tests(self.0);
        }
    }

    fn object_with_kind_of(id: ObjectId, kind_of: &str) -> Arc<RwLock<Object>> {
        let mut template = DefaultThingTemplate::new(format!("TestKindOf{id}"));
        let mut properties = HashMap::new();
        properties.insert("KindOf".to_string(), kind_of.to_string());
        template.parse_object_fields_from_ini(&properties);
        Arc::new(RwLock::new(Object::new_raw(
            Arc::new(template),
            id,
            ObjectStatusMaskType::none(),
            None,
        )))
    }

    #[test]
    fn salvage_crate_parse_from_ini_preserves_cpp_fields() {
        let _lock = crate::test_sync::lock();

        let mut data = SalvageCrateCollideModuleData::default();
        let mut ini = INI::new();
        ini.with_inline_source(
            "WeaponChance = 35%\n\
             LevelChance = 40%\n\
             MoneyChance = 80%\n\
             MinMoney = 123\n\
             MaxMoney = 456\n\
             RequiredKindOf = SALVAGER|VEHICLE\n\
             ExecuteAnimationTime = 2.5\n\
             End\n",
            |ini| data.parse_from_ini(ini),
        )
        .expect("salvage crate ini parses");

        assert!((data.weapon_chance - 0.35).abs() < f32::EPSILON);
        assert!((data.level_chance - 0.40).abs() < f32::EPSILON);
        assert!((data.money_chance - 0.80).abs() < f32::EPSILON);
        assert_eq!(data.minimum_money, 123);
        assert_eq!(data.maximum_money, 456);
        assert_ne!(
            data.base.required_kind_of & (1u64 << (KindOf::Salvager as u32)),
            0
        );
        assert_ne!(
            data.base.required_kind_of & (1u64 << (KindOf::Vehicle as u32)),
            0
        );
        assert!((data.base.execute_animation_display_time_seconds - 2.5).abs() < f32::EPSILON);
    }

    #[test]
    fn salvage_crate_rejects_missing_cpp_field_value() {
        let mut data = SalvageCrateCollideModuleData::default();
        let mut ini = INI::new();

        let err = ini
            .with_inline_source("WeaponChance =\nEnd\n", |ini| data.parse_from_ini(ini))
            .expect_err("missing chance value should fail");

        assert!(matches!(err, INIError::InvalidData));
        assert!((data.weapon_chance - 1.0).abs() < f32::EPSILON);
    }

    #[test]
    fn salvage_money_label_formats_cpp_style_template() {
        assert_eq!(format_cash_template("+$%d", 125, "+"), "+$125");
        assert_eq!(format_cash_template("+$%i", 125, "+"), "+$125");
        assert_eq!(format_cash_template("+$%u", 125, "+"), "+$125");
        assert_eq!(format_cash_template("+$%04d", 125, "+"), "+$125");
        assert_eq!(format_cash_template("+$%-6u", 125, "+"), "+$125");
        assert_eq!(format_cash_template("%% +$%d", 125, "+"), "% +$125");
        assert_eq!(format_cash_template("GUI:AddCash", 125, "+"), "+$125");
    }

    #[test]
    fn salvage_money_floating_text_uses_crate_position_like_cpp() {
        let _lock = crate::test_sync::lock();

        OBJECT_REGISTRY.clear();

        let crate_obj = Arc::new(RwLock::new(Object::new_test(1003, 100.0)));
        crate_obj
            .write()
            .expect("crate write")
            .set_position(&Coord3D::new(11.0, 22.0, 3.0))
            .expect("crate position");
        OBJECT_REGISTRY.register_object(1003, &crate_obj);

        let collector = Arc::new(RwLock::new(Object::new_test(1004, 100.0)));
        collector
            .write()
            .expect("collector write")
            .set_position(&Coord3D::new(200.0, 300.0, 4.0))
            .expect("collector position");

        let module = SalvageCrateCollide::new(1003, SalvageCrateCollideModuleData::default());
        let position = module
            .money_floating_text_position(&collector)
            .expect("money text position");

        assert!((position.x - 11.0).abs() < f32::EPSILON);
        assert!((position.y - 22.0).abs() < f32::EPSILON);
        assert!((position.z - 13.0).abs() < f32::EPSILON);

        OBJECT_REGISTRY.clear();
    }

    #[test]
    fn successful_salvage_records_academy_stat_like_cpp() {
        let _lock = crate::test_sync::lock();
        let _audio_guard = AudioEventsGuard::disabled();

        player_list().write().expect("player list write").clear();

        let player = Arc::new(RwLock::new(Player::new(0)));
        player_list()
            .write()
            .expect("player list write")
            .add_player(Arc::clone(&player));

        let team = Arc::new(RwLock::new(Team::new("SalvageCollectorTeam".into(), 991)));
        team.write()
            .expect("team write")
            .set_controlling_player_id(Some(0));

        let collector = Arc::new(RwLock::new(Object::new_test(991, 100.0)));
        collector
            .write()
            .expect("collector write")
            .set_team(Some(team))
            .expect("collector team set");

        let data = SalvageCrateCollideModuleData {
            weapon_chance: 0.0,
            level_chance: 0.0,
            minimum_money: 0,
            maximum_money: 0,
            ..Default::default()
        };
        let mut module = SalvageCrateCollide::new(1001, data);

        assert!(module
            .execute_crate_behavior(&collector)
            .expect("salvage executes"));

        assert_eq!(
            player
                .read()
                .expect("player read")
                .get_academy_stats()
                .get_salvage_collected(),
            1
        );

        player_list().write().expect("player list write").clear();
    }

    #[test]
    fn on_collide_rejects_non_salvager_after_base_building_pickup_allows_it() {
        let _lock = crate::test_sync::lock();

        player_list().write().expect("player list write").clear();

        let player = Arc::new(RwLock::new(Player::new(0)));
        player
            .write()
            .expect("player write")
            .get_money_mut()
            .set_money(100);
        player_list()
            .write()
            .expect("player list write")
            .add_player(Arc::clone(&player));

        let team = Arc::new(RwLock::new(Team::new(
            "NonSalvagerBuildingTeam".into(),
            992,
        )));
        team.write()
            .expect("team write")
            .set_controlling_player_id(Some(0));

        let collector = object_with_kind_of(992, "STRUCTURE");
        collector
            .write()
            .expect("collector write")
            .set_team(Some(team))
            .expect("collector team set");

        let data = SalvageCrateCollideModuleData {
            base: CrateCollideModuleData {
                is_building_pickup: true,
                ..CrateCollideModuleData::default()
            },
            minimum_money: 50,
            maximum_money: 50,
            ..Default::default()
        };
        let mut module = SalvageCrateCollide::new(1002, data);
        let origin = CollisionCoord3D::new(0.0, 0.0, 0.0);

        module
            .on_collide(Some(&collector), &origin, &origin)
            .expect("collide returns ok");

        let player = player.read().expect("player read");
        assert_eq!(player.get_money().get_money(), 100);
        assert_eq!(player.get_academy_stats().get_salvage_collected(), 0);

        player_list().write().expect("player list write").clear();
    }
}

impl game_engine::common::system::Snapshotable for SalvageCrateCollide {
    fn crc(&self, xfer: &mut dyn game_engine::common::system::Xfer) -> Result<(), String> {
        self.base.crc(xfer)
    }

    fn xfer(&mut self, xfer: &mut dyn game_engine::common::system::Xfer) -> Result<(), String> {
        // C++ parity: versioned xfer entry point (current version 1).
        let mut version: u8 = 1;
        xfer.xfer_version(&mut version, 1)
            .map_err(|err| err.to_string())?;
        self.base.xfer(xfer)
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        self.base.load_post_process()
    }
}
