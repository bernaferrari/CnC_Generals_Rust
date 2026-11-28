//! Script continuation shared by the receiving simulation and its save codecs.

use super::{
    attack_priorities::AttackPriorityInfo,
    events::ScriptEventHistory,
    frame::{BreezeInfo, ScriptFade, TFade},
    interpreter::ScriptInterpreter,
    object_types::ObjectTypes,
};
use game_engine::common::ascii_string::AsciiString;
use generals_game_domain::AIDifficulty;
use glam::Vec3;

/// ScriptEngine saves the difficulty as its established integer ordinal.
pub const fn difficulty_to_wire(difficulty: AIDifficulty) -> i32 {
    match difficulty {
        AIDifficulty::Easy => 0,
        AIDifficulty::Medium => 1,
        AIDifficulty::Hard => 2,
        AIDifficulty::Brutal => 3,
    }
}

pub const fn difficulty_from_wire(ordinal: i32) -> Option<AIDifficulty> {
    match ordinal {
        0 => Some(AIDifficulty::Easy),
        1 => Some(AIDifficulty::Medium),
        2 => Some(AIDifficulty::Hard),
        3 => Some(AIDifficulty::Brutal),
        _ => None,
    }
}

impl ScriptInterpreter {
    pub fn capture_continuation(&self, named_objects: Vec<(String, u32)>) -> ScriptContinuation {
        let events = self.events().snapshot();
        let frame = self.frame();
        let priorities = self.attack_priorities();
        let attack_priorities = priorities
            .entries()
            .iter()
            .map(|info| {
                let mut entries: Vec<(String, i32)> = info
                    .priority_map
                    .iter()
                    .map(|(name, priority)| (name.clone(), *priority))
                    .collect();
                entries.sort_by(|a, b| a.0.cmp(&b.0));
                (info.name.clone(), info.default_priority, entries)
            })
            .collect();
        let object_attack_priority_sets = priorities
            .assignments()
            .map(|(id, name)| (id, name.to_owned()))
            .collect();
        let mut object_types: Vec<(String, Vec<String>)> = frame
            .object_types
            .iter()
            .map(|(name, list)| {
                (
                    name.clone(),
                    list.iter()
                        .map(|entry| entry.as_str().to_string())
                        .collect(),
                )
            })
            .collect();
        object_types.sort_by(|a, b| a.0.cmp(&b.0));
        ScriptContinuation {
            attack_priorities,
            object_attack_priority_sets,
            end_game_timer: frame.mission_timers.end_game,
            close_window_timer: frame.mission_timers.close_window,
            named_objects,
            first_update: frame.first_update,
            fade: frame.fade.mode as i32,
            min_fade: frame.fade.min,
            max_fade: frame.fade.max,
            cur_fade_value: frame.fade.value,
            cur_fade_frame: frame.fade.frame,
            fade_frames_increase: frame.fade.increase,
            fade_frames_hold: frame.fade.hold,
            fade_frames_decrease: frame.fade.decrease,
            completed_video: events.completed_video,
            testing_speech: events.testing_speech,
            testing_audio: events.testing_audio,
            ui_interactions: frame.ui_interactions.clone(),
            triggered_special_powers: events.triggered_special_powers,
            midway_special_powers: events.midway_special_powers,
            finished_special_powers: events.finished_special_powers,
            completed_upgrades: events.completed_upgrades,
            acquired_sciences: events.acquired_sciences,
            topple_directions: self
                .topple_directions()
                .iter()
                .map(|(name, coord)| (name.clone(), [coord.x, coord.y, coord.z]))
                .collect(),
            breeze_direction: frame.breeze_info.direction,
            breeze_direction_vec: frame.breeze_info.direction_vec,
            breeze_intensity: frame.breeze_info.intensity,
            breeze_lean: frame.breeze_info.lean,
            breeze_randomness: frame.breeze_info.randomness,
            breeze_period: frame.breeze_info.breeze_period,
            breeze_version: frame.breeze_info.breeze_version,
            game_difficulty: difficulty_to_wire(frame.difficulty),
            freeze_by_script: frame.freeze_by_script,
            object_types,
            objects_should_receive_difficulty_bonus: frame.objects_should_receive_difficulty_bonus,
            current_track_name: self.current_track_name(),
            choose_victim_always_uses_normal: frame.choose_victim_always_uses_normal,
        }
    }

    pub fn restore_continuation(&self, tail: &ScriptContinuation) {
        self.events().restore(ScriptEventHistory {
            completed_video: tail.completed_video.clone(),
            testing_speech: tail.testing_speech.clone(),
            testing_audio: tail.testing_audio.clone(),
            triggered_special_powers: tail.triggered_special_powers.clone(),
            midway_special_powers: tail.midway_special_powers.clone(),
            finished_special_powers: tail.finished_special_powers.clone(),
            completed_upgrades: tail.completed_upgrades.clone(),
            acquired_sciences: tail.acquired_sciences.clone(),
        });
        let mut frame = self.frame_mut();
        let mut priorities = self.attack_priorities_mut();
        // Original ScriptConditions reset precedes load; this observation
        // history has no Xfer payload and must not retain the donor run.
        self.clear_transport_observations();
        priorities.restore(
            tail.attack_priorities
                .iter()
                .map(|(name, default_priority, entries)| {
                    let mut info = AttackPriorityInfo::new();
                    info.name = name.clone();
                    info.default_priority = *default_priority;
                    info.priority_map = entries.iter().cloned().collect();
                    info
                })
                .collect(),
        );
        priorities.clear_assignments();
        for (id, name) in &tail.object_attack_priority_sets {
            if !name.is_empty() {
                priorities.assign(*id, name);
            }
        }
        frame.mission_timers.end_game = tail.end_game_timer;
        frame.mission_timers.close_window = tail.close_window_timer;
        frame.first_update = tail.first_update;
        frame.fade.mode = match tail.fade {
            1 => TFade::Subtract,
            2 => TFade::Add,
            3 => TFade::Saturate,
            4 => TFade::Multiply,
            _ => TFade::None,
        };
        frame.fade.min = tail.min_fade;
        frame.fade.max = tail.max_fade;
        frame.fade.value = tail.cur_fade_value;
        frame.fade.frame = tail.cur_fade_frame;
        frame.fade.increase = tail.fade_frames_increase;
        frame.fade.hold = tail.fade_frames_hold;
        frame.fade.decrease = tail.fade_frames_decrease;
        frame.ui_interactions = tail.ui_interactions.clone();
        *self.topple_directions_mut() = tail
            .topple_directions
            .iter()
            .map(|(name, xyz)| (name.clone(), Vec3::new(xyz[0], xyz[1], xyz[2])))
            .collect();
        frame.breeze_info.direction = tail.breeze_direction;
        frame.breeze_info.direction_vec = tail.breeze_direction_vec;
        frame.breeze_info.intensity = tail.breeze_intensity;
        frame.breeze_info.lean = tail.breeze_lean;
        frame.breeze_info.randomness = tail.breeze_randomness;
        frame.breeze_info.breeze_period = tail.breeze_period;
        frame.breeze_info.breeze_version = tail.breeze_version;
        frame.difficulty =
            difficulty_from_wire(tail.game_difficulty).unwrap_or(AIDifficulty::Medium);
        frame.freeze_by_script = tail.freeze_by_script;
        frame.object_types.clear();
        for (list_name, types) in &tail.object_types {
            let mut list = ObjectTypes::with_list_name(AsciiString::from(list_name.as_str()));
            for object_type in types {
                list.add_object_type(AsciiString::from(object_type.as_str()));
            }
            frame.object_types.insert(list_name.clone(), list);
        }
        frame.objects_should_receive_difficulty_bonus =
            tail.objects_should_receive_difficulty_bonus;
        self.set_current_track_name(tail.current_track_name.clone());
        frame.choose_victim_always_uses_normal = tail.choose_victim_always_uses_normal;
    }

    /// An archive load starts the standard intro when no saved fade is active.
    /// ScriptEngine.cpp:9277–9286.
    pub fn restore_archive_continuation(&self, tail: &ScriptContinuation) {
        self.restore_continuation(tail);
        let mut frame = self.frame_mut();
        if frame.fade.mode == TFade::None {
            frame.fade = ScriptFade::map_intro();
        }
    }

    /// Saves without a script continuation restart the initial name scan.
    pub fn reset_names_for_legacy_load(&self) {
        self.clear_transport_observations();
        self.frame_mut().first_update = true;
    }
}

/// C++ `ScriptEngine::xfer` fields after counters/flags (`ScriptEngine.cpp:8812+`).
#[derive(Debug, Clone, PartialEq)]
pub struct ScriptContinuation {
    pub attack_priorities: Vec<(String, i32, Vec<(String, i32)>)>,
    pub object_attack_priority_sets: Vec<(u32, String)>,
    pub end_game_timer: i32,
    pub close_window_timer: i32,
    pub named_objects: Vec<(String, u32)>,
    pub first_update: bool,
    pub fade: i32,
    pub min_fade: f32,
    pub max_fade: f32,
    pub cur_fade_value: f32,
    pub cur_fade_frame: i32,
    pub fade_frames_increase: i32,
    pub fade_frames_hold: i32,
    pub fade_frames_decrease: i32,
    pub completed_video: Vec<String>,
    pub testing_speech: Vec<(String, u32)>,
    pub testing_audio: Vec<(String, u32)>,
    pub ui_interactions: Vec<String>,
    pub triggered_special_powers: Vec<Vec<(String, u32)>>,
    pub midway_special_powers: Vec<Vec<(String, u32)>>,
    pub finished_special_powers: Vec<Vec<(String, u32)>>,
    pub completed_upgrades: Vec<Vec<(String, u32)>>,
    pub acquired_sciences: Vec<Vec<i32>>,
    pub topple_directions: Vec<(String, [f32; 3])>,
    pub breeze_direction: f32,
    pub breeze_direction_vec: [f32; 2],
    pub breeze_intensity: f32,
    pub breeze_lean: f32,
    pub breeze_randomness: f32,
    pub breeze_period: i16,
    pub breeze_version: i16,
    pub game_difficulty: i32,
    pub freeze_by_script: bool,
    pub object_types: Vec<(String, Vec<String>)>,
    pub objects_should_receive_difficulty_bonus: bool,
    pub current_track_name: String,
    pub choose_victim_always_uses_normal: bool,
}

impl Default for ScriptContinuation {
    fn default() -> Self {
        let breeze = BreezeInfo::default();
        Self {
            attack_priorities: Vec::new(),
            object_attack_priority_sets: Vec::new(),
            end_game_timer: -1,
            close_window_timer: -1,
            named_objects: Vec::new(),
            first_update: true,
            fade: TFade::None as i32,
            min_fade: 0.0,
            max_fade: 1.0,
            cur_fade_value: 0.0,
            cur_fade_frame: 0,
            fade_frames_increase: 0,
            fade_frames_hold: 0,
            fade_frames_decrease: 0,
            completed_video: Vec::new(),
            testing_speech: Vec::new(),
            testing_audio: Vec::new(),
            ui_interactions: Vec::new(),
            triggered_special_powers: Vec::new(),
            midway_special_powers: Vec::new(),
            finished_special_powers: Vec::new(),
            completed_upgrades: Vec::new(),
            acquired_sciences: Vec::new(),
            topple_directions: Vec::new(),
            breeze_direction: breeze.direction,
            breeze_direction_vec: breeze.direction_vec,
            breeze_intensity: breeze.intensity,
            breeze_lean: breeze.lean,
            breeze_randomness: breeze.randomness,
            breeze_period: breeze.breeze_period,
            breeze_version: breeze.breeze_version,
            game_difficulty: 1,
            freeze_by_script: false,
            object_types: Vec::new(),
            objects_should_receive_difficulty_bonus: true,
            current_track_name: String::new(),
            choose_victim_always_uses_normal: false,
        }
    }
}
