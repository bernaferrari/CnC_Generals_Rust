//! Canonical session radar-provider admission and immediate lifecycle edges.
use super::super::*;

impl GameLogic {
    /// C++ RadarUpgrade contributions in authored module order.
    fn host_radar_provider_slots(object: &Object) -> impl ExactSizeIterator<Item = (bool, &[u32])> {
        object
            .thing()
            .template
            .radar_upgrade_rules
            .as_deref()
            .unwrap_or_default()
            .iter()
            .enumerate()
            .map(|(index, rule)| {
                let contributors = object
                    .radar_upgrade_contributors
                    .get(index)
                    .map(Vec::as_slice)
                    .unwrap_or_default();
                (rule.disable_proof, contributors)
            })
    }

    fn host_radar_providers(&self, id: ObjectId) -> Vec<Vec<(u32, bool)>> {
        let Some(object) = self.objects.get(&id) else {
            return Vec::new();
        };
        Self::host_radar_provider_slots(object)
            .map(|(proof, owners)| owners.iter().map(|&owner| (owner, proof)).collect())
            .collect()
    }

    fn host_radar_providers_are_current(&self, id: ObjectId) -> bool {
        let admitted = self.host_radar.providers.get(&id);
        let Some(object) = self.objects.get(&id) else {
            return admitted.is_none();
        };
        let slots = Self::host_radar_provider_slots(object);
        let Some(admitted) = admitted else {
            return slots.len() == 0;
        };
        !admitted.is_empty()
            && slots.len() == admitted.len()
            && slots.zip(admitted).all(|((proof, owners), recorded)| {
                owners.iter().map(|&owner| (owner, proof)).eq(recorded.iter().copied())
            })
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
        let Some(rules) = &object.thing().template.radar_upgrade_rules else {
            return;
        };
        let resize = object.radar_upgrade_executed.len() != rules.len()
            || object.radar_upgrade_contributors.len() != rules.len();
        if !resize && object.radar_upgrade_executed.iter().all(|&executed| executed) {
            return;
        }
        let rules = rules.clone();
        if resize {
            let object = self.objects.get_mut(&id).unwrap();
            object.radar_upgrade_executed.resize(rules.len(), false);
            object.radar_upgrade_contributors.resize(rules.len(), Vec::new());
        }
        for (index, rule) in rules.iter().enumerate() {
            if self.objects[&id].radar_upgrade_executed[index] {
                continue;
            }
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
            for upgrade in &rule.removes_upgrades {
                self.remove_object_upgrade_with_receiving_rules(id, upgrade);
            }
            let object = self.objects.get_mut(&id).unwrap();
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

    pub(in crate::game_logic) fn capture_authored_radar_upgrade_slot(
        &mut self,
        id: ObjectId,
        index: usize,
    ) {
        self.admit_host_radar_disabled_edges(id);
        let Some(object) = self.objects.get(&id).filter(|object| {
            !object.is_disabled()
                && object.radar_upgrade_executed.get(index).copied().unwrap_or(false)
        }) else {
            return;
        };
        let Some(rules) = &object.thing().template.radar_upgrade_rules else {
            return;
        };
        let Some(rule) = rules.get(index) else {
            return;
        };
        let proof = rule.disable_proof;
        let owner = self.player_owner_for_host_object(object);
        let contributors = &mut self.objects.get_mut(&id).unwrap().radar_upgrade_contributors[index];
        // One callback transfers one contribution. Duplicate grants remain.
        let old = contributors.pop();
        if let Some(owner) = owner {
            contributors.push(owner);
        }
        if let Some(old) = old {
            self.change_host_radar_provider(old, proof, false);
        }
        if let Some(owner) = owner {
            self.change_host_radar_provider(owner, proof, true);
        }
        self.host_radar.providers.insert(id, self.host_radar_providers(id));
    }

    pub(crate) fn capture_authored_radar_provider_with_effects(
        &mut self,
        id: ObjectId,
        effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) {
        self.admit_host_radar_disabled_edges(id);
        if self.objects.get(&id).is_some_and(Object::is_disabled) {
            return;
        }
        let slots = self.objects.get(&id)
            .and_then(|object| object.thing().template.radar_upgrade_rules.as_ref())
            .map_or(0, |rules| rules.len());
        for index in 0..slots {
            self.capture_authored_radar_upgrade_slot(id, index);
        }
        self.admit_host_radar_provider_with_effects(id, effects);
    }

    /// Restored Player counts remain canonical; rebuild only derived identities.
    pub(crate) fn rebuild_host_radar_provider_admission(&mut self) {
        self.host_radar.providers = self
            .objects
            .registered_keys()
            .filter_map(|&id| {
                let providers = self.host_radar_providers(id);
                (!providers.is_empty()).then_some((id, providers))
            })
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
        let has_radar_work = self.objects.get(&id).is_some_and(|object| {
            !object.pending_radar_disabled_edges.is_empty()
                || object
                    .thing()
                    .template
                    .radar_upgrade_rules
                    .as_ref()
                    .is_some_and(|rules| {
                        !rules.is_empty()
                            || !object.radar_upgrade_executed.is_empty()
                            || !object.radar_upgrade_contributors.is_empty()
                    })
        });
        if !has_radar_work && !self.host_radar.providers.contains_key(&id) {
            return;
        }
        self.admit_host_radar_disabled_edges(id);
        self.attempt_authored_radar_upgrades_with_effects(id, effects);
        if self.host_radar_providers_are_current(id) {
            return;
        }
        let next = self.host_radar_providers(id);
        let previous = if next.is_empty() {
            self.host_radar.providers.remove(&id)
        } else {
            self.host_radar.providers.insert(id, next.clone())
        }
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
    #[cfg(test)]
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
        if self
            .objects
            .get(&id)
            .is_none_or(|object| object.pending_radar_disabled_edges.is_empty())
        {
            return;
        }
        let edges = self.objects.get_mut(&id).unwrap().take_radar_disabled_edges();
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

    /// RadarUpgrade::onDelete removes one contribution from the current controller.
    pub(super) fn delete_radar_upgrade_slot(&mut self, id: ObjectId, index: usize) {
        self.admit_host_radar_disabled_edges(id);
        let Some(object) = self.objects.get(&id).filter(|object| {
            !object.is_disabled()
                && object
                    .radar_upgrade_executed
                    .get(index)
                    .copied()
                    .unwrap_or(false)
        }) else {
            return;
        };
        let Some(rule) = object
            .thing()
            .template
            .radar_upgrade_rules
            .as_ref()
            .and_then(|rules| rules.get(index))
        else {
            return;
        };
        let proof = rule.disable_proof;
        if let Some(owner) = self.player_owner_for_host_object(object) {
            self.change_host_radar_provider(owner, proof, false);
        }
        let object = self.objects.get_mut(&id).unwrap();
        object.radar_upgrade_executed[index] = false;
        object.radar_upgrade_contributors[index].clear();
        if let Some(slot) = self
            .host_radar
            .providers
            .get_mut(&id)
            .and_then(|slots| slots.get_mut(index))
        {
            slot.clear();
        }
    }

    pub(in super::super) fn remove_host_radar_provider(&mut self, id: ObjectId) {
        self.admit_host_radar_disabled_edges(id);
        let authored_slots = self
            .objects
            .get(&id)
            .and_then(|object| object.thing().template.radar_upgrade_rules.as_ref())
            .map(|rules| rules.len());
        if let Some(count) = authored_slots {
            for index in 0..count {
                self.delete_radar_upgrade_slot(id, index);
            }
            // Disabled native modules skip their counter callback.
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
        if (!online && !offline)
            || !self.host_logic_step_active
            || self.frame == 0
            || self.host_radar.client_quiet
        {
            return;
        }
        let Some(sound) = self.session_view.definitions.radar_edge_sound(online) else {
            return;
        };
        if self.players.get(&owner).is_some_and(|player| {
            player.is_alive
                && !self
                    .victory_conditions
                    .has_single_player_been_defeated_owned(
                        player,
                        &self.players,
                        self.objects.read(),
                        self.player_template_definitions.as_ref(),
                    )
        }) {
            self.queue_audio_event(
                AudioEventRequest::new(sound)
                    .with_player_index(owner as i32)
                    .with_priority(130),
            );
        }
    }
}

#[cfg(test)]
#[path = "radar_provider_owner_tests.rs"]
mod tests;
