//! Canonical session radar-provider admission and immediate lifecycle edges.
use super::super::*;

impl GameLogic {
    fn host_radar_providers(&self, id: ObjectId) -> Vec<Vec<(u32, bool)>> {
        let Some(obj) = self.objects.get(&id) else {
            return Vec::new();
        };
        use crate::game_logic::host_radar::*;
        use crate::game_logic::host_upgrades::*;
        let owner = self.player_owner_for_host_object(obj);
        if let Some(rules) = &obj.thing().template.radar_upgrade_rules {
            return rules
                .iter()
                .enumerate()
                .map(|(index, rule)| {
                    obj.radar_upgrade_contributors
                        .get(index)
                        .into_iter()
                        .flatten()
                        .map(|&contributor| (contributor, rule.disable_proof))
                        .collect()
                })
                .collect();
        }
        // Synthesized seed templates have no authored module list. This adapter
        // is deliberately unreachable for parsed definitions, even Some(empty).
        if is_disabled_for_radar(obj.is_disabled(), obj.status.under_construction)
            || !is_legal_radar_provider(
                obj.is_alive(),
                obj.is_constructed() && !obj.status.under_construction,
                obj.is_command_center() || obj.is_kind_of(KindOf::CommandCenter),
                &obj.template_name,
            )
        {
            return Vec::new();
        }
        let Some(owner) = owner else {
            return Vec::new();
        };
        if let Some(required) = radar_provider_required_research_upgrade(&obj.template_name) {
            let researched = self.players.get(&owner).is_some_and(|player| {
                player.has_unlocked_upgrade(required)
                    || player
                        .unlocked_sciences
                        .iter()
                        .any(|name| normalize_upgrade_identity(name).contains("chinaradar"))
            });
            let tagged = obj.has_upgrade_tag(required)
                || obj
                    .applied_upgrades
                    .iter()
                    .any(|name| normalize_upgrade_identity(name).contains("chinaradar"));
            if !researched && !tagged {
                return Vec::new();
            }
        }
        vec![vec![(owner, is_radar_van_template(&obj.template_name))]]
    }

    /// The combined player/object upgrade mask is observed once per module.
    /// Executed remains true when triggers subsequently disappear (UpgradeMux).
    fn attempt_authored_radar_upgrades(&mut self, id: ObjectId) {
        self.attempt_authored_radar_upgrades_with_effects(id, &mut Default::default())
    }

    fn attempt_authored_radar_upgrades_with_effects(
        &mut self,
        id: ObjectId,
        effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) {
        let Some(object) = self.objects.get(&id) else {
            return;
        };
        if object.status.under_construction || object.status.destroyed {
            return;
        }
        let Some(owner) = self.player_owner_for_host_object(object) else {
            return;
        };
        let Some(rules) = object.thing().template.radar_upgrade_rules.clone() else {
            return;
        };
        {
            let object = self.objects.get_mut(&id).unwrap();
            object.radar_upgrade_executed.resize(rules.len(), false);
            object
                .radar_upgrade_contributors
                .resize(rules.len(), Vec::new());
        }
        for (index, rule) in rules.iter().enumerate() {
            // Resolve effective definitions through this session's admitted
            // immutable catalog, before borrowing the mutable object.
            if rule
                .triggered_by
                .iter()
                .chain(&rule.conflicts_with)
                .chain(&rule.removes_upgrades)
                .any(|name| self.upgrade_template(name).is_none())
            {
                continue;
            }
            let object = &self.objects[&id];
            if object.radar_upgrade_executed[index] {
                continue;
            }
            let player = &self.players[&owner];
            let matches = rule.would_upgrade(|name| {
                object.has_upgrade_tag(name)
                    || player.has_unlocked_upgrade(name)
                    || player.unlocked_sciences.iter().any(|owned| {
                        crate::game_logic::host_upgrades::normalize_upgrade_identity(owned)
                            == crate::game_logic::host_upgrades::normalize_upgrade_identity(name)
                    })
            });
            if !matches {
                continue;
            }
            if let Some(fx) = &rule.fx_list {
                self.dispatch_fx_list_at_host_object_with_effects(fx, id, None, effects);
            }
            let object = self.objects.get_mut(&id).unwrap();
            for upgrade in &rule.removes_upgrades {
                object.remove_upgrade_tag(upgrade);
            }
            object.radar_upgrade_executed[index] = true;
            // C++ upgradeImplementation adds radar even if disabled already;
            // a later real onDisabled callback is what removes this contribution.
            object.radar_upgrade_contributors[index].push(owner);
            if let Some(frames) = object.thing().template.radar_extend_frames {
                object.extend_radar(self.frame.wrapping_add(frames));
                self.radar_extend_starts = self.radar_extend_starts.saturating_add(1);
            }
        }
    }

    pub(crate) fn capture_authored_radar_provider(&mut self, id: ObjectId) {
        self.capture_authored_radar_provider_with_effects(id, &mut Default::default())
    }

    pub(crate) fn capture_authored_radar_provider_with_effects(
        &mut self,
        id: ObjectId,
        effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) {
        self.admit_host_radar_disabled_edges(id);
        let Some(object) = self.objects.get(&id) else {
            return;
        };
        let Some(rules) = object.thing().template.radar_upgrade_rules.clone() else {
            self.admit_host_radar_provider_with_effects(id, effects);
            return;
        };
        if object.is_disabled() {
            return;
        }
        let owner = self.player_owner_for_host_object(object);
        let executed = object.radar_upgrade_executed.clone();
        for (index, rule) in rules.iter().enumerate() {
            if !executed.get(index).copied().unwrap_or(false) {
                continue;
            }
            let contributors = &mut self
                .objects
                .get_mut(&id)
                .unwrap()
                .radar_upgrade_contributors[index];
            // One module callback removes one previous controller contribution,
            // then adds one new contribution; exceptional duplicate grants remain.
            let old = contributors.pop();
            if let Some(owner) = owner {
                contributors.push(owner);
            }
            if let Some(old) = old {
                self.change_host_radar_provider(old, rule.disable_proof, false);
            }
            if let Some(owner) = owner {
                self.change_host_radar_provider(owner, rule.disable_proof, true);
            }
        }
        self.host_radar
            .providers
            .insert(id, self.host_radar_providers(id));
        self.admit_host_radar_provider_with_effects(id, effects);
    }

    /// Restored Player counts remain canonical; rebuild only derived identities.
    pub(crate) fn rebuild_host_radar_provider_admission(&mut self) {
        self.host_radar.providers = self
            .objects
            .registered_keys()
            .map(|&id| (id, self.host_radar_providers(id)))
            .collect();
    }

    pub(crate) fn admit_host_radar_provider(&mut self, id: ObjectId) {
        self.admit_host_radar_provider_with_effects(id, &mut Default::default())
    }

    pub(crate) fn admit_host_radar_provider_with_effects(
        &mut self,
        id: ObjectId,
        effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) {
        self.admit_host_radar_disabled_edges(id);
        self.attempt_authored_radar_upgrades_with_effects(id, effects);
        let next = self.host_radar_providers(id);
        let previous = self
            .host_radar
            .providers
            .insert(id, next.clone())
            .unwrap_or_default();
        for index in 0..previous.len().max(next.len()) {
            let mut old = previous.get(index).cloned().unwrap_or_default();
            let mut new = next.get(index).cloned().unwrap_or_default();
            let mut i = 0;
            while i < old.len() {
                if let Some(j) = new.iter().position(|value| *value == old[i]) {
                    old.remove(i);
                    new.remove(j);
                } else {
                    i += 1;
                }
            }
            for (owner, proof) in old {
                self.change_host_radar_provider(owner, proof, false);
            }
            for (owner, proof) in new {
                self.change_host_radar_provider(owner, proof, true);
            }
        }
    }

    /// Run a bounded object status mutation and admit its callbacks before the
    /// next script/behavior observes Player radar. No ambient owner selection.
    pub(crate) fn with_owned_radar_disabled_operation<R>(
        &mut self,
        id: ObjectId,
        operation: impl FnOnce(&mut Object) -> R,
    ) -> Option<R> {
        let object = self.objects.get_mut(&id)?;
        let result = operation(object);
        self.admit_host_radar_disabled_edges(id);
        Some(result)
    }

    pub(crate) fn with_owned_radar_disabled_partition_operation<R>(
        &mut self,
        id: ObjectId,
        operation: impl FnOnce(
            &mut Object,
            &mut crate::game_logic::partition_manager::PartitionManager,
        ) -> R,
    ) -> Option<R> {
        let result = operation(self.objects.get_mut(&id)?, &mut self.partition_manager);
        self.admit_host_radar_disabled_edges(id);
        Some(result)
    }

    /// Consume original callback order, including disable→enable in one operation.
    pub(in super::super) fn admit_host_radar_disabled_edges(&mut self, id: ObjectId) {
        let edges = self
            .objects
            .get_mut(&id)
            .map(Object::take_radar_disabled_edges)
            .unwrap_or_default();
        for edge in edges {
            let Some(owner) = edge.player_id else {
                continue;
            };
            let providers = self.host_radar.providers.entry(id).or_default();
            if providers.len() <= edge.module_index {
                providers.resize(edge.module_index + 1, Vec::new());
            }
            if edge.becoming_disabled {
                if let Some(index) = providers[edge.module_index]
                    .iter()
                    .position(|&(id, _)| id == owner)
                {
                    providers[edge.module_index].remove(index);
                }
            } else {
                providers[edge.module_index].push((owner, edge.disable_proof));
            }
            // C++ callback adjusts the current controller unconditionally,
            // including exceptional disabled grants and owner changes.
            self.change_host_radar_provider(owner, edge.disable_proof, !edge.becoming_disabled);
        }
    }

    pub(in super::super) fn remove_host_radar_provider(&mut self, id: ObjectId) {
        self.admit_host_radar_disabled_edges(id);
        let authored = self
            .objects
            .get(&id)
            .and_then(|object| object.thing().template.radar_upgrade_rules.clone());
        if let Some(rules) = authored {
            let Some(object) = self.objects.get(&id) else {
                return;
            };
            if !object.is_disabled() {
                let owner = self.player_owner_for_host_object(object);
                let executed = object.radar_upgrade_executed.clone();
                for (index, rule) in rules.iter().enumerate() {
                    if executed.get(index).copied().unwrap_or(false) {
                        if let Some(owner) = owner {
                            self.change_host_radar_provider(owner, rule.disable_proof, false);
                        }
                    }
                }
                let object = self.objects.get_mut(&id).unwrap();
                object.radar_upgrade_executed.fill(false);
                object
                    .radar_upgrade_contributors
                    .iter_mut()
                    .for_each(Vec::clear);
            }
            // Disabled onDelete skips the counter callback. Physical removal
            // drops derived metadata without repairing original odd counts.
            self.host_radar.providers.remove(&id);
        } else if let Some(providers) = self.host_radar.providers.remove(&id) {
            for (owner, proof) in providers.into_iter().flatten() {
                self.change_host_radar_provider(owner, proof, false);
            }
        }
    }

    fn change_host_radar_provider(&mut self, owner: u32, proof: bool, add: bool) {
        let Some(player) = self.players.get_mut(&owner) else {
            return;
        };
        let had = player.has_radar();
        if add {
            player.add_radar(proof);
        } else {
            player.remove_radar(proof);
        }
        let count = player.radar_count.max(0) as u32;
        let has = player.has_radar();
        self.record_host_radar_transition(owner, count, had, has);
    }

    pub(crate) fn set_radar_client_quiet(&mut self, quiet: bool) {
        self.host_radar.client_quiet = quiet;
    }

    pub(in super::super) fn record_host_radar_transition(
        &mut self,
        owner: u32,
        count: u32,
        had: bool,
        has: bool,
    ) {
        let (online, offline) = self.host_radar.record_player_radar(count, had, has);
        let allowed = self.host_logic_step_active
            && self.frame > 0
            && !self.host_radar.client_quiet
            && self.players.get(&owner).is_some_and(|player| {
                player.is_alive
                    && !self
                        .victory_conditions
                        .has_single_player_been_defeated_owned(
                            player,
                            &self.players,
                            self.objects.read(),
                            self.player_template_definitions.as_ref(),
                        )
            });
        if !allowed {
            return;
        }
        let sound = if online {
            Some(crate::game_logic::host_radar::RADAR_ONLINE_AUDIO)
        } else if offline {
            Some(crate::game_logic::host_radar::RADAR_OFFLINE_AUDIO)
        } else {
            None
        };
        if let Some(sound) = sound {
            self.queue_audio_event(AudioEventRequest::new(sound).with_priority(130));
        }
    }
}

#[cfg(test)]
#[path = "radar_provider_owner_tests.rs"]
mod tests;
