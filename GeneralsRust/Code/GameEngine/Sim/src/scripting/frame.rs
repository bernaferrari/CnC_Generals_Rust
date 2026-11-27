//! Per-interpreter frame state and update order (ScriptEngine.cpp:5480–5582).

use super::variables::{MissionTimers, ScriptVariables};
use crate::GameLogicResult;

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

/// Mutable state used and persisted by one script interpreter.
pub struct ScriptFrame {
    pub variables: ScriptVariables,
    pub mission_timers: MissionTimers,
    pub fade: ScriptFade,
    pub first_update: bool,
    pub freeze_by_debug: bool,
    pub ui_interactions: Vec<String>,
}

impl Default for ScriptFrame {
    fn default() -> Self {
        Self {
            variables: ScriptVariables::default(),
            mission_timers: MissionTimers::default(),
            fade: ScriptFade::default(),
            first_update: true,
            freeze_by_debug: false,
            ui_interactions: Vec::new(),
        }
    }
}

impl ScriptFrame {
    pub fn take_first_update(&mut self) -> bool {
        std::mem::take(&mut self.first_update)
    }

    pub fn reset(&mut self) {
        self.reset_map_state();
        self.fade.mode = TFade::None;
        self.fade.value = 0.0;
        self.fade.frame = 0;
        self.freeze_by_debug = false;
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

/// Callbacks may re-enter the interpreter. Each state edit ends before dispatch.
pub trait ScriptFrameDriver {
    fn with_frame<R>(&self, edit: impl FnOnce(&mut ScriptFrame) -> R) -> R;
    fn create_named_cache(&mut self);
    fn close_windows(&mut self) -> GameLogicResult<()>;
    fn queue_clear_game_data(&mut self);
    fn execute_sides(&mut self) -> GameLogicResult<()>;
    fn update_teams(&mut self) -> GameLogicResult<()>;
    fn execute_sequences(&mut self) -> GameLogicResult<()>;
}

#[derive(Debug, PartialEq, Eq)]
pub enum ScriptFrameProgress {
    Suspended,
    Complete,
}

pub fn advance(driver: &mut impl ScriptFrameDriver) -> GameLogicResult<ScriptFrameProgress> {
    if driver.with_frame(ScriptFrame::take_first_update) {
        driver.create_named_cache();
    }
    if driver.with_frame(|frame| frame.mission_timers.tick_close_window()) {
        driver.close_windows()?;
    }
    if driver.with_frame(|frame| frame.mission_timers.tick_end_game()) {
        driver.queue_clear_game_data();
    }

    let run_scripts = driver.with_frame(|frame| {
        if frame.freeze_by_debug {
            return false;
        }
        frame.fade.advance();
        if frame.mission_timers.end_game_active() {
            return false;
        }
        frame.variables.tick();
        true
    });
    if !run_scripts {
        return Ok(ScriptFrameProgress::Suspended);
    }

    driver.execute_sides()?;
    driver.update_teams()?;
    driver.with_frame(|frame| frame.ui_interactions.clear());
    driver.execute_sequences()?;
    Ok(ScriptFrameProgress::Complete)
}
