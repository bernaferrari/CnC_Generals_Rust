//! Anthrax Bomb Special Power
//!
//! GLA special power that drops a bomb releasing deadly anthrax gas that damages
//! infantry and leaves a toxic cloud for a duration.

use super::base_power::*;
use super::cooldown::CooldownState;
use super::targeting::TargetingInfo;
use super::types::*;
use crate::common::*;
use crate::helpers::{TheFXList, TheThingFactory};
use crate::player::player_list;

const ANTHRAX_RADIUS: Real = 200.0;
const ANTHRAX_DURATION: Real = 45.0;
const ANTHRAX_DPS: Real = 50.0; // Damage per second

#[derive(Debug, Clone)]
pub struct AnthraxBombPowerData {
    pub base: SpecialPowerModuleData,
    pub cloud_radius: Real,
    pub cloud_duration: Real,
    pub damage_per_second: Real,
    pub affects_buildings: Bool,
    pub bomb_template: AsciiString,
}

impl AnthraxBombPowerData {
    pub fn new(name: AsciiString) -> Self {
        let mut base = SpecialPowerModuleData::new(name, SpecialPowerKind::OCL);
        base.flags = SpecialPowerFlags::REQUIRES_TARGETING
            | SpecialPowerFlags::AFFECTS_ENEMY
            | SpecialPowerFlags::SUPERWEAPON;
        base.recharge_time = 360.0; // 6 minutes (360000 ms)
        base.cost = 0;
        base.range = 0.0;
        base.radius = ANTHRAX_RADIUS;

        Self {
            base,
            cloud_radius: ANTHRAX_RADIUS,
            cloud_duration: ANTHRAX_DURATION,
            damage_per_second: ANTHRAX_DPS,
            affects_buildings: false, // Only affects infantry
            bomb_template: "AnthraxBomb".into(),
        }
    }
}

pub struct AnthraxBombPower {
    data: AnthraxBombPowerData,
    cooldown: CooldownState,
    stats: SpecialPowerStats,
    bomber_id: Option<ObjectID>,
    cloud_id: Option<ObjectID>,
}

impl AnthraxBombPower {
    pub fn new(data: AnthraxBombPowerData) -> Self {
        let cooldown = CooldownState::new(data.base.recharge_time, data.base.init_charge_time);

        Self {
            data,
            cooldown,
            stats: SpecialPowerStats::new(),
            bomber_id: None,
            cloud_id: None,
        }
    }

    fn execute_strike(
        &mut self,
        player_id: ObjectID,
        targeting: &TargetingInfo,
    ) -> Result<(), String> {
        log::info!(
            "Anthrax Bomb activated at position {:?}",
            targeting.position
        );

        // Spawn bomb payload (bomber/transport is handled by OCL in the original engine)
        let spawn_pos = Coord3D::new(
            targeting.position.x,
            targeting.position.y,
            targeting.position.z + 300.0,
        );

        log::debug!("Spawning anthrax payload at {:?}", spawn_pos);
        if !self.data.bomb_template.is_empty() {
            if let Some(template) = TheThingFactory::find_template(self.data.bomb_template.as_str())
            {
                let list = player_list()
                    .read()
                    .map_err(|_| "PlayerList lock poisoned".to_string())?;
                let player = list
                    .get_player(player_id as Int)
                    .cloned()
                    .ok_or_else(|| format!("Invalid player id {}", player_id))?;
                let team_arc = player
                    .read()
                    .map_err(|_| "Player lock poisoned".to_string())?
                    .get_default_team()
                    .ok_or_else(|| format!("Player {} has no default team", player_id))?;

                let team_guard = team_arc
                    .read()
                    .map_err(|_| "Team lock poisoned".to_string())?;
                let factory = TheThingFactory::get().map_err(|e| e.to_string())?;
                let bomb = factory
                    .new_object(template.clone(), &*team_guard)
                    .map_err(|e| e.to_string())?;

                bomb.write()
                    .map_err(|_| "Anthrax bomb lock poisoned".to_string())?
                    .set_position(&spawn_pos)?;

                let bomb_id = bomb
                    .read()
                    .map_err(|_| "Anthrax bomb lock poisoned".to_string())?
                    .get_id();
                self.bomber_id = Some(bomb_id);
            } else {
                log::warn!(
                    "Anthrax bomb template '{}' not found",
                    self.data.bomb_template
                );
            }
        }

        if self.bomber_id.is_none() {
            self.bomber_id = Some(60001);
        }

        if let Some(fx_list) = TheFXList::get() {
            fx_list.do_fx_at_position("FX_AnthraxBomb", &targeting.position);
        }

        // Drop bomb and create toxic cloud
        self.create_anthrax_cloud(player_id, targeting)?;

        Ok(())
    }

    fn create_anthrax_cloud(
        &mut self,
        attacker_id: ObjectID,
        targeting: &TargetingInfo,
    ) -> Result<(), String> {
        log::debug!(
            "Creating anthrax cloud: radius={}, duration={}s",
            self.data.cloud_radius,
            self.data.cloud_duration
        );

        use super::area_damage::{AreaDamageApplicator, AreaDamageConfig, DamageFalloff};

        let mut config = AreaDamageConfig::new(self.data.damage_per_second, self.data.cloud_radius);
        config.min_damage = 0.0;
        config.falloff = DamageFalloff::Linear;
        config.damage_type = DamageTypeFlags::POISON;
        config.affects_friendlies = false;
        config.affects_buildings = self.data.affects_buildings;
        config.affects_terrain = false;

        let field_id = AreaDamageApplicator::create_damage_over_time(
            &config,
            &targeting.position,
            self.data.cloud_duration,
            1.0,
            attacker_id,
        )?;

        self.cloud_id = Some(field_id);

        // Apply initial damage
        self.apply_cloud_damage(&targeting.position, &config, attacker_id)?;

        Ok(())
    }

    fn apply_cloud_damage(
        &mut self,
        position: &Coord3D,
        config: &super::area_damage::AreaDamageConfig,
        attacker_id: ObjectID,
    ) -> Result<(), String> {
        log::debug!("Applying anthrax cloud damage");

        if let Ok(result) = super::area_damage::AreaDamageApplicator::apply_damage_at_location(
            config,
            position,
            attacker_id,
        ) {
            self.stats.record_damage(result.total_damage);
        } else {
            self.stats.record_damage(self.data.damage_per_second);
        }

        Ok(())
    }
}

impl SpecialPowerModuleInterface for AnthraxBombPower {
    fn get_data(&self) -> &SpecialPowerModuleData {
        &self.data.base
    }

    fn get_data_mut(&mut self) -> &mut SpecialPowerModuleData {
        &mut self.data.base
    }

    fn get_cooldown_state(&self) -> &CooldownState {
        &self.cooldown
    }

    fn get_cooldown_state_mut(&mut self) -> &mut CooldownState {
        &mut self.cooldown
    }

    fn get_stats(&self) -> &SpecialPowerStats {
        &self.stats
    }

    fn get_stats_mut(&mut self) -> &mut SpecialPowerStats {
        &mut self.stats
    }

    fn try_activate(
        &mut self,
        player_id: ObjectID,
        targeting: Option<&TargetingInfo>,
        current_frame: UnsignedInt,
    ) -> ActivationResult {
        let targeting = match targeting {
            Some(t) => t,
            None => {
                return ActivationResult::InvalidTarget {
                    reason: "Anthrax Bomb requires targeting".to_string(),
                };
            }
        };

        if self.is_on_cooldown() {
            return ActivationResult::OnCooldown {
                remaining: self.cooldown.time_remaining,
            };
        }

        if let Err(reason) = self.execute_strike(player_id, targeting) {
            return ActivationResult::Failed { reason };
        }

        self.cooldown.start_cooldown(current_frame);
        self.stats
            .record_activation(current_frame, self.data.base.cost);

        ActivationResult::Success
    }

    fn execute(&mut self, targeting: &TargetingInfo) -> Result<(), String> {
        self.execute_strike(0, targeting)
    }
}
