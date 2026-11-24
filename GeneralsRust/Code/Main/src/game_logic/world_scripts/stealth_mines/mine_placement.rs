//! Mine, demo trap and charge placement.

use super::*;

impl GameLogic {
    /// Place a residual land mine at `position` for `team`.
    pub fn place_land_mine(
        &mut self,
        team: Team,
        position: Vec3,
        producer: Option<ObjectId>,
    ) -> Option<ObjectId> {
        self.place_land_mine_with_effects(team, position, producer, &mut Default::default())
    }

    pub(crate) fn place_land_mine_with_effects(
        &mut self,
        team: Team,
        position: Vec3,
        producer: Option<ObjectId>,
        effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> Option<ObjectId> {
        self.place_mine_kind_with_effects(
            crate::game_logic::host_mines::HostMineKind::LandMine,
            "TestLandMine",
            team,
            position,
            producer,
            None,
            None,
            effects,
        )
    }

    /// Place a residual GLA demo trap (proximity mode, standard detonation).
    pub fn place_demo_trap(
        &mut self,
        team: Team,
        position: Vec3,
        producer: Option<ObjectId>,
    ) -> Option<ObjectId> {
        self.place_demo_trap_with_effects(team, position, producer, &mut Default::default())
    }

    pub(crate) fn place_demo_trap_with_effects(
        &mut self,
        team: Team,
        position: Vec3,
        producer: Option<ObjectId>,
        effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> Option<ObjectId> {
        self.place_demo_trap_named_with_effects(
            "TestDemoTrap",
            team,
            position,
            producer,
            false,
            effects,
        )
    }

    /// Place a residual demo trap with Chem/Demo/Standard profile from template name.
    ///
    /// `has_gamma` applies Chem Gamma death weapon residual when true.
    pub fn place_demo_trap_named(
        &mut self,
        template_name: &str,
        team: Team,
        position: Vec3,
        producer: Option<ObjectId>,
        has_gamma: bool,
    ) -> Option<ObjectId> {
        self.place_demo_trap_named_with_effects(
            template_name,
            team,
            position,
            producer,
            has_gamma,
            &mut Default::default(),
        )
    }

    pub(crate) fn place_demo_trap_named_with_effects(
        &mut self,
        template_name: &str,
        team: Team,
        position: Vec3,
        producer: Option<ObjectId>,
        has_gamma: bool,
        effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> Option<ObjectId> {
        use crate::game_logic::host_mines::{HostMineData, HostMineKind, demo_trap_profile};

        self.ensure_residual_mine_template(template_name, HostMineKind::DemoTrap);
        let id =
            self.create_object_with_script_effects(template_name, team, position, None, effects)?;
        // Bound DemoTrapUpdate templates keep the factory-admitted runtime:
        // C++ createObject + setProducer only — no residual profile, weapon
        // strip, KindOf add, stealth grant, or invented place audio.
        {
            let object = self.objects.get_mut(&id)?;
            if object.get_template().receiving_object_definition.is_some() {
                object.producer_id = producer;
                if let Some(data) = object
                    .mine_data
                    .as_mut()
                    .filter(|data| data.kind == HostMineKind::DemoTrap)
                {
                    data.producer_id = producer;
                }
                self.mine_residual_places = self.mine_residual_places.saturating_add(1);
                return Some(id);
            }
        }
        let profile = demo_trap_profile(template_name, has_gamma, false);
        let mut data = HostMineData::demo_trap_with_profile(profile);
        if let Some(p) = producer {
            data = data.with_producer(p);
        }
        if let Some(obj) = self.objects.get_mut(&id) {
            obj.mine_data = Some(data);
            obj.producer_id = producer;

            obj.movement.max_speed = 0.0;
            obj.weapon = None;
            obj.secondary_weapon = None;
            obj.template_mut().add_kind_of(KindOf::DemoTrap);
            obj.apply_mine_innate_stealth();
        }
        self.mine_residual_places = self.mine_residual_places.saturating_add(1);
        self.queue_audio_event(
            AudioEventRequest::new(HostMineKind::DemoTrap.place_audio())
                .with_object(id)
                .with_position(position)
                .with_priority(150),
        );
        Some(id)
    }

    /// Place a residual timed demo charge (detonates after delay frames).
    pub fn place_timed_demo_charge(
        &mut self,
        team: Team,
        position: Vec3,
        producer: Option<ObjectId>,
        attach_to: Option<ObjectId>,
        delay_frames: Option<u32>,
    ) -> Option<ObjectId> {
        self.place_timed_demo_charge_with_effects(
            team,
            position,
            producer,
            attach_to,
            delay_frames,
            &mut Default::default(),
        )
    }

    pub(crate) fn place_timed_demo_charge_with_effects(
        &mut self,
        team: Team,
        position: Vec3,
        producer: Option<ObjectId>,
        attach_to: Option<ObjectId>,
        delay_frames: Option<u32>,
        effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> Option<ObjectId> {
        use crate::game_logic::host_mines::{
            retail_timed_charge_lifetime_frames, retail_timed_charge_template,
        };
        let producer_template =
            producer.and_then(|pid| self.objects.get(&pid).map(|o| o.template_name.clone()));
        let template_name = retail_timed_charge_template(producer_template.as_deref());
        let delay =
            delay_frames.or_else(|| Some(retail_timed_charge_lifetime_frames(template_name)));
        self.place_mine_kind_with_effects(
            crate::game_logic::host_mines::HostMineKind::TimedDemoCharge,
            template_name,
            team,
            position,
            producer,
            attach_to,
            delay,
            effects,
        )
    }

    /// Place a residual remote demo charge (no auto-timer; remote detonate only).
    /// Fail-closed: not full StickyBombUpdate attach bones / max-charge list.
    pub fn place_remote_demo_charge(
        &mut self,
        team: Team,
        position: Vec3,
        producer: Option<ObjectId>,
        attach_to: Option<ObjectId>,
    ) -> Option<ObjectId> {
        self.place_remote_demo_charge_with_effects(
            team,
            position,
            producer,
            attach_to,
            &mut Default::default(),
        )
    }

    pub(crate) fn place_remote_demo_charge_with_effects(
        &mut self,
        team: Team,
        position: Vec3,
        producer: Option<ObjectId>,
        attach_to: Option<ObjectId>,
        effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> Option<ObjectId> {
        use crate::game_logic::host_mines::BURTON_REMOTE_CHARGE_OBJECT;
        self.place_mine_kind_with_effects(
            crate::game_logic::host_mines::HostMineKind::RemoteDemoCharge,
            BURTON_REMOTE_CHARGE_OBJECT,
            team,
            position,
            producer,
            attach_to,
            None,
            effects,
        )
    }

    /// Detonate all residual remote demo charges planted by any of `producers`.
    /// Matches C++ SPECIAL_REMOTE_CHARGES no-target path (StickyBombUpdate::detonate).
    /// Returns the number of charges detonated.
    pub fn detonate_remote_demo_charges(&mut self, producers: &[ObjectId]) -> gamelogic::GameLogicResult<u32> {
        self.detonate_remote_demo_charges_with_effects(producers, &mut Default::default())
    }

    pub(crate) fn detonate_remote_demo_charges_with_effects(
        &mut self,
        producers: &[ObjectId],
        effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> gamelogic::GameLogicResult<u32> {
        use crate::game_logic::host_mines::HostMineDetonateReason;

        if producers.is_empty() {
            return Ok(0);
        }
        let producer_set: std::collections::HashSet<ObjectId> = producers.iter().copied().collect();
        let due: Vec<ObjectId> = self
            .objects
            .registered()
            .filter_map(|(id, obj)| {
                let producer = obj.remote_charge_planter()?;
                if !producer_set.contains(&producer) {
                    return None;
                }
                Some(*id)
            })
            .collect();

        let mut count = 0u32;
        for mine_id in due {
            if self.detonate_mine_internal_with_effects(
                mine_id,
                HostMineDetonateReason::Manual,
                None,
                effects,
            )? {
                count = count.saturating_add(1);
            }
        }
        if count > 0 {
            self.hero_abilities.record_remote_charge_detonate(count);
        }
        Ok(count)
    }

    /// C++ GenerateMinefieldBehavior::placeMineAt underwater / cliff / under-structure skip.
    pub(super) fn mine_spot_blocked(&self, pos: Vec3) -> bool {
        use crate::game_logic::host_mines::{LAND_MINE_GEOMETRY_RADIUS, mine_spot_under_structure};
        if let Some(terrain) = self.terrain.as_ref() {
            if self.is_underwater_at(pos) || terrain.is_cliff_at_world(pos) {
                return true;
            }
        }
        self.objects.registered_values().any(|obj| {
            obj.is_alive()
                && obj.is_kind_of(KindOf::Structure)
                && mine_spot_under_structure(
                    pos,
                    obj.get_position(),
                    obj.selection_radius
                        .max(obj.thing().geometry.radius)
                        .max(1.0),
                    LAND_MINE_GEOMETRY_RADIUS,
                )
        })
    }

    /// Place a named land-mine template (ChinaStandardMine / ChinaClusterMine / EMP).
    pub fn place_land_mine_named(
        &mut self,
        template_name: &str,
        team: Team,
        position: Vec3,
        producer: Option<ObjectId>,
    ) -> Option<ObjectId> {
        self.place_land_mine_named_with_effects(
            template_name,
            team,
            position,
            producer,
            &mut Default::default(),
        )
    }

    pub(crate) fn place_land_mine_named_with_effects(
        &mut self,
        template_name: &str,
        team: Team,
        position: Vec3,
        producer: Option<ObjectId>,
        effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> Option<ObjectId> {
        self.place_land_mine_named_with_interpreter_and_effects(
            template_name,
            team,
            position,
            producer,
            None,
            effects,
        )
    }

    pub(crate) fn place_land_mine_named_with_interpreter_and_effects(
        &mut self,
        template_name: &str,
        team: Team,
        position: Vec3,
        producer: Option<ObjectId>,
        interpreter: Option<&gamelogic::scripting::engine::ScriptEngine>,
        effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> Option<ObjectId> {
        self.place_mine_kind_with_interpreter_and_effects(
            crate::game_logic::host_mines::HostMineKind::LandMine,
            template_name,
            team,
            position,
            producer,
            None,
            None,
            interpreter,
            effects,
        )
    }

    /// C++ GenerateMinefieldBehavior upgradeImplementation + EMP swap update().
    pub(in super::super::super) fn apply_structure_minefield_upgrade_with_interpreter_and_effects(
        &mut self,
        object_id: ObjectId,
        upgrade: &str,
        interpreter: Option<&gamelogic::scripting::engine::ScriptEngine>,
        effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> u32 {
        use crate::game_logic::host_mines::{
            HostMineKind, china_mine_template_for_upgrade, is_china_emp_mines_upgrade,
            is_china_mines_upgrade, structure_minefield_positions_for_geom,
        };
        if !is_china_mines_upgrade(upgrade) {
            return 0;
        }
        let Some(obj) = self.objects.get(&object_id) else {
            return 0;
        };
        if !obj.is_alive() || !obj.is_kind_of(KindOf::Structure) {
            return 0;
        }
        let team = obj.team;
        let pos = obj.get_position();
        let major = obj
            .selection_radius
            .max(obj.thing().geometry.radius)
            .max(
                ((obj.thing().geometry.bounds_max.x - obj.thing().geometry.bounds_min.x).abs())
                    * 0.5,
            )
            .max(1.0);
        let minor = obj
            .thing()
            .geometry
            .radius
            .max(
                ((obj.thing().geometry.bounds_max.z - obj.thing().geometry.bounds_min.z).abs())
                    * 0.5,
            )
            .max(1.0);
        let is_box = (major - minor).abs() > 1.0;
        let template = china_mine_template_for_upgrade(upgrade);

        if is_china_emp_mines_upgrade(upgrade) {
            let old: Vec<ObjectId> = self
                .objects
                .registered()
                .filter_map(|(id, o)| {
                    let md = o.mine_data.as_ref()?;
                    if o.is_alive()
                        && matches!(md.kind, HostMineKind::LandMine)
                        && (md.producer_id == Some(object_id) || o.producer_id == Some(object_id))
                    {
                        Some(*id)
                    } else {
                        None
                    }
                })
                .collect();
            // C++ GenerateMinefieldBehavior::update (GenerateMinefieldBehavior.cpp:455-465):
            // TheGameLogic->destroyObject per old mine — DESTROYED latch, no onDie.
            for id in old {
                let _ = self.destroy_object_with_effects(id, effects);
                if let Some(o) = self.objects.get_mut(&id) {
                    if let Some(md) = o.mine_data.as_mut() {
                        md.detonated = true;
                    }
                }
            }
        } else {
            let already = self.objects.registered_values().any(|o| {
                o.is_alive()
                    && o.mine_data.as_ref().is_some_and(|md| {
                        matches!(md.kind, HostMineKind::LandMine)
                            && (md.producer_id == Some(object_id)
                                || o.producer_id == Some(object_id))
                    })
            });
            if already {
                return 0;
            }
        }

        let spots = structure_minefield_positions_for_geom(pos, major, minor, is_box);
        let mut placed = 0u32;
        for spot in spots {
            if self.mine_spot_blocked(spot) {
                continue;
            }
            if self
                .place_land_mine_named_with_interpreter_and_effects(
                    template,
                    team,
                    spot,
                    Some(object_id),
                    interpreter,
                    effects,
                )
                .is_some()
            {
                placed = placed.saturating_add(1);
            }
        }
        self.structure_minefield_placements =
            self.structure_minefield_placements.saturating_add(placed);
        placed
    }

    /// C++ LandMineInterface::setScootParms(start, dest) after placeMineAt.
    pub(super) fn apply_land_mine_scoot_with_effects(
        &mut self,
        mine_id: ObjectId,
        start: Vec3,
        dest: Vec3,
        effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) {
        let tree_move_frame = self.frame;
        let ground_y = self.terrain_height_at(dest).unwrap_or(dest.y);
        let Some(obj) = self.objects.get_mut(&mine_id) else {
            return;
        };
        let Some(md) = obj.mine_data.as_mut() else {
            return;
        };
        if !matches!(
            md.kind,
            crate::game_logic::host_mines::HostMineKind::LandMine
        ) {
            return;
        }
        let spawn = md.set_scoot_parms(start, dest, ground_y);
        let _ = obj;
        self.set_owned_object_position_with_effects(mine_id, spawn, tree_move_frame, effects);
    }

    /// C++ MinefieldBehavior::update scoot integrate.
    pub(super) fn update_land_mine_scoot_with_effects(
        &mut self,
        effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) {
        let tree_move_frame = self.frame;
        let ids: Vec<ObjectId> = self
            .objects
            .registered()
            .filter_map(|(id, o)| {
                let md = o.mine_data.as_ref()?;
                if md.is_scooting() && o.is_alive() {
                    Some(*id)
                } else {
                    None
                }
            })
            .collect();
        for id in ids {
            let pos = match self.objects.get(&id) {
                Some(o) => o.get_position(),
                None => continue,
            };
            let ground_y = self.terrain_height_at(pos).unwrap_or(pos.y);
            if let Some(obj) = self.objects.get_mut(&id) {
                if let Some(md) = obj.mine_data.as_mut() {
                    if let Some(next) = md.tick_scoot(pos, ground_y) {
                        let _ = obj;
                        self.set_owned_object_position_with_effects(id, next, tree_move_frame, effects);
                    }
                }
            }
        }
    }

    pub(in super::super::super) fn ensure_residual_mine_template(
        &mut self,
        template_name: &str,
        kind: crate::game_logic::host_mines::HostMineKind,
    ) {
        if self.templates.contains_key(template_name) {
            return;
        }
        let mut t = ThingTemplate::new(template_name);
        // Mines are not infantry/vehicles; residual treats them as Neutral objects
        // with mine_data driving behavior. Demo trap is structure-like but residual
        // does not require full structure production path.
        match kind {
            crate::game_logic::host_mines::HostMineKind::DemoTrap => {
                t.add_kind_of(KindOf::Structure)
                    .add_kind_of(KindOf::Selectable)
                    .add_kind_of(KindOf::DemoTrap)
                    .set_health(100.0)
                    .set_cost(400, 0);
            }
            crate::game_logic::host_mines::HostMineKind::LandMine
            | crate::game_logic::host_mines::HostMineKind::TimedDemoCharge
            | crate::game_logic::host_mines::HostMineKind::RemoteDemoCharge => {
                t.add_kind_of(KindOf::Mine).set_health(100.0).set_cost(0, 0);
            }
        }
        self.templates.insert(template_name.to_string(), t);
    }

    pub(in super::super::super) fn place_mine_kind(
        &mut self,
        kind: crate::game_logic::host_mines::HostMineKind,
        template_name: &str,
        team: Team,
        position: Vec3,
        producer: Option<ObjectId>,
        attach_to: Option<ObjectId>,
        delay_frames: Option<u32>,
    ) -> Option<ObjectId> {
        self.place_mine_kind_with_effects(
            kind,
            template_name,
            team,
            position,
            producer,
            attach_to,
            delay_frames,
            &mut Default::default(),
        )
    }

    pub(in super::super::super) fn place_mine_kind_with_effects(
        &mut self,
        kind: crate::game_logic::host_mines::HostMineKind,
        template_name: &str,
        team: Team,
        position: Vec3,
        producer: Option<ObjectId>,
        attach_to: Option<ObjectId>,
        delay_frames: Option<u32>,
        effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> Option<ObjectId> {
        self.place_mine_kind_with_interpreter_and_effects(
            kind,
            template_name,
            team,
            position,
            producer,
            attach_to,
            delay_frames,
            None,
            effects,
        )
    }

    pub(in super::super::super) fn place_mine_kind_with_interpreter_and_effects(
        &mut self,
        kind: crate::game_logic::host_mines::HostMineKind,
        template_name: &str,
        team: Team,
        position: Vec3,
        producer: Option<ObjectId>,
        attach_to: Option<ObjectId>,
        delay_frames: Option<u32>,
        interpreter: Option<&gamelogic::scripting::engine::ScriptEngine>,
        effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> Option<ObjectId> {
        let tree_move_frame = self.frame;
        use crate::game_logic::host_mines::{
            BURTON_UNIQUE_CHARGE_TARGETS, HostMineData, HostMineKind, can_place_remote_charge,
            can_place_timed_charge,
        };

        // C++ MaxSpecialObjects + UniqueSpecialObjectTargets residual (Burton C4).
        if matches!(
            kind,
            HostMineKind::TimedDemoCharge | HostMineKind::RemoteDemoCharge
        ) {
            if let Some(pid) = producer {
                let mut timed_n = 0u32;
                let mut remote_n = 0u32;
                for o in self.objects.registered_values() {
                    if !o.is_alive() {
                        continue;
                    }
                    let Some(md) = o.mine_data.as_ref() else {
                        continue;
                    };
                    if md.detonated {
                        continue;
                    }
                    // C++ counts the planter's live special objects by the
                    // mine_data bond; generic ProducerID is the charge's
                    // target now, not the planter.
                    let owned = md.producer_id == Some(pid);
                    if !owned {
                        continue;
                    }
                    match md.kind {
                        HostMineKind::TimedDemoCharge => timed_n = timed_n.saturating_add(1),
                        HostMineKind::RemoteDemoCharge => remote_n = remote_n.saturating_add(1),
                        _ => {}
                    }
                }
                match kind {
                    HostMineKind::TimedDemoCharge if !can_place_timed_charge(timed_n) => {
                        return None;
                    }
                    HostMineKind::RemoteDemoCharge if !can_place_remote_charge(remote_n) => {
                        return None;
                    }
                    _ => {}
                }
            }
            if BURTON_UNIQUE_CHARGE_TARGETS {
                if let Some(tid) = attach_to {
                    let dup = self.objects.registered_values().any(|o| {
                        o.is_alive()
                            && o.mine_data
                                .as_ref()
                                .map(|m| {
                                    !m.detonated
                                        && matches!(
                                            m.kind,
                                            HostMineKind::TimedDemoCharge
                                                | HostMineKind::RemoteDemoCharge
                                        )
                                        && m.attached_to == Some(tid)
                                })
                                .unwrap_or(false)
                    });
                    if dup {
                        return None;
                    }
                }
            }
        }

        // A bound Object template is the authored special object:
        // SpecialAbilityUpdate.cpp:1610-1647 newObject on the planter's team +
        // StickyBombUpdate.cpp:76-135 initStickyBomb handle pose, ProducerID
        // and the authored sound. An unbound timed name still manufactures
        // the residual fixture charge; remote charges require authored content.
        let is_sticky = matches!(
            kind,
            HostMineKind::TimedDemoCharge | HostMineKind::RemoteDemoCharge
        );
        if is_sticky
            && self
                .templates
                .get(template_name)
                .is_some_and(|template| template.receiving_object_definition.is_some())
        {
            return self.place_authored_sticky_charge(
                kind,
                template_name,
                team,
                position,
                producer,
                attach_to,
                interpreter,
                effects,
            );
        }
        if kind == HostMineKind::RemoteDemoCharge {
            return None;
        }
        self.ensure_residual_mine_template(template_name, kind);
        let id = self.create_object_with_script_effects(
            template_name,
            team,
            position,
            interpreter,
            effects,
        )?;

        // C++ GenerateMinefieldBehavior::placeMineAt (cpp:199-211): the factory
        // births the authored template; producer + LandMineInterface scoot are
        // the only placement mutations. Bound pads keep the MinefieldBehavior
        // admission made at create (charges/regen/thresh/scoot), their weapons,
        // and their authored KindOf set — no synthetic data or flag install.
        if kind == HostMineKind::LandMine
            && self
                .host_object(id)
                .is_some_and(|object| {
                    object.get_template().receiving_object_definition.is_some()
                })
        {
            let start =
                producer.and_then(|pid| self.host_object(pid).map(Object::get_position));
            let mine = self.objects.get_mut(&id)?;
            mine.producer_id = producer;
            if let Some(data) = mine
                .mine_data
                .as_mut()
                .filter(|data| data.kind == HostMineKind::LandMine)
            {
                data.producer_id = producer;
            }
            if let Some(start) = start {
                self.apply_land_mine_scoot_with_effects(id, start, position, effects);
            }
            self.mine_residual_places = self.mine_residual_places.saturating_add(1);
            return Some(id);
        }

        let mut data = match kind {
            crate::game_logic::host_mines::HostMineKind::LandMine => {
                HostMineData::land_mine_for_template(template_name)
            }
            crate::game_logic::host_mines::HostMineKind::DemoTrap => HostMineData::demo_trap(),
            crate::game_logic::host_mines::HostMineKind::TimedDemoCharge => {
                let mut d = HostMineData::timed_demo_charge(self.frame);
                if let Some(delay) = delay_frames {
                    d = d.with_lifetime_frames(self.frame, delay);
                }
                d
            }
            crate::game_logic::host_mines::HostMineKind::RemoteDemoCharge => {
                // Ruled out above: remote charges require authored content.
                return None;
            }
        };
        if let Some(p) = producer {
            data = data.with_producer(p);
        }
        if let Some(t) = attach_to {
            data = data.with_attach(t);
        }

        let planter_owner =
            producer.and_then(|pid| self.objects.get(&pid).and_then(|o| o.owner_player_id));
        let producer_pos =
            producer.and_then(|pid| self.objects.get(&pid).map(|o| o.get_position()));
        let dest_ground = self.terrain_height_at(position).unwrap_or(position.y);
        let sink_to_planter = kind == crate::game_logic::host_mines::HostMineKind::TimedDemoCharge;

        if let Some(obj) = self.objects.get_mut(&id) {
            obj.mine_data = Some(data);
            obj.producer_id = producer;
            // C++ SpecialAbilityUpdate::createSpecialObject setExperienceSink(planter).
            // Charges are untrainable; without a sink scoreTheKill drops the XP.
            if sink_to_planter {
                if let Some(p) = producer {
                    obj.set_experience_sink(Some(p));
                }
                if let Some(owner) = planter_owner {
                    obj.owner_player_id = Some(owner);
                }
            }

            // Mines/charges are not combat movers.
            obj.movement.max_speed = 0.0;
            obj.weapon = None;
            obj.secondary_weapon = None;
            match kind {
                crate::game_logic::host_mines::HostMineKind::DemoTrap => {
                    obj.template_mut().add_kind_of(KindOf::DemoTrap);
                }
                _ => {
                    obj.template_mut().add_kind_of(KindOf::Mine);
                }
            }
            obj.apply_mine_innate_stealth();
            if matches!(kind, crate::game_logic::host_mines::HostMineKind::LandMine) {
                if let Some(start) = producer_pos {
                    if let Some(md) = obj.mine_data.as_mut() {
                        let spawn = md.set_scoot_parms(start, position, dest_ground);
                        let _ = obj;
                        if !self.set_owned_object_position_with_effects(id, spawn, tree_move_frame, effects) {
                            return None;
                        }
                    }
                }
            }
        }

        self.mine_residual_places = self.mine_residual_places.saturating_add(1);
        self.queue_audio_event(
            AudioEventRequest::new(kind.place_audio())
                .with_object(id)
                .with_position(position)
                .with_priority(150),
        );
        if kind == crate::game_logic::host_mines::HostMineKind::TimedDemoCharge {
            // C++ StickyBombUpdate::initStickyBomb: getPerUnitSound("StickyBombCreated") at bomb pos.
            self.queue_resolved_per_unit_sound(
                id,
                crate::game_logic::host_mines::STICKY_BOMB_CREATED_AUDIO,
                false,
                true,
                None,
                155,
            );
        }
        Some(id)
    }

    /// C++ SpecialAbilityUpdate::createSpecialObject
    /// (SpecialAbilityUpdate.cpp:1610-1647) + StickyBombUpdate::initStickyBomb
    /// (StickyBombUpdate.cpp:76-135) for a bound timed/remote charge template.
    /// Births on the planter's exact team, applies the bomber pose and XP
    /// sink, stores the TARGET in generic ProducerID, keeps the planter bond
    /// in mine_data, lands an immobile target's charge at the bomber's feet,
    /// and queues the authored StickyBombCreated sound at the final pose.
    /// No synthetic data, weapon strip, stealth grant, or invented flags.
    fn place_authored_sticky_charge(
        &mut self,
        kind: crate::game_logic::host_mines::HostMineKind,
        template_name: &str,
        team: Team,
        position: Vec3,
        planter_id: Option<ObjectId>,
        target_id: Option<ObjectId>,
        interpreter: Option<&gamelogic::scripting::engine::ScriptEngine>,
        effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> Option<ObjectId> {
        use crate::game_logic::host_mines::{
            HostMineKind, STICKY_BOMB_CREATED_AUDIO, UNIT_BOMB_PING_FRAMES,
        };

        // The typed immutable offset projection doubles as the
        // StickyBombUpdate-authored gate: without it there is nothing to run.
        self.templates.get(template_name)?.sticky_bomb_offset_z()?;

        let participant_birth = |id| match id {
            Some(id) => {
                let object = self.objects.get(&id)?;
                Some(Some((
                    id,
                    std::sync::Arc::downgrade(&object.visual_draw_identity_token),
                )))
            }
            None => Some(None),
        };
        let planter_birth = participant_birth(planter_id)?;
        let target_birth = participant_birth(target_id)?;
        let planter = planter_id.and_then(|id| self.objects.get(&id));
        let owner_player_id =
            planter.and_then(|object| self.player_owner_for_host_object(object));
        let exact_team = planter
            .filter(|object| !object.team_instance_name.is_empty())
            .and_then(|object| {
                Some((
                    object.team,
                    object.team_instance_name.clone(),
                    object.team_instance_id?,
                ))
            });
        let birth_position = if planter_birth.is_some() {
            // C++ newObject(thing, Team*) returns before callers set any pose.
            Vec3::ZERO
        } else {
            position
        };
        let creation = match exact_team {
            Some((planter_team, name, identity)) => self
                .create_object_for_exact_team_with_birth_checked(
                    template_name,
                    planter_team,
                    owner_player_id,
                    &name,
                    identity,
                    birth_position,
                    interpreter,
                    effects,
                ),
            None => self.create_object_for_owner_or_team_with_birth_checked(
                template_name,
                team,
                owner_player_id,
                birth_position,
                interpreter,
                effects,
            ),
        };
        let (id, birth) = match creation {
            Ok(Some(receipt)) => receipt,
            Ok(None) => return None,
            Err(error) => {
                if let Some((created_id, created_birth)) = error.committed {
                    if self.spawn_identity_matches(created_id, created_birth.0, created_birth.1) {
                        let _ = self.destroy_object_with_effects(created_id, effects);
                    }
                }
                log::error!("Failed to create StickyBombUpdate charge: {}", error.error);
                return None;
            }
        };

        // Factory callbacks can replace a participant mid-call; continue only
        // with the births C++ initStickyBomb would see.
        let participants_match = |world: &Self| {
            world.spawn_identity_matches(id, birth.0, birth.1)
                && planter_birth
                    .as_ref()
                    .is_none_or(|(pid, birth)| world.object_birth_matches(*pid, birth))
                && target_birth
                    .as_ref()
                    .is_none_or(|(tid, birth)| world.object_birth_matches(*tid, birth))
        };
        let initialized = (|| {
            if !participants_match(self) {
                return None;
            }
            if let Some((pid, _)) = &planter_birth {
                // SpecialAbilityUpdate::createSpecialObject: setPosition,
                // setOrientation, then setExperienceSink after newObject.
                // Re-read the planter between pose operations because pose
                // callbacks can run.
                let planter_position = self.objects.get(pid)?.get_position();
                if !self.set_owned_object_position_with_effects(
                    id,
                    planter_position,
                    self.frame,
                    effects,
                ) {
                    return None;
                }
                if !participants_match(self) {
                    return None;
                }
                let angle = self.objects.get(pid)?.get_orientation();
                self.objects
                    .get_mut(&id)?
                    .set_orientation_with_partition_owner(angle, &mut self.partition_manager);
                if !participants_match(self) {
                    return None;
                }
            }
            let charge = self.objects.get_mut(&id)?;
            // Timed ping rounds the remaining native lifetime down to whole
            // seconds; remote pings one second out (StickyBombUpdate.cpp:83-104).
            let next_ping = match kind {
                HostMineKind::TimedDemoCharge => {
                    let deadline =
                        charge.native_lifecycle.as_ref()?.first_lifetime_deadline()?;
                    self.frame
                        .wrapping_add(deadline.wrapping_sub(self.frame) % UNIT_BOMB_PING_FRAMES)
                }
                HostMineKind::RemoteDemoCharge => {
                    self.frame.wrapping_add(UNIT_BOMB_PING_FRAMES)
                }
                _ => return None,
            };
            let data = charge.mine_data.as_mut().filter(|data| data.kind == kind)?;
            // StickyBombUpdate.cpp:76-135: generic ProducerID is the TARGET;
            // the planter bond lives in mine_data.
            data.producer_id = planter_id;
            data.attached_to = target_id;
            data.next_ping_frame = Some(next_ping);
            charge.set_experience_sink(planter_id);
            charge.producer_id = target_id;
            if let Some((tid, _)) = &target_birth {
                let offset = self
                    .objects
                    .get(&id)?
                    .get_template()
                    .sticky_bomb_offset_z()?;
                let target = self.objects.get(tid)?;
                // initStickyBomb: an exact IMMOBILE target keeps the charge at
                // the bomber's XZ snapped to terrain; otherwise it rides the
                // target at the authored OffsetZ.
                let pose = match (target.is_kind_of(KindOf::Immobile), planter_id) {
                    (true, Some(planter)) => {
                        let pos = self.objects.get(&planter)?.get_position();
                        Vec3::new(
                            pos.x,
                            self.terrain_ground_height_at(pos).unwrap_or(0.0),
                            pos.z,
                        )
                    }
                    _ => crate::game_logic::host_mines::sticky_vehicle_follow_pos(
                        target.get_position(),
                        offset,
                    ),
                };
                if !self.set_owned_object_position_with_effects(id, pose, self.frame, effects) {
                    return None;
                }
                if !participants_match(self) {
                    return None;
                }
                // getPerUnitSound("StickyBombCreated") at the bomb's final
                // position; unauthored slots stay silent.
                self.queue_resolved_per_unit_sound(
                    id,
                    STICKY_BOMB_CREATED_AUDIO,
                    false,
                    true,
                    None,
                    155,
                );
            }
            Some(())
        })()
        .is_some();
        if !initialized {
            if self.spawn_identity_matches(id, birth.0, birth.1) {
                let _ = self.destroy_object_with_effects(id, effects);
            }
            return None;
        }
        self.mine_residual_places = self.mine_residual_places.saturating_add(1);
        Some(id)
    }
}
