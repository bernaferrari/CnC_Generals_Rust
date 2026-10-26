/// Explicit synchronous retirement, invoked only after constructor/module
/// callbacks release their borrows. Implementations must never retain service
/// guards across the callback walk.
pub trait NativeDrawableRetirement {
    fn retire(
        &mut self,
        owner: &mut NativeDrawableOwner<'_>,
        selected: Option<DrawableId>,
    ) -> GameClientResult<()>;
}
impl NativeDrawableRetirement for crate::effects::particle_manager::ParticleSystemManager {
    fn retire(
        &mut self,
        owner: &mut NativeDrawableOwner<'_>,
        selected: Option<DrawableId>,
    ) -> GameClientResult<()> {
        match selected {
            Some(id) => owner.try_remove_for_map_attempt(id, Some(self)),
            None => owner.try_clear_for_map_attempt(Some(self)),
        }
    }
}
/// Existing application particle boundary, kept explicit until particle ownership
/// migrates. Acquires the service ONLY during retirement, never during callbacks.
pub struct NativeParticleRetirementService;
impl NativeDrawableRetirement for NativeParticleRetirementService {
    fn retire(
        &mut self,
        owner: &mut NativeDrawableOwner<'_>,
        selected: Option<DrawableId>,
    ) -> GameClientResult<()> {
        if !owner
            .drawables
            .native_particle_retirement_required(selected)?
        {
            return match selected {
                Some(id) => owner.try_remove_for_map_attempt(id, None),
                None => owner.try_clear_for_map_attempt(None),
            };
        }
        owner
            .drawables
            .retire_native_particles_with_services(selected, owner.particles)?;
        // W3DTankDraw/TruckDraw destructors may discover the existing particle
        // manager bridge. Its guard MUST be released before arbitrary module
        // Drop or ambient-stop callbacks execute. Native IDs are now retired,
        // so the ordinary no-manager preflight is authoritative again.
        match selected {
            Some(id) => owner.try_remove_for_map_attempt(id, None),
            None => owner.try_clear_for_map_attempt(None),
        }
    }
}

// One real CPU Drawable constructor shared by factory admission and presentation.
// Included in game_client so the existing owners and module kernels stay private.
/// Observed semantic constructor guards and the existing late presentation
/// startup are distinct policies until Main supplies every factory birth.
enum NativeConstructorAmbientPolicy {
    Observed(NativeDrawableBirthContext),
    PresentationStartup,
}

pub struct NativeDrawableOwner<'a> {
    drawables: &'a mut drawable_world::DrawableState,
    particles: &'a mut crate::effects::particle_manager::ParticleSystemManager,
}
impl NativeDrawableWorld {
    pub fn borrow_native(&mut self) -> NativeDrawableOwner<'_> {
        NativeDrawableOwner {
            drawables: &mut self.state,
            particles: &mut self.particles,
        }
    }
}
impl NativeDrawableOwner<'_> {
    /// Short loan of this same canonical owner for one synchronous render phase.
    pub fn reborrow(&mut self) -> NativeDrawableOwner<'_> {
        NativeDrawableOwner {
            drawables: &mut *self.drawables,
            particles: &mut *self.particles,
        }
    }
    pub fn particle_systems(&self) -> &crate::effects::particle_manager::ParticleSystemManager {
        self.particles
    }
    pub fn particle_systems_mut(
        &mut self,
    ) -> &mut crate::effects::particle_manager::ParticleSystemManager {
        self.particles
    }

    /// Query current-client W3D bones on this exact receiving Drawable owner.
    /// The short borrow ends before FX nugget callbacks can reenter the owner.
    pub fn current_client_bone_transforms(
        &self,
        object_id: u32,
        bone_name: &str,
        start: i32,
        max_bones: usize,
    ) -> Vec<glam::Mat4> {
        if max_bones == 0 || bone_name.is_empty() {
            return Vec::new();
        }
        let Some(drawable_id) = self.drawables.get_drawable_for_object(object_id) else {
            return Vec::new();
        };
        let Some(drawable) = self.drawables.find_drawable_by_id(drawable_id) else {
            return Vec::new();
        };
        let Some(basic) = drawable
            .as_any()
            .downcast_ref::<crate::drawable::BasicDrawable>()
        else {
            return Vec::new();
        };
        let mut positions = vec![crate::drawable::Vector3::zero(); max_bones];
        let mut transforms = vec![crate::drawable::Matrix4::identity(); max_bones];
        let count = basic.get_current_client_bone_positions(
            bone_name,
            start,
            &mut positions,
            &mut transforms,
        );
        transforms
            .into_iter()
            .take(count.max(0) as usize)
            .map(|transform| transform.to_glam())
            .collect()
    }

    /// Missing runtime binding does not imply that its actual CPU actor is
    /// absent (for example, the current Client load-postprocess clears keys).
    /// A broken inverse map also cannot masquerade as prebirth absence.
    pub fn has_object_drawable(&self, object: u32) -> bool {
        self.drawables.drawable_object_map.contains_key(&object)
            || self
                .drawables
                .drawable_map
                .values()
                .any(|drawable| drawable.get_object_id() == Some(object))
    }
    pub fn binding_key(
        &self,
        epoch: u64,
        object: u32,
    ) -> Option<PresentationDirectDrawableBindingKey> {
        self.drawables
            .presentation_direct_drawable_state(epoch, object)
            .map(|state| state.binding_key)
    }
    /// Immediate original Body observation on the exact receiving native lifetime.
    /// Snapshot restamping does not call this operation.
    pub fn body_state_changed_with_audio(
        &mut self,
        key: PresentationDirectDrawableBindingKey,
        state: gamelogic::common::types::BodyDamageType,
        loading_map: bool,
        time_of_day: crate::system::TimeOfDay,
        audio: Option<&mut dyn crate::drawable::drawable::NativeAmbientAudio>,
        locality: &game_engine::common::audio::AudioSubmissionLocality,
    ) -> GameClientResult<()> {
        if self.binding_key(key.host_epoch, key.object_id) != Some(key) {
            return Err(GameClientError::InvalidOperation(
                "native Body visual receiver has a stale binding".into(),
            ));
        }
        let receiver = self
            .drawables
            .drawable_map
            .get_mut(&key.drawable_id)
            .and_then(|drawable| drawable.as_any_mut().downcast_mut::<BasicDrawable>())
            .ok_or_else(|| {
                GameClientError::InvalidOperation(
                    "native Body visual receiver is not its BasicDrawable".into(),
                )
            })?;
        receiver
            .preflight_native_ambient()
            .map_err(GameClientError::InvalidOperation)?;
        let audio = if loading_map {
            None
        } else {
            Some(audio.ok_or_else(|| {
                GameClientError::InvalidOperation(
                    "native Body ambient transition requires its captured audio service".into(),
                )
            })?)
        };
        // Flags may invoke Draw modules. No AudioManager guard spans that walk.
        receiver.react_to_body_damage_state_change(state);
        if let Some(audio) = audio {
            audio
                .restart_ambient(receiver, state, time_of_day, locality)
                .map_err(GameClientError::InvalidOperation)?;
        }
        Ok(())
    }

    /// One factory birth, independent of GPU/presentation residency. Never prunes
    /// another actor. The caller supplies the original pre-placement pose.
    pub fn admit_semantic_drawable_with_content(
        &mut self,
        entry: PresentationDrawableSync,
        birth: NativeDrawableBirthContext,
        sync_ms: u32,
        content: &mut game_engine::common::thing::module_factory::ModuleFactory,
        policy: ReceivingDrawablePolicy,
        retirement: &mut dyn NativeDrawableRetirement,
        mut observe: impl FnMut(PresentationDrawableMutation<'_>) -> GameClientResult<()>,
    ) -> GameClientResult<PresentationDirectDrawableBindingKey> {
        if entry.module_admission.authored_template().is_none() {
            return Err(GameClientError::InvalidOperation(
                "semantic birth requires its admitted receiving template".into(),
            ));
        }
        self.admit_fresh_drawable(
            &entry,
            sync_ms,
            Some(content),
            Some(policy),
            true,
            NativeConstructorAmbientPolicy::Observed(birth),
            None,
            retirement,
            &mut observe,
        )
    }

    fn admit_fresh_drawable(
        &mut self,
        e: &PresentationDrawableSync,
        sync_ms: u32,
        mut content: Option<&mut game_engine::common::thing::module_factory::ModuleFactory>,
        policy: Option<ReceivingDrawablePolicy>,
        semantic_lifetime: bool,
        ambient: NativeConstructorAmbientPolicy,
        inherited_material_opacity: Option<f32>,
        retirement: &mut dyn NativeDrawableRetirement,
        observe: &mut impl FnMut(PresentationDrawableMutation<'_>) -> GameClientResult<()>,
    ) -> GameClientResult<PresentationDirectDrawableBindingKey> {
        self.particles
            .admit_effective_definitions()
            .map_err(|error| GameClientError::SubsystemError(error.to_string()))?;
        let receiving_tod = if e.module_admission.authored_template().is_some() {
            Some(
                policy
                    .and_then(|policy| policy.time_of_day)
                    .ok_or_else(|| {
                        GameClientError::InvalidOperation(
                            "authored Drawable requires receiving time of day".into(),
                        )
                    })?,
            )
        } else {
            None
        };
        if self
            .drawables
            .drawable_object_map
            .contains_key(&e.object_id)
        {
            return Err(GameClientError::InvalidOperation(
                "fresh Drawable birth already has an Object association".into(),
            ));
        }
        if e.receiving_science_player.is_none()
            && GameClient::fresh_drawable_requires_science(e, policy)
        {
            return Err(GameClientError::InvalidOperation(
                "authored science Drawable requires its receiving local-player observation".into(),
            ));
        }
        let visual_template_name = GameClient::presentation_visual_template_name(e).to_string();
        let id = self.drawables.alloc_drawable_id();
        let mut drawable = BasicDrawable::new(id);
        drawable.admit_receiving_time_of_day(receiving_tod);
        if let Some(opacity) = inherited_material_opacity {
            drawable.set_second_material_pass_opacity(opacity);
        }
        if !visual_template_name.is_empty() {
            drawable.initialize_native_constructor_template(
                Some(visual_template_name.clone()),
                match &e.module_admission {
                    PresentationModuleAdmission::Authored(template) => Some(Arc::clone(template)),
                    PresentationModuleAdmission::Compatibility => None,
                },
            );
        }
        // Drawable.cpp338-344: registration precedes all module constructors.
        // Object binding is absent throughout construction and onObjectCreated.
        self.drawables.admit_native_drawable(id, Box::new(drawable));
        let result = (|| {
            let drawable = self
                .drawables
                .drawable_map
                .get_mut(&id)
                .unwrap()
                .as_any_mut()
                .downcast_mut::<BasicDrawable>()
                .unwrap();
            observe(PresentationDrawableMutation::Creating {
                source: e,
                initial_transform: drawable.native_base_transform(),
                sync_ms,
            })?;
            if let Some(admitted) = e.module_admission.authored_template() {
                let keys = content
                    .as_deref_mut()
                    .and_then(|owner| owner.receiving_name_keys_mut())
                    .ok_or_else(|| {
                        GameClientError::InvalidOperation(
                            "authored Drawable requires its receiving name-key owner".into(),
                        )
                    })?;
                let policy = policy.ok_or_else(|| {
                    GameClientError::InvalidOperation(
                        "authored Drawable requires receiving LOD policy".into(),
                    )
                })?;
                let modules = GameClient::create_receiving_modules_from_plan(
                    admitted,
                    keys,
                    policy,
                    (INVALID_ID, e.draw_logic_frame),
                )?;
                // Inter-module resolution sees every installed module, as CPP.
                for module in modules {
                    drawable.add_draw_module(module);
                }
                for slot in drawable.get_draw_modules_mut() {
                    if let Some(module) = slot.logic_module_mut() {
                        gamelogic::object::drawable::create_native_draw_module_unbound(
                            module,
                            admitted.per_unit_sound_event("TruckLandingSound"),
                            admitted.per_unit_sound_event("TruckPowerslideSound"),
                        );
                    }
                }
            } else {
                GameClient::attach_presentation_specialized_draw_modules(drawable, e);
            }
            match ambient {
                NativeConstructorAmbientPolicy::Observed(birth) => {
                    if let Some(tod) = receiving_tod {
                        drawable.finish_native_constructor_ambient_at(
                            birth.loading_map,
                            birth.in_load_game,
                            tod,
                        );
                    } else {
                        drawable.finish_native_constructor_ambient(
                            birth.loading_map,
                            birth.in_load_game,
                        );
                    }
                }
                NativeConstructorAmbientPolicy::PresentationStartup => {
                    // Keep the existing late presentation startup behavior.
                    // This is not a claim about an unobserved factory phase.
                    if let Some(tod) = receiving_tod {
                        drawable.start_ambient_sound_at(false, tod);
                    } else {
                        drawable.start_ambient_sound(false);
                    }
                }
            }
            // GameLogic::sendObjectCreated binds only after newDrawable returns.
            drawable.set_object_id(Some(e.object_id));
            for slot in drawable.get_draw_modules_mut() {
                if let Some(module) = slot.logic_module_mut() {
                    gamelogic::object::drawable::bind_native_draw_module_owner(
                        module,
                        e.object_id,
                        e.draw_indicator_color,
                    );
                    if let Some(science) = module.as_any_mut().downcast_mut::<W3DScienceModelDraw>()
                    {
                        science
                            .bind_receiving_science_player(e.receiving_science_player.as_deref())
                            .map_err(|error| {
                                GameClientError::InvalidOperation(format!(
                                    "receiving science binding: {error:?}"
                                ))
                            })?;
                    }
                    if let Some(beacon) = module
                        .as_any_mut()
                        .downcast_mut::<BeaconClientUpdateModule>()
                    {
                        beacon.bind_owner_id(e.object_id);
                    } else if let Some(sway) =
                        module.as_any_mut().downcast_mut::<SwayClientUpdateModule>()
                    {
                        sway.bind_owner_id(e.object_id);
                    } else if let Some(animated) = module
                        .as_any_mut()
                        .downcast_mut::<AnimatedParticleSysBoneClientUpdateModule>(
                    ) {
                        animated.bind_owner_id(e.object_id);
                    }
                }
            }
            drawable.set_position(Vector3::new(e.position[0], e.position[1], e.position[2]));
            observe(PresentationDrawableMutation::TransformInstalled {
                source: e,
                transform: drawable.native_base_transform(),
            })?;
            drawable.set_instance_transform(
                Matrix4::rotation_y(e.orientation)
                    .mul(&Matrix4::rotation_z(e.float_yaw))
                    .mul(&Matrix4::rotation_x(e.float_pitch)),
            );
            observe(PresentationDrawableMutation::TransformInstalled {
                source: e,
                transform: drawable.native_base_transform(),
            })?;
            GameClient::stamp_presentation_object_residual(drawable, e, !semantic_lifetime);
            let flags = drawable.get_model_condition_flags();
            let bits = (0..128.min(flags.get_bit_names().len()))
                .fold(0u128, |bits, i| bits | ((flags.test(i) as u128) << i));
            observe(PresentationDrawableMutation::ConditionsMarked { source: e, bits })?;
            let native_laser = GameClient::owned_laser_count(drawable) == 1;
            GameClient::bind_owned_laser_module(drawable, id);
            if native_laser {
                self.drawables.owned_laser_order.insert(0, id);
            }
            self.drawables.drawable_object_map.insert(e.object_id, id);
            let binding_key = PresentationDirectDrawableBindingKey {
                host_epoch: e.host_epoch,
                object_id: e.object_id,
                drawable_id: id,
                binding_generation: self.drawables.alloc_binding_generation(),
            };
            self.drawables.presentation_direct_drawable_bindings.insert(
                id,
                PresentationDirectDrawableBinding {
                    semantic_lifetime,
                    binding_key,
                    visual_template_name,
                },
            );
            Ok(binding_key)
        })();
        if let Err(constructor) = result.as_ref() {
            // The selected receiver still exists during retirement preflight.
            // Missing required services must never turn into primitive unlink.
            let cleanup = retirement.retire(self, Some(id));
            let renderer_cleanup = if !self.drawables.drawable_map.contains_key(&id) {
                observe(PresentationDrawableMutation::Destroyed {
                    host_epoch: e.host_epoch,
                    object_id: e.object_id,
                })
            } else {
                Ok(())
            };
            if cleanup.is_err() || renderer_cleanup.is_err() {
                return Err(GameClientError::SubsystemError(format!(
                    "Drawable constructor failed: {constructor}; receiver retirement: {}; renderer retirement: {}",
                    cleanup
                        .err()
                        .map(|e| e.to_string())
                        .unwrap_or_else(|| "completed".into()),
                    renderer_cleanup
                        .err()
                        .map(|e| e.to_string())
                        .unwrap_or_else(|| "completed".into()),
                )));
            }
        }
        result
    }
}

#[cfg(test)]
mod semantic_admission_tests {
    use super::*;
    use game_engine::common::ini::INILoadType;
    use game_engine::common::name_key_generator::NameKeyNamespace;
    use game_engine::common::thing::module::StaticGameLodLevel;
    use game_engine::common::thing::module_factory::ModuleFactory;
    use game_engine::common::thing::thing_template::{ObjectIniFields, ReceivingTemplateKind};

    fn content(
        rate: f32,
    ) -> (
        ModuleFactory,
        Arc<game_engine::common::thing::thing_template::AdmittedReceivingTemplate>,
    ) {
        let mut factory = ModuleFactory::new_receiving(NameKeyNamespace::new());
        gamelogic::contain_module_overrides::register_native_module_overrides(&mut factory)
            .unwrap();
        let source = format!("Draw = W3DTankDraw TankTag\nTreadAnimationRate = {rate}\nEnd\nEnd");
        let lines = source.lines().collect::<Vec<_>>();
        let fields = ObjectIniFields::capture_body(&lines, 0).0;
        let plan = ThingTemplate::new()
            .admit_named_receiving_module_source(
                "SemanticTank",
                &fields,
                INILoadType::Overwrite,
                ReceivingTemplateKind::Object,
                &mut factory,
            )
            .unwrap();
        (factory, Arc::new(plan))
    }
    fn policy() -> ReceivingDrawablePolicy {
        ReceivingDrawablePolicy {
            time_of_day: Some(crate::system::TimeOfDay::Afternoon),
            use_draw_module_lod: true,
            static_game_lod: StaticGameLodLevel::High,
            use_tree_sway: true,
        }
    }
    fn entry(
        id: u32,
        plan: Arc<game_engine::common::thing::thing_template::AdmittedReceivingTemplate>,
    ) -> PresentationDrawableSync {
        let mut entry = super::tests::presentation_drawable_sync_for_test(
            id,
            5,
            "SemanticTank",
            false,
            false,
            [0.0; 3],
            0.0,
        );
        entry.module_admission = PresentationModuleAdmission::Authored(plan);
        entry
    }
    fn rate(client: &mut GameClient, id: DrawableId) -> f32 {
        client
            .drawables
            .drawable_map
            .get_mut(&id)
            .unwrap()
            .as_any_mut()
            .downcast_mut::<BasicDrawable>()
            .unwrap()
            .get_draw_modules_mut()[0]
            .logic_module_mut()
            .unwrap()
            .get_module_data()
            .as_any()
            .downcast_ref::<W3DTankDrawModuleData>()
            .unwrap()
            .tread_animation_rate
    }
    #[test]
    fn semantic_cpu_birth_preserves_same_id_peers_order_and_nonresident_lifetime() {
        let (mut catalog, plan) = content(2.0);
        let (mut peer_catalog, peer_plan) = content(7.0);
        let mut client = GameClient::new().unwrap();
        let mut peer = GameClient::new().unwrap();
        let child = client
            .split_frame_snow_and_drawable_owner()
            .2
            .admit_semantic_drawable_with_content(
                entry(77, Arc::clone(&plan)),
                NativeDrawableBirthContext {
                    loading_map: false,
                    in_load_game: false,
                },
                0,
                &mut catalog,
                policy(),
                &mut NativeParticleRetirementService,
                |_| Ok(()),
            )
            .unwrap();
        let parent = client
            .split_frame_snow_and_drawable_owner()
            .2
            .admit_semantic_drawable_with_content(
                entry(42, Arc::clone(&plan)),
                NativeDrawableBirthContext {
                    loading_map: false,
                    in_load_game: false,
                },
                0,
                &mut catalog,
                policy(),
                &mut NativeParticleRetirementService,
                |_| Ok(()),
            )
            .unwrap();
        let peer_parent = peer
            .split_frame_snow_and_drawable_owner()
            .2
            .admit_semantic_drawable_with_content(
                entry(42, peer_plan),
                NativeDrawableBirthContext {
                    loading_map: false,
                    in_load_game: false,
                },
                0,
                &mut peer_catalog,
                policy(),
                &mut NativeParticleRetirementService,
                |_| Ok(()),
            )
            .unwrap();
        assert_ne!(parent.drawable_id.0, parent.object_id);
        assert!((rate(&mut client, parent.drawable_id) - 2.0 / 30.0).abs() < 0.00001);
        assert!((rate(&mut peer, peer_parent.drawable_id) - 7.0 / 30.0).abs() < 0.00001);
        assert_eq!(
            client.drawables.native_drawable_order,
            vec![parent.drawable_id, child.drawable_id]
        );
        assert_eq!(
            peer.drawables.native_drawable_order,
            vec![peer_parent.drawable_id]
        );
        assert_eq!(client.get_drawable_for_object(77), Some(child.drawable_id));
        assert_eq!(client.get_drawable_for_object(42), Some(parent.drawable_id));
        assert_eq!(
            client
                .sync_presentation_drawables(std::iter::empty())
                .unwrap(),
            (0, 0, 0)
        );
        assert_eq!(client.get_drawable_for_object(42), Some(parent.drawable_id));
        assert_eq!(
            client
                .sync_presentation_drawables_observed_with_content(
                    [entry(42, plan)],
                    0,
                    &mut catalog,
                    policy(),
                    |_| {},
                )
                .unwrap(),
            (0, 1, 0)
        );
        assert_eq!(client.get_drawable_for_object(77), Some(child.drawable_id));
        assert_eq!(
            peer.get_drawable_for_object(42),
            Some(peer_parent.drawable_id)
        );
    }
    #[test]
    fn detached_cpu_owner_admits_without_client_or_gpu_and_rejects_duplicate_binding() {
        let (mut catalog, plan) = content(2.0);
        let mut world = NativeDrawableWorld::new();
        let key = world
            .borrow_native()
            .admit_semantic_drawable_with_content(
                entry(42, Arc::clone(&plan)),
                NativeDrawableBirthContext {
                    loading_map: false,
                    in_load_game: false,
                },
                0,
                &mut catalog,
                policy(),
                &mut NativeParticleRetirementService,
                |_| Ok(()),
            )
            .unwrap();
        assert_eq!(world.borrow_native().binding_key(5, 42), Some(key));
        assert!(
            world
                .borrow_native()
                .admit_semantic_drawable_with_content(
                    entry(42, plan),
                    NativeDrawableBirthContext {
                        loading_map: false,
                        in_load_game: false
                    },
                    0,
                    &mut catalog,
                    policy(),
                    &mut NativeParticleRetirementService,
                    |_| Ok(()),
                )
                .is_err()
        );
        assert_eq!(world.native_drawable_order, vec![key.drawable_id]);
        assert_eq!(world.drawable_map.len(), 1);
    }
    #[test]
    fn failed_constructor_removes_registered_receiver_without_pruning_peer() {
        let (mut catalog, plan) = content(2.0);
        let mut world = NativeDrawableWorld::new();
        let peer = world
            .borrow_native()
            .admit_semantic_drawable_with_content(
                entry(77, Arc::clone(&plan)),
                NativeDrawableBirthContext {
                    loading_map: false,
                    in_load_game: false,
                },
                0,
                &mut catalog,
                policy(),
                &mut NativeParticleRetirementService,
                |_| Ok(()),
            )
            .unwrap();
        let mut wrong_content = ModuleFactory::new();
        let mut phases = Vec::new();
        assert!(
            world
                .borrow_native()
                .admit_semantic_drawable_with_content(
                    entry(42, plan),
                    NativeDrawableBirthContext {
                        loading_map: false,
                        in_load_game: false
                    },
                    0,
                    &mut wrong_content,
                    policy(),
                    &mut NativeParticleRetirementService,
                    |event| {
                        match event {
                            PresentationDrawableMutation::Creating { .. } => {
                                phases.push("registered-before-construction")
                            }
                            PresentationDrawableMutation::Destroyed { .. } => {
                                phases.push("retired-failed-receiver")
                            }
                            _ => {}
                        };
                        Ok(())
                    },
                )
                .is_err()
        );
        assert_eq!(
            phases,
            vec!["registered-before-construction", "retired-failed-receiver"]
        );
        assert_eq!(world.native_drawable_order, vec![peer.drawable_id]);
        assert_eq!(world.drawable_map.len(), 1);
        assert_eq!(world.borrow_native().binding_key(5, 42), None);
    }
    #[test]
    fn constructor_error_cannot_primitive_unlink_when_retirement_rejects() {
        struct Reject;
        impl NativeDrawableRetirement for Reject {
            fn retire(
                &mut self,
                _: &mut NativeDrawableOwner<'_>,
                _: Option<DrawableId>,
            ) -> GameClientResult<()> {
                Err(GameClientError::SubsystemError(
                    "receiving retirement unavailable".into(),
                ))
            }
        }
        let (mut catalog, plan) = content(2.0);
        let mut world = NativeDrawableWorld::new();
        let peer = world
            .borrow_native()
            .admit_semantic_drawable_with_content(
                entry(77, Arc::clone(&plan)),
                NativeDrawableBirthContext {
                    loading_map: false,
                    in_load_game: false,
                },
                0,
                &mut catalog,
                policy(),
                &mut NativeParticleRetirementService,
                |_| Ok(()),
            )
            .unwrap();
        let mut destroyed = false;
        let result = world.borrow_native().admit_semantic_drawable_with_content(
            entry(42, plan),
            NativeDrawableBirthContext {
                loading_map: false,
                in_load_game: false,
            },
            0,
            &mut catalog,
            policy(),
            &mut Reject,
            |event| {
                if matches!(event, PresentationDrawableMutation::Destroyed { .. }) {
                    destroyed = true;
                }
                Err(GameClientError::SubsystemError(
                    "actual checked observer rejected Creating".into(),
                ))
            },
        );
        assert!(result.is_err());
        assert!(
            !destroyed,
            "renderer retirement cannot be announced before real receiver retirement"
        );
        assert_eq!(
            world.drawable_map.len(),
            2,
            "failed preflight preserves selected receiver for explicit retry"
        );
        assert!(world.drawable_map.contains_key(&peer.drawable_id));
        NativeParticleRetirementService
            .retire(&mut world.borrow_native(), None)
            .unwrap();
        assert!(world.drawable_map.is_empty());
    }
    #[test]
    fn native_early_body_flags_precede_audio_and_loading_map_needs_no_audio_receiver() {
        use crate::system::TimeOfDay;
        use game_engine::common::audio::AudioSubmissionLocality;
        use gamelogic::common::types::BodyDamageType;
        struct Audio(Vec<(BodyDamageType, TimeOfDay)>);
        impl crate::drawable::drawable::NativeAmbientAudio for Audio {
            fn restart_ambient(
                &mut self,
                drawable: &mut BasicDrawable,
                state: BodyDamageType,
                tod: TimeOfDay,
                _: &AudioSubmissionLocality,
            ) -> Result<(), String> {
                let bit = if state == BodyDamageType::Damaged {
                    game_engine::common::bit_flags::ModelConditionFlags::DAMAGED
                } else {
                    game_engine::common::bit_flags::ModelConditionFlags::REALLYDAMAGED
                };
                assert!(
                    drawable.get_model_condition_flags().test(bit as usize),
                    "model flags precede ambient service admission"
                );
                self.0.push((state, tod));
                Ok(())
            }
        }
        // Actual local AudioManager supplies only immutable locality policy;
        // its public resolver implementation is not required by this receiver.
        struct Players;
        impl game_engine::common::audio::AudioLocalityResolver for Players {
            fn get_local_player_index(&self) -> Option<i32> {
                Some(0)
            }
            fn is_player_active(&self, _: i32) -> bool {
                true
            }
            fn player_exists(&self, _: i32) -> bool {
                true
            }
            fn has_default_team(&self, _: i32) -> bool {
                true
            }
            fn get_relationship_to_local_team(
                &self,
                _: i32,
                _: i32,
            ) -> game_engine::common::audio::AudioLocalityRelationship {
                game_engine::common::audio::AudioLocalityRelationship::Allies
            }
        }
        let (mut catalog, plan) = content(2.0);
        let mut client = GameClient::new().unwrap();
        let key = client
            .split_frame_snow_and_drawable_owner()
            .2
            .admit_semantic_drawable_with_content(
                entry(77, plan),
                NativeDrawableBirthContext {
                    loading_map: true,
                    in_load_game: false,
                },
                0,
                &mut catalog,
                policy(),
                &mut NativeParticleRetirementService,
                |_| Ok(()),
            )
            .unwrap();
        let locality = AudioSubmissionLocality::from_resolver(0, &Players);
        client
            .split_frame_snow_and_drawable_owner()
            .2
            .body_state_changed_with_audio(
                key,
                BodyDamageType::Damaged,
                true,
                TimeOfDay::Night,
                None,
                &locality,
            )
            .unwrap();
        let mut audio = Audio(Vec::new());
        client
            .split_frame_snow_and_drawable_owner()
            .2
            .body_state_changed_with_audio(
                key,
                BodyDamageType::ReallyDamaged,
                false,
                TimeOfDay::Morning,
                Some(&mut audio),
                &locality,
            )
            .unwrap();
        assert_eq!(
            audio.0,
            vec![(BodyDamageType::ReallyDamaged, TimeOfDay::Morning)]
        );
        assert!(
            client
                .split_frame_snow_and_drawable_owner()
                .2
                .body_state_changed_with_audio(
                    key,
                    BodyDamageType::Rubble,
                    false,
                    TimeOfDay::Night,
                    None,
                    &locality
                )
                .is_err()
        );
    }
}

impl GameClient {
    fn fresh_drawable_requires_science(
        e: &PresentationDrawableSync,
        policy: Option<ReceivingDrawablePolicy>,
    ) -> bool {
        e.module_admission
            .authored_template()
            .is_some_and(|template| {
                template.draw_modules().iter().any(|entry| {
                    !policy.is_some_and(|policy| {
                        policy.use_draw_module_lod
                            && (entry.data.get_minimum_required_game_lod() as i32)
                                > (policy.static_game_lod as i32)
                    }) && entry
                        .data
                        .as_any()
                        .downcast_ref::<W3DScienceModelDrawModuleData>()
                        .is_some_and(|data| {
                            data.required_science != game_engine::common::rts::SCIENCE_INVALID
                        })
                })
            })
    }
}

impl NativeDrawableOwner<'_> {
    pub fn collect_projected_decal_items(
        &self,
        use_shadow_decals: bool,
    ) -> Vec<crate::effects::decals::DecalRenderItem> {
        self.drawables
            .projected_decals
            .manager
            .collect_render_items_with_policy(use_shadow_decals)
    }

    /// RadiusDecal is a client resource and does not require an Object→Drawable
    /// association (constructor and objectless use are both original cases).
    pub fn allocate_projected_radius(
        &mut self,
        texture: &str,
        radius: f32,
        position: [f32; 3],
        color: u32,
        opacity: f32,
    ) -> Option<crate::radius_decal::ShadowHandle> {
        crate::radius_decal::enqueue_delivery_decal_argb_with_owner(
            &mut self.drawables.projected_decals.manager,
            texture,
            radius,
            position[0],
            position[1],
            position[2],
            color,
            opacity,
        )
    }
}

impl GameClient {
    /// Immutable render input from this actual client's live projected owner.
    /// Current native production allocates Radius addDecal resources here;
    /// model-shadow callbacks remain a separately held migration.
    pub fn collect_native_radius_render_items(
        &mut self,
    ) -> Vec<crate::effects::decals::DecalRenderItem> {
        // Completed logic released only its own handles. Compact this manager
        // before freezing frame output; no legacy manager is involved.
        self.drawables.projected_decals.manager.cleanup();
        self.drawables
            .projected_decals
            .manager
            .collect_render_items_with_policy(false)
    }
}
