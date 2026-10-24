//! C++ RiderChangeContain::update ScuttleDelay after an ordinary rider exit.
use super::*;

impl GameLogic {
    pub(super) fn tick_scuttled_riders_with_effects(
        &mut self,
        effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) {
        // C++ RiderChangeContain::update: after an ordinary rider exit, the
        // bike remains as an unselectable toppled shell for ScuttleDelay, then
        // dies with DEATH_TOPPLED.  Replacement keeps m_containing=true and
        // therefore never reaches this list.
        let scuttles_due: Vec<ObjectId> = self
            .objects
            .registered()
            .filter_map(|(&id, object)| {
                let delay = object
                    .thing()
                    .template
                    .contain_module
                    .rider_change_scuttle_delay_frames?;
                let started = object.rider_change_scuttled_on_frame;
                (object.thing().template.contain_module.kind
                    == crate::game_logic::ContainModuleKind::RiderChange
                    && started != 0
                    && !object.status.destroyed
                    && self.frame >= started.saturating_add(delay))
                .then_some(id)
            })
            .collect();
        for object_id in scuttles_due {
            if let Some(object) = self.objects.get_mut(&object_id) {
                object.status.death_type =
                    crate::game_logic::host_usa_pilot::HostDeathType::Toppled;
            }
            self.mark_destroyed_authority_aware(object_id, None);
            self.mark_object_for_destruction_with_effects(object_id, None, effects);
        }
    }
}
