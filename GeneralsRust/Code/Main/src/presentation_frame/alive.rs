use super::*;
use glam::{Mat4, Vec2, Vec3};

impl PresentationFrame {
    /// Hotkey residual: idle selectable friendly workers/dozers/chinooks/supply/hack.
    pub fn alive_selectable_friendly_idle_worker_ids(
        &self,
        player_team: crate::game_logic::Team,
    ) -> Vec<ObjectId> {
        use crate::unit_control::UnitControlSystem;
        let mut ids: Vec<ObjectId> = self
            .objects
            .iter()
            .filter(|o| {
                if o.team != player_team
                    || o.destroyed
                    || !UnitControlSystem::presentation_is_selectable(o)
                {
                    return false;
                }
                if !Self::presentation_is_worker_like(o) {
                    return false;
                }
                // Prefer idle residual (no move dest / attack / construct busy).
                o.move_destination.is_none()
                    && o.attack_target.is_none()
                    && !o.under_construction
                    && o.ai_state_ordinal == 0
            })
            .map(|o| o.id)
            .collect();
        ids.sort_by_key(|id| id.0);
        ids
    }

    /// Hotkey residual: busy selectable friendly workers (non-idle worker-like).
    pub fn alive_selectable_friendly_busy_worker_ids(
        &self,
        player_team: crate::game_logic::Team,
    ) -> Vec<ObjectId> {
        use crate::unit_control::UnitControlSystem;
        let idle: std::collections::HashSet<_> = self
            .alive_selectable_friendly_idle_worker_ids(player_team)
            .into_iter()
            .collect();
        let mut ids: Vec<ObjectId> = self
            .objects
            .iter()
            .filter(|o| {
                o.team == player_team
                    && !o.destroyed
                    && UnitControlSystem::presentation_is_selectable(o)
                    && Self::presentation_is_worker_like(o)
                    && !idle.contains(&o.id)
            })
            .map(|o| o.id)
            .collect();
        ids.sort_by_key(|id| id.0);
        ids
    }

    /// Hotkey residual: unfinished (under construction, not sold) friendly selectables.
    pub fn alive_selectable_friendly_unfinished_ids(
        &self,
        player_team: crate::game_logic::Team,
    ) -> Vec<ObjectId> {
        use crate::unit_control::UnitControlSystem;
        let mut ids: Vec<ObjectId> = self
            .objects
            .iter()
            .filter(|o| {
                o.team == player_team
                    && !o.destroyed
                    && UnitControlSystem::presentation_is_selectable(o)
                    && o.under_construction
                    && !o.sold
            })
            .map(|o| o.id)
            .collect();
        ids.sort_by_key(|id| id.0);
        ids
    }

    pub fn presentation_is_worker_like(o: &RenderableObject) -> bool {
        use crate::game_logic::KindOf;
        if Self::object_has_kind(o, KindOf::Worker) {
            return true;
        }
        let n = o.template_name.to_ascii_lowercase();
        n.contains("dozer")
            || n.contains("worker")
            || n.contains("chinook")
            || n.contains("supply")
            || n.contains("hack")
            || n.contains("crane")
    }

    /// Runtime-host residual: first friendly Command Center pose.
    pub fn first_friendly_command_center_position(
        &self,
        player_team: crate::game_logic::Team,
    ) -> Option<Vec3> {
        use crate::game_logic::KindOf;
        // Wave 1101: fail-closed on sold/UC/disabled CC residual.
        self.objects
            .iter()
            .find(|o| {
                o.team == player_team
                    && !o.destroyed
                    && !o.sold
                    && !o.under_construction
                    && !o.disabled
                    && (o.building_type == Some(PresentationBuildingType::CommandCenter)
                        || Self::object_has_kind(o, KindOf::CommandCenter))
            })
            .map(|o| o.position)
    }

    /// Runtime-host residual: count of alive mobile friendlies.
    pub fn count_mobile_friendlies(&self, player_team: crate::game_logic::Team) -> u32 {
        use crate::unit_control::UnitControlSystem;
        // Wave 1102: mobile count residual uses presentation selectable legality.
        self.objects
            .iter()
            .filter(|o| {
                o.team == player_team
                    && o.is_mobile
                    && UnitControlSystem::presentation_is_selectable(o)
            })
            .count() as u32
    }

    /// Runtime-host residual: selected friendly count from snapshot.
    pub fn count_selected_friendlies(&self, player_team: crate::game_logic::Team) -> u32 {
        use crate::unit_control::UnitControlSystem;
        // Wave 1103: selected count residual uses presentation selectable legality.
        self.objects
            .iter()
            .filter(|o| {
                o.team == player_team
                    && o.selected
                    && UnitControlSystem::presentation_is_selectable(o)
            })
            .count() as u32
    }

    /// Runtime-host residual: under-construction friendly count from snapshot.
    pub fn count_under_construction_friendlies(&self, player_team: crate::game_logic::Team) -> u32 {
        // Wave 1104: fail-closed on sold UC residual count.
        self.objects
            .iter()
            .filter(|o| o.team == player_team && !o.destroyed && !o.sold && o.under_construction)
            .count() as u32
    }

    /// Runtime-host residual: first alive friendly sample pose + name.
    pub fn first_friendly_sample_label(
        &self,
        player_team: crate::game_logic::Team,
    ) -> Option<String> {
        // Wave 1106: sample label residual fail-closed on sold.
        self.objects
            .iter()
            .find(|o| o.team == player_team && !o.destroyed && !o.sold)
            .map(|o| {
                format!(
                    "{:.1},{:.1},{:.1}:{}",
                    o.position.x, o.position.y, o.position.z, o.template_name
                )
            })
    }

    pub fn centroid_of_ids(&self, ids: &[ObjectId]) -> Option<glam::Vec3> {
        // Wave 1107: camera/group centroid residual fail-closed on sold.
        let mut sum = glam::Vec3::ZERO;
        let mut n = 0u32;
        for id in ids {
            if let Some(o) = self.objects.iter().find(|o| o.id == *id) {
                if o.destroyed || o.sold {
                    continue;
                }
                sum += o.position;
                n += 1;
            }
        }
        if n == 0 { None } else { Some(sum / n as f32) }
    }

    pub fn first_alive_position_for_template(&self, template_name: &str) -> Option<glam::Vec3> {
        // Wave 1106: template pose residual fail-closed on sold.
        self.objects
            .iter()
            .find(|o| {
                !o.destroyed && !o.sold && o.template_name.eq_ignore_ascii_case(template_name)
            })
            .map(|o| o.position)
    }

    pub fn filter_alive_selectable_ids(
        &self,
        ids: &[ObjectId],
        player_team: crate::game_logic::Team,
    ) -> Vec<ObjectId> {
        use crate::unit_control::UnitControlSystem;
        let _ = player_team;
        let mut out = Vec::new();
        for id in ids {
            if let Some(o) = self.objects.iter().find(|o| o.id == *id) {
                if self.is_owned_by_local(o) && UnitControlSystem::presentation_is_selectable(o) {
                    out.push(*id);
                }
            }
        }
        out
    }

    /// C++ `Object::isSelectable` (`Object.cpp:3001-3020`) used by
    /// `Squad::getLiveObjects`. Control-group recall calls `selectDrawable`
    /// on these members — it does **not** use `CanSelectDrawable`, so contained
    /// (garrison/transport/FireBase) and MASKED members stay live.
    pub fn presentation_is_squad_live(o: &RenderableObject) -> bool {
        if crate::unit_control::UnitControlSystem::presentation_is_always_selectable(o) {
            return true;
        }
        !o.destroyed
            && !o.unselectable
            && Self::object_has_kind(o, crate::game_logic::KindOf::Selectable)
    }

    /// C++ `Squad::getLiveObjects` then optional
    /// `obj->getControllingPlayer() == localPlayer` (SELECT_TEAM only).
    /// ADD_TEAM / double-tap `lookAt` pass `require_local_owner = false`.
    pub fn filter_live_squad_ids(
        &self,
        ids: &[ObjectId],
        require_local_owner: bool,
    ) -> Vec<ObjectId> {
        let mut out = Vec::new();
        for id in ids {
            if let Some(o) = self.objects.iter().find(|o| o.id == *id) {
                if Self::presentation_is_squad_live(o)
                    && (!require_local_owner || self.is_owned_by_local(o))
                {
                    out.push(*id);
                }
            }
        }
        out
    }

    /// Old snapshots did not include object owner provenance.  Preserve their
    /// faction-only selection behavior only when the whole frame is legacy;
    /// mixing a `None` owner into a live owner-aware frame must not let a
    /// same-faction opponent become selectable.
    #[inline]
    pub fn uses_legacy_team_ownership_fallback(&self) -> bool {
        self.objects
            .iter()
            .all(|object| object.owner_player_id.is_none())
    }

    /// Whether this frozen object is controlled by the local player. Faction
    /// is an art/template property, not authority, once the frame has owner
    /// provenance.
    #[inline]
    pub fn is_owned_by_local(&self, object: &RenderableObject) -> bool {
        match object.owner_player_id {
            Some(owner_player_id) => owner_player_id == self.local_player_id,
            None => self.uses_legacy_team_ownership_fallback() && object.team == self.local_team,
        }
    }

    /// Player relationship represented by this presentation snapshot.  It
    /// intentionally uses the same limited rule as host GameLogic: self and
    /// matching non-negative alliance slots are allied; distinct live slots
    /// are enemies. Unknown owner provenance stays neutral/fail-closed.
    pub fn is_allied_with_local(&self, object: &RenderableObject) -> bool {
        let Some(owner_player_id) = object.owner_player_id else {
            return self.uses_legacy_team_ownership_fallback() && object.team == self.local_team;
        };
        if owner_player_id == self.local_player_id {
            return true;
        }
        let Some(local) = self.player_info(self.local_player_id) else {
            return false;
        };
        let Some(owner) = self.player_info(owner_player_id) else {
            return false;
        };
        local.is_alive
            && owner.is_alive
            && local.alliance_team >= 0
            && local.alliance_team == owner.alliance_team
    }

    /// C++ `Team::getRelationship` slice used only by the StealthUpdate
    /// viewer look. Unlike selection ownership, the source relation does not
    /// turn an otherwise-allied object into an enemy merely because that
    /// object's controlling player is dead; the caller separately handles an
    /// inactive *viewer*.
    #[inline]
    fn local_stealth_viewer_sees_as_allied(&self, object: &RenderableObject) -> bool {
        let Some(owner_player_id) = object.owner_player_id else {
            return self.uses_legacy_team_ownership_fallback() && object.team == self.local_team;
        };
        if owner_player_id == self.local_player_id {
            return true;
        }
        let Some(local) = self.player_info(self.local_player_id) else {
            return false;
        };
        let Some(owner) = self.player_info(owner_player_id) else {
            return false;
        };
        local.alliance_team >= 0 && local.alliance_team == owner.alliance_team
    }

    /// Frozen viewer-relation slice of C++
    /// `StealthUpdate::calcStealthedStatusForPlayer` for the direct scene and
    /// ordinary WGPU mesh gates.
    ///
    /// An inactive local player (including observer/dead state in the current
    /// host projection) sees the source relation as `ALLIES`; an effectively
    /// dead object likewise has `StealthLook::None`.  Keep this separate from
    /// generic ownership/alliance queries, whose input/selection meanings are
    /// intentionally different.
    #[inline]
    pub fn local_viewer_hides_stealthed(&self, object: &RenderableObject) -> bool {
        object.effectively_stealthed
            && !object.can_disguise_as_team
            && !object.drawable_shroud.effectively_dead
            && self.local_is_alive
            && !self.local_stealth_viewer_sees_as_allied(object)
    }

    /// Whether C++ would select VISIBLE_FRIENDLY or
    /// VISIBLE_FRIENDLY_DETECTED for this frozen object.  Unlike the generic
    /// selection/alliance helper this intentionally does not require the
    /// owner player to be alive: StealthUpdate asks the viewer's relationship
    /// and treats an inactive viewer as ALLIES.
    #[inline]
    pub fn local_viewer_uses_friendly_stealth_look(&self, object: &RenderableObject) -> bool {
        object.stealthed
            && !object.can_disguise_as_team
            && !object.drawable_shroud.effectively_dead
            && self.local_stealth_viewer_sees_as_allied(object)
    }

    /// True only for a proven, active opposing player. This avoids inventing
    /// hostility for neutral props or ownerless compatibility records.
    pub fn is_enemy_of_local(&self, object: &RenderableObject) -> bool {
        if object.team == crate::game_logic::Team::Neutral {
            return false;
        }
        let Some(owner_player_id) = object.owner_player_id else {
            return self.uses_legacy_team_ownership_fallback() && object.team != self.local_team;
        };
        if owner_player_id == self.local_player_id {
            return false;
        }
        let Some(local) = self.player_info(self.local_player_id) else {
            return false;
        };
        let Some(owner) = self.player_info(owner_player_id) else {
            return false;
        };
        local.is_alive
            && owner.is_alive
            && !(local.alliance_team >= 0 && local.alliance_team == owner.alliance_team)
    }

    /// All alive selectable friendlies (Ctrl+A / Tab cycle residual).
    /// Local player team frozen on this frame (selection/hotkey consumers).
    #[inline]
    pub fn local_team(&self) -> Team {
        self.local_team
    }

    /// Frozen team base pose for camera snap proximity (None if unknown).
    pub fn local_team_base_or_hint(&self, fallback: Vec3) -> Vec3 {
        self.local_team_base_position.unwrap_or(fallback)
    }

    /// Wave 563: presentation-owned template name residual (train/UI contains).
    #[inline]
    pub fn has_template_name(&self, name: &str) -> bool {
        // Binary search on sorted freeze; fail-closed on miss (no live dual-read).
        self.known_template_names
            .binary_search_by(|n| n.as_str().cmp(name))
            .is_ok()
    }

    /// Look up frozen player roster entry by id.
    #[inline]
    pub fn player_info(&self, id: u32) -> Option<&PresentationPlayerInfo> {
        self.players.iter().find(|p| p.id == id)
    }

    /// Frozen player display name (defeat/alliance UI residual).
    #[inline]
    pub fn player_name(&self, id: u32) -> Option<&str> {
        self.player_info(id).map(|p| p.name.as_str())
    }

    /// Frozen player team (radar/defeat residual).
    #[inline]
    pub fn player_team(&self, id: u32) -> Option<Team> {
        self.player_info(id).map(|p| p.team)
    }

    /// Select friendlies inside a world-XZ radius of a center (on-screen residual).
    pub fn alive_selectable_friendly_near(
        &self,
        player_team: crate::game_logic::Team,
        center: glam::Vec3,
        radius: f32,
    ) -> Vec<ObjectId> {
        use crate::unit_control::UnitControlSystem;
        let r2 = radius * radius;
        let mut ids: Vec<ObjectId> = self
            .objects
            .iter()
            .filter(|o| {
                if o.team != player_team || !UnitControlSystem::presentation_is_selectable(o) {
                    return false;
                }
                let dx = o.position.x - center.x;
                let dz = o.position.z - center.z;
                dx * dx + dz * dz <= r2
            })
            .map(|o| o.id)
            .collect();
        ids.sort_by_key(|id| id.0);
        ids
    }

    pub fn alive_selectable_friendly_ids(
        &self,
        player_team: crate::game_logic::Team,
    ) -> Vec<ObjectId> {
        use crate::unit_control::UnitControlSystem;
        let mut ids: Vec<ObjectId> = self
            .objects
            .iter()
            .filter(|o| o.team == player_team && UnitControlSystem::presentation_is_selectable(o))
            .map(|o| o.id)
            .collect();
        ids.sort_by_key(|id| id.0);
        ids
    }

    /// Retail SELECT_HERO residual: friendly selectable heroes from snapshot.
    pub fn alive_selectable_friendly_hero_ids(
        &self,
        player_team: crate::game_logic::Team,
    ) -> Vec<ObjectId> {
        use crate::game_logic::KindOf;
        use crate::unit_control::UnitControlSystem;
        let mut ids: Vec<ObjectId> = self
            .objects
            .iter()
            .filter(|o| {
                o.team == player_team
                    && UnitControlSystem::presentation_is_selectable(o)
                    && !o.destroyed
                    && (o.kind_of.contains(&KindOf::Hero) || o.template_name.contains("Hero"))
            })
            .map(|o| o.id)
            .collect();
        ids.sort_by_key(|id| id.0);
        ids
    }

    /// Retail SELECT_ALL_AIRCRAFT (KEY_W) residual.
    /// Generic friendly selectable filter residual from snapshot.
    pub fn alive_selectable_friendly_filtered_ids(
        &self,
        _player_team: crate::game_logic::Team,
        mut pred: impl FnMut(&RenderableObject) -> bool,
    ) -> Vec<ObjectId> {
        use crate::unit_control::UnitControlSystem;
        let mut ids: Vec<ObjectId> = self
            .objects
            .iter()
            .filter(|o| {
                self.is_owned_by_local(o)
                    && !o.destroyed
                    && UnitControlSystem::presentation_is_selectable(o)
                    && pred(o)
            })
            .map(|o| o.id)
            .collect();
        ids.sort_by_key(|id| id.0);
        ids
    }

    /// C++ `kindOfUnitSelection` / CommandXlat SELECT_ALL filters.
    pub fn is_select_all_disqualified(obj: &RenderableObject) -> bool {
        use crate::game_logic::KindOf;
        Self::object_has_kind(obj, KindOf::Dozer)
            || Self::object_has_kind(obj, KindOf::Harvester)
            || Self::object_has_kind(obj, KindOf::IgnoresSelectAll)
            || obj.is_structure
            || Self::object_has_kind(obj, KindOf::Structure)
    }

    /// Mass-selectable locally-owned units eligible for SELECT_ALL / SELECT_ALL_AIRCRAFT.
    pub fn alive_select_all_unit_ids(
        &self,
        player_team: crate::game_logic::Team,
        aircraft_only: bool,
    ) -> Vec<ObjectId> {
        use crate::game_logic::KindOf;
        self.alive_selectable_friendly_filtered_ids(player_team, |o| {
            if o.contained_by.is_some() || !Self::presentation_is_mass_selectable(o) {
                return false;
            }
            if Self::is_select_all_disqualified(o) {
                return false;
            }
            if aircraft_only && !Self::object_has_kind(o, KindOf::Aircraft) {
                return false;
            }
            true
        })
    }

    /// Project IDs into the current view (C++ `iterateDrawablesInRegion` screen pass).
    pub fn filter_ids_on_screen(
        &self,
        ids: &[ObjectId],
        view_matrix: Mat4,
        projection_matrix: Mat4,
        viewport_size: Vec2,
    ) -> Vec<ObjectId> {
        let viewport_width = viewport_size.x.max(1.0);
        let viewport_height = viewport_size.y.max(1.0);
        let view_projection = projection_matrix * view_matrix;
        if !view_projection.is_finite() {
            return Vec::new();
        }
        ids.iter()
            .copied()
            .filter(|id| {
                self.objects.iter().any(|object| {
                    object.id == *id
                        && project_position_to_screen(
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

    /// Combat units residual (mobile non-structure, not pure dozer/supply).
    pub fn alive_selectable_friendly_combat_ids(
        &self,
        player_team: crate::game_logic::Team,
    ) -> Vec<ObjectId> {
        self.alive_selectable_friendly_filtered_ids(player_team, |o| {
            if o.is_structure {
                return false;
            }
            let n = o.template_name.to_ascii_lowercase();
            if n.contains("dozer") || n.contains("worker") || n.contains("supply") {
                return false;
            }
            o.is_mobile || o.has_weapon || o.is_unit
        })
    }

    /// Runtime-host residual: sellable friendly structures (non-CC).
    pub fn alive_sellable_friendly_structure_ids(
        &self,
        player_team: crate::game_logic::Team,
    ) -> Vec<ObjectId> {
        use crate::game_logic::KindOf;
        use crate::unit_control::UnitControlSystem;
        let mut ids: Vec<ObjectId> = self
            .objects
            .iter()
            .filter(|o| {
                o.team == player_team
                    && !o.destroyed
                    && !o.sold
                    && !o.under_construction
                    // Wave 856: selectable OR known structure/building residual (construct
                    // same-frame may lag presentation_is_selectable flags).
                    // Wave 1102: sold residual fail-closed even on structure OR path.
                    && (UnitControlSystem::presentation_is_selectable(o)
                        || o.is_structure
                        || o.building_type.is_some()
                        || o.object_type == PresentationObjectType::Building)
                    && (Self::object_has_kind(o, KindOf::Structure)
                        || o.object_type == PresentationObjectType::Building
                        || o.is_structure
                        || o.building_type.is_some())
                    && !Self::object_has_kind(o, KindOf::CommandCenter)
                    && o.building_type != Some(PresentationBuildingType::CommandCenter)
            })
            .map(|o| o.id)
            .collect();
        ids.sort_by_key(|id| id.0);
        ids
    }

    /// Runtime-host residual: constructed friendly structures that can queue upgrades.
    pub fn alive_upgrade_producer_structure_ids(
        &self,
        player_team: crate::game_logic::Team,
    ) -> Vec<ObjectId> {
        use crate::game_logic::KindOf;
        // Wave 1102: fail-closed on sold/disabled upgrade-producer residual.
        let mut ids: Vec<ObjectId> = self
            .objects
            .iter()
            .filter(|o| {
                o.team == player_team
                    && !o.destroyed
                    && !o.sold
                    && !o.disabled
                    && !o.under_construction
                    && (Self::object_has_kind(o, KindOf::Structure)
                        || o.object_type == PresentationObjectType::Building
                        || o.can_produce
                        || o.building_type.is_some())
            })
            .map(|o| o.id)
            .collect();
        ids.sort_by_key(|id| id.0);
        ids
    }

    /// Runtime-host residual: friendly dozers/workers that can construct.
    pub fn alive_construct_builder_ids(
        &self,
        player_team: crate::game_logic::Team,
    ) -> Vec<ObjectId> {
        use crate::game_logic::KindOf;
        let mut ids: Vec<ObjectId> = self
            .objects
            .iter()
            .filter(|o| {
                // Wave 1101: fail-closed on sold/disabled construct builders.
                if o.team != player_team || o.destroyed || o.sold || o.disabled {
                    return false;
                }
                if Self::object_has_kind(o, KindOf::Worker) {
                    return true;
                }
                let name = o.template_name.to_ascii_lowercase();
                name.contains("dozer")
                    || name.contains("worker")
                    || name.contains("crane")
                    || name.contains("construction")
            })
            .map(|o| o.id)
            .collect();
        ids.sort_by_key(|id| id.0);
        ids
    }

    pub fn alive_selectable_friendly_moving_ids(
        &self,
        player_team: crate::game_logic::Team,
    ) -> Vec<ObjectId> {
        self.alive_selectable_friendly_filtered_ids(player_team, |o| {
            !o.is_structure
                && (o.moving
                    || o.move_destination.is_some()
                    || o.path_len > 0
                    || o.velocity.length_squared() > 1e-6)
        })
    }

    pub fn alive_selectable_friendly_attacking_ids(
        &self,
        player_team: crate::game_logic::Team,
    ) -> Vec<ObjectId> {
        self.alive_selectable_friendly_filtered_ids(player_team, |o| {
            !o.is_structure
                && (o.attacking
                    || o.attack_target.is_some()
                    || o.is_firing_weapon
                    || o.is_aiming_weapon)
        })
    }

    pub fn alive_selectable_friendly_guarding_ids(
        &self,
        player_team: crate::game_logic::Team,
    ) -> Vec<ObjectId> {
        // host_ai_state_ordinal: GuardingArea=9, GuardingObject=10, GuardRetaliating=20
        self.alive_selectable_friendly_filtered_ids(player_team, |o| {
            o.guard_position.is_some()
                || o.guard_target.is_some()
                || matches!(o.ai_state_ordinal, 9 | 10 | 20)
        })
    }

    pub fn alive_selectable_friendly_patrolling_ids(
        &self,
        player_team: crate::game_logic::Team,
    ) -> Vec<ObjectId> {
        // host_ai_state_ordinal: Patrolling = 11
        self.alive_selectable_friendly_filtered_ids(player_team, |o| o.ai_state_ordinal == 11)
    }

    pub fn alive_selectable_friendly_gathering_ids(
        &self,
        player_team: crate::game_logic::Team,
    ) -> Vec<ObjectId> {
        // AIState::Gathering=5, ReturningResources=6
        self.alive_selectable_friendly_filtered_ids(player_team, |o| {
            matches!(o.ai_state_ordinal, 5 | 6)
        })
    }

    pub fn alive_selectable_friendly_stealthed_ids(
        &self,
        player_team: crate::game_logic::Team,
    ) -> Vec<ObjectId> {
        self.alive_selectable_friendly_filtered_ids(player_team, |o| o.effectively_stealthed)
    }

    pub fn alive_selectable_friendly_veteran_ids(
        &self,
        player_team: crate::game_logic::Team,
    ) -> Vec<ObjectId> {
        self.alive_selectable_friendly_filtered_ids(player_team, |o| {
            !o.is_structure && !matches!(o.veterancy, PresentationVeterancy::Rookie)
        })
    }

    pub fn alive_selectable_friendly_harvester_ids(
        &self,
        player_team: crate::game_logic::Team,
    ) -> Vec<ObjectId> {
        self.alive_selectable_friendly_filtered_ids(player_team, |o| {
            let n = o.template_name.to_ascii_lowercase();
            n.contains("supply")
                || n.contains("harvester")
                || n.contains("gatherer")
                || n.contains("worker")
                || matches!(o.ai_state_ordinal, 5 | 6)
        })
    }

    pub fn alive_selectable_friendly_idle_harvester_ids(
        &self,
        player_team: crate::game_logic::Team,
    ) -> Vec<ObjectId> {
        self.alive_selectable_friendly_filtered_ids(player_team, |o| {
            let n = o.template_name.to_ascii_lowercase();
            let is_h = n.contains("supply")
                || n.contains("harvester")
                || n.contains("gatherer")
                || n.contains("worker");
            is_h && o.ai_state_ordinal == 0 && !o.moving
        })
    }

    pub fn alive_selectable_friendly_occupied_transport_ids(
        &self,
        player_team: crate::game_logic::Team,
    ) -> Vec<ObjectId> {
        self.alive_selectable_friendly_filtered_ids(player_team, |o| {
            !o.is_structure
                && o.occupant_count > 0
                && (o.max_transport > 0
                    || o.is_humvee_transport
                    || o.is_troop_crawler_transport
                    || o.is_combat_chinook_transport
                    || o.is_helix_transport
                    || o.is_battle_bus_transport
                    || o.is_technical_transport
                    || o.is_combat_cycle_transport)
        })
    }

    pub fn alive_selectable_friendly_docked_aircraft_ids(
        &self,
        player_team: crate::game_logic::Team,
    ) -> Vec<ObjectId> {
        // host_ai_state_ordinal Docked = 12
        self.alive_selectable_friendly_filtered_ids(player_team, |o| {
            (o.object_type == PresentationObjectType::Aircraft || o.airborne_target)
                && o.ai_state_ordinal == 12
        })
    }

    pub fn alive_selectable_friendly_repairing_ids(
        &self,
        player_team: crate::game_logic::Team,
    ) -> Vec<ObjectId> {
        // Repairing = 8, SeekingRepair = 15
        self.alive_selectable_friendly_filtered_ids(player_team, |o| {
            matches!(o.ai_state_ordinal, 8 | 15)
        })
    }

    pub fn alive_selectable_friendly_constructing_worker_ids(
        &self,
        player_team: crate::game_logic::Team,
    ) -> Vec<ObjectId> {
        // Constructing = 7
        self.alive_selectable_friendly_filtered_ids(player_team, |o| {
            let n = o.template_name.to_ascii_lowercase();
            o.ai_state_ordinal == 7
                || ((n.contains("dozer") || n.contains("worker") || n.contains("constructor"))
                    && (o.moving || o.ai_state_ordinal == 7))
        })
    }

    pub fn alive_selectable_friendly_idle_military_ids(
        &self,
        player_team: crate::game_logic::Team,
    ) -> Vec<ObjectId> {
        self.alive_selectable_friendly_filtered_ids(player_team, |o| {
            if o.is_structure {
                return false;
            }
            let n = o.template_name.to_ascii_lowercase();
            if n.contains("dozer") || n.contains("supply") || n.contains("harvester") {
                return false;
            }
            let combat = o.is_mobile || o.has_weapon || o.is_unit;
            combat
                && o.ai_state_ordinal == 0
                && !o.moving
                && !o.attacking
                && o.attack_target.is_none()
        })
    }

    pub fn alive_selectable_friendly_mobile_ids(
        &self,
        player_team: crate::game_logic::Team,
    ) -> Vec<ObjectId> {
        self.alive_selectable_friendly_filtered_ids(player_team, |o| o.is_mobile)
    }

    /// Damaged mobile units residual (health < max).
    pub fn alive_selectable_friendly_damaged_unit_ids(
        &self,
        player_team: crate::game_logic::Team,
    ) -> Vec<ObjectId> {
        use crate::unit_control::UnitControlSystem;
        let mut pairs: Vec<(ObjectId, f32)> = self
            .objects
            .iter()
            .filter(|o| {
                o.team == player_team
                    && !o.destroyed
                    && !o.is_structure
                    && UnitControlSystem::presentation_is_selectable(o)
                    && o.is_mobile
                    && o.health_max > 0.0
                    && o.health_current + 1e-3 < o.health_max
            })
            .map(|o| (o.id, o.health_current / o.health_max.max(1e-3)))
            .collect();
        pairs.sort_by(|a, b| {
            a.1.partial_cmp(&b.1)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then(a.0.0.cmp(&b.0.0))
        });
        pairs.into_iter().map(|(id, _)| id).collect()
    }

    pub fn alive_selectable_friendly_damaged_structure_ids(
        &self,
        player_team: crate::game_logic::Team,
    ) -> Vec<ObjectId> {
        use crate::unit_control::UnitControlSystem;
        let mut pairs: Vec<(ObjectId, f32)> = self
            .objects
            .iter()
            .filter(|o| {
                o.team == player_team
                    && !o.destroyed
                    && o.is_structure
                    && !o.under_construction
                    && UnitControlSystem::presentation_is_selectable(o)
                    && o.health_max > 0.0
                    && o.health_current + 1e-3 < o.health_max
            })
            .map(|o| (o.id, o.health_current / o.health_max.max(1e-3)))
            .collect();
        pairs.sort_by(|a, b| {
            a.1.partial_cmp(&b.1)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then(a.0.0.cmp(&b.0.0))
        });
        pairs.into_iter().map(|(id, _)| id).collect()
    }

    pub fn alive_selectable_friendly_busy_producer_ids(
        &self,
        player_team: crate::game_logic::Team,
    ) -> Vec<ObjectId> {
        self.alive_selectable_friendly_filtered_ids(player_team, |o| {
            o.is_structure && !o.production_queue.is_empty()
        })
    }

    pub fn alive_selectable_friendly_ready_special_power_ids(
        &self,
        player_team: crate::game_logic::Team,
    ) -> Vec<ObjectId> {
        self.alive_selectable_friendly_filtered_ids(player_team, |o| {
            o.is_structure && o.special_power_ready
        })
    }

    /// Stop-all residual: friendly mobile (non-structure) units.
    pub fn alive_friendly_stoppable_ids(
        &self,
        player_team: crate::game_logic::Team,
    ) -> Vec<ObjectId> {
        use crate::unit_control::UnitControlSystem;
        // Wave 1102: stop-all residual uses full presentation selectable legality.
        let mut ids: Vec<ObjectId> = self
            .objects
            .iter()
            .filter(|o| {
                o.team == player_team
                    && !o.is_structure
                    && o.is_mobile
                    && UnitControlSystem::presentation_is_selectable(o)
            })
            .map(|o| o.id)
            .collect();
        ids.sort_by_key(|id| id.0);
        ids
    }

    pub fn alive_selectable_friendly_aircraft_ids(
        &self,
        player_team: crate::game_logic::Team,
    ) -> Vec<ObjectId> {
        use crate::unit_control::UnitControlSystem;
        let mut ids: Vec<ObjectId> = self
            .objects
            .iter()
            .filter(|o| {
                o.team == player_team
                    && UnitControlSystem::presentation_is_selectable(o)
                    && (o.object_type == PresentationObjectType::Aircraft || o.airborne_target)
            })
            .map(|o| o.id)
            .collect();
        ids.sort_by_key(|id| id.0);
        ids
    }

    pub fn box_select_unit_ids(
        &self,
        player_team: crate::game_logic::Team,
        min_x: f32,
        max_x: f32,
        min_z: f32,
        max_z: f32,
    ) -> Vec<ObjectId> {
        let _ = player_team;
        let mut candidates = Vec::new();
        for o in &self.objects {
            let pos = o.position;
            if pos.x < min_x || pos.x > max_x || pos.z < min_z || pos.z > max_z {
                continue;
            }
            if let Some(picked) = self.resolve_box_pick_object(o) {
                candidates.push(picked);
            }
        }
        self.finalize_box_selection(candidates)
    }

    /// Select units inside the actual screen-space drag rectangle.
    ///
    /// C++ `SelectionTranslator` sends an `IRegion2D` pixel region to
    /// `TacticalView::iterateDrawablesInRegion`; it does not turn the two
    /// ground-ray intersections into a world X/Z rectangle.  More
    /// specifically, `W3DView::iterateDrawablesInRegion` projects each
    /// drawable's center and compares that point to the normalized region;
    /// drag selection does not inflate the region by geometry or selection
    /// radius. The presentation object list remains frozen, while the current
    /// camera matrices only project those frozen positions into the input/UI
    /// coordinate system.
    ///
    /// Selection membership follows `SelectionXlat.cpp:628-693` / `761-795`:
    /// friendlies win; a lone locally-owned building or FireBase container
    /// can be selected; otherwise exactly one enemy, civilian, or ally.
    pub fn box_select_unit_ids_in_screen_rect(
        &self,
        player_team: crate::game_logic::Team,
        view_matrix: Mat4,
        projection_matrix: Mat4,
        start: Vec2,
        end: Vec2,
        viewport_size: Vec2,
    ) -> Vec<ObjectId> {
        let _ = player_team;
        let viewport_width = viewport_size.x.max(1.0);
        let viewport_height = viewport_size.y.max(1.0);
        let view_projection = projection_matrix * view_matrix;
        if !view_projection.is_finite() {
            return Vec::new();
        }
        let min_x = start.x.min(end.x);
        let max_x = start.x.max(end.x);
        let min_y = start.y.min(end.y);
        let max_y = start.y.max(end.y);

        let mut candidates = Vec::new();
        for object in &self.objects {
            let Some(screen_position) = project_position_to_screen(
                view_projection,
                object.position,
                viewport_width,
                viewport_height,
            ) else {
                continue;
            };
            if screen_position.x < min_x
                || screen_position.x > max_x
                || screen_position.y < min_y
                || screen_position.y > max_y
            {
                continue;
            }
            if let Some(picked) = self.resolve_box_pick_object(object) {
                candidates.push(picked);
            }
        }
        self.finalize_box_selection(candidates)
    }

    /// C++ `ActionManager::canPlayerGarrison` residual for `SelectionInfo.cpp:129-131`.
    /// Used only by live box-select to leave infantry+one-garrisonable as enter.
    pub fn garrisonable_building_ids_in_screen_rect(
        &self,
        view_matrix: Mat4,
        projection_matrix: Mat4,
        start: Vec2,
        end: Vec2,
        viewport_size: Vec2,
    ) -> Vec<ObjectId> {
        let viewport_width = viewport_size.x.max(1.0);
        let viewport_height = viewport_size.y.max(1.0);
        let view_projection = projection_matrix * view_matrix;
        if !view_projection.is_finite() {
            return Vec::new();
        }
        let min_x = start.x.min(end.x);
        let max_x = start.x.max(end.x);
        let min_y = start.y.min(end.y);
        let max_y = start.y.max(end.y);

        let mut ids = Vec::new();
        for object in &self.objects {
            if !self.presentation_can_player_garrison(object) {
                continue;
            }
            let Some(screen_position) = project_position_to_screen(
                view_projection,
                object.position,
                viewport_width,
                viewport_height,
            ) else {
                continue;
            };
            if screen_position.x < min_x
                || screen_position.x > max_x
                || screen_position.y < min_y
                || screen_position.y > max_y
            {
                continue;
            }
            if !ids.contains(&object.id) {
                ids.push(object.id);
            }
        }
        ids
    }

    fn presentation_can_player_garrison(&self, object: &RenderableObject) -> bool {
        use crate::game_logic::KindOf;
        if object.destroyed || object.sold {
            return false;
        }
        if !object.capture_garrisonable && object.max_garrison == 0 {
            return false;
        }
        let is_structure = Self::presentation_is_structure(object)
            || object.capture_garrisonable
            || Self::object_has_kind(object, KindOf::GarrisonableUntilDestroyed);
        if !is_structure {
            return false;
        }
        if self.is_owned_by_local(object) {
            return true;
        }
        if self.is_enemy_of_local(object) {
            return false;
        }
        object.occupant_count == 0 && object.garrisoned_units.is_empty()
    }

    /// C++ `addDrawableToList` + FireBase non-enclosing container prop
    /// (`SelectionInfo.cpp:336-346`). Occupants that are not independently
    /// selectable promote their visible container.
    fn resolve_box_pick_object<'a>(
        &'a self,
        object: &'a RenderableObject,
    ) -> Option<&'a RenderableObject> {
        use crate::unit_control::UnitControlSystem;
        if UnitControlSystem::presentation_is_selectable(object) && object.contained_by.is_none() {
            if self.box_pick_hides_non_local(object) {
                return None;
            }
            return Some(object);
        }
        let container_id = object.contained_by?;
        let container = self
            .objects
            .iter()
            .find(|candidate| candidate.id == container_id)?;
        if !crate::game_logic::host_fire_base::is_fire_base_template(&container.template_name) {
            return None;
        }
        self.resolve_box_pick_object(container)
    }

    /// C++ `addDrawableToList` fog / undetected-stealth peel for neutrals and enemies.
    pub fn box_pick_hides_non_local(&self, object: &RenderableObject) -> bool {
        if self.is_owned_by_local(object) {
            return false;
        }
        let fogged = matches!(
            object.drawable_shroud.raw_status,
            PresentationObjectShroudStatus::Fogged
                | PresentationObjectShroudStatus::Shrouded
                | PresentationObjectShroudStatus::InvalidButPreviousValid
        ) || object.fow_visibility.visibility_alpha < 0.95;
        let stealthed = object.stealthed && !object.detected;
        fogged || stealthed
    }

    fn presentation_is_structure(object: &RenderableObject) -> bool {
        use crate::game_logic::KindOf;
        object.is_structure
            || Self::object_has_kind(object, KindOf::Structure)
            || object.object_type == PresentationObjectType::Building
    }

    /// C++ `SelectionXlat.cpp:628-693` then the `761-795` emit filter.
    fn finalize_box_selection(&self, candidates: Vec<&RenderableObject>) -> Vec<ObjectId> {
        use crate::game_logic::Team;
        use crate::unit_control::UnitControlSystem;

        let mut unique: Vec<&RenderableObject> = Vec::new();
        for candidate in candidates {
            if unique.iter().any(|existing| existing.id == candidate.id) {
                continue;
            }
            unique.push(candidate);
        }

        let mut mine = 0u32;
        let mut mine_buildings = 0u32;
        let mut enemies = 0u32;
        let mut civilians = 0u32;
        let mut friends = 0u32;
        for object in &unique {
            if self.is_owned_by_local(object) {
                mine += 1;
                if Self::presentation_is_structure(object) {
                    mine_buildings += 1;
                }
            } else if self.is_enemy_of_local(object) {
                enemies += 1;
            } else if object.team == Team::Neutral {
                civilians += 1;
            } else if self.is_allied_with_local(object) {
                friends += 1;
            }
        }

        let mut select_mine = false;
        let mut select_mine_buildings = false;
        let mut select_enemies = false;
        let mut select_civilians = false;
        let mut select_friends = false;
        if mine > 0 {
            select_mine = true;
            if mine_buildings == 1 && mine == 1 {
                select_mine_buildings = true;
            } else if mine_buildings > 0 {
                let mut only_one_building_selectable = true;
                let mut building_id = None;
                for object in &unique {
                    if Self::presentation_is_structure(object) {
                        match building_id {
                            None => building_id = Some(object.id),
                            Some(id) if id != object.id => only_one_building_selectable = false,
                            _ => {}
                        }
                    } else if UnitControlSystem::presentation_is_selectable(object) {
                        only_one_building_selectable = false;
                    }
                    if !only_one_building_selectable {
                        break;
                    }
                }
                if only_one_building_selectable {
                    select_mine_buildings = true;
                }
            }
        } else if enemies > 0 && civilians > 0 && friends > 0 {
            return Vec::new();
        } else if enemies == 1 {
            select_enemies = true;
        } else if civilians == 1 {
            select_civilians = true;
        } else if friends == 1 {
            select_friends = true;
        }

        if !(select_mine || select_enemies || select_civilians || select_friends) {
            return Vec::new();
        }

        unique
            .into_iter()
            .filter_map(|object| {
                if object.contained_by.is_some() {
                    return None;
                }
                if select_mine && self.is_owned_by_local(object) {
                    if !Self::presentation_is_structure(object) || select_mine_buildings {
                        return Some(object.id);
                    }
                    return None;
                }
                if select_enemies && self.is_enemy_of_local(object) {
                    return Some(object.id);
                }
                if select_civilians && object.team == Team::Neutral {
                    return Some(object.id);
                }
                if select_friends
                    && self.is_allied_with_local(object)
                    && !self.is_owned_by_local(object)
                {
                    return Some(object.id);
                }
                None
            })
            .collect()
    }

    /// Structures residual (KindOf::Structure or object_type Building).
    pub fn structure_objects(&self) -> Vec<&RenderableObject> {
        use crate::game_logic::KindOf;
        // Wave 1101: fail-closed on sold structure residual feed.
        self.objects
            .iter()
            .filter(|o| {
                !o.destroyed
                    && !o.sold
                    && (Self::object_has_kind(o, KindOf::Structure)
                        || o.object_type == PresentationObjectType::Building)
            })
            .collect()
    }

    /// Harvestable resource objects residual.
    pub fn harvestable_objects(&self) -> Vec<&RenderableObject> {
        use crate::game_logic::KindOf;
        // Wave 1101: fail-closed on sold harvestable residual feed.
        self.objects
            .iter()
            .filter(|o| !o.destroyed && !o.sold && Self::object_has_kind(o, KindOf::Harvestable))
            .collect()
    }

    /// Worker units residual (dozer / worker command feed).
    pub fn worker_objects(&self) -> Vec<&RenderableObject> {
        use crate::game_logic::KindOf;
        // Wave 1101: fail-closed on sold/disabled worker residual feed.
        self.objects
            .iter()
            .filter(|o| {
                !o.destroyed && !o.sold && !o.disabled && Self::object_has_kind(o, KindOf::Worker)
            })
            .collect()
    }
}

pub(super) fn project_position_to_screen(
    view_projection: Mat4,
    position: Vec3,
    viewport_width: f32,
    viewport_height: f32,
) -> Option<Vec2> {
    let clip = view_projection * position.extend(1.0);
    // A non-positive W is behind the camera for Main's right-handed WGPU
    // projection.  Keeping the sign is essential: `abs(w)` would mirror
    // behind-camera objects into a legitimate drag rectangle.
    if !clip.is_finite() || clip.w <= f32::EPSILON {
        return None;
    }
    let ndc = clip.truncate() / clip.w;
    if !ndc.is_finite() || !(0.0..=1.0).contains(&ndc.z) {
        return None;
    }
    let screen = Vec2::new(
        (ndc.x + 1.0) * 0.5 * viewport_width,
        (1.0 - ndc.y) * 0.5 * viewport_height,
    );
    screen.is_finite().then_some(screen)
}
