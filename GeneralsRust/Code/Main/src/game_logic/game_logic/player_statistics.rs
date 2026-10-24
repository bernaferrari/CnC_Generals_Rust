//! ScoreKeeper belongs to this Player. Callers observe derived totals or record
//! source facts; they cannot maintain a second writable aggregate score table.
use super::PlayerStatistics;
use crate::game_logic::{KindOf, ThingTemplate};
use game_engine::common::rts::score_keeper::{KindOf as ScoreKind, KindOfMaskType, ScoreKeeper};

pub(crate) fn score_kind_of(template: &ThingTemplate) -> KindOfMaskType {
    let mut mask = KindOfMaskType::new();
    for (kind, score) in [
        (KindOf::Structure, ScoreKind::Structure),
        (KindOf::Score, ScoreKind::Score),
        (KindOf::ScoreCreate, ScoreKind::ScoreCreate),
        (KindOf::ScoreDestroy, ScoreKind::ScoreDestroy),
        (KindOf::Infantry, ScoreKind::Infantry),
        (KindOf::Vehicle, ScoreKind::Vehicle),
        (KindOf::Aircraft, ScoreKind::Aircraft),
    ] {
        if template.is_kind_of(kind) {
            mask.set(score);
        }
    }
    mask
}

impl PlayerStatistics {
    pub(crate) fn for_player(id: u32) -> Self {
        let mut stats = Self::default();
        stats.score.reset(id as i32);
        stats
    }
    pub(crate) fn from_snapshot_totals(
        id: u32,
        units_built: u32,
        units_lost: u32,
        buildings_built: u32,
        buildings_lost: u32,
        resources_collected: u32,
    ) -> Self {
        Self {
            score: ScoreKeeper::with_legacy_counts(
                id as i32,
                units_built as i32,
                units_lost as i32,
                buildings_built as i32,
                buildings_lost as i32,
            ),
            resources_collected,
            ..Self::default()
        }
    }
    pub fn score_keeper(&self) -> &ScoreKeeper {
        &self.score
    }
    pub(crate) fn restore_score_keeper(
        &mut self,
        score: ScoreKeeper,
        id: u32,
    ) -> Result<(), &'static str> {
        if score.player_index() != id as i32 {
            return Err("ScoreKeeper player identity mismatch");
        }
        self.score = score;
        Ok(())
    }
    pub(crate) fn clear_unavailable_snapshot_score(&mut self, id: u32) {
        self.score = ScoreKeeper::with_legacy_counts(
            id as i32,
            self.score.get_total_units_built(),
            self.score.get_total_units_lost(),
            self.score.get_total_buildings_built(),
            self.score.get_total_buildings_lost(),
        );
    }
    pub(crate) fn record_built(&mut self, name: &str, mask: &KindOfMaskType) {
        self.score.add_object_built(name, mask, false);
    }
    pub(crate) fn remove_built(&mut self, name: &str, mask: &KindOfMaskType) {
        self.score.remove_object_built(name, mask);
    }
    pub(crate) fn record_lost(
        &mut self,
        name: &str,
        mask: &KindOfMaskType,
        under_construction: bool,
    ) {
        self.score.add_object_lost(name, mask, under_construction);
    }
    pub(crate) fn record_destroyed(
        &mut self,
        name: &str,
        mask: &KindOfMaskType,
        victim: u32,
        under_construction: bool,
    ) {
        self.score
            .add_object_destroyed(name, mask, victim as usize, under_construction);
    }
    pub(crate) fn record_captured(&mut self, name: &str, mask: &KindOfMaskType) {
        self.score.add_object_captured(name, mask);
    }
    pub(crate) fn earn_money(&mut self, amount: u32) {
        self.score.add_money_earned(amount as i32);
    }
    pub(crate) fn spend_money(&mut self, amount: u32) {
        self.score.add_money_spent(amount as i32);
    }
    pub fn units_destroyed(&self) -> u32 {
        self.score.get_total_units_destroyed().max(0) as u32
    }
    pub fn units_lost(&self) -> u32 {
        self.score.get_total_units_lost().max(0) as u32
    }
    pub fn units_built(&self) -> u32 {
        self.score.get_total_units_built().max(0) as u32
    }
    pub fn structures_destroyed(&self) -> u32 {
        self.score.get_total_buildings_destroyed().max(0) as u32
    }
    pub fn structures_lost(&self) -> u32 {
        self.score.get_total_buildings_lost().max(0) as u32
    }
    pub fn structures_built(&self) -> u32 {
        self.score.get_total_buildings_built().max(0) as u32
    }
    pub fn units_destroyed_self(&self) -> u32 {
        self.score.own_units_destroyed().max(0) as u32
    }
    pub fn structures_destroyed_self(&self) -> u32 {
        self.score.own_buildings_destroyed().max(0) as u32
    }
    pub fn resources_spent(&self) -> u32 {
        self.score.get_total_money_spent().max(0) as u32
    }
    pub fn money_earned(&self) -> u32 {
        self.score.get_total_money_earned().max(0) as u32
    }
    pub fn objects_captured(&self) -> u32 {
        self.score
            .get_total_tech_buildings_captured()
            .wrapping_add(self.score.get_total_faction_buildings_captured())
            .max(0) as u32
    }
}
