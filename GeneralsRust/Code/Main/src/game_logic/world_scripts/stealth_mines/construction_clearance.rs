//! Source-order construction footprint mine callbacks.
use super::*;

impl GameLogic {
    /// C++ Object::setStatus UNDER_CONSTRUCTION edge (Object.cpp:985-1011).
    /// Leftover `object_status::set_status` is the reference: enemy mines
    /// `kill(LANDMINE, EXPLODED)`, ally/neutral mines silent `destroyObject`.
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
        use crate::game_logic::host_mines::HostMineDetonateReason;
        use gamelogic::common::Relationship;

        let Some(building) = self.objects.get(&building_id) else {
            return;
        };
        if building.status.destroyed {
            return;
        }
        let bpos = building.get_position();
        let br = if building.thing().template.geometry_info.authored {
            building
                .thing()
                .template
                .geometry_info
                .bounding_circle_radius()
        } else {
            Self::structure_place_radius(building)
        }
        .max(1.0);

        let mut hits: Vec<(ObjectId, Relationship, bool)> = Vec::new();
        for (id, obj) in &self.objects {
            if *id == building_id || obj.status.destroyed {
                continue;
            }
            let is_mine = obj.is_kind_of(KindOf::Mine) || obj.mine_data.is_some();
            if !is_mine {
                continue;
            }
            let mpos = obj.get_position();
            let mr = if obj.thing().template.geometry_info.authored {
                obj.thing().template.geometry_info.bounding_circle_radius()
            } else {
                obj.selection_radius.max(1.0)
            };
            let dx = mpos.x - bpos.x;
            let dz = mpos.z - bpos.z;
            let limit = br + mr;
            if dx * dx + dz * dz > limit * limit {
                continue;
            }
            let rel = self.object_relationship(building, obj);
            hits.push((*id, rel, obj.mine_data.is_some()));
        }

        for (mine_id, rel, has_data) in hits {
            match rel {
                Relationship::Enemies => {
                    if has_data {
                        let _ = self.detonate_mine_internal_with_effects(
                            mine_id,
                            HostMineDetonateReason::Proximity,
                            None,
                            effects,
                        );
                    } else if let Some(obj) = self.objects.get_mut(&mine_id) {
                        {
                            let owned_victim_id = obj.id;
                            let owned_damage_amount = obj.health.maximum.max(1.0);
                            let owned_damage_type = crate::game_logic::combat::DamageType::LandMine;
                            self.apply_residual_damage_owned_with_effects(
                                owned_victim_id,
                                owned_damage_amount,
                                Some(building_id),
                                owned_damage_type,
                                crate::game_logic::host_usa_pilot::HostDeathType::Exploded,
                                None,
                                effects,
                            )
                        };
                    }
                }
                Relationship::Allies | Relationship::Neutral => {
                    self.destroy_object_with_effects(mine_id, effects);
                }
            }
        }
    }
}
