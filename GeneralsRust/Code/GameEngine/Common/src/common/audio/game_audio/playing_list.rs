use super::*;

impl AudioManager {
    /// C++ `MilesAudioManager::adjustVolumeOfPlayingAudio` (MilesAudioManager.cpp:2079-2116).
    /// Sets event volume then immediately re-applies `AIL_set_*_volume` as sink volume.
    pub fn adjust_volume_of_playing_audio(&mut self, event_name: &str, new_volume: Real) {
        for event in self.active_audio_events.values_mut() {
            if event.get_event_name() == event_name {
                event.set_volume(new_volume);
                let desired_volume = event.get_volume() * event.get_volume_shift();
                let handle = event.get_playing_handle();
                let _ = self.playback_recipient.with_hook(|hook| {
                    hook.set_sink_volume(handle, desired_volume);
                });
            }
        }
    }

    /// Bus changes become visible at the next original audio service phase.
    /// Only admitted receipts of this session receive the new script factors.
    pub fn refresh_owned_script_volumes(
        &mut self,
        receipts: &[AudioHandle],
        volumes: super::super::script_policy::ScriptAudioVolumes,
    ) {
        let refresh = |event: &mut AudioEventRts| {
            if receipts.contains(&event.get_playing_handle())
                && event.owned_script_volumes.is_some()
                && event.owned_script_volumes != Some(volumes)
            {
                event.owned_script_volumes = Some(volumes);
                event.owned_mix_dirty = true;
            }
        };
        for event in self.active_audio_events.values_mut() {
            refresh(event);
        }
        for fade in &mut self.fading_audio {
            refresh(&mut fade.event);
        }
        for event in &mut self.audio_force_played {
            refresh(event);
        }
        for request in &mut self.audio_requests {
            if let Some(event) = request.get_pending_event_mut() {
                refresh(event);
            }
        }
    }

    /// Advance attached object sounds from one receiving world's immutable
    /// spatial snapshot. Handles are the receiving session's admission receipts;
    /// same numeric object IDs in another world cannot update these events.
    pub fn refresh_receiving_object_audio_positions(
        &mut self,
        receipts: &[AudioHandle],
        objects: &[(ObjectId, Weak<()>, Coord3D, Int)],
    ) {
        let refresh = |event: &mut AudioEventRts| {
            if !receipts.contains(&event.get_playing_handle())
                || event.owner_type != super::super::audio_event_rts::OwnerType::Object
            {
                return;
            }
            let Some(identity) = event.receiving_object_identity.as_ref() else {
                return;
            };
            if let Some((_, _, position, player_index)) =
                objects.iter().find(|(id, candidate, _, _)| {
                    *id == event.object_id && Weak::ptr_eq(identity, candidate)
                })
            {
                event.position_of_audio = *position;
                event.player_index = *player_index;
            } else {
                // The admitted lifetime left this world or its ID was reused.
                // Preserve the last observed point, as C++ attached audio does.
                event.owner_type = super::super::audio_event_rts::OwnerType::Dead;
            }
        };
        for event in self.active_audio_events.values_mut() {
            refresh(event);
        }
        for fade in &mut self.fading_audio {
            refresh(&mut fade.event);
        }
        for event in &mut self.audio_force_played {
            refresh(event);
        }
        for request in &mut self.audio_requests {
            if let Some(event) = request.get_pending_event_mut() {
                refresh(event);
            }
        }
    }
    /// C++ adjustVolumeOfPlayingAudio changes live raw sample volume immediately.
    /// Restore(-1)/clear-table callers intentionally do not invoke this operation.
    pub fn adjust_owned_playing_volume(
        &mut self,
        receipts: &[AudioHandle],
        name: &str,
        volume: Real,
    ) {
        for event in self.active_audio_events.values_mut() {
            if receipts.contains(&event.get_playing_handle()) && event.get_event_name() == name {
                event.set_volume(volume);
                let handle = event.get_playing_handle();
                let desired = event.get_volume() * event.get_volume_shift();
                let _ = self
                    .playback_recipient
                    .with_hook(|hook| hook.set_sink_volume(handle, desired));
            }
        }
    }
    pub fn pause_owned_audio(&mut self, receipts: &[AudioHandle], which: AudioAffect) {
        let handles: Vec<_> = self
            .active_audio_events
            .values()
            .filter(|event| {
                receipts.contains(&event.get_playing_handle())
                    && event_matches_audio_affect(event, which)
            })
            .map(AudioEventRts::get_playing_handle)
            .collect();
        let _ = self.playback_recipient.with_hook(|hook| {
            for handle in handles {
                hook.pause(handle);
            }
        });
        self.audio_requests.retain(|request| {
            request.request != RequestType::Play
                || !request
                    .get_pending_event()
                    .is_some_and(|event| receipts.contains(&event.get_playing_handle()))
        });
    }
    pub fn resume_owned_audio(&mut self, receipts: &[AudioHandle], which: AudioAffect) {
        let handles: Vec<_> = self
            .active_audio_events
            .values()
            .filter(|event| {
                receipts.contains(&event.get_playing_handle())
                    && event_matches_audio_affect(event, which)
            })
            .map(AudioEventRts::get_playing_handle)
            .collect();
        let _ = self.playback_recipient.with_hook(|hook| {
            for handle in handles {
                hook.resume(handle);
            }
        });
    }
    pub fn remove_owned_disabled_audio(&mut self, receipts: &[AudioHandle]) {
        let handles: Vec<_> = self
            .active_audio_events
            .values()
            .filter(|event| {
                receipts.contains(&event.get_playing_handle()) && event.get_volume() == 0.
            })
            .map(AudioEventRts::get_playing_handle)
            .collect();
        for handle in handles {
            self.release_playing_handle(handle);
        }
    }

    /// Same C++ removePlayingAudio name operation, limited to receipts admitted
    /// by the explicitly receiving owner. Source MilesAudioManager releases
    /// currently playing samples synchronously; queued/fading/force records are
    /// outside this named operation, unlike handle-based AR_Stop admission.
    pub fn remove_owned_playing_audio(&mut self, event_name: &str, receipts: &[AudioHandle]) {
        let handles: Vec<_> = self
            .active_audio_events
            .values()
            .filter(|event| {
                event.get_event_name() == event_name
                    && receipts.contains(&event.get_playing_handle())
            })
            .map(AudioEventRts::get_playing_handle)
            .collect();
        for handle in handles {
            self.release_playing_handle(handle);
        }
    }

    /// Remove all playing audio events matching the given name
    pub fn remove_playing_audio(&mut self, event_name: &str) {
        let handles_to_stop: Vec<AudioHandle> = self
            .active_audio_events
            .values()
            .filter_map(|e| {
                if e.get_event_name() == event_name {
                    Some(e.get_playing_handle())
                } else {
                    None
                }
            })
            .collect();

        for handle in handles_to_stop {
            self.remove_audio_event(handle);
        }
    }

    /// Remove all disabled audio events (volume = 0)
    pub fn remove_all_disabled_audio(&mut self) {
        let handles_to_stop: Vec<AudioHandle> = self
            .active_audio_events
            .values()
            .filter_map(|e| {
                if e.get_volume() <= 0.0 {
                    Some(e.get_playing_handle())
                } else {
                    None
                }
            })
            .collect();

        for handle in handles_to_stop {
            self.remove_audio_event(handle);
        }
    }

    /// C++ `MilesAudioManager::has3DSensitiveStreamsPlaying` (2381-2404).
    /// Only music/speech streams count. `Game_` music is not sensitive.
    pub fn has_3d_sensitive_streams_playing(&self) -> Bool {
        let is_sensitive = |event: &AudioEventRts| -> bool {
            let Some(info) = event.get_audio_event_info() else {
                return false;
            };
            match info.sound_type {
                AudioType::Streaming => true,
                AudioType::Music => !event.get_event_name().starts_with("Game_"),
                AudioType::SoundEffect => false,
            }
        };
        self.playback_recipient
            .with_hook(|hook| {
                self.active_audio_events
                    .values()
                    .any(|event| is_sensitive(event) && hook.is_playing(event.get_playing_handle()))
            })
            .unwrap_or_else(|| self.active_audio_events.values().any(is_sensitive))
    }

    pub fn stop_music_for_handles(&mut self, handles: &[AudioHandle], fade: bool) {
        if fade {
            self.fade_music_for_handles(handles);
            return;
        }
        let active: Vec<_> = self
            .active_audio_events
            .values()
            .filter(|event| {
                handles.contains(&event.get_playing_handle())
                    && event
                        .get_audio_event_info()
                        .is_some_and(|info| info.sound_type == AudioType::Music)
            })
            .map(AudioEventRts::get_playing_handle)
            .collect();
        for handle in active {
            self.release_playing_handle(handle);
        }
        let mut retained = Vec::with_capacity(self.fading_audio.len());
        for fade in std::mem::take(&mut self.fading_audio) {
            let handle = fade.event.get_playing_handle();
            if handles.contains(&handle) {
                let _ = self.playback_recipient.with_hook(|hook| hook.stop(handle));
            } else {
                retained.push(fade);
            }
        }
        self.fading_audio = retained;
    }
    pub fn fade_music_for_handles(&mut self, handles: &[AudioHandle]) {
        self.begin_music_fade_selected(Some(handles));
    }
    pub(super) fn begin_music_fade(&mut self) {
        self.begin_music_fade_selected(None);
    }
    pub(super) fn begin_music_fade_selected(&mut self, handles: Option<&[AudioHandle]>) {
        let music_events: Vec<AudioEventRts> = self
            .active_audio_events
            .values()
            .filter(|event| {
                handles.is_none_or(|handles| handles.contains(&event.get_playing_handle()))
                    && event
                        .get_audio_event_info()
                        .is_some_and(|info| info.sound_type == AudioType::Music)
            })
            .cloned()
            .collect();

        for event in music_events {
            let handle = event.get_playing_handle();
            if self
                .fading_audio
                .iter()
                .any(|fade| fade.event.get_playing_handle() == handle)
            {
                continue;
            }
            self.active_audio_events.shift_remove(&handle);
            self.fading_audio.push(FadingAudio {
                event,
                frames_faded: 0,
            });
        }
    }

    pub(super) fn release_fading_audio(&mut self) {
        let fading = std::mem::take(&mut self.fading_audio);
        for fade in fading {
            let handle = fade.event.get_playing_handle();
            let _ = self.playback_recipient.with_hook(|hook| hook.stop(handle));
        }
    }

    pub(super) fn adjust_playing_volume(&self, event: &AudioEventRts) {
        let volume = self.get_effective_volume(event);
        let _ = self.playback_recipient.with_hook(|hook| {
            hook.set_sink_volume(event.get_playing_handle(), volume);
        });
    }

    pub(super) fn process_playing_list(&mut self, visible: &mut dyn FnMut(&Coord3D) -> bool) {
        // C++ MilesAudioManager::processPlayingList (MilesAudioManager.cpp:2242-2368).
        let handles: Vec<AudioHandle> = self.active_audio_events.keys().copied().collect();
        let mut to_stop: Vec<AudioHandle> = Vec::new();
        let volume_changed = self.volume_has_changed;

        for handle in handles {
            let Some(mut event) = self.active_audio_events.shift_remove(&handle) else {
                continue;
            };

            if volume_changed || event.owned_mix_dirty {
                event.playback_sliders = Some(self.event_volume_sliders(&event));
                self.adjust_playing_volume(&event);
                event.owned_mix_dirty = false;
            }

            if event.is_positional_audio() {
                let has_pos = event.get_current_position().is_some();
                let is_dead = event.is_dead();
                if !has_pos {
                    self.notify_sample_completion_if_effect(&event);
                    to_stop.push(handle);
                    continue;
                } else if is_dead {
                    // C++ stopAudioEvent: requestStop, sample keeps playing for Decay.
                    event.set_request_stop(true);
                } else {
                    let vol_for_consideration = {
                        let effective = self.get_effective_volume(&event);
                        let sliders = self.event_volume_sliders(&event);
                        if sliders.sound_3d_volume > 0.0 {
                            // C++ keeps the raw divisor: 0/0 is NaN and skips the
                            // threshold comparison; tiny values must not be clamped.
                            effective / sliders.sound_volume
                        } else {
                            effective
                        }
                    };
                    let play_anyways = event.get_audio_event_info().is_some_and(|info| {
                        (info.type_field & ST_GLOBAL) != 0
                            || info.priority == AudioPriority::Critical
                    });
                    if vol_for_consideration < self.audio_settings.min_volume && !play_anyways {
                        self.notify_sample_completion_if_effect(&event);
                        to_stop.push(handle);
                        continue;
                    }
                    event.playback_sliders = Some(self.event_volume_sliders(&event));
                    let _ = self
                        .playback_recipient
                        .with_hook(|hook| hook.set_event_volume(&event));
                }
            }

            if !self
                .playback_recipient
                .with_hook(|hook| hook.is_playing(handle))
                .unwrap_or(true)
            {
                if self.notify_of_audio_completion_with_visibility(&mut event, visible) {
                    let still_playing = self
                        .playback_recipient
                        .with_hook(|hook| hook.is_playing(handle))
                        .unwrap_or(false);
                    if still_playing {
                        self.active_audio_events.insert(handle, event);
                        continue;
                    }
                    // Delayed loop queued a new request; this sample has ended.
                    self.notify_sample_completion_if_effect(&event);
                    continue;
                }
                self.notify_sample_completion_if_effect(&event);
                continue;
            }

            self.active_audio_events.insert(handle, event);
        }

        for handle in to_stop {
            let _ = self.playback_recipient.with_hook(|hook| hook.stop(handle));
        }

        if self.volume_has_changed {
            self.volume_has_changed = false;
        }
    }

    pub(super) fn process_fading_list(&mut self) {
        // C++ MilesAudioManager::processFadingList (MilesAudioManager.cpp:2410-2458).
        let fade_frames = self.audio_settings.fade_audio_frames.max(1);
        let mut i = 0;
        while i < self.fading_audio.len() {
            if self.fading_audio[i].frames_faded >= fade_frames {
                let handle = self.fading_audio[i].event.get_playing_handle();
                let _ = self.playback_recipient.with_hook(|hook| hook.stop(handle));
                self.fading_audio.remove(i);
                continue;
            }

            self.fading_audio[i].frames_faded += 1;
            let frames_faded = self.fading_audio[i].frames_faded;
            let mut volume = self.get_effective_volume(&self.fading_audio[i].event);
            volume *= 1.0 - (frames_faded as Real / fade_frames as Real);
            let handle = self.fading_audio[i].event.get_playing_handle();
            let _ = self
                .playback_recipient
                .with_hook(|hook| hook.set_sink_volume(handle, volume));
            i += 1;
        }
    }

    pub(super) fn notify_of_audio_completion(&mut self, event: &mut AudioEventRts) -> bool {
        self.notify_of_audio_completion_with_visibility(event, &mut legacy_audio_visibility)
    }
    pub(super) fn notify_of_audio_completion_with_visibility(
        &mut self,
        event: &mut AudioEventRts,
        visible: &mut dyn FnMut(&Coord3D) -> bool,
    ) -> bool {
        // C++ MilesAudioManager::notifyOfAudioCompletion (MilesAudioManager.cpp:1507-1566).
        if self.disallow_speech
            && event
                .get_audio_event_info()
                .is_some_and(|info| info.sound_type == AudioType::Streaming)
        {
            self.disallow_speech = false;
        }

        let is_loop = event
            .get_audio_event_info()
            .is_some_and(|info| (info.control & AC_LOOP) != 0);
        if is_loop {
            if event.get_next_play_portion() == PortionToPlay::Attack {
                event.set_next_play_portion(PortionToPlay::Sound);
            }
            if event.get_next_play_portion() == PortionToPlay::Sound {
                event.decrease_loop_count();
                if self.start_next_loop(event, visible) {
                    return true;
                }
            }
        }

        event.advance_next_play_portion();
        if event.get_next_play_portion() != PortionToPlay::Done
            && event
                .get_audio_event_info()
                .is_some_and(|info| info.sound_type != AudioType::Music)
            && self.replay_playing_event(event, visible)
        {
            return true;
        }

        if event
            .get_audio_event_info()
            .is_some_and(|info| info.sound_type == AudioType::Music)
            && self.replay_playing_event(event, visible)
        {
            return true;
        }

        false
    }

    pub(super) fn replay_playing_event(
        &self,
        event: &AudioEventRts,
        visible: &mut dyn FnMut(&Coord3D) -> bool,
    ) -> bool {
        let mut submitted = event.clone();
        submitted.playback_sliders = Some(self.event_volume_sliders(event));
        match self.playback_recipient.with_hook(|hook| {
            hook.play_with_visibility(&submitted, playback_visibility(&submitted, visible))
        }) {
            Some(Ok(())) => true,
            Some(Err(_)) => false,
            None => true,
        }
    }

    pub(super) fn start_next_loop(
        &mut self,
        event: &mut AudioEventRts,
        visible: &mut dyn FnMut(&Coord3D) -> bool,
    ) -> bool {
        // C++ MilesAudioManager::startNextLoop (MilesAudioManager.cpp:2719-2756).
        // Loop count is decreased by notifyOfAudioCompletion before this call.
        if event.get_request_stop() {
            return false;
        }
        if !event.has_more_loops() {
            return false;
        }

        event.generate_filename_with_settings(&self.audio_paths());
        if event.get_delay() > MSEC_PER_LOGICFRAME_REAL {
            let mut request = AudioRequest::new_with_event(RequestType::Play, event.clone());
            request.set_requires_check_for_sample(true);
            self.append_audio_request(request);
            return true;
        }

        self.replay_playing_event(event, visible)
    }
}
