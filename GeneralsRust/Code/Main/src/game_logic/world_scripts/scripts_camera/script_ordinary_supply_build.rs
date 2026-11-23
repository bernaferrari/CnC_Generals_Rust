//! Receiving coarse site selection for C++ AIPlayer::buildBySupplies.
use super::*;

const SUPPLY_CENTER_CLOSE_DIST: f32 = 20.0 * gamelogic::path::PATHFIND_CELL_SIZE_F;

impl GameLogic {
    pub(in super::super) fn prepare_ordinary_script_supply_build(
        &self,
        player: u32,
        building: &str,
        minimum_cash: i32,
    ) -> Option<OrdinarySupplyBuild> {
        let ai = self.ai_manager.ai_players.get(&player)?;
        let toward_enemy = !self.templates.get(building)?.is_authored_cash_generator();
        // Solo AIPlayer::getAiEnemy() returns NULL. The defensive anchor
        // requests the script's enemy only after a warehouse is selected.
        let candidate = self.find_ordinary_supply_warehouse(player, ai.base_center, minimum_cash);
        let warehouse = if toward_enemy {
            ai.remembered_supply_warehouse()
                .filter(|id| self.host_object(*id).is_some())
                .or(candidate)?
        } else {
            candidate?
        };
        Some(OrdinarySupplyBuild {
            warehouse,
            toward_enemy,
        })
    }

    pub(in super::super) fn admit_ordinary_script_supply_build(
        &mut self,
        player: u32,
        building: &str,
        build: OrdinarySupplyBuild,
        enemy: Option<u32>,
        effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) {
        let Some(ai) = self.ai_manager.ai_players.get(&player) else {
            return;
        };
        let Some(warehouse) = self.host_object(build.warehouse) else {
            return;
        };
        let (anchor, radius) = if build.toward_enemy {
            let Some(enemy) = enemy else { return };
            (
                self.ordinary_supply_enemy_structure_midpoint(enemy),
                warehouse.geometry_info().bounding_circle_radius(),
            )
        } else {
            (ai.base_center, 3.0 * gamelogic::path::PATHFIND_CELL_SIZE_F)
        };
        let mut position = warehouse.get_position();
        let offset = (horizontal(position) - horizontal(anchor)).normalize_or_zero() * radius;
        position.x -= offset.x;
        position.z -= offset.y;
        if let Some((position, angle)) =
            self.ordinary_supply_build_site(player, building, position, effects)
        {
            let row = ordinary_priority_build_row(building, position, angle);
            self.ai_manager
                .ai_players
                .get_mut(&player)
                .unwrap()
                .admit_ordinary_supply_row(row, build.warehouse);
        }
    }

    /// C++ AIPlayer.cpp:2155–2230, including the strictly-less-than
    /// boundary distance used by PartitionManager's FROM_BOUNDINGSPHERE_2D.
    fn find_ordinary_supply_warehouse(
        &self,
        player: u32,
        base: Vec3,
        mut minimum_cash: i32,
    ) -> Option<ObjectId> {
        let base = horizontal(base);
        let value_per_box = self.supply_box_value();
        let cash_generators = self
            .host_objects()
            .registered_values()
            .filter(|actor| {
                !actor.status.off_map
                    && self.player_owner_for_host_object(actor) == Some(player)
                    && actor.thing().template.is_authored_cash_generator()
            })
            .map(|actor| {
                (
                    horizontal(actor.get_position()),
                    actor.geometry_info().bounding_circle_radius(),
                )
            })
            .collect::<Vec<_>>();
        let candidates =
            self.host_objects()
                .registered_values()
                .filter_map(|warehouse| {
                    let template = &warehouse.thing().template;
                    if !warehouse.is_kind_of(KindOf::Structure)
                        || !warehouse.is_kind_of(KindOf::SupplySource)
                        || !template
                            .receiving_client_template
                            .as_ref()
                            .is_some_and(|source| {
                                source.has_behavior_module("SupplyWarehouseDockUpdate")
                            })
                        || Self::object_relationship_from_owner_to_team_identity(
                            &self.team_factory,
                            &self.players,
                            Some(player),
                            warehouse.owner_player_id,
                            warehouse.team_instance_id,
                            &warehouse.team_instance_name,
                        ) == gamelogic::common::Relationship::Enemies
                    {
                        return None;
                    }
                    let pos = horizontal(warehouse.get_position());
                    let clearance = SUPPLY_CENTER_CLOSE_DIST
                        + warehouse.geometry_info().bounding_circle_radius();
                    if cash_generators.iter().any(|&(center, radius)| {
                        (center.distance(pos) - radius).max(0.0) < clearance
                    }) {
                        return None;
                    }
                    Some((
                        warehouse.id,
                        warehouse.warehouse_boxes().wrapping_mul(value_per_box),
                        pos.distance_squared(base),
                    ))
                })
                .collect::<Vec<_>>();
        loop {
            // The receiving registration order is the Object linked order.
            // Only a strictly closer warehouse replaces an earlier tie.
            let best = candidates
                .iter()
                .copied()
                .filter(|&(_, cash, _)| cash >= minimum_cash)
                .reduce(|a, b| if b.2 < a.2 { b } else { a });
            if let Some((id, _, _)) = best {
                return Some(id);
            }
            minimum_cash /= 2;
            if minimum_cash <= 100 {
                return None;
            }
        }
    }

    /// AIPlayer.cpp:3761–3830, with the default non-conservative bounds.
    /// The original uses structure centers and leaves empty bounds at zero.
    fn ordinary_supply_enemy_structure_midpoint(&self, enemy: u32) -> Vec3 {
        let Some((lo, hi)) = self
            .host_objects()
            .registered_values()
            .filter(|actor| {
                actor.is_kind_of(KindOf::Structure)
                    && self.player_owner_for_host_object(actor) == Some(enemy)
            })
            .map(|actor| horizontal(actor.get_position()))
            .fold(None::<(glam::Vec2, glam::Vec2)>, |bounds, p| {
                Some(bounds.map_or((p, p), |(lo, hi)| (lo.min(p), hi.max(p))))
            })
        else {
            return Vec3::ZERO;
        };
        let midpoint = (lo + hi) * 0.5;
        Vec3::new(midpoint.x, 0.0, midpoint.y)
    }
}

fn horizontal(position: Vec3) -> glam::Vec2 {
    glam::Vec2::new(position.x, position.z)
}

#[cfg(test)]
mod signed_inventory_tests {
use super::*;
use crate::ai::AIDifficulty;

#[test]
fn authored_supply_build_values_each_receiving_warehouses_stock() {
    use game_engine::common::ini::ini_game_data::ProductionEnergyRules;
    const CONTENT: &str = "Object ValueWarehouse\nKindOf = STRUCTURE SUPPLY_SOURCE\n\
        Body = ActiveBody Body\nMaxHealth = 100\nInitialHealth = 100\nEnd\n\
        Behavior = SupplyWarehouseDockUpdate Dock\nStartingBoxes = 1\nEnd\nEnd\n\
        Object ValueCenter\nKindOf = STRUCTURE CASH_GENERATOR\n\
        Body = ActiveBody Body\nMaxHealth = 100\nInitialHealth = 100\nEnd\nEnd\n";
    let world = |value_per_supply_box| {
        let mut world = GameLogic::new();
        world.admit_object_ini_sources([("ValueEconomy.ini".into(), CONTENT.into())]).unwrap();
        world.admit_production_energy_base(ProductionEnergyRules {
            value_per_supply_box,
            ..Default::default()
        });
        world.add_player(Player::new(1, Team::USA, "Builder", true));
        world.ai_manager.add_ai_player(1, Team::USA, AIDifficulty::Medium);
        world.ai_manager.ai_players.get_mut(&1).unwrap().base_center = Vec3::ZERO;
        let negative = world.create_object("ValueWarehouse", Team::Neutral, Vec3::X * 20.0).unwrap();
        let near = world.create_object("ValueWarehouse", Team::Neutral, Vec3::X * 40.0).unwrap();
        let far = world.create_object("ValueWarehouse", Team::Neutral, Vec3::X * 80.0).unwrap();
        world.host_object_mut(negative).unwrap().set_warehouse_boxes(-1);
        world.host_object_mut(far).unwrap().set_warehouse_boxes(2);
        (world, near, far)
    };
    let (custom, near, far) = world(125);
    let (stock, peer_near, peer_far) = world(75);
    assert_eq!((near, far), (peer_near, peer_far));
    assert_eq!(
        custom.prepare_ordinary_script_supply_build(1, "ValueCenter", 105).unwrap().warehouse,
        near
    );
    assert_eq!(
        stock.prepare_ordinary_script_supply_build(1, "ValueCenter", 105).unwrap().warehouse,
        far
    );
}
}
