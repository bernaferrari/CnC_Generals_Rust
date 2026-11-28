//! Per-interpreter frame state and update order (ScriptEngine.cpp:5480–5582).

use super::{
    interpreter::ScriptInterpreter,
    object_types::ObjectTypes,
    sequential_scripts::SequentialProgramDriver,
    variables::{MissionTimers, ScriptVariables},
};
use crate::{GameLogicResult, time::LOGIC_HZ};
pub use generals_game_domain::AIDifficulty;
use std::collections::HashMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TFade {
    None = 0,
    Subtract = 1,
    Add = 2,
    Saturate = 3,
    Multiply = 4,
}

/// Fields are also used by the existing save adapters.
pub struct ScriptFade {
    pub mode: TFade,
    pub min: f32,
    pub max: f32,
    pub value: f32,
    pub frame: i32,
    pub increase: i32,
    pub hold: i32,
    pub decrease: i32,
}

impl Default for ScriptFade {
    fn default() -> Self {
        Self {
            mode: TFade::None,
            min: 0.0,
            max: 1.0,
            value: 0.0,
            frame: 0,
            increase: 0,
            hold: 0,
            decrease: 0,
        }
    }
}

impl ScriptFade {
    pub fn start(mode: TFade, min: f32, max: f32, increase: i32, hold: i32, decrease: i32) -> Self {
        let mut fade = Self {
            mode,
            min,
            max,
            value: min,
            frame: 0,
            increase,
            hold,
            decrease,
        };
        if increase == 0 {
            fade.advance();
        }
        fade
    }

    pub fn map_intro() -> Self {
        Self {
            mode: TFade::Multiply,
            min: 1.0,
            max: 0.0,
            decrease: 33,
            ..Self::default()
        }
    }

    /// ScriptEngine::updateFades. Rendering only reads the completed value.
    pub fn advance(&mut self) {
        if self.mode == TFade::None {
            return;
        }

        self.frame += 1;
        let mut elapsed = self.frame;
        if elapsed <= self.increase {
            let factor = self.frame as f32 / self.increase as f32;
            self.value = self.min + factor * (self.max - self.min);
            return;
        }
        elapsed -= self.increase;
        if elapsed <= self.hold {
            self.value = self.max;
            return;
        }
        elapsed -= self.hold;
        if elapsed <= self.decrease {
            let divisor = self.decrease + 1;
            let factor = elapsed as f32 / if divisor == 0 { 1 } else { divisor } as f32;
            self.value = self.max + factor * (self.min - self.max);
            return;
        }
        self.mode = TFade::None;
    }
}

/// Script-controlled tree sway. The existing wire form stores both direction
/// components and the signed period and version independently.
#[derive(Debug, Clone)]
pub struct BreezeInfo {
    pub direction: f32,
    pub direction_vec: [f32; 2],
    pub intensity: f32,
    pub lean: f32,
    pub randomness: f32,
    pub breeze_period: i16,
    pub breeze_version: i16,
}

impl Default for BreezeInfo {
    fn default() -> Self {
        let direction = std::f32::consts::PI / 3.0;
        let amplitude = 0.07 * std::f32::consts::PI / 4.0;
        Self {
            direction,
            direction_vec: [direction.sin(), direction.cos()],
            intensity: amplitude,
            lean: amplitude,
            randomness: 0.2,
            breeze_period: (LOGIC_HZ * 5) as i16,
            breeze_version: 0,
        }
    }
}

impl BreezeInfo {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn set_sway(
        &mut self,
        direction: f32,
        intensity: f32,
        lean: f32,
        period: i32,
        randomness: f32,
    ) {
        self.breeze_version = self.breeze_version.wrapping_add(1);
        self.direction = direction;
        self.direction_vec = [direction.sin(), direction.cos()];
        self.intensity = intensity;
        self.lean = lean;
        self.breeze_period = period.clamp(1, i16::MAX as i32) as i16;
        self.randomness = randomness;
    }
}

/// Mutable state used and persisted by one script interpreter.
pub struct ScriptFrame {
    pub variables: ScriptVariables,
    pub mission_timers: MissionTimers,
    pub fade: ScriptFade,
    pub breeze_info: BreezeInfo,
    pub choose_victim_always_uses_normal: bool,
    pub object_types: HashMap<String, ObjectTypes>,
    object_counts: HashMap<i32, HashMap<String, i32>>,
    pub first_update: bool,
    pub freeze_by_debug: bool,
    pub freeze_by_script: bool,
    pub difficulty: AIDifficulty,
    pub objects_should_receive_difficulty_bonus: bool,
    pub ui_interactions: Vec<String>,
}

impl Default for ScriptFrame {
    fn default() -> Self {
        Self {
            variables: ScriptVariables::default(),
            mission_timers: MissionTimers::default(),
            fade: ScriptFade::default(),
            breeze_info: BreezeInfo::default(),
            choose_victim_always_uses_normal: false,
            object_types: HashMap::new(),
            object_counts: HashMap::new(),
            first_update: true,
            freeze_by_debug: false,
            freeze_by_script: false,
            difficulty: AIDifficulty::Medium,
            objects_should_receive_difficulty_bonus: true,
            ui_interactions: Vec::new(),
        }
    }
}

impl ScriptFrame {
    pub fn object_count(&self, player: i32, name: &str) -> i32 {
        self.object_counts.get(&player).and_then(|counts| counts.get(name)).copied().unwrap_or(0)
    }

    pub fn set_object_count(&mut self, player: i32, name: &str, count: i32) {
        self.object_counts.entry(player).or_default().insert(name.to_owned(), count);
    }

    pub fn observe_object_count(&mut self, player: i32, name: &str, count: i32, active: bool) -> bool {
        let previous = if active { self.object_count(player, name) } else { 0 };
        if count != previous {
            self.set_object_count(player, name, count);
        }
        count < previous
    }

    pub fn take_first_update(&mut self) -> bool {
        std::mem::take(&mut self.first_update)
    }

    pub fn reset(&mut self) {
        self.reset_map_state();
        self.fade.mode = TFade::None;
        self.fade.value = 0.0;
        self.fade.frame = 0;
        self.freeze_by_debug = false;
        self.freeze_by_script = false;
        self.difficulty = AIDifficulty::Medium;
        self.objects_should_receive_difficulty_bonus = true;
        self.breeze_info = BreezeInfo::default();
        self.choose_victim_always_uses_normal = false;
        self.object_types.clear();
        self.object_counts.clear();
    }

    pub fn new_map(&mut self) {
        self.reset_map_state();
        self.fade = ScriptFade::map_intro();
    }

    fn reset_map_state(&mut self) {
        self.variables.reset();
        self.variables.reserve_empty_slots();
        self.mission_timers = MissionTimers::default();
        self.first_update = true;
        self.ui_interactions.clear();
    }
}

/// World observations and effects for one script frame. Callbacks may re-enter
/// the interpreter after each state borrow has ended.
pub trait ScriptFrameDriver: SequentialProgramDriver {
    fn create_named_cache(&mut self);
    fn destroy_win_lose_window(&mut self) -> GameLogicResult<()>;
    fn queue_clear_game_data(&mut self);
    fn side_player(&mut self, side: usize) -> Option<String>;
    fn update_teams(&mut self, interpreter: &ScriptInterpreter) -> GameLogicResult<()>;
    fn frame_completed(&mut self) {}
}

impl ScriptInterpreter {
    /// Run the authored phases in C++ UPDATE order on the receiving interpreter.
    pub fn advance_frame(
        &self,
        frame: u32,
        driver: &mut impl ScriptFrameDriver,
    ) -> GameLogicResult<()> {
        self.observe_object_count_changed_frame(driver.receiving_world());
        if self.frame_mut().take_first_update() {
            driver.create_named_cache();
        }
        if self.frame_mut().mission_timers.tick_close_window() {
            self.presentation().record_window_close(false);
            driver.destroy_win_lose_window()?;
        }
        if self.frame_mut().mission_timers.tick_end_game() {
            driver.queue_clear_game_data();
        }

        {
            let mut state = self.frame_mut();
            if state.freeze_by_debug {
                return Ok(());
            }
            state.fade.advance();
            if state.mission_timers.end_game_active() {
                return Ok(());
            }
            state.variables.tick();
        }

        self.execute_sides(frame, driver)?;
        driver.update_teams(self)?;
        self.frame_mut().ui_interactions.clear();
        self.advance_sequences(frame, driver)?;
        driver.frame_completed();
        Ok(())
    }
}
