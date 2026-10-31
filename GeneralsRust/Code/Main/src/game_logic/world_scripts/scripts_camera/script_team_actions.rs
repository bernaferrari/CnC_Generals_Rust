//! Host script team, containment, trigger, and attack behavior.
#![allow(unused_imports, non_snake_case)]
use super::*;

impl GameLogic {
    /// C++ `doTeamGuardPosition` / `Object` / `Area` / `InTunnelNetwork`.
    /// Leftover queues [`gamelogic::scripting::HostScriptGuardVariantRequest`].
    pub(super) fn apply_host_guard_variant_script_requests(&mut self) {
        self.apply_host_guard_variant_script_requests_with_effects(&mut Default::default())
    }

    pub(super) fn apply_host_guard_variant_script_requests_with_effects(
        &mut self,
        effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) {
        use gamelogic::scripting::HostScriptGuardVariantRequest;
        for req in gamelogic::scripting::take_host_script_guard_variant_requests() {
            match req {
                HostScriptGuardVariantRequest::TeamGuardPosition { team, waypoint } => {
                    let Some(dest) = self.host_script_waypoint_position(&waypoint) else {
                        continue;
                    };
                    for id in self.host_script_team_member_ids(&team) {
                        if self.host_script_unit_can_guard(id) {
                            let _ =
                                self.unit_command_guard_position_with_effects(id, dest, effects);
                        }
                    }
                }
                HostScriptGuardVariantRequest::TeamGuardObject { team, unit } => {
                    let Some(tid) = self.host_object_id_by_script_name(&unit) else {
                        continue;
                    };
                    for id in self.host_script_team_member_ids(&team) {
                        if self.host_script_unit_can_guard(id) {
                            let _ = self.unit_command_guard_object(id, tid);
                        }
                    }
                }
                HostScriptGuardVariantRequest::TeamGuardArea { team, area } => {
                    let Some(dest) = self.host_script_area_center(&area) else {
                        continue;
                    };
                    let poly_radius = self.host_named_guard_area_polygon(&area).map(|(_, r, _)| r);
                    for id in self.host_script_team_member_ids(&team) {
                        if self.host_script_unit_can_guard(id) {
                            let _ =
                                self.unit_command_guard_position_with_effects(id, dest, effects);
                            if let Some(u) = self.objects.get_mut(&id) {
                                u.set_guard_area_trigger(Some(area.clone()));
                                if let Some(r) = poly_radius {
                                    if r > 0.0 {
                                        u.guard_radius = r;
                                    }
                                }
                            }
                        }
                    }
                }
                HostScriptGuardVariantRequest::TeamGuardTunnel { team } => {
                    for id in self.host_script_team_member_ids(&team) {
                        if !self.host_script_unit_can_guard(id) {
                            continue;
                        }
                        if let Some(tid) = self.host_script_nearest_tunnel(id) {
                            let _ = self.unit_command_guard_object(id, tid);
                        }
                    }
                }
            }
        }
    }

    /// C++ `doNamedFireSpecialPowerAtWaypoint` / `AtNamed`.
    /// Leftover queues [`gamelogic::scripting::HostScriptNamedFireSpecialPowerRequest`].
    pub(super) fn apply_host_named_fire_special_script_requests(&mut self) {
        use crate::command_system::PowerTarget;
        use gamelogic::scripting::HostScriptNamedFireSpecialPowerRequest;
        for req in gamelogic::scripting::take_host_script_named_fire_special_requests() {
            match req {
                HostScriptNamedFireSpecialPowerRequest::AtWaypoint {
                    unit,
                    power,
                    waypoint,
                } => {
                    let Some((wid, dest)) = self.host_script_leftover_waypoint(&waypoint) else {
                        continue;
                    };
                    self.host_script_fire_named_special_power(
                        &unit,
                        &power,
                        PowerTarget::Location(dest),
                        Some(wid),
                    );
                }
                HostScriptNamedFireSpecialPowerRequest::AtNamed {
                    unit,
                    power,
                    target,
                } => {
                    let Some(tid) = self.host_object_id_by_script_name(&target) else {
                        continue;
                    };
                    self.host_script_fire_named_special_power(
                        &unit,
                        &power,
                        PowerTarget::Object(tid),
                        None,
                    );
                }
            }
        }
    }

    /// C++ `Object::getSpecialPowerModule(TheSpecialPowerStore->find…)`.
    pub(super) fn host_script_special_power_type_for(
        &self,
        id: ObjectId,
        power_name: &str,
    ) -> Option<crate::command_system::SpecialPowerType> {
        let obj = self.host_object(id)?;
        for module in &obj.thing().template.special_power_modules {
            if module
                .special_power_template
                .eq_ignore_ascii_case(power_name)
            {
                if let Some(power) = module.command_power.clone() {
                    return Some(power);
                }
                return crate::command_system::special_power_type_from_template_name(power_name);
            }
        }
        let power = crate::command_system::special_power_type_from_template_name(power_name)?;
        if obj
            .thing()
            .template
            .special_power_module_for_command(&power)
            .is_some()
            || obj.special_power_cooldowns.contains_key(&power)
        {
            Some(power)
        } else {
            None
        }
    }

    /// C++ `mod->doSpecialPowerAtLocation/Object(..., COMMAND_FIRED_BY_SCRIPT)`.
    /// When `waypoint_id` is set, PUC leftover `scriptedWaypointMode` drives
    /// the outgoing chain instead of SwathOfDeath on a static point.
    pub(super) fn host_script_fire_named_special_power(
        &mut self,
        unit: &str,
        power_name: &str,
        target: crate::command_system::PowerTarget,
        waypoint_id: Option<u32>,
    ) {
        if let Some(id) = self.host_object_id_by_script_name(unit) {
            self.fire_receiving_named_special_power(
                id,
                power_name,
                target,
                waypoint_id,
                None,
                &mut Default::default(),
            );
        }
    }

    pub(super) fn fire_receiving_named_special_power(
        &mut self,
        id: ObjectId,
        power_name: &str,
        target: crate::command_system::PowerTarget,
        waypoint_id: Option<u32>,
        interpreter: Option<&gamelogic::scripting::engine::ScriptEngine>,
        effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) {
        let Some(power) = self.host_script_special_power_type_for(id, power_name) else {
            return;
        };
        let player_id = {
            let Some(obj) = self.host_object(id) else {
                return;
            };
            if !obj.is_alive() || obj.status.destroyed || obj.status.sold || obj.is_disabled() {
                return;
            }
            if obj.is_special_power_countdown_paused(&power) {
                return;
            }
            obj.owner_player_id.unwrap_or(0)
        };
        // C++ script fire does not consult isReady; only paused/disabled.
        if !self.is_special_power_ready_for(id, &power) {
            let _ = self.script_set_special_power_countdown(id, &power, 0);
            if let Some(obj) = self.host_object(id) {
                if let Some(pid) = self.player_owner_for_host_object(obj) {
                    if let Some(player) = self.get_player_mut(pid) {
                        player.express_shared_special_power_ready_now(&power);
                    }
                }
            }
        }
        // C++ doSpecialPower* COMMAND_FIRED_BY_SCRIPT — location-only stays swath.
        self.special_power_strikes
            .note_script_fired_special_power(id);
        if let Some(wid) = waypoint_id {
            if crate::game_logic::special_power_strikes::HostSuperweaponKind::from_command_power(
                &power,
            ) == Some(
                crate::game_logic::special_power_strikes::HostSuperweaponKind::ParticleCannon,
            ) {
                self.special_power_strikes
                    .note_scripted_waypoint_special_power(id, wid);
            }
        }
        let command = crate::command_system::GameCommand {
            command_type: crate::command_system::CommandType::DoSpecialPower {
                power_type: power,
                target,
            },
            player_id,
            command_id: 0,
            timestamp: std::time::SystemTime::now(),
            selected_units: vec![id],
            modifier_keys: crate::command_system::ModifierKeys::default(),
        };
        if let Some(interpreter) = interpreter {
            let _ = crate::command_executor::CommandExecutor::new_with_target_policy_and_effects(
                self,
                player_id,
                interpreter,
                effects.reborrow(),
            )
            .execute_command(command);
        } else {
            self.queue_command(command);
        }
    }

    pub(super) fn host_script_unit_can_guard(&self, id: ObjectId) -> bool {
        // C++ doTeamGuard / leftover group_guard: AIUpdateInterface only.
        self.host_unit_can_guard(id)
    }

    /// C++ AITNGuardMachine nearest entrance for `aiGuardTunnelNetwork`.
    pub(super) fn host_script_nearest_tunnel(&self, from: ObjectId) -> Option<ObjectId> {
        let obj = self.objects.get(&from)?;
        let origin = obj.get_position();
        let key = obj.tunnel_system_key();
        self.objects
            .registered()
            .filter(|(_, o)| {
                o.is_alive()
                    && !o.status.sold
                    && o.tunnel_system_key() == key
                    && (o.is_tunnel_network_style_container()
                        || crate::game_logic::host_tunnel_network::is_tunnel_network_template(
                            &o.template_name,
                        ))
            })
            .min_by(|a, b| {
                origin
                    .distance(a.1.get_position())
                    .partial_cmp(&origin.distance(b.1.get_position()))
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
            .map(|(id, _)| *id)
    }

    /// C++ ScriptActions CREATE_OBJECT family live drain.
    pub(in crate::game_logic::game_logic) fn apply_host_create_script_requests(&mut self) {
        self.apply_host_create_script_requests_with_interpreter(None);
    }

    pub(super) fn apply_host_create_script_requests_with_interpreter(
        &mut self,
        interpreter: Option<&gamelogic::scripting::engine::ScriptEngine>,
    ) {
        self.apply_host_create_script_requests_with_effects(interpreter, &mut Default::default())
    }

    pub(super) fn apply_host_create_script_requests_with_effects(
        &mut self,
        interpreter: Option<&gamelogic::scripting::engine::ScriptEngine>,
        ui: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) {
        use gamelogic::scripting::HostScriptCreateRequest;
        for req in gamelogic::scripting::take_host_script_create_requests() {
            match req {
                HostScriptCreateRequest::Object {
                    name,
                    thing,
                    team,
                    x,
                    y,
                    z,
                    angle,
                } => {
                    self.host_script_create_object_with_effects(
                        name.as_deref(),
                        &thing,
                        &team,
                        x,
                        y,
                        z,
                        angle,
                        interpreter,
                        ui,
                    );
                }
                HostScriptCreateRequest::ReinforcementTeam { team, waypoint } => {
                    self.host_script_create_reinforcement_team_with_effects(
                        &team,
                        &waypoint,
                        interpreter,
                        ui,
                    );
                }
            }
        }
    }

    /// C++ `ScriptActions::doNamedSetBoobytrapped` / `doTeamSetBoobytrapped`.
    /// Leftover queues [`gamelogic::scripting::HostScriptBoobytrapRequest`].
    pub(super) fn apply_host_boobytrap_script_requests(
        &mut self,
    ) -> gamelogic::GameLogicResult<()> {
        self.apply_host_boobytrap_script_requests_with_effects(None, &mut Default::default())
    }

    pub(super) fn apply_host_boobytrap_script_requests_with_effects(
        &mut self,
        interpreter: Option<&gamelogic::scripting::engine::ScriptEngine>,
        effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> gamelogic::GameLogicResult<()> {
        use gamelogic::scripting::HostScriptBoobytrapRequest;
        for req in gamelogic::scripting::take_host_script_boobytrap_requests() {
            match req {
                HostScriptBoobytrapRequest::Named { thing, unit } => {
                    if let Some(id) = self.host_object_id_by_script_name(&unit) {
                        self.host_script_plant_boobytrap_with_effects(
                            &thing,
                            id,
                            interpreter,
                            effects,
                        )?;
                    }
                }
                HostScriptBoobytrapRequest::Team { thing, team } => {
                    let members = match self.resolve_owned_script_team(&team, None, None) {
                        Ok(Some((_, members))) => members,
                        Ok(None) => continue,
                        Err(error) => {
                            log::error!("Cannot resolve scripted boobytrap Team: {error}");
                            continue;
                        }
                    };
                    for member in members {
                        self.host_script_plant_boobytrap_with_effects(
                            &thing,
                            ObjectId(member),
                            interpreter,
                            effects,
                        )?;
                    }
                }
            }
        }
        Ok(())
    }

    /// C++ `ScriptActions::doNamedSetUnmanned` / `doTeamSetUnmanned` /
    /// `deleteAllUnmanned`. Leftover queues [`gamelogic::scripting::HostScriptUnmannedRequest`].
    pub(in crate::game_logic::game_logic) fn apply_host_unmanned_script_requests(&mut self) {
        self.apply_host_unmanned_script_requests_with_effects(&mut Default::default())
    }

    pub(in crate::game_logic::game_logic) fn apply_host_unmanned_script_requests_with_effects(
        &mut self,
        effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) {
        use gamelogic::scripting::HostScriptUnmannedRequest;
        for req in gamelogic::scripting::take_host_script_unmanned_requests() {
            match req {
                HostScriptUnmannedRequest::Named { unit } => {
                    if let Some(id) = self.host_object_id_by_script_name(&unit) {
                        self.host_script_set_unmanned(id);
                    }
                }
                HostScriptUnmannedRequest::Team { team } => {
                    for id in self.host_script_team_member_ids(&team) {
                        self.host_script_set_unmanned(id);
                    }
                }
                HostScriptUnmannedRequest::DeleteAll => {
                    let ids: Vec<ObjectId> = self
                        .objects
                        .registered_values()
                        .filter(|obj| obj.status.disabled_unmanned && !obj.status.destroyed)
                        .map(|obj| obj.id)
                        .collect();
                    for id in ids {
                        self.destroy_object_with_effects(id, effects);
                    }
                }
            }
        }
    }

    /// C++ `setDisabled(DISABLED_UNMANNED)` + `deselectObject(PLAYERMASK_ALL)` +
    /// `setTeam(Neutral default team)`.
    pub(super) fn host_script_set_unmanned(&mut self, id: ObjectId) {
        {
            let Some(obj) = self.objects.get_mut(&id) else {
                return;
            };
            if !obj.is_alive() || obj.status.destroyed {
                return;
            }
            obj.apply_kill_pilot_unmanned_at_frame(
                &mut self.health_events,
                &mut crate::game_logic::ObjectAudio::queue(&mut self.queued_audio_events),
                self.frame,
            );
            obj.deselect();
            obj.set_team_with_partition_owner(
                Team::Neutral,
                &mut self.ownership_events,
                &mut self.partition_manager,
            );
        }
        self.admit_host_radar_disabled_edges(id);
        self.admit_local_drawable_deselected(id);
        for player in self.players.values_mut() {
            player.selected_objects.retain(|sid| *sid != id);
        }
    }

    /// C++ `doObjectRadarCreateEvent` / `doTeamRadarCreateEvent`.
    pub(in crate::game_logic::game_logic) fn apply_host_radar_event_script_requests(&mut self) {
        use gamelogic::scripting::HostScriptRadarEventRequest;
        for req in gamelogic::scripting::take_host_script_radar_event_requests() {
            let (pos, event_type) = match req {
                HostScriptRadarEventRequest::Object { unit, event_type } => {
                    let Some(id) = self.host_object_id_by_script_name(&unit) else {
                        continue;
                    };
                    let Some(obj) = self.objects.get(&id) else {
                        continue;
                    };
                    (obj.get_position(), event_type)
                }
                HostScriptRadarEventRequest::Team { team, event_type } => {
                    let Some(pos) = self.host_script_estimate_team_position(&team) else {
                        continue;
                    };
                    (pos, event_type)
                }
            };
            self.host_create_radar_event(pos, Self::host_script_radar_event_type(event_type));
        }
    }

    pub(in crate::game_logic::game_logic) fn host_script_radar_event_type(
        event_type: i32,
    ) -> game_engine::common::system::radar::RadarEventType {
        use game_engine::common::system::radar::RadarEventType;
        match event_type {
            1 => RadarEventType::Construction,
            2 => RadarEventType::Upgrade,
            3 => RadarEventType::UnderAttack,
            4 => RadarEventType::Information,
            5 => RadarEventType::BeaconPulse,
            6 => RadarEventType::Infiltration,
            7 => RadarEventType::BattlePlan,
            8 => RadarEventType::StealthDiscovered,
            9 => RadarEventType::StealthNeutralized,
            10 => RadarEventType::Fake,
            _ => RadarEventType::Invalid,
        }
    }

    /// C++ `Team::getEstimateTeamPosition` — first living member, not centroid.
    pub(super) fn host_script_estimate_team_position(&self, team_name: &str) -> Option<glam::Vec3> {
        let ids = self.host_script_team_member_ids(team_name);
        let mut first: Option<(u32, glam::Vec3)> = None;
        for id in ids {
            let Some(obj) = self.objects.get(&id) else {
                continue;
            };
            if !obj.is_alive() || obj.status.destroyed {
                continue;
            }
            if first.map(|(oid, _)| id.0 < oid).unwrap_or(true) {
                first = Some((id.0, obj.get_position()));
            }
        }
        first.map(|(_, pos)| pos)
    }

    /// C++ `doNamedEnableStealth` / `doTeamEnableStealth`.
    pub(in crate::game_logic::game_logic) fn apply_host_stealth_enabled_script_requests(&mut self) {
        use gamelogic::scripting::HostScriptStealthEnabledRequest;
        for req in gamelogic::scripting::take_host_script_stealth_enabled_requests() {
            match req {
                HostScriptStealthEnabledRequest::Named { unit, enabled } => {
                    if let Some(id) = self.host_object_id_by_script_name(&unit) {
                        self.host_script_set_stealth_enabled(id, enabled);
                    }
                }
                HostScriptStealthEnabledRequest::Team { team, enabled } => {
                    for id in self.host_script_team_member_ids(&team) {
                        self.host_script_set_stealth_enabled(id, enabled);
                    }
                }
            }
        }
    }

    /// C++ `setScriptStatus(OBJECT_STATUS_SCRIPT_UNSTEALTHED, !enabled)`.
    pub(super) fn host_script_set_stealth_enabled(&mut self, id: ObjectId, enabled: bool) {
        let frame = self.frame;
        let Some(obj) = self.objects.get_mut(&id) else {
            return;
        };
        obj.set_script_unstealthed(!enabled);
        if !enabled {
            obj.apply_stealth_allowed_update(frame, false);
        }
    }

    /// C++ `TheThingFactory->newObject(thing, obj->getTeam())` then
    /// `StickyBombUpdate::initStickyBomb(obj, NULL, &perimeterPos)`.
    pub(super) fn host_script_plant_boobytrap(
        &mut self,
        thing: &str,
        target_id: ObjectId,
    ) -> gamelogic::GameLogicResult<()> {
        self.host_script_plant_boobytrap_with_effects(
            thing,
            target_id,
            None,
            &mut Default::default(),
        )
    }

    pub(super) fn host_script_plant_boobytrap_with_effects(
        &mut self,
        thing: &str,
        target_id: ObjectId,
        interpreter: Option<&gamelogic::scripting::engine::ScriptEngine>,
        effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> gamelogic::GameLogicResult<()> {
        self.create_script_sticky_bomb_with_effects(thing, target_id, interpreter, effects)?;
        Ok(())
    }

    /// C++ `ScriptActions::doGuardSupplyCenter` live drain.
    pub(super) fn apply_host_guard_supply_center_script_requests(&mut self) {
        let requests = gamelogic::scripting::take_host_guard_supply_center_requests();
        if requests.is_empty() {
            return;
        }
        let mut ai_mgr = std::mem::take(&mut self.ai_manager);
        for (team_name, min_supplies) in requests {
            let _ = ai_mgr.guard_supply_center_for_team(self, &team_name, min_supplies);
        }
        self.ai_manager = ai_mgr;
    }

    /// C++ SKIRMISH_ATTACK_NEAREST_GROUP_WITH_VALUE /
    /// SKIRMISH_PERFORM_COMMANDBUTTON_ON_MOST_VALUABLE_OBJECT.
    pub(super) fn apply_host_skirmish_fight_script_requests(&mut self) {
        self.apply_host_skirmish_fight_script_requests_with_effects(&mut Default::default())
    }

    pub(super) fn apply_host_skirmish_fight_script_requests_with_effects(
        &mut self,
        effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) {
        self.with_owned_script_engine(|engine, world| {
            world.apply_host_skirmish_fight_script_requests_with_policy(effects, engine);
        })
        .expect("skirmish command-button drain owns its receiving interpreter");
    }

    pub(super) fn apply_host_skirmish_fight_script_requests_with_policy(
        &mut self,
        effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
        interpreter: &gamelogic::scripting::engine::ScriptEngine,
    ) {
        let attacks = gamelogic::scripting::take_host_skirmish_attack_nearest_group_requests();
        for (team, comparison, value) in attacks {
            let members = self.host_script_team_member_ids(&team);
            if members.is_empty() {
                continue;
            }
            let mut cx = 0.0;
            let mut cz = 0.0;
            let mut n = 0.0;
            for id in &members {
                if let Some(obj) = self.objects.get(id) {
                    let p = obj.get_position();
                    cx += p.x;
                    cz += p.z;
                    n += 1.0;
                }
            }
            if n <= 0.0 {
                continue;
            }
            let origin = glam::Vec3::new(cx / n, 0.0, cz / n);
            let attacker_pid = self
                .objects
                .get(&members[0])
                .and_then(|o| o.owner_player_id)
                .unwrap_or(0);
            let mut dest = origin;
            if matches!(comparison, 3 | 4) {
                if let Some(pos) =
                    self.host_script_nearest_enemy_group_with_value(attacker_pid, origin, value)
                {
                    dest = pos;
                }
            }
            for id in members {
                let _ = self.unit_command_attack_move_to_with_effects(id, dest, effects);
            }
        }

        let buttons =
            gamelogic::scripting::take_host_skirmish_command_button_most_valuable_requests();
        for (team, ability, range) in buttons {
            self.host_script_skirmish_command_button_on_most_valuable_with_policy(
                &team,
                &ability,
                range,
                interpreter,
            );
        }
    }

    /// C++ `ScriptActions::doSkirmishCommandButtonOnMostValuable` /
    /// leftover `do_skirmish_perform_command_button_on_most_valuable_object`.
    /// Find the scripted button, pick the most valuable valid target in range
    /// of the group center, then `groupDoCommandButtonAtObject`.
    pub(super) fn host_script_skirmish_command_button_on_most_valuable(
        &mut self,
        team: &str,
        ability: &str,
        range: f32,
    ) {
        self.with_owned_script_engine(|engine, world| {
            world.host_script_skirmish_command_button_on_most_valuable_with_policy(
                team, ability, range, engine,
            );
        })
        .expect("skirmish command-button operation owns its receiving interpreter");
    }

    fn host_script_skirmish_command_button_on_most_valuable_with_policy(
        &mut self,
        team: &str,
        ability: &str,
        range: f32,
        interpreter: &gamelogic::scripting::engine::ScriptEngine,
    ) {
        use crate::command_executor::CommandExecutor;
        use gamelogic::object::update::special_power_update::SpecialPowerCommandOption;

        if !self.leftover_skirmish_command_button_exists(ability) {
            return;
        }
        let members = self.host_script_team_member_ids(team);
        if members.is_empty() {
            return;
        }
        let Some(center) = self.host_script_team_center(&members) else {
            return;
        };
        let pid = self
            .host_object(members[0])
            .and_then(|o| o.owner_player_id)
            .unwrap_or(0);
        let range2 = range.max(0.0) * range.max(0.0);
        let team_set: std::collections::HashSet<_> = members.iter().copied().collect();
        let options = self.leftover_skirmish_command_button_options(ability);
        let requires_object_target = options.intersects(
            SpecialPowerCommandOption::NEED_TARGET_ENEMY_OBJECT
                | SpecialPowerCommandOption::NEED_TARGET_NEUTRAL_OBJECT
                | SpecialPowerCommandOption::NEED_TARGET_ALLY_OBJECT
                | SpecialPowerCommandOption::NEED_TARGET_PRISONER,
        );

        let mut best: Option<(i32, crate::game_logic::ObjectId)> = None;
        for obj in self.objects.registered_values() {
            if !obj.is_alive() || obj.status.destroyed || obj.status.effectively_dead {
                continue;
            }
            if obj.status.under_construction {
                continue;
            }
            if team_set.contains(&obj.id) {
                continue;
            }
            let p = obj.get_position();
            let dx = p.x - center.x;
            let dz = p.z - center.z;
            if dx * dx + dz * dz > range2 {
                continue;
            }
            let rel = self.host_script_relationship(pid, obj);
            let relationship_ok = if requires_object_target {
                (options.contains(SpecialPowerCommandOption::NEED_TARGET_ENEMY_OBJECT)
                    && rel == gamelogic::common::Relationship::Enemies)
                    || (options.contains(SpecialPowerCommandOption::NEED_TARGET_NEUTRAL_OBJECT)
                        && rel == gamelogic::common::Relationship::Neutral)
                    || (options.contains(SpecialPowerCommandOption::NEED_TARGET_ALLY_OBJECT)
                        && rel == gamelogic::common::Relationship::Allies)
                    || (!options.intersects(
                        SpecialPowerCommandOption::NEED_TARGET_ENEMY_OBJECT
                            | SpecialPowerCommandOption::NEED_TARGET_NEUTRAL_OBJECT
                            | SpecialPowerCommandOption::NEED_TARGET_ALLY_OBJECT,
                    ) && rel == gamelogic::common::Relationship::Enemies)
            } else {
                rel == gamelogic::common::Relationship::Enemies
            };
            if !relationship_ok {
                continue;
            }
            let cost = obj.thing().template.build_cost.supplies as i32;
            if best.map(|(c, _)| cost > c).unwrap_or(true) {
                best = Some((cost, obj.id));
            }
        }
        let Some((_, tid)) = best else {
            return;
        };
        let _ = CommandExecutor::new_with_target_policy(self, pid, interpreter)
            .execute_do_command_button(&members, ability, None, Some(tid));
    }

    /// C++ `TheControlBar->findCommandButton` / leftover host-queue payload.
    /// Admitted action definitions belong to this receiving GameLogic. Only
    /// fixtures without an admitted catalog use the standalone compatibility lookup.
    pub(super) fn leftover_skirmish_command_button_exists(&self, ability: &str) -> bool {
        use crate::command_system::command_type_from_button_name;
        if ability.trim().is_empty() {
            return false;
        }
        if let Some(actions) = self
            .receiving_module_content
            .as_ref()
            .map(|content| content.action_definitions())
        {
            return actions
                .buttons
                .find_command_button_resolved(ability)
                .is_some();
        }
        if let Some(bar) = gamelogic::control_bar::get_control_bar_bridge() {
            if bar.find_command_button_by_name(ability).is_some() {
                return true;
            }
        }
        if let Some(bar) = game_engine::common::ini::ini_command_button::get_control_bar() {
            if bar.find_command_button_resolved(ability).is_some() {
                return true;
            }
            if !bar.get_button_names().is_empty() {
                return false;
            }
        }
        command_type_from_button_name(ability).is_some()
    }

    pub(super) fn leftover_skirmish_command_button_options(
        &self,
        ability: &str,
    ) -> gamelogic::object::update::special_power_update::SpecialPowerCommandOption {
        use gamelogic::object::update::special_power_update::SpecialPowerCommandOption;
        if let Some(actions) = self
            .receiving_module_content
            .as_ref()
            .map(|content| content.action_definitions())
        {
            return actions
                .buttons
                .find_command_button_resolved(ability)
                .map(|button| SpecialPowerCommandOption::from_bits_truncate(button.options_bits))
                .unwrap_or_else(|| SpecialPowerCommandOption::from_bits_truncate(0));
        }
        if let Some(bar) = gamelogic::control_bar::get_control_bar_bridge() {
            if let Some(btn) = bar.find_command_button_by_name(ability) {
                return SpecialPowerCommandOption::from_bits_truncate(btn.get_options_bits());
            }
        }
        if let Some(bar) = game_engine::common::ini::ini_command_button::get_control_bar() {
            if let Some(btn) = bar.find_command_button_resolved(ability) {
                return SpecialPowerCommandOption::from_bits_truncate(btn.options_bits);
            }
        }
        SpecialPowerCommandOption::from_bits_truncate(0)
    }

    pub(super) fn host_script_relationship(
        &self,
        viewer: u32,
        candidate: &crate::game_logic::Object,
    ) -> gamelogic::common::Relationship {
        use crate::game_logic::Team;
        use gamelogic::common::Relationship;
        let Some(oid) = candidate.owner_player_id else {
            let vt = self
                .players
                .get(&viewer)
                .map(|p| p.team)
                .unwrap_or(Team::Neutral);
            if candidate.team == Team::Neutral || vt == Team::Neutral {
                return Relationship::Neutral;
            }
            if candidate.team == vt {
                return Relationship::Allies;
            }
            return Relationship::Enemies;
        };
        self.players
            .get(&viewer)
            .and_then(|p| p.map_relationship(oid))
            .unwrap_or_else(|| {
                let vt = self
                    .players
                    .get(&viewer)
                    .map(|p| p.team)
                    .unwrap_or(candidate.team);
                let ot = self
                    .players
                    .get(&oid)
                    .map(|p| p.team)
                    .unwrap_or(candidate.team);
                if vt == ot {
                    Relationship::Allies
                } else if vt == Team::Neutral || ot == Team::Neutral {
                    Relationship::Neutral
                } else {
                    Relationship::Enemies
                }
            })
    }

    /// C++ `getPlayersWithRelationship(ALLOW_ENEMIES)` — player map, not faction Team.
    pub(super) fn host_script_enemy_player_mask(&self, attacker_pid: u32) -> u32 {
        use crate::game_logic::Team;
        use gamelogic::common::Relationship;
        let mut mask = 0u32;
        for &oid in self.players.keys() {
            if oid == attacker_pid {
                continue;
            }
            let rel = self
                .players
                .get(&attacker_pid)
                .and_then(|p| p.map_relationship(oid))
                .unwrap_or_else(|| {
                    let vt = self
                        .players
                        .get(&attacker_pid)
                        .map(|p| p.team)
                        .unwrap_or(Team::Neutral);
                    let ot = self
                        .players
                        .get(&oid)
                        .map(|p| p.team)
                        .unwrap_or(Team::Neutral);
                    if vt == Team::Neutral || ot == Team::Neutral {
                        Relationship::Neutral
                    } else {
                        Relationship::Enemies
                    }
                });
            if rel == Relationship::Enemies {
                mask |= 1u32 << oid.min(15);
            }
        }
        if mask == 0 {
            for obj in self.objects.registered_values() {
                if !obj.is_alive() {
                    continue;
                }
                let Some(pid) = obj.owner_player_id else {
                    continue;
                };
                if pid == attacker_pid {
                    continue;
                }
                let rel = self
                    .players
                    .get(&attacker_pid)
                    .and_then(|p| p.map_relationship(pid))
                    .unwrap_or(Relationship::Enemies);
                if rel == Relationship::Enemies {
                    mask |= 1u32 << pid.min(15);
                }
            }
        }
        mask
    }

    /// C++ `PartitionManager::getNearestGroupWithValue` / leftover cell cash strict `>`.
    /// ALLOW_ENEMIES player mask (relationship, not faction Team). Per-cell aggregate
    /// cash, BFS from the group center, dest is the cell corner.
    #[cfg(test)]
    pub(crate) fn fixture_nearest_enemy_scalar_group(
        &self,
        attacker: u32,
        source: glam::Vec3,
        required: i32,
    ) -> Option<glam::Vec3> {
        self.host_script_nearest_enemy_group_with_value(attacker, source, required)
    }

    pub(super) fn host_script_nearest_enemy_group_with_value(
        &self,
        attacker_pid: u32,
        origin: glam::Vec3,
        value: i32,
    ) -> Option<glam::Vec3> {
        let enemy_mask = self.host_script_enemy_player_mask(attacker_pid);
        let source = gamelogic::common::Coord3D::new(origin.x, origin.z, 0.0);
        let location = self
            .world_services
            .shroud()
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .nearest_host_scalar_group(
                &source,
                enemy_mask,
                value,
                gamelogic::object::collide::partition_manager::ValueOrThreat::CashValue,
            )?;
        Some(glam::Vec3::new(location.x, 0.0, location.y))
    }

    pub(super) fn host_script_coord_to_world(x: f32, y: f32, z: f32) -> glam::Vec3 {
        // Generals Coord3D: (x,y) map plane, z = height.
        glam::Vec3::new(x, z, y)
    }

    pub(super) fn host_script_create_team(&self, team_name: &str) -> crate::game_logic::Team {
        if let Ok(factory) = self.team_factory.lock() {
            if let Some(proto) = factory.find_team_prototype(team_name) {
                let owner = proto.get_owner_name().to_string();
                if !owner.is_empty() {
                    if let Some(pid) = self.host_player_id_for_script_token(&owner) {
                        if let Some(player) = self.players.get(&pid) {
                            return player.team;
                        }
                    }
                }
            }
        }
        Self::resolve_host_team_name(team_name).unwrap_or(crate::game_logic::Team::Neutral)
    }

    /// CPP ScriptActions952: resolve/create this session's team before newObject.
    pub(super) fn apply_owned_script_create_object(
        &mut self,
        request: gamelogic::scripting::engine::ScriptObjectCreation<'_>,
        calling: Option<u32>,
        condition: Option<u32>,
        interpreter: Option<&gamelogic::scripting::engine::ScriptEngine>,
    ) -> gamelogic::GameLogicResult<()> {
        self.apply_owned_script_create_object_with_effects(
            request,
            calling,
            condition,
            interpreter,
            &mut Default::default(),
        )
    }

    pub(super) fn apply_owned_script_create_object_with_effects(
        &mut self,
        request: gamelogic::scripting::engine::ScriptObjectCreation<'_>,
        calling: Option<u32>,
        condition: Option<u32>,
        interpreter: Option<&gamelogic::scripting::engine::ScriptEngine>,
        ui: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> gamelogic::GameLogicResult<()> {
        let mut created_identity = None;
        let resolved = self.resolve_owned_script_team(request.team, calling, condition)?;
        let team_id = if let Some((id, _)) = resolved {
            id
        } else {
            if request.team == gamelogic::scripting::core::THIS_TEAM {
                return Ok(());
            }
            self.require_owned_team_creation_interpreter(request.team, true, interpreter)?;
            let creation = self
                .team_factory
                .lock()
                .map_err(|error| gamelogic::GameLogicError::Threading(error.to_string()))?
                .create_inactive_team_with_owned_effects(request.team);
            let Some((team, effects)) = creation else {
                return Ok(());
            };
            let id = {
                let team = team
                    .read()
                    .map_err(|error| gamelogic::GameLogicError::Threading(error.to_string()))?;
                team.get_id()
            };
            created_identity = Some(team.clone());
            self.run_owned_team_creation_actions(&team, effects, interpreter, ui)?;
            if let Some(current) = self
                .team_factory
                .lock()
                .map_err(|error| gamelogic::GameLogicError::Threading(error.to_string()))?
                .find_team_by_id(id)
            {
                if !std::sync::Arc::ptr_eq(&current, &team) {
                    return Ok(());
                }
                current
                    .write()
                    .map_err(|error| gamelogic::GameLogicError::Threading(error.to_string()))?
                    .set_active();
            }
            id
        };
        // Re-resolve after callbacks: deleted/replaced teams cannot admit objects.
        let identity = self
            .team_factory
            .lock()
            .map_err(|error| gamelogic::GameLogicError::Threading(error.to_string()))?
            .find_team_by_id(team_id)
            .filter(|current| {
                created_identity
                    .as_ref()
                    .is_none_or(|created| std::sync::Arc::ptr_eq(current, created))
            })
            .and_then(|team| {
                team.read().ok().map(|team| {
                    (
                        team.get_name().as_str().to_owned(),
                        team.get_controlling_player_id(),
                    )
                })
            });
        let Some((name, Some(controller))) = identity else {
            return Ok(());
        };
        let position = Self::host_script_coord_to_world(request.x, request.y, request.z);
        if let Some(id) = self.create_object_on_exact_script_team_with_effects_checked(
            request.template,
            controller,
            &name,
            team_id,
            position,
            false,
            interpreter,
            ui,
        )? {
            if let Some(object) = self.objects.get_mut(&id) {
                object.set_orientation_with_partition_owner(
                    request.angle,
                    &mut self.partition_manager,
                );
            }
        }
        Ok(())
    }

    pub(super) fn host_script_create_object(
        &mut self,
        name: Option<&str>,
        thing: &str,
        team_name: &str,
        x: f32,
        y: f32,
        z: f32,
        angle: f32,
    ) -> Option<ObjectId> {
        self.host_script_create_object_with_interpreter(
            name, thing, team_name, x, y, z, angle, None,
        )
    }

    fn host_script_create_object_with_interpreter(
        &mut self,
        name: Option<&str>,
        thing: &str,
        team_name: &str,
        x: f32,
        y: f32,
        z: f32,
        angle: f32,
        interpreter: Option<&gamelogic::scripting::engine::ScriptEngine>,
    ) -> Option<ObjectId> {
        self.host_script_create_object_with_effects(
            name,
            thing,
            team_name,
            x,
            y,
            z,
            angle,
            interpreter,
            &mut Default::default(),
        )
    }

    pub(super) fn host_script_create_object_with_effects(
        &mut self,
        name: Option<&str>,
        thing: &str,
        team_name: &str,
        x: f32,
        y: f32,
        z: f32,
        angle: f32,
        interpreter: Option<&gamelogic::scripting::engine::ScriptEngine>,
        ui: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> Option<ObjectId> {
        let old_named_object = name
            .filter(|name| !name.is_empty())
            .and_then(|name| self.host_object_id_by_script_name(name));
        if let Some(unit_name) = name.filter(|n| !n.is_empty()) {
            if let Some(id) = self.host_object_id_by_script_name(unit_name) {
                if self
                    .objects
                    .get(&id)
                    .is_some_and(|obj| !obj.status.effectively_dead)
                {
                    return None;
                }
            }
        }
        let mut pos = Self::host_script_coord_to_world(x, y, z);
        if z == 0.0 {
            if let Some(h) = self.terrain_height_at(glam::Vec3::new(pos.x, 0.0, pos.z)) {
                pos.y = h;
            }
        }
        self.require_owned_team_creation_interpreter(team_name, false, interpreter)
            .ok()?;
        // Source getTeamNamed preserves the actual calling/condition Team
        // before consulting the prototype's instance list.
        let (calling, condition) = interpreter
            .map(|engine| (engine.get_calling_team_id(), engine.get_condition_team_id()))
            .unwrap_or_default();
        let resolved = self
            .resolve_owned_script_team(team_name, calling, condition)
            .ok()?;
        if resolved.is_none() && team_name == gamelogic::scripting::core::THIS_TEAM {
            return None;
        }
        let selected = {
            let mut factory = self.team_factory.lock().ok()?;
            if let Some((id, _)) = resolved {
                factory.find_team_by_id(id).map(|team| (team, Vec::new()))
            } else {
                factory
                    .find_team_with_owned_effects(team_name)
                    .or_else(|| factory.create_inactive_team_with_owned_effects(team_name))
            }
        };
        let id = if let Some((selected, effects)) = selected {
            let identity = selected.read().ok()?.get_id();
            if let Err(error) =
                self.run_owned_team_creation_actions(&selected, effects, interpreter, ui)
            {
                log::error!("script object creation callback failed: {error}");
                return None;
            }
            let current = self.team_factory.lock().ok()?.find_team_by_id(identity)?;
            if !std::sync::Arc::ptr_eq(&current, &selected) {
                return None;
            }
            let (controller, selected_name) = {
                let current = current.read().ok()?;
                (
                    current.get_controlling_player_id()?,
                    current.get_name().as_str().to_owned(),
                )
            };
            self.create_object_on_exact_script_team_with_effects(
                thing,
                controller,
                &selected_name,
                identity,
                pos,
                true,
                interpreter,
                ui,
            )?
        } else {
            // Source cannot construct an object when no receiving Team exists.
            return None;
        };
        if let Some(obj) = self.objects.get_mut(&id) {
            obj.set_orientation_with_partition_owner(angle, &mut self.partition_manager);
            // The semantic factory admitted the actual resident Team; a dead
            // controller may have redirected it to neutral. Never overwrite
            // that canonical name from the original requested script string.
            if let Some(unit_name) = name.filter(|n| !n.is_empty()) {
                obj.name = unit_name.to_string();
            }
        }
        if let Some(unit_name) = name.filter(|name| !name.is_empty()) {
            if old_named_object.is_some() || self.named_script_cache.did_exist(unit_name) {
                self.transfer_receiving_script_name(unit_name, id);
            } else {
                self.add_receiving_named_script_object(id);
            }
        }
        Some(id)
    }

    pub(super) fn host_script_create_reinforcement_team(
        &mut self,
        team_name: &str,
        waypoint_name: &str,
    ) {
        self.host_script_create_reinforcement_team_with_interpreter(team_name, waypoint_name, None);
    }

    pub(super) fn host_script_create_reinforcement_team_with_interpreter(
        &mut self,
        team_name: &str,
        waypoint_name: &str,
        interpreter: Option<&gamelogic::scripting::engine::ScriptEngine>,
    ) {
        self.host_script_create_reinforcement_team_with_effects(
            team_name,
            waypoint_name,
            interpreter,
            &mut Default::default(),
        )
    }

    pub(super) fn host_script_create_reinforcement_team_with_effects(
        &mut self,
        team_name: &str,
        waypoint_name: &str,
        interpreter: Option<&gamelogic::scripting::engine::ScriptEngine>,
        ui: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) {
        let Some(dest) = self.host_script_waypoint_position(waypoint_name) else {
            return;
        };
        let mut origin = dest;
        let (start, transport, units) = {
            let Ok(factory) = self.team_factory.lock() else {
                return;
            };
            let Some(proto) = factory.find_team_prototype(team_name) else {
                return;
            };
            (
                proto.get_start_reinforce_waypoint().to_string(),
                proto.get_transport_unit_type().to_string(),
                proto
                    .units_info()
                    .iter()
                    .filter(|unit| unit.max_units >= 1 && !unit.unit_thing_name.is_empty())
                    .map(|unit| (unit.unit_thing_name.to_string(), unit.max_units))
                    .collect::<Vec<_>>(),
            )
        };
        if !start.is_empty() {
            if let Some(start_pos) = self.host_script_waypoint_position(&start) {
                origin = start_pos;
            }
        }
        if let Err(error) =
            self.require_owned_team_creation_interpreter(team_name, true, interpreter)
        {
            log::error!("reinforcement Team creation failed: {error}");
            return;
        }
        // C++510 creates one inactive instance before any transport/member.
        let created = self
            .team_factory
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .create_inactive_team_with_owned_effects(team_name);
        let Some((team, effects)) = created else {
            return;
        };
        let identity = team
            .read()
            .unwrap_or_else(|error| error.into_inner())
            .get_id();
        if let Err(error) = self.run_owned_team_creation_actions(&team, effects, interpreter, ui) {
            log::error!("reinforcement creation callback failed: {error}");
            return;
        }
        let current = self
            .team_factory
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .find_team_by_id(identity);
        if current.is_none_or(|current| !std::sync::Arc::ptr_eq(&current, &team)) {
            return;
        }
        let Some(controller) = team
            .read()
            .unwrap_or_else(|error| error.into_inner())
            .get_controlling_player_id()
        else {
            return;
        };
        let mut spawned: Vec<ObjectId> = Vec::new();
        if !transport.is_empty() {
            if let Some(id) = self.create_object_on_exact_script_team_with_effects(
                &transport,
                controller,
                team_name,
                identity,
                origin,
                false,
                interpreter,
                ui,
            ) {
                spawned.push(id);
            }
        }
        let mut slot = 0i32;
        for (thing, count) in units {
            for _ in 0..count {
                let offset = slot as f32 * 5.0;
                if let Some(id) = self.create_object_on_exact_script_team_with_effects(
                    &thing,
                    controller,
                    team_name,
                    identity,
                    glam::Vec3::new(origin.x + offset, origin.y, origin.z),
                    false,
                    interpreter,
                    ui,
                ) {
                    spawned.push(id);
                }
                slot += 1;
            }
        }
        if transport.is_empty() {
            if let Some(current) = self
                .team_factory
                .lock()
                .unwrap_or_else(|error| error.into_inner())
                .find_team_by_id(identity)
            {
                if !std::sync::Arc::ptr_eq(&current, &team) {
                    return;
                }
                current
                    .write()
                    .unwrap_or_else(|error| error.into_inner())
                    .set_active();
            }
        }
        if (origin - dest).length_squared() > 1.0 {
            for id in spawned {
                let _ = self.unit_command_move_to(id, dest);
            }
        }
    }

    /// C++ `ScriptActions::doTeamHuntWithCommandButton` live drain.
    pub(super) fn host_script_team_hunt_with_command_button(
        &mut self,
        team: &str,
        button: &str,
        effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> gamelogic::GameLogicResult<()> {
        // C++ findCommandButton + command-type switch before any unit is armed.
        // Leftover `command_button_is_hunt_capable` is the same gate.
        if !self.leftover_command_button_is_hunt_capable(button) {
            return Ok(());
        }
        let ids = self.host_script_hunt_guard_team_member_ids(team);
        let button = if button.is_empty() {
            None
        } else {
            Some(button)
        };
        for id in ids {
            if self.unit_can_team_hunt_with_command_button(id, button) {
                let _ = self.start_command_button_hunt_named_with_effects(id, button, effects)?;
            }
        }
        Ok(())
    }

    /// C++ `Object::leaveGroup` removes membership but leaves the independent
    /// formation ID and offset unchanged.
    pub(super) fn host_object_leave_group(&mut self, id: ObjectId) {
        self.leave_ai_group(id);
    }

    /// C++ `getAIUpdateInterface` + `getCurLocomotor`. Structures have no
    /// current locomotor; generic test scouts without KindOf still stamp.
    pub(super) fn host_script_member_has_cur_locomotor(
        obj: &crate::game_logic::object::Object,
    ) -> bool {
        use crate::game_logic::KindOf;
        !obj.is_kind_of(KindOf::Structure)
    }

    fn receiving_garrison_members(
        &self,
        name: &str,
        calling: Option<u32>,
        condition: Option<u32>,
    ) -> gamelogic::GameLogicResult<Vec<ObjectId>> {
        Ok(self
            .resolve_owned_script_team(name, calling, condition)?
            .map(|(_, members)| members.into_iter().map(ObjectId).collect())
            .unwrap_or_default())
    }

    pub(super) fn apply_receiving_team_exit_all(
        &mut self,
        team: &str,
        calling: Option<u32>,
        condition: Option<u32>,
        effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> gamelogic::GameLogicResult<()> {
        let Some((_, members)) = self.resolve_owned_script_team(team, calling, condition)? else {
            return Ok(());
        };
        // ScriptActions::doTeamExitAll creates and fills the AIGroup before
        // groupEvacuate; admission also leaves any previous group immediately.
        let group = self.create_team_ai_group(&members);
        for id in self.ai_group_members(group) {
            if self.ai_group_id_of(id) != Some(group) {
                continue;
            }
            let Some(member) = self.host_object(id) else {
                continue;
            };
            let has_ai = member.has_ai_update_interface();
            let is_structure = member.is_kind_of(crate::game_logic::KindOf::Structure);
            let airborne_aircraft = member.is_kind_of(crate::game_logic::KindOf::Aircraft)
                && member.status.airborne_target;

            if airborne_aircraft && has_ai {
                // C++ AIGroup.cpp:2416-2422 snapshots the aircraft position,
                // picks the highest receiving-world layer below it, and
                // synchronously calls aiMoveToAndEvacuate(CMD_FROM_SCRIPT).
                // Run the same existing owner-bound transport state machine
                // here, inline with roster traversal, before the next member's
                // evacuation callbacks.
                let Some((position, ground_height)) = self
                    .host_object(id)
                    .map(|object| (object.get_position(), object.ground_height))
                else {
                    continue;
                };
                let destination_y = self.evacuation_surface_height(position, ground_height);
                let destination = glam::Vec3::new(position.x, destination_y, position.z);
                let mut executor = crate::command_executor::CommandExecutor::new_with_effects(
                    self,
                    0,
                    effects.reborrow(),
                );
                executor.execute_move_to_and_evacuate_with_source(
                    &[id],
                    destination,
                    false,
                    crate::game_logic::host_command_button_hunt::HUNT_CMD_FROM_SCRIPT,
                )?;
                continue;
            }

            if has_ai {
                self.order_owned_script_passengers_to_exit(id, true);
            } else if is_structure {
                // Buildings have no AIUpdateInterface in C++; groupEvacuate
                // directly orders each passenger to exit without removing it.
                self.order_owned_script_passengers_to_exit(id, false);
            }
        }
        Ok(())
    }

    pub(super) fn apply_receiving_garrison_request(
        &mut self,
        req: gamelogic::scripting::HostScriptGarrisonEnterExitRequest,
        this_object: Option<ObjectId>,
        calling: Option<u32>,
        condition: Option<u32>,
        effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> gamelogic::GameLogicResult<()> {
        use gamelogic::scripting::HostScriptGarrisonEnterExitRequest;

        match req {
            HostScriptGarrisonEnterExitRequest::TeamEnter { team, dest } => {
                let Some(dest_id) = self.owned_named_script_object(&dest, this_object) else {
                    return Ok(());
                };
                let members = self.receiving_garrison_members(&team, calling, condition)?;
                for id in members {
                    self.host_script_ai_enter(id, dest_id)?;
                }
            }
            HostScriptGarrisonEnterExitRequest::TeamGarrisonSpecific { team, building } => {
                let Some(building_id) = self.owned_named_script_object(&building, this_object)
                else {
                    return Ok(());
                };
                let members = self.receiving_garrison_members(&team, calling, condition)?;
                let player_id = members
                    .first()
                    .and_then(|id| self.host_object(*id).and_then(|o| o.owner_player_id));
                if !self.host_script_building_can_garrison(building_id, player_id) {
                    return Ok(());
                }
                for id in members {
                    self.host_script_ai_enter(id, building_id)?;
                }
            }
            HostScriptGarrisonEnterExitRequest::ExitSpecificBuilding { building } => {
                let Some(id) = self.owned_named_script_object(&building, this_object) else {
                    return Ok(());
                };
                if !self
                    .host_object(id)
                    .is_some_and(|o| o.is_kind_of(crate::game_logic::KindOf::Structure))
                {
                    return Ok(());
                }
                self.host_script_ai_evacuate_with_effects(id, effects);
            }
            HostScriptGarrisonEnterExitRequest::TeamGarrisonNearest { team } => {
                self.host_script_team_garrison_nearest(&team, calling, condition)?;
            }
            HostScriptGarrisonEnterExitRequest::TeamExitAllBuildings { team } => {
                for id in self.receiving_garrison_members(&team, calling, condition)? {
                    self.host_script_ai_exit_with_effects(id, effects);
                }
            }
            HostScriptGarrisonEnterExitRequest::NamedGarrisonSpecific { unit, building } => {
                let Some(unit_id) = self.owned_named_script_object(&unit, this_object) else {
                    return Ok(());
                };
                let Some(building_id) = self.owned_named_script_object(&building, this_object)
                else {
                    return Ok(());
                };
                let player_id = self.host_object(unit_id).and_then(|o| o.owner_player_id);
                if !self.host_script_building_can_garrison(building_id, player_id) {
                    return Ok(());
                }
                self.host_script_ai_enter(unit_id, building_id)?;
            }
            HostScriptGarrisonEnterExitRequest::NamedGarrisonNearest { unit } => {
                if let Some(id) = self.owned_named_script_object(&unit, this_object) {
                    self.host_script_garrison_nearest_for_object(id)?;
                }
            }
            HostScriptGarrisonEnterExitRequest::NamedExitBuilding { unit } => {
                let Some(id) = self.owned_named_script_object(&unit, this_object) else {
                    return Ok(());
                };
                self.host_script_ai_exit_with_effects(id, effects);
            }
            HostScriptGarrisonEnterExitRequest::PlayerGarrisonAll { player } => {
                self.host_script_player_garrison_all(&player)?;
            }
            HostScriptGarrisonEnterExitRequest::PlayerExitAll { player } => {
                self.host_script_player_exit_all_with_effects(&player, effects);
            }
        }
        Ok(())
    }

    pub(super) fn host_script_contain_slots_available(
        obj: &crate::game_logic::object::Object,
    ) -> i32 {
        if let Some(bd) = obj.building_data.as_ref() {
            return bd.max_garrison as i32 - bd.garrisoned_units.len() as i32;
        }
        let max = obj.max_transport as i32;
        if max > 0 {
            return max - obj.occupants.len() as i32;
        }
        0
    }

    pub(super) fn host_script_building_can_garrison(
        &self,
        building_id: ObjectId,
        player_id: Option<u32>,
    ) -> bool {
        use crate::game_logic::KindOf;
        let Some(building) = self.host_object(building_id) else {
            return false;
        };
        if !building.is_kind_of(KindOf::Structure)
            || !building.is_alive()
            || building.status.destroyed
        {
            return false;
        }
        if Self::host_script_contain_slots_available(building) <= 0 {
            return false;
        }
        if building.player_who_entered.is_empty() {
            return true;
        }
        let Some(pid) = player_id else {
            return false;
        };
        self.players
            .get(&pid)
            .is_some_and(|p| building.player_who_entered.eq_ignore_ascii_case(&p.name))
    }

    pub(super) fn host_script_ai_enter(
        &mut self,
        unit: ObjectId,
        dest: ObjectId,
    ) -> gamelogic::GameLogicResult<()> {
        use crate::game_logic::AIState;
        let Some(pos) = self.host_object(dest).map(|obj| obj.get_position()) else {
            return Ok(());
        };
        self.host_object_leave_group(unit);
        let _ = self.apply_unit_locomotor_set(unit, "normal");
        self.unit_command_stop_moving_order_target(unit, Some(dest))?;
        if !self.unit_command_path_with_state_ignoring(unit, pos, AIState::Entering, Some(dest))? {
            if let Some((obj, audio_events)) = self.host_object_and_audio_events_mut(unit) {
                obj.ignored_obstacle_id = Some(dest);
                obj.set_ai_state(
                    AIState::Entering,
                    &mut crate::game_logic::ObjectAudio::queue(audio_events),
                );
            }
        }
        Ok(())
    }
    pub(super) fn host_script_ai_evacuate(&mut self, id: ObjectId) {
        self.host_script_ai_evacuate_with_effects(id, &mut Default::default())
    }

    pub(super) fn host_script_ai_evacuate_with_effects(
        &mut self,
        id: ObjectId,
        effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) {
        self.host_object_leave_group(id);
        let _ = self.apply_unit_locomotor_set(id, "normal");
        let _ = self.evacuate_container_now_with_effects(id, false, effects);
    }

    pub(super) fn host_script_ai_exit(&mut self, id: ObjectId) {
        self.host_script_ai_exit_with_effects(id, &mut Default::default());
    }

    pub(super) fn host_script_ai_exit_with_effects(
        &mut self,
        id: ObjectId,
        effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) {
        self.host_object_leave_group(id);
        let _ = self.apply_unit_locomotor_set(id, "normal");
        let Some(cid) = self.host_object(id).and_then(|obj| obj.contained_by) else {
            return;
        };
        let pos = self
            .host_object(cid)
            .map(|obj| obj.get_position())
            .unwrap_or(glam::Vec3::ZERO);
        let _ = self.unit_command_exit_drop_with_effects(id, pos, effects);
    }

    pub(super) fn host_script_is_garrison_nearest_candidate(
        &self,
        building: &crate::game_logic::object::Object,
        leader_off_map: bool,
        leader_is_hacker: bool,
        player_id: Option<u32>,
    ) -> bool {
        use crate::game_logic::KindOf;
        if !building.is_alive() || building.status.destroyed {
            return false;
        }
        if building.status.off_map != leader_off_map {
            return false;
        }
        let is_internet = building.is_kind_of(KindOf::FSInternetCenter);
        if leader_is_hacker {
            if !is_internet {
                return false;
            }
        } else if is_internet || !building.is_kind_of(KindOf::Structure) {
            return false;
        }
        if Self::host_script_contain_slots_available(building) <= 0 {
            return false;
        }
        if !leader_is_hacker && !building.player_who_entered.is_empty() {
            let Some(pid) = player_id else {
                return false;
            };
            if !self
                .players
                .get(&pid)
                .is_some_and(|p| building.player_who_entered.eq_ignore_ascii_case(&p.name))
            {
                return false;
            }
        }
        true
    }

    pub(super) fn host_script_team_garrison_nearest(
        &mut self,
        team: &str,
        calling: Option<u32>,
        condition: Option<u32>,
    ) -> gamelogic::GameLogicResult<()> {
        use crate::game_logic::KindOf;
        let members = self.receiving_garrison_members(team, calling, condition)?;
        let Some(&leader_id) = members.first() else {
            return Ok(());
        };
        let Some((leader_pos, leader_off_map, leader_is_hacker, player_id)) =
            self.host_object(leader_id).map(|obj| {
                (
                    obj.get_position(),
                    obj.status.off_map,
                    obj.is_kind_of(KindOf::MoneyHacker),
                    obj.owner_player_id,
                )
            })
        else {
            return Ok(());
        };
        let mut buildings: Vec<(f32, ObjectId)> = self
            .objects
            .registered_values()
            .filter(|obj| obj.id != leader_id)
            .filter(|obj| {
                self.host_script_is_garrison_nearest_candidate(
                    obj,
                    leader_off_map,
                    leader_is_hacker,
                    player_id,
                )
            })
            .map(|obj| {
                let pos = obj.get_position();
                let dx = pos.x - leader_pos.x;
                let dy = pos.y - leader_pos.y;
                let dz = pos.z - leader_pos.z;
                (dx * dx + dy * dy + dz * dz, obj.id)
            })
            .collect();
        buildings.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));
        let mut member_idx = 0usize;
        for (_, building_id) in buildings {
            let slots = self
                .host_object(building_id)
                .map(Self::host_script_contain_slots_available)
                .unwrap_or(0);
            if slots <= 0 {
                continue;
            }
            let mut filled = 0i32;
            while filled < slots && member_idx < members.len() {
                let member_id = members[member_idx];
                member_idx += 1;
                let ok = self.host_object(member_id).is_some_and(|obj| {
                    obj.is_kind_of(KindOf::Infantry) && !obj.is_kind_of(KindOf::NoGarrison)
                });
                if !ok {
                    continue;
                }
                self.host_script_ai_enter(member_id, building_id)?;
                filled += 1;
            }
            if member_idx >= members.len() {
                break;
            }
        }
        Ok(())
    }

    pub(super) fn host_script_named_garrison_nearest(
        &mut self,
        unit: &str,
    ) -> gamelogic::GameLogicResult<()> {
        if let Some(id) = self.host_object_id_by_script_name(unit) {
            self.host_script_garrison_nearest_for_object(id)?;
        }
        Ok(())
    }

    pub(super) fn host_script_garrison_nearest_for_object(
        &mut self,
        unit_id: ObjectId,
    ) -> gamelogic::GameLogicResult<()> {
        use crate::game_logic::KindOf;
        let Some((unit_pos, unit_off_map, unit_is_hacker, player_id)) =
            self.host_object(unit_id).map(|obj| {
                (
                    obj.get_position(),
                    obj.status.off_map,
                    obj.is_kind_of(KindOf::MoneyHacker),
                    obj.owner_player_id,
                )
            })
        else {
            return Ok(());
        };
        let mut best: Option<(f32, ObjectId)> = None;
        for obj in self.objects.registered_values() {
            if obj.id == unit_id {
                continue;
            }
            if !self.host_script_is_garrison_nearest_candidate(
                obj,
                unit_off_map,
                unit_is_hacker,
                player_id,
            ) {
                continue;
            }
            let pos = obj.get_position();
            let dx = pos.x - unit_pos.x;
            let dy = pos.y - unit_pos.y;
            let dz = pos.z - unit_pos.z;
            let dist = dx * dx + dy * dy + dz * dz;
            if best.is_none_or(|(best_dist, _)| dist < best_dist) {
                best = Some((dist, obj.id));
            }
        }
        let Some((_, building_id)) = best else {
            return Ok(());
        };
        self.host_script_ai_enter(unit_id, building_id)?;
        Ok(())
    }

    pub(super) fn host_script_player_garrison_all(
        &mut self,
        player: &str,
    ) -> gamelogic::GameLogicResult<()> {
        use crate::game_logic::KindOf;
        let Some(pid) = self.host_player_id_for_script_token(player) else {
            return Ok(());
        };
        let unit_ids: Vec<ObjectId> = self
            .objects
            .registered_values()
            .filter(|obj| {
                obj.is_alive()
                    && !obj.status.destroyed
                    && obj.owner_player_id == Some(pid)
                    && !obj.is_kind_of(KindOf::Structure)
                    && obj.is_kind_of(KindOf::Infantry)
                    && !obj.is_kind_of(KindOf::NoGarrison)
            })
            .map(|obj| obj.id)
            .collect();
        for unit_id in unit_ids {
            let Some(unit_pos) = self.host_object(unit_id).map(|obj| obj.get_position()) else {
                continue;
            };
            let mut best: Option<(f32, ObjectId)> = None;
            for obj in self.objects.registered_values() {
                if obj.id == unit_id {
                    continue;
                }
                if !self.host_script_building_can_garrison(obj.id, Some(pid)) {
                    continue;
                }
                let pos = obj.get_position();
                let dx = pos.x - unit_pos.x;
                let dy = pos.y - unit_pos.y;
                let dz = pos.z - unit_pos.z;
                let dist = dx * dx + dy * dy + dz * dz;
                if best.is_none_or(|(best_dist, _)| dist < best_dist) {
                    best = Some((dist, obj.id));
                }
            }
            let Some((_, building_id)) = best else {
                continue;
            };
            self.host_script_ai_enter(unit_id, building_id)?;
        }
        Ok(())
    }

    pub(super) fn host_script_player_exit_all(&mut self, player: &str) {
        self.host_script_player_exit_all_with_effects(player, &mut Default::default())
    }

    pub(super) fn host_script_player_exit_all_with_effects(
        &mut self,
        player: &str,
        effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) {
        use crate::game_logic::KindOf;
        let Some(pid) = self.host_player_id_for_script_token(player) else {
            return;
        };
        let ids: Vec<ObjectId> = self
            .objects
            .registered_values()
            .filter(|obj| {
                obj.is_alive()
                    && !obj.status.destroyed
                    && obj.owner_player_id == Some(pid)
                    && obj.is_kind_of(KindOf::Structure)
            })
            .map(|obj| obj.id)
            .collect();
        for id in ids {
            self.host_script_ai_evacuate_with_effects(id, effects);
        }
    }

    /// C++ `ScriptEngine::getTeamNamed` / leftover `TeamFactory::find_team`.
    /// First leftover instance members, else live `team_instance_name` — never faction Team.
    pub(super) fn host_script_hunt_guard_team_member_ids(&self, team_name: &str) -> Vec<ObjectId> {
        let needle = team_name.trim();
        if needle.is_empty() {
            return Vec::new();
        }
        let leftover_ids: Vec<ObjectId> = self
            .team_factory
            .lock()
            .ok()
            .and_then(|mut factory| {
                factory
                    .find_team(needle)
                    .and_then(|team| team.read().ok().map(|tg| tg.get_members().to_vec()))
            })
            .unwrap_or_default()
            .into_iter()
            .map(ObjectId)
            .filter(|id| {
                self.objects
                    .get(id)
                    .is_some_and(|o| o.is_alive() && !o.status.destroyed)
            })
            .collect();
        if !leftover_ids.is_empty() {
            return leftover_ids;
        }
        self.host_script_team_census_member_ids(needle)
            .into_iter()
            .map(ObjectId)
            .filter(|id| {
                self.objects
                    .get(&id)
                    .is_some_and(|o| o.is_alive() && !o.status.destroyed)
            })
            .collect()
    }

    /// Leftover `command_button_is_hunt_capable` + C++ findCommandButton NULL → no-op.
    pub(super) fn leftover_command_button_is_hunt_capable(&self, ability: &str) -> bool {
        if ability.is_empty() {
            return false;
        }
        if let Some(actions) = self
            .receiving_module_content
            .as_ref()
            .map(|content| content.action_definitions())
        {
            let Some(btn) = actions.buttons.find_command_button_resolved(ability) else {
                return false;
            };
            return Self::leftover_command_type_is_hunt_capable(
                gamelogic::command_button::map_gui_command_to_command_type(&btn.command),
                btn.get_special_power_template().is_some(),
                btn.options_bits,
            );
        }
        if let Some(bar) = gamelogic::control_bar::get_control_bar_bridge() {
            if let Some(btn) = bar.find_command_button_by_name(ability) {
                return Self::leftover_command_type_is_hunt_capable(
                    btn.get_command_type(),
                    btn.get_special_power_template().is_some(),
                    btn.get_options_bits(),
                );
            }
        }
        if let Some(bar) = game_engine::common::ini::ini_command_button::get_control_bar() {
            if let Some(btn) = bar.find_command_button_resolved(ability) {
                return Self::leftover_command_type_is_hunt_capable(
                    gamelogic::command_button::map_gui_command_to_command_type(&btn.command),
                    btn.get_special_power_template().is_some(),
                    btn.options_bits,
                );
            }
            if !bar.get_button_names().is_empty() {
                return false;
            }
        }
        false
    }

    /// Leftover `ScriptActionDispatcher::command_button_is_hunt_capable` switch.
    pub(super) fn leftover_command_type_is_hunt_capable(
        command_type: gamelogic::commands::command::CommandType,
        has_special_power_template: bool,
        options_bits: u32,
    ) -> bool {
        use gamelogic::commands::command::CommandType;
        use gamelogic::object::update::special_power_update::SpecialPowerCommandOption;
        match command_type {
            CommandType::DoSpecialPower => {
                if !has_special_power_template {
                    return false;
                }
                let options = SpecialPowerCommandOption::from_bits_truncate(options_bits);
                options.intersects(
                    SpecialPowerCommandOption::NEED_TARGET_ENEMY_OBJECT
                        | SpecialPowerCommandOption::NEED_TARGET_NEUTRAL_OBJECT
                        | SpecialPowerCommandOption::NEED_TARGET_ALLY_OBJECT,
                )
            }
            CommandType::SwitchWeapons
            | CommandType::DoAttackObject
            | CommandType::Enter
            | CommandType::ConvertToCarbomb => true,
            _ => false,
        }
    }
    pub(super) fn host_script_team_member_ids(&self, team_name: &str) -> Vec<ObjectId> {
        let needle = team_name.trim();
        if needle.is_empty() {
            return Vec::new();
        }
        // C++ team lookup resolves the named team instance first; the faction
        // fallback exists only for worlds whose objects carry no team
        // instance identity. Once a named team has members, the faction
        // fallback must not mix same-faction enemies into the scripted group
        // (a USA-owned hut must not join "teamAmerica" and flip the group's
        // controlling player).
        let by_instance: Vec<ObjectId> = self
            .objects
            .registered_values()
            .filter(|obj| {
                obj.is_alive()
                    && !obj.status.destroyed
                    && !obj.team_instance_name.is_empty()
                    && obj.team_instance_name.eq_ignore_ascii_case(needle)
            })
            .map(|obj| obj.id)
            .collect();
        if !by_instance.is_empty() {
            return by_instance;
        }
        let faction = Self::resolve_host_team_name(team_name);
        self.objects
            .registered_values()
            .filter(|obj| {
                obj.is_alive()
                    && !obj.status.destroyed
                    && (faction.map(|t| obj.team == t).unwrap_or(false)
                        || obj.team.get_name().eq_ignore_ascii_case(needle))
            })
            .map(|obj| obj.id)
            .collect()
    }

    /// C++ `ThePartitionManager->getClosestObject` (template or ObjectTypes,
    /// `FROM_CENTER_2D`, polygon trigger, same on/off-map).
    pub(super) fn host_script_closest_object_of_type_in_trigger(
        &self,
        interpreter: &gamelogic::scripting::engine::ScriptEngine,
        source_id: ObjectId,
        object_type: &str,
        trigger_name: &str,
    ) -> Option<ObjectId> {
        let source = self.host_object(source_id)?;
        let source_pos = source.get_position();
        let source_off_map = source.get_position().x < self.world_min.x
            || source.get_position().x > self.world_max.x
            || source.get_position().z < self.world_min.z
            || source.get_position().z > self.world_max.z;
        let type_names = if self.templates.contains_key(object_type) {
            vec![object_type.to_owned()]
        } else {
            host_script_object_type_names(interpreter, object_type)?
        };
        if type_names.is_empty() {
            return None;
        }
        let mut best: Option<(ObjectId, f32)> = None;
        for (id, obj) in self.host_objects().registered() {
            if *id == source_id || !obj.is_alive() || obj.status.destroyed {
                continue;
            }
            let off_map = obj.get_position().x < self.world_min.x
                || obj.get_position().x > self.world_max.x
                || obj.get_position().z < self.world_min.z
                || obj.get_position().z > self.world_max.z;
            if off_map != source_off_map {
                continue;
            }
            if !type_names
                .iter()
                .any(|name| obj.template_name.eq_ignore_ascii_case(name))
            {
                continue;
            }
            if !self.host_script_point_in_trigger(obj.get_position(), trigger_name) {
                continue;
            }
            let pos = obj.get_position();
            let dx = pos.x - source_pos.x;
            let dz = pos.z - source_pos.z;
            let dist = dx * dx + dz * dz;
            if best.map(|(_, best_dist)| dist < best_dist).unwrap_or(true) {
                best = Some((*id, dist));
            }
        }
        best.map(|(id, _)| id)
    }

    pub(super) fn host_script_point_in_trigger(&self, pos: glam::Vec3, trigger_name: &str) -> bool {
        self.world_services
            .terrain()
            .read()
            .ok()
            .and_then(|terrain| {
                terrain
                    .get_trigger_area_by_name(trigger_name)
                    .map(|trigger| {
                        trigger.point_in_trigger_int(&gamelogic::common::ICoord3D::new(
                            pos.x as i32,
                            pos.z as i32,
                            0,
                        ))
                    })
            })
            .unwrap_or(false)
    }

    pub(super) fn host_script_leftover_waypoint(
        &self,
        waypoint_name: &str,
    ) -> Option<(u32, glam::Vec3)> {
        let name = gamelogic::common::AsciiString::from(waypoint_name);
        let terrain_owner_handle = self.world_services.terrain();
        let (wid, loc) = terrain_owner_handle.read().ok().and_then(|terrain| {
            terrain
                .get_waypoint_by_name(&name)
                .map(|wp| (wp.get_id(), *wp.get_location()))
        })?;
        let mut pos = glam::Vec3::new(loc.x, loc.z, loc.y);
        if let Some(h) = self.terrain_height_at(glam::Vec3::new(pos.x, 0.0, pos.z)) {
            pos.y = h;
        }
        Some((wid, pos))
    }

    pub(super) fn host_script_waypoint_position(&self, waypoint_name: &str) -> Option<glam::Vec3> {
        self.host_script_leftover_waypoint(waypoint_name)
            .map(|(_, pos)| pos)
    }

    /// C++ `TheTerrainLogic->getClosestWaypointOnPath` id for checkBridges.
    pub(super) fn host_script_closest_waypoint_id(
        &self,
        path_label: &str,
        from: glam::Vec3,
    ) -> Option<u32> {
        let leftover_pos = gamelogic::common::Coord3D::new(from.x, from.z, from.y);
        let terrain_owner_handle = self.world_services.terrain();
        let terrain = terrain_owner_handle.read().ok()?;
        let start = terrain.get_closest_waypoint_on_path(&leftover_pos, path_label)?;
        Some(start.get_id())
    }

    /// C++ `aiFollowWaypointPath` / `groupFollowWaypointPath` / Exact / AsTeam.
    pub(super) fn host_script_issue_follow_waypoint_path(
        &mut self,
        units: &[ObjectId],
        waypoints: &[glam::Vec3],
        exact: bool,
        as_team: bool,
        completion: Option<&super::waypoint_routes::HostWaypointIdentity>,
    ) {
        if waypoints.is_empty() {
            return;
        }
        let mut movers: Vec<(ObjectId, glam::Vec3, glam::Vec2)> = Vec::new();
        for &unit_id in units {
            let Some(unit) = self.host_object(unit_id) else {
                continue;
            };
            if !unit.is_alive() || !unit.can_move() {
                continue;
            }
            if unit.is_kind_of(crate::game_logic::KindOf::Immobile)
                || unit.is_kind_of(crate::game_logic::KindOf::Structure)
            {
                continue;
            }
            movers.push((unit_id, unit.get_position(), unit.formation_offset));
        }
        if movers.is_empty() {
            return;
        }
        let (mut cx, mut cz) = (0.0f32, 0.0f32);
        for (_, pos, _) in &movers {
            cx += pos.x;
            cz += pos.z;
        }
        let n = movers.len() as f32;
        cx /= n;
        cz /= n;
        let fid0 = self
            .host_object(movers[0].0)
            .map(|o| o.formation_id)
            .unwrap_or(0);
        let use_formation = as_team
            && fid0 != 0
            && movers.iter().all(|(id, _, _)| {
                self.host_object(*id)
                    .map(|o| o.formation_id == fid0)
                    .unwrap_or(false)
            });

        for (unit_id, pos, form_off) in movers {
            let offset = if as_team {
                if use_formation {
                    form_off
                } else {
                    glam::Vec2::new(pos.x - cx, pos.z - cz)
                }
            } else {
                glam::Vec2::ZERO
            };
            let unit_wps: Vec<glam::Vec3> = waypoints
                .iter()
                .map(|wp| glam::Vec3::new(wp.x + offset.x, wp.y, wp.z + offset.y))
                .collect();
            let goal = *unit_wps.last().unwrap();
            let via = &unit_wps[..unit_wps.len().saturating_sub(1)];
            let adjusts = self.note_move_to_request_path(unit_id);
            if !exact {
                if let Some(unit) = self.host_object_mut(unit_id) {
                    if !crate::game_logic::PathfindingGrid::is_doing_ground_movement_full(unit) {
                        unit.set_adjust_destinations(false);
                    }
                }
            }
            if let Some(unit) = self.host_object_mut(unit_id) {
                unit.clear_pending_waypoint_labels();
                unit.last_command_source =
                    crate::game_logic::host_command_button_hunt::HUNT_CMD_FROM_SCRIPT;
            }
            let _ = self.unit_command_waypoint_path_prep(unit_id, as_team);
            let assigned = if exact {
                self.assign_unit_path_exact(unit_id, goal, via)
            } else {
                self.assign_unit_path(unit_id, goal, via)
            };
            if assigned && adjusts {
                self.register_ground_path_goal(unit_id, goal);
            }
            if assigned {
                if let Some(unit) = self.host_object_mut(unit_id) {
                    if let Some(completion) = completion {
                        unit.stamp_pending_waypoint(completion.id, completion.labels.clone());
                    }
                }
            }
        }
    }

    /// C++ AIGroup::friend_moveFormationToPos issues aiFollowPath with the
    /// trimmed shared spine. Preserve the existing formation by letting the
    /// receiving path owner apply each member's stored formation offset.
    pub(in crate::game_logic) fn host_script_issue_group_follow_path(
        &mut self,
        unit: ObjectId,
        path: &[glam::Vec3],
    ) {
        self.host_script_issue_follow_waypoint_path(&[unit], path, false, true, None);
    }

    pub(super) fn host_script_area_center(&self, area_name: &str) -> Option<glam::Vec3> {
        let center = {
            let terrain = self.world_services.terrain().read().ok()?;
            let trigger = terrain.get_trigger_area_by_name(area_name)?;
            // PolygonTrigger::getCenterPoint uses the bounds midpoint. Its
            // standalone convenience method reads ambient terrain for Z.
            let bounds = trigger.get_bounds();
            (
                (bounds.lo.x + bounds.hi.x) as f32 / 2.0,
                (bounds.lo.y + bounds.hi.y) as f32 / 2.0,
            )
        };
        let mut pos = glam::Vec3::new(center.0, 0.0, center.1);
        if let Some(height) = self.terrain_height_at(glam::Vec3::new(pos.x, 0.0, pos.z)) {
            pos.y = height;
        }
        Some(pos)
    }

    pub(super) fn host_script_nearest_team_victim(
        &self,
        from: ObjectId,
        victim_team: &str,
    ) -> Option<ObjectId> {
        let origin = self.objects.get(&from)?.get_position();
        self.host_script_team_member_ids(victim_team)
            .into_iter()
            .filter(|&id| id != from)
            .filter_map(|id| {
                self.objects
                    .get(&id)
                    .map(|obj| (id, origin.distance(obj.get_position())))
            })
            .min_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal))
            .map(|(id, _)| id)
    }

    /// CPP ScriptActions1414-1436: resolve team then victim, retain roster
    /// identity and issue group AttackObject at the synchronous action phase.
    /// CPP1362: validate the target team and AI before source preparation.
    pub(super) fn apply_owned_named_attack_team(
        &mut self,
        unit: &str,
        team: &str,
        this_object: Option<ObjectId>,
        calling: Option<u32>,
        condition: Option<u32>,
        force_normal: bool,
        effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> gamelogic::GameLogicResult<()> {
        let Some(id) = self.owned_named_script_object(unit, this_object) else {
            return Ok(());
        };
        let Some((team_id, _members)) = self.resolve_owned_script_team(team, calling, condition)?
        else {
            return Ok(());
        };
        if !self
            .host_object(id)
            .is_some_and(|object| object.has_ai_update_interface())
        {
            return Ok(());
        }
        self.host_object_leave_group(id);
        self.apply_unit_locomotor_set(id, "normal");
        self.apply_owned_attack_team_member(id, team_id, force_normal, effects)?;
        Ok(())
    }

    pub(super) fn apply_owned_attack_team_member(
        &mut self,
        id: ObjectId,
        team_id: u32,
        force_normal: bool,
        effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> gamelogic::GameLogicResult<()> {
        self.drop_jet_targeters_on_attack_exit(id);
        self.clear_unit_movement_path(id);
        let (object, audio_events) = self.host_object_and_audio_events_mut(id).unwrap();
        Self::clear_owned_named_ai_goal(
            object,
            &mut crate::game_logic::ObjectAudio::queue(audio_events),
        );
        object.stop_moving(&mut crate::game_logic::ObjectAudio::queue(audio_events));
        object.set_ai_state(
            AIState::Idle,
            &mut crate::game_logic::ObjectAudio::queue(audio_events),
        );
        let identity = format!("owned:{team_id}");
        let target = self.choose_attack_team_victim_with_policy(id, &identity, false, force_normal);
        if let Some(target) = target {
            self.unit_command_script_attack_with_effects(id, target, effects)?;
        }
        if let Some(object) = self.host_object_mut(id) {
            object.set_max_shots_to_fire(-1);
            object.set_auto_acquire_when_idle(true);
            object.attack_priority_set = Some(format!("AIGroup.AttackTeam.{identity}"));
            object.last_command_source =
                crate::game_logic::host_command_button_hunt::HUNT_CMD_FROM_SCRIPT;
        }
        Ok(())
    }

    pub(super) fn host_script_attack_team(
        &mut self,
        unit_id: ObjectId,
        victim_team: &str,
        effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> gamelogic::GameLogicResult<()> {
        if let Some(team) = Self::resolve_host_team_name(victim_team) {
            if team != crate::game_logic::Team::Neutral {
                if let Some(unit) = self.objects.get_mut(&unit_id) {
                    unit.set_max_shots_to_fire(-1);
                    unit.set_auto_acquire_when_idle(true);
                    unit.attack_priority_set =
                        Some(format!("AIGroup.AttackTeam.{}", victim_team.trim()));
                }
            }
        }
        if let Some(vid) = self.host_script_nearest_team_victim(unit_id, victim_team) {
            self.unit_command_attack_with_effects(unit_id, vid, effects)?;
            if let Some(team) = Self::resolve_host_team_name(victim_team) {
                if team != crate::game_logic::Team::Neutral {
                    if let Some(unit) = self.objects.get_mut(&unit_id) {
                        unit.set_max_shots_to_fire(-1);
                        unit.set_auto_acquire_when_idle(true);
                        unit.attack_priority_set =
                            Some(format!("AIGroup.AttackTeam.{}", victim_team.trim()));
                    }
                }
            }
        }
        Ok(())
    }

    /// CPP ScriptActions1336–1359: validate first, then leave/Normal/AttackArea.
    pub(super) fn apply_owned_named_attack_area(
        &mut self,
        unit: &str,
        area: &str,
        this_object: Option<ObjectId>,
    ) -> gamelogic::GameLogicResult<()> {
        self.apply_owned_named_attack_area_with_effects(
            unit,
            area,
            this_object,
            &mut Default::default(),
        )
    }

    pub(super) fn apply_owned_named_attack_area_with_effects(
        &mut self,
        unit: &str,
        area: &str,
        this_object: Option<ObjectId>,
        effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> gamelogic::GameLogicResult<()> {
        let Some(id) = self.owned_named_script_object(unit, this_object) else {
            return Ok(());
        };
        if self.host_named_guard_area_polygon(area).is_none()
            || !self
                .host_object(id)
                .is_some_and(|object| object.has_ai_update_interface())
        {
            return Ok(());
        }
        self.host_object_leave_group(id);
        self.apply_unit_locomotor_set(id, "normal");
        self.host_script_attack_area_with_effects(id, area, effects)?;
        Ok(())
    }

    pub(super) fn host_script_attack_area(
        &mut self,
        unit_id: ObjectId,
        area_name: &str,
    ) -> gamelogic::GameLogicResult<()> {
        self.host_script_attack_area_with_effects(unit_id, area_name, &mut Default::default())
    }

    pub(super) fn host_script_attack_area_with_effects(
        &mut self,
        unit_id: ObjectId,
        area_name: &str,
        effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> gamelogic::GameLogicResult<()> {
        let tag = format!("AIGroup.AttackArea.poly:{area_name}");
        let center = self
            .host_named_guard_area_polygon(area_name)
            .map(|(center, _, _)| center);
        if let Some(unit) = self.objects.get_mut(&unit_id) {
            unit.set_auto_acquire_when_idle(true);
            unit.attack_priority_set = Some(tag.clone());
        }
        let victim = self.find_attack_area_victim(
            unit_id,
            center.unwrap_or(glam::Vec3::ZERO),
            1.0,
            Some(area_name),
        );
        if let Some(vid) = victim {
            self.unit_command_script_attack_with_effects(unit_id, vid, effects)?;
            if let Some(unit) = self.objects.get_mut(&unit_id) {
                unit.set_auto_acquire_when_idle(true);
                unit.attack_priority_set = Some(tag);
            }
        } else if let Some(dest) = center {
            let _ = self.unit_command_script_attack_move_to_with_effects(unit_id, dest, effects);
            if let Some(unit) = self.objects.get_mut(&unit_id) {
                unit.attack_priority_set = Some(tag);
            }
        }
        Ok(())
    }
}

#[cfg(test)]
#[test]
#[cfg(not(target_arch = "wasm32"))]
fn waypoint_follow_clears_adjust_for_aircraft() {
    super::sequential_actor_tests::isolated(
        module_path!(),
        "waypoint_follow_clears_adjust_for_aircraft",
        || {
            use crate::game_logic::{GameLogic, Team};
            use game_engine::common::ini::ini_locomotor::load_locomotors_from_str;
            // CPP AIUpdate.cpp:2339–2380 rejects a NULL current locomotor even
            // for infantry. Bind real authored locomotors through normal creation;
            // a KindOf-only Object::new fixture cannot model ground movement.
            assert_eq!(
                load_locomotors_from_str(
                    r#"
Locomotor WaypointFixtureGround
  Surfaces = GROUND
  Speed = 20
  SpeedDamaged = 20
  TurnRate = 180
  Acceleration = 100
  AccelerationDamaged = 100
  Braking = 100
  Appearance = TWO_LEGS
End
Locomotor WaypointFixtureAir
  Surfaces = AIR
  Speed = 30
  SpeedDamaged = 30
  TurnRate = 180
  Acceleration = 100
  AccelerationDamaged = 100
  Braking = 100
  Appearance = HOVER
  PreferredHeight = 50
  AllowAirborneMotiveForce = Yes
End
"#
                )
                .expect("authored waypoint locomotors"),
                2
            );
            let mut logic = GameLogic::new();
            // Use the actual registered C++ class, not the old projection-only
            // AIUpdate shorthand (ModuleFactory.cpp:396).
            logic
                .admit_receiving_module_content(
                    crate::assets::ReceivingObjectContent::from_sources([(
                        "waypoint_fixture.ini".to_owned(),
                        r#"
Object WaypointFixtureAircraft
  KindOf = VEHICLE AIRCRAFT
  Body = ActiveBody ModuleTag_Body
    MaxHealth = 100
    InitialHealth = 100
  End
  Behavior = AIUpdateInterface ModuleTag_AI
  End
  Locomotor = SET_NORMAL WaypointFixtureAir
End
Object WaypointFixtureInfantry
  KindOf = INFANTRY
  Body = ActiveBody ModuleTag_Body
    MaxHealth = 100
    InitialHealth = 100
  End
  Behavior = AIUpdateInterface ModuleTag_AI
  End
  Locomotor = SET_NORMAL WaypointFixtureGround
End
"#
                        .to_owned(),
                    )])
                    .unwrap(),
                )
                .unwrap();
            let air = logic
                .create_object("WaypointFixtureAircraft", Team::USA, glam::Vec3::ZERO)
                .unwrap();
            let foot = logic
                .create_object(
                    "WaypointFixtureInfantry",
                    Team::USA,
                    glam::Vec3::new(10.0, 0.0, 0.0),
                )
                .unwrap();
            // CPP ScriptActions.cpp:1623 chooses SET_NORMAL before following.
            assert!(logic.apply_unit_locomotor_set(air, "normal"));
            assert!(logic.apply_unit_locomotor_set(foot, "normal"));
            assert!(logic.unit_can_move(air));
            assert!(logic.unit_can_move(foot));
            assert!(logic.host_object(air).unwrap().cur_locomotor_name.is_some());
            assert!(logic
                .host_object(foot)
                .unwrap()
                .cur_locomotor_name
                .is_some());
            assert!(
                !crate::game_logic::PathfindingGrid::is_doing_ground_movement_full(
                    logic.host_object(air).unwrap()
                )
            );
            assert!(
                crate::game_logic::PathfindingGrid::is_doing_ground_movement_full(
                    logic.host_object(foot).unwrap()
                )
            );
            assert!(logic.host_object(air).unwrap().adjust_destinations());
            assert!(logic.host_object(foot).unwrap().adjust_destinations());
            let wps = [glam::Vec3::new(40.0, 0.0, 0.0)];
            logic.host_script_issue_follow_waypoint_path(&[air], &wps, false, false, None);
            logic.host_script_issue_follow_waypoint_path(&[foot], &wps, false, false, None);
            assert!(
                !logic.host_object(air).unwrap().adjust_destinations(),
                "an aircraft waypoint path must not adjust"
            );
            assert!(
                logic.host_object(foot).unwrap().adjust_destinations(),
                "infantry on the ground keeps the move-request flag"
            );
            assert_eq!(
                logic.host_object(foot).unwrap().path_goal_position,
                Some(wps[0]),
                "actual waypoint command admitted"
            );
        },
    );
}

#[cfg(test)]
#[path = "area_owner_tests.rs"]
mod area_owner_tests;
