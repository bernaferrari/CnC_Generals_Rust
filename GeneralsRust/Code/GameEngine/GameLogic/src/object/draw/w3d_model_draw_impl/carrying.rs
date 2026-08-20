/// Merge a deferred MODELCONDITION_CARRYING bit into an incoming drawable flag set.
///
/// C++ `W3DModelDraw::updateDrawModuleSupplyStatus` writes the bit on the Drawable
/// (`set/clearModelConditionState(MODELCONDITION_CARRYING)`). The Rust callback
/// runs under the drawable lock, so we cannot re-enter `Drawable::set_model_condition_state`.
/// Persist the intent on the module and re-apply it on every later replace so damage /
/// night / move bitsets cannot wipe CARRYING.
fn merge_carrying_flag(
    conditions: ModelConditionFlags,
    pending_carrying: Option<bool>,
) -> ModelConditionFlags {
    let mut merged = conditions;
    match pending_carrying {
        Some(true) => {
            merged.insert(ModelConditionFlags::CARRYING);
        }
        Some(false) => {
            merged.remove(ModelConditionFlags::CARRYING);
        }
        None => {}
    }
    merged
}

impl W3DModelDraw {
    fn apply_pending_carrying(&self, conditions: ModelConditionFlags) -> ModelConditionFlags {
        merge_carrying_flag(conditions, self.pending_carrying)
    }

    fn note_supply_carrying(&mut self, current_supply: i32) {
        self.pending_carrying = Some(current_supply > 0);
    }
}
