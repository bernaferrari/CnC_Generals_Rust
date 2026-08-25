//! C++ `DIRTY_CONDITION_FLAGS` + `getDrawModules` replace dispatch.
//!
//! `Drawable::clearAndSetModelConditionFlags` marks the drawable dirty;
//! the next `getDrawModules` walks every `ObjectDrawInterface` and calls
//! `replaceModelConditionState`. Force-replace applies immediately.

use super::*;
use game_engine::common::bit_flags::{
    ModelConditionBitFlags, ModelConditionFlags, create_model_condition_flags,
};

impl BasicDrawable {
    /// C++ `Drawable::clearAndSetModelConditionFlags`.
    pub fn clear_and_set_model_condition_flags(
        &mut self,
        clr: &ModelConditionBitFlags,
        set: &ModelConditionBitFlags,
    ) {
        let old = self.model_condition_flags.clone();
        self.model_condition_flags.clear_and_set(clr, set);
        if self.model_condition_flags != old {
            self.is_model_dirty = true;
        }
    }

    /// C++ `Drawable::clearModelConditionFlags`.
    pub fn clear_model_condition_flags(&mut self, clr: &ModelConditionBitFlags) {
        let empty = create_model_condition_flags();
        self.clear_and_set_model_condition_flags(clr, &empty);
    }

    /// C++ `Drawable::setModelConditionFlags`.
    pub fn set_model_condition_flags(&mut self, set: &ModelConditionBitFlags) {
        let empty = create_model_condition_flags();
        self.clear_and_set_model_condition_flags(&empty, set);
    }

    /// Cave / garrison leave path: drop `GARRISONED` and dirty-apply.
    pub fn clear_model_condition_garrisoned(&mut self) {
        let mut clr = create_model_condition_flags();
        clr.set(ModelConditionFlags::GARRISONED, true);
        self.clear_model_condition_flags(&clr);
    }

    /// C++ `Drawable::replaceModelConditionFlags`.
    pub fn replace_model_condition_flags(
        &mut self,
        flags: ModelConditionBitFlags,
        force_replace: bool,
    ) {
        if !force_replace && self.model_condition_flags == flags {
            return;
        }
        self.model_condition_flags = flags;
        if force_replace {
            self.apply_model_condition_to_draw_modules();
            self.is_model_dirty = false;
        } else {
            self.is_model_dirty = true;
        }
    }

    /// C++ `Drawable::setModelConditionState`.
    pub fn set_model_condition_state(&mut self, index: usize) {
        let empty = create_model_condition_flags();
        let mut set = create_model_condition_flags();
        set.set(index, true);
        self.clear_and_set_model_condition_flags(&empty, &set);
    }

    /// C++ `Drawable::clearModelConditionState`.
    pub fn clear_model_condition_state(&mut self, index: usize) {
        let mut clr = create_model_condition_flags();
        clr.set(index, true);
        let empty = create_model_condition_flags();
        self.clear_and_set_model_condition_flags(&clr, &empty);
    }

    /// C++ `Drawable::getDrawModules` dirty flush.
    pub(super) fn flush_dirty_model_condition(&mut self) {
        if !self.is_model_dirty {
            return;
        }
        self.apply_model_condition_to_draw_modules();
        self.is_model_dirty = false;
    }

    fn apply_model_condition_to_draw_modules(&mut self) {
        let flags = self.model_condition_flags.clone();
        for module in &mut self.draw_modules {
            module.replace_model_condition_state(&flags);
        }
    }
}
