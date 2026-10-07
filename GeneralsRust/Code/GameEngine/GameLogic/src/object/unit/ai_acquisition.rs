//! C++ AIUpdate.cpp:4471-4648: native mood-target admission and scan order.
//! The source is the constructor-bound Object. Target/world services still
//! use the active-world adapters and are not instance-isolated by this module.
use super::ai_core::UnitAIUpdate;
use super::imports::*;
use super::registry::{dual_world_registry_unavailable, get_unit_arc};
use crate::ai::{search_qualifiers, vision_factors};
use crate::object::update::ai_update_interface::{
    AUTO_ACQUIRE_IDLE, AUTO_ACQUIRE_IDLE_ATTACK_BUILDINGS, AUTO_ACQUIRE_IDLE_NOT_WHILE_ATTACKING,
    AUTO_ACQUIRE_IDLE_STEALTHED,
};

impl UnitAIUpdate {
    pub(super) fn get_next_mood_target_id(
        &mut self,
        called_by_ai: bool,
        called_during_idle: bool,
    ) -> ObjectID {
        self.get_next_mood_target_for_state(called_by_ai, called_during_idle, None)
    }

    /// An in-state caller supplies the current State's real virtual classifier.
    /// External callers obtain it from the machine. No classification is cached.
    pub(super) fn get_next_mood_target_for_state(
        &mut self,
        called_by_ai: bool,
        called_during_idle: bool,
        is_attacking: Option<bool>,
    ) -> ObjectID {
        if self.owner.is_none() {
            return self.get_legacy_mood_target(called_by_ai, called_during_idle);
        }
        let Some(owner) = self.owner.as_ref().and_then(Weak::upgrade) else {
            return INVALID_ID;
        };
        let Ok(source) = owner.read() else {
            return INVALID_ID;
        };
        if source.is_effectively_dead() || source.test_status(ObjectStatusTypes::IsUsingAbility) {
            return INVALID_ID;
        }
        let mask = self.data.auto_acquire_enemies_when_idle;
        if called_during_idle && mask & AUTO_ACQUIRE_IDLE == 0 {
            return INVALID_ID;
        }
        if mask & AUTO_ACQUIRE_IDLE_NOT_WHILE_ATTACKING != 0
            && is_attacking.unwrap_or_else(|| self.is_attacking())
        {
            return INVALID_ID;
        }
        if called_during_idle && source.test_status(ObjectStatusTypes::Stealthed) {
            let special_power = source.get_stealth().is_some_and(|handle| {
                handle
                    .lock()
                    .ok()
                    .is_some_and(|stealth| stealth.is_granted_by_special_power())
            });
            if !special_power && mask & AUTO_ACQUIRE_IDLE_STEALTHED == 0 {
                let passenger_may_fire = source
                    .get_contained_by()
                    .and_then(|id| {
                        crate::object::registry::OBJECT_REGISTRY.with_object(id, |container| {
                            container.get_contain().is_some_and(|contain| {
                                contain.lock().ok().is_some_and(|contain| {
                                    contain.is_passenger_allowed_to_fire(None)
                                })
                            })
                        })
                    })
                    .unwrap_or(false);
                if !passenger_may_fire {
                    return INVALID_ID;
                }
            }
        }
        let now = TheGameLogic::get_frame();
        if called_by_ai {
            let common_target = source
                .get_team()
                .and_then(|team| {
                    let team = team.read().ok()?;
                    team.attack_common_target()
                        .then(|| team.get_team_target_object())
                })
                .unwrap_or(INVALID_ID);
            if common_target != INVALID_ID && common_target != source.get_id() {
                let can_attack = crate::object::registry::OBJECT_REGISTRY
                    .with_object(common_target, |target| {
                        matches!(
                            source.get_able_to_attack_specific_object_for_objects(
                                crate::attack::AbleToAttackType::NewTarget,
                                target,
                                CommandSourceType::FromAi,
                            ),
                            crate::attack::CanAttackResult::Possible
                                | crate::attack::CanAttackResult::PossibleAfterMoving
                        )
                    })
                    .unwrap_or(false);
                if can_attack
                    && matches!(
                        self.data.attitude,
                        AIAttitudeType::Normal
                            | AIAttitudeType::Defensive
                            | AIAttitudeType::Aggressive
                    )
                {
                    return common_target;
                }
            }
            if now < self.data.next_mood_check_time {
                return INVALID_ID;
            }
            let rate = self.data.mood_attack_check_rate_frames as i32;
            self.data.next_mood_check_time = now.wrapping_add(rate as u32);
            if self.data.randomly_offset_mood_check {
                let half_rate = rate >> 1;
                let offset = game_engine::common::random_value::get_game_logic_random_value(
                    -half_rate, half_rate,
                );
                self.data.next_mood_check_time =
                    self.data.next_mood_check_time.wrapping_add(offset as u32);
                self.data.randomly_offset_mood_check = false;
            }
        }
        let ai_store = the_ai();
        let Ok(ai) = ai_store.read() else {
            return INVALID_ID;
        };
        let mut range = ai.get_adjusted_vision_range_for_source(
            &source,
            vision_factors::OWNER_TYPE | vision_factors::MOOD,
            Some(self.data.attitude),
        );
        if range <= 0.0 {
            return INVALID_ID;
        }
        if let Some(container_id) = source.get_contained_by() {
            if let Some(radius) = crate::object::registry::OBJECT_REGISTRY
                .with_object(container_id, |container| {
                    container.get_geometry_info().get_bounding_circle_radius()
                })
            {
                range += radius;
            }
        }
        let controller_is_human = source.with_controlling_player(|player| {
            player.get_player_type() == crate::player::PlayerType::Human
        });
        let human = controller_is_human == Some(true);
        if controller_is_human == Some(false) && self.data.attitude == AIAttitudeType::Passive {
            if source.get_body_module().is_none() {
                return INVALID_ID;
            }
            let Some(damage) = source.get_last_damage_info() else {
                return INVALID_ID;
            };
            if damage.input.damage_type != crate::damage::DamageType::Healing {
                return crate::object::registry::OBJECT_REGISTRY
                    .get_object(damage.input.source_id)
                    .map(|_| damage.input.source_id)
                    .unwrap_or(INVALID_ID);
            }
        }
        let rules = ai.get_ai_data();
        let mut qualifiers = search_qualifiers::CAN_ATTACK;
        if rules.attack_uses_line_of_sight && source.is_kind_of(KindOf::AttackNeedsLineOfSight) {
            qualifiers |= search_qualifiers::CAN_SEE;
        }
        if rules.attack_ignore_insignificant_buildings {
            qualifiers |= search_qualifiers::IGNORE_INSIGNIFICANT_BUILDINGS;
        }
        if mask & AUTO_ACQUIRE_IDLE_ATTACK_BUILDINGS != 0 {
            qualifiers |= search_qualifiers::ATTACK_BUILDINGS;
        }
        if called_by_ai && human {
            qualifiers |= search_qualifiers::WITHIN_ATTACK_RANGE | search_qualifiers::UNFOGGED;
        }
        let priorities = ai.attack_priority_info_for_source(&source);
        ai.find_closest_enemy_for_source(&source, range, qualifiers, priorities.as_ref(), None)
            .ok()
            .flatten()
            .unwrap_or(INVALID_ID)
    }

    // Explicit standalone adapter; native Objects never enter the Unit lookup.
    pub(super) fn get_legacy_mood_target(
        &mut self,
        use_existing_target: bool,
        _ignore_attacked: bool,
    ) -> ObjectID {
        // Wave 258: empty dual-world → invalid id.

        if dual_world_registry_unavailable() {
            return INVALID_ID;
        }

        let Some(unit) = get_unit_arc(self.unit_id) else {
            return INVALID_ID;
        };
        let Ok(guard) = unit.read() else {
            return INVALID_ID;
        };
        if !guard.can_auto_acquire_now() {
            return INVALID_ID;
        }

        let max_range = guard.engagement_range;
        if use_existing_target {
            if let Some(existing_id) = self.get_current_victim() {
                if let Some(existing_arc) =
                    crate::object::registry::OBJECT_REGISTRY.get_object(existing_id)
                {
                    if let Ok(existing_guard) = existing_arc.read() {
                        let relationship = guard
                            .base_arc()
                            .read()
                            .ok()
                            .map(|base| base.relationship_to(&existing_guard))
                            .unwrap_or(Relationship::Neutral);
                        if relationship == Relationship::Enemies {
                            let target_pos = *existing_guard.get_position();
                            let self_pos = guard.get_position();
                            let dx = target_pos.x - self_pos.x;
                            let dy = target_pos.y - self_pos.y;
                            let dist = (dx * dx + dy * dy).sqrt();
                            if dist <= max_range && guard.can_detect_target(&existing_guard, dist) {
                                return existing_id;
                            }
                        }
                    }
                }
            }
        }

        let ai_store = the_ai();
        let Ok(ai) = ai_store.read() else {
            return INVALID_ID;
        };
        let ai_data = ai.get_ai_data();

        let mut qualifiers = search_qualifiers::CAN_ATTACK;
        if ai_data.attack_uses_line_of_sight {
            qualifiers |= search_qualifiers::CAN_SEE;
        }
        if ai_data.attack_ignore_insignificant_buildings {
            qualifiers |= search_qualifiers::IGNORE_INSIGNIFICANT_BUILDINGS;
        }
        if guard.auto_acquire_attack_buildings {
            qualifiers |= search_qualifiers::ATTACK_BUILDINGS;
        }

        ai.find_closest_enemy(guard.get_id(), max_range, qualifiers, None, None)
            .ok()
            .flatten()
            .unwrap_or(INVALID_ID)
    }
}
