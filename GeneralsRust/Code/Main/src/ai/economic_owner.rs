//! Original base-building effects run with the canonical AI roster installed.
use super::*;

pub(super) struct ScaffoldAttempt {
    pub player: u32,
    pub token: u64,
    pub template_name: String,
    pub position: Vec3,
    pub dozer: ObjectId,
    pub cost: Resources,
}
impl ScaffoldAttempt {
    pub(super) fn execute(&self, world: &mut GameLogic) -> Option<ObjectId> {
        self.execute_with_ui(world, &mut None)
    }

    pub(super) fn execute_with_ui(
        &self,
        world: &mut GameLogic,
        ui: &mut Option<&mut dyn crate::game_logic::ScriptUiEffects>,
    ) -> Option<ObjectId> {
        let structure = world.create_dozer_scaffold_with_ui(
            &self.template_name,
            self.player,
            self.dozer,
            self.position,
            0.0,
            self.cost,
            ui,
        )?;
        self.admit_construct_order(world, structure)
    }
    #[cfg(test)]
    fn execute_observing(
        &self,
        world: &mut GameLogic,
        observe: impl FnMut(&GameLogic, ObjectId),
    ) -> Option<ObjectId> {
        let structure = world.create_dozer_scaffold_observing(
            &self.template_name,
            self.player,
            self.dozer,
            self.position,
            self.cost,
            observe,
        )?;
        self.admit_construct_order(world, structure)
    }
    fn admit_construct_order(
        &self,
        world: &mut GameLogic,
        structure: ObjectId,
    ) -> Option<ObjectId> {
        if world.resume_construction(&[self.dozer], structure)
            && world.unit_command_begin_construct(self.dozer, self.position)
        {
            return Some(structure);
        }
        world.cancel_dozers_building(structure);
        world.destroy_object(structure);
        None
    }
}

pub(super) struct DozerQueueAttempt {
    pub player: u32,
    pub template: String,
    pub factory: ObjectId,
    pub preexisting: Vec<ObjectId>,
}
impl DozerQueueAttempt {
    pub(super) fn execute(&self, world: &mut GameLogic) -> bool {
        AIPlayer::with_can_build_units(world, self.player, true, |world| {
            world.enqueue_production(self.factory, self.template.clone())
        })
    }
}

impl GameLogic {
    pub(crate) fn update_owned_ai_economy(&mut self, player: u32, now: f32, data: &AiDataView) {
        self.update_owned_ai_economy_with_ui(player, now, data, &mut None)
    }

    pub(crate) fn update_owned_ai_economy_with_ui(
        &mut self,
        player: u32,
        now: f32,
        data: &AiDataView,
        ui: &mut Option<&mut dyn crate::game_logic::ScriptUiEffects>,
    ) {
        let Some(ai) = self.ai_manager.ai_players.get(&player) else {
            return;
        };
        if now < ai.next_building_time {
            return;
        }
        let team = ai.team;
        let repair = ai.repair_dozer;
        let unfinished: Vec<_> = ai
            .building_queue
            .iter()
            .filter_map(|pad| {
                let id = pad.object_id?;
                let object = self.host_object(id)?;
                (self.object_owned_by_player(object, player)
                    && object.is_alive()
                    && object.status.under_construction)
                    .then_some((id, object.get_position()))
            })
            .collect();
        self.resume_owned_ai_construction(player, team, repair, unfinished);
        self.queue_owned_ai_dozer(player, now);
        let Some(mut ai) = self.ai_manager.ai_players.remove(&player) else {
            return;
        };
        let attempt = ai.prepare_scaffold_attempt(self, now, data);
        self.ai_manager.ai_players.insert(player, ai);
        if let Some(attempt) = attempt {
            if let Some(object) = attempt.execute_with_ui(self, ui) {
                if let Some(ai) = self.ai_manager.ai_players.get_mut(&player) {
                    ai.finish_scaffold_attempt(&attempt, object);
                }
            }
        }
        let Some(mut ai) = self.ai_manager.ai_players.remove(&player) else {
            return;
        };
        ai.sync_build_list_object_status(self, now);
        ai.next_building_time =
            now + ai.scaled_interval_seconds(self, AIPlayer::STRUCTURE_SECONDS, true);
        self.ai_manager.ai_players.insert(player, ai);
    }

    pub(super) fn resume_owned_ai_construction(
        &mut self,
        player: u32,
        team: Team,
        repair: Option<ObjectId>,
        unfinished: Vec<(ObjectId, Vec3)>,
    ) {
        for (id, position) in unfinished {
            let builder = self.host_objects().values().any(|object| {
                self.object_owned_by_player(object, player)
                    && object.is_alive()
                    && object.can_construct()
                    && object.target == Some(id)
                    && object.ai_state == AIState::Constructing
            });
            if !builder {
                if let Some(dozer) = AIPlayer::find_available_dozer(self, team, position, repair) {
                    if self.resume_construction(&[dozer], id) {
                        self.unit_command_begin_construct(dozer, position);
                    }
                }
            }
        }
    }

    pub(super) fn queue_owned_ai_dozer(&mut self, player: u32, now: f32) {
        let Some(ai) = self.ai_manager.ai_players.get(&player) else {
            return;
        };
        if AIPlayer::team_has_any_dozer(self, ai.team) {
            return;
        }
        self.admit_owned_ai_dozer(player, now);
    }

    pub(super) fn admit_owned_ai_dozer(&mut self, player: u32, now: f32) {
        let Some(ai) = self.ai_manager.ai_players.get(&player) else {
            return;
        };
        let attempt = ai.prepare_dozer_queue(self);
        if let Some(attempt) = attempt {
            let queued = attempt.execute(self);
            if let Some(ai) = self.ai_manager.ai_players.get_mut(&player) {
                ai.finish_dozer_queue(attempt, queued, now);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn world() -> (GameLogic, ObjectId) {
        let mut world = GameLogic::new();
        for (id, name) in [(1, "BuilderOwner"), (2, "PeerOwner")] {
            let mut player = Player::new(id, Team::USA, name, false);
            player.resources.supplies = 10_000;
            world.add_player(player);
            world.add_ai_opponent(id, Team::USA, AIDifficulty::Medium);
            world
                .ai_manager
                .ai_players
                .get_mut(&id)
                .unwrap()
                .building_queue
                .clear();
        }
        let mut dozer = ThingTemplate::new("OwnedEconomyDozer");
        dozer
            .add_kind_of(KindOf::Dozer)
            .add_kind_of(KindOf::Selectable);
        dozer.locomotor_name = Some("BasicHumanLocomotor".into());
        world.templates.insert(dozer.name.clone(), dozer);
        let mut pad = ThingTemplate::new("OwnedEconomyPad");
        pad.add_kind_of(KindOf::Structure).set_cost(100, 0);
        world.templates.insert(pad.name.clone(), pad);
        let dozer = world
            .create_object_for_player("OwnedEconomyDozer", 1, Vec3::new(25.0, 0.0, 25.0))
            .unwrap();
        world
            .ai_manager
            .ai_players
            .get_mut(&1)
            .unwrap()
            .add_building("OwnedEconomyPad", Vec3::new(100.0, 0.0, 100.0), 1);
        (world, dozer)
    }

    #[test]
    fn actual_scaffold_factory_observes_canonical_controllers_and_reconciles_reordered_pad() {
        let (mut world, dozer) = world();
        let data = AiDataView::from_world(&world);
        let mut ai = world.ai_manager.ai_players.remove(&1).unwrap();
        let attempt = ai.prepare_scaffold_attempt(&world, 0.0, &data).unwrap();
        world.ai_manager.ai_players.insert(1, ai);
        // A synchronous owner effect can reorder the authored list. The
        // continuation identifies its row rather than trusting an old index.
        world
            .ai_manager
            .ai_players
            .get_mut(&1)
            .unwrap()
            .building_queue
            .insert(0, AIBuildingInfo::new("OtherPad".into(), Vec3::ZERO, 1));
        let mut observed = false;
        let structure = attempt
            .execute_observing(&mut world, |world, _| {
                assert_eq!(world.ai_manager.ai_players.len(), 2);
                assert_eq!(world.ai_manager.ai_players[&1].player_id, 1);
                assert_eq!(world.ai_manager.ai_players[&2].player_id, 2);
                observed = true;
            })
            .unwrap();
        assert!(observed);
        world
            .ai_manager
            .ai_players
            .get_mut(&1)
            .unwrap()
            .finish_scaffold_attempt(&attempt, structure);
        let pads = &world.ai_manager.ai_players[&1].building_queue;
        assert!(pads[0].object_id.is_none());
        assert_eq!(pads[1].object_id, Some(structure));
        assert_eq!(pads[1].rebuild_count, 1);
        assert_eq!(world.get_player(1).unwrap().resources.supplies, 9900);
        assert_eq!(world.host_object(dozer).unwrap().target, Some(structure));
        assert!(
            world
                .host_object(structure)
                .unwrap()
                .status
                .under_construction
        );
    }

    #[test]
    fn owned_economic_pass_builds_one_paid_scaffold_with_peer_unchanged() {
        let (mut world, _) = world();
        let data = AiDataView::from_world(&world);
        world.update_owned_ai_economy(1, 0.0, &data);
        assert!(
            world.ai_manager.ai_players[&1].building_queue[0]
                .object_id
                .is_some()
        );
        assert!(world.ai_manager.ai_players[&2].building_queue.is_empty());
        assert_eq!(world.get_player(1).unwrap().resources.supplies, 9900);
        assert_eq!(world.get_player(2).unwrap().resources.supplies, 10000);
        assert_eq!(world.ai_manager.ai_players.len(), 2);
    }
}
