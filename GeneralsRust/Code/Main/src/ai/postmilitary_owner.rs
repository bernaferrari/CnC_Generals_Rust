//! CPP AIPlayer::update resumes upgrades, science and bridge effects with
//! the session's canonical controllers visible at each immediate effect.
use super::*;

pub(super) struct UpgradeAttempt {
    pub player: u32,
    pub team: Team,
    pub name: String,
    pub producer: ObjectId,
    pub cost: Resources,
}
impl UpgradeAttempt {
    pub(super) fn execute(&self, world: &mut GameLogic) -> bool {
        let kind = world.upgrade_type(&self.name);
        let Some((player, economy_events)) = world.player_and_economy_events_mut(self.player)
        else {
            return false;
        };
        if !player.queue_upgrade(&self.name, &self.cost, kind, economy_events) {
            return false;
        }
        let seconds = world.upgrade_research_time_secs(&self.name);
        if !world.unit_command_building_add_upgrade_to_queue(
            self.producer,
            &self.name,
            seconds,
            self.cost,
        ) {
            if let Some((player, economy_events)) = world.player_and_economy_events_mut(self.player)
            {
                let _ = player.cancel_queued_upgrade(&self.name, &self.cost, economy_events);
            }
            return false;
        }
        world.record_host_upgrade_queued(self.player, self.team, &self.name, Some(self.producer));
        world
            .host_upgrades_mut()
            .set_build_cost_paid(&self.name, self.player, self.cost.supplies);
        world.host_upgrades_mut().set_resolved_research_frames(
            &self.name,
            self.player,
            (seconds * LOGIC_FRAMES_PER_SECOND).round().max(1.0) as u32,
        );
        true
    }
}

pub(super) enum BridgeRepairEffect {
    QueueDozer,
    Repair { dozer: ObjectId, bridge: ObjectId },
    Home { dozer: ObjectId, position: Vec3 },
}
impl BridgeRepairEffect {
    pub(super) fn execute(self, world: &mut GameLogic, player: u32) -> bool {
        match self {
            Self::QueueDozer => {
                unreachable!("dozer production requires the canonical AI continuation")
            }
            Self::Repair { dozer, bridge } => {
                world.queue_command(crate::command_system::GameCommand {
                    command_type: crate::command_system::CommandType::Repair { target_id: bridge },
                    player_id: player,
                    command_id: 0,
                    timestamp: std::time::SystemTime::now(),
                    selected_units: vec![dozer],
                    modifier_keys: crate::command_system::ModifierKeys::default(),
                });
                true
            }
            Self::Home {
                dozer,
                mut position,
            } => {
                let _ = world.adjust_to_possible_destination(dozer, &mut position);
                let _ = world.unit_command_move_to(dozer, position);
                false
            }
        }
    }
}

impl GameLogic {
    pub(crate) fn build_owned_script_ai_upgrade(&mut self, player: u32, upgrade: &str) -> bool {
        let attempt = self
            .ai_manager
            .ai_players
            .get(&player)
            .and_then(|ai| ai.prepare_script_upgrade(self, upgrade));
        let Some(attempt) = attempt else {
            return false;
        };
        if !attempt.execute(self) {
            return false;
        }
        if let Some(ai) = self.ai_manager.ai_players.get_mut(&player) {
            ai.activity_count = ai.activity_count.saturating_add(1);
        }
        true
    }

    pub(crate) fn update_owned_ai_postmilitary(
        &mut self,
        player: u32,
        now: f32,
        data: &AiDataView,
    ) {
        self.update_owned_ai_postmilitary_observing(player, now, data, |_, _| {});
    }

    // Observation cannot replace the actual effects or mutate the owner.
    fn update_owned_ai_postmilitary_observing(
        &mut self,
        player: u32,
        now: f32,
        data: &AiDataView,
        mut observe: impl FnMut(&GameLogic, &'static str),
    ) {
        // CPP AIPlayer.cpp2910 suppresses both upgrade and science admission
        // in the first two logic frames. Bridge and local strategy still run.
        if self.get_frame() >= 2 {
            let attempts = self
                .ai_manager
                .ai_players
                .get(&player)
                .map(|ai| ai.prepare_structure_upgrades(self))
                .unwrap_or_default();
            for attempt in attempts {
                observe(self, "upgrade");
                if attempt.execute(self) {
                    if let Some(ai) = self.ai_manager.ai_players.get_mut(&player) {
                        ai.activity_count = ai.activity_count.saturating_add(1);
                    }
                    break;
                }
            }
            let Some(mut ai) = self.ai_manager.ai_players.remove(&player) else {
                return;
            };
            let sciences = ai.prepare_skillset_sciences(self, data);
            self.ai_manager.ai_players.insert(player, ai);
            for science in sciences {
                observe(self, "science");
                AIPlayer::purchase_skillset_science(self, player, &science);
            }
        }
        let Some(mut ai) = self.ai_manager.ai_players.remove(&player) else {
            return;
        };
        let bridge = ai.prepare_bridge_repair(self, now);
        self.ai_manager.ai_players.insert(player, ai);
        if let Some(effect) = bridge {
            observe(self, "bridge");
            match effect {
                BridgeRepairEffect::QueueDozer => {
                    self.admit_owned_ai_dozer(player, now);
                    if let Some(ai) = self.ai_manager.ai_players.get_mut(&player) {
                        ai.dozer_queued_for_repair = true;
                    }
                }
                effect => {
                    let repairing = effect.execute(self, player);
                    if repairing {
                        if let Some(ai) = self.ai_manager.ai_players.get_mut(&player) {
                            ai.dozer_is_repairing = true;
                        }
                    }
                }
            }
        }
        let Some(mut ai) = self.ai_manager.ai_players.remove(&player) else {
            return;
        };
        // These two functions read world state and mutate only this controller.
        ai.update_strategy_phase(self, now);
        ai.update_build_phase(self, now);
        self.ai_manager.ai_players.insert(player, ai);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game_logic::{Player, ThingTemplate};
    fn world() -> (GameLogic, ObjectId) {
        let mut world = GameLogic::new();
        for (id, name) in [(1, "UpgradeOwner"), (2, "UpgradePeer")] {
            let mut player = Player::new(id, Team::USA, name, false);
            player.resources.supplies = 20000;
            player.science_purchase_points = 1;
            world.add_player(player);
            world.add_ai_opponent(id, Team::USA, AIDifficulty::Medium);
            world
                .ai_manager
                .ai_players
                .get_mut(&id)
                .unwrap()
                .skillset_selector = 0;
        }
        let mut barracks = ThingTemplate::new("AmericaBarracks");
        barracks
            .add_kind_of(KindOf::Structure)
            .add_kind_of(KindOf::FSBarracks)
            .set_health(1000.0);
        world.templates.insert(barracks.name.clone(), barracks);
        let producer = world
            .create_object_for_player("AmericaBarracks", 1, Vec3::ZERO)
            .unwrap();
        (world, producer)
    }

    #[test]
    fn upgrades_and_science_effects_observe_canonical_controllers_in_cpp_phase_order() {
        let (mut world, producer) = world();
        world.frame = 2;
        let data = AiDataView::from_world(&world);
        let mut phases = Vec::new();
        world.update_owned_ai_postmilitary_observing(1, 0.0, &data, |world, phase| {
            assert_eq!(world.ai_manager.ai_players.len(), 2);
            assert_eq!(world.ai_manager.ai_players[&1].player_id, 1);
            assert_eq!(world.ai_manager.ai_players[&2].player_id, 2);
            phases.push(phase);
        });
        assert_eq!(phases.first(), Some(&"upgrade"));
        assert!(phases[1..].iter().all(|phase| *phase == "science"));
        assert!(phases.len() > 1);
        assert!(
            world
                .get_player(1)
                .unwrap()
                .has_queued_upgrade("Upgrade_AmericaRangerCaptureBuilding")
        );
        assert!(
            world
                .host_object(producer)
                .unwrap()
                .building_data
                .as_ref()
                .unwrap()
                .production_queue
                .iter()
                .any(|item| item.is_upgrade())
        );
        assert_eq!(world.ai_manager.ai_players[&1].activity_count, 1);
        assert!(
            !world
                .get_player(2)
                .unwrap()
                .has_queued_upgrade("Upgrade_AmericaRangerCaptureBuilding")
        );
        assert_eq!(world.get_player(2).unwrap().resources.supplies, 20000);
        assert_eq!(world.get_player(2).unwrap().science_purchase_points, 1);
    }

    #[test]
    fn first_two_frames_do_not_admit_upgrade_or_science_effects() {
        let (mut world, _) = world();
        let data = AiDataView::from_world(&world);
        for frame in [0, 1] {
            world.frame = frame;
            world.update_owned_ai_postmilitary_observing(1, 0.0, &data, |_, _| {
                panic!("first two frames cannot admit upgrade/science effects");
            });
        }
        assert_eq!(world.ai_manager.ai_players.len(), 2);
        assert_eq!(world.get_player(1).unwrap().resources.supplies, 20000);
        assert_eq!(world.get_player(1).unwrap().science_purchase_points, 1);
    }
    #[test]
    fn completed_bridge_repair_returns_dozer_home_with_canonical_roster_visible() {
        let (mut world, _) = world();
        let mut bridge_template = ThingTemplate::new("OwnedCompletedBridge");
        bridge_template
            .add_kind_of(KindOf::Structure)
            .set_health(2000.0);
        world
            .templates
            .insert(bridge_template.name.clone(), bridge_template);
        let bridge = world
            .create_object(
                "OwnedCompletedBridge",
                Team::Neutral,
                Vec3::new(80.0, 0.0, 80.0),
            )
            .unwrap();
        let mut dozer_template = ThingTemplate::new("OwnedRepairDozer");
        dozer_template
            .add_kind_of(KindOf::Dozer)
            .add_kind_of(KindOf::Vehicle)
            .add_kind_of(KindOf::Selectable);
        dozer_template.locomotor_name = Some("BasicHumanLocomotor".into());
        world
            .templates
            .insert(dozer_template.name.clone(), dozer_template);
        let dozer = world
            .create_object_for_player("OwnedRepairDozer", 1, Vec3::ZERO)
            .unwrap();
        let home = Vec3::new(120.0, 0.0, 120.0);
        let ai = world.ai_manager.ai_players.get_mut(&1).unwrap();
        ai.structures_to_repair.push(bridge);
        ai.repair_dozer = Some(dozer);
        ai.dozer_is_repairing = true;
        ai.base_center = home;
        ai.last_bridge_repair_time = -1.0;
        let data = AiDataView::from_world(&world);
        let mut observed = 0;
        world.update_owned_ai_postmilitary_observing(1, 0.0, &data, |world, phase| {
            assert_eq!(phase, "bridge");
            assert_eq!(world.ai_manager.ai_players.len(), 2);
            assert!(
                world.ai_manager.ai_players[&1]
                    .structures_to_repair
                    .is_empty()
            );
            assert!(!world.ai_manager.ai_players[&1].dozer_is_repairing);
            observed += 1;
        });
        assert_eq!(observed, 1);
        assert_eq!(world.host_object(dozer).unwrap().ai_state, AIState::Moving);
        assert_eq!(
            world.host_object(dozer).unwrap().path_goal_position,
            Some(home)
        );
        assert!(world.ai_manager.ai_players[&2].repair_dozer.is_none());
    }
}
