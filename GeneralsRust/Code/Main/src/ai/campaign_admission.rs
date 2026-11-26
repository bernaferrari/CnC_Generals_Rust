//! PlayerList admission for ordinary campaign controllers.
use super::*;
use gamelogic::{build_list_info::BuildListInfo, player::GameDifficulty};

impl GameLogic {
    /// Player.cpp:736–755, 791–819. The outgoing TeamFactory was already
    /// retired when this map admitted its named PlayerList.
    pub(crate) fn admit_direct_map_ai_controllers(&mut self, difficulty: Option<GameDifficulty>) {
        let difficulty = match difficulty {
            Some(GameDifficulty::Easy) => AIDifficulty::Easy,
            None | Some(GameDifficulty::Normal) => AIDifficulty::Medium,
            Some(GameDifficulty::Hard) => AIDifficulty::Hard,
            Some(GameDifficulty::Brutal) => AIDifficulty::Brutal,
        };
        self.ai_manager.ai_players.clear();
        let settings = self.ai_definitions.data();
        let team_seconds = settings.team_seconds as i32;
        let force_skirmish = settings.force_skirmish_ai;
        let mut computer_players: Vec<_> = self
            .get_players()
            .values()
            .filter(|player| player.is_computer())
            .map(|player| player.id)
            .collect();
        computer_players.sort_unstable();
        for id in computer_players {
            let player = self.get_player_mut(id).expect("admitted map player");
            let team = player.team;
            let skirmish = player.map_side.is_skirmish_ai || force_skirmish;
            player.map_side.is_skirmish_ai = skirmish;
            player.set_can_build_units(skirmish);
            if skirmish {
                self.ai_manager.add_ai_player(id, team, difficulty);
                continue;
            }

            let mut ai =
                AIPlayer::new_with_team_factory(id, team, difficulty, self.team_factory.clone());
            ai.base_radius = 0.0;
            ai.team_seconds = team_seconds as f32;
            ai.ordinary_timing = Some(OrdinaryAiTiming {
                ready_to_build_team: false,
                ready_to_build_structure: false,
                team_timer: 2,
                structure_timer: 2,
                build_delay: 0,
                team_delay: 0,
                team_seconds,
                frame_last_building_built: self.frame as i32,
                bridge_timer: 0,
            });
            self.ai_manager.ai_players.insert(id, ai);
        }
    }

    /// AIPlayer.cpp:3010–3046. Visit the original authored rows after
    /// prepending live factories. Each row keeps its identity across scripts.
    pub(super) fn new_map_owned_ordinary_ai(
        &mut self,
        player: u32,
        effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> generals_sim::GameLogicResult<()> {
        self.prepend_owned_campaign_factories(player);
        let Some(ai) = self.ai_manager.ai_players.get_mut(&player) else {
            return Ok(());
        };
        let visits: Vec<_> = (0..ai.building_queue.len())
            .filter_map(|index| {
                ai.building_queue[index].completion.as_ref()?;
                ai.campaign_build_row_visit(index)
            })
            .collect();

        for token in visits {
            let Some((index, row)) = self.ai_manager.ai_players.get_mut(&player).and_then(|ai| {
                ai.building_queue
                    .iter_mut()
                    .enumerate()
                    .find(|(_, row)| row.runtime_visit_token == token)
            }) else {
                continue;
            };
            let (Some(ordinary), Some(completion)) = (&row.ordinary, &row.completion) else {
                continue;
            };
            if row.template_name.is_empty() || !self.templates.contains_key(&row.template_name) {
                continue;
            }
            if ordinary.initially_built {
                let authored = crate::game_logic::HostAuthoredBuild {
                    template: row.template_name.clone(),
                    position: (row.position.x, row.position.z, row.position.y),
                    angle: ordinary.angle,
                    num_rebuilds: row.max_rebuilds,
                    initially_built: true,
                    completion: completion.clone(),
                };
                self.build_prepared_campaign_initial_structure(player, index, &authored, effects)?;
            } else if row.max_rebuilds != UNLIMITED_REBUILDS {
                row.max_rebuilds += 1;
            }
        }
        Ok(())
    }
}

impl AIBuildingInfo {
    pub(crate) fn from_campaign_build_list(entry: &BuildListInfo) -> Self {
        let location = entry.get_location();
        let rally = entry.get_rally_offset();
        let object_id = object_id_from_build_list(entry.get_object_id());
        let mut row = Self::new(
            entry.get_template_name().to_string(),
            Vec3::new(location.x, location.z, location.y),
            entry.get_num_rebuilds(),
        );
        row.object_id = object_id;
        row.is_built = object_id.is_some() && !entry.is_under_construction();
        row.is_priority = entry.is_priority_build();
        row.automatic_build = entry.is_automatic_build();
        row.supply_building = entry.is_supply_building();
        row.desired_gatherers = entry.get_desired_gatherers();
        row.current_gatherers = entry.get_current_gatherers();
        row.completion = Some(AuthoredStructureCompletion {
            name: entry.get_building_name().to_string(),
            script: entry.get_script().to_string(),
            health_percent: entry.get_health(),
            unsellable: entry.get_unsellable(),
        });
        row.ordinary = Some(Box::new(OrdinaryBuildListFields {
            rally_point_offset: [rally.x, rally.y],
            angle: entry.get_angle(),
            initially_built: entry.is_initially_built(),
            whiner: entry.get_whiner(),
            repairable: entry.get_repairable(),
            object_timestamp: entry.get_object_timestamp(),
            under_construction: entry.is_under_construction(),
            resource_gatherers: std::array::from_fn(|index| {
                object_id_from_build_list(entry.get_gatherer_id(index as i32))
            }),
        }));
        row
    }

    pub(super) fn preplaced_campaign_factory(object: &Object) -> Self {
        let mut row = Self::new(object.template_name.clone(), object.get_position(), 0);
        row.object_id = Some(object.id);
        row.is_built = object.is_constructed();
        row.set_ordinary_defaults(object.get_orientation());
        row
    }

    pub(super) fn set_ordinary_defaults(&mut self, angle: f32) {
        self.ordinary = Some(Box::new(OrdinaryBuildListFields {
            rally_point_offset: [0.0; 2],
            angle,
            initially_built: false,
            whiner: true,
            repairable: true,
            object_timestamp: 0,
            under_construction: false,
            resource_gatherers: [None; BuildListInfo::MAX_RESOURCE_GATHERERS],
        }));
    }
}

fn object_id_from_build_list(id: gamelogic::common::ObjectID) -> Option<ObjectId> {
    (id != gamelogic::common::INVALID_OBJECT_ID).then_some(ObjectId(id))
}
