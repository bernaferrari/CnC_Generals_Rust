use super::*;

impl AudioManager {
    pub fn should_play_locally(&self, audio_event: &AudioEventRts) -> Bool {
        self.should_play_locally_for_player(audio_event, None)
    }

    pub(super) fn should_play_locally_for_player(
        &self,
        audio_event: &AudioEventRts,
        owner_player_index: Option<Int>,
    ) -> Bool {
        let Some(event_info) = audio_event.get_audio_event_info() else {
            return false;
        };

        if event_info.sound_type == AudioType::Music {
            return true;
        }

        let player_restriction_mask = ST_PLAYER | ST_ALLIES | ST_ENEMIES | ST_EVERYONE;
        if (event_info.type_field & player_restriction_mask) == 0 {
            return true;
        }

        if (event_info.type_field & ST_EVERYONE) != 0 {
            return true;
        }

        // Live host registers a resolver; leftover PlayerList is not the player path.
        with_audio_locality_resolver(|resolver| {
            self.should_play_locally_with_resolver(
                audio_event,
                &event_info,
                resolver,
                owner_player_index,
            )
        })
        .unwrap_or(true)
    }

    pub(super) fn should_play_locally_with_resolver(
        &self,
        audio_event: &AudioEventRts,
        event_info: &AudioEventInfo,
        resolver: &dyn AudioLocalityResolver,
        owner_player_index: Option<Int>,
    ) -> Bool {
        let owning_player_index =
            owner_player_index.unwrap_or_else(|| audio_event.get_player_index());
        AudioSubmissionLocality::from_resolver(owning_player_index, resolver)
            .should_play_locally(event_info)
    }

    // C++ parity methods used by SoundManager for audio culling

    /// C++ `MilesAudioManager::doesViolateLimit` (MilesAudioManager.cpp:1802-1882).
    /// At limit, records the oldest same-name playing handle on the event.
    /// `AC_INTERRUPT` compares request-list count vs playing count.
    pub fn does_violate_limit(&self, event: &mut AudioEventRts) -> Bool {
        let Some(event_info) = event.get_audio_event_info() else {
            return false;
        };

        if event_info.limit == 0 {
            return false;
        }
        let limit = event_info.limit;
        let interrupting = (event_info.control & AC_INTERRUPT) != 0;
        let event_name = event.get_event_name();
        let positional = event.is_positional_audio();
        // Miles counts list membership until processPlayingList removes a
        // completed sample, rather than querying the device during admission.
        let mut oldest = None;
        let mut total_playing_count = 0;
        for playing in self.active_audio_events.values() {
            if Self::is_playing_sample(playing)
                && playing.get_event_name() == event_name
                && playing.is_positional_audio() == positional
            {
                oldest.get_or_insert(playing.get_playing_handle());
                total_playing_count += 1;
            }
        }

        let mut total_request_count = 0;
        for request in &self.audio_requests {
            if !request.use_pending_event {
                continue;
            }
            if request
                .get_pending_event()
                .is_some_and(|pending| pending.get_event_name() == event_name)
            {
                total_request_count += 1;
                total_playing_count += 1;
            }
        }
        if let Some(oldest) = oldest {
            event.set_handle_to_kill(oldest);
        }

        if interrupting {
            if total_request_count < limit {
                if total_request_count + (total_playing_count - total_request_count) < limit {
                    event.set_handle_to_kill(0);
                    return false;
                }
                // Exceeding via playing sounds: keep kill handle, still allow the request.
                return false;
            }
        }

        if total_playing_count < limit {
            event.set_handle_to_kill(0);
            return false;
        }

        true
    }


    /// C++ `MilesAudioManager::isPlayingLowerPriority` (1993-2024).
    /// Uses INI `AudioEventInfo.m_priority`, not the per-event field (always Normal).
    pub fn is_playing_lower_priority(&self, event: &AudioEventRts) -> Bool {
        let event_priority = Self::sample_event_priority(event);
        if event_priority == AudioPriority::Lowest {
            return false;
        }
        let positional = event.is_positional_audio();

        let playing_is_lower = |playing: &AudioEventRts| {
            if !Self::is_playing_sample(playing) || playing.is_positional_audio() != positional {
                return false;
            }
            Self::sample_event_priority(playing) < event_priority
        };

        self.playback_recipient
            .with_hook(|hook| {
                self.active_audio_events.values().any(|playing| {
                    hook.is_playing(playing.get_playing_handle()) && playing_is_lower(playing)
                })
            })
            .unwrap_or_else(|| self.active_audio_events.values().any(playing_is_lower))
    }

    pub(super) fn sample_event_priority(event: &AudioEventRts) -> AudioPriority {
        event
            .get_audio_event_info()
            .map(|info| info.priority)
            .unwrap_or_else(|| event.get_audio_priority())
    }

    pub(super) fn is_playing_sample(event: &AudioEventRts) -> bool {
        event
            .get_audio_event_info()
            .is_some_and(|info| info.sound_type == AudioType::SoundEffect)
    }

    pub(super) fn sample_slot_available(&self, event: &AudioEventRts) -> bool {
        let positional = event.is_positional_audio();
        let cap = if positional {
            self.get_num_3d_samples()
        } else {
            self.get_num_2d_samples()
        }
        .max(0) as usize;
        let used = self
            .active_audio_events
            .values()
            .filter(|playing| {
                Self::is_playing_sample(playing) && playing.is_positional_audio() == positional
            })
            .count();
        used < cap
    }

    /// C++ `MilesAudioManager::findLowestPrioritySound` (1935-1990).
    pub(super) fn find_lowest_priority_sound(&self, event: &AudioEventRts) -> Option<AudioHandle> {
        let priority = Self::sample_event_priority(event);
        if priority == AudioPriority::Lowest {
            return None;
        }
        let positional = event.is_positional_audio();
        let mut lowest: Option<(AudioPriority, AudioHandle)> = None;
        for playing in self.active_audio_events.values() {
            if !Self::is_playing_sample(playing) || playing.is_positional_audio() != positional {
                continue;
            }
            let playing_priority = Self::sample_event_priority(playing);
            if playing_priority >= priority {
                continue;
            }
            if lowest.is_none_or(|(current, _)| playing_priority < current) {
                lowest = Some((playing_priority, playing.get_playing_handle()));
                if playing_priority == AudioPriority::Lowest {
                    break;
                }
            }
        }
        lowest.map(|(_, handle)| handle)
    }

    /// C++ `MilesAudioManager::killLowestPrioritySoundImmediately` (2027-2074).
    pub(super) fn kill_lowest_priority_sound_immediately(&mut self, event: &AudioEventRts) -> bool {
        let Some(handle) = self.find_lowest_priority_sound(event) else {
            return false;
        };
        self.release_playing_handle(handle);
        true
    }

    /// C++ `MilesAudioManager::isPlayingAlready` (1886-1905).
    /// 2D events scan 2D samples only; 3D events scan 3D samples only.
    pub fn is_playing_already(&self, event: &AudioEventRts) -> Bool {
        let event_name = event.get_event_name();
        let positional = event.is_positional_audio();

        self.active_audio_events.values().any(|playing| {
            Self::is_playing_sample(playing)
                && playing.get_event_name() == event_name
                && playing.is_positional_audio() == positional
        })
    }

    /// Check if a specific object is currently playing a voice sound.
    /// Used to prevent multiple voice sounds from the same object.
    pub fn is_object_playing_voice(&self, object_id: ObjectId) -> Bool {
        const ST_VOICE: u32 = 0x00000010;

        self.playback_recipient
            .with_hook(|hook| {
                self.active_audio_events
                    .values()
                    .filter_map(|e| {
                        let is_voice = e
                            .get_audio_event_info()
                            .map(|info| (info.type_field & ST_VOICE) != 0)
                            .unwrap_or(false);
                        if is_voice
                            && e.get_object_id() == object_id
                            && hook.is_playing(e.get_playing_handle())
                        {
                            Some(())
                        } else {
                            None
                        }
                    })
                    .next()
                    .is_some()
            })
            .unwrap_or_else(|| {
                self.active_audio_events.values().any(|e| {
                    let is_voice = e
                        .get_audio_event_info()
                        .map(|info| (info.type_field & ST_VOICE) != 0)
                        .unwrap_or(false);
                    is_voice && e.get_object_id() == object_id
                })
            })
    }
}
