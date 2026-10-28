//! Residual muzzle/impact spawn for the direct `update_combat` finalizer.
//!
//! Wave-3 moved named FireFX playback to `world_combat::play_dispatch_fire_fx`.
//! That path fail-closes when the weapon has no FireFX list. This file owns the
//! host residual registry entries the combat tests observe for nameless weapons.
#![allow(unused_imports, non_snake_case)]
use super::super::*;

impl GameLogic {
    /// Spawn host residual muzzle/impact when visual dispatch will not.
    ///
    /// C++ `Weapon.cpp:904-939`: `FXList::doFXPos` only when `getFireFX` /
    /// `getProjectileDetonateFX` is non-null. `play_dispatch_fire_fx` mirrors
    /// that (`selected_fx_name` empty → return). `Weapon.cpp:889` still calls
    /// `Drawable::handleWeaponFireFX` (`Drawable.cpp:4216`) when FXList is null
    /// so recoil/barrel run; the host `MuzzleFlash`/`BulletImpact` registry is
    /// the residual stand-in for that client fire cue.
    ///
    /// Named FireFX stays on the dispatch path (do not double-spawn).
    /// Stealth gate matches `Weapon.cpp:911-919` (pretend handled, no FX).
    pub(in super::super) fn spawn_residual_muzzle_when_dispatch_has_no_fire_fx(
        &mut self,
        suppress_fire_fx: bool,
        fire_fx: &str,
        det_fx: &str,
        muzzle_pos: Vec3,
        impact_pos: Option<Vec3>,
        fire_frame: u32,
        attacker_id: ObjectId,
        fire_target: Option<ObjectId>,
    ) {
        if suppress_fire_fx || !fire_fx.is_empty() {
            return;
        }
        let _ = self.combat_particles.spawn_weapon_fire_fx_named(
            muzzle_pos,
            impact_pos,
            fire_frame,
            attacker_id,
            fire_target,
            fire_fx,
            det_fx,
        );
    }

    /// C++ FireSpreadUpdate.cpp:101 `ObjectCreationList::create(OCL_BurningEmbers)`.
    pub(crate) fn spawn_fire_spread_embers(
        &mut self,
        source_id: ObjectId,
        pos: Vec3,
        ocl_name: &str,
    ) -> Vec<ObjectId> {
        self.spawn_fire_spread_embers_with_effects(
            source_id,
            pos,
            ocl_name,
            &mut Default::default(),
        )
    }

    pub(crate) fn spawn_fire_spread_embers_with_effects(
        &mut self,
        source_id: ObjectId,
        pos: Vec3,
        ocl_name: &str,
        effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> Vec<ObjectId> {
        use crate::game_logic::host_fire_spread::TREE_OCL_EMBERS;
        let ocl = if ocl_name.is_empty() {
            TREE_OCL_EMBERS
        } else {
            ocl_name
        };
        let (team, yaw, vel) = self
            .objects
            .get(&source_id)
            .map(|o| (o.team, o.get_orientation(), o.movement.velocity))
            .unwrap_or((crate::game_logic::Team::Neutral, 0.0, Vec3::ZERO));
        let created = self.execute_parsed_weapon_ocl_at_with_effects(
            ocl,
            Some(source_id),
            team,
            crate::game_logic::VeterancyLevel::Rookie,
            yaw,
            vel,
            pos,
            effects,
        );
        let _ = self.combat_particles.spawn_weapon_fire_fx_named_ocl(
            pos, None, self.frame, source_id, None, "", "", ocl, "", 0.0, 0.0,
        );
        if created.is_empty() {
            const EMBER_TEMPLATE: &str = "BurningEmbers";
            if !self.templates.contains_key(EMBER_TEMPLATE) {
                let mut tpl = crate::game_logic::ThingTemplate::new(EMBER_TEMPLATE);
                tpl.set_health(1.0);
                self.templates.insert(EMBER_TEMPLATE.to_string(), tpl);
            }
            if let Some(id) =
                self.create_object_with_script_effects(EMBER_TEMPLATE, team, pos, None, effects)
            {
                if let Some(obj) = self.objects.get_mut(&id) {
                    obj.producer_id = Some(source_id);
                    if obj.native_lifecycle.is_none() {
                        obj.lifetime_update = Some(
                        crate::game_logic::host_lifetime_update::HostLifetimeUpdateData::from_delay_frames(
                            self.frame, 30,
                        ),
                    );
                    }
                }
                return vec![id];
            }
        }
        created
    }

    /// C++ FlammableUpdate::startBurningSound GenericFireMediumLoop.
    pub(crate) fn start_fire_spread_burning_sound(
        &mut self,
        object_id: ObjectId,
        pos: Vec3,
        sound_name: &str,
    ) {
        if sound_name.is_empty() {
            return;
        }
        self.queue_audio_event(
            AudioEventRequest::new(sound_name)
                .with_object(object_id)
                .with_position(pos)
                .with_priority(140)
                .looping(),
        );
    }

    /// C++ FlammableUpdate::stopBurningSound removeAudioEvent.
    pub(crate) fn stop_fire_spread_burning_sound(
        &mut self,
        object_id: ObjectId,
        pos: Vec3,
        sound_name: &str,
    ) {
        if sound_name.is_empty() {
            return;
        }
        self.queue_audio_event(
            AudioEventRequest::new(sound_name)
                .with_object(object_id)
                .with_position(pos)
                .with_priority(200),
        );
    }

    /// Select the exact admitted Object definition for this world. Admission
    /// is explicit: an admitted catalog miss stays empty instead of falling
    /// through to the application AssetManager.
    fn receiving_particle_sys_bones(
        &self,
        template_name: &str,
        condition_bits: u128,
    ) -> Vec<(String, String)> {
        if self.receiving_module_content.is_some() {
            let definition = self
                .receiving_object_definitions
                .as_ref()
                .and_then(|definitions| definitions.get(template_name))
                .map(std::sync::Arc::as_ref);
            crate::game_logic::combat_particles::particle_sys_bones_for_receiving_definition(
                definition,
                template_name,
                condition_bits,
            )
        } else {
            crate::game_logic::combat_particles::particle_sys_bones_for_template(
                template_name,
                condition_bits,
            )
        }
    }

    /// C++ `recalcBonesForClientParticleSystems` on create.
    pub(crate) fn spawn_particle_sys_bones_for_object(
        &mut self,
        object_id: ObjectId,
        particle_owner: &mut dyn crate::game_logic::combat_particles::ParticleBindingAccess,
    ) {
        let Some(obj) = self.objects.get(&object_id) else {
            return;
        };
        let bones = self.receiving_particle_sys_bones(&obj.template_name, obj.model_condition_bits);
        let pos = obj.get_position();
        let model = obj
            .thing()
            .template
            .model_name_for_conditions(obj.model_condition_bits);
        let scale = obj.thing().template.asset_scale;
        let yaw = obj.get_orientation();
        let frame = self.frame;
        let pose = crate::game_logic::combat_particles::BodyAutoParticlePose::with_visibility(
            &model,
            scale,
            yaw,
            self.host_fx_object_is_shrouded(object_id.0),
        );
        self.combat_particles.sync_particle_sys_bones(
            frame,
            object_id,
            pos,
            &bones,
            pose,
            particle_owner,
        );
    }

    /// Keep ParticleSysBone and damaged-body fire/smoke attached to live objects.
    pub(crate) fn sync_live_state_particles(
        &mut self,
        particle_owner: &mut dyn crate::game_logic::combat_particles::ParticleBindingAccess,
    ) {
        let frame = self.frame;
        let snapshots: Vec<_> = self
            .objects
            .registered()
            .map(|(id, obj)| {
                (
                    *id,
                    obj.get_position(),
                    obj.template_name.clone(),
                    obj.thing()
                        .template
                        .model_name_for_conditions(obj.model_condition_bits),
                    obj.thing().template.asset_scale,
                    obj.get_orientation(),
                    obj.model_condition_bits,
                )
            })
            .collect();
        for (id, pos, template, model, scale, yaw, bits) in snapshots {
            if gamelogic::object::update::leftover_template_uses_animated_particle_sys_bones(
                &template,
            ) {
                gamelogic::object::update::tick_live_host_animated_particle_sys_bones(id.0);
            }
            let bones = self.receiving_particle_sys_bones(&template, bits);
            let pose = crate::game_logic::combat_particles::BodyAutoParticlePose::with_visibility(
                &model,
                scale,
                yaw,
                self.host_fx_object_is_shrouded(id.0),
            );
            self.combat_particles.sync_particle_sys_bones(
                frame,
                id,
                pos,
                &bones,
                pose,
                particle_owner,
            );
            // ActiveBody replaces its auto particles at its own visual and
            // AFLAME callbacks, not by reconciling Body state each frame.
            self.combat_particles
                .follow_attached_body_particles(id, pos, yaw);
        }
    }
}

#[cfg(test)]
mod particle_sys_bone_owner_tests {
    use super::*;

    fn world_with_receiving_template(particle: &str) -> GameLogic {
        let source = format!(
            "Object SharedFxOwner\nDraw = W3DModelDraw OwnerDraw\nDefaultConditionState\nModel = SharedFxOwner.w3d\nParticleSysBone = fire {particle}\nEnd\nEnd\nEnd"
        );
        let content = crate::assets::ReceivingObjectContent::from_sources([(
            "owner-test.ini".to_owned(),
            source,
        )])
        .unwrap();
        let (modules, definitions) = content.into_parts();
        let mut world = GameLogic::new();
        world.receiving_module_content = Some(modules);
        world.receiving_object_definitions = Some(definitions);
        world
    }

    fn world_with_empty_receiving_template(name: &str) -> GameLogic {
        let content = crate::assets::ReceivingObjectContent::from_sources([(
            "owner-test-empty.ini".to_owned(),
            format!("Object {name}\nEnd"),
        )])
        .unwrap();
        let (modules, definitions) = content.into_parts();
        let mut world = GameLogic::new();
        world.receiving_module_content = Some(modules);
        world.receiving_object_definitions = Some(definitions);
        world
    }

    #[test]
    fn live_bone_lookup_uses_each_receiving_catalog_and_keeps_admitted_misses_empty() {
        let a = world_with_receiving_template("WorldAFire");
        let b = world_with_receiving_template("WorldBFire");
        assert_eq!(
            a.receiving_particle_sys_bones("SharedFxOwner", 0),
            vec![("fire".to_owned(), "WorldAFire".to_owned())]
        );
        assert_eq!(
            b.receiving_particle_sys_bones("SharedFxOwner", 0),
            vec![("fire".to_owned(), "WorldBFire".to_owned())]
        );
        assert!(
            a.receiving_particle_sys_bones("MissingInReceivingCatalog", 0)
                .is_empty()
        );
        let empty = world_with_empty_receiving_template("RepairVehiclesInAreaMarker");
        assert!(
            empty
                .receiving_particle_sys_bones("RepairVehiclesInAreaMarker", 0)
                .is_empty(),
            "admitted empty definitions do not select legacy global or name fallback"
        );
    }
}
