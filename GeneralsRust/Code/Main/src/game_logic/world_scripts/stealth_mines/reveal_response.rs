//! Source-order stealth reveal response.
use super::*;

impl GameLogic {
    /// C++ `StealthUpdate::markAsDetected` (`StealthUpdate.cpp:892-910`) +
    /// `setWakeupIfInRange` / `wakeUpAndAttemptToTarget`.
    ///
    /// Object has no world access, so the idle-enemy walk lives here. Retail
    /// Burton / Kell / Lotus / Pathfinder / Listening Outpost / CamoNetting
    /// author `OrderIdleEnemiesToAttackMeUponReveal`. Idle enemy AI in vision
    /// gets `m_nextMoodCheckTime = now` so same-frame mood acquire can fire.

    pub(crate) fn order_idle_enemies_to_attack_on_reveal(&mut self, victim_id: ObjectId) {
        let Some(victim) = self.objects.get(&victim_id) else {
            return;
        };
        if !crate::game_logic::object::order_idle_enemies_on_reveal(&victim.template_name) {
            return;
        }
        let v_team = victim.team;
        let v_owner = victim.owner_player_id;
        let v_pos = victim.get_position();
        let now = self.frame;

        let idle_in_vision: Vec<ObjectId> = self
            .objects
            .iter()
            .filter_map(|(&id, o)| {
                if id == victim_id || !o.is_alive() {
                    return None;
                }
                // C++ `setWakeupIfInRange` requires `getAI()`.
                if matches!(o.object_type, ObjectType::Projectile) {
                    return None;
                }
                // C++ Object::getRelationship (Object.cpp:1548-1568) falls through
                // to the controlling players' team relationship: two living
                // players on different non-neutral teams are ENEMIES even when
                // the lobby carries no explicit playerEnemies row. A strict
                // player-map NEUTRAL must not hard-reject the wake.
                let mut enemy = match (o.owner_player_id, v_owner) {
                    (Some(a), Some(b)) => {
                        self.player_relationship(a, b) == gamelogic::common::Relationship::Enemies
                    }
                    _ => o.team != v_team && o.team != Team::Neutral && v_team != Team::Neutral,
                };
                if !enemy
                    && o.owner_player_id.is_some()
                    && v_owner.is_some()
                    && o.team != Team::Neutral
                    && v_team != Team::Neutral
                    && o.team != v_team
                {
                    enemy = true;
                }
                if !enemy {
                    return None;
                }
                let vision = if o.vision_range > 0.0 {
                    o.vision_range
                } else {
                    o.get_template().sight_range.max(0.0)
                };
                if vision <= 0.0 || o.get_position().distance(v_pos) > vision {
                    return None;
                }
                // C++ `wakeUpAndAttemptToTarget` no-ops unless idle.
                let idle = matches!(o.ai_state, AIState::Idle) && o.target.is_none();
                idle.then_some(id)
            })
            .collect();

        for eid in idle_in_vision {
            if let Some(enemy) = self.objects.get_mut(&eid) {
                enemy.next_mood_check_time = now;
                let can_attack = enemy.weapon.is_some()
                    || enemy.is_kind_of(KindOf::Attackable)
                    || enemy.can_attack()
                    || matches!(
                        enemy.object_type,
                        ObjectType::Infantry | ObjectType::Vehicle | ObjectType::Aircraft
                    );
                if !can_attack
                    || enemy.is_kind_of(KindOf::Structure)
                    || enemy.is_kind_of(KindOf::Worker)
                    || enemy.is_worker()
                {
                    continue;
                }
                enemy.set_target(
                    Some(victim_id),
                    &mut self.attack_events,
                    &mut crate::game_logic::MovementAudio::queue(&mut self.queued_audio_events),
                );
                {}
                self.camo_netting_order_idle_enemies_count =
                    self.camo_netting_order_idle_enemies_count.saturating_add(1);
            }
        }
    }
}
