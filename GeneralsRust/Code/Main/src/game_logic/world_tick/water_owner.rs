//! Main owns synchronous water effects; Core terrain supplies one in-place change.
use super::super::*;
use gamelogic::scripting::engine::ScriptWaterRequest;
use gamelogic::terrain::{TerrainLogic, WaterHeightChange};

impl GameLogic {
    pub(in crate::game_logic) fn apply_owned_script_water(
        &mut self,
        request: ScriptWaterRequest<'_>,
    ) -> gamelogic::GameLogicResult<()> {
        let handle = self.world_services.terrain().clone();
        let change = {
            let mut terrain = handle.write().map_err(|_| {
                gamelogic::GameLogicError::Threading("owned terrain lock poisoned".into())
            })?;
            match request {
                ScriptWaterRequest::SetHeight { name, height } => {
                    terrain.mutate_named_water_height(&name.into(), height, 999_999.9, true)
                }
                ScriptWaterRequest::OverTime {
                    name,
                    height,
                    seconds,
                    damage,
                } => {
                    terrain.change_water_height_over_time(&name.into(), height, seconds, damage);
                    None
                }
            }
        };
        if let Some(change) = change {
            self.apply_owned_water_change(change);
        }
        Ok(())
    }

    pub(in crate::game_logic) fn update_owned_water(&mut self) {
        let handle = self.world_services.terrain().clone();
        // CPP TerrainLogic.cpp:1007: scripts observe last frame's bridge
        // transitions before this terrain phase clears the aggregate gate.
        handle
            .write()
            .expect("owned terrain bridge phase")
            .begin_host_bridge_frame();
        let count = handle
            .read()
            .expect("owned terrain read")
            .dynamic_water_count();
        for index in (0..count).rev() {
            let change = handle
                .write()
                .expect("owned terrain write")
                .advance_dynamic_water(index, self.frame);
            if let Some((change, terminal)) = change {
                self.apply_owned_water_change(change);
                handle
                    .write()
                    .expect("owned terrain write")
                    .finish_dynamic_water(index, terminal);
            }
        }
    }

    fn apply_owned_water_change(&mut self, change: WaterHeightChange) {
        let handle = self.world_services.terrain().clone();
        let ids = {
            let terrain = handle.read().expect("owned terrain read");
            // Original forceMapRecalculation precedes every damage callback.
            if change.reclassify {
                self.reclassify_pathfinding_from_terrain(&terrain);
            }
            let ids = self.owned_water_damage_candidates(&terrain, change);
            terrain.water_grid_state().publish_visual();
            ids
        }; // Release terrain before object damage/destruction callbacks.
        self.damage_owned_water_candidates(ids, change.damage_amount);
    }

    fn owned_water_damage_candidates(
        &mut self,
        terrain: &TerrainLogic,
        change: WaterHeightChange,
    ) -> Vec<ObjectId> {
        let center_x = (change.affected_region.lo.x + change.affected_region.hi.x) * 0.5;
        let center_y = (change.affected_region.lo.y + change.affected_region.hi.y) * 0.5;
        let width = change.affected_region.hi.x - change.affected_region.lo.x;
        let height = change.affected_region.hi.y - change.affected_region.lo.y;
        let radius_squared = width * width + height * height;
        let mut ids = Vec::new();
        // Use the same canonical store visitation as the Main object phase.
        let order: Vec<_> = self.objects.keys().copied().collect();
        for id in order {
            let Some(object) = self.objects.get_mut(&id) else {
                continue;
            };
            let position = object.get_position();
            let underwater = terrain.is_underwater(position.x, position.z, None, None);
            object.cell_is_underwater = underwater;
            let dx = position.x - center_x;
            let dy = position.z - center_y;
            if change.damage_amount > 0.0 && dx * dx + dy * dy <= radius_squared {
                ids.push(id);
            }
        }
        ids
    }

    pub(in crate::game_logic) fn damage_owned_water_candidates(
        &mut self,
        ids: Vec<ObjectId>,
        damage: f32,
    ) -> u32 {
        let mut hit = 0;
        let terrain = self.world_services.terrain().clone();
        for id in ids {
            let Some(position) = self.objects.get(&id).map(|object| object.get_position()) else {
                continue;
            };
            // Recheck at the original callback point; an earlier damage effect
            // may change a later object's position or water table.
            if !terrain
                .read()
                .expect("owned terrain read")
                .is_underwater(position.x, position.z, None, None)
            {
                continue;
            }
            let Some(object) = self.objects.get(&id) else {
                continue;
            };
            if !object.is_alive() || object.status.destroyed {
                continue;
            }
            // TerrainLogic has no projectile/aircraft/boat exclusion. Complete
            // the canonical body callbacks before the next water candidate.
            let result = self
                .apply_owned_damage(
                    id,
                    damage,
                    None,
                    crate::game_logic::combat::DamageType::Water,
                    crate::game_logic::host_usa_pilot::HostDeathType::Normal,
                    None,
                    &crate::game_logic::object::DamageHitContext::default(),
                )
                .expect("admitted water victim remains installed");
            hit += 1;
            if result.destroyed
                || self
                    .objects
                    .get(&id)
                    .is_some_and(|object| object.status.destroyed || object.health.current <= 0.0)
            {
                self.mark_object_for_destruction(id, None);
            }
        }
        hit
    }
}
