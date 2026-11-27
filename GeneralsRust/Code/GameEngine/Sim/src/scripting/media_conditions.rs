//! Media predicates in ScriptConditions.cpp:1418–1437 and 2664–2668.
use super::ScriptError;
use super::events::TimedMediaKind;
use super::interpreter::ScriptInterpreter;
use super::program::{Condition, ConditionType};

/// Observations from the receiving world's audio catalog and playback service.
pub trait ScriptMediaQueries {
    fn audio_duration_ms(&self, name: &str) -> f32;

    fn music_track_completed(&mut self, name: &str, times: i32) -> Result<bool, ScriptError>;
}

impl ScriptInterpreter {
    pub fn try_evaluate_media_condition(
        &self,
        condition: &Condition,
        frame: u32,
        audio: &mut impl ScriptMediaQueries,
    ) -> Result<Option<bool>, ScriptError> {
        let name = || {
            condition
                .get_parameter(0)
                .map(|parameter| parameter.get_string())
                .ok_or_else(|| ScriptError::missing_parameter(0))
        };
        let predicate = condition.get_condition_type();
        let result = match predicate {
            ConditionType::HasFinishedVideo => self.events().has_video(name()?, true),
            ConditionType::HasFinishedSpeech | ConditionType::HasFinishedAudio => {
                let kind = if predicate == ConditionType::HasFinishedSpeech {
                    TimedMediaKind::Speech
                } else {
                    TimedMediaKind::Audio
                };
                let name = name()?;
                self.events()
                    .timed_media_complete(kind, name, frame, true, || audio.audio_duration_ms(name))
            }
            ConditionType::MusicTrackHasCompleted => {
                let name = name()?;
                let times = condition
                    .get_parameter(1)
                    .map_or(0, |parameter| parameter.get_int());
                audio.music_track_completed(name, times)?
            }
            _ => return Ok(None),
        };
        Ok(Some(result))
    }
}
