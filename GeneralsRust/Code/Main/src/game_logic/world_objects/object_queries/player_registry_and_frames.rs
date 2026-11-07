use super::*;

impl GameLogic {
    /// Get current frame number
    pub fn get_current_frame(&self) -> u64 {
        self.frame as u64
    }

    /// Set current frame number (for snapshot restoration)
    pub fn set_current_frame(&mut self, frame: u64) {
        self.frame = frame as u32;
        self.host_trigger_world
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .set_current_frame(self.frame);
    }

    /// Compatibility for detached/headless restore tools. A live native world
    /// must pass its actual receiver to the checked operation instead.
    pub fn clear_all_objects(&mut self) {
        self.clear_all_objects_with_effects_checked(&mut Default::default())
            .expect("bulk Object retirement must finish before roster reset");
    }

    pub(crate) fn clear_all_objects_with_effects_checked(
        &mut self,
        effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> gamelogic::GameLogicResult<()> {
        self.destroy_all_objects_immediate_with_effects_checked(effects)?;
        let ghost_parents: Vec<_> = self
            .w3d_ghosts
            .used()
            .iter()
            .filter_map(|ghost| ghost.parent_object_id().map(ObjectId))
            .collect();
        for id in ghost_parents {
            self.detach_host_fog_ghost(id);
        }
        // Recreated ObjectIDs must not inherit the old roster's comparisons.
        // Keep authoritative saved trigger flags, restored before this call.
        self.host_guard_area_occupancy.get_mut().clear();
        #[cfg(feature = "game_client")]
        {
            self.host_physics_visuals = Default::default();
        }
        self.visual_world_epoch = self.visual_world_epoch.wrapping_add(1).max(1);
        self.finish_all_building_sounds();
        self.objects.clear();
        self.ai_groups.clear_groups();
        self.beacon_client.clear();
        self.named_script_cache.clear();
        // Playback/retry deadlines are transient; restored IDs start fresh.
        self.ambient_sound_retries.clear();
        self.next_object_id = ObjectId(1);
        self.next_formation_id = 1;
        Ok(())
    }

    /// Read the next object ID counter for the snapshot (C++
    /// `GameLogic::getObjectIDCounter`, xferred by GameStateMap::xfer).
    pub fn next_object_id_for_snapshot(&self) -> ObjectId {
        self.next_object_id
    }

    /// Set the next object ID counter (for snapshot restoration).
    pub fn set_next_object_id_for_restore(&mut self, next_object_id: ObjectId) {
        self.next_object_id = next_object_id;
    }

    /// C++ TheAI::getNextFormationID residual.
    pub fn alloc_formation_id(&mut self) -> u32 {
        let id = self.next_formation_id;
        self.next_formation_id = self.next_formation_id.saturating_add(1).max(1);
        id
    }

    /// C++ `TheAI` next formation id after restoring stamped formations.
    pub fn set_next_formation_id_for_restore(&mut self, next_formation_id: u32) {
        self.next_formation_id = next_formation_id.max(1);
    }

    /// C++ `Object::m_containedByFrame` map for HealContain auto-exit.
    pub fn contained_by_frame_for_snapshot(&self, unit_id: ObjectId) -> Option<u32> {
        self.tunnel_network.contained_by_frame(unit_id)
    }

    pub fn stamp_contained_by_frame(&mut self, unit_id: ObjectId, frame: u32) {
        self.tunnel_network.stamp_contained_by_frame(unit_id, frame);
    }

    pub fn restore_contained_by_frames(&mut self, frames: &[(ObjectId, u32)]) {
        self.tunnel_network.restore_contained_by_frames(frames);
    }

    /// Clear all players (for snapshot restoration)
    pub fn clear_all_players(&mut self) {
        self.players.clear();
        self.select_local_player(None);
        self.player_template_bindings.clear();
    }

    /// Select the receiving session's one local player. C++ PlayerList stores
    /// one `m_local` pointer, independently of the players' controller types.
    pub fn select_local_player(&mut self, player_id: Option<u32>) -> bool {
        if player_id.is_some_and(|id| !self.players.contains_key(&id)) {
            return false;
        }
        self.local_player_id = player_id;
        // Client FXList::doFXObj visibility reads PlayerList::get_local_player_index
        // (C++ ThePlayerList->getLocalPlayer). Keep the world's published list in sync.
        let index = player_id.map(|id| id as i32).unwrap_or(-1);
        self.with_own_services(|| {
            if let Ok(mut list) = gamelogic::player::player_list().write() {
                list.set_local_player_index(index);
            }
        });
        true
    }

    /// Add a player directly (for snapshot restoration)
    pub fn add_player(&mut self, player: Player) {
        self.try_add_player(player)
            .expect("Player admission requires an exact unambiguous script identity");
    }

    pub fn try_add_player(&mut self, player: Player) -> gamelogic::GameLogicResult<()> {
        self.with_own_services_mut(|this| this.try_add_player_with_services(player))
    }

    pub(super) fn try_add_player_with_services(
        &mut self,
        player: Player,
    ) -> gamelogic::GameLogicResult<()> {
        let invalid = |text: &str| gamelogic::GameLogicError::Configuration(text.into());
        let owner = player
            .script_identity()
            .ok_or_else(|| invalid("Player has no admitted script identity"))?;
        if self
            .players
            .get(&player.id)
            .is_some_and(|current| current.script_identity() != Some(owner))
        {
            return Err(invalid(
                "existing Player controller cannot be replaced by a different script identity",
            ));
        }
        if self
            .players
            .values()
            .any(|other| other.id != player.id && other.script_identity() == Some(owner))
        {
            return Err(invalid(
                "Player script identity already belongs to another controller",
            ));
        }
        let is_human = player.is_human && !player.is_observer;
        let name = format!("team{owner}");
        let factory = self
            .team_factory
            .lock()
            .map_err(|_| invalid("Player Team owner is poisoned"))?;
        if let Some(prototype) = factory.find_team_prototype(&name) {
            let controller_matches = factory
                .owned_team_prototypes_for_controller(player.id)
                .iter()
                .any(|owned| std::sync::Arc::ptr_eq(owned, &prototype));
            let instances_match = factory.find_team_instances(&name).iter().all(|team| {
                team.read()
                    .map(|team| team.get_controlling_player_id() == Some(player.id))
                    .unwrap_or(false)
            });
            if prototype.get_owner_name().as_str() != owner
                || !controller_matches
                || !instances_match
            {
                return Err(invalid(
                    "Default Team prototype belongs to a different Player identity",
                ));
            }
        }
        drop(factory);
        self.player_template_bindings.remove(&player.id);
        let id = player.id;
        self.players.insert(id, player);
        // Ordinary offline admission chooses the first human in authored
        // order. Explicit later overrides are applied once the complete
        // admission list is known (map/skirmish/replay paths).
        if self.local_player_id.is_none() && is_human {
            self.select_local_player(Some(id));
        }
        self.ensure_host_default_team(id);
        Ok(())
    }

    pub fn command_center_position(&self, team: Team) -> Option<Vec3> {
        let mut fallback = None;
        let mut highest_cost = i32::MIN;

        for obj in self.objects.registered_values() {
            if obj.team != team || !obj.is_alive() {
                continue;
            }

            if obj.is_kind_of(KindOf::CommandCenter) {
                return Some(obj.get_position());
            }

            if obj.is_kind_of(KindOf::Structure) {
                let cost = obj.thing().template.build_cost.supplies as i32;
                if cost > highest_cost {
                    highest_cost = cost;
                    fallback = Some(obj.get_position());
                }
            }
        }

        fallback
    }

    /// Get player by ID
    pub fn get_player(&self, player_id: u32) -> Option<&Player> {
        self.players.get(&player_id)
    }

    /// Lend the saved Drawable loader this session's definitions and object
    /// colors without publishing or copying another world's mutable state.
    pub(crate) fn drawable_restore_inputs(
        &mut self,
        is_night: bool,
    ) -> (
        Option<&mut game_engine::common::thing::receiving_module_content::ReceivingModuleContent>,
        impl FnMut(u32) -> Result<(u8, u8, u8), String> + '_,
    ) {
        let objects = &self.objects;
        let players = &self.players;
        let content = self.receiving_module_content.as_mut();
        let color_for_object = move |object_id| {
            let object = objects
                .get(&ObjectId(object_id))
                .ok_or_else(|| format!("missing receiving Object {object_id} for Drawable"))?;
            if let Some(raw) = object.custom_indicator_color {
                return Ok(((raw >> 16) as u8, (raw >> 8) as u8, raw as u8));
            }
            Ok(object
                .owner_player_id
                .and_then(|owner| players.get(&owner))
                .map(|player| {
                    if is_night {
                        player.color_night_rgb
                    } else {
                        player.color_rgb
                    }
                })
                .unwrap_or((0, 0, 0)))
        };
        (content, color_for_object)
    }

    /// Install application definitions before binding any player to a template.
    pub fn admit_player_template_definitions(
        &mut self,
        definitions: game_engine::common::rts::player_template::PlayerTemplateStore,
    ) -> Result<(), String> {
        if !self.player_template_bindings.is_empty() {
            return Err("cannot replace PlayerTemplate definitions while players are bound".into());
        }
        self.player_template_definitions = Some(definitions);
        Ok(())
    }

    pub(crate) fn player_template_identity_from_name(
        &self,
        name: &str,
    ) -> Option<PlayerTemplateIdentity> {
        match &self.player_template_definitions {
            Some(store) => PlayerTemplateIdentity::from_exact_name_in(name, store),
            None => PlayerTemplateIdentity::from_exact_name(name),
        }
    }

    pub(crate) fn resolve_player_template_identity(
        &self,
        identity: &PlayerTemplateIdentity,
    ) -> Option<game_engine::common::rts::player_template::PlayerTemplate> {
        match &self.player_template_definitions {
            Some(store) => identity.resolve_in(store).cloned(),
            None => identity.resolve(),
        }
    }

    /// Exact C++ PlayerTemplate selection retained for this host session.
    pub fn player_template_identity(&self, player_id: u32) -> Option<&PlayerTemplateIdentity> {
        self.player_template_bindings.get(&player_id)
    }

    /// Snapshot-only copy of exact offline identities.  The map remains
    /// session-private so ordinary gameplay cannot replace a selected General
    /// without going through the validated bind path.
    pub(crate) fn player_template_identities_for_snapshot(
        &self,
    ) -> Vec<(u32, PlayerTemplateIdentity)> {
        self.player_template_bindings
            .iter()
            .map(|(&player_id, identity)| (player_id, identity.clone()))
            .collect()
    }

    /// Install a snapshot-owned exact identity after `restore_all_players`.
    ///
    /// This deliberately does *not* call `bind_player_template_identity`:
    /// `PlayerSnapshot` has already restored the saved resources, sciences,
    /// and GameInfo overrides, and re-applying `Player::init(pt)` would
    /// overwrite them.  The indexed lookup makes a stale store ordering or
    /// name fail closed, and prevents a saved Random slot from being resolved
    /// a second time.
    pub(crate) fn install_restored_player_template_identity(
        &mut self,
        player_id: u32,
        player_template: PlayerTemplateIdentity,
    ) -> bool {
        let Some(template) = self.resolve_player_template_identity(&player_template) else {
            return false;
        };
        let Some(template_team) = PlayerTemplateIdentity::team_for_template(&template) else {
            return false;
        };
        let Some(player) = self.players.get(&player_id) else {
            return false;
        };
        if player.team != template_team {
            return false;
        }

        self.player_template_bindings
            .insert(player_id, player_template);
        true
    }

    /// Resolve the immutable Common PlayerTemplate for a concrete host player.
    /// Callers must use this before Team residual tables so a selected General
    /// never silently degrades to a base faction.
    pub(crate) fn resolved_player_template(
        &self,
        player_id: u32,
    ) -> Option<game_engine::common::rts::player_template::PlayerTemplate> {
        self.player_template_identity(player_id)
            .and_then(|identity| self.resolve_player_template_identity(identity))
    }

    /// Exact `PlayerTemplate::ProductionCostChange` factor for one player and
    /// ThingTemplate.  A host player without a bound Campaign/Challenge
    /// template retains the ordinary `1.0` factor; a bound General never falls
    /// back to a base-side table.
    pub(crate) fn player_template_production_cost_factor(
        &self,
        player_id: u32,
        build_template_name: &str,
    ) -> f32 {
        self.resolved_player_template(player_id)
            .map(|template| {
                PlayerTemplateIdentity::production_cost_factor_for_template(
                    &template,
                    build_template_name,
                )
            })
            .unwrap_or(1.0)
    }

    /// Exact `PlayerTemplate::ProductionTimeChange` factor for one player and
    /// ThingTemplate.  Low-power timing remains a separate production/construction
    /// concern, as it is in C++ `ThingTemplate::calcTimeToBuild`.
    pub(crate) fn player_template_production_time_factor(
        &self,
        player_id: u32,
        build_template_name: &str,
    ) -> f32 {
        self.resolved_player_template(player_id)
            .map(|template| {
                PlayerTemplateIdentity::production_time_factor_for_template(
                    &template,
                    build_template_name,
                )
            })
            .unwrap_or(1.0)
    }

    /// C++ Object construction sets the controller's exact
    /// `ProductionVeterancyLevel` for every spawned ThingTemplate.  `None`
    /// means this player is not backed by a selected PlayerTemplate; a bound
    /// template with no entry explicitly maps to Main's Rookie default.
    pub(crate) fn player_template_production_veterancy(
        &self,
        player_id: u32,
        build_template_name: &str,
    ) -> Option<VeterancyLevel> {
        self.resolved_player_template(player_id).map(|template| {
            PlayerTemplateIdentity::production_veterancy_for_template(
                &template,
                build_template_name,
            )
        })
    }

    /// Wave 238: economy probe without exposing `&Player` to engine dual-read paths.
    #[inline]
    pub fn player_economy(&self, id: u32) -> Option<(u32, i32, i32, i32)> {
        self.players.get(&id).map(|p| {
            (
                p.effective_supplies(),
                p.energy_available_at(self.frame),
                p.energy_production_at(self.frame),
                p.power_consumed,
            )
        })
    }

    /// Wave 238: unlocked sciences without exposing `&Player`.
    #[inline]
    pub fn player_unlocked_sciences(&self, id: u32) -> Vec<String> {
        self.players
            .get(&id)
            .map(|p| p.unlocked_sciences.iter().cloned().collect())
            .unwrap_or_default()
    }

    /// Wave 238: science purchase points without exposing `&Player`.
    #[inline]
    pub fn player_science_purchase_points(&self, id: u32) -> i32 {
        self.players
            .get(&id)
            .map(|p| p.science_purchase_points)
            .unwrap_or(0)
    }

    /// Wave 238: science purchase capability without exposing `&Player`.
    #[inline]
    pub fn player_can_purchase_science(&self, id: u32, science_name: &str) -> bool {
        let Some(player) = self.players.get(&id) else {
            return false;
        };
        let Some(content) = &self.receiving_module_content else {
            return player.is_capable_of_purchasing_science(science_name);
        };
        let facts = content.science_facts_ref();
        let canonical = if facts.science(science_name.trim()).is_some() {
            science_name.trim().to_owned()
        } else {
            crate::game_logic::host_sp_science_upgrade_player_team_residual_wave109::normalize_science_name_residual(science_name)
        };
        player
            .science_purchase_cost_with_facts(&canonical, facts)
            .is_some()
    }

    /// Wave 239: team probe without exposing `&Player`.
    #[inline]
    pub fn player_team(&self, id: u32) -> Option<Team> {
        self.players.get(&id).map(|p| p.team)
    }

    /// Wave 239: command-center world pose for a specific player.
    /// C++ `viewCommandCenter` iterates `localPlayer` objects only
    /// (`CommandXlat.cpp:780-793`), then most expensive structure via
    /// `calcCostToBuild(controllingPlayer)`.
    pub fn player_command_center_position(&self, id: u32) -> Option<glam::Vec3> {
        self.players.get(&id)?;
        let mut fallback = None;
        let mut center = None;
        let mut highest_cost = -1;

        for obj in self.objects.registered_values() {
            if !obj.is_alive() {
                continue;
            }
            if obj.owner_player_id != Some(id) {
                continue;
            }
            if obj.is_kind_of(KindOf::CommandCenter) {
                // The source callback continues iterating and retains the last
                // command center visited, despite its "first" comment.
                center = Some(obj.get_position());
                continue;
            }
            if obj.is_kind_of(KindOf::Structure) {
                let cost = self.modified_build_cost_supplies(
                    id,
                    &obj.template_name,
                    obj.thing().template.build_cost.supplies,
                ) as i32;
                if cost > highest_cost {
                    highest_cost = cost;
                    fallback = Some(obj.get_position());
                }
            }
        }
        center.or(fallback)
    }

    /// Wave 240: existence probe without exposing `&Player`.
    #[inline]
    pub fn player_exists(&self, id: u32) -> bool {
        self.players.contains_key(&id)
    }

    /// Wave 240: lowest player id (boot local residual).
    #[inline]
    pub fn min_player_id(&self) -> Option<u32> {
        self.players.keys().copied().min()
    }

    /// Wave 240: display name without exposing `&Player`.
    #[inline]
    pub fn player_name(&self, id: u32) -> Option<String> {
        self.players.get(&id).map(|p| p.name.clone())
    }

    /// Wave 240: alive flag without exposing `&Player`.
    #[inline]
    pub fn player_is_alive(&self, id: u32) -> bool {
        self.players.get(&id).map(|p| p.is_alive).unwrap_or(false)
    }

    /// Wave 240: local flag without exposing `&Player`.
    #[inline]
    pub fn player_is_local(&self, id: u32) -> bool {
        self.is_local_player(id)
    }

    /// Wave 240: UI color without exposing `&Player`.
    #[inline]
    pub fn player_color_rgb(&self, id: u32) -> Option<(u8, u8, u8)> {
        self.players
            .get(&id)
            .map(|p| p.house_color_rgb_at(self.lighting_rules().time_of_day))
    }

    /// C++ Object::getIndicatorColor → controlling Player::getPlayerColor.
    /// Black / missing owner is not a house color (Create_Render_Obj reallycolor).
    #[inline]
    pub fn player_house_color_rgba(&self, owner_player_id: Option<u32>) -> Option<[f32; 4]> {
        let (r, g, b) = self.player_color_rgb(owner_player_id?)?;
        if r == 0 && g == 0 && b == 0 {
            return None;
        }
        Some([r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0, 1.0])
    }

    /// Wave 240: selected object ids without exposing `&Player`.
    #[inline]
    pub fn player_selected_objects(&self, id: u32) -> Vec<ObjectId> {
        self.players
            .get(&id)
            .map(|p| p.selected_objects.clone())
            .unwrap_or_default()
    }

    /// Wave 240: ordered player id roster without exposing `&Player`.
    #[inline]
    pub fn player_ids(&self) -> Vec<u32> {
        let mut ids: Vec<u32> = self.players.keys().copied().collect();
        ids.sort_unstable();
        ids
    }

    /// Wave 240: raise supplies floor without exposing `&mut Player`.
    #[inline]
    pub fn ensure_player_min_supplies(&mut self, id: u32, min_supplies: u32) {
        if let Some(p) = self.players.get_mut(&id) {
            p.resources.supplies = p.resources.supplies.max(min_supplies);
        }
    }

    /// Wave 242: extend selection without exposing `&mut Player`.
    #[inline]
    pub fn player_extend_selection(&mut self, id: u32, units: &[ObjectId]) {
        let local = self.is_local_player(id);
        let Some(player) = self.players.get_mut(&id) else {
            return;
        };
        for unit in units {
            if !player.selected_objects.contains(unit) {
                player.selected_objects.push(*unit);
            }
        }
        if local {
            for &unit in units {
                self.admit_local_drawable_selected(unit);
            }
        }
    }

    /// Wave 242: selection count without exposing `&Player`.
    #[inline]
    pub fn player_selected_count(&self, id: u32) -> usize {
        self.players
            .get(&id)
            .map(|p| p.selected_objects.len())
            .unwrap_or(0)
    }

    /// Wave 243: spend build cost without exposing `&mut Player`.
    #[inline]
    pub fn try_spend_player_resources(&mut self, id: u32, cost: &Resources) -> bool {
        let Some(p) = self.players.get_mut(&id) else {
            return false;
        };
        p.spend_resources(cost, &mut self.economy_events)
    }

    /// Wave 243: refund supplies without exposing `&mut Player`.
    #[inline]
    pub fn player_refund_supplies(&mut self, id: u32, supplies: u32) {
        if supplies == 0 {
            return;
        }
        if let Some(player) = self.players.get_mut(&id) {
            player.refund_production_cost(
                &Resources { supplies, power: 0 },
                &mut self.economy_events,
            );
        }
    }

    /// Wave 243: constructor team probe without exposing `&Object`.
    #[inline]
    pub fn unit_team_if_can_construct(&self, id: ObjectId) -> Option<Team> {
        let obj = self.objects.get(&id)?;
        if obj.can_construct() {
            Some(obj.team)
        } else {
            None
        }
    }

    /// Exact constructor provenance for player-issued construction. Returning
    /// both values preserves the faction data the template path needs while
    /// keeping billing and ownership with the actual player slot.
    #[inline]
    pub fn unit_owner_if_can_construct(&self, id: ObjectId) -> Option<(u32, Team)> {
        let obj = self.objects.get(&id)?;
        let player_id = obj.owner_player_id?;
        if !obj.can_construct() {
            return None;
        }
        self.players
            .get(&player_id)
            .filter(|player| player.is_alive && player.team == obj.team)
            .map(|player| (player.id, player.team))
    }

    /// Get mutable player by ID
    pub fn get_player_mut(&mut self, player_id: u32) -> Option<&mut Player> {
        self.players.get_mut(&player_id)
    }

    pub(crate) fn player_and_economy_events_mut(
        &mut self,
        player_id: u32,
    ) -> Option<(
        &mut Player,
        &mut crate::game_logic::host_economy_log::EconomyEvents,
    )> {
        self.players
            .get_mut(&player_id)
            .map(|player| (player, &mut self.economy_events))
    }

    pub(crate) fn player_for_team_and_economy_events_mut(
        &mut self,
        team: Team,
    ) -> Option<(
        &mut Player,
        &mut crate::game_logic::host_economy_log::EconomyEvents,
    )> {
        let key = self
            .players
            .iter()
            .find_map(|(id, player)| (player.team == team).then_some(*id))?;
        self.player_and_economy_events_mut(key)
    }

    pub fn get_player_mut_by_team(&mut self, team: Team) -> Option<&mut Player> {
        let key = self
            .players
            .iter()
            .find_map(|(id, p)| if p.team == team { Some(*id) } else { None })?;
        self.players.get_mut(&key)
    }

    pub fn get_player_by_team(&self, team: Team) -> Option<&Player> {
        self.players.values().find(|p| p.team == team)
    }
}
