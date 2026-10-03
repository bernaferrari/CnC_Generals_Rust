use super::AiData;
use crate::common::{
    Snapshot,
    xfer::{Xfer, XferExt},
};

impl Snapshot for AiData {
    fn crc(&self, xfer: &mut dyn Xfer) {
        let mut structure_seconds = self.structure_seconds;
        let _ = xfer.xfer_real(&mut structure_seconds);
        let mut team_seconds = self.team_seconds;
        let _ = xfer.xfer_real(&mut team_seconds);
        let mut resources_wealthy = self.resources_wealthy;
        let _ = xfer.xfer_int(&mut resources_wealthy);
        let mut resources_poor = self.resources_poor;
        let _ = xfer.xfer_int(&mut resources_poor);
        let mut force_idle_frames_count = self.force_idle_frames_count;
        let _ = xfer.xfer_unsigned_int(&mut force_idle_frames_count);
        let mut structures_wealthy_mod = self.structures_wealthy_mod;
        let _ = xfer.xfer_real(&mut structures_wealthy_mod);
        let mut team_wealthy_mod = self.team_wealthy_mod;
        let _ = xfer.xfer_real(&mut team_wealthy_mod);
        let mut structures_poor_mod = self.structures_poor_mod;
        let _ = xfer.xfer_real(&mut structures_poor_mod);
        let mut team_poor_mod = self.team_poor_mod;
        let _ = xfer.xfer_real(&mut team_poor_mod);
        let mut team_resources_to_build = self.team_resources_to_build;
        let _ = xfer.xfer_real(&mut team_resources_to_build);
        let mut guard_inner_modifier_ai = self.guard_inner_modifier_ai;
        let _ = xfer.xfer_real(&mut guard_inner_modifier_ai);
        let mut guard_outer_modifier_ai = self.guard_outer_modifier_ai;
        let _ = xfer.xfer_real(&mut guard_outer_modifier_ai);
        let mut guard_inner_modifier_human = self.guard_inner_modifier_human;
        let _ = xfer.xfer_real(&mut guard_inner_modifier_human);
        let mut guard_outer_modifier_human = self.guard_outer_modifier_human;
        let _ = xfer.xfer_real(&mut guard_outer_modifier_human);
        let mut guard_chase_unit_frames = self.guard_chase_unit_frames;
        let _ = xfer.xfer_unsigned_int(&mut guard_chase_unit_frames);
        let mut guard_enemy_scan_rate = self.guard_enemy_scan_rate;
        let _ = xfer.xfer_unsigned_int(&mut guard_enemy_scan_rate);
        let mut guard_enemy_return_scan_rate = self.guard_enemy_return_scan_rate;
        let _ = xfer.xfer_unsigned_int(&mut guard_enemy_return_scan_rate);
        let mut alert_range_modifier = self.alert_range_modifier;
        let _ = xfer.xfer_real(&mut alert_range_modifier);
        let mut aggressive_range_modifier = self.aggressive_range_modifier;
        let _ = xfer.xfer_real(&mut aggressive_range_modifier);
        let mut attack_priority_distance_modifier = self.attack_priority_distance_modifier;
        let _ = xfer.xfer_real(&mut attack_priority_distance_modifier);
        let mut max_recruit_distance = self.max_recruit_distance;
        let _ = xfer.xfer_real(&mut max_recruit_distance);
        let mut skirmish_base_defense_extra_distance = self.skirmish_base_defense_extra_distance;
        let _ = xfer.xfer_real(&mut skirmish_base_defense_extra_distance);
        let mut repulsed_distance = self.repulsed_distance;
        let _ = xfer.xfer_real(&mut repulsed_distance);
        let mut enable_repulsors = self.enable_repulsors;
        let _ = xfer.xfer_bool(&mut enable_repulsors);
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) {
        let mut version: u8 = 1;
        let _ = xfer.xfer_version(&mut version, 1);
    }

    fn load_post_process(&mut self) {}
}
