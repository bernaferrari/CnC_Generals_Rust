//! Host tick `impl GameLogic` — `presence`.
#![allow(unused_imports, non_snake_case)]
use super::super::*;
impl GameLogic {
    pub(in super::super) fn ensure_non_shell_player_presence(
        &mut self,
        parsed_settings: Option<&super::super::script_loader::MapMetadata>,
    ) {
        if self.game_mode == GameMode::Shell {
            return;
        }
        // Wave 732: free fallback CC/dozer seeding is opt-in only (default fail-closed).
        // Retail maps must supply start objects. Vertical-slice/incomplete catalogs may set
        // GENERALS_RUNTIME_HOST_SEED_START_PRESENCE=1.
        let allow_seed =
            std::env::var_os("GENERALS_RUNTIME_HOST_SEED_START_PRESENCE").is_some_and(|v| {
                let s = v.to_string_lossy();
                !(s.is_empty()
                    || s == "0"
                    || s.eq_ignore_ascii_case("false")
                    || s.eq_ignore_ascii_case("no"))
            });
        if !allow_seed {
            return;
        }

        let mut team_order = Vec::new();
        let mut player_ids: Vec<u32> = self.players.keys().copied().collect();
        player_ids.sort_unstable();
        for player_id in player_ids {
            let Some(player) = self.players.get(&player_id) else {
                continue;
            };
            if player.team == Team::Neutral || team_order.contains(&player.team) {
                continue;
            }
            team_order.push(player.team);
        }
        if team_order.is_empty() {
            return;
        }

        let default_bounds_min = Vec3::new(-300.0, 0.0, -300.0);
        let default_bounds_max = Vec3::new(300.0, 0.0, 300.0);
        let (bounds_min, bounds_max) = parsed_settings
            .and_then(|meta| {
                meta.world_min.zip(meta.world_max).map(|(min, max)| {
                    (
                        Vec3::new(min.x, min.y, min.z),
                        Vec3::new(max.x, max.y, max.z),
                    )
                })
            })
            .filter(|(min, max)| (max.x - min.x).abs() >= 1.0 && (max.z - min.z).abs() >= 1.0)
            .unwrap_or((default_bounds_min, default_bounds_max));

        let span = bounds_max - bounds_min;
        let spawn_positions = [
            Vec3::new(
                bounds_min.x + span.x * 0.20,
                0.0,
                bounds_min.z + span.z * 0.20,
            ),
            Vec3::new(
                bounds_max.x - span.x * 0.20,
                0.0,
                bounds_max.z - span.z * 0.20,
            ),
            Vec3::new(
                bounds_max.x - span.x * 0.20,
                0.0,
                bounds_min.z + span.z * 0.20,
            ),
            Vec3::new(
                bounds_min.x + span.x * 0.20,
                0.0,
                bounds_max.z - span.z * 0.20,
            ),
            Vec3::new(
                bounds_min.x + span.x * 0.50,
                0.0,
                bounds_min.z + span.z * 0.15,
            ),
            Vec3::new(
                bounds_min.x + span.x * 0.50,
                0.0,
                bounds_max.z - span.z * 0.15,
            ),
        ];

        let mut spawned_count = 0usize;
        for (index, team) in team_order.into_iter().enumerate() {
            let has_presence = self
                .objects
                .values()
                .any(|object| object.team == team && object.is_alive());
            if has_presence {
                continue;
            }

            let mut spawn_position = spawn_positions[index % spawn_positions.len()];
            if let Some(ground_height) =
                self.terrain_height_at(Vec3::new(spawn_position.x, 0.0, spawn_position.z))
            {
                spawn_position.y = ground_height;
            }

            // Ensure faction template catalog (CC, barracks, combat units) exists.
            self.ensure_ai_faction_templates(team);

            let primary_template = match team {
                Team::USA => "USA_CommandCenter",
                Team::GLA => "GLA_CommandCenter",
                Team::China => "China_CommandCenter",
                Team::Neutral => "CommandCenter",
            };

            if self
                .create_object(primary_template, team, spawn_position)
                .is_none()
            {
                // Fallbacks for incomplete catalogs.
                let _ = self
                    .create_object("CommandCenter", team, spawn_position)
                    .or_else(|| self.create_object("GoldenCC", team, spawn_position));
            }
            // Seed a worker for the first human-capable team so DozerConstruct can start.
            if index == 0 {
                let dozer_pos = spawn_position + Vec3::new(20.0, 0.0, 0.0);
                let dozer_name = match team {
                    Team::USA => "USA_Dozer",
                    Team::China => "China_Dozer",
                    Team::GLA => "GLA_Worker",
                    Team::Neutral => "GoldenDozer",
                };
                if self.create_object(dozer_name, team, dozer_pos).is_none() {
                    // Install a minimal dozer if faction worker missing.
                    if !self.templates.contains_key("GoldenDozer") {
                        let mut d = ThingTemplate::new("GoldenDozer");
                        d.set_health(300.0);
                        d.set_cost(1000, 0);
                        d.add_kind_of(KindOf::Vehicle);
                        d.add_kind_of(KindOf::Worker);
                        d.add_kind_of(KindOf::Selectable);
                        self.templates.insert("GoldenDozer".into(), d);
                    }
                    let _ = self.create_object("GoldenDozer", team, dozer_pos);
                }
            }
            spawned_count += 1;
        }

        if spawned_count > 0 {
            log::info!(
                "Seeded {} fallback player start structures for non-shell map '{}'",
                spawned_count,
                self.map_name
            );
        }
    }

    pub(in super::super) fn configure_victory_rules_for_map(&mut self, map_name: &str) {
        let rules = victory_rules_for_map(map_name);
        self.victory_conditions.set_victory_conditions(rules);
        log::info!(
            "Configured victory rules for map '{}': require units = {}, require buildings = {}",
            map_name,
            rules.requires_units(),
            rules.requires_buildings()
        );
    }

    pub(in super::super) fn convert_script_event(&self, event: &ScriptEvent) -> Option<MissionScriptEvent> {
        use ScriptValue as Val;
        let mut params = HashMap::new();
        let event_type = match event {
            ScriptEvent::PlayerDefeated { player_id } => {
                params.insert("player_id".into(), Val::PlayerId(*player_id));
                "player_defeated"
            }
            ScriptEvent::AllianceStateChanged { player_id, state } => {
                params.insert("player_id".into(), Val::PlayerId(*player_id));
                params.insert(
                    "state".into(),
                    Val::String(format!("{:?}", state).to_lowercase()),
                );
                "alliance_state_changed"
            }
            ScriptEvent::RevealMapForPlayer { player_id } => {
                params.insert("player_id".into(), Val::PlayerId(*player_id));
                "reveal_map_for_player"
            }
            ScriptEvent::CompletedSpecialPower {
                player_id,
                special_power_name,
                creator_id,
            } => {
                params.insert("player_id".into(), Val::PlayerId(*player_id));
                params.insert(
                    "special_power".into(),
                    Val::String(special_power_name.clone()),
                );
                params.insert("creator_id".into(), Val::String(creator_id.to_string()));
                "completed_special_power"
            }
        };

        Some(MissionScriptEvent {
            event_type: event_type.to_string(),
            parameters: params,
            timestamp: std::time::Instant::now(),
            priority: ScriptPriority::Normal,
        })
    }
}
