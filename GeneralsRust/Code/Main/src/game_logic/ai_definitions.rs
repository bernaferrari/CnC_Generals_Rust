//! C++ AI.cpp:286–414: engine baseline, map overrides, and reset to baseline.
//! Definitions are immutable while a match executes; admission requires its owner.
use game_engine::common::ini::{AIData, ini_ai_data::AIDataStore};
use std::sync::Arc;

#[derive(Debug, Clone)]
pub(crate) struct AiDefinitions {
    base: Arc<AIData>,
    layers: Vec<Arc<AIData>>,
}

impl AiDefinitions {
    pub(crate) fn from_base(data: AIData) -> Self {
        let base = Arc::new(data);
        Self {
            base: Arc::clone(&base),
            layers: vec![base],
        }
    }
    pub(crate) fn data(&self) -> &AIData {
        self.layers.last().expect("AI baseline always exists")
    }
    pub(crate) fn snapshot(&self) -> Arc<AIData> {
        Arc::clone(self.layers.last().expect("AI baseline always exists"))
    }
    /// C++ AI::crc walks the override head through its older definitions;
    /// TAiData::crc (AI.cpp:929-962) includes these scalar fields, not WallHeight.
    pub(in crate::game_logic) fn fold_crc(&self, crc: &mut game_engine::common::crc::Crc) {
        for data in self.layers.iter().rev() {
            crc.compute_crc(b"MARKER:TAiData");
            let words = [
                data.structure_seconds.to_bits(),
                data.team_seconds.to_bits(),
                data.resources_wealthy as u32,
                data.resources_poor as u32,
                data.force_idle_frames_count,
                data.structures_wealthy_mod.to_bits(),
                data.team_wealthy_mod.to_bits(),
                data.structures_poor_mod.to_bits(),
                data.team_poor_mod.to_bits(),
                data.team_resources_to_build.to_bits(),
                data.guard_inner_modifier_ai.to_bits(),
                data.guard_outer_modifier_ai.to_bits(),
                data.guard_inner_modifier_human.to_bits(),
                data.guard_outer_modifier_human.to_bits(),
                data.guard_chase_unit_frames,
                data.guard_enemy_scan_rate,
                data.guard_enemy_return_scan_rate,
                data.alert_range_modifier.to_bits(),
                data.aggressive_range_modifier.to_bits(),
                data.attack_priority_distance_modifier.to_bits(),
                data.max_recruit_distance.to_bits(),
                data.skirmish_base_defense_extra_distance.to_bits(),
                data.repulsed_distance.to_bits(),
            ];
            for word in words {
                crc.compute_crc(&word.to_le_bytes());
            }
            crc.compute_crc(&[u8::from(data.enable_repulsors)]);
        }
    }
    pub(crate) fn baseline(&self) -> &AIData {
        &self.base
    }
    pub(crate) fn reset(&mut self) {
        self.layers = vec![Arc::clone(&self.base)];
    }
    /// Each CREATE_OVERRIDES block pushes a head in this operation-owned draft.
    pub(crate) fn map_override_draft(&self) -> AIDataStore {
        let mut store = AIDataStore::default();
        store.ensure_base();
        *store.get_active_mut().expect("draft base") = self.data().clone();
        store
    }
    /// Admit only a map/Solo CREATE_OVERRIDES draft. Its first entry is the
    /// untouched starting head; subsequent entries preserve override order.
    pub(crate) fn admit_map_overrides(
        &mut self,
        draft: AIDataStore,
    ) -> Result<(), String> {
        let mut definitions = draft.into_definitions().into_iter();
        // The first draft entry copies the current head, already present here.
        definitions.next().ok_or("AIData draft has no baseline")?;
        self.layers.extend(definitions.map(Arc::new));
        Ok(())
    }
}

impl super::GameLogic {
    pub(crate) fn set_ai_definition_base(&mut self, data: AIData) {
        self.ai_definitions = AiDefinitions::from_base(data);
        self.apply_aidata_enable_repulsors();
        self.refresh_pathfinding_ai_definitions();
    }
    pub(crate) fn refresh_pathfinding_ai_definitions(&mut self) {
        self.pathfinding_system
            .set_ai_definitions(self.ai_definitions.snapshot());
    }
    pub(crate) fn admit_ai_map_overrides(
        &mut self,
        draft: AIDataStore,
    ) -> Result<(), String> {
        self.ai_definitions.admit_map_overrides(draft)?;
        self.apply_aidata_enable_repulsors();
        self.refresh_pathfinding_ai_definitions();
        Ok(())
    }
}

#[cfg(test)]
mod tests;
