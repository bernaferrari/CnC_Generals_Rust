// Route legacy ScriptEngine event entry points through the simulation owner.

impl ScriptEngine {
    pub fn script_events(&self) -> &ScriptEvents {
        self.runtime.events()
    }

    pub fn notify_of_completed_video(&self, video_name: &str) {
        self.runtime.events().record_video(video_name);
    }

    pub fn is_video_complete(&self, video_name: &str, remove: bool) -> bool {
        self.runtime.events().has_video(video_name, remove)
    }

    pub fn notify_of_triggered_special_power(&self, player: usize, name: &str, source: ObjectId) {
        self.runtime.events().record_player_event(
            PlayerEventKind::PowerTriggered,
            player,
            name,
            source,
        );
    }

    pub fn notify_of_midway_special_power(&self, player: usize, name: &str, source: ObjectId) {
        self.runtime.events()
            .record_player_event(PlayerEventKind::PowerMidway, player, name, source);
    }

    pub fn notify_of_completed_special_power(&self, player: usize, name: &str, source: ObjectId) {
        self.runtime.events().record_player_event(
            PlayerEventKind::PowerCompleted,
            player,
            name,
            source,
        );
    }

    pub fn notify_of_completed_upgrade(&self, player: usize, name: &str, source: ObjectId) {
        self.runtime.events().record_player_event(
            PlayerEventKind::UpgradeCompleted,
            player,
            name,
            source,
        );
    }

    pub fn is_special_power_triggered(
        &self,
        player: usize,
        name: &str,
        remove: bool,
        source: ObjectId,
    ) -> bool {
        self.has_player_event(
            PlayerEventKind::PowerTriggered,
            player,
            name,
            remove,
            source,
        )
    }

    pub fn is_special_power_midway(
        &self,
        player: usize,
        name: &str,
        remove: bool,
        source: ObjectId,
    ) -> bool {
        self.has_player_event(PlayerEventKind::PowerMidway, player, name, remove, source)
    }

    pub fn is_special_power_complete(
        &self,
        player: usize,
        name: &str,
        remove: bool,
        source: ObjectId,
    ) -> bool {
        self.has_player_event(
            PlayerEventKind::PowerCompleted,
            player,
            name,
            remove,
            source,
        )
    }

    pub fn is_upgrade_complete(
        &self,
        player: usize,
        name: &str,
        remove: bool,
        source: ObjectId,
    ) -> bool {
        self.has_player_event(
            PlayerEventKind::UpgradeCompleted,
            player,
            name,
            remove,
            source,
        )
    }

    fn has_player_event(
        &self,
        kind: PlayerEventKind,
        player: usize,
        name: &str,
        remove: bool,
        source: ObjectId,
    ) -> bool {
        let source = (source != INVALID_ID).then_some(source);
        self.runtime.events()
            .has_player_event(kind, player, name, source, remove)
    }

    pub fn notify_of_acquired_science(&self, player: usize, science: ScienceType) {
        self.runtime.events().record_science(player, science);
    }

    pub fn is_science_acquired(&self, player: usize, science: ScienceType, remove: bool) -> bool {
        self.runtime.events().has_science(player, science, remove)
    }

    /// Standalone adapters read the active frame. The host supplies its own below.
    pub fn is_speech_complete(&self, name: &str, remove: bool) -> bool {
        self.is_speech_complete_at_frame(name, remove, TheGameLogic::get_frame())
    }

    pub fn is_audio_complete(&self, name: &str, remove: bool) -> bool {
        self.is_audio_complete_at_frame(name, remove, TheGameLogic::get_frame())
    }

    pub fn is_speech_complete_at_frame(&self, name: &str, remove: bool, frame: u32) -> bool {
        self.is_timed_media_complete(TimedMediaKind::Speech, name, remove, frame)
    }

    pub fn is_audio_complete_at_frame(&self, name: &str, remove: bool, frame: u32) -> bool {
        self.is_timed_media_complete(TimedMediaKind::Audio, name, remove, frame)
    }

    fn is_timed_media_complete(
        &self,
        kind: TimedMediaKind,
        name: &str,
        remove: bool,
        frame: u32,
    ) -> bool {
        self.runtime.events()
            .timed_media_complete(kind, name, frame, remove, || {
                self.standalone_audio_duration_ms(name)
            })
    }

    pub(super) fn standalone_audio_duration_ms(&self, name: &str) -> f32 {
        self.owning_world_services().map_or(0.0, |world| {
            let event = crate::common::audio::AudioEventRts::new(name);
            TheAudio::of(&world).get_audio_length_ms(&event)
        })
    }
}
