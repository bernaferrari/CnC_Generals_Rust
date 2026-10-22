//! Source-order construction footprint mine callbacks.
use super::*;

#[path = "construction_collision.rs"]
mod construction_collision;

impl GameLogic {
    /// C++ Object::setStatus runs its callbacks before returning to its caller.
    /// No Object borrow spans a mine's synchronous Body/Die/Destroy callbacks.
    pub(crate) fn set_object_under_construction_with_effects(
        &mut self,
        id: ObjectId,
        value: bool,
        effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> bool {
        let Some(object) = self.objects.get_mut(&id) else {
            return false;
        };
        let changed = object.under_construction_status() != value;
        object.set_status_under_construction(value);
        if changed {
            self.sweep_under_construction_footprint_mines_with_effects(id, effects);
        }
        changed
    }

    /// StatusBitsUpgrade calls setStatus and clearStatus separately. Each
    /// effective edge completes before the next mask or upgrade callback.
    pub(crate) fn apply_object_status_bits_upgrade_with_effects(
        &mut self,
        id: ObjectId,
        set: &[&str],
        clear: &[&str],
        effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> Option<(u32, u32)> {
        let identity = self.live_draw_playback_identity(self.objects.get(&id)?);
        let mut counts = (0, 0);
        let empty: &[&str] = &[];
        for (set, clear) in [(set, empty), (empty, clear)] {
            if !self
                .objects
                .get(&id)
                .is_some_and(|object| self.live_draw_playback_identity(object) == identity)
            {
                break;
            }
            let object = self
                .objects
                .get_mut(&id)
                .expect("validated receiving Object");
            let before = object.under_construction_status();
            let (set_count, clear_count) = object.apply_status_bits_upgrade_masks(set, clear);
            let changed = before != object.under_construction_status();
            counts.0 += set_count;
            counts.1 += clear_count;
            if changed {
                self.sweep_under_construction_footprint_mines_with_effects(id, effects);
            }
        }
        Some(counts)
    }

    /// C++ Object::setStatus UNDER_CONSTRUCTION edge (Object.cpp:985-1011).
    /// Enemy mines receive ordinary Object::kill; allied/neutral mines receive
    /// destroyObject, retaining their Body and the later physical-removal phase.
    pub(in super::super::super) fn sweep_under_construction_footprint_mines(
        &mut self,
        building_id: ObjectId,
    ) {
        self.sweep_under_construction_footprint_mines_with_effects(
            building_id,
            &mut Default::default(),
        )
    }

    pub(in super::super::super) fn sweep_under_construction_footprint_mines_with_effects(
        &mut self,
        building_id: ObjectId,
        effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) {
        use gamelogic::common::Relationship;

        let Some(building) = self.objects.get(&building_id) else {
            return;
        };
        if building.status.destroyed {
            return;
        }
        let building_identity = self.live_draw_playback_identity(building);
        let mut hits = Vec::new();
        for (id, obj) in &self.objects {
            if *id == building_id || obj.status.destroyed {
                continue;
            }
            let is_mine = obj.is_kind_of(KindOf::Mine);
            if !is_mine {
                continue;
            }
            if !construction_collision::object_would_collide(
                building.get_position(),
                &building.thing().template.geometry_info,
                building.get_orientation(),
                obj.get_position(),
                &obj.thing().template.geometry_info,
                obj.get_orientation(),
            ) {
                continue;
            }
            hits.push((*id, self.live_draw_playback_identity(obj)));
        }

        for (mine_id, mine_identity) in hits {
            // Earlier callbacks may destroy/capture another candidate. Resolve
            // its current owner and lifetime before this callback, not in a batch.
            let Some(building) = self.objects.get(&building_id) else {
                break;
            };
            let Some(mine) = self.objects.get(&mine_id) else {
                continue;
            };
            if self.live_draw_playback_identity(building) != building_identity {
                break;
            }
            if mine.status.destroyed || self.live_draw_playback_identity(mine) != mine_identity {
                continue;
            }
            let relation = self.object_relationship(building, mine);
            match relation {
                Relationship::Enemies => {
                    self.apply_owned_kill_with_effects(
                        mine_id,
                        crate::game_logic::combat::DamageType::Unresistable,
                        crate::game_logic::host_usa_pilot::HostDeathType::Normal,
                        effects,
                    );
                }
                Relationship::Allies | Relationship::Neutral => {
                    self.destroy_object_with_effects(mine_id, effects);
                }
            }
        }
    }
}
