//! Host script team, containment, trigger, and attack behavior.
#![allow(unused_imports, non_snake_case)]
use super::*;

impl GameLogic {
    /// C++ `doTeamGuardPosition` / `Object` / `Area` / `InTunnelNetwork`.
    /// Leftover queues [`gamelogic::scripting::HostScriptGuardVariantRequest`].
    pub(super) fn apply_host_guard_variant_script_requests(&mut self) {
        use gamelogic::scripting::HostScriptGuardVariantRequest;
        for req in gamelogic::scripting::take_host_script_guard_variant_requests() {
            match req {
                HostScriptGuardVariantRequest::TeamGuardPosition { team, waypoint } => {
                    let Some(dest) = self.host_script_waypoint_position(&waypoint) else {
                        continue;
                    };
                    for id in self.host_script_team_member_ids(&team) {
                        if self.host_script_unit_can_guard(id) {
                            let _ = self.unit_command_guard_position(id, dest);
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
                    let poly_radius =
                        crate::game_logic::GameLogic::host_named_guard_area_polygon(&area)
                            .map(|(_, r, _)| r);
                    for id in self.host_script_team_member_ids(&team) {
                        if self.host_script_unit_can_guard(id) {
                            let _ = self.unit_command_guard_position(id, dest);
                            if let Some(u) = self.objects.get_mut(&id) {
                                u.guard_area_trigger = Some(area.clone());
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
        for module in &obj.thing.template.special_power_modules {
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
            .thing
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
        let Some(id) = self.host_object_id_by_script_name(unit) else {
            return;
        };
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
        self.queue_command(crate::command_system::GameCommand {
            command_type: crate::command_system::CommandType::DoSpecialPower {
                power_type: power,
                target,
            },
            player_id,
            command_id: 0,
            timestamp: std::time::SystemTime::now(),
            selected_units: vec![id],
            modifier_keys: crate::command_system::ModifierKeys::default(),
        });
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
            .iter()
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
                    self.host_script_create_object(name.as_deref(), &thing, &team, x, y, z, angle);
                }
                HostScriptCreateRequest::ReinforcementTeam { team, waypoint } => {
                    self.host_script_create_reinforcement_team(&team, &waypoint);
                }
            }
        }
    }

    /// C++ `ScriptActions::doNamedSetBoobytrapped` / `doTeamSetBoobytrapped`.
    /// Leftover queues [`gamelogic::scripting::HostScriptBoobytrapRequest`].
    pub(super) fn apply_host_boobytrap_script_requests(&mut self) {
        use gamelogic::scripting::HostScriptBoobytrapRequest;
        for req in gamelogic::scripting::take_host_script_boobytrap_requests() {
            match req {
                HostScriptBoobytrapRequest::Named { thing, unit } => {
                    if let Some(id) = self.host_object_id_by_script_name(&unit) {
                        self.host_script_plant_boobytrap(&thing, id);
                    }
                }
                HostScriptBoobytrapRequest::Team { thing, team } => {
                    for id in self.host_script_team_member_ids(&team) {
                        self.host_script_plant_boobytrap(&thing, id);
                    }
                }
            }
        }
    }

    /// C++ `ScriptActions::doNamedSetUnmanned` / `doTeamSetUnmanned` /
    /// `deleteAllUnmanned`. Leftover queues [`gamelogic::scripting::HostScriptUnmannedRequest`].
    pub(in crate::game_logic::game_logic) fn apply_host_unmanned_script_requests(&mut self) {
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
                        .values()
                        .filter(|obj| obj.status.disabled_unmanned && !obj.status.destroyed)
                        .map(|obj| obj.id)
                        .collect();
                    for id in ids {
                        self.destroy_object(id);
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
            obj.apply_kill_pilot_unmanned();
            obj.deselect();
            obj.set_team(Team::Neutral);
        }
        self.selected_objects.retain(|sid| *sid != id);
        for player in self.players.values_mut() {
            player.selected_objects.retain(|sid| *sid != id);
        }
    }

    /// C++ `doObjectRadarCreateEvent` / `doTeamRadarCreateEvent`.
    pub(in crate::game_logic::game_logic) fn apply_host_radar_event_script_requests(&mut self) {
        use crate::game_logic::host_radar::host_create_radar_event;
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
            host_create_radar_event(pos, Self::host_script_radar_event_type(event_type));
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
    pub(super) fn host_script_plant_boobytrap(&mut self, thing: &str, target_id: ObjectId) {
        use crate::game_logic::host_booby_trap::BOOBY_TRAP_OBJECT;

        let thing = thing.trim();
        if thing.is_empty() {
            return;
        }
        let lower = thing.to_ascii_lowercase();
        // C++ only inits when the new object has StickyBombUpdate.
        if !(lower.contains("boobytrap")
            || lower.contains("sticky")
            || lower.contains("democharge")
            || lower.contains("remotecharge"))
        {
            return;
        }
        let Some(obj) = self.objects.get(&target_id) else {
            return;
        };
        if !obj.is_alive() || obj.status.destroyed {
            return;
        }
        let team = obj.team;
        let owner = obj.owner_player_id;
        let geom = obj.selection_radius.max(8.0);
        let p = obj.get_position();
        let pos = glam::Vec3::new(p.x, p.y + 8.0, p.z);
        let frame = self.frame;

        let charge = if self.templates.contains_key(thing) {
            self.create_object_for_owner_or_team(thing, team, owner, pos)
        } else if thing.eq_ignore_ascii_case(BOOBY_TRAP_OBJECT) {
            self.spawn_booby_trap_special_object(target_id, team, target_id)
        } else {
            None
        };
        let Some(cid) = charge else {
            return;
        };

        let is_booby_kind = thing.to_ascii_lowercase().contains("boobytrap");
        if let Some(o) = self.objects.get_mut(&cid) {
            o.booby_trap_special = true;
            o.booby_trap_attached_to = Some(target_id);
            o.producer_id = Some(target_id);
        }
        if is_booby_kind {
            let _ = self
                .booby_trap
                .install(target_id, target_id, team, frame, geom, Some(cid));
            if let Some(target) = self.objects.get_mut(&target_id) {
                target.set_status_booby_trapped(true);
            }
        }
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
                let _ = self.unit_command_attack_move_to(id, dest);
            }
        }

        let buttons =
            gamelogic::scripting::take_host_skirmish_command_button_most_valuable_requests();
        for (team, ability, range) in buttons {
            self.host_script_skirmish_command_button_on_most_valuable(&team, &ability, range);
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
        use crate::command_executor::CommandExecutor;
        use gamelogic::object::update::special_power_update::SpecialPowerCommandOption;

        if !Self::leftover_skirmish_command_button_exists(ability) {
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
        let options = Self::leftover_skirmish_command_button_options(ability);
        let requires_object_target = options.intersects(
            SpecialPowerCommandOption::NEED_TARGET_ENEMY_OBJECT
                | SpecialPowerCommandOption::NEED_TARGET_NEUTRAL_OBJECT
                | SpecialPowerCommandOption::NEED_TARGET_ALLY_OBJECT
                | SpecialPowerCommandOption::NEED_TARGET_PRISONER,
        );

        let mut best: Option<(i32, crate::game_logic::ObjectId)> = None;
        for obj in self.objects.values() {
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
            let cost = obj.thing.template.build_cost.supplies as i32;
            if best.map(|(c, _)| cost > c).unwrap_or(true) {
                best = Some((cost, obj.id));
            }
        }
        let Some((_, tid)) = best else {
            return;
        };
        let _ = CommandExecutor::new(self, pid).execute_do_command_button(
            &members,
            ability,
            None,
            Some(tid),
        );
    }

    /// C++ `TheControlBar->findCommandButton` / leftover host-queue payload.
    /// Empty INI catalog falls back to the live button mapper.
    pub(super) fn leftover_skirmish_command_button_exists(ability: &str) -> bool {
        use crate::command_system::command_type_from_button_name;
        if ability.trim().is_empty() {
            return false;
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
        ability: &str,
    ) -> gamelogic::object::update::special_power_update::SpecialPowerCommandOption {
        use gamelogic::object::update::special_power_update::SpecialPowerCommandOption;
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
            for obj in self.objects.values() {
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
    pub(super) fn host_script_nearest_enemy_group_with_value(
        &self,
        attacker_pid: u32,
        origin: glam::Vec3,
        value: i32,
    ) -> Option<glam::Vec3> {
        use crate::game_logic::partition_manager::PARTITION_CELL_SIZE_RESIDUAL as CELL;
        use gamelogic::object::collide::partition_manager::ValueOrThreat;
        use std::collections::{HashMap, HashSet, VecDeque};

        let enemy_mask = self.host_script_enemy_player_mask(attacker_pid);
        if enemy_mask == 0 {
            return None;
        }

        let source = gamelogic::common::Coord3D::new(origin.x, origin.z, 0.0);
        if let Some(loc) = gamelogic::helpers::ThePartitionManager::get().and_then(|pm| {
            pm.get_nearest_group_with_value(
                attacker_pid as i32,
                enemy_mask,
                ValueOrThreat::CashValue,
                &source,
                value,
                true,
            )
        }) {
            return Some(glam::Vec3::new(loc.x, 0.0, loc.y));
        }

        let mut cells: HashMap<(i32, i32), i32> = HashMap::new();
        for obj in self.objects.values() {
            if !obj.is_alive() || obj.status.destroyed || obj.status.effectively_dead {
                continue;
            }
            if obj.status.under_construction {
                continue;
            }
            let Some(pid) = obj.owner_player_id else {
                continue;
            };
            let bit = 1u32 << pid.min(15);
            if (enemy_mask & bit) == 0 {
                continue;
            }
            let pos = obj.get_position();
            let cx = (pos.x / CELL).floor() as i32;
            let cz = (pos.z / CELL).floor() as i32;
            let cash = if obj.partition_cash_value > 0 {
                obj.partition_cash_value as i32
            } else {
                obj.thing.template.build_cost.supplies as i32
            };
            *cells.entry((cx, cz)).or_insert(0) += cash;
        }
        if cells.is_empty() {
            return None;
        }

        // C++ `parms.greaterThan = valueRequired` (nonzero Int → strict >).
        let greater_than = value != 0;
        let start = (
            (origin.x / CELL).floor() as i32,
            (origin.z / CELL).floor() as i32,
        );
        let mut min_x = start.0;
        let mut min_z = start.1;
        let mut max_x = start.0;
        let mut max_z = start.1;
        for &(cx, cz) in cells.keys() {
            min_x = min_x.min(cx);
            min_z = min_z.min(cz);
            max_x = max_x.max(cx);
            max_z = max_z.max(cz);
        }
        let max_x_ex = max_x + 1;
        let max_z_ex = max_z + 1;

        let mut visited: HashSet<(i32, i32)> = HashSet::new();
        let mut queue: VecDeque<(i32, i32)> = VecDeque::new();
        visited.insert(start);
        queue.push_back(start);
        while let Some((cx, cz)) = queue.pop_front() {
            let neighbors = [(cx - 1, cz), (cx, cz - 1), (cx + 1, cz), (cx, cz + 1)];
            for n in neighbors {
                if n.0 >= min_x
                    && n.0 < max_x_ex
                    && n.1 >= min_z
                    && n.1 < max_z_ex
                    && visited.insert(n)
                {
                    queue.push_back(n);
                }
            }
            let cash = cells.get(&(cx, cz)).copied().unwrap_or(0);
            let passes = if greater_than {
                cash > value
            } else {
                cash < value
            };
            if passes {
                return Some(glam::Vec3::new(cx as f32 * CELL, 0.0, cz as f32 * CELL));
            }
        }
        None
    }

    pub(super) fn host_script_coord_to_world(x: f32, y: f32, z: f32) -> glam::Vec3 {
        // Generals Coord3D: (x,y) map plane, z = height.
        glam::Vec3::new(x, z, y)
    }

    pub(super) fn host_script_create_team(&self, team_name: &str) -> crate::game_logic::Team {
        if let Ok(factory) = gamelogic::team::get_team_factory().lock() {
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
        if let Some(unit_name) = name.filter(|n| !n.is_empty()) {
            if let Some(id) = self.host_object_id_by_script_name(unit_name) {
                if self
                    .objects
                    .get(&id)
                    .is_some_and(|obj| obj.is_alive() && !obj.status.destroyed)
                {
                    return None;
                }
            }
        }
        let team = self.host_script_create_team(team_name);
        let mut pos = Self::host_script_coord_to_world(x, y, z);
        if z == 0.0 {
            if let Some(h) = self.terrain_height_at(glam::Vec3::new(pos.x, 0.0, pos.z)) {
                pos.y = h;
            }
        }
        let id = self.create_object(thing, team, pos)?;
        if let Some(obj) = self.objects.get_mut(&id) {
            obj.set_orientation(angle);
            if !team_name.trim().is_empty() {
                obj.team_instance_name = team_name.to_string();
            }
            if let Some(unit_name) = name.filter(|n| !n.is_empty()) {
                obj.name = unit_name.to_string();
            }
        }
        Some(id)
    }

    pub(super) fn host_script_create_reinforcement_team(
        &mut self,
        team_name: &str,
        waypoint_name: &str,
    ) {
        let Some(dest) = self.host_script_waypoint_position(waypoint_name) else {
            return;
        };
        let mut origin = dest;
        let (start, transport, units) = {
            let Ok(factory) = gamelogic::team::get_team_factory().lock() else {
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
        let mut spawned: Vec<ObjectId> = Vec::new();
        if !transport.is_empty() {
            if let Some(id) = self.host_script_create_object(
                None, &transport, team_name, origin.x, origin.z, origin.y, 0.0,
            ) {
                spawned.push(id);
            }
        }
        let mut slot = 0i32;
        for (thing, count) in units {
            for _ in 0..count {
                let offset = slot as f32 * 5.0;
                if let Some(id) = self.host_script_create_object(
                    None,
                    &thing,
                    team_name,
                    origin.x + offset,
                    origin.z,
                    origin.y,
                    0.0,
                ) {
                    spawned.push(id);
                }
                slot += 1;
            }
        }
        if (origin - dest).length_squared() > 1.0 {
            for id in spawned {
                let _ = self.unit_command_move_to(id, dest);
            }
        }
    }

    /// C++ `ScriptActions::doTeamHuntWithCommandButton` live drain.
    pub(super) fn host_script_team_hunt_with_command_button(&mut self, team: &str, button: &str) {
        // C++ findCommandButton + command-type switch before any unit is armed.
        // Leftover `command_button_is_hunt_capable` is the same gate.
        if !Self::leftover_command_button_is_hunt_capable(button) {
            return;
        }
        let ids = self.host_script_hunt_guard_team_member_ids(team);
        let button = if button.is_empty() {
            None
        } else {
            Some(button)
        };
        for id in ids {
            if self.unit_can_team_hunt_with_command_button(id, button) {
                let _ = self.start_command_button_hunt_named(id, button);
            }
        }
    }

    /// C++ `Object::leaveGroup` before PLAYER_HUNT `aiHunt`.
    /// Live formation_id is the AIGroup/formation membership leftover `group_id` maps to.
    pub(super) fn host_object_leave_group(&mut self, id: ObjectId) {
        if let Some(unit) = self.objects.get_mut(&id) {
            if unit.formation_id != 0 || unit.formation_offset != glam::Vec2::ZERO {
                unit.set_formation(0, glam::Vec2::ZERO);
            }
        }
        let _ = gamelogic::object::registry::OBJECT_REGISTRY.with_object_mut(id.0, |obj| {
            obj.leave_group();
        });
    }

    /// C++ `getAIUpdateInterface` + `getCurLocomotor`. Structures have no
    /// current locomotor; generic test scouts without KindOf still stamp.
    pub(super) fn host_script_member_has_cur_locomotor(
        obj: &crate::game_logic::object::Object,
    ) -> bool {
        use crate::game_logic::KindOf;
        !obj.is_kind_of(KindOf::Structure)
    }

    /// C++ enter/garrison/exit family live drain (`aiEnter` / `aiEvacuate` / `aiExit`).
    pub fn apply_host_garrison_enter_exit_script_requests(&mut self) {
        use gamelogic::scripting::HostScriptGarrisonEnterExitRequest;
        for req in gamelogic::scripting::take_host_script_garrison_enter_requests() {
            match req {
                HostScriptGarrisonEnterExitRequest::NamedEnter { unit, dest } => {
                    let Some(unit_id) = self.host_object_id_by_script_name(&unit) else {
                        continue;
                    };
                    let Some(dest_id) = self.host_object_id_by_script_name(&dest) else {
                        continue;
                    };
                    self.host_script_ai_enter(unit_id, dest_id);
                }
                HostScriptGarrisonEnterExitRequest::TeamEnter { team, dest } => {
                    let Some(dest_id) = self.host_object_id_by_script_name(&dest) else {
                        continue;
                    };
                    let members = self.host_script_garrison_team_member_ids(&team);
                    for id in members {
                        self.host_script_ai_enter(id, dest_id);
                    }
                }
                HostScriptGarrisonEnterExitRequest::NamedExitAll { unit } => {
                    let Some(id) = self.host_object_id_by_script_name(&unit) else {
                        continue;
                    };
                    self.host_script_ai_evacuate(id);
                }
                HostScriptGarrisonEnterExitRequest::TeamExitAll { team } => {
                    for id in self.host_script_garrison_team_member_ids(&team) {
                        self.host_script_ai_evacuate(id);
                    }
                }
                HostScriptGarrisonEnterExitRequest::TeamGarrisonSpecific { team, building } => {
                    let Some(building_id) = self.host_object_id_by_script_name(&building) else {
                        continue;
                    };
                    let members = self.host_script_garrison_team_member_ids(&team);
                    let player_id = members
                        .first()
                        .and_then(|id| self.host_object(*id).and_then(|o| o.owner_player_id));
                    if !self.host_script_building_can_garrison(building_id, player_id) {
                        continue;
                    }
                    for id in members {
                        self.host_script_ai_enter(id, building_id);
                    }
                }
                HostScriptGarrisonEnterExitRequest::ExitSpecificBuilding { building } => {
                    let Some(id) = self.host_object_id_by_script_name(&building) else {
                        continue;
                    };
                    if !self
                        .host_object(id)
                        .is_some_and(|o| o.is_kind_of(crate::game_logic::KindOf::Structure))
                    {
                        continue;
                    }
                    self.host_script_ai_evacuate(id);
                }
                HostScriptGarrisonEnterExitRequest::TeamGarrisonNearest { team } => {
                    self.host_script_team_garrison_nearest(&team);
                }
                HostScriptGarrisonEnterExitRequest::TeamExitAllBuildings { team } => {
                    for id in self.host_script_garrison_team_member_ids(&team) {
                        self.host_script_ai_exit(id);
                    }
                }
                HostScriptGarrisonEnterExitRequest::NamedGarrisonSpecific { unit, building } => {
                    let Some(unit_id) = self.host_object_id_by_script_name(&unit) else {
                        continue;
                    };
                    let Some(building_id) = self.host_object_id_by_script_name(&building) else {
                        continue;
                    };
                    let player_id = self.host_object(unit_id).and_then(|o| o.owner_player_id);
                    if !self.host_script_building_can_garrison(building_id, player_id) {
                        continue;
                    }
                    self.host_script_ai_enter(unit_id, building_id);
                }
                HostScriptGarrisonEnterExitRequest::NamedGarrisonNearest { unit } => {
                    self.host_script_named_garrison_nearest(&unit);
                }
                HostScriptGarrisonEnterExitRequest::NamedExitBuilding { unit } => {
                    let Some(id) = self.host_object_id_by_script_name(&unit) else {
                        continue;
                    };
                    self.host_script_ai_exit(id);
                }
                HostScriptGarrisonEnterExitRequest::PlayerGarrisonAll { player } => {
                    self.host_script_player_garrison_all(&player);
                }
                HostScriptGarrisonEnterExitRequest::PlayerExitAll { player } => {
                    self.host_script_player_exit_all(&player);
                }
            }
        }
    }

    pub(super) fn host_script_garrison_team_member_ids(&self, team: &str) -> Vec<ObjectId> {
        let mut ids = self.host_script_team_member_ids(team);
        ids.sort_by_key(|id| id.0);
        ids
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

    pub(super) fn host_script_ai_enter(&mut self, unit: ObjectId, dest: ObjectId) {
        use crate::game_logic::AIState;
        let Some(pos) = self.host_object(dest).map(|obj| obj.get_position()) else {
            return;
        };
        self.host_object_leave_group(unit);
        let _ = self.apply_unit_locomotor_set(unit, "normal");
        let _ = self.unit_command_stop_moving_order_target(unit, Some(dest));
        if !self.unit_command_path_with_state_ignoring(unit, pos, AIState::Entering, Some(dest)) {
            if let Some(obj) = self.host_object_mut(unit) {
                obj.ignored_obstacle_id = Some(dest);
                obj.set_ai_state(AIState::Entering);
            }
        }
    }

    pub(super) fn host_script_ai_evacuate(&mut self, id: ObjectId) {
        self.host_object_leave_group(id);
        let _ = self.apply_unit_locomotor_set(id, "normal");
        let _ = self.evacuate_container_now(id, false);
    }

    pub(super) fn host_script_ai_exit(&mut self, id: ObjectId) {
        self.host_object_leave_group(id);
        let _ = self.apply_unit_locomotor_set(id, "normal");
        let Some(cid) = self.host_object(id).and_then(|obj| obj.contained_by) else {
            return;
        };
        let pos = self
            .host_object(cid)
            .map(|obj| obj.get_position())
            .unwrap_or(glam::Vec3::ZERO);
        let _ = self.unit_command_exit_drop(id, pos);
    }

    pub(super) fn host_script_is_garrison_nearest_candidate(
        &self,
        building: &crate::game_logic::object::Object,
        leader_off_map: bool,
        leader_is_hacker: bool,
        player_id: Option<u32>,
    ) -> bool {
        use crate::game_logic::host_deliver_payload::is_off_map_default_residual;
        use crate::game_logic::KindOf;
        if !building.is_alive() || building.status.destroyed {
            return false;
        }
        if is_off_map_default_residual(building.get_position()) != leader_off_map {
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

    pub(super) fn host_script_team_garrison_nearest(&mut self, team: &str) {
        use crate::game_logic::host_deliver_payload::is_off_map_default_residual;
        use crate::game_logic::KindOf;
        let members = self.host_script_garrison_team_member_ids(team);
        let Some(&leader_id) = members.first() else {
            return;
        };
        let Some((leader_pos, leader_off_map, leader_is_hacker, player_id)) =
            self.host_object(leader_id).map(|obj| {
                (
                    obj.get_position(),
                    is_off_map_default_residual(obj.get_position()),
                    obj.is_kind_of(KindOf::MoneyHacker),
                    obj.owner_player_id,
                )
            })
        else {
            return;
        };
        let mut buildings: Vec<(f32, ObjectId)> = self
            .objects
            .values()
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
                self.host_script_ai_enter(member_id, building_id);
                filled += 1;
            }
            if member_idx >= members.len() {
                break;
            }
        }
    }

    pub(super) fn host_script_named_garrison_nearest(&mut self, unit: &str) {
        use crate::game_logic::host_deliver_payload::is_off_map_default_residual;
        use crate::game_logic::KindOf;
        let Some(unit_id) = self.host_object_id_by_script_name(unit) else {
            return;
        };
        let Some((unit_pos, unit_off_map, unit_is_hacker, player_id)) =
            self.host_object(unit_id).map(|obj| {
                (
                    obj.get_position(),
                    is_off_map_default_residual(obj.get_position()),
                    obj.is_kind_of(KindOf::MoneyHacker),
                    obj.owner_player_id,
                )
            })
        else {
            return;
        };
        let mut best: Option<(f32, ObjectId)> = None;
        for obj in self.objects.values() {
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
            return;
        };
        self.host_script_ai_enter(unit_id, building_id);
    }

    pub(super) fn host_script_player_garrison_all(&mut self, player: &str) {
        use crate::game_logic::KindOf;
        let Some(pid) = self.host_player_id_for_script_token(player) else {
            return;
        };
        let unit_ids: Vec<ObjectId> = self
            .objects
            .values()
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
            for obj in self.objects.values() {
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
            self.host_script_ai_enter(unit_id, building_id);
        }
    }

    pub(super) fn host_script_player_exit_all(&mut self, player: &str) {
        use crate::game_logic::KindOf;
        let Some(pid) = self.host_player_id_for_script_token(player) else {
            return;
        };
        let ids: Vec<ObjectId> = self
            .objects
            .values()
            .filter(|obj| {
                obj.is_alive()
                    && !obj.status.destroyed
                    && obj.owner_player_id == Some(pid)
                    && obj.is_kind_of(KindOf::Structure)
            })
            .map(|obj| obj.id)
            .collect();
        for id in ids {
            self.host_script_ai_evacuate(id);
        }
    }

    /// C++ `ScriptEngine::getTeamNamed` / leftover `TeamFactory::find_team`.
    /// First leftover instance members, else live `team_instance_name` — never faction Team.
    pub(super) fn host_script_hunt_guard_team_member_ids(&self, team_name: &str) -> Vec<ObjectId> {
        let needle = team_name.trim();
        if needle.is_empty() {
            return Vec::new();
        }
        let leftover_ids: Vec<ObjectId> = gamelogic::team::get_team_factory()
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
    pub(super) fn leftover_command_button_is_hunt_capable(ability: &str) -> bool {
        if ability.is_empty() {
            return false;
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
        let faction = Self::resolve_host_team_name(team_name);
        self.objects
            .values()
            .filter(|obj| {
                obj.is_alive()
                    && !obj.status.destroyed
                    && (faction.map(|t| obj.team == t).unwrap_or(false)
                        || (!obj.team_instance_name.is_empty()
                            && obj.team_instance_name.eq_ignore_ascii_case(needle))
                        || obj.team.get_name().eq_ignore_ascii_case(needle))
            })
            .map(|obj| obj.id)
            .collect()
    }

    /// C++ `ThePartitionManager->getClosestObject` (template or ObjectTypes,
    /// `FROM_CENTER_2D`, polygon trigger, same on/off-map).
    pub(super) fn host_script_closest_object_of_type_in_trigger(
        &self,
        source_id: ObjectId,
        object_type: &str,
        trigger_name: &str,
    ) -> Option<ObjectId> {
        let source = self.host_object(source_id)?;
        let source_pos = source.get_position();
        let source_off_map = source.position.x < self.world_min.x
            || source.position.x > self.world_max.x
            || source.position.z < self.world_min.z
            || source.position.z > self.world_max.z;
        let type_names = host_script_object_type_names(object_type);
        if type_names.is_empty() {
            return None;
        }
        let mut best: Option<(ObjectId, f32)> = None;
        for (id, obj) in self.host_objects() {
            if *id == source_id || !obj.is_alive() || obj.status.destroyed {
                continue;
            }
            let off_map = obj.position.x < self.world_min.x
                || obj.position.x > self.world_max.x
                || obj.position.z < self.world_min.z
                || obj.position.z > self.world_max.z;
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
        if let Some(trigger) =
            gamelogic::scripting::host_script_lookup_polygon_trigger(trigger_name)
        {
            return trigger.point_in_trigger_int(&gamelogic::common::ICoord3D::new(
                pos.x as i32,
                pos.z as i32,
                0,
            ));
        }
        if let Some((min_x, min_z, max_x, max_z)) =
            gamelogic::scripting::host_script_area_bounds(trigger_name)
        {
            return pos.x >= min_x && pos.x <= max_x && pos.z >= min_z && pos.z <= max_z;
        }
        false
    }

    pub(super) fn host_script_leftover_waypoint(
        &self,
        waypoint_name: &str,
    ) -> Option<(u32, glam::Vec3)> {
        let name = gamelogic::common::AsciiString::from(waypoint_name);
        let (wid, loc) = gamelogic::terrain::get_terrain_logic()
            .read()
            .ok()
            .and_then(|terrain| {
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

    /// C++ `TheTerrainLogic->getClosestWaypointOnPath` then `link[0]` chain.
    pub(super) fn host_script_waypoint_path_from(
        &self,
        path_label: &str,
        from: glam::Vec3,
    ) -> Option<Vec<glam::Vec3>> {
        let leftover_pos = gamelogic::common::Coord3D::new(from.x, from.z, from.y);
        let terrain = gamelogic::terrain::get_terrain_logic().read().ok()?;
        let start = terrain.get_closest_waypoint_on_path(&leftover_pos, path_label)?;
        let chain = terrain.walk_link0_chain(start, gamelogic::terrain::WAYPOINT_PATH_LIMIT);
        if chain.is_empty() {
            return None;
        }
        Some(
            chain
                .into_iter()
                .map(|wp| {
                    let loc = *wp.get_location();
                    let mut pos = glam::Vec3::new(loc.x, loc.z, loc.y);
                    if let Some(h) = self.terrain_height_at(glam::Vec3::new(pos.x, 0.0, pos.z)) {
                        pos.y = h;
                    }
                    pos
                })
                .collect(),
        )
    }

    /// C++ `TheTerrainLogic->getClosestWaypointOnPath` id for checkBridges.
    pub(super) fn host_script_closest_waypoint_id(
        &self,
        path_label: &str,
        from: glam::Vec3,
    ) -> Option<u32> {
        let leftover_pos = gamelogic::common::Coord3D::new(from.x, from.z, from.y);
        let terrain = gamelogic::terrain::get_terrain_logic().read().ok()?;
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
        path_label: &str,
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
        let last = *waypoints.last().unwrap();
        let labels = leftover_waypoint_path_labels(path_label, last);
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
            let _ = self.unit_command_waypoint_path_prep(unit_id, as_team);
            let assigned = if exact {
                self.assign_unit_path_exact(unit_id, goal, via)
            } else {
                self.assign_unit_path(unit_id, goal, via)
            };
            if assigned {
                if let Some(unit) = self.host_object_mut(unit_id) {
                    unit.stamp_pending_waypoint_labels(labels.iter().cloned());
                }
            }
        }
    }

    pub(super) fn host_script_area_center(&self, area_name: &str) -> Option<glam::Vec3> {
        if let Ok(terrain) = gamelogic::terrain::get_terrain_logic().read() {
            if let Some(trigger) = terrain.get_trigger_area_by_name(area_name) {
                let c = trigger.get_center_point();
                let mut pos = glam::Vec3::new(c.x, c.z, c.y);
                if let Some(h) = self.terrain_height_at(glam::Vec3::new(pos.x, 0.0, pos.z)) {
                    pos.y = h;
                }
                return Some(pos);
            }
        }
        for (name, (min_x, min_z, max_x, max_z)) in
            gamelogic::scripting::engine::get_area_tracker().all_area_aabbs()
        {
            if name.eq_ignore_ascii_case(area_name) {
                let mut pos = glam::Vec3::new((min_x + max_x) * 0.5, 0.0, (min_z + max_z) * 0.5);
                if let Some(h) = self.terrain_height_at(pos) {
                    pos.y = h;
                }
                return Some(pos);
            }
        }
        None
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

    pub(super) fn host_script_attack_team(&mut self, unit_id: ObjectId, victim_team: &str) {
        if let Some(team) = Self::resolve_host_team_name(victim_team) {
            if team != crate::game_logic::Team::Neutral {
                if let Some(unit) = self.objects.get_mut(&unit_id) {
                    unit.set_max_shots_to_fire(-1);
                    unit.auto_acquire_when_idle = true;
                    unit.attack_priority_set =
                        Some(format!("AIGroup.AttackTeam.{}", team.get_name()));
                }
            }
        }
        if let Some(vid) = self.host_script_nearest_team_victim(unit_id, victim_team) {
            let _ = self.unit_command_attack(unit_id, vid);
            if let Some(team) = Self::resolve_host_team_name(victim_team) {
                if team != crate::game_logic::Team::Neutral {
                    if let Some(unit) = self.objects.get_mut(&unit_id) {
                        unit.set_max_shots_to_fire(-1);
                        unit.auto_acquire_when_idle = true;
                        unit.attack_priority_set =
                            Some(format!("AIGroup.AttackTeam.{}", team.get_name()));
                    }
                }
            }
        }
    }

    pub(super) fn host_script_attack_area(&mut self, unit_id: ObjectId, area_name: &str) {
        let tag = format!("AIGroup.AttackArea.poly:{area_name}");
        let center = self.host_script_area_center(area_name);
        if let Some(unit) = self.objects.get_mut(&unit_id) {
            unit.auto_acquire_when_idle = true;
            unit.attack_priority_set = Some(tag.clone());
        }
        let victim = self.find_attack_area_victim(
            unit_id,
            center.unwrap_or(glam::Vec3::ZERO),
            1.0,
            Some(area_name),
        );
        if let Some(vid) = victim {
            let _ = self.unit_command_attack(unit_id, vid);
            if let Some(unit) = self.objects.get_mut(&unit_id) {
                unit.auto_acquire_when_idle = true;
                unit.attack_priority_set = Some(tag);
            }
        } else if let Some(dest) = center {
            let _ = self.unit_command_attack_move_to(unit_id, dest);
            if let Some(unit) = self.objects.get_mut(&unit_id) {
                unit.attack_priority_set = Some(tag);
            }
        }
    }
}
