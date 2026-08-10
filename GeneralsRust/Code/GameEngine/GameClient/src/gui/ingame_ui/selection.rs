// Selection box/state, idle workers, matching, and control groups.
// Split from `gui/ingame_ui.rs` dump. Included by `ingame_ui/mod.rs`.

impl InGameUI {
    fn perform_box_selection(&mut self, rect: UIRect, selection_type: SelectionType) -> Result<()> {
        let start = Vec2::new(rect.x, rect.y);
        let end = Vec2::new(rect.x + rect.width, rect.y + rect.height);
        let Some(world_start) = self.screen_to_world(start) else {
            return Ok(());
        };
        let Some(world_end) = self.screen_to_world(end) else {
            return Ok(());
        };

        let min_x = world_start.x.min(world_end.x).floor() as i32;
        let max_x = world_start.x.max(world_end.x).ceil() as i32;
        let min_y = world_start.y.min(world_end.y).floor() as i32;
        let max_y = world_start.y.max(world_end.y).ceil() as i32;

        let region = IRegion2D::new(ICoord2D::new(min_x, min_y), ICoord2D::new(max_x, max_y));

        let selection_manager = get_selection_manager();
        if let Ok(mut manager) = selection_manager.write() {
            if let Some(selection) = manager.get_player_selection(self.player_id as i32) {
                selection.select_in_region(region, selection_type, None);
            }
        }
        self.sync_selection_state();
        Ok(())
    }

    /// Perform single click selection
    fn perform_click_selection(&mut self, pos: Vec2, selection_type: SelectionType) -> Result<()> {
        if let Some(object_id) = self.pick_object_at_screen(pos) {
            let selection_manager = get_selection_manager();
            let mut manager = match selection_manager.write() {
                Ok(manager) => manager,
                Err(_) => {
                    self.sync_selection_state();
                    return Ok(());
                }
            };
            if let Some(selection) = manager.get_player_selection(self.player_id as i32) {
                selection.select_objects(vec![object_id], selection_type);
            }
        } else if matches!(selection_type, SelectionType::Replace) {
            let selection_manager = get_selection_manager();
            let mut manager = match selection_manager.write() {
                Ok(manager) => manager,
                Err(_) => {
                    self.sync_selection_state();
                    return Ok(());
                }
            };
            if let Some(selection) = manager.get_player_selection(self.player_id as i32) {
                selection.clear_selection();
            }
        }
        self.sync_selection_state();
        Ok(())
    }

    fn screen_to_world(&self, screen_pos: Vec2) -> Option<Coord3D> {
        let screen_pt = IPoint2::new(screen_pos.x as i32, screen_pos.y as i32);
        with_tactical_view_ref(|view| {
            view.screen_to_world(&screen_pt)
                .ok()
                .map(|pt| Coord3D::new(pt.x, pt.y, pt.z))
        })
    }

    fn world_to_screen(&self, world: &Coord3D) -> Option<Vec2> {
        let point = Point3::new(world.x, world.y, world.z);
        with_tactical_view_ref(|view| {
            view.world_to_screen(&point)
                .map(|pt| Vec2::new(pt.x as f32, pt.y as f32))
        })
    }

    fn pick_object_at_screen(&self, screen_pos: Vec2) -> Option<ObjectID> {
        const PICK_RADIUS_WORLD: f32 = 12.0;
        let Some(world) = self.screen_to_world(screen_pos) else {
            return None;
        };
        // Host/presentation path: no dual-world factory objects to pick.
        if OBJECT_REGISTRY.is_empty() {
            return None;
        }

        let mut best: Option<(ObjectID, f32)> = None;
        for obj in OBJECT_REGISTRY.get_all_objects() {
            let Ok(guard) = obj.read() else {
                continue;
            };
            if !guard.is_selectable() {
                continue;
            }
            let pos = guard.get_position();
            let dx = pos.x - world.x;
            let dy = pos.y - world.y;
            let dist_sq = dx * dx + dy * dy;
            if dist_sq <= PICK_RADIUS_WORLD * PICK_RADIUS_WORLD
                && best
                    .map(|(_, best_dist)| dist_sq < best_dist)
                    .unwrap_or(true)
            {
                best = Some((guard.get_id(), dist_sq));
            }
        }
        best.map(|(id, _)| id)
    }

    fn select_similar_units(
        &mut self,
        template_object_id: ObjectID,
        add_to_selection: bool,
    ) -> Result<()> {
        // Wave 966: host empty dual-world → presentation unit catalog residual.
        if dual_world_registry_unavailable() {
            let seed = template_object_id;
            let Some(reference) = self
                .presentation_unit_catalog
                .iter()
                .find(|u| u.object_id == seed)
                .cloned()
            else {
                return Ok(());
            };
            if !reference.selectable || reference.template_name.is_empty() {
                return Ok(());
            }
            // Wave 1089: select-similar seed residual fail-closed on unusable seed
            // (destroyed/sold/masked/disabled must not drive mass-select).
            if reference.destroyed
                || reference.sold
                || reference.masked
                || reference.disabled
                || reference.unselectable
            {
                return Ok(());
            }
            let mut matching: Vec<ObjectID> = self
                .presentation_unit_catalog
                .iter()
                .filter(|u| {
                    // Wave 1089: candidate residual fail-closed on unusable / non-local
                    // stealth/FOW (matches collect_selectable_objects_from_presentation).
                    if u.destroyed || u.sold || u.unselectable || u.masked || u.disabled {
                        return false;
                    }
                    let local_team =
                        crate::presentation_translator_residual::translator_local_team_name();
                    let local = !local_team.is_empty() && u.team_name == local_team;
                    if u.effectively_stealthed && !local {
                        return false;
                    }
                    let fogged = matches!(
                        u.shroud_status,
                        ObjectShroudStatus::PartialClear
                            | ObjectShroudStatus::Fogged
                            | ObjectShroudStatus::Shrouded
                    );
                    if fogged && !local {
                        return false;
                    }
                    // Wave 1041: match on apparent template/team for disguised units
                    // (C++ non-allied viewers see disguise identity).
                    let uref_t = if reference.disguised {
                        reference
                            .disguise_as_template
                            .as_deref()
                            .filter(|s| !s.is_empty())
                            .unwrap_or(reference.template_name.as_str())
                    } else {
                        reference.template_name.as_str()
                    };
                    let uref_team = if reference.disguised {
                        reference
                            .disguise_as_team
                            .as_deref()
                            .filter(|s| !s.is_empty())
                            .unwrap_or(reference.team_name.as_str())
                    } else {
                        reference.team_name.as_str()
                    };
                    // Local player always matches real identity of own units.
                    let local =
                        crate::presentation_translator_residual::translator_local_team_name();
                    let (ut, uteam) = if !local.is_empty() && u.team_name == local {
                        (u.template_name.as_str(), u.team_name.as_str())
                    } else if u.disguised {
                        (
                            u.disguise_as_template
                                .as_deref()
                                .filter(|s| !s.is_empty())
                                .unwrap_or(u.template_name.as_str()),
                            u.disguise_as_team
                                .as_deref()
                                .filter(|s| !s.is_empty())
                                .unwrap_or(u.team_name.as_str()),
                        )
                    } else {
                        (u.template_name.as_str(), u.team_name.as_str())
                    };
                    // Reference apparent from local view:
                    let (rt, rteam) = if !local.is_empty() && reference.team_name == local {
                        (
                            reference.template_name.as_str(),
                            reference.team_name.as_str(),
                        )
                    } else {
                        (uref_t, uref_team)
                    };
                    u.selectable && ut == rt && uteam == rteam
                })
                .map(|u| u.object_id)
                .collect();
            if matching.is_empty() {
                return Ok(());
            }
            matching.sort_unstable();
            matching.dedup();

            let selection_type = if add_to_selection {
                SelectionType::Add
            } else {
                SelectionType::Replace
            };
            let selection_manager = get_selection_manager();
            if let Ok(mut manager) = selection_manager.write() {
                if let Some(selection) = manager.get_player_selection(self.player_id as i32) {
                    selection.select_objects(matching, selection_type);
                }
            }
            self.sync_selection_state();
            return Ok(());
        }

        let Some(reference) = OBJECT_REGISTRY.get_object(template_object_id) else {
            return Ok(());
        };
        let Ok(reference_guard) = reference.read() else {
            return Ok(());
        };
        let template_name = reference_guard.get_template_name().to_string();
        let owner_id = reference_guard
            .get_controlling_player_id()
            .map(|id| id as i32);

        let mut matching: Vec<ObjectID> = Vec::new();
        for obj in OBJECT_REGISTRY.get_all_objects() {
            let Ok(guard) = obj.read() else {
                continue;
            };
            if !guard.is_selectable() {
                continue;
            }
            if guard.get_template_name() != template_name {
                continue;
            }
            if let Some(owner) = owner_id {
                if guard.get_controlling_player_id().map(|id| id as i32) != Some(owner) {
                    continue;
                }
            }
            matching.push(guard.get_id());
        }

        if matching.is_empty() {
            return Ok(());
        }

        let selection_type = if add_to_selection {
            SelectionType::Add
        } else {
            SelectionType::Replace
        };
        let selection_manager = get_selection_manager();
        if let Ok(mut manager) = selection_manager.write() {
            if let Some(selection) = manager.get_player_selection(self.player_id as i32) {
                selection.select_objects(matching, selection_type);
            }
        }
        self.sync_selection_state();
        Ok(())
    }

    fn sync_selection_state(&mut self) {
        let selection_manager = get_selection_manager();
        let selected_objects = if let Ok(manager) = selection_manager.read() {
            manager
                .get_player_selection_ref(self.player_id as i32)
                .map(|selection| selection.get_selected_objects())
                .unwrap_or_default()
        } else {
            Vec::new()
        };
        self.selection_state.selected = selected_objects.into_iter().map(DrawableID).collect();
    }

    fn find_selected_builder(&self) -> Option<ObjectID> {
        let selection_manager = get_selection_manager();
        let selected_ids = if let Ok(manager) = selection_manager.read() {
            manager
                .get_player_selection_ref(self.player_id as i32)
                .map(|selection| selection.get_selected_objects())
                .unwrap_or_default()
        } else {
            Vec::new()
        };

        for object_id in &selected_ids {
            if let Some(object_arc) = TheGameLogic::find_object_by_id(*object_id) {
                if let Ok(object_guard) = object_arc.read() {
                    if object_guard.is_kind_of(KindOf::Dozer) {
                        return Some(*object_id);
                    }
                }
            }
        }

        selected_ids.first().copied()
    }

    /// Start building placement preview
    pub fn select_object(&mut self, id: u32, add_to_selection: bool) {
        let selection_type = if add_to_selection {
            SelectionType::Add
        } else {
            SelectionType::Replace
        };
        let selection_manager = get_selection_manager();
        if let Ok(mut manager) = selection_manager.write() {
            if let Some(selection) = manager.get_player_selection(self.player_id as i32) {
                selection.select_objects(vec![id as ObjectID], selection_type);
            }
        }
        self.sync_selection_state();
    }

    /// Deselect object
    pub fn deselect_object(&mut self, id: u32) {
        let selection_manager = get_selection_manager();
        if let Ok(mut manager) = selection_manager.write() {
            if let Some(selection) = manager.get_player_selection(self.player_id as i32) {
                selection.select_objects(vec![id as ObjectID], SelectionType::Remove);
            }
        }
        self.sync_selection_state();
    }

    /// Clear all selections
    pub fn clear_selection(&mut self) {
        let selection_manager = get_selection_manager();
        if let Ok(mut manager) = selection_manager.write() {
            if let Some(selection) = manager.get_player_selection(self.player_id as i32) {
                selection.clear_selection();
            }
        }
        self.sync_selection_state();
    }

    /// Get current selection
    pub fn get_selection(&self) -> Vec<u32> {
        let selection_manager = get_selection_manager();
        if let Ok(manager) = selection_manager.read() {
            if let Some(selection) = manager.get_player_selection_ref(self.player_id as i32) {
                return selection.get_selected_objects().into_iter().collect();
            }
        }
        self.selection_state
            .get_selected()
            .iter()
            .map(|id| id.0)
            .collect()
    }

    /// Wave 964: stamp presentation selection residual for host empty dual-world path.
    pub fn set_presentation_selection_residual(
        &mut self,
        units: Vec<PresentationSelectedUnitResidual>,
    ) {
        self.presentation_selected = units;
    }

    /// Wave 964: read-only presentation selection residual.
    pub fn presentation_selection_residual(&self) -> &[PresentationSelectedUnitResidual] {
        &self.presentation_selected
    }

    /// Wave 966/968: stamp presentation unit catalog + local team residual.
    pub fn set_presentation_unit_catalog(&mut self, units: Vec<PresentationUnitCatalogEntry>) {
        self.presentation_unit_catalog = units;
    }

    /// Wave 968: stamp local player team residual (Debug name).
    pub fn set_presentation_local_team_name(&mut self, team_name: impl Into<String>) {
        self.presentation_local_team_name = team_name.into();
    }

    pub fn presentation_local_team_name(&self) -> &str {
        &self.presentation_local_team_name
    }

    /// Wave 966: read-only presentation unit catalog residual.
    pub fn presentation_unit_catalog(&self) -> &[PresentationUnitCatalogEntry] {
        &self.presentation_unit_catalog
    }

    /// Set selection group
    pub fn set_selection_group(&mut self, group: usize) {
        if group < 10 {
            let selection_manager = get_selection_manager();
            if let Ok(mut manager) = selection_manager.write() {
                if let Some(selection) = manager.get_player_selection(self.player_id as i32) {
                    selection.create_control_group(group);
                }
            }
            self.sync_selection_state();
        }
    }

    /// Recall selection group
    pub fn recall_selection_group(&mut self, group: usize) {
        let selection_manager = get_selection_manager();
        if let Ok(mut manager) = selection_manager.write() {
            if let Some(selection) = manager.get_player_selection(self.player_id as i32) {
                selection.select_control_group(group, false);
            }
        }
        self.sync_selection_state();
    }

    /// Set local player id for selection routing.
    pub fn add_idle_worker(&mut self, object_id: ObjectID, player_index: u8) {
        if !self.idle_workers.iter().any(|w| w.object_id == object_id) {
            self.idle_workers.push(IdleWorkerData {
                object_id,
                player_index,
            });
        }
    }

    pub fn remove_idle_worker(&mut self, object_id: ObjectID, _player_index: u8) {
        self.idle_workers.retain(|w| w.object_id != object_id);
    }

    pub fn find_idle_worker(&self, object_id: ObjectID) -> bool {
        self.idle_workers.iter().any(|w| w.object_id == object_id)
    }

    pub fn get_idle_worker_count(&self, player_index: u8) -> usize {
        self.idle_workers
            .iter()
            .filter(|w| w.player_index == player_index)
            .count()
    }

    pub fn select_next_idle_worker(&self, player_index: u8) -> Option<ObjectID> {
        self.idle_workers
            .iter()
            .find(|w| w.player_index == player_index)
            .map(|w| w.object_id)
    }

    pub fn reset_idle_workers(&mut self) {
        self.idle_workers.clear();
    }

    pub fn add_to_control_group(&mut self, group: i32, obj_id: ObjectID) {
        if !(0..=9).contains(&group) {
            return;
        }
        let group_idx = group as usize;
        let selection_manager = get_selection_manager();
        let Ok(mut manager) = selection_manager.write() else {
            return;
        };
        if let Some(state) = manager.get_player_selection(self.player_id as i32) {
            let group_ids = state.get_control_group_objects(group_idx).to_vec();
            if !group_ids.contains(&obj_id) {
                let mut updated = group_ids;
                updated.push(obj_id);
                state.set_control_group_objects(group_idx, updated);
            }
        }
    }

    /// Get all objects in a control group. Returns empty vec if group is empty.
    pub fn get_control_group(&self, group: i32) -> Vec<ObjectID> {
        if !(0..=9).contains(&group) {
            return Vec::new();
        }
        let selection_manager = get_selection_manager();
        if let Ok(manager) = selection_manager.read() {
            if let Some(state) = manager.get_player_selection_ref(self.player_id as i32) {
                return state.get_control_group_objects(group as usize).to_vec();
            }
        }
        Vec::new()
    }

    /// Select all objects in a control group. Replaces current selection.
    pub fn select_control_group(&mut self, group: i32) {
        if !(0..=9).contains(&group) {
            return;
        }
        let group_idx = group as usize;
        let group_ids = {
            let selection_manager = get_selection_manager();
            let Ok(manager) = selection_manager.read() else {
                return;
            };
            let Some(state) = manager.get_player_selection_ref(self.player_id as i32) else {
                return;
            };
            let ids = state.get_control_group_objects(group_idx);
            if ids.is_empty() {
                return;
            }
            ids.to_vec()
        };

        let selection_manager = get_selection_manager();
        if let Ok(mut manager) = selection_manager.write() {
            if let Some(state) = manager.get_player_selection(self.player_id as i32) {
                state.select_objects(group_ids, SelectionType::Replace);
            }
        }
        self.frame_selection_changed = self.current_frame;
        self.sync_selection_state();
    }

    // ── Build placement template methods ─────────────────────────────
    // C++: placeBuildAvailable(), getPendingPlaceType()

    /// Set the current build placement template. C++: placeBuildAvailable()
    /// Passing None clears the placement state.
    pub fn are_selected_objects_controllable(&self) -> bool {
        let selection_manager = get_selection_manager();
        let Ok(manager) = selection_manager.read() else {
            return false;
        };
        let Some(selection) = manager.get_player_selection_ref(self.player_id as i32) else {
            return false;
        };
        let selected = selection.get_selected_objects();
        if selected.is_empty() {
            return false;
        }
        // C++: All selected objects have the same local controller, return first one
        if let Some(&first_id) = selected.first() {
            if let Some(obj) = TheGameLogic::find_object_by_id(first_id) {
                if let Ok(guard) = obj.read() {
                    return guard.is_locally_controlled();
                }
            }
        }
        false
    }

    pub fn is_any_selected_kind_of(&self, kind_of: KindOf) -> bool {
        // Wave 964: host empty dual-world → presentation kind residual.
        if dual_world_registry_unavailable() {
            let name = format!("{kind_of:?}");
            return self.presentation_selected.iter().any(|u| {
                // Wave 1090: selected-kind residual ignores unusable entries.
                if u.destroyed || u.sold || u.masked || u.unselectable {
                    return false;
                }
                u.kind_names
                    .iter()
                    .any(|k| k == &name || k.eq_ignore_ascii_case(&name))
            });
        }

        let selection_manager = get_selection_manager();
        let Ok(manager) = selection_manager.read() else {
            return false;
        };
        let Some(selection) = manager.get_player_selection_ref(self.player_id as i32) else {
            return false;
        };
        for object_id in selection.get_selected_objects() {
            if let Some(obj) = OBJECT_REGISTRY.get_object(object_id) {
                if let Ok(guard) = obj.read() {
                    if guard.is_kind_of(kind_of) {
                        return true;
                    }
                }
            }
        }
        false
    }

    pub fn is_all_selected_kind_of(&self, kind_of: KindOf) -> bool {
        // Wave 964: host empty dual-world → presentation kind residual.
        if dual_world_registry_unavailable() {
            let name = format!("{kind_of:?}");
            // Wave 1090: evaluate only usable selected entries (empty usable → true).
            let usable: Vec<_> = self
                .presentation_selected
                .iter()
                .filter(|u| !(u.destroyed || u.sold || u.masked || u.unselectable))
                .collect();
            if usable.is_empty() {
                return true;
            }
            return usable.iter().all(|u| {
                u.kind_names
                    .iter()
                    .any(|k| k == &name || k.eq_ignore_ascii_case(&name))
            });
        }

        let selection_manager = get_selection_manager();
        let Ok(manager) = selection_manager.read() else {
            return true; // vacuously true when nothing selected (matches C++ empty-loop behavior)
        };
        let Some(selection) = manager.get_player_selection_ref(self.player_id as i32) else {
            return true;
        };
        for object_id in selection.get_selected_objects() {
            if let Some(obj) = OBJECT_REGISTRY.get_object(object_id) {
                if let Ok(guard) = obj.read() {
                    if !guard.is_kind_of(kind_of) {
                        return false;
                    }
                }
            }
        }
        true
    }

    // ── Advanced selection methods ─────────────────────────────────────
    // C++: InGameUI.cpp:4900 (selectUnitsMatchingCurrentSelection)
    // C++: InGameUI.cpp:4850 (selectMatchingAcrossMap)

    pub fn select_units_matching_current_selection(&mut self) -> i32 {
        // C++: First tries selectMatchingAcrossScreen(), if 0 results tries selectMatchingAcrossMap()
        let screen_count = self.select_matching_across_screen();
        if screen_count > 0 {
            return screen_count;
        }
        self.select_matching_across_map()
    }

    pub fn select_matching_across_screen(&mut self) -> i32 {
        let screen_region = with_tactical_view_ref(|view| {
            let tl = view.screen_to_world(&IPoint2::new(0, 0)).ok()?;
            let br = view
                .screen_to_world(&IPoint2::new(
                    self.screen_size.x as i32,
                    self.screen_size.y as i32,
                ))
                .ok()?;
            Some(IRegion2D::new(
                ICoord2D::new(tl.x.min(br.x).floor() as i32, tl.y.min(br.y).floor() as i32),
                ICoord2D::new(tl.x.max(br.x).ceil() as i32, tl.y.max(br.y).ceil() as i32),
            ))
        });
        let region = match screen_region {
            Some(r) => r,
            None => return self.select_matching_across_map(),
        };
        self.select_matching_across_region(&region)
    }

    /// Wave 967: host empty dual-world select-matching via presentation unit catalog.
    fn select_matching_from_presentation_catalog(&mut self, region: Option<&IRegion2D>) -> i32 {
        // Seed templates from selection manager, else presentation selection residual.
        let selection_manager = get_selection_manager();
        let mut selected_ids: Vec<ObjectID> = if let Ok(manager) = selection_manager.read() {
            manager
                .get_player_selection_ref(self.player_id as i32)
                .map(|s| s.get_selected_objects())
                .unwrap_or_default()
        } else {
            Vec::new()
        };
        if selected_ids.is_empty() {
            selected_ids = self
                .presentation_selected
                .iter()
                .map(|u| u.object_id)
                .collect();
        }
        if selected_ids.is_empty() || self.presentation_unit_catalog.is_empty() {
            return -1;
        }

        let mut templates: Vec<String> = Vec::new();
        let mut local_team: Option<String> = None;
        for &object_id in &selected_ids {
            let Some(entry) = self
                .presentation_unit_catalog
                .iter()
                .find(|u| u.object_id == object_id)
            else {
                // Fall back to presentation_selected residual for template name.
                if let Some(sel) = self
                    .presentation_selected
                    .iter()
                    .find(|u| u.object_id == object_id)
                {
                    if !sel.template_name.is_empty() && !templates.contains(&sel.template_name) {
                        templates.push(sel.template_name.clone());
                    }
                }
                continue;
            };
            if !entry.selectable {
                continue;
            }
            // Wave 1089: select-matching seed residual fail-closed on unusable.
            if entry.destroyed || entry.sold || entry.masked || entry.disabled || entry.unselectable
            {
                continue;
            }
            if local_team.is_none() {
                local_team = Some(entry.team_name.clone());
            }
            if local_team.as_deref() != Some(entry.team_name.as_str()) {
                continue;
            }
            if !entry.template_name.is_empty() && !templates.contains(&entry.template_name) {
                templates.push(entry.template_name.clone());
            }
        }
        if templates.is_empty() {
            return -1;
        }
        let team_filter = local_team;

        let mut matching: Vec<ObjectID> = self
            .presentation_unit_catalog
            .iter()
            .filter(|u| {
                if !u.selectable {
                    return false;
                }
                // Wave 1089: select-matching candidate residual fail-closed on
                // unusable / non-local stealth/FOW.
                if u.destroyed || u.sold || u.unselectable || u.masked || u.disabled {
                    return false;
                }
                let local_name =
                    crate::presentation_translator_residual::translator_local_team_name();
                let local = !local_name.is_empty() && u.team_name == local_name;
                if u.effectively_stealthed && !local {
                    return false;
                }
                let fogged = matches!(
                    u.shroud_status,
                    ObjectShroudStatus::PartialClear
                        | ObjectShroudStatus::Fogged
                        | ObjectShroudStatus::Shrouded
                );
                if fogged && !local {
                    return false;
                }
                if let Some(team) = team_filter.as_ref() {
                    if &u.team_name != team {
                        return false;
                    }
                }
                if let Some(region) = region {
                    let (x, y) = (u.position[0], u.position[1]);
                    if x < region.lo.x as f32
                        || x > region.hi.x as f32
                        || y < region.lo.y as f32
                        || y > region.hi.y as f32
                    {
                        return false;
                    }
                }
                templates.iter().any(|t| t == &u.template_name)
            })
            .map(|u| u.object_id)
            .collect();
        if matching.is_empty() {
            return 0;
        }
        matching.sort_unstable();
        matching.dedup();
        let count = matching.len() as i32;
        if let Ok(mut manager) = selection_manager.write() {
            if let Some(selection) = manager.get_player_selection(self.player_id as i32) {
                selection.select_objects(matching, SelectionType::Add);
            }
        }
        self.frame_selection_changed = self.current_frame;
        self.sync_selection_state();
        count
    }

    fn select_matching_across_region(&mut self, region: &IRegion2D) -> i32 {
        // Wave 967: host empty dual-world → presentation unit catalog residual.
        if dual_world_registry_unavailable() {
            return self.select_matching_from_presentation_catalog(Some(region));
        }

        let selection_manager = get_selection_manager();
        let selected_ids = if let Ok(manager) = selection_manager.read() {
            manager
                .get_player_selection_ref(self.player_id as i32)
                .map(|s| s.get_selected_objects())
                .unwrap_or_default()
        } else {
            return -1;
        };

        let mut templates: Vec<String> = Vec::new();
        for &object_id in &selected_ids {
            if let Some(obj) = OBJECT_REGISTRY.get_object(object_id) {
                if let Ok(guard) = obj.read() {
                    if guard.is_locally_controlled() {
                        let name = guard.get_template_name().to_string();
                        if !templates.contains(&name) {
                            templates.push(name);
                        }
                    }
                }
            }
        }

        if templates.is_empty() {
            return -1;
        }
        // Host/presentation path: no dual-world objects.
        if OBJECT_REGISTRY.is_empty() {
            return -1;
        }

        let mut matching: Vec<ObjectID> = Vec::new();
        for obj in OBJECT_REGISTRY.get_all_objects() {
            let Ok(guard) = obj.read() else {
                continue;
            };
            if !guard.is_selectable() || !guard.is_locally_controlled() {
                continue;
            }
            let pos = guard.get_position();
            if pos.x < region.lo.x as f32
                || pos.x > region.hi.x as f32
                || pos.y < region.lo.y as f32
                || pos.y > region.hi.y as f32
            {
                continue;
            }
            if templates.iter().any(|t| t == guard.get_template_name()) {
                matching.push(guard.get_id());
            }
        }

        if matching.is_empty() {
            return 0;
        }

        let count = matching.len() as i32;
        let selection_manager = get_selection_manager();
        if let Ok(mut manager) = selection_manager.write() {
            if let Some(selection) = manager.get_player_selection(self.player_id as i32) {
                selection.select_objects(matching, SelectionType::Add);
            }
        }
        self.frame_selection_changed = self.current_frame;
        self.sync_selection_state();
        count
    }

    pub fn select_matching_across_map(&mut self) -> i32 {
        // Wave 967: host empty dual-world → presentation unit catalog residual.
        if dual_world_registry_unavailable() {
            return self.select_matching_from_presentation_catalog(None);
        }

        // C++: InGameUI.cpp:4671 (selectMatchingAcrossRegion with NULL region)
        // Gets templates from current selection, iterates all objects, selects matching
        let selection_manager = get_selection_manager();
        let selected_ids = if let Ok(manager) = selection_manager.read() {
            manager
                .get_player_selection_ref(self.player_id as i32)
                .map(|s| s.get_selected_objects())
                .unwrap_or_default()
        } else {
            return -1;
        };

        // Collect unique template names from locally-controlled selected objects
        let mut templates: Vec<String> = Vec::new();
        for &object_id in &selected_ids {
            if let Some(obj) = OBJECT_REGISTRY.get_object(object_id) {
                if let Ok(guard) = obj.read() {
                    if guard.is_locally_controlled() {
                        let name = guard.get_template_name().to_string();
                        if !templates.contains(&name) {
                            templates.push(name);
                        }
                    }
                }
            }
        }

        if templates.is_empty() {
            return -1;
        }
        // Host/presentation path: no dual-world objects.
        if OBJECT_REGISTRY.is_empty() {
            return -1;
        }

        // Select all matching objects across the map
        let mut matching: Vec<ObjectID> = Vec::new();
        for obj in OBJECT_REGISTRY.get_all_objects() {
            let Ok(guard) = obj.read() else {
                continue;
            };
            if !guard.is_selectable() {
                continue;
            }
            if !guard.is_locally_controlled() {
                continue;
            }
            let obj_template = guard.get_template_name();
            if templates.iter().any(|t| t == obj_template) {
                matching.push(guard.get_id());
            }
        }

        if matching.is_empty() {
            return 0;
        }

        let count = matching.len() as i32;
        let selection_manager = get_selection_manager();
        if let Ok(mut manager) = selection_manager.write() {
            if let Some(selection) = manager.get_player_selection(self.player_id as i32) {
                selection.select_objects(matching, SelectionType::Add);
            }
        }
        self.frame_selection_changed = self.current_frame;
        self.sync_selection_state();
        count
    }

    // ── Drawable lifecycle ─────────────────────────────────────────────
    // C++: InGameUI.cpp:3415 (disregardDrawable)

    pub fn disregard_drawable(&mut self, drawable_id: u32) {
        self.deselect_object(drawable_id);
    }

    // ── Selection change tracking ──────────────────────────────────────

    pub fn get_frame_selection_changed(&self) -> u32 {
        self.frame_selection_changed
    }

    // ── Movie playback ────────────────────────────────────────────────
    // C++: InGameUI.cpp:3874 (playMovie), 3901 (stopMovie),
    //       3929 (playCameoMovie), 3959 (stopCameoMovie)

    /// Get selection count. C++: InGameUI::getSelectCount() (InGameUI.h)
    fn get_select_count(&self) -> usize {
        self.get_selection().len()
    }

}
