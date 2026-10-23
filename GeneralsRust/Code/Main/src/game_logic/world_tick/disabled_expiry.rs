//! Original end-of-frame disabled expiry and its synchronous owner effects.
use super::super::*;

impl GameLogic {
    pub(in super::super) fn expire_owned_disabled_statuses(&mut self) {
        self.expire_owned_disabled_statuses_with_effects(&mut Default::default())
    }

    pub(in super::super) fn expire_owned_disabled_statuses_with_effects(
        &mut self,
        effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) {
        // Optional coupled experiments retain their explicit timer phase.
        {}
        // C++ GameLogic.cpp:3783-3792 walks the current live object list after
        // destruction and victory. Release each Object borrow before admitting
        // its Player callbacks; no object is temporarily removed from its store.
        let ids: Vec<_> = self.objects.keys().copied().collect();
        for id in ids {
            let Some(object) = self.objects.get(&id) else {
                continue;
            };
            let was_disabled = object.is_disabled();
            if !was_disabled {
                continue;
            }
            let owner = self.player_owner_for_host_object(object);
            let restores_generation = object.power_provided > 0;
            let object = self.objects.get_mut(&id).unwrap();
            object.tick_disabled_hacked(self.frame, &mut self.health_events);
            object.tick_disabled_emp(self.frame, &mut self.health_events);
            object.tick_disabled_paralyzed(self.frame);
            object.tick_remaining_disabled(self.frame);
            let reenabled = !object.is_disabled();

            // Admit this driving world's Object-local callbacks before the
            // frame ends. Foreign worlds retain their own pending edges.
            self.admit_disabled_expiry_radar_edges_with_effects(id, effects);
            // C++ Object.cpp:3826-3855 adjusts only positive producers. A
            // disabled consumer keeps its consumption to prevent oscillation.
            if reenabled && restores_generation {
                if let Some(player_id) = owner {
                    self.refresh_power_after_disabled_expiry(player_id);
                    self.admit_expiry_power_state_with_effects(player_id, effects);
                }
            }
        }
    }

    fn admit_disabled_expiry_radar_edges(&mut self, id: ObjectId) {
        self.admit_disabled_expiry_radar_edges_with_effects(id, &mut Default::default())
    }

    fn admit_disabled_expiry_radar_edges_with_effects(
        &mut self,
        id: ObjectId,
        effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) {
        self.admit_host_radar_provider_with_effects(id, effects);
    }

    fn admit_expiry_power_state(&mut self, player_id: u32) {
        self.admit_expiry_power_state_with_effects(player_id, &mut Default::default())
    }

    fn admit_expiry_power_state_with_effects(
        &mut self,
        player_id: u32,
        effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) {
        let Some(player) = self.players.get_mut(&player_id) else {
            return;
        };
        let had = player.has_radar();
        let brownout = !player.has_sufficient_energy_at(self.frame);
        if brownout {
            player.disable_radar();
        } else {
            player.enable_radar();
        }
        let count = player.radar_count.max(0) as u32;
        let has = player.has_radar();
        self.record_host_radar_transition(player_id, count, had, has);

        // Player.cpp:3232-3241 changes only this owner's KINDOF_POWERED objects.
        let powered: Vec<_> = self
            .objects
            .values()
            .filter(|object| {
                object.is_kind_of(KindOf::Powered)
                    && self.player_owner_for_host_object(object) == Some(player_id)
            })
            .map(|object| object.id)
            .collect();
        for id in powered {
            let object = self.objects.get_mut(&id).unwrap();
            let disabled = brownout && object.is_alive() && object.is_constructed();
            let was = object.status.disabled_underpowered;
            let was_disabled = object.is_disabled();
            let already_power = object.is_power_style_disabled();
            if disabled && !was && !already_power {
                object.queue_power_disable_misc_audio(true, &mut self.health_events);
            } else if !disabled
                && was
                && !(object.status.disabled_emp
                    || object.status.disabled_subdued
                    || object.status.disabled_hacked)
            {
                object.queue_power_disable_misc_audio(false, &mut self.health_events);
            }
            object.status.disabled_underpowered = disabled;
            if was_disabled != object.is_disabled() {
                object.on_disabled_edge(object.is_disabled());
            }
            self.admit_disabled_expiry_radar_edges_with_effects(id, effects);
        }
    }
}

#[cfg(test)]
mod energy_observer_tests {
    use super::*;
    #[test]
    fn expiry_callback_uses_current_energy_not_stale_or_forged_projection() {
        let mut world = GameLogic::new();
        let mut player = crate::game_logic::Player::new(1, Team::USA, "ExpiryOwner", true);
        player.power_produced = 20;
        player.power_consumed = 10;
        player.power_sabotaged_till_frame = 50;
        player.power_available = -10;
        player.radar_count = 1;
        world.add_player(player);
        world.frame = 50;
        world.admit_expiry_power_state(1);
        assert!(world.players[&1].has_radar());
        world.frame = 49;
        world.players.get_mut(&1).unwrap().power_available = 999;
        world.admit_expiry_power_state(1);
        assert!(!world.players[&1].has_radar());
        assert_eq!(world.players[&1].power_produced, 20);
    }
}
