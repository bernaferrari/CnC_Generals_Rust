//! Receiving player and object upgrade application.
//! The production AI/combat passes live in world_tick; no second update loop belongs here.
#![allow(unused_imports, non_snake_case)]
use super::super::*;
use super::weapon_upgrades::UpgradeRecipients;

impl GameLogic {
    pub(in super::super) fn update_player_upgrades(&mut self) {
        self.update_player_upgrades_with_effects(&mut Default::default())
    }

    pub(in super::super) fn update_player_upgrades_with_effects(
        &mut self,
        effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) {
        // Residual: complete research when residual frames elapse for entries
        // that are NOT still advancing on a building PRODUCTION_UPGRADE queue.
        // Building-path completions are applied in `update_production`.
        // Frame event clear runs at the start of the production phase so
        // building-path `record_complete` events survive presentation freeze.

        use crate::game_logic::buildings::ProductionKind;
        use crate::game_logic::host_upgrades::HostUpgradePhase;

        // Upgrades currently researching on a producer building.
        let mut building_researching: std::collections::HashSet<(u32, String)> =
            std::collections::HashSet::new();
        for obj in self.objects.registered_values() {
            let Some(building) = obj.building_data.as_ref() else {
                continue;
            };
            let Some(player_id) = self.player_owner_for_host_object(obj) else {
                continue;
            };
            for item in &building.production_queue {
                if item.kind == ProductionKind::Upgrade {
                    building_researching.insert((
                        player_id,
                        crate::game_logic::host_upgrades::normalize_upgrade_identity(
                            &item.template_name,
                        ),
                    ));
                }
            }
        }

        let frame = self.frame;
        let mut completed: Vec<(Team, u32, String)> = Vec::new();
        let mut cancelled: Vec<(u32, String)> = Vec::new();
        for entry in self.host_upgrades.entries_snapshot() {
            if entry.phase != HostUpgradePhase::Queued {
                continue;
            }
            let key = (
                entry.player_id,
                crate::game_logic::host_upgrades::normalize_upgrade_identity(&entry.name),
            );
            // Building owns the timer while the PRODUCTION_UPGRADE entry is live.
            if building_researching.contains(&key) {
                continue;
            }
            // C++ ProductionUpdate.cpp:636-648 / 1109-1112 — research lives
            // only on the producer's queue. Sold, dead, or a vanished queue
            // must cancel; never complete leftover residual frames.
            if let Some(source_id) = entry.source_object {
                let queue_live = self.objects.get(&source_id).is_some_and(|obj| {
                    obj.is_alive()
                        && !obj.status.sold
                        && obj.building_data.as_ref().is_some_and(|building| {
                            building.production_queue.iter().any(|item| {
                                item.kind == ProductionKind::Upgrade
                                    && crate::game_logic::host_upgrades::normalize_upgrade_identity(
                                        &item.template_name,
                                    ) == key.1
                            })
                        })
                });
                if !queue_live {
                    cancelled.push((entry.player_id, entry.name.clone()));
                    continue;
                }
            }
            let needed = entry.residual_research_frames.max(1);
            // Count the current simulation step as one research frame residual
            // (frame counter increments after update_simulation returns).
            let elapsed = frame.saturating_sub(entry.queue_frame).saturating_add(1);
            if elapsed >= needed {
                completed.push((entry.team, entry.player_id, entry.name.clone()));
            }
        }

        for (player_id, name) in cancelled {
            if let Some(player) = self.players.get_mut(&player_id) {
                if let Some(queued) = player.find_queued_upgrade_name(&name) {
                    player.queued_upgrades.remove(&queued);
                }
            }
            self.host_upgrades.record_cancel(&name, player_id);
        }

        // Direct player.queue_upgrade without host record (unit-test path):
        // complete after one simulation frame residual.
        for player in self.players.values() {
            for name in &player.queued_upgrades {
                let key = (
                    player.id,
                    crate::game_logic::host_upgrades::normalize_upgrade_identity(name),
                );
                if building_researching.contains(&key) {
                    continue;
                }
                let already = completed
                    .iter()
                    .any(|(t, pid, n)| *pid == player.id && n.eq_ignore_ascii_case(name));
                if already {
                    continue;
                }
                // No host entry → residual complete this update (legacy test path).
                let has_host = self.host_upgrades.entries_snapshot().iter().any(|e| {
                    e.player_id == player.id
                        && e.phase == HostUpgradePhase::Queued
                        && crate::game_logic::host_upgrades::normalize_upgrade_identity(&e.name)
                            == key.1
                });
                if !has_host {
                    completed.push((player.team, player.id, name.clone()));
                }
            }
        }

        for (team, player_id, name) in completed {
            let already = self
                .players
                .get(&player_id)
                .map(|p| p.has_unlocked_upgrade(&name))
                .unwrap_or(false);
            let upgrade_type = self.upgrade_type(&name);
            let definition = self.upgrade_template(&name);
            if let (Some(player), Some(definition)) =
                (self.players.get_mut(&player_id), definition.as_deref())
            {
                player.record_upgrade_production_spending(definition);
            }
            if let Some(player) = self.players.get_mut(&player_id) {
                player.complete_researched_upgrade(&name, upgrade_type);
            }
            if !already {
                self.apply_host_upgrade_complete_with_effects(team, player_id, &name, effects);
            }
            if let (Some(player), Some(definition)) =
                (self.players.get_mut(&player_id), definition.as_deref())
            {
                player.record_upgrade_production_academy(definition, self.frame);
            }
        }
    }

    /// Record that a player queued upgrade research (host residual honesty).
    pub fn record_host_upgrade_queued(
        &mut self,
        player_id: u32,
        team: Team,
        upgrade_name: &str,
        source_object: Option<ObjectId>,
    ) {
        self.host_upgrades
            .record_queue(upgrade_name, team, player_id, self.frame, source_object);
    }

    /// Record that a player cancelled upgrade research (host residual honesty).
    pub fn record_host_upgrade_cancelled(&mut self, player_id: u32, upgrade_name: &str) {
        self.host_upgrades.record_cancel(upgrade_name, player_id);
    }

    /// Apply unlock effects for a completed upgrade and record honesty.
    /// Matches C++ ProductionUpdate upgrade-complete: player mask + object giveUpgrade.

    /// C++ StatusBitsUpgrade::upgradeImplementation residual for team units.

    /// C++ PassengersFireUpgrade residual for Helix BattleBunker unlock.

    /// C++ ActiveShroudUpgrade::upgradeImplementation residual.
    pub fn apply_active_shroud_upgrade(&mut self, id: ObjectId, new_shroud_range: f32) -> bool {
        let Some(obj) = self.objects.get_mut(&id) else {
            return false;
        };
        obj.set_shroud_range(new_shroud_range);
        self.active_shroud_upgrade_reg
            .record_apply(obj.shroud_range);
        // C++ ActiveShroudUpgrade::upgradeImplementation: handlePartitionCellMaintenance
        // so Object::shroud cover applies without waiting for the next look tick.
        self.update_main_crate_vision();
        true
    }

    pub(in super::super) fn apply_active_shroud_upgrade_to_team(
        &mut self,
        recipients: UpgradeRecipients,
        upgrade_name: &str,
    ) -> u32 {
        use crate::game_logic::host_active_shroud_upgrade::{
            peel_applies_to_template, peels_for_upgrade,
        };
        let peels = peels_for_upgrade(upgrade_name);
        if peels.is_empty() {
            return 0;
        }
        let ids: Vec<ObjectId> = self
            .objects
            .registered()
            .filter(|(_, o)| o.is_alive() && recipients.matches_object(o))
            .map(|(id, _)| *id)
            .collect();
        let mut n = 0u32;
        for id in ids {
            let Some(obj) = self.objects.get_mut(&id) else {
                continue;
            };
            for peel in &peels {
                if !peel_applies_to_template(peel, &obj.template_name) {
                    continue;
                }
                obj.set_shroud_range(peel.new_shroud_range);
                self.active_shroud_upgrade_reg
                    .record_apply(obj.shroud_range);
                n = n.saturating_add(1);
            }
        }
        if n > 0 {
            self.update_main_crate_vision();
        }
        n
    }

    pub(in super::super) fn apply_passengers_fire_upgrade_to_team(
        &mut self,
        recipients: UpgradeRecipients,
        upgrade_name: &str,
    ) -> u32 {
        use crate::game_logic::host_passengers_fire_upgrade::should_enable_passengers_fire;
        if !crate::game_logic::host_passengers_fire_upgrade::is_passengers_fire_upgrade(
            upgrade_name,
        ) {
            return 0;
        }
        let ids: Vec<ObjectId> = self
            .objects
            .registered()
            .filter(|(_, o)| o.is_alive() && recipients.matches_object(o))
            .filter(|(_, o)| should_enable_passengers_fire(upgrade_name, &o.template_name))
            .map(|(id, _)| *id)
            .collect();
        let mut n = 0u32;
        for id in ids {
            if let Some(obj) = self.objects.get_mut(&id) {
                obj.passengers_allowed_to_fire = true;
                n = n.saturating_add(1);
            }
        }
        if n > 0 {
            self.passengers_fire_upgrade_reg.record_apply(n);
        }
        n
    }

    #[cfg(test)]
    pub(in super::super) fn apply_status_bits_upgrade_to_team(
        &mut self,
        recipients: UpgradeRecipients,
        upgrade_name: &str,
    ) -> u32 {
        use crate::game_logic::host_status_bits_upgrade::collect_status_bits_for_object;
        let ids: Vec<ObjectId> = self
            .objects
            .registered()
            .filter(|(_, o)| o.is_alive() && recipients.matches_object(o))
            .map(|(id, _)| *id)
            .collect();
        let mut touched = 0u32;
        for id in ids {
            let Some(object) = self.objects.get(&id) else {
                continue;
            };
            let pairs = collect_status_bits_for_object(upgrade_name, object);
            let mut any = false;
            for (set, clear) in &pairs {
                let set_refs: Vec<&str> = set.iter().map(String::as_str).collect();
                let clear_refs: Vec<&str> = clear.iter().map(String::as_str).collect();
                if let Some((set_c, clear_c)) = self.apply_object_status_bits_upgrade_with_effects(
                    id,
                    &set_refs,
                    &clear_refs,
                    &mut Default::default(),
                ) {
                    self.status_bits_upgrade_reg.record_apply(set_c, clear_c);
                    any = true;
                }
            }
            if any {
                touched = touched.saturating_add(1);
            }
        }
        touched
    }

    pub(crate) fn apply_host_upgrade_complete(
        &mut self,
        team: Team,
        player_id: u32,
        upgrade_name: &str,
    ) {
        self.apply_host_upgrade_complete_with_effects(
            team,
            player_id,
            upgrade_name,
            &mut Default::default(),
        )
    }

    pub(crate) fn apply_host_upgrade_complete_with_effects(
        &mut self,
        team: Team,
        player_id: u32,
        upgrade_name: &str,
        effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) {
        self.apply_host_upgrade_complete_with_interpreter_and_effects(
            team,
            player_id,
            upgrade_name,
            None,
            effects,
        );
    }

    pub(crate) fn apply_host_upgrade_complete_with_interpreter_and_effects(
        &mut self,
        team: Team,
        player_id: u32,
        upgrade_name: &str,
        interpreter: Option<&gamelogic::scripting::engine::ScriptEngine>,
        effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) {
        let source = self
            .host_upgrades
            .last_source_object_for(player_id, upgrade_name);
        let affected = self.apply_host_upgrade_modules_from_source_with_interpreter_and_effects(
            team,
            player_id,
            upgrade_name,
            source,
            interpreter,
            effects,
        );
        self.record_host_upgrade_completion(team, player_id, upgrade_name, source, affected);
        self.present_host_upgrade_completion(team, player_id, upgrade_name, source);
    }

    pub(crate) fn apply_host_upgrade_modules_from_source_with_effects(
        &mut self,
        team: Team,
        player_id: u32,
        upgrade_name: &str,
        source: Option<ObjectId>,
        effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> u32 {
        self.apply_host_upgrade_modules_from_source_with_interpreter_and_effects(
            team,
            player_id,
            upgrade_name,
            source,
            None,
            effects,
        )
    }

    pub(crate) fn apply_host_upgrade_modules_from_source_with_interpreter_and_effects(
        &mut self,
        team: Team,
        player_id: u32,
        upgrade_name: &str,
        source: Option<ObjectId>,
        interpreter: Option<&gamelogic::scripting::engine::ScriptEngine>,
        effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> u32 {
        use crate::game_logic::host_upgrades::{HostUpgradeKind, upgrade_mux_target_ids};

        let kind = HostUpgradeKind::from_name(upgrade_name);
        let object_scoped = self.is_object_scoped_upgrade(upgrade_name);

        // C++ ProductionUpdate: PLAYER upgrades update the player mask then
        // every object `updateUpgradeModules`. OBJECT upgrades `giveUpgrade`
        // only on the producer.
        // Canonical ledger aliases are applied once; object callbacks run only
        // on the exact receiving Team prototype/instance/member traversal below.
        let ledger = UpgradeRecipients::PlayerLedger { team, player_id };
        if !object_scoped {
            let _ = self.apply_player_unlock_upgrade(ledger, upgrade_name, upgrade_name);
        }
        if !object_scoped
            && matches!(
                kind,
                HostUpgradeKind::CashBounty
                    | HostUpgradeKind::SatelliteHack
                    | HostUpgradeKind::HelixNapalmBomb
                    | HostUpgradeKind::HelixNukeBomb
            )
        {
            let _ =
                self.apply_upgrade_kind_to_recipients(ledger, upgrade_name, interpreter, effects);
        }
        let mut units_affected = 0u32;
        if object_scoped {
            if let Some(id) = source {
                units_affected += self.apply_upgrade_to_exact_recipient(
                    team,
                    player_id,
                    id,
                    upgrade_name,
                    true,
                    interpreter,
                    effects,
                ) as u32;
            }
        } else {
            let teams = self
                .team_factory
                .lock()
                .unwrap_or_else(|error| error.into_inner())
                .owned_team_instances_for_controller(player_id);
            for owned_team in teams {
                // Snapshot each current Team immediately before visiting it, so
                // synchronous changes to later Teams are visible at their turn.
                let (team_id, members) = {
                    let team = owned_team.read().unwrap_or_else(|error| error.into_inner());
                    (team.get_id(), team.get_members().to_vec())
                };
                for raw in members {
                    let id = ObjectId(raw);
                    if !self.objects.get(&id).is_some_and(|object| {
                        object.owner_player_id == Some(player_id)
                            && object.team_instance_id == Some(team_id)
                    }) {
                        continue;
                    }
                    units_affected += self.apply_upgrade_to_exact_recipient(
                        team,
                        player_id,
                        id,
                        upgrade_name,
                        false,
                        interpreter,
                        effects,
                    ) as u32;
                }
            }
        }

        units_affected
    }

    fn apply_upgrade_to_exact_recipient(
        &mut self,
        team: Team,
        player_id: u32,
        id: ObjectId,
        upgrade_name: &str,
        object_scoped: bool,
        interpreter: Option<&gamelogic::scripting::engine::ScriptEngine>,
        effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> bool {
        let Some(object) = self.objects.get(&id).filter(|object| {
            object.owner_player_id == Some(player_id)
                && !object.status.under_construction
                && !object.status.destroyed
        }) else {
            return false;
        };
        if !object_scoped {
            // These residuals represent concrete retail module eligibility.
            // A Player recheck visits every member, but an unrelated member
            // must not execute this family's generic residual callbacks.
            use crate::game_logic::host_upgrades::{
                HostUpgradeKind, is_camo_netting_structure_template, is_camouflage_unit_template,
                is_supply_center_template,
            };
            match HostUpgradeKind::from_name(upgrade_name) {
                HostUpgradeKind::Camouflage
                    if !object.is_kind_of(KindOf::Infantry)
                        || !is_camouflage_unit_template(&object.template_name) =>
                {
                    return false;
                }
                HostUpgradeKind::CamoNetting
                    if !is_camo_netting_structure_template(&object.template_name) =>
                {
                    return false;
                }
                HostUpgradeKind::SupplyLines
                    if !object.is_kind_of(KindOf::SupplyCenter)
                        && !is_supply_center_template(&object.template_name) =>
                {
                    return false;
                }
                _ => {}
            }
        }
        let identity = std::sync::Arc::clone(&object.visual_draw_identity_token);
        let recipient = UpgradeRecipients::Object {
            team,
            player_id,
            object_id: id,
        };
        if !object_scoped {
            self.apply_upgrade_kind_to_recipients(recipient, upgrade_name, interpreter, effects);
        }
        if !self.objects.get(&id).is_some_and(|object| {
            std::sync::Arc::ptr_eq(&object.visual_draw_identity_token, &identity)
        }) {
            return false;
        }
        if object_scoped {
            self.apply_upgrade_to_object_with_interpreter_checked(
                id,
                upgrade_name,
                interpreter,
                effects,
            )
        } else {
            self.apply_player_research_to_object_with_interpreter_checked(
                id,
                upgrade_name,
                interpreter,
                effects,
            )
        }
        .expect("owned upgrade callback admission must complete");
        if !object_scoped {
            self.apply_passengers_fire_upgrade_to_team(recipient, upgrade_name);
            self.apply_active_shroud_upgrade_to_team(recipient, upgrade_name);
        }
        true
    }

    pub(crate) fn record_host_upgrade_completion(
        &mut self,
        team: Team,
        player_id: u32,
        upgrade_name: &str,
        source: Option<ObjectId>,
        units_affected: u32,
    ) {
        use crate::game_logic::host_upgrades::HostUpgradeKind;
        let kind = HostUpgradeKind::from_name(upgrade_name);
        // Ensure registry has a queue entry even if command path skipped record
        // (e.g. direct Player::queue_upgrade in unit tests).
        self.host_upgrades.record_queue(
            upgrade_name,
            team,
            player_id,
            self.frame.saturating_sub(1),
            source,
        );
        self.host_upgrades
            .record_complete(upgrade_name, player_id, self.frame, units_affected);

        log::info!(
            "Host upgrade complete: player={} team={:?} '{}' kind={} units_affected={}",
            player_id,
            team,
            upgrade_name,
            kind.label(),
            units_affected
        );
    }

    pub(crate) fn ordered_upgrade_owner_object_ids(&self, player_id: u32) -> Vec<ObjectId> {
        let teams = self
            .team_factory
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .owned_team_instances_for_controller(player_id);
        let mut ids = Vec::new();
        for team in teams {
            let team = team.read().unwrap_or_else(|e| e.into_inner());
            let team_id = team.get_id();
            for &raw in team.get_members() {
                let id = ObjectId(raw);
                if self.objects.get(&id).is_some_and(|object| {
                    object.owner_player_id == Some(player_id)
                        && object.team_instance_id == Some(team_id)
                }) {
                    ids.push(id);
                }
            }
        }
        ids
    }

    fn apply_upgrade_kind_to_recipients(
        &mut self,
        recipients: UpgradeRecipients,
        upgrade_name: &str,
        interpreter: Option<&gamelogic::scripting::engine::ScriptEngine>,
        effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> u32 {
        use crate::game_logic::host_upgrades::HostUpgradeKind;
        let kind = HostUpgradeKind::from_name(upgrade_name);
        match kind {
            HostUpgradeKind::FlashBangGrenade => {
                self.apply_flashbang_unlock_to_team(recipients, upgrade_name)
            }
            HostUpgradeKind::TowMissile => self.apply_tow_unlock_to_team(recipients, upgrade_name),
            HostUpgradeKind::CaptureBuilding => {
                self.apply_capture_unlock_tags_to_team(recipients, upgrade_name)
            }
            HostUpgradeKind::SupplyLines => {
                self.apply_supply_lines_tags_to_team(recipients, upgrade_name)
            }
            HostUpgradeKind::NeutronShells => {
                self.apply_neutron_shells_unlock_to_team(recipients, upgrade_name)
            }
            HostUpgradeKind::BunkerBusters => {
                self.apply_bunker_busters_unlock_to_team(recipients, upgrade_name)
            }
            HostUpgradeKind::ComancheRocketPods => {
                self.apply_comanche_rocket_pods_unlock_to_team(recipients, upgrade_name)
            }
            HostUpgradeKind::SentryDroneGun => {
                self.apply_sentry_drone_gun_unlock_to_team(recipients, upgrade_name)
            }
            HostUpgradeKind::Camouflage => {
                self.apply_camouflage_unlock_to_team(recipients, upgrade_name)
            }
            HostUpgradeKind::CamoNetting => {
                self.apply_camo_netting_unlock_to_team(recipients, upgrade_name)
            }
            HostUpgradeKind::CompositeArmor => {
                self.apply_composite_armor_unlock_to_team(recipients, upgrade_name)
            }
            HostUpgradeKind::WorkerShoes => {
                self.apply_worker_shoes_unlock_to_team(recipients, upgrade_name)
            }
            HostUpgradeKind::NuclearTanks => {
                self.apply_nuclear_tanks_unlock_to_team(recipients, upgrade_name)
            }
            HostUpgradeKind::BoobyTrap => {
                self.apply_booby_trap_unlock_to_team(recipients, upgrade_name)
            }
            HostUpgradeKind::AnthraxGamma => {
                self.apply_anthrax_gamma_unlock_to_team(recipients, upgrade_name)
            }
            HostUpgradeKind::SuicideBomb => {
                self.apply_demo_suicide_bomb_unlock_to_team(recipients, upgrade_name)
            }
            HostUpgradeKind::AdvancedControlRods => {
                self.apply_advanced_control_rods_to_team(recipients, upgrade_name)
            }
            HostUpgradeKind::SubliminalMessaging => {
                self.apply_subliminal_messaging_to_team(recipients, upgrade_name)
            }
            HostUpgradeKind::ScorpionRocket => {
                self.apply_scorpion_rocket_to_team(recipients, upgrade_name)
            }
            HostUpgradeKind::ApRockets => self.apply_ap_rockets_to_team(recipients, upgrade_name),
            HostUpgradeKind::LaserMissiles => {
                self.apply_laser_missiles_to_team(recipients, upgrade_name)
            }
            HostUpgradeKind::Nationalism => {
                self.apply_nationalism_to_team(recipients, upgrade_name)
            }
            HostUpgradeKind::Fanaticism => self.apply_fanaticism_to_team(recipients, upgrade_name),

            HostUpgradeKind::ChainGuns => self.apply_chain_guns_to_team(recipients, upgrade_name),
            HostUpgradeKind::UraniumShells => {
                self.apply_uranium_shells_to_team(recipients, upgrade_name)
            }
            HostUpgradeKind::BlackNapalm => {
                self.apply_black_napalm_to_team(recipients, upgrade_name)
            }
            HostUpgradeKind::ApBullets => self.apply_ap_bullets_to_team(recipients, upgrade_name),
            HostUpgradeKind::AnthraxBeta => {
                self.apply_anthrax_beta_to_team(recipients, upgrade_name)
            }
            HostUpgradeKind::ToxinShells => {
                self.apply_toxin_shells_to_team(recipients, upgrade_name)
            }
            HostUpgradeKind::AdvancedTraining => {
                self.apply_advanced_training_to_team(recipients, upgrade_name)
            }
            HostUpgradeKind::TacticalNukeMig => {
                self.apply_tactical_nuke_mig_to_team(recipients, upgrade_name)
            }
            HostUpgradeKind::DroneArmor => self.apply_drone_armor_to_team(recipients, upgrade_name),
            HostUpgradeKind::AircraftArmor => {
                self.apply_aircraft_armor_to_team(recipients, upgrade_name)
            }
            HostUpgradeKind::ChinaMines => {
                self.apply_player_unlock_upgrade(recipients, upgrade_name, "Upgrade_ChinaMines")
            }
            HostUpgradeKind::EmpMines => {
                self.apply_player_unlock_upgrade(recipients, upgrade_name, "Upgrade_ChinaEMPMines")
            }
            HostUpgradeKind::FortifiedStructure => {
                self.apply_fortified_structure_to_team(recipients, upgrade_name)
            }
            HostUpgradeKind::Radar => match recipients {
                UpgradeRecipients::Object { object_id, .. } => {
                    self.admit_host_radar_provider_with_effects(object_id, effects);
                    1
                }
                UpgradeRecipients::Player { player_id, .. } => self
                    .apply_radar_research_to_player_with_effects(player_id, upgrade_name, effects),
                _ => 0,
            },
            HostUpgradeKind::RadarVanScan => self.apply_player_unlock_upgrade(
                recipients,
                upgrade_name,
                "Upgrade_GLARadarVanScan",
            ),
            HostUpgradeKind::ChemicalSuits => {
                self.apply_chemical_suits_to_team(recipients, upgrade_name)
            }
            HostUpgradeKind::Moab => {
                self.apply_player_unlock_upgrade(recipients, upgrade_name, "Upgrade_AmericaMOAB")
            }
            HostUpgradeKind::SatelliteHack => {
                self.apply_satellite_hack_to_team(recipients, upgrade_name)
            }
            HostUpgradeKind::Countermeasures => {
                self.apply_countermeasures_to_team(recipients, upgrade_name)
            }
            HostUpgradeKind::SlaveDrone => self
                .apply_slave_drone_upgrade_to_team_with_interpreter_and_effects(
                    recipients,
                    upgrade_name,
                    interpreter,
                    effects,
                ),
            HostUpgradeKind::CashBounty => {
                self.apply_cash_bounty_upgrade_to_team(recipients, upgrade_name)
            }
            HostUpgradeKind::HelixNapalmBomb => self.apply_helix_bomb_upgrade_to_team(
                recipients,
                upgrade_name,
                crate::game_logic::host_helix_napalm::UPGRADE_HELIX_NAPALM_BOMB,
            ),
            HostUpgradeKind::HelixNukeBomb => self.apply_helix_bomb_upgrade_to_team(
                recipients,
                upgrade_name,
                crate::game_logic::host_helix_napalm::UPGRADE_HELIX_NUKE_BOMB,
            ),
            HostUpgradeKind::Other => 0,
        }
    }

    pub(crate) fn present_host_upgrade_completion(
        &mut self,
        team: Team,
        player_id: u32,
        upgrade_name: &str,
        source: Option<ObjectId>,
    ) {
        // CPP local message/radar precede either completion sound branch.
        self.try_radar_upgrade_complete(player_id, team, upgrade_name, source);
        // C++ ProductionUpdate.cpp:888-917: only the local producer presents
        // completion audio. A valid authored ResearchSound is positional on
        // that producer and replaces EVA; UnitSpecificSound is independent
        // and is submitted after either branch.
        if self.is_local_player(player_id) {
            let completion_sounds = self
                .upgrade_template(upgrade_name)
                .filter(|template| !template.get_display_name().is_empty())
                .map(|template| {
                    let research = template.get_research_sound();
                    let unit_specific = template.get_unit_specific_sound();
                    (
                        research
                            .is_valid()
                            .then(|| research.playable_event_name().to_string()),
                        unit_specific
                            .is_valid()
                            .then(|| unit_specific.playable_event_name().to_string()),
                    )
                });

            if let Some((research_sound, unit_specific_sound)) = completion_sounds {
                if let Some(event_name) = research_sound {
                    let mut request = AudioEventRequest::new(&event_name);
                    if let Some(producer) = source {
                        request = request.with_object(producer);
                    }
                    self.queue_audio_event(request);
                } else {
                    self.try_eva_upgrade_complete(player_id);
                }

                if let Some(event_name) = unit_specific_sound {
                    let mut request = AudioEventRequest::new(&event_name);
                    if let Some(producer) = source {
                        request = request.with_object(producer);
                    }
                    self.queue_audio_event(request);
                }
            }
        }
    }
}

#[cfg(test)]
mod live_upgrade_mux_tests {
    use crate::game_logic::host_overlord_addons::UPGRADE_OVERLORD_BUNKER;
    use crate::game_logic::host_upgrades::UPGRADE_CHINA_RADAR;
    use crate::game_logic::{GameLogic, KindOf, Player, Team, ThingTemplate};
    use glam::Vec3;

    fn add_player(logic: &mut GameLogic, id: u32, team: Team) {
        if logic.get_player(id).is_none() {
            let mut player = Player::new(id, team, "TestPlayer", true);
            player.resources.supplies = 100_000;
            logic.add_player(player);
        }
    }

    fn register_upgrade_completion_sounds(
        logic: &mut GameLogic,
        name: &str,
        research_sound: Option<&str>,
        unit_specific_sound: Option<&str>,
    ) {
        let mut source = format!("Upgrade {name}\nDisplayName = CONTROLBAR:{name}\n");
        if let Some(sound) = research_sound {
            source.push_str(&format!("ResearchSound = {sound}\n"));
        }
        if let Some(sound) = unit_specific_sound {
            source.push_str(&format!("UnitSpecificSound = {sound}\n"));
        }
        source.push_str("End\n");
        let center = &mut logic.upgrade_definitions;
        {
            let mut ini = game_engine::common::ini::INI::new();
            ini.with_inline_source(&source, |ini| {
                ini.read_line()?;
                center
                    .parse_upgrade_definition(ini)
                    .map_err(|_| game_engine::common::ini::INIError::InvalidData)
            })
            .expect("register upgrade completion sounds");
        }
    }

    fn upgrade_audio_fixture(upgrade_name: &str) -> (GameLogic, crate::game_logic::ObjectId) {
        let mut logic = GameLogic::new();
        add_player(&mut logic, 0, Team::USA);
        let mut producer = ThingTemplate::new("UpgradeAudioProducer");
        producer.set_health(1000.0);
        producer.add_kind_of(KindOf::Structure);
        logic
            .templates
            .insert("UpgradeAudioProducer".into(), producer);
        let producer_id = logic
            .create_object_for_player("UpgradeAudioProducer", 0, Vec3::ZERO)
            .expect("upgrade producer");
        logic.record_host_upgrade_queued(0, Team::USA, upgrade_name, Some(producer_id));
        // Object creation may enqueue unrelated lifecycle audio; this fixture
        // measures only the subsequent upgrade-completion branch.
        logic.queued_audio_events.clear();
        (logic, producer_id)
    }

    #[test]
    fn authored_upgrade_sounds_queue_on_producer_and_suppress_eva() {
        const UPGRADE: &str = "Upgrade_TestAuthoredCompletionSounds_hq_ib5c9";
        const RESEARCH: &str = "TestResearchComplete_hq_ib5c9";
        const UNIT: &str = "TestUnitSpecificComplete_hq_ib5c9";
        let (mut logic, producer) = upgrade_audio_fixture(UPGRADE);
        register_upgrade_completion_sounds(&mut logic, UPGRADE, Some(RESEARCH), Some(UNIT));

        logic.apply_host_upgrade_complete(Team::USA, 0, UPGRADE);

        assert_eq!(
            logic
                .queued_audio_events
                .iter()
                .filter(|event| event.event_type == RESEARCH || event.event_type == UNIT)
                .count(),
            2
        );
        assert!(
            logic
                .queued_audio_events
                .iter()
                .any(|event| event.event_type == RESEARCH && event.object_id == Some(producer))
        );
        assert!(
            logic
                .queued_audio_events
                .iter()
                .any(|event| event.event_type == UNIT && event.object_id == Some(producer))
        );
        assert!(!logic.honesty_eva_upgrade_complete_ok());
        assert!(
            logic
                .queued_audio_events
                .iter()
                .all(|event| event.event_type != "UpgradeComplete")
        );
    }

    #[test]
    fn absent_research_sound_uses_eva_but_keeps_authored_unit_sound() {
        const UPGRADE: &str = "Upgrade_TestUnitOnlyCompletionSound_hq_ib5c9";
        const UNIT: &str = "TestUnitOnlyComplete_hq_ib5c9";
        let (mut logic, producer) = upgrade_audio_fixture(UPGRADE);
        register_upgrade_completion_sounds(&mut logic, UPGRADE, None, Some(UNIT));

        logic.apply_host_upgrade_complete(Team::USA, 0, UPGRADE);

        assert!(logic.honesty_eva_upgrade_complete_ok());
        let unit_events: Vec<_> = logic
            .queued_audio_events
            .iter()
            .filter(|event| event.event_type == UNIT)
            .collect();
        assert_eq!(unit_events.len(), 1);
        assert_eq!(unit_events[0].object_id, Some(producer));
        assert!(
            logic
                .queued_audio_events
                .iter()
                .all(|event| event.event_type != "UpgradeComplete")
        );
    }

    #[test]
    fn host_complete_runs_replace_object_mux() {
        // C++ Object.cpp:2410 updateUpgradeModules → ReplaceObjectUpgrade
        // (hq-xmqu4). Live complete must call apply_upgrade_to_object.
        let mut logic = GameLogic::new();
        add_player(&mut logic, 2, Team::GLA);
        let mut fake = ThingTemplate::new("FakeGLABarracks");
        fake.set_health(500.0);
        fake.add_kind_of(KindOf::Structure);
        logic.templates.insert("FakeGLABarracks".into(), fake);
        let mut real = ThingTemplate::new("GLABarracks");
        real.set_health(1000.0);
        real.add_kind_of(KindOf::Structure);
        logic.templates.insert("GLABarracks".into(), real);

        let id = logic
            .create_object_for_player("FakeGLABarracks", 2, Vec3::new(10.0, 0.0, 20.0))
            .expect("fake barracks");
        logic.record_host_upgrade_queued(2, Team::GLA, "Upgrade_BecomeRealGLABarracks", Some(id));
        logic.apply_host_upgrade_complete(Team::GLA, 2, "Upgrade_BecomeRealGLABarracks");
        assert!(
            logic.objects[&id].status.destroyed,
            "live mux queues old fake without physical erasure"
        );
        assert_eq!(
            logic.objects[&id].health.current, 500.0,
            "direct replacement does not apply body death"
        );
        assert!(logic.has_pending_destroy_work());
        logic.process_destroy_list();
        assert!(
            logic.objects.get(&id).is_none(),
            "physical flush removes old fake after replacement birth"
        );
        assert!(
            logic
                .objects
                .registered_values()
                .any(|obj| obj.template_name == "GLABarracks" && obj.is_alive()),
            "real barracks must spawn from live complete"
        );
    }

    #[test]
    fn host_complete_grants_moab_science() {
        let mut logic = GameLogic::new();
        add_player(&mut logic, 0, Team::USA);
        let mut t = ThingTemplate::new("AmericaStrategyCenter");
        t.set_health(1000.0);
        t.add_kind_of(KindOf::Structure);
        logic.templates.insert("AmericaStrategyCenter".into(), t);
        let id = logic
            .create_object_for_player("AmericaStrategyCenter", 0, Vec3::ZERO)
            .expect("strategy center");
        logic.record_host_upgrade_queued(0, Team::USA, "Upgrade_AmericaMOAB", Some(id));
        logic.apply_host_upgrade_complete(Team::USA, 0, "Upgrade_AmericaMOAB");
        let player = logic.get_player(0).expect("player");
        assert!(
            player.has_unlocked_science("SCIENCE_MOAB"),
            "GrantScienceUpgrade must run on live complete"
        );
    }

    #[test]
    fn object_scoped_bunker_does_not_fan_out() {
        // C++ OBJECT upgrade giveUpgrade is producer-only (hq-w4syv).
        let mut logic = GameLogic::new();
        add_player(&mut logic, 1, Team::China);
        let mut t = ThingTemplate::new("ChinaTankOverlord");
        t.set_health(1000.0);
        t.add_kind_of(KindOf::Vehicle);
        logic.templates.insert("ChinaTankOverlord".into(), t);

        let a = logic
            .create_object_for_player("ChinaTankOverlord", 1, Vec3::new(0.0, 0.0, 0.0))
            .expect("overlord a");
        let b = logic
            .create_object_for_player("ChinaTankOverlord", 1, Vec3::new(40.0, 0.0, 0.0))
            .expect("overlord b");
        logic.record_host_upgrade_queued(1, Team::China, UPGRADE_OVERLORD_BUNKER, Some(a));
        logic.apply_host_upgrade_complete(Team::China, 1, UPGRADE_OVERLORD_BUNKER);

        let producer = logic.host_object(a).expect("producer");
        let other = logic.host_object(b).expect("other");
        assert_eq!(
            producer.overlord_bunker_slot_capacity(),
            5,
            "producer bunker must install"
        );
        assert_eq!(
            other.overlord_bunker_slot_capacity(),
            0,
            "sibling Overlord must not receive the object upgrade"
        );
    }

    #[test]
    fn china_command_center_radar_requires_research() {
        // C++ RadarUpgrade TriggeredBy Upgrade_ChinaRadar (hq-zx8e6).
        let mut logic = GameLogic::new();
        add_player(&mut logic, 1, Team::China);
        let mut t = ThingTemplate::new("ChinaCommandCenter");
        t.set_health(2000.0);
        t.add_kind_of(KindOf::Structure)
            .add_kind_of(KindOf::CommandCenter);
        logic.templates.insert("ChinaCommandCenter".into(), t);
        let cc = logic
            .create_object_for_player("ChinaCommandCenter", 1, Vec3::ZERO)
            .expect("china cc");
        if let Some(obj) = logic.host_object_mut(cc) {
            obj.set_status_under_construction(false);
            obj.construction_percent = 1.0;
        }
        logic.update_player_radar();
        assert!(
            !logic.get_player(1).expect("player").has_radar(),
            "China CC must not grant radar before research"
        );
        logic
            .get_player_mut(1)
            .expect("player")
            .add_completed_upgrade(UPGRADE_CHINA_RADAR);
        logic.update_player_radar();
        assert!(
            logic.get_player(1).expect("player").has_radar(),
            "China CC grants radar after Upgrade_ChinaRadar"
        );
    }

    #[test]
    fn gla_command_center_does_not_grant_radar() {
        // Leftover/INI: GLACommandCenter has no RadarUpgrade (hq-0clzf).
        let mut logic = GameLogic::new();
        add_player(&mut logic, 1, Team::GLA);
        let mut t = ThingTemplate::new("GLACommandCenter");
        t.set_health(2000.0);
        t.add_kind_of(KindOf::Structure)
            .add_kind_of(KindOf::CommandCenter);
        logic.templates.insert("GLACommandCenter".into(), t);
        let cc = logic
            .create_object_for_player("GLACommandCenter", 1, Vec3::ZERO)
            .expect("gla cc");
        if let Some(obj) = logic.host_object_mut(cc) {
            obj.set_status_under_construction(false);
            obj.construction_percent = 1.0;
        }
        logic.update_player_radar();
        assert!(
            !logic.get_player(1).expect("player").has_radar(),
            "GLA CC must not invent radar without a Radar Van"
        );
    }

    #[test]
    fn emp_revokes_command_center_radar_via_leftover_disabled_edge() {
        // leftover Object::on_disabled_edge removeRadar (hq-13kyr).
        let mut logic = GameLogic::new();
        add_player(&mut logic, 1, Team::USA);
        let mut t = ThingTemplate::new("AmericaCommandCenter");
        t.set_health(2000.0);
        t.add_kind_of(KindOf::Structure)
            .add_kind_of(KindOf::CommandCenter);
        logic.templates.insert("AmericaCommandCenter".into(), t);
        let cc = logic
            .create_object_for_player("AmericaCommandCenter", 1, Vec3::ZERO)
            .expect("usa cc");
        if let Some(obj) = logic.host_object_mut(cc) {
            obj.set_status_under_construction(false);
            obj.construction_percent = 1.0;
        }
        logic.update_player_radar();
        assert!(
            logic.get_player(1).expect("player").has_radar(),
            "America CC grants radar"
        );
        if let Some((obj, health_events)) = logic.host_object_and_health_events_mut(cc) {
            obj.apply_disabled_emp(
                100,
                health_events,
                &mut crate::game_logic::ObjectAudio::record(&mut Vec::new()),
            );
        }
        logic.update_player_radar();
        assert!(
            !logic.get_player(1).expect("player").has_radar(),
            "EMP on the only CC must take radar offline"
        );
        if let Some((obj, health_events)) = logic.host_object_and_health_events_mut(cc) {
            obj.tick_disabled_emp(100, health_events);
        }
        logic.update_player_radar();
        assert!(
            logic.get_player(1).expect("player").has_radar(),
            "radar returns when EMP expires"
        );
    }

    #[test]
    fn emp_revokes_radar_van_even_when_disable_proof() {
        // Disable-proof only survives player brownout, not EMP on the van.
        let mut logic = GameLogic::new();
        add_player(&mut logic, 1, Team::GLA);
        let mut t = ThingTemplate::new("GLAVehicleRadarVan");
        t.set_health(200.0);
        t.add_kind_of(KindOf::Vehicle);
        logic.templates.insert("GLAVehicleRadarVan".into(), t);
        let van = logic
            .create_object_for_player("GLAVehicleRadarVan", 1, Vec3::ZERO)
            .expect("van");
        if let Some(obj) = logic.host_object_mut(van) {
            obj.set_status_under_construction(false);
            obj.construction_percent = 1.0;
        }
        logic.update_player_radar();
        assert!(
            logic.get_player(1).expect("player").has_radar(),
            "Radar Van grants radar"
        );
        if let Some((obj, health_events)) = logic.host_object_and_health_events_mut(van) {
            obj.apply_disabled_hacked(
                80,
                health_events,
                &mut crate::game_logic::ObjectAudio::record(&mut Vec::new()),
            );
        }
        logic.update_player_radar();
        assert!(
            !logic.get_player(1).expect("player").has_radar(),
            "hacked Radar Van must drop radar"
        );
    }

    #[test]
    fn object_upgrade_complete_stays_off_player_set() {
        let mut logic = GameLogic::new();
        add_player(&mut logic, 1, Team::China);
        let mut t = ThingTemplate::new("ChinaTankOverlord");
        t.set_health(1000.0);
        t.add_kind_of(KindOf::Vehicle);
        logic.templates.insert("ChinaTankOverlord".into(), t);
        let a = logic
            .create_object_for_player("ChinaTankOverlord", 1, Vec3::ZERO)
            .expect("overlord");
        {
            let (player, economy_events) = logic.player_and_economy_events_mut(1).unwrap();
            player.queue_upgrade(
                UPGRADE_OVERLORD_BUNKER,
                &crate::game_logic::Resources {
                    supplies: 0,
                    power: 0,
                },
                gamelogic::upgrade::UpgradeType::Object,
                economy_events,
            )
        };
        logic.record_host_upgrade_queued(1, Team::China, UPGRADE_OVERLORD_BUNKER, Some(a));
        logic.apply_host_upgrade_complete(Team::China, 1, UPGRADE_OVERLORD_BUNKER);
        logic
            .get_player_mut(1)
            .expect("player")
            .complete_researched_upgrade(
                UPGRADE_OVERLORD_BUNKER,
                gamelogic::upgrade::UpgradeType::Object,
            );
        assert!(
            !logic
                .get_player(1)
                .expect("player")
                .has_unlocked_upgrade(UPGRADE_OVERLORD_BUNKER),
            "OBJECT bunker must not stamp the player unlock set"
        );
    }

    #[test]
    fn dead_producer_cancels_residual_research() {
        use crate::game_logic::host_upgrades::HostUpgradePhase;
        let mut logic = GameLogic::new();
        add_player(&mut logic, 0, Team::USA);
        let mut t = ThingTemplate::new("AmericaWarFactory");
        t.set_health(1000.0);
        t.add_kind_of(KindOf::Structure);
        logic.templates.insert("AmericaWarFactory".into(), t);
        let id = logic
            .create_object_for_player("AmericaWarFactory", 0, Vec3::ZERO)
            .expect("factory");
        {
            let (player, economy_events) = logic.player_and_economy_events_mut(0).unwrap();
            player.queue_upgrade(
                "Upgrade_AmericaCompositeArmor",
                &crate::game_logic::Resources {
                    supplies: 0,
                    power: 0,
                },
                gamelogic::upgrade::UpgradeType::Player,
                economy_events,
            )
        };
        logic.record_host_upgrade_queued(0, Team::USA, "Upgrade_AmericaCompositeArmor", Some(id));
        logic.objects.remove(&id);
        logic.update_player_upgrades();
        let player = logic.get_player(0).expect("player");
        assert!(
            !player.has_unlocked_upgrade("Upgrade_AmericaCompositeArmor"),
            "research must die with the producer"
        );
        assert!(!player.has_queued_upgrade("Upgrade_AmericaCompositeArmor"));
        assert!(
            logic.host_upgrades().entries_snapshot().iter().any(|e| e
                .name
                .eq_ignore_ascii_case("Upgrade_AmericaCompositeArmor")
                && e.phase == HostUpgradePhase::Cancelled),
            "host residual must record cancel, not complete"
        );
    }

    #[test]
    fn vanished_producer_queue_does_not_complete_residual() {
        // C++ ProductionUpdate.cpp:636-648 empty queue returns; 647-648 SOLD
        // freezes production; 1109-1112 onDie cancelAndRefundAllProduction.
        // Residual frames must not finish research after the queue vanishes.
        use crate::game_logic::buildings::{BuildingData, BuildingType};
        use crate::game_logic::host_upgrades::HostUpgradePhase;
        let mut logic = GameLogic::new();
        add_player(&mut logic, 0, Team::USA);
        let mut t = ThingTemplate::new("AmericaWarFactory");
        t.set_health(1000.0);
        t.add_kind_of(KindOf::Structure);
        logic.templates.insert("AmericaWarFactory".into(), t);
        let id = logic
            .create_object_for_player("AmericaWarFactory", 0, Vec3::ZERO)
            .expect("factory");
        if let Some(obj) = logic.host_object_mut(id) {
            obj.building_data = Some(BuildingData::new(BuildingType::WarFactory));
        }
        {
            let (player, economy_events) = logic.player_and_economy_events_mut(0).unwrap();
            player.queue_upgrade(
                "Upgrade_AmericaCompositeArmor",
                &crate::game_logic::Resources {
                    supplies: 0,
                    power: 0,
                },
                gamelogic::upgrade::UpgradeType::Player,
                economy_events,
            )
        };
        logic.record_host_upgrade_queued(0, Team::USA, "Upgrade_AmericaCompositeArmor", Some(id));
        logic.update_player_upgrades();
        let player = logic.get_player(0).expect("player");
        assert!(
            !player.has_unlocked_upgrade("Upgrade_AmericaCompositeArmor"),
            "vanished queue must not complete residual research"
        );
        assert!(!player.has_queued_upgrade("Upgrade_AmericaCompositeArmor"));
        assert!(
            logic.host_upgrades().entries_snapshot().iter().any(|e| e
                .name
                .eq_ignore_ascii_case("Upgrade_AmericaCompositeArmor")
                && e.phase == HostUpgradePhase::Cancelled),
            "host residual must record cancel, not complete"
        );
        assert!(
            logic.host_object(id).is_some_and(|o| o.is_alive()),
            "producer is still alive — only the queue vanished"
        );
    }
}
