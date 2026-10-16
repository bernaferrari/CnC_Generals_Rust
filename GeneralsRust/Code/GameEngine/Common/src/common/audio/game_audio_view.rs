// Receiving audio view observation, listener arithmetic and update phase.
// Included in game_audio so the original owner and private state stay singular.

/// Immutable receiving-view facts in original C++ Z-up coordinates.
/// Supplying an observation never advances or republishes the camera.
#[derive(Clone, Copy, Debug)]
pub struct AudioViewObservation {
    pub ground_position: Coord3D,
    pub heading: Real,
    pub camera_position: Coord3D,
    pub ground_height: Real,
}

fn legacy_audio_visibility(position: &Coord3D) -> bool {
    AUDIO_VIEW_RESOLVER
        .get()
        .map(|resolver| resolver.is_world_position_on_screen(position))
        .unwrap_or(true)
}
fn playback_visibility(event: &AudioEventRts, visible: &mut dyn FnMut(&Coord3D) -> bool) -> bool {
    if event.is_positional_audio()
        && event
            .get_audio_event_info()
            .is_some_and(|info| info.low_pass_freq > 0.)
    {
        visible(&miles_event_world_position(event))
    } else {
        true
    }
}

impl AudioManager {
    pub fn update(&mut self) {
        // C++ MilesAudioManager::update (MilesAudioManager.cpp:460-468):
        // AudioManager::update (listener/zoom) → setDeviceListenerPosition →
        // processRequestList → processPlayingList → processFadingList → processStoppedList.
        self.update_listener_from_view();
        self.update_after_listener(&mut legacy_audio_visibility);
    }

    /// Native owner supplies its completed camera and receiving terrain sample.
    pub fn update_with_view(
        &mut self,
        observation: AudioViewObservation,
        visible: &mut dyn FnMut(&Coord3D) -> bool,
    ) {
        self.update_listener_with_view(observation);
        self.update_after_listener(visible);
    }

    fn update_after_listener(&mut self, visible: &mut dyn FnMut(&Coord3D) -> bool) {
        self.process_request_list_with_visibility(visible);
        self.process_playing_list(visible);
        self.process_fading_list();
        if let Some(sound_mgr) = &mut self.sound_manager {
            sound_mgr.update();
        }
        self.purge_inactive_events();
    }

    fn update_listener_from_view(&mut self) {
        let Some(resolver) = AUDIO_VIEW_RESOLVER.get() else {
            return;
        };

        let ground_pos = resolver.get_tactical_view_position();
        let angle = resolver.get_tactical_view_angle();
        let camera_pos = resolver.get_3d_camera_position();
        let ground_height = resolver.get_ground_height(ground_pos.x, ground_pos.y);
        self.update_listener_with_view(AudioViewObservation {
            ground_position: ground_pos,
            heading: angle,
            camera_position: camera_pos,
            ground_height,
        });
    }

    fn update_listener_with_view(&mut self, observation: AudioViewObservation) {
        let AudioViewObservation {
            ground_position: ground_pos,
            heading: angle,
            camera_position: camera_pos,
            ground_height,
        } = observation;
        let look_to = Coord3D {
            x: -angle.sin(),
            y: angle.cos(),
            z: 0.0,
        };

        let desired_height = self.audio_settings.microphone_desired_height_above_terrain;
        let max_percentage = self
            .audio_settings
            .microphone_max_percentage_between_ground_and_camera;

        let mut ground_to_camera = Coord3D {
            x: camera_pos.x - ground_pos.x,
            y: camera_pos.y - ground_pos.y,
            z: camera_pos.z - ground_pos.z,
        };

        let best_scale_factor = if camera_pos.z <= desired_height || ground_to_camera.z <= 0.0 {
            max_percentage
        } else {
            let z_scale = desired_height / ground_to_camera.z;
            max_percentage.min(z_scale)
        };

        ground_to_camera.x *= best_scale_factor;
        ground_to_camera.y *= best_scale_factor;
        ground_to_camera.z *= best_scale_factor;

        let mut microphone_pos = Coord3D {
            x: ground_pos.x,
            y: ground_pos.y,
            z: ground_height,
        };
        microphone_pos.x += ground_to_camera.x;
        microphone_pos.y += ground_to_camera.y;
        microphone_pos.z += ground_to_camera.z;

        self.set_listener_position(&microphone_pos, &look_to);

        let max_boost_scalar = self.audio_settings.zoom_sound_volume_percentage_amount;
        let min_dist = self.audio_settings.zoom_min_distance;
        let max_dist = self.audio_settings.zoom_max_distance;

        self.zoom_volume = 1.0 - max_boost_scalar;

        if max_boost_scalar > 0.0 {
            let dx = camera_pos.x - microphone_pos.x;
            let dy = camera_pos.y - microphone_pos.y;
            let dz = camera_pos.z - microphone_pos.z;
            let dist = (dx * dx + dy * dy + dz * dz).sqrt();

            if dist < min_dist {
                self.zoom_volume = 1.0;
            } else if dist < max_dist {
                let scalar = (dist - min_dist) / (max_dist - min_dist);
                self.zoom_volume = 1.0 - scalar * max_boost_scalar;
            }
        }

        self.set_3d_volume_adjustment(self.zoom_volume);
    }
}
