// Message filter/translator traits and command-evaluator adapters.
// Split from `core/game_client.rs` dump. Included by `game_client/mod.rs`
// so this stays one logical `game_client` module (public API identical).

/// Trait for message filtering components
pub trait MessageFilter {
    fn should_keep_message(&self, msg: &GameMessage) -> bool;
    fn transform_message(&self, msg: &mut GameMessage) -> GameMessageResult<()>;
}

/// In-Game UI interface for managing game interface elements
pub trait InGameUI: SubsystemInterface + Send + Sync {
    /// Stop tracking a drawable object in the UI
    fn disregard_drawable(&self, drawable: &dyn Drawable)
        -> Result<(), Box<dyn std::error::Error>>;

    /// React to beacon changes so the HUD/radar can display the correct data.
    fn handle_beacon_notification(
        &mut self,
        _notification: &BeaconNotification,
    ) -> Result<(), Box<dyn std::error::Error>> {
        Ok(())
    }
}

/// Video player interface for playing cutscenes and videos
pub trait VideoPlayerInterface: SubsystemInterface + Send + Sync {
    // Video player methods would be defined here
    // For now it's just a marker trait
}

/// Command translator interface for context-sensitive commands
pub trait CommandTranslator: Send + Sync {
    fn evaluate_context_command(
        &self,
        drawable: &dyn Drawable,
        position: &Coord3D,
        cmd_type: CommandEvaluateType,
    ) -> GameMessageResult<GameMessageType>;
}

/// Command evaluation types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommandEvaluateType {
    Primary,
    Secondary,
    Context,
}

impl CommandTranslator for RwLock<CommandTranslatorImpl> {
    fn evaluate_context_command(
        &self,
        drawable: &dyn Drawable,
        position: &Coord3D,
        cmd_type: CommandEvaluateType,
    ) -> GameMessageResult<GameMessageType> {
        let mut translator = self.write().map_err(|err| {
            GameClientError::SubsystemError(format!("Command translator lock poisoned: {err}"))
        })?;
        translator.evaluate_context_command(drawable, position, cmd_type)
    }
}

fn map_client_time_of_day_to_ini(time_of_day: TimeOfDay) -> IniTimeOfDay {
    match time_of_day {
        TimeOfDay::Morning => IniTimeOfDay::Morning,
        TimeOfDay::Afternoon => IniTimeOfDay::Afternoon,
        TimeOfDay::Evening => IniTimeOfDay::Evening,
        TimeOfDay::Night => IniTimeOfDay::Night,
    }
}
