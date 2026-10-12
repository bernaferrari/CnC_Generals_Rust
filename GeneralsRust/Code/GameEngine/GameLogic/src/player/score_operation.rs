//! Explicit C++ ScoreKeeper policy admission for one borrowed operation.
use super::ScoreKeeper;
use crate::common::Int;

pub struct ScoreOperation<'a> {
    keeper: &'a mut ScoreKeeper,
    scoring_enabled: bool,
}
impl<'a> ScoreOperation<'a> {
    pub(super) fn new(keeper: &'a mut ScoreKeeper, scoring_enabled: bool) -> Self {
        Self {
            keeper,
            scoring_enabled,
        }
    }
    pub fn add_unit_built(&mut self) {
        self.keeper.add_unit_built_with_policy(self.scoring_enabled);
    }

    pub fn add_unit_killed(&mut self) {
        self.keeper
            .add_unit_killed_with_policy(self.scoring_enabled);
    }

    pub fn add_unit_lost(&mut self) {
        self.keeper.add_unit_lost_with_policy(self.scoring_enabled);
    }

    pub fn add_object_built_template(&mut self, template_name: &str, retail_kindof_bits: u64) {
        self.keeper.add_object_built_template_with_policy(
            template_name,
            retail_kindof_bits,
            self.scoring_enabled,
        );
    }

    pub fn add_object_destroyed_template(
        &mut self,
        template_name: &str,
        retail_kindof_bits: u64,
        owner_player_index: Int,
        under_construction: bool,
    ) {
        self.keeper.add_object_destroyed_template_with_policy(
            template_name,
            retail_kindof_bits,
            owner_player_index,
            under_construction,
            self.scoring_enabled,
        );
    }

    pub fn add_object_lost_template(
        &mut self,
        template_name: &str,
        retail_kindof_bits: u64,
        under_construction: bool,
    ) {
        self.keeper.add_object_lost_template_with_policy(
            template_name,
            retail_kindof_bits,
            under_construction,
            self.scoring_enabled,
        );
    }

    pub fn add_building_destroyed(&mut self) {
        self.keeper
            .add_building_destroyed_with_policy(self.scoring_enabled);
    }

    pub fn add_building_built(&mut self) {
        self.keeper
            .add_building_built_with_policy(self.scoring_enabled);
    }

    pub fn add_object_lost_obj(
        &mut self,
        object: &dyn game_engine::common::rts::score_keeper::ScoreableObject,
    ) {
        self.keeper
            .add_object_lost_obj_with_policy(object, self.scoring_enabled);
    }

    pub fn add_object_destroyed_obj(
        &mut self,
        object: &dyn game_engine::common::rts::score_keeper::ScoreableObject,
    ) {
        self.keeper
            .add_object_destroyed_obj_with_policy(object, self.scoring_enabled);
    }

    pub fn add_object_built_obj(
        &mut self,
        object: &dyn game_engine::common::rts::score_keeper::ScoreableObject,
    ) {
        self.keeper
            .add_object_built_obj_with_policy(object, self.scoring_enabled);
    }

    pub fn remove_object_built_obj(
        &mut self,
        object: &dyn game_engine::common::rts::score_keeper::ScoreableObject,
    ) {
        self.keeper
            .remove_object_built_obj_with_policy(object, self.scoring_enabled);
    }

    pub fn add_object_captured_obj(
        &mut self,
        object: &dyn game_engine::common::rts::score_keeper::ScoreableObject,
    ) {
        self.keeper
            .add_object_captured_obj_with_policy(object, self.scoring_enabled);
    }
    // C++ money ledgers are not gated by gameplay scoring.
    pub fn add_money_earned(&mut self, amount: u32) {
        self.keeper.add_money_earned(amount);
    }
    pub fn add_money_spent(&mut self, amount: u32) {
        self.keeper.add_money_spent(amount);
    }
}
