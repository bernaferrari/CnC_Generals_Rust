//! C++ `StealthUpdate::update` (StealthUpdate.cpp:568-814) for every Object
//! that owns a StealthUpdate module. The module data comes from the
//! template's authored `Behavior = StealthUpdate` block (or the inherited
//! DefaultThingTemplate module); no template name selects a behavior.

use super::*;
use crate::game_logic::object::{StealthForbiddenConditions, StealthUpdateData, StealthWorldFacts};

/// The stealth owner's rules an Object's StealthUpdate applies (C++
/// `calcStealthOwner` + `getStealthLevel` / `getStealthDelay` /
/// `getOrderIdleEnemiesToAttackMeUponReveal`, StealthUpdate.cpp:241-251, 596-610, 852-869).
#[derive(Clone, Copy, Debug)]
pub(crate) struct StealthOwnerRules {
    pub owner: ObjectId,
    pub flags: StealthForbiddenConditions,
    pub stealth_delay: u32,
    pub order_idle_enemies_to_attack: bool,
}

impl GameLogic {
    /// C++ `receiveGrant`: apply to the Object, then its RiderChange rider.
    pub(crate) fn receive_stealth_grant(&mut self, mut id: ObjectId, active: bool, frames: u32) {
        loop {
            let Some(obj) = self.objects.get_mut(&id) else {
                return;
            };
            if obj.stealth_update_data().is_none() || obj.stealth_can_disguise()
                || (active && obj.status.destroyed)
            {
                return;
            }
            if active {
                obj.apply_grant_stealth(self.frame);
                obj.stealth_frames_granted = frames;
            } else {
                obj.revoke_grant_stealth();
            }
            let rider = (obj.thing().template.contain_module.kind
                == crate::game_logic::ContainModuleKind::RiderChange)
                .then(|| obj.contained_unit_ids().first().copied())
                .flatten();
            let Some(rider) = rider else {
                return;
            };
            id = rider;
        }
    }

    /// C++ `StealthUpdate::calcStealthOwner` (StealthUpdate.cpp:531-556):
    /// under `UseRiderStealth` the first contained rider, else the Object.
    fn stealth_owner_id(&self, id: ObjectId, data: &StealthUpdateData) -> ObjectId {
        if data.use_rider_stealth {
            if let Some(rider) = self
                .objects
                .get(&id)
                .and_then(|obj| obj.contained_unit_ids().first().copied())
            {
                return rider;
            }
        }
        id
    }

    /// Rules of `id`'s StealthUpdate, read from the stealth owner's module
    /// data when a rider owns the stealth. None when `id` has no StealthUpdate.
    pub(crate) fn stealth_owner_rules(&self, id: ObjectId) -> Option<StealthOwnerRules> {
        let own = *self.objects.get(&id)?.stealth_update_data()?;
        let owner = self.stealth_owner_id(id, &own);
        let rules = if owner == id {
            own
        } else {
            self.objects
                .get(&owner)
                .and_then(|rider| rider.stealth_update_data().copied())
                .unwrap_or(own)
        };
        Some(StealthOwnerRules {
            owner,
            flags: rules.forbidden_conditions,
            stealth_delay: rules.stealth_delay_frames,
            order_idle_enemies_to_attack: rules.order_idle_enemies_to_attack_me_upon_reveal,
        })
    }

    /// C++ `StealthUpdate::markAsDetected(numFrames)` (StealthUpdate.cpp:846-912):
    /// detection timer, disguise removal, then the idle-enemy wake walk.
    pub(crate) fn mark_stealth_detected(&mut self, id: ObjectId, num_frames: u32) {
        let now = self.frame;
        let Some(rules) = self.stealth_owner_rules(id) else {
            return;
        };
        if let Some(obj) = self.objects.get_mut(&id) {
            obj.mark_as_detected_for(now, num_frames, rules.stealth_delay);
        }
        self.order_idle_enemies_to_attack_on_reveal(id);
    }

    /// Run every StealthUpdate module once (C++ per-object update).
    pub(super) fn update_stealth_update_modules(&mut self, now: u32) {
        let ids: Vec<ObjectId> = self
            .objects
            .registered()
            .filter(|(_, obj)| obj.is_alive() && obj.stealth_update_data().is_some())
            .map(|(id, _)| *id)
            .collect();
        for id in ids {
            self.update_stealth_update_module(id, now);
        }
    }

    fn update_stealth_update_module(&mut self, id: ObjectId, now: u32) {
        use crate::game_logic::host_radar_stealth_vision_residual::is_mine_stealth_kind_residual;
        let Some(obj) = self.objects.get(&id) else {
            return;
        };
        let Some(data) = obj.stealth_update_data().copied() else {
            return;
        };
        if !obj.stealth_or_detector_update_processes() || !obj.stealth_update_awake {
            return;
        }
        if !obj.stealth_update_enabled {
            if let Some(obj) = self.objects.get_mut(&id) {
                obj.stealth_update_awake = false;
            }
            return;
        }
        // DisguisesAsTeam modules are enabled and woken by disguiseAsObject.
        let disguise_enabled =
            data.disguises_as_team && obj.stealth_update_enabled && obj.stealth_update_awake;
        let runs = !data.disguises_as_team && obj.stealth_update_runs(&data);
        if !(runs || disguise_enabled) {
            return;
        }
        // Mines keep the MinefieldBehavior-owned innate cloak.
        if is_mine_stealth_kind_residual(
            obj.is_kind_of(KindOf::Mine),
            obj.is_kind_of(KindOf::DemoTrap),
            obj.mine_data.is_some(),
        ) {
            return;
        }
        let Some(rules) = self.stealth_owner_rules(id) else {
            return;
        };
        // Drawable opacity pulse precedes the reveal and cloak checks
        // (StealthUpdate.cpp:622-672).
        if runs {
            if let Some(obj) = self.objects.get_mut(&id) {
                obj.apply_stealth_update_pulse(&data);
            }
        }
        let Some(obj) = self.objects.get(&id) else {
            return;
        };

        // RevealDistanceFromTarget (StealthUpdate.cpp:675-693).
        if data.reveal_distance_from_target > 0.0 {
            let close = obj
                .target
                .and_then(|victim| self.objects.get(&victim))
                .is_some_and(|victim| {
                    Object::stealth_detector_distance_2d(obj.get_position(), victim.get_position())
                        <= data.reveal_distance_from_target
                });
            if close {
                self.mark_stealth_detected(id, 0);
                return;
            }
        }
        if !runs {
            return;
        }
        // Temporary grants count awake updates, not elapsed world frames.
        let revoke = self.objects.get_mut(&id).is_some_and(|obj| {
            if obj.stealth_frames_granted == 0 {
                return false;
            }
            obj.stealth_frames_granted -= 1;
            obj.stealth_frames_granted == 0
                || (obj.has_ai_update_interface()
                    && obj.last_command_source
                        == crate::game_logic::host_command_button_hunt::HUNT_CMD_FROM_PLAYER)
        });
        if revoke {
            self.receive_stealth_grant(id, false, 0);
        }


        let allowed = self.allowed_to_stealth_in_world(id, rules.owner, now);
        let Some(obj) = self.objects.get_mut(&id) else {
            return;
        };
        if allowed && obj.stealth_allowed_frame > now {
            return;
        }
        obj.apply_stealth_allowed(now, allowed, rules.stealth_delay);
        let detected = obj.detection_expires_frame > now;
        let changed = obj.status.detected != detected;
        obj.set_status_detected(detected);
        let container = if changed { obj.contained_by } else { None };
        if let Some(container) = container {
            self.recalc_garrisons_after_occupant_detect_change(&[container]);
        }
    }

    /// SpawnBehavior asks `allowedToStealth(child)`, not the STEALTHED bit.
    fn allowed_to_stealth_in_world(&mut self, id: ObjectId, owner: ObjectId, now: u32) -> bool {
        type C = StealthForbiddenConditions;
        let Some(obj) = self.objects.get(&id) else {
            return false;
        };
        let Some(data) = obj.stealth_update_data().copied() else {
            return false;
        };
        let flags = if owner == id {
            data.forbidden_conditions
        } else {
            self.objects.get(&owner)
                .and_then(|rider| rider.stealth_update_data())
                .map_or(data.forbidden_conditions, |rider_data| rider_data.forbidden_conditions)
        };
        if obj.is_kind_of(KindOf::SpawnsAreTheWeapons)
            && Self::supports_spawn_behavior_child_roster(obj)
        {
            let children = obj.spawn_behavior_children();
            let mut all_allowed = true;
            for (child_id, birth) in &children {
                let Some((parent, child)) = self.objects.get(&id).zip(self.objects.get(child_id)) else {
                    continue;
                };
                if !parent.spawn_behavior_child_birth_matches(*child_id, birth, child) {
                    continue;
                }
                if child.stealth_update_data().is_none()
                    || !self.allowed_to_stealth_in_world(*child_id, *child_id, now)
                {
                    all_allowed = false;
                    break;
                }
            }
            if !all_allowed {
                for (child_id, birth) in children {
                    let valid = self.objects.get(&id).zip(self.objects.get(&child_id))
                        .is_some_and(|(parent, child)| {
                            parent.spawn_behavior_child_birth_matches(child_id, &birth, child)
                                && child.stealth_update_data().is_some()
                        });
                    if valid {
                        self.mark_stealth_detected(child_id, 0);
                    }
                }
                return false;
            }
        }
        let Some(obj) = self.objects.get(&id) else {
            return false;
        };
        let owner_can_stealth = if owner == id {
            obj.innate_stealth
        } else {
            self.objects.get(&owner).is_some_and(|rider| rider.innate_stealth)
        };
        let contained_by_non_garrisonable = obj
            .contained_by
            .and_then(|container| self.objects.get(&container))
            .is_some_and(|container| {
                !(container.is_garrison_contain()
                    || container.thing().template.garrison_contain_max.is_some())
            });
        let riders_attacking = flags.intersects(C::RIDERS_ATTACKING)
            && obj.passengers_allowed_to_fire
            && obj.contained_unit_ids().iter().any(|rider| {
                self.objects.get(rider).is_some_and(|rider| {
                    rider.is_alive()
                        && (rider.status.attacking
                            || matches!(
                                rider.ai_state(),
                                AIState::Attacking | AIState::AttackMoving | AIState::AttackingGround
                            ))
                })
            });
        // NO_BLACK_MARKET (StealthUpdate.cpp:280-292) is reached only after
        // the ATTACKING / USING_ABILITY returns, and only when due; the timer
        // advance consumes one logic random value.
        let black_market_due = flags.intersects(C::NO_BLACK_MARKET)
            && !(flags.intersects(C::ATTACKING) && obj.status.is_firing_weapon)
            && !(flags.intersects(C::USING_ABILITY) && obj.status.using_ability)
            && obj.stealth_next_black_market_check_frame < now;
        let controller = obj.owner_player_id;
        let black_market_missing = black_market_due && {
            !self.objects.registered_values().any(|market| {
                market.owner_player_id == controller
                    && market.is_kind_of(KindOf::FSBlackMarket)
                    && market.is_alive()
                    && !market.status.under_construction
                    && !market.status.sold
            })
        };
        if black_market_due {
            let jitter = u32::try_from(self.logic_random.next_int(0, 10)).unwrap_or(0);
            if let Some(obj) = self.objects.get_mut(&id) {
                obj.stealth_next_black_market_check_frame = obj
                    .stealth_next_black_market_check_frame
                    .wrapping_add(data.black_market_check_frames)
                    .wrapping_add(jitter);
            }
        }
        let facts = StealthWorldFacts {
            flags,
            owner_can_stealth,
            black_market_missing,
            contained_by_non_garrisonable,
            riders_attacking,
        };
        self.objects.get(&id)
            .is_some_and(|obj| obj.allowed_to_stealth(&data, facts, now))
    }
}

#[cfg(test)]
mod grant_tests {
    use crate::game_logic::{GameLogic, Player, Team};
    use glam::Vec3;

    #[test]
    fn rider_change_grants_and_revokes_the_actual_rider() {
        let mut world = GameLogic::new();
        world.admit_object_ini_sources([("RiderGrant.ini".into(),
            "Object GrantBike\n KindOf = VEHICLE SELECTABLE\n Body = ActiveBody Body\n MaxHealth = 100\n InitialHealth = 100\n End\n Behavior = RiderChangeContain Rider\n Slots = 1\n End\n Behavior = StealthUpdate Cloak\n StealthDelay = 1000\n InnateStealth = No\n GrantedBySpecialPower = Yes\n End\nEnd\nObject GrantRider\n KindOf = INFANTRY SELECTABLE\n Body = ActiveBody Body\n MaxHealth = 100\n InitialHealth = 100\n End\n Behavior = StealthUpdate Cloak\n StealthDelay = 1000\n InnateStealth = No\n GrantedBySpecialPower = Yes\n End\nEnd\nObject GrantTransport\n KindOf = VEHICLE SELECTABLE\n Body = ActiveBody Body\n MaxHealth = 100\n InitialHealth = 100\n End\n Behavior = TransportContain Transport\n Slots = 1\n End\n Behavior = StealthUpdate Cloak\n StealthDelay = 1000\n InnateStealth = No\n GrantedBySpecialPower = Yes\n End\nEnd\n".into())]).unwrap();
        world.add_player(Player::new(1, Team::GLA, "Owner", true));
        let bike = world.create_object_for_player("GrantBike", 1, Vec3::ZERO).unwrap();
        let rider = world.create_object_for_player("GrantRider", 1, Vec3::ZERO).unwrap();
        let transport = world.create_object_for_player("GrantTransport", 1, Vec3::ZERO).unwrap();
        let passenger = world.create_object_for_player("GrantRider", 1, Vec3::ZERO).unwrap();
        world.host_object_mut(bike).unwrap().occupants.push(rider);
        world.host_object_mut(transport).unwrap().occupants.push(passenger);

        world.receive_stealth_grant(bike, true, 7);
        world.receive_stealth_grant(transport, true, 7);
        for id in [bike, rider, transport] {
            let object = world.host_object(id).unwrap();
            assert!(object.innate_stealth && object.status.stealthed);
            assert_eq!(object.stealth_frames_granted, 7);
        }
        assert!(!world.host_object(passenger).unwrap().innate_stealth);

        world.receive_stealth_grant(bike, false, 0);
        for id in [bike, rider] {
            let object = world.host_object(id).unwrap();
            assert!(!object.innate_stealth && !object.status.stealthed);
            assert_eq!(object.stealth_frames_granted, 0);
        }
        assert!(world.host_object(transport).unwrap().innate_stealth);

        world.receive_stealth_grant(bike, true, 1);
        world.update_stealth_and_detection();
        for id in [bike, rider] {
            let object = world.host_object(id).unwrap();
            assert!(!object.innate_stealth && !object.status.stealthed);
            assert_eq!(object.stealth_frames_granted, 0);
        }
    }
}
