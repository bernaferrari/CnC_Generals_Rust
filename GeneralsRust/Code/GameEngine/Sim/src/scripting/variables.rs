//! Named counters, flags, and mission timers for one script interpreter.

use crate::error::{GameLogicError, GameLogicResult};
use crate::time::LOGIC_FRAMES_PER_SECOND;
use game_engine::base_type::real_to_int_ceil;

// C++ ScriptEngine.h reserves slot zero in each table.
pub const MAX_COUNTERS: usize = 256;
pub const MAX_FLAGS: usize = 256;

#[derive(Debug, Clone)]
pub struct Counter {
    pub value: i32,
    pub name: String,
    pub is_countdown_timer: bool,
}

impl Counter {
    pub fn new(name: String) -> Self {
        Self {
            value: 0,
            name,
            is_countdown_timer: false,
        }
    }
}

#[derive(Debug, Clone)]
pub struct Flag {
    pub value: bool,
    pub name: String,
}

impl Flag {
    pub fn new(name: String) -> Self {
        Self { value: false, name }
    }
}

pub type NamedTrackersSnapshot = (Vec<(String, i32, bool)>, Vec<(String, bool)>);

pub struct ScriptVariables {
    counters: Vec<Option<Counter>>,
    counter_count: usize,
    flags: Vec<Option<Flag>>,
    flag_count: usize,
}

impl Default for ScriptVariables {
    fn default() -> Self {
        Self {
            counters: vec![None; MAX_COUNTERS],
            counter_count: 1,
            flags: vec![None; MAX_FLAGS],
            flag_count: 1,
        }
    }
}

impl ScriptVariables {
    pub fn reset(&mut self) {
        self.counters.fill(None);
        self.counter_count = 1;
        self.flags.fill(None);
        self.flag_count = 1;
    }

    /// Startup and reset admit slot zero after clearing the tables.
    pub fn reserve_empty_slots(&mut self) {
        self.counters[0].get_or_insert_with(|| Counter::new(String::new()));
        self.flags[0].get_or_insert_with(|| Flag::new(String::new()));
    }

    pub fn allocate_counter(&mut self, name: &str) -> GameLogicResult<usize> {
        if let Some(index) = self
            .counters
            .iter()
            .position(|slot| slot.as_ref().is_some_and(|counter| counter.name == name))
        {
            return Ok(index);
        }
        let index = self
            .counters
            .iter()
            .position(Option::is_none)
            .ok_or_else(|| GameLogicError::Configuration("Maximum counters exceeded".into()))?;
        self.counters[index] = Some(Counter::new(name.to_owned()));
        self.counter_count = self.counter_count.max(index + 1);
        Ok(index)
    }

    pub fn allocate_flag(&mut self, name: &str) -> GameLogicResult<usize> {
        if let Some(index) = self
            .flags
            .iter()
            .position(|slot| slot.as_ref().is_some_and(|flag| flag.name == name))
        {
            return Ok(index);
        }
        let index = self
            .flags
            .iter()
            .position(Option::is_none)
            .ok_or_else(|| GameLogicError::Configuration("Maximum flags exceeded".into()))?;
        self.flags[index] = Some(Flag::new(name.to_owned()));
        self.flag_count = self.flag_count.max(index + 1);
        Ok(index)
    }

    pub fn counter(&self, name: &str) -> Option<&Counter> {
        self.counters
            .iter()
            .flatten()
            .find(|counter| counter.name == name)
    }

    pub fn flag(&self, name: &str) -> Option<&Flag> {
        self.flags.iter().flatten().find(|flag| flag.name == name)
    }

    pub(super) fn counter_mut(&mut self, name: &str) -> GameLogicResult<&mut Counter> {
        let index = self.allocate_counter(name)?;
        Ok(self.counters[index]
            .as_mut()
            .expect("allocated counter slot"))
    }

    pub(super) fn flag_mut(&mut self, name: &str) -> GameLogicResult<&mut Flag> {
        let index = self.allocate_flag(name)?;
        Ok(self.flags[index].as_mut().expect("allocated flag slot"))
    }

    pub fn set_counter(&mut self, name: &str, value: i32) -> GameLogicResult<()> {
        self.counter_mut(name)?.value = value;
        Ok(())
    }

    pub fn set_flag(&mut self, name: &str, value: bool) -> GameLogicResult<()> {
        self.flag_mut(name)?.value = value;
        Ok(())
    }

    pub fn increment_counter(&mut self, name: &str, amount: i32) -> GameLogicResult<()> {
        let counter = self.counter_mut(name)?;
        counter.value = counter.value.saturating_add(amount);
        Ok(())
    }

    pub fn decrement_counter(&mut self, name: &str, amount: i32) -> GameLogicResult<()> {
        let counter = self.counter_mut(name)?;
        counter.value = counter.value.saturating_sub(amount);
        Ok(())
    }

    /// C++ ScriptEngine::clearFlag clears the player-suffixed names.
    pub fn clear_flag(&mut self, name: &str, player_count: usize) {
        for player in 0..player_count {
            let player_name = format!("{name}{player}");
            for index in 1..self.flag_count {
                if let Some(flag) = &mut self.flags[index]
                    && flag.name == player_name
                {
                    flag.value = false;
                }
            }
        }
    }

    pub fn set_timer(&mut self, name: &str, frames: i32) -> GameLogicResult<()> {
        let counter = self.counter_mut(name)?;
        counter.value = frames;
        counter.is_countdown_timer = true;
        Ok(())
    }

    pub fn set_timer_seconds(&mut self, name: &str, seconds: f32) -> GameLogicResult<()> {
        self.set_timer(name, (seconds * LOGIC_FRAMES_PER_SECOND) as i32)
    }

    pub fn set_timer_millisecond_script_seconds(
        &mut self,
        name: &str,
        seconds: f32,
    ) -> GameLogicResult<()> {
        self.set_timer(name, millisecond_script_frames(seconds))
    }

    /// Resolve the duration after allocation. Random actions consume no RNG
    /// if the counter table is full.
    pub fn set_timer_from(
        &mut self,
        name: &str,
        millisecond_timer: bool,
        duration: impl FnOnce() -> f32,
    ) -> GameLogicResult<()> {
        let counter = self.counter_mut(name)?;
        let value = duration();
        counter.value = if millisecond_timer {
            millisecond_script_frames(value)
        } else {
            value as i32
        };
        counter.is_countdown_timer = true;
        Ok(())
    }

    pub fn stop_timer(&mut self, name: &str) -> GameLogicResult<()> {
        self.counter_mut(name)?.is_countdown_timer = false;
        Ok(())
    }

    pub fn restart_timer(&mut self, name: &str) -> GameLogicResult<()> {
        let counter = self.counter_mut(name)?;
        if counter.value > 0 {
            counter.is_countdown_timer = true;
        }
        Ok(())
    }

    pub fn add_to_timer_millisecond_script_seconds(
        &mut self,
        name: &str,
        seconds: f32,
    ) -> GameLogicResult<()> {
        self.add_timer_frames(name, millisecond_script_frames(seconds))
    }

    pub fn subtract_from_timer_millisecond_script_seconds(
        &mut self,
        name: &str,
        seconds: f32,
    ) -> GameLogicResult<()> {
        self.add_timer_frames(name, millisecond_script_frames(-seconds))
    }

    fn add_timer_frames(&mut self, name: &str, frames: i32) -> GameLogicResult<()> {
        let counter = self.counter_mut(name)?;
        counter.value = counter.value.wrapping_add(frames);
        Ok(())
    }

    pub fn adjust_timer(
        &mut self,
        name: &str,
        value: i32,
        millisecond_timer: bool,
        add: bool,
    ) -> GameLogicResult<()> {
        if millisecond_timer {
            let seconds = value as f32;
            let seconds = if add { seconds } else { -seconds };
            self.add_timer_frames(name, millisecond_script_frames(seconds))
        } else {
            self.add_timer_frames(name, if add { value } else { value.wrapping_neg() })
        }
    }

    /// C++ ScriptEngine::update counts down through zero and stops at -1.
    pub fn tick(&mut self) {
        for counter in self.counters.iter_mut().flatten() {
            if counter.is_countdown_timer && counter.value >= 0 {
                counter.value -= 1;
            }
        }
    }

    pub fn snapshot(&self) -> NamedTrackersSnapshot {
        let counters = self
            .counters
            .iter()
            .flatten()
            .filter(|counter| !counter.name.is_empty())
            .map(|counter| {
                (
                    counter.name.clone(),
                    counter.value,
                    counter.is_countdown_timer,
                )
            })
            .collect();
        let flags = self
            .flags
            .iter()
            .flatten()
            .filter(|flag| !flag.name.is_empty())
            .map(|flag| (flag.name.clone(), flag.value))
            .collect();
        (counters, flags)
    }

    pub fn restore(
        &mut self,
        counters: &[(String, i32, bool)],
        flags: &[(String, bool)],
    ) -> GameLogicResult<()> {
        for (name, value, is_countdown) in counters {
            let counter = self.counter_mut(name)?;
            counter.value = *value;
            counter.is_countdown_timer = *is_countdown;
        }
        for (name, value) in flags {
            self.set_flag(name, *value)?;
        }
        Ok(())
    }

    /// Legacy Xfer serializes slots (including zero) and a separate count.
    pub fn legacy_counters(&mut self) -> (&mut [Option<Counter>], &mut usize) {
        (&mut self.counters, &mut self.counter_count)
    }

    /// Legacy Xfer serializes slots (including zero) and a separate count.
    pub fn legacy_flags(&mut self) -> (&mut [Option<Flag>], &mut usize) {
        (&mut self.flags, &mut self.flag_count)
    }
}

/// The original "millisecond" script actions specify real-valued seconds.
fn millisecond_script_frames(seconds: f32) -> i32 {
    let milliseconds = seconds * 1000.0;
    real_to_int_ceil(milliseconds * (LOGIC_FRAMES_PER_SECOND / 1000.0))
}

/// Whole-frame deadlines from ScriptEngine::update and its Xfer tail.
pub struct MissionTimers {
    pub end_game: i32,
    pub close_window: i32,
}

impl Default for MissionTimers {
    fn default() -> Self {
        Self {
            end_game: -1,
            close_window: -1,
        }
    }
}

impl MissionTimers {
    pub fn start_end_game(&mut self) {
        self.end_game = 120;
    }

    pub fn start_quick_end_game(&mut self) {
        self.end_game = 1;
    }

    pub fn start_close_window(&mut self) {
        self.close_window = 120;
    }

    pub fn end_game_active(&self) -> bool {
        self.end_game >= 0
    }

    pub fn tick_end_game(&mut self) -> bool {
        tick_deadline(&mut self.end_game)
    }

    pub fn tick_close_window(&mut self) -> bool {
        tick_deadline(&mut self.close_window)
    }
}

fn tick_deadline(frames: &mut i32) -> bool {
    if *frames > 0 {
        *frames -= 1;
        *frames == 0
    } else {
        false
    }
}
