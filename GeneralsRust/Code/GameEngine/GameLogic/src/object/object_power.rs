//! Object.cpp2050-2304 / 3790-3856: borrowed power-disable phases.
use super::object_impl_imports::*;
use super::*;

/// Effects after the object's default behavior callbacks, in original order.
pub(crate) struct PowerDisabledEdge {
    pub radar: Option<bool>,
    pub power: i32,
    pub becoming_disabled: bool,
}

impl Object {
    pub(crate) fn has_spy_disabled_callback(&self) -> bool {
        self.modules.iter().any(|entry| entry.has_spy_vision)
    }

    pub(crate) fn validate_power_disable_owned(
        &self,
        frame: u32,
        native_idle: bool,
    ) -> Result<(), String> {
        if (self.is_kind_of(KindOf::Dozer) && !native_idle)
            || self.is_kind_of(KindOf::SpawnsAreTheWeapons)
        {
            return Err("power operation requires supplied dozer/spawn continuations".into());
        }
        if self.behaviors.iter().any(|b| {
            !b.has_owned_default_disabled_callback() && !b.has_owned_spy_disabled_callback()
        }) {
            return Err("power operation requires supplied custom disabled callback".into());
        }
        for entry in &self.modules {
            if entry.has_spy_vision {
                if !native_idle {
                    return Err("power SpyVision requires supplied scheduler callback".into());
                }
                continue;
            }
            if entry.with_module(|module| {
                Self::get_special_power_from_module(module)
                    .is_some_and(|power| !power.supports_owned_pause())
            }) {
                return Err("power operation requires supplied special-power clock".into());
            }
        }
        for behavior in &self.behaviors {
            let mut behavior = behavior.clone();
            let mut guard = behavior
                .access()
                .map_err(|_| "power behavior inaccessible")?;
            if guard
                .get_special_power_module_interface()
                .is_some_and(|power| !power.supports_owned_pause())
            {
                return Err("power operation requires supplied special-power clock".into());
            }
        }
        if self.contain.as_ref().is_some_and(|contain| {
            contain
                .lock()
                .ok()
                .is_some_and(|contain| contain.get_rider_id().is_some())
        }) {
            return Err("power operation requires supplied rider continuation".into());
        }
        for (index, &deadline) in self.disabled_till_frame.iter().enumerate() {
            if let Some(kind) = disabled_type_from_index(index) {
                if kind != DisabledType::DisabledUnderpowered
                    && self.is_disabled_by_type(kind)
                    && deadline <= frame
                {
                    return Err(
                        "power operation requires supplied expired-disable continuation".into(),
                    );
                }
            }
        }
        if let Some(drawable) = &self.drawable {
            drop(drawable.write().map_err(|_| "power drawable poisoned")?);
        }
        Ok(())
    }

    pub(crate) fn power_disable_sound_owned(&self, disabling: bool) -> Option<bool> {
        let power_types = [
            DisabledType::DisabledUnderpowered,
            DisabledType::DisabledEmp,
            DisabledType::DisabledSubdued,
            DisabledType::DisabledHacked,
        ];
        let other = power_types.into_iter().any(|kind| {
            self.is_disabled_by_type(kind)
                && (disabling || kind != DisabledType::DisabledUnderpowered)
        });
        if other || (!disabling && !self.is_disabled_by_type(DisabledType::DisabledUnderpowered)) {
            return None;
        }
        if self.is_kind_of(KindOf::Structure) {
            Some(true)
        } else if self.is_kind_of(KindOf::Vehicle) {
            Some(false)
        } else {
            None
        }
    }

    fn pause_power_specials_owned(
        &self,
        pause: bool,
        frame: u32,
        uses_delay: bool,
        players: &crate::player::PlayerList,
    ) -> Result<(), String> {
        let controller = self.get_team().and_then(|team| {
            team.read()
                .ok()
                .and_then(|team| team.get_controlling_player_id())
        });
        let mut apply = |power: &mut dyn SpecialPowerModuleInterface| -> Result<(), String> {
            let template = power.get_special_power_template_full();
            let shared =
                if let Some(template) = template.filter(|template| template.is_shared_n_sync()) {
                    if let Some(player) =
                        controller.and_then(|id| players.get_player(id as PlayerIndex))
                    {
                        Some(
                            player
                                .write()
                                .map_err(|_| "power shared timer player poisoned")?
                                .get_or_start_special_power_ready_frame_at(&template, frame),
                        )
                    } else {
                        None
                    }
                } else {
                    None
                };
            power.pause_countdown_owned(pause, frame, uses_delay, shared)
        };
        for entry in &self.modules {
            entry.with_module(|module| {
                if let Some(power) = Self::get_special_power_from_module(module) {
                    apply(power)
                } else {
                    Ok(())
                }
            })?;
        }
        for behavior in &self.behaviors {
            let mut behavior = behavior.clone();
            let mut guard = behavior
                .access()
                .map_err(|_| "power behavior inaccessible")?;
            if let Some(power) = guard.get_special_power_module_interface() {
                apply(power)?;
            }
        }
        Ok(())
    }

    pub(crate) fn apply_power_disable_owned(
        &mut self,
        disabling: bool,
        frame: u32,
        uses_delay: bool,
        players: &crate::player::PlayerList,
        native_idle: bool,
    ) -> Result<Option<bool>, String> {
        self.validate_power_disable_owned(frame, native_idle)
            .expect("preflighted power effects");
        let kind = DisabledType::DisabledUnderpowered;
        if !disabling && !self.is_disabled_by_type(kind) {
            return Ok(None);
        }
        let was_disabled = self.is_disabled();
        let index = self
            .get_disabled_type_index(kind)
            .expect("underpowered deadline slot");
        if disabling {
            if self.disabled_till_frame[index] != game_engine::common::game_common::FOREVER {
                if !self.is_disabled_by_type(kind) {
                    self.pause_power_specials_owned(true, frame, uses_delay, players)?;
                }
                // CPP setDisabled delegates setDisabledUntil(FOREVER).
                self.disabled_till_frame[index] = game_engine::common::game_common::FOREVER;
                if self.disabled_till_frame[index] > frame {
                    self.disabled_mask.set_disabled(kind);
                } else {
                    self.disabled_mask.clear(kind);
                }
                if self.is_disabled() {
                    if let Some(drawable) = &self.drawable {
                        drawable
                            .write()
                            .expect("preflighted drawable")
                            .set_tint_status(crate::object::drawable::TintStatus::DISABLED);
                    }
                }
            }
        } else {
            self.pause_power_specials_owned(false, frame, uses_delay, players)?;
            self.disabled_mask.clear(kind);
            self.disabled_till_frame[index] = game_engine::common::game_common::NEVER;
            if Self::flags_requiring_disabled_tint(self.disabled_mask).is_empty() {
                if let Some(drawable) = &self.drawable {
                    drawable
                        .write()
                        .expect("preflighted drawable")
                        .clear_tint_status(crate::object::drawable::TintStatus::DISABLED);
                }
            }
        }
        if was_disabled == self.is_disabled() {
            return Ok(None);
        }
        for behavior in &mut self.behaviors {
            if !behavior.has_owned_spy_disabled_callback() {
                behavior
                    .access()
                    .map_err(|_| "power behavior unavailable")?
                    .on_disabled_edge(disabling);
            }
        }
        Ok(Some(disabling))
    }

    pub(crate) fn power_disabled_edge_facts(&self, disabling: bool) -> PowerDisabledEdge {
        let radar = self.find_module_by_name("RadarUpgrade").and_then(|entry| {
            entry.with_module(|module| {
                module
                    .as_any()
                    .downcast_ref::<crate::object::upgrade::radar_upgrade::RadarUpgrade>()
                    .filter(|radar| radar.is_applied())
                    .map(|radar| radar.is_disable_proof())
            })
        });
        let mut power = self.get_template().get_energy_production().max(0);
        let bonus = self.get_template().get_energy_bonus();
        if power > 0 && bonus != 0 {
            let upgraded = self.find_module_by_name("PowerPlantUpgrade").is_some_and(|entry| {
                entry.with_module(|module| module.as_any()
                    .downcast_ref::<crate::object::upgrade::power_plant_upgrade::PowerPlantUpgrade>()
                    .is_some_and(|upgrade| upgrade.is_applied()))
            });
            if upgraded {
                power += bonus;
            }
            let overcharged = self.modules.iter().any(|entry| {
                entry.with_module(|module| {
                    module_behavior_utility_kind(module)
                        .and_then(BehaviorUtilityModuleKindMut::overcharge_active)
                        .unwrap_or(false)
                })
            });
            if overcharged {
                power += bonus;
            }
        }
        PowerDisabledEdge {
            radar,
            power,
            becoming_disabled: disabling,
        }
    }
}
