//! Original end-of-frame disabled expiry and its synchronous owner effects.
use super::super::*;

impl GameLogic {
    pub(in super::super) fn expire_owned_disabled_statuses(&mut self) {
        // Optional coupled experiments retain their explicit timer phase.
        if crate::gameworld_shadow::gameworld_shadow_enabled()
            && crate::gameworld_shadow::shadow_coupled_tick_active()
        {
            return;
        }
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
            object.tick_disabled_hacked(self.frame);
            object.tick_disabled_emp(self.frame);
            object.tick_disabled_paralyzed(self.frame);
            let reenabled = !object.is_disabled();

            // Admit this driving world's Object-local callbacks before the
            // frame ends. Foreign worlds retain their own pending edges.
            self.admit_disabled_expiry_radar_edges(id);
            // C++ Object.cpp:3826-3855 adjusts only positive producers. A
            // disabled consumer keeps its consumption to prevent oscillation.
            if reenabled && restores_generation {
                if let Some(player_id) = owner {
                    self.refresh_power_after_disabled_expiry(player_id);
                    self.admit_expiry_power_state(player_id);
                }
            }
        }
    }

    fn admit_disabled_expiry_radar_edges(&mut self, id: ObjectId) {
        let edges = self
            .objects
            .get_mut(&id)
            .map(Object::take_radar_disabled_edges)
            .unwrap_or_default();
        for edge in edges {
            let Some(player_id) = edge.player_id else {
                continue;
            };
            let Some(player) = self.players.get_mut(&player_id) else {
                continue;
            };
            let had = player.has_radar();
            if edge.becoming_disabled {
                player.remove_radar(edge.disable_proof);
            } else {
                player.add_radar(edge.disable_proof);
            }
            let count = player.radar_count.max(0) as u32;
            let has = player.has_radar();
            self.record_disabled_expiry_radar_transition(count, had, has);
        }
    }

    fn admit_expiry_power_state(&mut self, player_id: u32) {
        let Some(player) = self.players.get_mut(&player_id) else {
            return;
        };
        let had = player.has_radar();
        let sabotaged =
            player.power_sabotaged_till_frame > 0 && self.frame < player.power_sabotaged_till_frame;
        let brownout = player.power_available < 0 || sabotaged;
        if brownout {
            player.disable_radar();
        } else {
            player.enable_radar();
        }
        let count = player.radar_count.max(0) as u32;
        let has = player.has_radar();
        self.record_disabled_expiry_radar_transition(count, had, has);

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
            object.status.disabled_underpowered = disabled;
            if disabled && !was && !already_power {
                object.queue_power_disable_misc_audio(true);
            } else if !disabled && was && !object.is_power_style_disabled() {
                object.queue_power_disable_misc_audio(false);
            }
            if was_disabled != object.is_disabled() {
                object.on_disabled_edge(object.is_disabled());
            }
            self.admit_disabled_expiry_radar_edges(id);
        }
    }

    fn record_disabled_expiry_radar_transition(&mut self, count: u32, had: bool, has: bool) {
        let (online, offline) = self.host_radar.record_player_radar(count, had, has);
        let sound = if online {
            Some(crate::game_logic::host_radar::RADAR_ONLINE_AUDIO)
        } else if offline {
            Some(crate::game_logic::host_radar::RADAR_OFFLINE_AUDIO)
        } else {
            None
        };
        if let Some(sound) = sound {
            self.queue_audio_event(AudioEventRequest::new(sound).with_priority(130));
        }
    }
}
