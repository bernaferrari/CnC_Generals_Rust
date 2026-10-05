//! C++ ActiveBody.cpp:646–653: score, onDie, fresh source lookup, DamageFX.
//! This owner keeps both objects installed and finishes before the next hit.
use super::super::*;
use crate::game_logic::object::{DamageApplication, DamageHitContext};

/// Retarget observations taken after body callbacks, before world onDie.
#[derive(Debug)]
pub(in crate::game_logic) struct OwnedDamageResult {
    pub(in crate::game_logic) destroyed: bool,
    pub(in crate::game_logic) victim_position: glam::Vec3,
    pub(in crate::game_logic) victim_team: Team,
}

impl GameLogic {
    pub(in crate::game_logic) fn apply_owned_damage(
        &mut self,
        victim_id: ObjectId,
        damage: f32,
        source_id: Option<ObjectId>,
        damage_type: crate::game_logic::combat::DamageType,
        death_type: crate::game_logic::host_usa_pilot::HostDeathType,
        fx_override: Option<crate::game_logic::combat::DamageType>,
        context: &DamageHitContext,
    ) -> Option<OwnedDamageResult> {
        let victim = self.objects.get_mut(&victim_id)?;
        let application: DamageApplication = victim.begin_damage_with_context(
            damage,
            source_id,
            damage_type,
            death_type,
            fx_override,
            self.frame,
            context,
        );
        let victim_position = victim.get_position();
        let victim_team = victim.team;
        if application.killed() {
            // Keep the existing once-only gates and XP-sink routing. Do not
            // predict a promotion or cache XP before the callbacks.
            if let Some(source) = source_id {
                self.award_score_the_kill_experience(source, victim_id);
            }
            let killer_team = source_id.and_then(|id| self.objects.get(&id).map(|obj| obj.team));
            self.mark_object_for_destruction(victim_id, killer_team);
        }
        let source = application
            .source_id()
            .and_then(|id| self.objects.get(&id))
            .map(crate::game_logic::host_transition_damage_fx::snapshot_damage_fx_source);
        // mark_object_for_destruction queues final removal; the synchronous
        // onDie boundary must leave the victim installed for its DamageFX.
        let victim = self
            .objects
            .get_mut(&victim_id)
            .expect("damage victim remains installed until destroy-list cleanup");
        Some(OwnedDamageResult {
            destroyed: application.finish(victim, source.as_ref()),
            victim_position,
            victim_team,
        })
    }
}
