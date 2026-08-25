use super::*;

fn template_uses_overlord_draw(template: &str) -> bool {
    let t = template.to_ascii_lowercase();
    if t.contains("overlord") || t.contains("helix") || t.contains("spectregunship") {
        return true;
    }
    #[cfg(feature = "game_client")]
    {
        if let Some(manager) = crate::assets::get_asset_manager() {
            if let Ok(manager) = manager.lock() {
                if let Some(definition) = manager.get_object_definition(template) {
                    return definition.draw_modules.iter().any(|module| {
                        module
                            .declaration
                            .split_whitespace()
                            .next()
                            .is_some_and(|name| {
                                name.eq_ignore_ascii_case("W3DOverlordTankDraw")
                                    || name.eq_ignore_ascii_case("W3DOverlordTruckDraw")
                                    || name.eq_ignore_ascii_case("W3DOverlordAircraftDraw")
                            })
                    });
                }
            }
        }
    }
    false
}

impl PresentationFrame {
    /// C++ `ActionManager::canEnterObject(..., CHECK_CAPACITY)` expressed only
    /// through the immutable frame used by physical RMB input.
    ///
    /// This is intentionally a capability/capacity gate rather than a target
    /// name heuristic.  The authoritative command executor repeats legality
    /// when the queued order is consumed, but the cursor/translator must make
    /// the same decision before it posts `MSG_ENTER`.
    pub fn normal_enter_target_available_for_local(&self, target: &RenderableObject) -> bool {
        self.normal_enter_available_capacity_for_local(target)
            .is_some_and(|available| available > 0)
    }

    /// Remaining capacity in the frozen target for the local player's selected
    /// rider.  A normal transport reports remaining authored passenger slots;
    /// garrison/tunnel roles report remaining bodies.  Physical RMB compares a
    /// selected rider's frozen slot count against this before it emits Enter.
    pub fn normal_enter_available_capacity_for_local(
        &self,
        target: &RenderableObject,
    ) -> Option<usize> {
        if target.destroyed || target.sold || target.under_construction || target.disabled_subdued {
            return None;
        }
        if !target.supports_normal_enter() {
            return None;
        }
        let Some(capacity) = target.normal_enter_capacity() else {
            return None;
        };

        let relationship = self.normal_enter_relationship_for_local(target);
        if !target.normal_enter_allows_relationship(relationship) {
            return None;
        }
        let same_controller = self.normal_enter_controller_matches_local(target);
        if target.normal_enter_requires_exact_controller() && !same_controller {
            return None;
        }

        if target.contain_module_kind == crate::game_logic::ContainModuleKind::RiderChange {
            // The selected source is checked against the frozen roster by the
            // input translator.  Do not use the occupied one-seat count here:
            // a legal RiderChange command replaces that payload atomically.
            return (!target.rider_change_allowed_templates.is_empty()).then_some(usize::MAX);
        }

        if target.is_tunnel_network {
            // TunnelContain keeps its own friendly/team body pool.  Unlike
            // TransportContain it does not share an owner's vehicle seats.
            if target.team != self.local_team && target.team != Team::Neutral {
                return None;
            }
            let occupied = self
                .objects
                .iter()
                .filter(|candidate| {
                    candidate.is_tunnel_network
                        && self.normal_enter_same_tunnel_controller(candidate, target)
                        && !candidate.destroyed
                        && !candidate.sold
                })
                .map(RenderableObject::normal_enter_occupant_count)
                .sum::<usize>();
            return capacity.checked_sub(occupied);
        }

        // C++ ActionManager.cpp:656-675: non-owner may Enter a non-faction
        // container that is empty *or* only STEALTH_GARRISON occupied.
        // Frozen stealth count is independent of hide-from-nonallies.
        if !same_controller {
            if target.is_faction_structure {
                return None;
            }
            let stealth = target.stealth_garrison_occupant_count as usize;
            let contain_count = target.normal_enter_occupant_count().max(stealth);
            let non_stealth = contain_count.saturating_sub(stealth);
            if non_stealth > 0 {
                return None;
            }
            if stealth > 0 && non_stealth == 0 {
                return Some(usize::MAX);
            }
        }

        if !target.normal_enter_uses_transport_slots() {
            return capacity.checked_sub(target.normal_enter_occupant_count());
        }

        // Mobile TransportContain tracks m_extraSlotsInUse.  The roster must
        // be complete enough to price every current passenger; old sparse
        // presentation records fail closed rather than charging one body per
        // vehicle.
        if target.normal_enter_occupant_count() != target.garrisoned_units.len() {
            return None;
        }
        let mut slots_in_use = 0usize;
        for occupant_id in &target.garrisoned_units {
            let occupant = self
                .objects
                .iter()
                .find(|candidate| candidate.id == *occupant_id)?;
            let slots = occupant.transport_slot_count;
            if slots == 0 {
                return None;
            }
            slots_in_use = slots_in_use.checked_add(slots)?;
        }
        capacity.checked_sub(slots_in_use)
    }

    /// Same exact controlling player check as the authority-side
    /// `GameLogic::normal_enter_controller_matches`, using only frozen player
    /// provenance.  Team fallback is allowed only when the whole frame is a
    /// genuinely ownerless, unambiguous legacy snapshot.
    fn normal_enter_controller_matches_local(&self, target: &RenderableObject) -> bool {
        match target.owner_player_id {
            Some(owner_player_id) => {
                self.normal_enter_valid_explicit_owner(target) == Some(owner_player_id)
                    && owner_player_id == self.local_player_id
            }
            None => {
                self.uses_legacy_team_ownership_fallback()
                    && target.team == self.local_team
                    && self.normal_enter_unique_live_player_for_team(target.team)
                        == Some(self.local_player_id)
            }
        }
    }

    /// Frozen C++ `Player::getRelationship` for OpenContain admission.
    fn normal_enter_relationship_for_local(
        &self,
        target: &RenderableObject,
    ) -> gamelogic::common::Relationship {
        use gamelogic::common::Relationship;

        match target.owner_player_id {
            Some(target_owner)
                if self.normal_enter_valid_explicit_owner(target) == Some(target_owner) =>
            {
                if target_owner == self.local_player_id {
                    return Relationship::Allies;
                }
                let Some(local) = self
                    .player_info(self.local_player_id)
                    .filter(|player| player.is_alive)
                else {
                    return Relationship::Neutral;
                };
                let Some(owner) = self
                    .player_info(target_owner)
                    .filter(|player| player.is_alive)
                else {
                    return Relationship::Neutral;
                };
                if local.alliance_team >= 0 && local.alliance_team == owner.alliance_team {
                    Relationship::Allies
                } else {
                    Relationship::Enemies
                }
            }
            None if self.uses_legacy_team_ownership_fallback()
                && self.normal_enter_unique_live_player_for_team(self.local_team)
                    == Some(self.local_player_id)
                && self
                    .normal_enter_unique_live_player_for_team(target.team)
                    .is_some() =>
            {
                if target.team == self.local_team {
                    Relationship::Allies
                } else if target.team == Team::Neutral || self.local_team == Team::Neutral {
                    Relationship::Neutral
                } else {
                    Relationship::Enemies
                }
            }
            _ => Relationship::Neutral,
        }
    }

    /// Explicit ownership must identify a live player with the matching
    /// faction.  A stale owner is intentionally not reinterpreted through
    /// team/faction fallback.
    fn normal_enter_valid_explicit_owner(&self, object: &RenderableObject) -> Option<u32> {
        object.owner_player_id.and_then(|player_id| {
            self.player_info(player_id)
                .filter(|player| player.is_alive && player.team == object.team)
                .map(|player| player.id)
        })
    }

    /// Tunnel pool grouping mirrors the authority side: exact owner identity
    /// when present, otherwise only an all-ownerless legacy frame may use the
    /// faction pool.
    fn normal_enter_same_tunnel_controller(
        &self,
        candidate: &RenderableObject,
        target: &RenderableObject,
    ) -> bool {
        match (target.owner_player_id, candidate.owner_player_id) {
            (Some(target_owner), Some(candidate_owner)) => {
                target_owner == candidate_owner
                    && self.normal_enter_valid_explicit_owner(target) == Some(target_owner)
                    && self.normal_enter_valid_explicit_owner(candidate) == Some(candidate_owner)
            }
            (None, None) => {
                self.uses_legacy_team_ownership_fallback() && candidate.team == target.team
            }
            _ => false,
        }
    }

    fn normal_enter_unique_live_player_for_team(&self, team: Team) -> Option<u32> {
        if team == Team::Neutral {
            return None;
        }
        let mut players = self
            .players
            .iter()
            .filter(|player| player.is_alive && player.team == team)
            .map(|player| player.id);
        let first = players.next()?;
        players.next().is_none().then_some(first)
    }

    pub fn alive_object_count(&self) -> usize {
        // Wave 1104: alive count residual excludes sold.
        self.objects
            .iter()
            .filter(|o| !o.destroyed && !o.sold)
            .count()
    }

    /// Stable object-id list for the production render collect path.
    /// Presentation owns unit identity + unit FOW; mesh asset load may still
    /// consult asset systems (not live object transform / shroud re-read).
    pub fn renderable_object_ids(&self) -> Vec<ObjectId> {
        self.objects
            .iter()
            .filter(|o| !o.destroyed)
            .map(|o| o.id)
            .collect()
    }

    /// Main unit mesh pass inputs from the snapshot only (no GameLogic / shroud borrow).
    ///
    /// The ordinary GameWorld roster filters gameplay-destroyed and
    /// engine-bridged objects (RenderBridge owns the latter).  A resident
    /// direct-host record may fill an omitted ordinary row so its C++ Drawable
    /// can remain visible during deferred death/rubble lifetime, but it never
    /// bypasses bridge, containment, or stealth ownership gates.
    /// Includes local-player FOW alpha for skip/darkening without mid-render queries.
    pub fn unit_render_inputs(&self) -> Vec<UnitRenderInput> {
        // Wave 502: stealth mesh residual from frozen presentation only.
        // Wave 504: skip contained_by units; stamp garrisoned bits on structures.
        // C++ StealthUpdate computes a viewer-relative look. Invisible
        // stealthed units are omitted; a visible-friendly/inactive-viewer look
        // remains in the mesh pass with a separate frozen presentation
        // opacity. FOW alpha must remain untouched for this path.
        let mesh_allowed = |object: &RenderableObject, allow_destroyed: bool| {
            (allow_destroyed || !object.destroyed)
                && !object.engine_bridged
                // Wave 504: contained units are not drawn as free world meshes
                // unless they are the C++ OverlordContain friend_getRider()
                // (W3DOverlordTankDraw.cpp:45-78 draws the rider after the hull)
                // or a non-enclosing Fire Base station occupant.
                && (object.contained_by.is_none()
                    || self.overlord_rider_should_draw(object)
                    || self.firebase_occupant_should_draw(object))
                && !self.local_viewer_hides_stealthed(object)
        };
        let make_input =
            |object: &RenderableObject| {
                let mut input = UnitRenderInput::from_renderable_with_environment(
                    object,
                    self.world_env.is_snow,
                    self.world_env.is_night,
                    self.frame.0,
                );
                if self.local_viewer_uses_friendly_stealth_look(object) {
                    // Leftover setEffectiveOpacity result lives on the host
                    // (SetVisionCamo / camo_friendly_opacity), including mines.
                    input.presentation_opacity = object.camo_friendly_opacity.clamp(0.0, 1.0);
                }
                let is_mine = object.has_mine
                    || object
                        .kind_of
                        .iter()
                        .any(|k| matches!(k, KindOf::Mine | KindOf::DemoTrap));
                let hint = !object.stealthed
                    && object.innate_stealth
                    && object.owner_player_id == Some(self.local_player_id)
                    && (object.is_firing_weapon || object.using_ability);
                // C++ Drawable::draw reads m_secondMaterialPassOpacity residual.
                // setStealthLook / detector pulse arm 1.0; draw fades by 0.8/frame.
                // Do not recompute a hard 1.0 from stealthed&&detected — that locks
                // the overlay for the whole DETECTED window.
                let gated = crate::game_logic::stealth_second_material_pass_opacity(
                    object.stealthed,
                    object.detected,
                    object.can_disguise_as_team,
                    is_mine,
                    object.drawable_shroud.effectively_dead,
                    hint,
                );
                input.second_material_pass_opacity = if gated > 0.0 {
                    object.camo_heat_vision_opacity
                } else {
                    0.0
                };
                let fade = crate::game_logic::drawable_explicit_fade_opacity(
                    object.drawable_fade_mode,
                    object.drawable_fade_start_frame,
                    object.drawable_fade_frames,
                    self.frame.0,
                );
                let explicit = if object.drawable_explicit_opacity.is_finite()
                    && object.drawable_explicit_opacity >= 0.0
                {
                    object.drawable_explicit_opacity.clamp(0.0, 1.0)
                } else {
                    1.0
                };
                if fade < 0.999 || explicit < 0.999 {
                    input.presentation_opacity =
                        (input.presentation_opacity * fade * explicit).clamp(0.0, 1.0);
                }
                // Compatibility fallback for a GameWorld-only record with no
                // resident direct Drawable identity. The normal direct-host
                // path applies `visual_template_name` below regardless of
                // viewer relation, matching C++ changeVisualDisguise.
                if object.disguised && !self.is_allied_with_local(object) {
                    if let Some(ref disguise_template) = object.disguise_as_template {
                        if !disguise_template.is_empty() {
                            let fallback_model_key =
                                crate::assets::mesh_asset_resolve::model_key_from_presentation(
                                    Some(disguise_template.as_str()),
                                    disguise_template,
                                );
                            let fallback_draw_models = (!fallback_model_key.trim().is_empty())
                                .then(|| crate::assets::AuthoredDrawModel {
                                    module_index: 0,
                                    model_key: fallback_model_key,
                                    ..Default::default()
                                });
                            input.draw_models =
                                crate::assets::resolve_presentation_draw_models_for_live_object(
                                    input.id.0,
                                    disguise_template,
                                    fallback_draw_models.as_slice(),
                                    input.model_condition_bits_with_combat_flags(),
                                );
                            input.model_key = input
                                .draw_models
                                .first()
                                .map(|model| model.model_key.clone())
                                .unwrap_or_default();
                        }
                    }
                }
                input
            };
        let apply_visual_template = |input: &mut UnitRenderInput,
                                     visual_template_name: &str,
                                     visual_mesh_scale: f32| {
            if visual_template_name.trim().is_empty() {
                return;
            }
            let fallback_model_key = crate::assets::mesh_asset_resolve::model_key_from_presentation(
                Some(visual_template_name),
                visual_template_name,
            );
            let fallback_draw_models =
                (!fallback_model_key.trim().is_empty()).then(|| crate::assets::AuthoredDrawModel {
                    module_index: 0,
                    model_key: fallback_model_key,
                    ..Default::default()
                });
            input.template_name = visual_template_name.to_owned();
            input.draw_models = crate::assets::resolve_presentation_draw_models_for_live_object(
                input.id.0,
                visual_template_name,
                fallback_draw_models.as_slice(),
                input.model_condition_bits_with_combat_flags(),
            );
            input.model_key = input
                .draw_models
                .first()
                .map(|model| model.model_key.clone())
                .unwrap_or_default();
            if visual_mesh_scale.is_finite() && visual_mesh_scale > 0.0 {
                input.mesh_scale = visual_mesh_scale;
            }
        };

        // C++ StealthUpdate::changeVisualDisguise destroys/recreates the
        // Drawable with `m_disguiseAsTemplate` before it decides only the
        // viewer-relative indicator color. A GameWorld-primary row may still
        // carry the original gameplay template, so make the frozen direct
        // visual identity authoritative for every matching resident row—not
        // merely for an absent/deferred-death fallback row.
        let direct_visual_templates: std::collections::HashMap<ObjectId, (&str, f32)> = self
            .direct_host_drawables
            .iter()
            .filter(|direct| direct.resident && !direct.visual_template_name.trim().is_empty())
            .map(|direct| {
                (
                    direct.object.id,
                    (
                        direct.visual_template_name.as_str(),
                        direct.visual_mesh_scale,
                    ),
                )
            })
            .collect();

        let mut inputs: Vec<UnitRenderInput> = self
            .objects
            .iter()
            .filter(|object| mesh_allowed(object, false))
            .map(|object| {
                let mut input = make_input(object);
                if let Some((visual_template_name, visual_mesh_scale)) =
                    direct_visual_templates.get(&object.id)
                {
                    apply_visual_template(&mut input, visual_template_name, *visual_mesh_scale);
                }
                input
            })
            .collect();
        let mut ordinary_input_ids: std::collections::HashSet<ObjectId> =
            inputs.iter().map(|input| input.id).collect();

        for direct in &self.direct_host_drawables {
            // A live ordinary row remains the sole mesh source.  This avoids
            // duplicate meshes when the GameWorld roster retained the entity.
            if !direct.resident || ordinary_input_ids.contains(&direct.object.id) {
                continue;
            }
            // If the current ordinary roster delegated this ID to RenderBridge,
            // the older host-only copy must not reclaim it as a main mesh.
            if self
                .objects
                .iter()
                .any(|object| object.id == direct.object.id && object.engine_bridged)
            {
                continue;
            }
            if !mesh_allowed(&direct.object, true) {
                continue;
            }

            let mut input = make_input(&direct.object);
            // Direct visual identity is frozen from immutable ThingTemplate /
            // committed disguise state, never mutable Object template bookkeeping.
            apply_visual_template(
                &mut input,
                &direct.visual_template_name,
                direct.visual_mesh_scale,
            );
            ordinary_input_ids.insert(input.id);
            inputs.push(input);
        }
        self.append_rally_point_marker_inputs(&mut inputs);
        self.append_generic_lockon_inputs(&mut inputs);
        inputs
    }

    /// C++ ControlBar::showRallyPoint — 3D RallyPointMarker, not a yellow line.
    fn append_rally_point_marker_inputs(&self, inputs: &mut Vec<UnitRenderInput>) {
        let mut leftover_any = false;
        #[cfg(feature = "game_client")]
        {
            for (draw_id, state) in
                gamelogic::helpers::TheGameClient.leftover_drawables_named("RallyPointMarker")
            {
                leftover_any = true;
                let Some(base) = self
                    .objects
                    .iter()
                    .find(|o| o.selected && o.rally_point.is_some())
                    .or_else(|| self.objects.iter().find(|o| o.rally_point.is_some()))
                else {
                    continue;
                };
                let mut marker = base.clone();
                marker.id = crate::game_logic::ObjectId(0xC000_0000 | draw_id);
                marker.template_name = "RallyPointMarker".to_string();
                marker.position =
                    glam::Vec3::new(state.position.x, state.position.y, state.position.z);
                marker.orientation = state.orientation;
                marker.team_color = [
                    state.indicator_color.r as f32 / 255.0,
                    state.indicator_color.g as f32 / 255.0,
                    state.indicator_color.b as f32 / 255.0,
                    1.0,
                ];
                marker.selected = false;
                marker.rally_point = None;
                marker.destroyed = false;
                marker.contained_by = None;
                let mut input = UnitRenderInput::from_renderable_with_environment(
                    &marker,
                    self.world_env.is_snow,
                    self.world_env.is_night,
                    self.frame.0,
                );
                input.draw_models = crate::assets::resolve_presentation_draw_models_for_conditions(
                    "RallyPointMarker",
                    &[],
                    input.model_condition_bits_with_combat_flags(),
                );
                input.model_key = input
                    .draw_models
                    .first()
                    .map(|model| model.model_key.clone())
                    .unwrap_or_default();
                inputs.push(input);
            }
        }
        if leftover_any {
            return;
        }
        for object in &self.objects {
            if object.destroyed || !object.selected {
                continue;
            }
            let Some(rp) = object.rally_point else {
                continue;
            };
            if !object.is_structure
                && !Self::object_has_kind(object, crate::game_logic::KindOf::AutoRallypoint)
            {
                continue;
            }
            let mut marker = object.clone();
            marker.id = crate::game_logic::ObjectId(0xC000_0000 | object.id.0);
            marker.template_name = "RallyPointMarker".to_string();
            marker.position = rp;
            marker.selected = false;
            marker.rally_point = None;
            marker.contained_by = None;
            let mut input = UnitRenderInput::from_renderable_with_environment(
                &marker,
                self.world_env.is_snow,
                self.world_env.is_night,
                self.frame.0,
            );
            input.draw_models = crate::assets::resolve_presentation_draw_models_for_conditions(
                "RallyPointMarker",
                &[],
                input.model_condition_bits_with_combat_flags(),
            );
            input.model_key = input
                .draw_models
                .first()
                .map(|model| model.model_key.clone())
                .unwrap_or_default();
            inputs.push(input);
        }
    }

    /// C++ `JetAIUpdate::buildLockonDrawableIfNecessary` — GenericLockon reticle.
    fn append_generic_lockon_inputs(&self, inputs: &mut Vec<UnitRenderInput>) {
        #[cfg(feature = "game_client")]
        {
            use crate::game_logic::object::STEALTH_FIGHTER_LOCKON_CURSOR;
            for (draw_id, state) in gamelogic::helpers::TheGameClient
                .leftover_drawables_named(STEALTH_FIGHTER_LOCKON_CURSOR)
            {
                let hidden = state
                    .drawable
                    .as_ref()
                    .and_then(|drawable| drawable.read().ok())
                    .is_some_and(|guard| guard.is_drawable_effectively_hidden());
                if hidden {
                    continue;
                }
                let controller_id = state.shroud_status_object_id;
                let Some(base) = self
                    .objects
                    .iter()
                    .find(|o| controller_id != 0 && o.id.0 == controller_id)
                    .or_else(|| {
                        self.objects.iter().find(|o| {
                            !o.destroyed
                                && o.template_name
                                    .to_ascii_lowercase()
                                    .contains("stealthfighter")
                        })
                    })
                    .or_else(|| self.objects.iter().find(|o| !o.destroyed))
                else {
                    continue;
                };
                let mut marker = base.clone();
                marker.id = crate::game_logic::ObjectId(0xC100_0000 | draw_id);
                marker.template_name = STEALTH_FIGHTER_LOCKON_CURSOR.to_string();
                marker.position =
                    glam::Vec3::new(state.position.x, state.position.y, state.position.z);
                marker.orientation = state.orientation;
                marker.selected = false;
                marker.rally_point = None;
                marker.destroyed = false;
                marker.contained_by = None;
                marker.shadows_enabled = false;
                let mut input = UnitRenderInput::from_renderable_with_environment(
                    &marker,
                    self.world_env.is_snow,
                    self.world_env.is_night,
                    self.frame.0,
                );
                input.draw_models = crate::assets::resolve_presentation_draw_models_for_conditions(
                    STEALTH_FIGHTER_LOCKON_CURSOR,
                    &[],
                    input.model_condition_bits_with_combat_flags(),
                );
                input.model_key = input
                    .draw_models
                    .first()
                    .map(|model| model.model_key.clone())
                    .unwrap_or_default();
                inputs.push(input);
            }
        }
        #[cfg(not(feature = "game_client"))]
        {
            let _ = inputs;
        }
    }

    /// C++ `OverlordContain::friend_getRider` is the first contained portable
    /// structure. `W3DOverlordTankDraw::doDrawModule` draws that rider after
    /// the hull even though contain hides it from the normal Drawable walk.
    fn overlord_rider_should_draw(&self, object: &RenderableObject) -> bool {
        let Some(container_id) = object.contained_by else {
            return false;
        };
        let container = self
            .objects
            .iter()
            .find(|candidate| candidate.id == container_id)
            .or_else(|| {
                self.direct_host_drawables
                    .iter()
                    .find(|direct| direct.object.id == container_id)
                    .map(|direct| &direct.object)
            });
        let Some(container) = container else {
            return false;
        };
        let visual = self
            .direct_host_drawables
            .iter()
            .find(|direct| direct.resident && direct.object.id == container_id)
            .map(|direct| direct.visual_template_name.as_str())
            .filter(|name| !name.trim().is_empty())
            .unwrap_or(container.template_name.as_str());
        if !template_uses_overlord_draw(visual) {
            return false;
        }
        container.garrisoned_units.first().copied() == Some(object.id)
    }

    /// C++ GarrisonContain IsEnclosingContainer=No: Fire Base infantry stay visible.
    fn firebase_occupant_should_draw(&self, object: &RenderableObject) -> bool {
        let Some(container_id) = object.contained_by else {
            return false;
        };
        let container = self
            .objects
            .iter()
            .find(|candidate| candidate.id == container_id)
            .or_else(|| {
                self.direct_host_drawables
                    .iter()
                    .find(|direct| direct.object.id == container_id)
                    .map(|direct| &direct.object)
            });
        container.is_some_and(|c| {
            crate::game_logic::host_fire_base::is_fire_base_template(&c.template_name)
        })
    }

    /// Projectile mesh pass inputs from frozen in-flight projectiles (model_key residual).
    pub fn projectile_render_inputs(&self) -> Vec<ProjectileRenderInput> {
        let mut out = Vec::new();
        for p in &self.projectiles {
            if let Some(input) = ProjectileRenderInput::from_presentation(p) {
                out.push(input);
            }
        }
        out.sort_by_key(|p| p.id.0);
        out
    }

    /// Structures with a non-empty production queue (ControlBar residual feed).
    pub fn structures_with_production(&self) -> Vec<&RenderableObject> {
        // Wave 1101: fail-closed on sold/disabled production-queue residual feed.
        self.objects
            .iter()
            .filter(|o| {
                o.is_structure
                    && !o.destroyed
                    && !o.sold
                    && !o.disabled
                    && !o.production_queue.is_empty()
            })
            .collect()
    }

    /// Structures currently holding garrisoned units (contain residual feed).
    pub fn garrisoned_structures(&self) -> Vec<&RenderableObject> {
        // Wave 1101: fail-closed on sold garrison residual feed.
        self.objects
            .iter()
            .filter(|o| o.is_structure && !o.destroyed && !o.sold && !o.garrisoned_units.is_empty())
            .collect()
    }

    /// Net power from non-destroyed objects (presentation economy residual).
    /// Count presentation objects with host turret idle-scan residual.
    pub fn turret_idle_scan_count(&self) -> usize {
        // Wave 1107: residual counts exclude sold.
        self.objects
            .iter()
            .filter(|o| o.turret_idle_scanning && !o.destroyed && !o.sold)
            .count()
    }

    /// Count presentation objects with host horde weapon-bonus residual.
    /// Count presentation objects with host detector residual.
    /// CommandSet name residual for the primary selected object.
    /// Prefers `command_set_override`; empty when unset (template default left to boot path).
    pub fn selected_command_set_name(&self) -> Option<&str> {
        // Wave 1105: primary selection residual fail-closed on sold/unselectable/
        // masked/disabled (not only destroyed) so ControlBar does not show a
        // command set for unusable selected objects.
        let usable = |o: &&RenderableObject| {
            o.selected && !o.destroyed && !o.sold && !o.unselectable && !o.masked && !o.disabled
        };
        let primary = self
            .selected
            .first()
            .copied()
            .or_else(|| self.objects.iter().find(usable).map(|o| o.id))?;
        let o = self.objects.iter().find(|o| o.id == primary)?;
        if o.destroyed || o.sold || o.unselectable || o.masked || o.disabled {
            return None;
        }
        if !o.command_set_name.is_empty() {
            return Some(o.command_set_name.as_str());
        }
        if o.command_set_override.is_empty() {
            None
        } else {
            Some(o.command_set_override.as_str())
        }
    }

    /// Command-set names for current multi-selection (override or ThingFactory template).
    /// Empty entries omitted; used to populate ControlBar without OBJECT_REGISTRY.
    pub fn selected_command_set_names(&self) -> Vec<String> {
        // Wave 1105: multi-select command-set residual fail-closed on sold/
        // unselectable/masked/disabled (not only destroyed).
        let usable = |o: &&RenderableObject| {
            !o.destroyed && !o.sold && !o.unselectable && !o.masked && !o.disabled
        };
        let ids: Vec<ObjectId> = if !self.selected.is_empty() {
            self.selected.clone()
        } else {
            self.objects
                .iter()
                .filter(|o| o.selected && usable(o))
                .map(|o| o.id)
                .collect()
        };
        let mut names = Vec::new();
        for id in ids {
            let Some(ro) = self.objects.iter().find(|o| o.id == id && usable(&o)) else {
                continue;
            };
            // Prefer freeze from build_from_logic; resolve only if older frames lack it.
            if !ro.command_set_name.is_empty() {
                names.push(ro.command_set_name.clone());
                continue;
            }
            let override_name = ro.command_set_override.as_str();
            if let Some(cs) = crate::ui::construction_panel::resolve_command_set_name(
                &ro.template_name,
                if override_name.is_empty() {
                    None
                } else {
                    Some(override_name)
                },
            ) {
                names.push(cs);
            }
        }
        names
    }

    pub fn detector_object_count(&self) -> usize {
        // Wave 1107: residual counts exclude sold.
        self.objects
            .iter()
            .filter(|o| o.is_detector && !o.destroyed && !o.sold)
            .count()
    }

    /// Count presentation objects with non-empty command_set_override residual.
    pub fn command_set_override_object_count(&self) -> usize {
        // Wave 1107: residual counts exclude sold.
        self.objects
            .iter()
            .filter(|o| !o.command_set_override.is_empty() && !o.destroyed && !o.sold)
            .count()
    }

    /// Count presentation objects with any Strategy Center battle-plan bonus residual.
    /// Count presentation objects with host hive-slave residual.
    /// Count presentation objects with host humvee transport residual.
    /// Count presentation objects with host innate_stealth residual.
    pub fn innate_stealth_object_count(&self) -> usize {
        // Wave 1107: residual counts exclude sold.
        self.objects
            .iter()
            .filter(|o| o.innate_stealth && !o.destroyed && !o.sold)
            .count()
    }

    /// Count presentation objects with non-zero detection_rate_frames residual.
    pub fn timed_detector_object_count(&self) -> usize {
        // Wave 1107: residual counts exclude sold.
        self.objects
            .iter()
            .filter(|o| o.detection_rate_frames > 0 && !o.destroyed && !o.sold)
            .count()
    }

    pub fn humvee_transport_object_count(&self) -> usize {
        // Wave 1107: residual counts exclude sold.
        self.objects
            .iter()
            .filter(|o| o.is_humvee_transport && !o.destroyed && !o.sold)
            .count()
    }

    /// Count presentation objects with host overlord gattling addon residual.
    pub fn overlord_gattling_object_count(&self) -> usize {
        // Wave 1107: residual counts exclude sold.
        self.objects
            .iter()
            .filter(|o| o.has_overlord_gattling_addon && !o.destroyed && !o.sold)
            .count()
    }

    pub fn hive_object_count(&self) -> usize {
        // Wave 1107: residual counts exclude sold.
        self.objects
            .iter()
            .filter(|o| o.hive_slave_count > 0 && !o.destroyed && !o.sold)
            .count()
    }

    /// Count presentation objects with continuous-fire residual > 0.
    pub fn continuous_fire_object_count(&self) -> usize {
        // Wave 1107: residual counts exclude sold.
        self.objects
            .iter()
            .filter(|o| o.continuous_fire_level > 0 && !o.destroyed && !o.sold)
            .count()
    }

    pub fn battle_plan_bonus_object_count(&self) -> usize {
        // Wave 1107: residual counts exclude sold.
        self.objects
            .iter()
            .filter(|o| {
                !o.destroyed
                    && !o.sold
                    && (o.weapon_bonus_battle_plan_bombardment
                        || o.weapon_bonus_battle_plan_hold_the_line
                        || o.weapon_bonus_battle_plan_search_and_destroy)
            })
            .count()
    }

    pub fn horde_bonus_object_count(&self) -> usize {
        // Wave 1107: residual counts exclude sold.
        self.objects
            .iter()
            .filter(|o| o.weapon_bonus_horde && !o.destroyed && !o.sold)
            .count()
    }

    pub fn net_power_from_objects(&self) -> i32 {
        // Wave 1108: power residual excludes sold structures (sell removes
        // power contribution from the residual power bar feed).
        self.objects
            .iter()
            .filter(|o| !o.destroyed && !o.sold)
            .map(|o| o.power_provided - o.power_consumed)
            .sum()
    }

    /// Objects still under construction (dozer / structure residual).
    pub fn under_construction_objects(&self) -> Vec<&RenderableObject> {
        // Wave 1108: UC residual excludes sold.
        self.objects
            .iter()
            .filter(|o| !o.destroyed && !o.sold && o.under_construction)
            .collect()
    }

    /// Units at Veteran or higher (chevron residual feed).
    pub fn veteran_or_higher_units(&self) -> Vec<&RenderableObject> {
        // Wave 1108: veterancy residual excludes sold.
        self.objects
            .iter()
            .filter(|o| {
                !o.destroyed
                    && !o.sold
                    && o.is_unit
                    && !matches!(o.veterancy, PresentationVeterancy::Rookie)
            })
            .collect()
    }

    /// Units currently attacking (status residual).
    pub fn attacking_units(&self) -> Vec<&RenderableObject> {
        // Wave 1106: attacking residual excludes sold.
        self.objects
            .iter()
            .filter(|o| !o.destroyed && !o.sold && o.attacking)
            .collect()
    }

    /// Effectively stealthed units (hidden from non-allied targeting residual).
    pub fn effectively_stealthed_units(&self) -> Vec<&RenderableObject> {
        // Wave 1106: stealth residual excludes sold.
        self.objects
            .iter()
            .filter(|o| !o.destroyed && !o.sold && o.effectively_stealthed)
            .collect()
    }

    /// Contained (garrisoned/transported) units residual.
    pub fn contained_units(&self) -> Vec<&RenderableObject> {
        // Wave 1106: contained residual excludes sold containers/occupants.
        self.objects
            .iter()
            .filter(|o| !o.destroyed && !o.sold && o.contained_by.is_some())
            .collect()
    }

    /// True when local player has any radar provider and radar is not disabled.
    pub fn local_radar_active(&self) -> bool {
        self.local_radar_count > 0 && !self.local_radar_disabled
    }

    /// Energy ratio residual (produced / max(consumed,1)) for power bar UI.
    pub fn local_energy_ratio(&self) -> f32 {
        let demand = self.local_power_consumed.max(1) as f32;
        self.local_power_produced as f32 / demand
    }

    /// Whether a science name is unlocked for the local player residual.
    pub fn local_has_science(&self, name: &str) -> bool {
        self.local_unlocked_sciences.iter().any(|s| s == name)
    }

    /// Generals rank residual frozen at snapshot.
    pub fn local_rank_level(&self) -> u32 {
        self.local_rank_level
    }

    /// GeneralsExperience skill points residual.
    pub fn local_skill_points(&self) -> i32 {
        self.local_skill_points
    }

    /// Remaining science purchase points residual.
    pub fn local_science_purchase_points(&self) -> i32 {
        self.local_science_purchase_points
    }

    /// ControlBar rank bar progress residual (0..100).
    pub fn local_rank_progress_percent(&self) -> i32 {
        self.local_rank_progress_percent
    }

    pub fn superweapon_timers(&self) -> &[PresentationSuperweaponTimer] {
        &self.superweapon_timers
    }

    pub fn ready_public_superweapons(&self) -> impl Iterator<Item = &PresentationSuperweaponTimer> {
        self.superweapon_timers.iter().filter(|t| t.ready)
    }

    /// Objects with a ready special power residual (UI / command button feed).
    pub fn special_power_ready_objects(&self) -> Vec<&RenderableObject> {
        // Wave 1102: fail-closed on sold/disabled SP-ready residual feed.
        self.objects
            .iter()
            .filter(|o| !o.destroyed && !o.sold && !o.disabled && o.special_power_ready)
            .collect()
    }

    /// Special-power cooldown fraction remaining in 0..1 (0 = ready).
    pub fn special_power_cooldown_fraction(obj: &RenderableObject) -> f32 {
        if obj.special_power_cooldown <= 0.0 {
            return 0.0;
        }
        (obj.special_power_cooldown_remaining / obj.special_power_cooldown).clamp(0.0, 1.0)
    }

    /// Objects that have applied at least one upgrade residual.
    pub fn upgraded_objects(&self) -> Vec<&RenderableObject> {
        // Wave 1109: upgrade residual feed excludes sold.
        self.objects
            .iter()
            .filter(|o| !o.destroyed && !o.sold && !o.applied_upgrades.is_empty())
            .collect()
    }

    /// Whether `upgrade` is applied on the object residual.
    pub fn object_has_upgrade(obj: &RenderableObject, upgrade: &str) -> bool {
        obj.applied_upgrades.iter().any(|u| u == upgrade)
    }

    /// Live mine / demo-trap presentation residuals.
    pub fn mine_objects(&self) -> Vec<&RenderableObject> {
        // Wave 1109: mine residual feed excludes sold.
        self.objects
            .iter()
            .filter(|o| !o.destroyed && !o.sold && o.has_mine)
            .collect()
    }

    /// True when snapshot object carries `kind` residual.
    pub fn object_has_kind(obj: &RenderableObject, kind: crate::game_logic::KindOf) -> bool {
        obj.kind_of.iter().any(|k| *k == kind)
    }

    /// C++ `Object::isMassSelectable` (`Object.cpp:3024`): selectable and not a structure.
    /// Double-click type-select refuses buildings (`SelectionXlat.cpp:475`).
    pub fn presentation_is_mass_selectable(obj: &RenderableObject) -> bool {
        use crate::game_logic::KindOf;
        use crate::unit_control::UnitControlSystem;
        UnitControlSystem::presentation_is_selectable(obj)
            && !obj.is_structure
            && !Self::object_has_kind(obj, KindOf::Structure)
            && obj.object_type != PresentationObjectType::Building
    }

    /// Same-template locally-owned mass-selectables (`similarUnitSelection`, InGameUI.cpp:150).
    /// Map-wide: C++ `selectMatchingAcrossRegion(NULL)` / ALT double-click.
    /// Matches `ThingTemplate::isEquivalentTo` plus `OBJECT_STATUS_IS_CARBOMB`,
    /// skips contained occupants (`!object->isContained()`), and skips
    /// `Object::isOffMap()` (playable extent via `world_env` bounds).
    pub fn similar_unit_ids(
        &self,
        clicked_id: ObjectId,
        player_team: crate::game_logic::Team,
    ) -> Vec<ObjectId> {
        let Some(clicked) = self.objects.iter().find(|o| o.id == clicked_id) else {
            return Vec::new();
        };
        if clicked.contained_by.is_some()
            || !self.similar_unit_is_local(clicked, player_team)
            || !Self::presentation_is_mass_selectable(clicked)
        {
            return Vec::new();
        }
        let clicked_is_carbomb = clicked.is_carbomb;
        let template = clicked.template_name.as_str();
        self.objects
            .iter()
            .filter(|o| {
                o.contained_by.is_none()
                    && self.similar_unit_is_local(o, player_team)
                    && Self::presentation_is_mass_selectable(o)
                    && !self.presentation_is_off_map(o)
                    && (Self::templates_equivalent_for_type_select(
                        template,
                        o.template_name.as_str(),
                    ) || (clicked_is_carbomb && o.is_carbomb))
            })
            .map(|o| o.id)
            .collect()
    }

    /// C++ `Object::isOffMap` — playable extent, not cargo-plane residual 0..500.
    fn presentation_is_off_map(&self, object: &RenderableObject) -> bool {
        let (world_min, world_max) = self.world_env.world_bounds_vec3();
        crate::game_logic::host_deliver_payload::is_off_map_residual(
            object.position,
            world_min.x,
            world_min.z,
            world_max.x,
            world_max.z,
        )
    }

    /// C++ `ThingTemplate::isEquivalentTo` (`ThingTemplate.cpp:1454-1494`).
    /// Identity plus leftover reskin / final-override / BuildVariations walk.
    /// Not a general-prefix stem compare.
    fn templates_equivalent_for_type_select(left: &str, right: &str) -> bool {
        crate::game_logic::weapon_bootstrap::splash_templates_equivalent(left, right)
    }

    fn similar_unit_is_local(
        &self,
        object: &RenderableObject,
        player_team: crate::game_logic::Team,
    ) -> bool {
        if object.owner_player_id.is_some() {
            self.is_owned_by_local(object)
        } else {
            object.team == player_team
        }
    }

    /// C++ `selectMatchingAcrossScreen` (`InGameUI.cpp:4789`): same template, current view only.
    pub fn similar_unit_ids_across_screen(
        &self,
        clicked_id: ObjectId,
        player_team: crate::game_logic::Team,
        view_matrix: glam::Mat4,
        projection_matrix: glam::Mat4,
        viewport_size: glam::Vec2,
    ) -> Vec<ObjectId> {
        let viewport_width = viewport_size.x.max(1.0);
        let viewport_height = viewport_size.y.max(1.0);
        let view_projection = projection_matrix * view_matrix;
        if !view_projection.is_finite() {
            return Vec::new();
        }
        self.similar_unit_ids(clicked_id, player_team)
            .into_iter()
            .filter(|id| {
                self.objects.iter().any(|object| {
                    object.id == *id
                        && super::alive::project_position_to_screen(
                            view_projection,
                            object.position,
                            viewport_width,
                            viewport_height,
                        )
                        .is_some_and(|screen| {
                            screen.x >= 0.0
                                && screen.y >= 0.0
                                && screen.x <= viewport_width
                                && screen.y <= viewport_height
                        })
                })
            })
            .collect()
    }

    /// Double-click / ALT variant (`SelectionXlat.cpp:466,498-501`).
    pub fn similar_unit_ids_for_double_click(
        &self,
        clicked_id: ObjectId,
        player_team: crate::game_logic::Team,
        across_map: bool,
        view_matrix: glam::Mat4,
        projection_matrix: glam::Mat4,
        viewport_size: glam::Vec2,
    ) -> Vec<ObjectId> {
        if across_map {
            self.similar_unit_ids(clicked_id, player_team)
        } else {
            self.similar_unit_ids_across_screen(
                clicked_id,
                player_team,
                view_matrix,
                projection_matrix,
                viewport_size,
            )
        }
    }

    /// Right-click residual: enemy attackable under cursor id from snapshot.
    pub fn is_enemy_attackable(
        &self,
        target_id: ObjectId,
        player_team: crate::game_logic::Team,
    ) -> bool {
        use crate::unit_control::UnitControlSystem;
        let _ = player_team;
        // Wave 1103: fail-closed on non-local FOW unless Clear (pick parity).
        self.objects
            .iter()
            .find(|o| o.id == target_id)
            .map(|o| {
                self.is_enemy_of_local(o)
                    && o.fow_visibility.visibility_alpha >= 0.95
                    && UnitControlSystem::presentation_is_attackable(o)
            })
            .unwrap_or(false)
    }

    /// Drag-box residual: friendly selectable units whose XZ pose is inside the rect.
    ///
    /// Prefer non-structures when any unit is in the box (C++ InGameUI drag residual).
    /// If only structures are hit, keep a single structure when exactly one is present.
    /// Filter stored ids to alive selectable friendlies (control-group recall residual).
    /// Script camera-slave residual: first non-destroyed object matching template (case-insensitive).
    /// Control-group double-tap residual: average XZ pose of listed alive objects.
    /// Runtime-host residual: first alive mobile friendly (select_local_unit).
    pub fn first_mobile_friendly_id(
        &self,
        player_team: crate::game_logic::Team,
    ) -> Option<ObjectId> {
        use crate::unit_control::UnitControlSystem;
        self.objects
            .iter()
            .find(|o| {
                o.team == player_team
                    && !o.destroyed
                    && o.is_mobile
                    && UnitControlSystem::presentation_is_selectable(o)
            })
            .map(|o| o.id)
    }

    /// Runtime-host residual: first constructed structure with production capacity.
    pub fn first_constructed_producer_id(
        &self,
        player_team: crate::game_logic::Team,
    ) -> Option<ObjectId> {
        // Prefer barracks/warfactory/airfield; fall back to any can_produce structure.
        // Wave 1100: fail-closed on sold/UC/disabled producers (train UI residual).
        let usable = |o: &&RenderableObject| {
            o.team == player_team
                && !o.destroyed
                && !o.sold
                && !o.under_construction
                && !o.disabled
                && o.can_produce
        };
        self.objects
            .iter()
            .find(|o| {
                usable(o)
                    && o.building_type
                        .map(PresentationBuildingType::is_unit_producer)
                        .unwrap_or(false)
            })
            .or_else(|| self.objects.iter().find(usable))
            .map(|o| o.id)
    }

    /// Structures that can produce units (ControlBar factory residual feed).
    pub fn unit_producer_structures(&self) -> Vec<&RenderableObject> {
        // Wave 1100: fail-closed on sold/UC/disabled factory residual feed.
        self.objects
            .iter()
            .filter(|o| {
                !o.destroyed
                    && !o.sold
                    && !o.under_construction
                    && !o.disabled
                    && o.can_produce
                    && o.building_type
                        .map(PresentationBuildingType::is_unit_producer)
                        .unwrap_or(false)
            })
            .collect()
    }

    /// Runtime-host residual: first alive enemy attackable.

    /// Unique non-empty model keys from alive objects (GPU preload residual).
    pub fn unique_model_keys(&self) -> Vec<String> {
        let mut keys = Vec::new();
        let mut seen = std::collections::HashSet::new();
        for o in &self.objects {
            if o.destroyed {
                continue;
            }
            if o.draw_models.is_empty() {
                if let Some(k) = o.model_key.as_ref() {
                    if !k.is_empty() && seen.insert(k.clone()) {
                        keys.push(k.clone());
                    }
                }
            } else {
                for draw_model in &o.draw_models {
                    if !draw_model.model_key.is_empty() && seen.insert(draw_model.model_key.clone())
                    {
                        keys.push(draw_model.model_key.clone());
                    }
                }
            }
        }
        keys
    }

    /// Structures holding supply crates residual (ControlBar / gather UI).
    pub fn supply_storage_structures(&self) -> Vec<&RenderableObject> {
        // Wave 1101: fail-closed on sold supply-storage residual feed.
        self.objects
            .iter()
            .filter(|o| {
                !o.destroyed
                    && !o.sold
                    && o.stored_supplies > 0
                    && (o.is_structure
                        || o.building_type.is_some()
                        || o.object_type == PresentationObjectType::Building
                        || o.object_type == PresentationObjectType::Supply)
            })
            .collect()
    }

    /// Friendly workers residual (dozer / worker command feed by team).
    pub fn friendly_workers(&self, player_team: crate::game_logic::Team) -> Vec<&RenderableObject> {
        use crate::game_logic::KindOf;
        let _ = player_team;
        // Wave 1101: fail-closed on sold/disabled worker residual feed.
        self.objects
            .iter()
            .filter(|o| {
                self.is_owned_by_local(o)
                    && !o.destroyed
                    && !o.sold
                    && !o.disabled
                    && (Self::object_has_kind(o, KindOf::Worker)
                        || o.template_name.contains("Dozer")
                        || o.template_name.contains("Worker")
                        || o.template_name.contains("Construction"))
            })
            .collect()
    }

    pub fn first_enemy_attackable_id(
        &self,
        player_team: crate::game_logic::Team,
    ) -> Option<ObjectId> {
        use crate::unit_control::UnitControlSystem;
        let _ = player_team;
        // Wave 1104: fail-closed on non-local FOW unless Clear (is_enemy_attackable parity).
        self.objects
            .iter()
            .find(|o| {
                self.is_enemy_of_local(o)
                    && o.fow_visibility.visibility_alpha >= 0.95
                    && UnitControlSystem::presentation_is_attackable(o)
            })
            .map(|o| o.id)
    }

    /// Host `attack_nearest_enemy` residual: FOW-clear attackable first, then force-attack.
    pub fn first_enemy_attack_command_id(
        &self,
        player_team: crate::game_logic::Team,
    ) -> Option<ObjectId> {
        // Wave 1115: prefer is_enemy_attackable parity, then force-attack fallback.
        self.first_enemy_attackable_id(player_team)
            .or_else(|| self.first_enemy_force_attack_id(player_team))
    }

    /// Runtime-host residual: prefer non-structure enemy, else any attackable enemy.
    pub fn first_enemy_force_attack_id(
        &self,
        player_team: crate::game_logic::Team,
    ) -> Option<ObjectId> {
        use crate::game_logic::KindOf;
        use crate::unit_control::UnitControlSystem;
        let _ = player_team;
        // Wave 1105: fail-closed on non-local FOW unless Clear (is_enemy_attackable /
        // first_enemy_attackable_id parity). Force-attack object residual must not
        // pick fogged/black enemies the local player cannot see.
        let visible_enemy = |o: &&RenderableObject| {
            self.is_enemy_of_local(o)
                && o.fow_visibility.visibility_alpha >= 0.95
                && UnitControlSystem::presentation_is_attackable(o)
        };
        let mobile = self.objects.iter().find(|o| {
            visible_enemy(o)
                && !Self::object_has_kind(o, KindOf::Structure)
                && o.object_type != PresentationObjectType::Building
        });
        mobile
            .or_else(|| self.objects.iter().find(visible_enemy))
            .map(|o| o.id)
    }
}
