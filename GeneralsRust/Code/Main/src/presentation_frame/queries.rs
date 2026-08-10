use super::*;

impl PresentationFrame {
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
    /// Filters destroyed and engine-bridged objects (RenderBridge owns those).
    /// Includes local-player FOW alpha for skip/darkening without mid-render queries.
    pub fn unit_render_inputs(&self) -> Vec<UnitRenderInput> {
        // Wave 502: stealth mesh residual from frozen presentation only.
        // Wave 504: skip contained_by units; stamp garrisoned bits on structures.
        // Enemy effectively-stealthed units are omitted from the main mesh pass
        // (C++ auto-target / draw residual: not a legal observe target).
        // Local-team stealthed units keep a translucent FOW alpha residual.
        const ALLY_STEALTH_ALPHA: f32 = 0.35;
        let local_team = self.local_team;
        self.objects
            .iter()
            .filter(|o| !o.destroyed && !o.engine_bridged)
            .filter(|o| {
                // Wave 504: contained units are not drawn as free world meshes.
                if o.contained_by.is_some() {
                    return false;
                }
                if o.effectively_stealthed && o.team != local_team {
                    false
                } else {
                    true
                }
            })
            .map(|o| {
                let mut input = UnitRenderInput::from_renderable(o);
                // Wave 509: world weather/tod mesh bits from frozen presentation env.
                input.world_is_snow = self.world_env.is_snow;
                input.world_is_night = self.world_env.is_night;
                // Wave 513: logic frame for coast/reload residual compare.
                input.logic_frame = self.frame.0;
                if o.effectively_stealthed && o.team == local_team {
                    input.fow_visibility.visibility_alpha = input
                        .fow_visibility
                        .visibility_alpha
                        .min(ALLY_STEALTH_ALPHA);
                }
                // Wave 503: non-allied viewers see disguise mesh residual.
                if o.disguised && o.team != local_team {
                    if let Some(ref dt) = o.disguise_as_template {
                        if !dt.is_empty() {
                            input.model_key =
                                crate::assets::mesh_asset_resolve::model_key_from_presentation(
                                    Some(dt.as_str()),
                                    dt,
                                );
                        }
                    }
                }
                input
            })
            .collect()
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

    /// Double-click residual: same-template selectable friendlies from snapshot.
    pub fn similar_unit_ids(
        &self,
        clicked_id: ObjectId,
        player_team: crate::game_logic::Team,
    ) -> Vec<ObjectId> {
        use crate::game_logic::KindOf;
        use crate::unit_control::UnitControlSystem;
        let Some(clicked) = self.objects.iter().find(|o| o.id == clicked_id) else {
            return Vec::new();
        };
        if clicked.team != player_team || !UnitControlSystem::presentation_is_selectable(clicked) {
            return Vec::new();
        }
        let template = clicked.template_name.as_str();
        self.objects
            .iter()
            .filter(|o| {
                o.team == player_team
                    && UnitControlSystem::presentation_is_selectable(o)
                    && o.template_name == template
            })
            .map(|o| o.id)
            .collect()
    }

    /// Right-click residual: enemy attackable under cursor id from snapshot.
    pub fn is_enemy_attackable(
        &self,
        target_id: ObjectId,
        player_team: crate::game_logic::Team,
    ) -> bool {
        use crate::unit_control::UnitControlSystem;
        // Wave 1103: fail-closed on non-local FOW unless Clear (pick parity).
        self.objects
            .iter()
            .find(|o| o.id == target_id)
            .map(|o| {
                o.team != player_team
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
            if let Some(k) = o.model_key.as_ref() {
                if !k.is_empty() && seen.insert(k.clone()) {
                    keys.push(k.clone());
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
        // Wave 1101: fail-closed on sold/disabled worker residual feed.
        self.objects
            .iter()
            .filter(|o| {
                o.team == player_team
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
        // Wave 1104: fail-closed on non-local FOW unless Clear (is_enemy_attackable parity).
        self.objects
            .iter()
            .find(|o| {
                o.team != player_team
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
        // Wave 1105: fail-closed on non-local FOW unless Clear (is_enemy_attackable /
        // first_enemy_attackable_id parity). Force-attack object residual must not
        // pick fogged/black enemies the local player cannot see.
        let visible_enemy = |o: &&RenderableObject| {
            o.team != player_team
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
